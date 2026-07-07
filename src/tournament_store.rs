use std::path::Path;
use std::sync::Mutex;

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, Transaction};
use serde::Deserialize;

use crate::match_record::now_unix;
use crate::migrate::migrate;
use crate::player::MatchOutcome;
use crate::store::normalize_name;
use crate::tournament::{
    compute_elo_deltas, placement_label, pool_round_robin_pairs, tournament_points_for_player,
    BracketFormat, PlayerTournamentResult, Pool, PoolPlayer, RegistrationStatus,
    Tournament, TournamentDetail, TournamentMatch, TournamentMatchStatus, TournamentPhase,
    TournamentPlayerSnapshot, TournamentRegistration, TournamentStatus, POOLS_EIGHT_CAPACITY,
    POOLS_FOUR_CAPACITY, WAITLIST_THRESHOLD,
};

pub struct TournamentStore {
    conn: Mutex<Connection>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTournamentRequest {
    pub name: String,
    #[serde(default = "default_bracket_format")]
    pub bracket_format: String,
}

fn default_bracket_format() -> String {
    "quarters_direct".into()
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub army_id: u32,
}

#[derive(Debug, Deserialize)]
pub struct AdminRegisterRequest {
    pub player_name: String,
    pub army_id: u32,
}

#[derive(Debug, Deserialize)]
pub struct SetupPoolsRequest {
    pub pools: Vec<PoolSetup>,
}

#[derive(Debug, Deserialize)]
pub struct PoolSetup {
    pub name: String,
    pub position: u8,
    pub players: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SubmitMatchRequest {
    pub player1_objectives: u8,
    pub player2_objectives: u8,
    #[serde(default)]
    pub player1_survivors: u16,
    #[serde(default)]
    pub player2_survivors: u16,
    #[serde(default)]
    pub player1_army_id: Option<u32>,
    #[serde(default)]
    pub player2_army_id: Option<u32>,
    #[serde(default)]
    pub scenario_id: Option<i64>,
    #[serde(default)]
    pub scenario_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ForfeitRequest {
    pub forfeit_player: String,
}

impl TournamentStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("impossible de créer {}", parent.display()))?;
        }

        let conn = Connection::open(path)
            .with_context(|| format!("impossible d'ouvrir {}", path.display()))?;
        migrate(&conn)?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn list(&self) -> Result<Vec<Tournament>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "
            SELECT id, name, status, pool_count, bracket_format,
                   created_at, started_at, pools_finalized_at, completed_at
            FROM tournaments
            ORDER BY created_at DESC
            ",
        )?;
        let rows = stmt.query_map([], row_to_tournament)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn create(&self, request: &CreateTournamentRequest) -> Result<Tournament> {
        let name = request.name.trim();
        if name.is_empty() {
            bail!("indiquez un nom de tournoi");
        }

        let bracket_format = BracketFormat::parse(&request.bracket_format)
            .ok_or_else(|| anyhow::anyhow!("format d'arbre invalide"))?;

        let now = now_unix();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "
            INSERT INTO tournaments (name, status, pool_count, bracket_format, created_at)
            VALUES (?1, ?2, 4, ?3, ?4)
            ",
            params![name, TournamentStatus::Draft.as_str(), bracket_format.as_str(), now],
        )?;
        let id = conn.last_insert_rowid();
        self.get_in_conn(&conn, id)?
            .context("tournoi introuvable après création")
    }

    pub fn get(&self, id: i64) -> Result<Option<Tournament>> {
        let conn = self.conn.lock().unwrap();
        self.get_in_conn(&conn, id)
    }

    pub fn get_detail(&self, id: i64) -> Result<Option<TournamentDetail>> {
        let conn = self.conn.lock().unwrap();
        let Some(tournament) = self.get_in_conn(&conn, id)? else {
            return Ok(None);
        };

        Ok(Some(TournamentDetail {
            registrations: self.list_registrations_in_conn(&conn, id)?,
            players: self.list_snapshots_in_conn(&conn, id)?,
            pools: self.list_pools_in_conn(&conn, id)?,
            matches: self.list_matches_in_conn(&conn, id)?,
            tournament,
        }))
    }

    pub fn open_registration(&self, id: i64) -> Result<Tournament> {
        let conn = self.conn.lock().unwrap();
        let tournament = self.get_in_conn(&conn, id)?.context("tournoi introuvable")?;
        if tournament.status != TournamentStatus::Draft
            && tournament.status != TournamentStatus::RegistrationClosed
        {
            bail!("impossible d'ouvrir les inscriptions dans cet état");
        }
        conn.execute(
            "UPDATE tournaments SET status = ?1 WHERE id = ?2",
            params![TournamentStatus::RegistrationOpen.as_str(), id],
        )?;
        self.get_in_conn(&conn, id)?.context("tournoi introuvable")
    }

    pub fn close_registration(&self, id: i64) -> Result<Tournament> {
        let conn = self.conn.lock().unwrap();
        let tournament = self.get_in_conn(&conn, id)?.context("tournoi introuvable")?;
        if tournament.status != TournamentStatus::RegistrationOpen {
            bail!("les inscriptions ne sont pas ouvertes");
        }
        conn.execute(
            "UPDATE tournaments SET status = ?1 WHERE id = ?2",
            params![TournamentStatus::RegistrationClosed.as_str(), id],
        )?;
        self.get_in_conn(&conn, id)?.context("tournoi introuvable")
    }

    pub fn register(
        &self,
        tournament_id: i64,
        player_name: &str,
        user_id: i64,
        army_id: u32,
    ) -> Result<TournamentRegistration> {
        let conn = self.conn.lock().unwrap();
        let tournament = self
            .get_in_conn(&conn, tournament_id)?
            .context("tournoi introuvable")?;
        if tournament.status != TournamentStatus::RegistrationOpen {
            bail!("les inscriptions ne sont pas ouvertes");
        }

        let key = normalize_name(player_name);
        if self.registration_for_player_in_conn(&conn, tournament_id, &key)?.is_some() {
            bail!("vous êtes déjà inscrit à ce tournoi");
        }

        let now = now_unix();
        conn.execute(
            "
            INSERT INTO tournament_registrations
                (tournament_id, player_name_key, player_name, user_id, status, requested_at, army_id)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ",
            params![
                tournament_id,
                key,
                player_name,
                user_id,
                RegistrationStatus::Pending.as_str(),
                now,
                army_id,
            ],
        )?;
        let reg_id = conn.last_insert_rowid();
        self.get_registration_in_conn(&conn, reg_id)?
            .context("inscription introuvable")
    }

    pub fn admin_register(
        &self,
        tournament_id: i64,
        player_name: &str,
        admin_id: i64,
        army_id: u32,
    ) -> Result<TournamentRegistration> {
        let conn = self.conn.lock().unwrap();
        let tournament = self
            .get_in_conn(&conn, tournament_id)?
            .context("tournoi introuvable")?;
        if !matches!(
            tournament.status,
            TournamentStatus::RegistrationOpen | TournamentStatus::RegistrationClosed
        ) {
            bail!("inscriptions fermées pour ce tournoi");
        }

        let key = normalize_name(player_name);
        if self.registration_for_player_in_conn(&conn, tournament_id, &key)?.is_some() {
            bail!("ce joueur est déjà inscrit");
        }

        let now = now_unix();
        conn.execute(
            "
            INSERT INTO tournament_registrations
                (tournament_id, player_name_key, player_name, status, requested_at, reviewed_at, reviewed_by, army_id)
            VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6, ?7)
            ",
            params![
                tournament_id,
                key,
                player_name,
                RegistrationStatus::Pending.as_str(),
                now,
                admin_id,
                army_id,
            ],
        )?;
        let reg_id = conn.last_insert_rowid();
        self.review_registration_in_tx(&conn, reg_id, "approved", admin_id)
    }

    pub fn review_registration(
        &self,
        registration_id: i64,
        action: &str,
        admin_id: i64,
    ) -> Result<TournamentRegistration> {
        let conn = self.conn.lock().unwrap();
        self.review_registration_in_tx(&conn, registration_id, action, admin_id)
    }

    fn review_registration_in_tx(
        &self,
        conn: &Connection,
        registration_id: i64,
        action: &str,
        admin_id: i64,
    ) -> Result<TournamentRegistration> {
        let registration = self
            .get_registration_in_conn(conn, registration_id)?
            .context("inscription introuvable")?;

        if registration.status != RegistrationStatus::Pending
            && registration.status != RegistrationStatus::Waitlisted
            && action != "approved"
        {
            // allow re-review from waitlisted when promoting
        }

        let now = now_unix();
        match action {
            "approved" => self.approve_registration(conn, &registration, admin_id, now)?,
            "rejected" => {
                conn.execute(
                    "
                    UPDATE tournament_registrations
                    SET status = ?1, reviewed_at = ?2, reviewed_by = ?3, waitlist_position = NULL
                    WHERE id = ?4
                    ",
                    params![
                        RegistrationStatus::Rejected.as_str(),
                        now,
                        admin_id,
                        registration_id
                    ],
                )?;
            }
            _ => bail!("action invalide"),
        }

        self.get_registration_in_conn(conn, registration_id)?
            .context("inscription introuvable")
    }

    fn approve_registration(
        &self,
        conn: &Connection,
        registration: &TournamentRegistration,
        admin_id: i64,
        now: u64,
    ) -> Result<()> {
        let tournament = self
            .get_in_conn(conn, registration.tournament_id)?
            .context("tournoi introuvable")?;

        let approved_count: i64 = conn.query_row(
            "
            SELECT COUNT(*) FROM tournament_registrations
            WHERE tournament_id = ?1 AND status = 'approved'
            ",
            params![registration.tournament_id],
            |row| row.get(0),
        )?;

        let waitlisted_count: i64 = conn.query_row(
            "
            SELECT COUNT(*) FROM tournament_registrations
            WHERE tournament_id = ?1 AND status = 'waitlisted'
            ",
            params![registration.tournament_id],
            |row| row.get(0),
        )?;

        let total_active = approved_count + waitlisted_count;

        if tournament.pool_count == 4 {
            if total_active >= WAITLIST_THRESHOLD as i64 {
                bail!("le tournoi est complet (32 joueurs)");
            }

            if approved_count >= POOLS_FOUR_CAPACITY as i64 {
                let waitlist_position = waitlisted_count + 1;
                conn.execute(
                    "
                    UPDATE tournament_registrations
                    SET status = ?1, reviewed_at = ?2, reviewed_by = ?3, waitlist_position = ?4
                    WHERE id = ?5
                    ",
                    params![
                        RegistrationStatus::Waitlisted.as_str(),
                        now,
                        admin_id,
                        waitlist_position,
                        registration.id,
                    ],
                )?;

                if total_active + 1 == WAITLIST_THRESHOLD as i64 {
                    self.switch_to_eight_pools(conn, registration.tournament_id, now, admin_id)?;
                }
                return Ok(());
            }

            conn.execute(
                "
                UPDATE tournament_registrations
                SET status = ?1, reviewed_at = ?2, reviewed_by = ?3, waitlist_position = NULL
                WHERE id = ?4
                ",
                params![
                    RegistrationStatus::Approved.as_str(),
                    now,
                    admin_id,
                    registration.id,
                ],
            )?;

            if total_active + 1 == WAITLIST_THRESHOLD as i64 {
                self.switch_to_eight_pools(conn, registration.tournament_id, now, admin_id)?;
            }
            return Ok(());
        }

        if approved_count >= POOLS_EIGHT_CAPACITY as i64 {
            bail!("le tournoi est complet (48 joueurs)");
        }

        conn.execute(
            "
            UPDATE tournament_registrations
            SET status = ?1, reviewed_at = ?2, reviewed_by = ?3, waitlist_position = NULL
            WHERE id = ?4
            ",
            params![
                RegistrationStatus::Approved.as_str(),
                now,
                admin_id,
                registration.id,
            ],
        )?;
        Ok(())
    }

    fn switch_to_eight_pools(
        &self,
        conn: &Connection,
        tournament_id: i64,
        now: u64,
        admin_id: i64,
    ) -> Result<()> {
        conn.execute(
            "
            UPDATE tournaments
            SET pool_count = 8, bracket_format = ?1
            WHERE id = ?2
            ",
            params![BracketFormat::RoundOf16Full.as_str(), tournament_id],
        )?;

        conn.execute(
            "
            UPDATE tournament_registrations
            SET status = ?1, reviewed_at = ?2, reviewed_by = ?3, waitlist_position = NULL
            WHERE tournament_id = ?4 AND status = ?5
            ",
            params![
                RegistrationStatus::Approved.as_str(),
                now,
                admin_id,
                tournament_id,
                RegistrationStatus::Waitlisted.as_str(),
            ],
        )?;
        Ok(())
    }

    pub fn start(
        &self,
        tournament_id: i64,
        player_ratings: &[(String, f64)],
    ) -> Result<Tournament> {
        let conn = self.conn.lock().unwrap();
        let tournament = self
            .get_in_conn(&conn, tournament_id)?
            .context("tournoi introuvable")?;

        if tournament.status != TournamentStatus::RegistrationClosed
            && tournament.status != TournamentStatus::RegistrationOpen
        {
            bail!("impossible de démarrer le tournoi dans cet état");
        }

        let approved: Vec<(String, String)> = conn.prepare(
            "
            SELECT player_name_key, player_name FROM tournament_registrations
            WHERE tournament_id = ?1 AND status = 'approved'
            ",
        )?
        .query_map(params![tournament_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

        if approved.is_empty() {
            bail!("aucun joueur validé");
        }

        let now = now_unix();
        let tx = conn.unchecked_transaction()?;

        for (key, name) in &approved {
            let rating = player_ratings
                .iter()
                .find(|(n, _)| normalize_name(n) == *key)
                .map(|(_, r)| *r)
                .unwrap_or(crate::player::DEFAULT_RATING);

            tx.execute(
                "
                INSERT OR REPLACE INTO tournament_players
                    (tournament_id, player_name_key, player_name, start_rating, bracket_rating)
                VALUES (?1, ?2, ?3, ?4, ?4)
                ",
                params![tournament_id, key, name, rating],
            )?;
        }

        tx.execute(
            "
            UPDATE tournaments SET status = ?1, started_at = ?2
            WHERE id = ?3
            ",
            params![TournamentStatus::Started.as_str(), now, tournament_id],
        )?;

        tx.commit()?;
        self.get_in_conn(&conn, tournament_id)?
            .context("tournoi introuvable")
    }

    pub fn setup_pools(&self, tournament_id: i64, request: &SetupPoolsRequest) -> Result<Vec<Pool>> {
        let conn = self.conn.lock().unwrap();
        let tournament = self
            .get_in_conn(&conn, tournament_id)?
            .context("tournoi introuvable")?;

        if tournament.status != TournamentStatus::Started {
            bail!("le tournoi n'est pas démarré");
        }

        if request.pools.len() != tournament.pool_count as usize {
            bail!(
                "attendu {} poules, reçu {}",
                tournament.pool_count,
                request.pools.len()
            );
        }

        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM tournament_matches WHERE tournament_id = ?1 AND phase = 'pool'",
            params![tournament_id],
        )?;
        tx.execute(
            "DELETE FROM pool_players WHERE pool_id IN (SELECT id FROM pools WHERE tournament_id = ?1)",
            params![tournament_id],
        )?;
        tx.execute(
            "DELETE FROM pools WHERE tournament_id = ?1",
            params![tournament_id],
        )?;

        for pool_setup in &request.pools {
            if pool_setup.players.len() > 6 {
                bail!("maximum 6 joueurs par poule");
            }
            tx.execute(
                "
                INSERT INTO pools (tournament_id, name, position)
                VALUES (?1, ?2, ?3)
                ",
                params![tournament_id, pool_setup.name, pool_setup.position],
            )?;
            let pool_id = tx.last_insert_rowid();

            for (seed, player_name) in pool_setup.players.iter().enumerate() {
                let key = normalize_name(player_name);
                tx.execute(
                    "
                    INSERT INTO pool_players (pool_id, player_name_key, player_name, seed)
                    VALUES (?1, ?2, ?3, ?4)
                    ",
                    params![pool_id, key, player_name, seed as u8],
                )?;
            }
        }

        tx.commit()?;
        self.list_pools_in_conn(&conn, tournament_id)
    }

    pub fn generate_pool_matches(&self, tournament_id: i64) -> Result<Vec<TournamentMatch>> {
        let conn = self.conn.lock().unwrap();
        let pools = self.list_pools_in_conn(&conn, tournament_id)?;

        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM tournament_matches WHERE tournament_id = ?1 AND phase = 'pool'",
            params![tournament_id],
        )?;

        for pool in &pools {
            let pairs = pool_round_robin_pairs(pool.players.len());
            for (i, j) in pairs {
                let p1 = &pool.players[i];
                let p2 = &pool.players[j];
                tx.execute(
                    "
                    INSERT INTO tournament_matches
                        (tournament_id, phase, pool_id, player1, player2, status)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                    ",
                    params![
                        tournament_id,
                        TournamentPhase::Pool.as_str(),
                        pool.id,
                        p1.player_name,
                        p2.player_name,
                        TournamentMatchStatus::Scheduled.as_str(),
                    ],
                )?;
            }
        }

        tx.commit()?;
        self.list_matches_in_conn(&conn, tournament_id)
    }

    pub fn get_match(&self, match_id: i64) -> Result<Option<TournamentMatch>> {
        let conn = self.conn.lock().unwrap();
        self.get_match_in_conn(&conn, match_id)
    }

    pub fn submit_match(
        &self,
        match_id: i64,
        request: &SubmitMatchRequest,
        user_id: i64,
        is_admin: bool,
        user_player_name: Option<&str>,
        k_factor: f64,
    ) -> Result<TournamentMatch> {
        let conn = self.conn.lock().unwrap();
        let mut tm = self
            .get_match_in_conn(&conn, match_id)?
            .context("match introuvable")?;

        if tm.status != TournamentMatchStatus::Scheduled
            && tm.status != TournamentMatchStatus::Submitted
        {
            bail!("ce match ne peut plus être modifié");
        }

        let p1 = tm.player1.clone().context("joueur 1 manquant")?;
        let p2 = tm.player2.clone().context("joueur 2 manquant")?;

        if !is_admin {
            let player_name = user_player_name.context("compte joueur requis")?;
            if normalize_name(player_name) != normalize_name(&p1)
                && normalize_name(player_name) != normalize_name(&p2)
            {
                bail!("vous ne participez pas à ce match");
            }
        }

        if request.player1_objectives > 10 || request.player2_objectives > 10 {
            bail!("objectifs invalides");
        }

        let outcome = if request.player1_objectives > request.player2_objectives {
            MatchOutcome::Player1Win
        } else if request.player2_objectives > request.player1_objectives {
            MatchOutcome::Player2Win
        } else {
            MatchOutcome::Draw
        };

        let tp1 = tournament_points_for_player(
            outcome,
            true,
            request.player1_objectives,
            request.player2_objectives,
            false,
        );
        let tp2 = tournament_points_for_player(
            outcome,
            false,
            request.player1_objectives,
            request.player2_objectives,
            false,
        );

        let (rating1, rating2) = self.ratings_for_match_in_conn(&conn, &tm, k_factor)?;
        let (delta1, delta2) = if tm.phase == TournamentPhase::Pool {
            compute_elo_deltas(rating1, rating2, outcome, k_factor)
        } else {
            compute_elo_deltas(rating1, rating2, outcome, k_factor)
        };

        let now = now_unix();
        let auto_confirm = is_admin;

        conn.execute(
            "
            UPDATE tournament_matches SET
                player1_objectives = ?1, player2_objectives = ?2,
                player1_survivors = ?3, player2_survivors = ?4,
                player1_tournament_points = ?5, player2_tournament_points = ?6,
                outcome = ?7, is_forfeit = 0, forfeit_player = NULL,
                player1_elo_delta = ?8, player2_elo_delta = ?9,
                player1_rating_used = ?10, player2_rating_used = ?11,
                status = ?12, submitted_by_user_id = ?13, submitted_at = ?14,
                confirmed_by_user_id = ?15, confirmed_at = ?16,
                scenario_id = ?17, scenario_name = ?18,
                player1_army_id = ?19, player2_army_id = ?20,
                played_at = ?21
            WHERE id = ?22
            ",
            params![
                request.player1_objectives,
                request.player2_objectives,
                request.player1_survivors,
                request.player2_survivors,
                tp1,
                tp2,
                outcome_to_str(outcome),
                delta1,
                delta2,
                rating1,
                rating2,
                if auto_confirm {
                    TournamentMatchStatus::Confirmed.as_str()
                } else {
                    TournamentMatchStatus::Submitted.as_str()
                },
                user_id,
                now,
                if auto_confirm { Some(user_id) } else { None::<i64> },
                if auto_confirm { Some(now) } else { None::<u64> },
                request.scenario_id,
                request.scenario_name,
                request.player1_army_id,
                request.player2_army_id,
                now,
                match_id,
            ],
        )?;

        tm = self
            .get_match_in_conn(&conn, match_id)?
            .context("match introuvable")?;

        if auto_confirm {
            drop(conn);
            self.apply_confirmed_match(match_id, k_factor)?;
            return self.get_match(match_id)?.context("match introuvable");
        }

        Ok(tm)
    }

    pub fn confirm_match(
        &self,
        match_id: i64,
        user_id: i64,
        is_admin: bool,
        user_player_name: Option<&str>,
        k_factor: f64,
    ) -> Result<TournamentMatch> {
        let conn = self.conn.lock().unwrap();
        let tm = self
            .get_match_in_conn(&conn, match_id)?
            .context("match introuvable")?;

        if tm.status != TournamentMatchStatus::Submitted {
            bail!("aucune soumission en attente");
        }

        if !is_admin {
            let player_name = user_player_name.context("compte joueur requis")?;
            let p1 = tm.player1.as_deref().unwrap_or("");
            let p2 = tm.player2.as_deref().unwrap_or("");
            let submitter = tm.submitted_by_user_id;
            let is_participant = normalize_name(player_name) == normalize_name(p1)
                || normalize_name(player_name) == normalize_name(p2);
            if !is_participant {
                bail!("seul un participant ou un admin peut confirmer");
            }
            if submitter == Some(user_id) {
                bail!("vous ne pouvez pas confirmer votre propre saisie");
            }
        }

        let now = now_unix();
        conn.execute(
            "
            UPDATE tournament_matches
            SET status = ?1, confirmed_by_user_id = ?2, confirmed_at = ?3
            WHERE id = ?4
            ",
            params![
                TournamentMatchStatus::Confirmed.as_str(),
                user_id,
                now,
                match_id,
            ],
        )?;
        drop(conn);

        self.apply_confirmed_match(match_id, k_factor)?;
        self.get_match(match_id)?.context("match introuvable")
    }

    pub fn declare_forfeit(
        &self,
        match_id: i64,
        forfeit_player: &str,
        user_id: i64,
        is_admin: bool,
    ) -> Result<TournamentMatch> {
        if !is_admin {
            bail!("seul un admin peut déclarer un forfait directement");
        }

        let conn = self.conn.lock().unwrap();
        let tm = self
            .get_match_in_conn(&conn, match_id)?
            .context("match introuvable")?;

        let p1 = tm.player1.clone().context("joueur 1 manquant")?;
        let p2 = tm.player2.clone().context("joueur 2 manquant")?;
        let ff_key = normalize_name(forfeit_player);

        let (winner, loser) = if ff_key == normalize_name(&p1) {
            (p2.clone(), p1.clone())
        } else if ff_key == normalize_name(&p2) {
            (p1.clone(), p2.clone())
        } else {
            bail!("joueur forfait invalide");
        };

        let now = now_unix();
        conn.execute(
            "
            UPDATE tournament_matches SET
                player1_objectives = 0, player2_objectives = 0,
                player1_survivors = 0, player2_survivors = 0,
                player1_tournament_points = ?1, player2_tournament_points = ?2,
                outcome = ?3, is_forfeit = 1, forfeit_player = ?4,
                player1_elo_delta = 0, player2_elo_delta = 0,
                status = ?5, confirmed_by_user_id = ?6, confirmed_at = ?7,
                played_at = ?7
            WHERE id = ?8
            ",
            params![
                if normalize_name(&p1) == normalize_name(&winner) {
                    5
                } else {
                    0
                },
                if normalize_name(&p2) == normalize_name(&winner) {
                    5
                } else {
                    0
                },
                if normalize_name(&p1) == normalize_name(&winner) {
                    "player1_win"
                } else {
                    "player2_win"
                },
                loser,
                TournamentMatchStatus::Confirmed.as_str(),
                user_id,
                now,
                match_id,
            ],
        )?;
        drop(conn);

        self.apply_confirmed_match(match_id, 32.0)?;
        self.get_match(match_id)?.context("match introuvable")
    }

    fn apply_confirmed_match(&self, match_id: i64, _k_factor: f64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let tm = self
            .get_match_in_conn(&conn, match_id)?
            .context("match introuvable")?;

        if tm.status != TournamentMatchStatus::Confirmed {
            return Ok(());
        }

        if tm.is_forfeit {
            self.update_pool_standings_in_conn(&conn, &tm)?;
            if tm.phase != TournamentPhase::Pool {
                self.advance_bracket_in_conn(&conn, &tm)?;
            }
            return Ok(());
        }

        if tm.phase == TournamentPhase::Pool {
            self.update_pool_standings_in_conn(&conn, &tm)?;
            let p1_key = normalize_name(tm.player1.as_ref().unwrap());
            let p2_key = normalize_name(tm.player2.as_ref().unwrap());

            conn.execute(
                "
                UPDATE tournament_players SET pool_elo_delta = pool_elo_delta + ?1
                WHERE tournament_id = ?2 AND player_name_key = ?3
                ",
                params![tm.player1_elo_delta, tm.tournament_id, p1_key],
            )?;
            conn.execute(
                "
                UPDATE tournament_players SET pool_elo_delta = pool_elo_delta + ?2
                WHERE tournament_id = ?1 AND player_name_key = ?3
                ",
                params![tm.tournament_id, tm.player2_elo_delta, p2_key],
            )?;
            return Ok(());
        }

        self.update_pool_standings_in_conn(&conn, &tm)?;
        self.advance_bracket_in_conn(&conn, &tm)?;
        Ok(())
    }

    pub fn finalize_pools(&self, tournament_id: i64) -> Result<Tournament> {
        let conn = self.conn.lock().unwrap();
        let tournament = self
            .get_in_conn(&conn, tournament_id)?
            .context("tournoi introuvable")?;

        if tournament.status != TournamentStatus::Started {
            bail!("tournoi non démarré");
        }

        let unconfirmed: i64 = conn.query_row(
            "
            SELECT COUNT(*) FROM tournament_matches
            WHERE tournament_id = ?1 AND phase = 'pool' AND status != 'confirmed'
            ",
            params![tournament_id],
            |row| row.get(0),
        )?;
        if unconfirmed > 0 {
            bail!("{unconfirmed} match(s) de poule non confirmé(s)");
        }

        let snapshots = self.list_snapshots_in_conn(&conn, tournament_id)?;
        for snap in &snapshots {
            let bracket_rating = snap.start_rating + snap.pool_elo_delta;
            conn.execute(
                "
                UPDATE tournament_players SET bracket_rating = ?1
                WHERE tournament_id = ?2 AND player_name_key = ?3
                ",
                params![
                    bracket_rating,
                    tournament_id,
                    normalize_name(&snap.player_name),
                ],
            )?;
        }

        let now = now_unix();
        conn.execute(
            "
            UPDATE tournaments SET pools_finalized_at = ?1 WHERE id = ?2
            ",
            params![now, tournament_id],
        )?;

        drop(conn);
        self.generate_bracket(tournament_id)?;
        self.get(tournament_id)?.context("tournoi introuvable")
    }

    pub fn generate_bracket(&self, tournament_id: i64) -> Result<Vec<TournamentMatch>> {
        let conn = self.conn.lock().unwrap();
        let tournament = self
            .get_in_conn(&conn, tournament_id)?
            .context("tournoi introuvable")?;

        let pools = self.list_pools_in_conn(&conn, tournament_id)?;
        let pool_standings: Vec<(i64, Vec<PoolPlayer>)> = pools
            .iter()
            .map(|pool| {
                let mut sorted = pool.players.clone();
                sorted.sort_by(|a, b| {
                    b.points
                        .cmp(&a.points)
                        .then_with(|| b.objectives.cmp(&a.objectives))
                        .then_with(|| b.survivors.cmp(&a.survivors))
                });
                (pool.id, sorted)
            })
            .collect();

        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM tournament_matches WHERE tournament_id = ?1 AND phase != 'pool'",
            params![tournament_id],
        )?;

        match tournament.bracket_format {
            BracketFormat::QuartersDirect => {
                self.insert_quarters_direct(&tx, tournament_id, &pool_standings)?;
            }
            BracketFormat::RoundOf16 => {
                self.insert_round_of_16_with_byes(&tx, tournament_id, &pool_standings)?;
            }
            BracketFormat::RoundOf16Full => {
                self.insert_round_of_16_full(&tx, tournament_id, &pool_standings)?;
            }
        }

        tx.commit()?;
        self.list_matches_in_conn(&conn, tournament_id)
    }

    fn insert_quarters_direct(
        &self,
        tx: &Transaction,
        tournament_id: i64,
        standings: &[(i64, Vec<PoolPlayer>)],
    ) -> Result<()> {
        if standings.len() != 4 {
            bail!("4 poules requises");
        }
        let mut firsts = Vec::new();
        let mut seconds = Vec::new();
        for (_, players) in standings {
            let first = players.first().context("poule vide")?;
            let second = players.get(1).context("pas assez de joueurs en poule")?;
            firsts.push(first.player_name.clone());
            seconds.push(second.player_name.clone());
        }

        let qf_pairs = [
            (&firsts[0], &seconds[1]),
            (&firsts[1], &seconds[0]),
            (&firsts[2], &seconds[3]),
            (&firsts[3], &seconds[2]),
        ];

        for (slot, (p1, p2)) in qf_pairs.iter().enumerate() {
            tx.execute(
                "
                INSERT INTO tournament_matches
                    (tournament_id, phase, bracket_slot, player1, player2, status)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ",
                params![
                    tournament_id,
                    TournamentPhase::Quarter.as_str(),
                    slot as u32,
                    p1,
                    p2,
                    TournamentMatchStatus::Scheduled.as_str(),
                ],
            )?;
        }

        for slot in 0..2 {
            tx.execute(
                "
                INSERT INTO tournament_matches
                    (tournament_id, phase, bracket_slot, status)
                VALUES (?1, ?2, ?3, ?4)
                ",
                params![
                    tournament_id,
                    TournamentPhase::Semi.as_str(),
                    slot,
                    TournamentMatchStatus::Scheduled.as_str(),
                ],
            )?;
        }

        tx.execute(
            "
            INSERT INTO tournament_matches
                (tournament_id, phase, bracket_slot, status)
            VALUES (?1, ?2, 0, ?3)
            ",
            params![
                tournament_id,
                TournamentPhase::Final.as_str(),
                TournamentMatchStatus::Scheduled.as_str(),
            ],
        )?;
        Ok(())
    }

    fn insert_round_of_16_with_byes(
        &self,
        tx: &Transaction,
        tournament_id: i64,
        standings: &[(i64, Vec<PoolPlayer>)],
    ) -> Result<()> {
        if standings.len() != 4 {
            bail!("4 poules requises");
        }

        for (slot, (_, players)) in standings.iter().enumerate() {
            let second = players.get(1).context("2e manquant")?;
            let third = players.get(2).context("3e manquant")?;
            tx.execute(
                "
                INSERT INTO tournament_matches
                    (tournament_id, phase, bracket_slot, player1, player2, status)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ",
                params![
                    tournament_id,
                    TournamentPhase::RoundOf16.as_str(),
                    slot as u32,
                    second.player_name,
                    third.player_name,
                    TournamentMatchStatus::Scheduled.as_str(),
                ],
            )?;
        }

        for slot in 0..4 {
            tx.execute(
                "
                INSERT INTO tournament_matches
                    (tournament_id, phase, bracket_slot, status)
                VALUES (?1, ?2, ?3, ?4)
                ",
                params![
                    tournament_id,
                    TournamentPhase::Quarter.as_str(),
                    slot,
                    TournamentMatchStatus::Scheduled.as_str(),
                ],
            )?;
        }

        for slot in 0..2 {
            tx.execute(
                "
                INSERT INTO tournament_matches
                    (tournament_id, phase, bracket_slot, status)
                VALUES (?1, ?2, ?3, ?4)
                ",
                params![
                    tournament_id,
                    TournamentPhase::Semi.as_str(),
                    slot,
                    TournamentMatchStatus::Scheduled.as_str(),
                ],
            )?;
        }

        tx.execute(
            "
            INSERT INTO tournament_matches
                (tournament_id, phase, bracket_slot, status)
            VALUES (?1, ?2, 0, ?3)
            ",
            params![
                tournament_id,
                TournamentPhase::Final.as_str(),
                TournamentMatchStatus::Scheduled.as_str(),
            ],
        )?;
        Ok(())
    }

    fn insert_round_of_16_full(
        &self,
        tx: &Transaction,
        tournament_id: i64,
        standings: &[(i64, Vec<PoolPlayer>)],
    ) -> Result<()> {
        if standings.len() != 8 {
            bail!("8 poules requises");
        }

        let mut firsts = Vec::new();
        let mut seconds = Vec::new();
        for (_, players) in standings {
            firsts.push(
                players
                    .first()
                    .context("poule vide")?
                    .player_name
                    .clone(),
            );
            seconds.push(
                players
                    .get(1)
                    .context("2e manquant")?
                    .player_name
                    .clone(),
            );
        }

        let pairs = [
            (&firsts[0], &seconds[1]),
            (&firsts[1], &seconds[0]),
            (&firsts[2], &seconds[3]),
            (&firsts[3], &seconds[2]),
            (&firsts[4], &seconds[5]),
            (&firsts[5], &seconds[4]),
            (&firsts[6], &seconds[7]),
            (&firsts[7], &seconds[6]),
        ];

        for (slot, (p1, p2)) in pairs.iter().enumerate() {
            tx.execute(
                "
                INSERT INTO tournament_matches
                    (tournament_id, phase, bracket_slot, player1, player2, status)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ",
                params![
                    tournament_id,
                    TournamentPhase::RoundOf16.as_str(),
                    slot as u32,
                    p1,
                    p2,
                    TournamentMatchStatus::Scheduled.as_str(),
                ],
            )?;
        }

        for slot in 0..4 {
            tx.execute(
                "
                INSERT INTO tournament_matches
                    (tournament_id, phase, bracket_slot, status)
                VALUES (?1, ?2, ?3, ?4)
                ",
                params![
                    tournament_id,
                    TournamentPhase::Quarter.as_str(),
                    slot,
                    TournamentMatchStatus::Scheduled.as_str(),
                ],
            )?;
        }

        for slot in 0..2 {
            tx.execute(
                "
                INSERT INTO tournament_matches
                    (tournament_id, phase, bracket_slot, status)
                VALUES (?1, ?2, ?3, ?4)
                ",
                params![
                    tournament_id,
                    TournamentPhase::Semi.as_str(),
                    slot,
                    TournamentMatchStatus::Scheduled.as_str(),
                ],
            )?;
        }

        tx.execute(
            "
            INSERT INTO tournament_matches
                (tournament_id, phase, bracket_slot, status)
            VALUES (?1, ?2, 0, ?3)
            ",
            params![
                tournament_id,
                TournamentPhase::Final.as_str(),
                TournamentMatchStatus::Scheduled.as_str(),
            ],
        )?;
        Ok(())
    }

    fn advance_bracket_in_conn(&self, conn: &Connection, tm: &TournamentMatch) -> Result<()> {
        let outcome = tm.outcome.context("résultat manquant")?;
        let winner = match outcome {
            MatchOutcome::Player1Win => tm.player1.clone().unwrap(),
            MatchOutcome::Player2Win => tm.player2.clone().unwrap(),
            MatchOutcome::Draw => bail!("match nul impossible en arbre"),
        };

        let next = next_bracket_phase(tm.phase);
        let Some(next_phase) = next else {
            self.complete_tournament_in_conn(conn, tm.tournament_id, &winner)?;
            return Ok(());
        };

        let next_slot = tm.bracket_slot.unwrap_or(0) / 2;
        let is_player1 = tm.bracket_slot.unwrap_or(0) % 2 == 0;

        let mut stmt = conn.prepare(
            "
            SELECT id, player1, player2 FROM tournament_matches
            WHERE tournament_id = ?1 AND phase = ?2 AND bracket_slot = ?3
            ",
        )?;
        let mut rows = stmt.query(params![
            tm.tournament_id,
            next_phase.as_str(),
            next_slot,
        ])?;

        if let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            if is_player1 {
                conn.execute(
                    "UPDATE tournament_matches SET player1 = ?1 WHERE id = ?2",
                    params![winner, id],
                )?;
            } else {
                conn.execute(
                    "UPDATE tournament_matches SET player2 = ?1 WHERE id = ?2",
                    params![winner, id],
                )?;
            }
        }

        if tm.phase == TournamentPhase::RoundOf16
            && matches!(
                self.get_in_conn(conn, tm.tournament_id)?.unwrap().bracket_format,
                BracketFormat::RoundOf16
            )
        {
            // Fill QF with pool winners waiting
            self.fill_quarters_after_r16(conn, tm)?;
        }

        Ok(())
    }

    fn fill_quarters_after_r16(&self, conn: &Connection, tm: &TournamentMatch) -> Result<()> {
        let slot = tm.bracket_slot.unwrap_or(0);
        let pools = self.list_pools_in_conn(conn, tm.tournament_id)?;
        if slot as usize >= pools.len() {
            return Ok(());
        }
        let mut sorted = pools[slot as usize].players.clone();
        sorted.sort_by(|a, b| {
            b.points
                .cmp(&a.points)
                .then_with(|| b.objectives.cmp(&a.objectives))
                .then_with(|| b.survivors.cmp(&a.survivors))
        });
        let first = sorted.first().context("poule vide")?.player_name.clone();

        let outcome = tm.outcome.unwrap();
        let r16_winner = match outcome {
            MatchOutcome::Player1Win => tm.player1.clone().unwrap(),
            MatchOutcome::Player2Win => tm.player2.clone().unwrap(),
            MatchOutcome::Draw => bail!("nul interdit"),
        };

        conn.execute(
            "
            UPDATE tournament_matches SET player1 = ?1, player2 = ?2
            WHERE tournament_id = ?3 AND phase = 'quarter' AND bracket_slot = ?4
            ",
            params![first, r16_winner, tm.tournament_id, slot],
        )?;
        Ok(())
    }

    fn complete_tournament_in_conn(
        &self,
        conn: &Connection,
        tournament_id: i64,
        winner: &str,
    ) -> Result<()> {
        let now = now_unix();
        conn.execute(
            "
            UPDATE tournaments SET status = ?1, completed_at = ?2 WHERE id = ?3
            ",
            params![TournamentStatus::Completed.as_str(), now, tournament_id],
        )?;

        conn.execute(
            "
            UPDATE tournament_players SET final_placement = 1
            WHERE tournament_id = ?1 AND player_name_key = ?2
            ",
            params![tournament_id, normalize_name(winner)],
        )?;

        Ok(())
    }

    fn ratings_for_match_in_conn(
        &self,
        conn: &Connection,
        tm: &TournamentMatch,
        _k_factor: f64,
    ) -> Result<(f64, f64)> {
        let p1 = tm.player1.as_ref().context("joueur 1 manquant")?;
        let p2 = tm.player2.as_ref().context("joueur 2 manquant")?;

        let snap1: f64 = conn.query_row(
            "
            SELECT CASE WHEN ?4 = 'pool'
                THEN start_rating
                ELSE bracket_rating END
            FROM tournament_players
            WHERE tournament_id = ?1 AND player_name_key = ?2
            ",
            params![
                tm.tournament_id,
                normalize_name(p1),
                normalize_name(p2),
                tm.phase.as_str(),
            ],
            |row| row.get(0),
        )?;

        let snap2: f64 = conn.query_row(
            "
            SELECT CASE WHEN ?3 = 'pool'
                THEN start_rating
                ELSE bracket_rating END
            FROM tournament_players
            WHERE tournament_id = ?1 AND player_name_key = ?2
            ",
            params![tm.tournament_id, normalize_name(p2), tm.phase.as_str()],
            |row| row.get(0),
        )?;

        Ok((snap1, snap2))
    }

    fn update_pool_standings_in_conn(&self, conn: &Connection, tm: &TournamentMatch) -> Result<()> {
        let p1 = tm.player1.as_ref().context("joueur 1 manquant")?;
        let p2 = tm.player2.as_ref().context("joueur 2 manquant")?;
        let k1 = normalize_name(p1);
        let k2 = normalize_name(p2);

        for (key, pts, obj, surv, win, draw, loss) in [
            (
                k1.as_str(),
                tm.player1_tournament_points as u32,
                tm.player1_objectives as u32,
                tm.player1_survivors as u32,
                matches!(tm.outcome, Some(MatchOutcome::Player1Win)) && !tm.is_forfeit,
                matches!(tm.outcome, Some(MatchOutcome::Draw)),
                matches!(tm.outcome, Some(MatchOutcome::Player2Win)) && !tm.is_forfeit,
            ),
            (
                k2.as_str(),
                tm.player2_tournament_points as u32,
                tm.player2_objectives as u32,
                tm.player2_survivors as u32,
                matches!(tm.outcome, Some(MatchOutcome::Player2Win)) && !tm.is_forfeit,
                matches!(tm.outcome, Some(MatchOutcome::Draw)),
                matches!(tm.outcome, Some(MatchOutcome::Player1Win)) && !tm.is_forfeit,
            ),
        ] {
            conn.execute(
                "
                UPDATE tournament_players SET
                    pool_points = pool_points + ?1,
                    pool_objectives = pool_objectives + ?2,
                    pool_survivors = pool_survivors + ?3
                WHERE tournament_id = ?4 AND player_name_key = ?5
                ",
                params![pts, obj, surv, tm.tournament_id, key],
            )?;
            let _ = (win, draw, loss);
        }
        Ok(())
    }

    pub fn player_tournament_results(&self, player_name: &str) -> Result<Vec<PlayerTournamentResult>> {
        let conn = self.conn.lock().unwrap();
        let key = normalize_name(player_name);
        let mut stmt = conn.prepare(
            "
            SELECT t.id, t.name, tp.final_placement, t.completed_at
            FROM tournament_players tp
            JOIN tournaments t ON t.id = tp.tournament_id
            WHERE tp.player_name_key = ?1 AND t.status = 'completed'
            ORDER BY t.completed_at DESC
            ",
        )?;
        let rows = stmt.query_map(params![key], |row| {
            let placement: Option<u32> = row.get(2)?;
            let label = placement.map(placement_label).unwrap_or_else(|| "—".into());
            Ok(PlayerTournamentResult {
                tournament_id: row.get(0)?,
                tournament_name: row.get(1)?,
                final_placement: placement,
                placement_label: label,
                completed_at: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn star_counts(&self) -> Result<std::collections::HashMap<String, u32>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "
            SELECT player_name, COUNT(*) FROM tournament_players
            WHERE final_placement = 1
            GROUP BY player_name_key
            ",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?))
        })?;
        rows.collect::<Result<std::collections::HashMap<_, _>, _>>()
            .map_err(Into::into)
    }

    pub fn pool_matches_pending_elo(&self, tournament_id: i64) -> Result<Vec<TournamentMatch>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "
            SELECT id FROM tournament_matches
            WHERE tournament_id = ?1 AND phase = 'pool'
              AND status = 'confirmed' AND is_forfeit = 0 AND elo_applied_at IS NULL
            ",
        )?;
        let ids: Vec<i64> = stmt
            .query_map(params![tournament_id], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ids
            .into_iter()
            .filter_map(|id| self.get_match_in_conn(&conn, id).ok().flatten())
            .collect())
    }

    pub fn mark_pool_elo_applied(&self, match_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE tournament_matches SET elo_applied_at = ?1 WHERE id = ?2",
            params![now_unix(), match_id],
        )?;
        Ok(())
    }

    pub fn update_bracket_rating(
        &self,
        tournament_id: i64,
        player_name: &str,
        delta: f64,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "
            UPDATE tournament_players SET
                bracket_rating = bracket_rating + ?1
            WHERE tournament_id = ?2 AND player_name_key = ?3
            ",
            params![delta, tournament_id, normalize_name(player_name)],
        )?;
        Ok(())
    }

    // --- row helpers ---

    fn get_in_conn(&self, conn: &Connection, id: i64) -> Result<Option<Tournament>> {
        let mut stmt = conn.prepare(
            "
            SELECT id, name, status, pool_count, bracket_format,
                   created_at, started_at, pools_finalized_at, completed_at
            FROM tournaments WHERE id = ?1
            ",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(row_to_tournament(row)?));
        }
        Ok(None)
    }

    fn list_registrations_in_conn(
        &self,
        conn: &Connection,
        tournament_id: i64,
    ) -> Result<Vec<TournamentRegistration>> {
        let mut stmt = conn.prepare(
            "
            SELECT id, tournament_id, player_name, user_id, status,
                   waitlist_position, requested_at, reviewed_at, reviewed_by, army_id
            FROM tournament_registrations
            WHERE tournament_id = ?1
            ORDER BY requested_at ASC
            ",
        )?;
        let rows = stmt.query_map(params![tournament_id], row_to_registration)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn list_snapshots_in_conn(
        &self,
        conn: &Connection,
        tournament_id: i64,
    ) -> Result<Vec<TournamentPlayerSnapshot>> {
        let mut stmt = conn.prepare(
            "
            SELECT player_name, start_rating, pool_elo_delta, bracket_rating,
                   pool_points, pool_objectives, pool_survivors, final_placement
            FROM tournament_players WHERE tournament_id = ?1
            ",
        )?;
        let rows = stmt.query_map(params![tournament_id], |row| {
            Ok(TournamentPlayerSnapshot {
                player_name: row.get(0)?,
                start_rating: row.get(1)?,
                pool_elo_delta: row.get(2)?,
                bracket_rating: row.get(3)?,
                pool_points: row.get(4)?,
                pool_objectives: row.get(5)?,
                pool_survivors: row.get(6)?,
                final_placement: row.get(7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn list_pools_in_conn(&self, conn: &Connection, tournament_id: i64) -> Result<Vec<Pool>> {
        let mut stmt = conn.prepare(
            "SELECT id, tournament_id, name, position FROM pools WHERE tournament_id = ?1 ORDER BY position",
        )?;
        let pool_rows: Vec<Pool> = stmt
            .query_map(params![tournament_id], |row| {
                Ok(Pool {
                    id: row.get(0)?,
                    tournament_id: row.get(1)?,
                    name: row.get(2)?,
                    position: row.get(3)?,
                    players: Vec::new(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut pools = Vec::new();
        for mut pool in pool_rows {
            pool.players = self.list_pool_players_in_conn(conn, pool.id, tournament_id)?;
            pools.push(pool);
        }
        Ok(pools)
    }

    fn list_pool_players_in_conn(
        &self,
        conn: &Connection,
        pool_id: i64,
        tournament_id: i64,
    ) -> Result<Vec<PoolPlayer>> {
        let mut stmt = conn.prepare(
            "
            SELECT pp.player_name, pp.seed,
                   COALESCE(tp.pool_points, 0),
                   COALESCE(tp.pool_objectives, 0),
                   COALESCE(tp.pool_survivors, 0)
            FROM pool_players pp
            LEFT JOIN tournament_players tp
                ON tp.tournament_id = ?2 AND tp.player_name_key = pp.player_name_key
            WHERE pp.pool_id = ?1
            ORDER BY pp.seed
            ",
        )?;
        let rows = stmt.query_map(params![pool_id, tournament_id], |row| {
            Ok(PoolPlayer {
                player_name: row.get(0)?,
                player_display_name: None,
                seed: row.get(1)?,
                points: row.get(2)?,
                objectives: row.get(3)?,
                survivors: row.get(4)?,
                wins: 0,
                draws: 0,
                losses: 0,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn list_matches_in_conn(
        &self,
        conn: &Connection,
        tournament_id: i64,
    ) -> Result<Vec<TournamentMatch>> {
        let mut stmt = conn.prepare(
            "SELECT id FROM tournament_matches WHERE tournament_id = ?1 ORDER BY id",
        )?;
        let ids: Vec<i64> = stmt
            .query_map(params![tournament_id], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ids
            .into_iter()
            .filter_map(|id| self.get_match_in_conn(conn, id).ok().flatten())
            .collect())
    }

    fn get_match_in_conn(&self, conn: &Connection, id: i64) -> Result<Option<TournamentMatch>> {
        let mut stmt = conn.prepare(
            "
            SELECT id, tournament_id, phase, pool_id, bracket_slot,
                   player1, player2,
                   player1_objectives, player2_objectives,
                   player1_survivors, player2_survivors,
                   player1_tournament_points, player2_tournament_points,
                   outcome, is_forfeit, forfeit_player,
                   player1_elo_delta, player2_elo_delta,
                   player1_rating_used, player2_rating_used,
                   elo_applied_at, status,
                   submitted_by_user_id, submitted_at,
                   confirmed_by_user_id, confirmed_at,
                   scenario_id, scenario_name,
                   player1_army_id, player2_army_id, played_at
            FROM tournament_matches WHERE id = ?1
            ",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(row_to_tournament_match(row)?));
        }
        Ok(None)
    }

    fn registration_for_player_in_conn(
        &self,
        conn: &Connection,
        tournament_id: i64,
        player_key: &str,
    ) -> Result<Option<TournamentRegistration>> {
        let mut stmt = conn.prepare(
            "
            SELECT id FROM tournament_registrations
            WHERE tournament_id = ?1 AND player_name_key = ?2
            ",
        )?;
        let mut rows = stmt.query(params![tournament_id, player_key])?;
        if let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            return self.get_registration_in_conn(conn, id);
        }
        Ok(None)
    }

    fn get_registration_in_conn(
        &self,
        conn: &Connection,
        id: i64,
    ) -> Result<Option<TournamentRegistration>> {
        let mut stmt = conn.prepare(
            "
            SELECT id, tournament_id, player_name, user_id, status,
                   waitlist_position, requested_at, reviewed_at, reviewed_by, army_id
            FROM tournament_registrations WHERE id = ?1
            ",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(row_to_registration(row)?));
        }
        Ok(None)
    }
}

fn next_bracket_phase(phase: TournamentPhase) -> Option<TournamentPhase> {
    match phase {
        TournamentPhase::RoundOf16 => Some(TournamentPhase::Quarter),
        TournamentPhase::Quarter => Some(TournamentPhase::Semi),
        TournamentPhase::Semi => Some(TournamentPhase::Final),
        TournamentPhase::Final => None,
        TournamentPhase::Pool => None,
    }
}

fn outcome_to_str(outcome: MatchOutcome) -> &'static str {
    match outcome {
        MatchOutcome::Player1Win => "player1_win",
        MatchOutcome::Player2Win => "player2_win",
        MatchOutcome::Draw => "draw",
    }
}

fn parse_outcome(value: &str) -> Option<MatchOutcome> {
    match value {
        "player1_win" => Some(MatchOutcome::Player1Win),
        "player2_win" => Some(MatchOutcome::Player2Win),
        "draw" => Some(MatchOutcome::Draw),
        _ => None,
    }
}

fn row_to_tournament(row: &rusqlite::Row<'_>) -> rusqlite::Result<Tournament> {
    let status_str: String = row.get(2)?;
    let format_str: String = row.get(4)?;
    Ok(Tournament {
        id: row.get(0)?,
        name: row.get(1)?,
        status: TournamentStatus::parse(&status_str).unwrap_or(TournamentStatus::Draft),
        pool_count: row.get(3)?,
        bracket_format: BracketFormat::parse(&format_str)
            .unwrap_or(BracketFormat::QuartersDirect),
        created_at: row.get(5)?,
        started_at: row.get(6)?,
        pools_finalized_at: row.get(7)?,
        completed_at: row.get(8)?,
    })
}

fn row_to_registration(row: &rusqlite::Row<'_>) -> rusqlite::Result<TournamentRegistration> {
    let status_str: String = row.get(4)?;
    Ok(TournamentRegistration {
        id: row.get(0)?,
        tournament_id: row.get(1)?,
        player_name: row.get(2)?,
        player_display_name: None,
        user_id: row.get(3)?,
        status: RegistrationStatus::parse(&status_str).unwrap_or(RegistrationStatus::Pending),
        waitlist_position: row.get(5)?,
        requested_at: row.get(6)?,
        reviewed_at: row.get(7)?,
        reviewed_by: row.get(8)?,
        army_id: row.get(9)?,
    })
}

fn row_to_tournament_match(row: &rusqlite::Row<'_>) -> rusqlite::Result<TournamentMatch> {
    let phase_str: String = row.get(2)?;
    let outcome: Option<String> = row.get(13)?;
    let status_str: String = row.get(21)?;
    Ok(TournamentMatch {
        id: row.get(0)?,
        tournament_id: row.get(1)?,
        phase: TournamentPhase::parse(&phase_str).unwrap_or(TournamentPhase::Pool),
        pool_id: row.get(3)?,
        bracket_slot: row.get(4)?,
        player1: row.get(5)?,
        player2: row.get(6)?,
        player1_display_name: None,
        player2_display_name: None,
        player1_objectives: row.get(7)?,
        player2_objectives: row.get(8)?,
        player1_survivors: row.get(9)?,
        player2_survivors: row.get(10)?,
        player1_tournament_points: row.get(11)?,
        player2_tournament_points: row.get(12)?,
        outcome: outcome.and_then(|v| parse_outcome(&v)),
        is_forfeit: row.get::<_, i64>(14)? != 0,
        forfeit_player: row.get(15)?,
        forfeit_player_display_name: None,
        player1_elo_delta: row.get(16)?,
        player2_elo_delta: row.get(17)?,
        player1_rating_used: row.get(18)?,
        player2_rating_used: row.get(19)?,
        elo_applied_at: row.get(20)?,
        status: TournamentMatchStatus::parse(&status_str)
            .unwrap_or(TournamentMatchStatus::Scheduled),
        submitted_by_user_id: row.get(22)?,
        submitted_at: row.get(23)?,
        confirmed_by_user_id: row.get(24)?,
        confirmed_at: row.get(25)?,
        scenario_id: row.get(26)?,
        scenario_name: row.get(27)?,
        player1_army_id: row.get(28)?,
        player2_army_id: row.get(29)?,
        played_at: row.get(30)?,
    })
}

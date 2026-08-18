use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Deserialize;

use crate::army_list::require_lists;
use crate::match_record::now_unix;
use crate::migrate::migrate;
use crate::player::MatchOutcome;
use crate::store::normalize_name;
use crate::tournament::{
    bracket_match_winner, bracket_scenario_phases, compute_elo_deltas, compute_bracket_placements,
    draw_seeded_pools, enrich_top_four_armies, placement_label, pool_round_robin_pairs,
    pool_scenario_letter, round_of_16_barrage_pairings, sort_pool_standings,
    tournament_points_for_player, BracketFormat, PlayerTournamentResult, Pool, PoolPlayer,
    RegistrationStatus, Tournament, TournamentDetail, TournamentListEntry, TournamentMatch,
    TournamentMatchStatus, TournamentPhase, TournamentPlayerSnapshot, TournamentRegistration,
    TournamentScenarioSlot, TournamentStatus, POOLS_EIGHT_CAPACITY, POOLS_FOUR_CAPACITY,
    POOL_SCENARIO_LETTERS, compute_display_status, compute_top_four, registration_counts,
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

#[derive(Debug, Deserialize)]
pub struct UpdateTournamentDetailsRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

fn default_bracket_format() -> String {
    "round_of_16".into()
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    // Inscription initiale sans listes (étape 1).
}

#[derive(Debug, Deserialize)]
pub struct CompleteRegistrationListsRequest {
    pub army_list_1: String,
    #[serde(default)]
    pub army_list_2: String,
}

#[derive(Debug, Deserialize)]
pub struct AdminRegisterRequest {
    pub player_name: String,
    /// Optionnel : déduit des codes de listes si absent.
    #[serde(default)]
    pub army_id: Option<u32>,
    pub army_list_1: String,
    #[serde(default)]
    pub army_list_2: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateBracketListsRequest {
    pub bracket_list_1: String,
    #[serde(default)]
    pub bracket_list_2: String,
}

#[derive(Debug, Deserialize)]
pub struct SetPoolScenariosRequest {
    /// Exactement 5 IDs de scénarios (A–E).
    pub scenario_ids: Vec<i64>,
}

#[derive(Debug, Deserialize)]
pub struct RerollScenarioRequest {
    /// Lettre A–E (poules) ou index 0–3 (bracket_pool).
    pub slot: String,
}

#[derive(Debug, Deserialize)]
pub struct SetBracketScenarioPoolRequest {
    /// Exactement 4 IDs (non encore assignés aux tours).
    pub scenario_ids: Vec<i64>,
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
pub struct SetupBracketRequest {
    pub matches: Vec<BracketSlotSetup>,
}

#[derive(Debug, Deserialize)]
pub struct BracketSlotSetup {
    pub bracket_slot: u32,
    pub player1: String,
    pub player2: String,
    #[serde(default)]
    pub quarter_player1: Option<String>,
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
    /// Slot 1 ou 2 parmi les listes d'inscription (poules) ou d'arbre.
    #[serde(default)]
    pub player1_list_slot: Option<u8>,
    #[serde(default)]
    pub player2_list_slot: Option<u8>,
    #[serde(default)]
    pub scenario_id: Option<i64>,
    #[serde(default)]
    pub scenario_other: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ForfeitRequest {
    pub forfeit_player: String,
}

fn registration_list_for_slot(
    registration: &TournamentRegistration,
    phase: TournamentPhase,
    slot: u8,
) -> Result<String> {
    let (list1, list2) = if phase == TournamentPhase::Pool {
        (
            registration.army_list_1.as_deref(),
            registration.army_list_2.as_deref(),
        )
    } else {
        (
            registration.bracket_list_1.as_deref(),
            registration.bracket_list_2.as_deref(),
        )
    };
    let code = match slot {
        1 => list1.filter(|s| !s.is_empty()),
        2 => list2.filter(|s| !s.is_empty()),
        _ => None,
    }
    .context(if phase == TournamentPhase::Pool {
        "choisissez la liste 1 ou 2 d'inscription"
    } else {
        "choisissez la liste 1 ou 2 d'arbre"
    })?;
    Ok(code.to_string())
}

fn registration_has_bracket_lists(registration: &TournamentRegistration) -> bool {
    registration
        .bracket_list_1
        .as_ref()
        .is_some_and(|s| !s.trim().is_empty())
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
            SELECT id, name, description, status, pool_count, bracket_format,
                   created_at, started_at, pools_finalized_at, completed_at
            FROM tournaments
            ORDER BY id DESC
            ",
        )?;
        let rows = stmt.query_map([], row_to_tournament)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn list_entries(&self) -> Result<Vec<TournamentListEntry>> {
        let conn = self.conn.lock().unwrap();
        let tournaments = {
            let mut stmt = conn.prepare(
                "
                SELECT id, name, description, status, pool_count, bracket_format,
                       created_at, started_at, pools_finalized_at, completed_at
                FROM tournaments
                ORDER BY id DESC
                ",
            )?;
            let rows = stmt.query_map([], row_to_tournament)?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        tournaments
            .into_iter()
            .map(|tournament| {
                let registrations = self.list_registrations_in_conn(&conn, tournament.id)?;
                let matches = self.list_matches_in_conn(&conn, tournament.id)?;
                let (registered_count, waitlist_count) = registration_counts(&registrations);
                let display_status = compute_display_status(&tournament, &matches);
                let mut top_four = if tournament.status == TournamentStatus::Completed {
                    compute_top_four(&matches)
                } else {
                    Vec::new()
                };
                enrich_top_four_armies(&mut top_four, &registrations);
                let bracket_matches: Vec<_> = matches
                    .into_iter()
                    .filter(|m| m.phase != TournamentPhase::Pool)
                    .collect();
                let pool_scenarios = self.list_scenarios_in_conn(&conn, tournament.id, "pool")?;

                Ok(TournamentListEntry {
                    tournament,
                    registered_count,
                    waitlist_count,
                    display_status,
                    top_four,
                    bracket_matches,
                    pool_scenarios,
                })
            })
            .collect()
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

    pub fn update_details(
        &self,
        tournament_id: i64,
        request: &UpdateTournamentDetailsRequest,
    ) -> Result<Tournament> {
        let name = request.name.trim();
        if name.is_empty() {
            bail!("indiquez un nom de tournoi");
        }
        let description = request.description.trim();

        let conn = self.conn.lock().unwrap();
        self.ensure_tournament_exists(&conn, tournament_id)?;
        let updated = conn.execute(
            "
            UPDATE tournaments
            SET name = ?1, description = ?2
            WHERE id = ?3
            ",
            params![name, description, tournament_id],
        )?;
        if updated == 0 {
            bail!("tournoi introuvable");
        }
        self.get_in_conn(&conn, tournament_id)?
            .context("tournoi introuvable après mise à jour")
    }

    /// Supprime un tournoi tant que la phase de poules n'a pas démarré
    /// (`draft` / inscriptions ouvertes ou fermées).
    pub fn delete(&self, tournament_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let tournament = self
            .get_in_conn(&conn, tournament_id)?
            .context("tournoi introuvable")?;

        match tournament.status {
            TournamentStatus::Draft
            | TournamentStatus::RegistrationOpen
            | TournamentStatus::RegistrationClosed => {}
            TournamentStatus::Started | TournamentStatus::Completed => {
                bail!("impossible de supprimer un tournoi dont la phase de poules a démarré");
            }
        }

        let tx = conn.unchecked_transaction()?;

        tx.execute(
            "UPDATE matches SET tournament_id = NULL, tournament_phase = NULL WHERE tournament_id = ?1",
            params![tournament_id],
        )?;
        tx.execute(
            "DELETE FROM tournament_scenarios WHERE tournament_id = ?1",
            params![tournament_id],
        )?;
        tx.execute(
            "DELETE FROM tournament_matches WHERE tournament_id = ?1",
            params![tournament_id],
        )?;
        tx.execute(
            "
            DELETE FROM pool_players
            WHERE pool_id IN (SELECT id FROM pools WHERE tournament_id = ?1)
            ",
            params![tournament_id],
        )?;
        tx.execute(
            "DELETE FROM pools WHERE tournament_id = ?1",
            params![tournament_id],
        )?;
        tx.execute(
            "DELETE FROM tournament_players WHERE tournament_id = ?1",
            params![tournament_id],
        )?;
        tx.execute(
            "DELETE FROM tournament_registrations WHERE tournament_id = ?1",
            params![tournament_id],
        )?;
        let deleted = tx.execute(
            "DELETE FROM tournaments WHERE id = ?1",
            params![tournament_id],
        )?;
        if deleted == 0 {
            bail!("tournoi introuvable");
        }

        tx.commit()?;
        Ok(())
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

        let registrations = self.list_registrations_in_conn(&conn, id)?;
        let matches = self.list_matches_in_conn(&conn, id)?;
        let (registered_count, waitlist_count) = registration_counts(&registrations);
        let display_status = compute_display_status(&tournament, &matches);
        let mut top_four = if tournament.status == TournamentStatus::Completed {
            compute_top_four(&matches)
        } else {
            Vec::new()
        };
        enrich_top_four_armies(&mut top_four, &registrations);

        Ok(Some(TournamentDetail {
            registrations,
            players: self.list_snapshots_in_conn(&conn, id)?,
            pools: self.list_pools_in_conn(&conn, id)?,
            matches,
            tournament,
            registered_count,
            waitlist_count,
            display_status,
            top_four,
            pool_scenarios: self.list_scenarios_in_conn(&conn, id, "pool")?,
            bracket_scenario_pool: self.list_scenarios_in_conn(&conn, id, "bracket_pool")?,
            bracket_scenarios: self.list_scenarios_in_conn(&conn, id, "bracket")?,
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
                (tournament_id, player_name_key, player_name, user_id, status, requested_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ",
            params![
                tournament_id,
                key,
                player_name,
                user_id,
                RegistrationStatus::Pending.as_str(),
                now,
            ],
        )?;
        let reg_id = conn.last_insert_rowid();
        self.get_registration_in_conn(&conn, reg_id)?
            .context("inscription introuvable")
    }

    /// Étape 2 : saisir / modifier / supprimer les listes (liste 2 optionnelle).
    /// Listes vides → suppression ; le statut repasse en attente des listes.
    pub fn complete_registration_lists(
        &self,
        tournament_id: i64,
        player_name: &str,
        army_list_1: &str,
        army_list_2: &str,
        army_id: Option<u32>,
    ) -> Result<TournamentRegistration> {
        let conn = self.conn.lock().unwrap();
        let tournament = self
            .get_in_conn(&conn, tournament_id)?
            .context("tournoi introuvable")?;
        if tournament.status != TournamentStatus::RegistrationOpen
            && tournament.status != TournamentStatus::RegistrationClosed
        {
            bail!("impossible de mettre à jour les listes pour ce tournoi");
        }

        let key = normalize_name(player_name);
        let registration = self
            .registration_for_player_in_conn(&conn, tournament_id, &key)?
            .context("inscription introuvable")?;
        if registration.status != RegistrationStatus::Pending
            && registration.status != RegistrationStatus::Waitlisted
            && registration.status != RegistrationStatus::Approved
        {
            bail!("cette inscription ne peut plus être modifiée");
        }

        let list1_opt = crate::army_list::normalize_army_list_code(army_list_1);
        let list2_opt = crate::army_list::normalize_army_list_code(army_list_2);

        if list1_opt.is_none() {
            if list2_opt.is_some() {
                bail!("indiquez la liste 1, ou laissez les deux listes vides pour les supprimer");
            }
            // Suppression des listes → en attente des listes.
            conn.execute(
                "
                UPDATE tournament_registrations
                SET army_id = NULL,
                    army_list_1 = NULL,
                    army_list_2 = NULL,
                    status = ?1,
                    reviewed_at = NULL,
                    reviewed_by = NULL,
                    waitlist_position = NULL
                WHERE id = ?2
                ",
                params![RegistrationStatus::Pending.as_str(), registration.id],
            )?;
            return self
                .get_registration_in_conn(&conn, registration.id)?
                .context("inscription introuvable");
        }

        let (list1, list2) = require_lists(army_list_1, army_list_2)?;
        let army_id = army_id.context("sectorielle manquante")?;
        let list2_stored = list2.as_deref().unwrap_or("");
        let lists_changed = registration.army_list_1.as_deref().unwrap_or("") != list1.as_str()
            || registration.army_list_2.as_deref().unwrap_or("") != list2_stored
            || registration.army_id != Some(army_id);

        // Toute modification des listes exige une nouvelle validation orga.
        let reset_to_pending = lists_changed
            && matches!(
                registration.status,
                RegistrationStatus::Approved | RegistrationStatus::Waitlisted
            );

        if reset_to_pending {
            conn.execute(
                "
                UPDATE tournament_registrations
                SET army_id = ?1,
                    army_list_1 = ?2,
                    army_list_2 = ?3,
                    status = ?4,
                    reviewed_at = NULL,
                    reviewed_by = NULL,
                    waitlist_position = NULL
                WHERE id = ?5
                ",
                params![
                    army_id,
                    list1,
                    list2_stored,
                    RegistrationStatus::Pending.as_str(),
                    registration.id
                ],
            )?;
        } else {
            conn.execute(
                "
                UPDATE tournament_registrations
                SET army_id = ?1, army_list_1 = ?2, army_list_2 = ?3
                WHERE id = ?4
                ",
                params![army_id, list1, list2_stored, registration.id],
            )?;
        }
        self.get_registration_in_conn(&conn, registration.id)?
            .context("inscription introuvable")
    }

    /// Se désinscrire tant que le tournoi n'a pas démarré.
    pub fn unregister(&self, tournament_id: i64, player_name: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let tournament = self
            .get_in_conn(&conn, tournament_id)?
            .context("tournoi introuvable")?;
        match tournament.status {
            TournamentStatus::Draft
            | TournamentStatus::RegistrationOpen
            | TournamentStatus::RegistrationClosed => {}
            _ => bail!("impossible de se désinscrire après le démarrage du tournoi"),
        }

        let key = normalize_name(player_name);
        let registration = self
            .registration_for_player_in_conn(&conn, tournament_id, &key)?
            .context("inscription introuvable")?;

        conn.execute(
            "DELETE FROM tournament_registrations WHERE id = ?1",
            params![registration.id],
        )?;

        let mut waitlisted = self.list_registrations_in_conn(&conn, tournament_id)?;
        waitlisted.retain(|r| r.status == RegistrationStatus::Waitlisted);
        waitlisted.sort_by_key(|r| r.waitlist_position.unwrap_or(u32::MAX));
        for (index, reg) in waitlisted.iter().enumerate() {
            conn.execute(
                "UPDATE tournament_registrations SET waitlist_position = ?1 WHERE id = ?2",
                params![(index + 1) as u32, reg.id],
            )?;
        }
        Ok(())
    }

    pub fn admin_register(
        &self,
        tournament_id: i64,
        player_name: &str,
        admin_id: i64,
        army_id: u32,
        army_list_1: &str,
        army_list_2: &str,
    ) -> Result<TournamentRegistration> {
        let (list1, list2) = require_lists(army_list_1, army_list_2)?;
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
                (tournament_id, player_name_key, player_name, status, requested_at,
                 reviewed_at, reviewed_by, army_id, army_list_1, army_list_2)
            VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6, ?7, ?8, ?9)
            ",
            params![
                tournament_id,
                key,
                player_name,
                RegistrationStatus::Pending.as_str(),
                now,
                admin_id,
                army_id,
                list1,
                list2.as_deref().unwrap_or(""),
            ],
        )?;
        let reg_id = conn.last_insert_rowid();
        self.review_registration_in_tx(&conn, reg_id, "approved", admin_id)
    }

    pub fn update_bracket_lists(
        &self,
        tournament_id: i64,
        player_name: &str,
        list1: &str,
        list2: &str,
    ) -> Result<TournamentRegistration> {
        let (list1, list2) = require_lists(list1, list2)?;
        let conn = self.conn.lock().unwrap();
        let tournament = self
            .get_in_conn(&conn, tournament_id)?
            .context("tournoi introuvable")?;
        if tournament.pools_finalized_at.is_none() {
            bail!("les listes d'arbre sont disponibles après la finalisation des poules");
        }
        if tournament.status == TournamentStatus::Completed {
            bail!("tournoi terminé");
        }

        let key = normalize_name(player_name);
        let updated = conn.execute(
            "
            UPDATE tournament_registrations
            SET bracket_list_1 = ?1, bracket_list_2 = ?2
            WHERE tournament_id = ?3 AND player_name_key = ?4
              AND status = 'approved'
            ",
            params![
                list1,
                list2.as_deref().unwrap_or(""),
                tournament_id,
                key
            ],
        )?;
        if updated == 0 {
            bail!("inscription validée introuvable");
        }
        self.registration_for_player_in_conn(&conn, tournament_id, &key)?
            .context("inscription introuvable")
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

        if registration.army_list_1.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
            bail!("la liste 1 Army est requise pour valider l'inscription");
        }

        if registration.army_id.is_none() {
            bail!("sectorielle manquante pour valider l'inscription");
        }

        let capacity = if tournament.pool_count >= 8 {
            POOLS_EIGHT_CAPACITY
        } else {
            POOLS_FOUR_CAPACITY
        } as i64;

        if approved_count >= capacity {
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

    pub fn draw_pools(&self, tournament_id: i64) -> Result<Vec<Pool>> {
        let conn = self.conn.lock().unwrap();
        let tournament = self
            .get_in_conn(&conn, tournament_id)?
            .context("tournoi introuvable")?;
        if tournament.status != TournamentStatus::Started {
            bail!("le tournoi n'est pas démarré");
        }

        let snapshots = self.list_snapshots_in_conn(&conn, tournament_id)?;
        if snapshots.is_empty() {
            bail!("aucun joueur démarré");
        }

        let ranked: Vec<(String, f64)> = snapshots
            .into_iter()
            .map(|s| (s.player_name, s.start_rating))
            .collect();
        let drawn = draw_seeded_pools(&ranked, tournament.pool_count as usize);
        let letters = "ABCDEFGH";
        let pools: Vec<PoolSetup> = drawn
            .into_iter()
            .enumerate()
            .map(|(index, players)| PoolSetup {
                name: format!("Poule {}", letters.chars().nth(index).unwrap_or('?')),
                position: (index + 1) as u8,
                players,
            })
            .collect();

        drop(conn);
        self.setup_pools(
            tournament_id,
            &SetupPoolsRequest { pools },
        )
    }

    pub fn set_pool_scenarios(
        &self,
        tournament_id: i64,
        scenario_ids: &[i64],
    ) -> Result<Vec<TournamentScenarioSlot>> {
        if scenario_ids.len() != POOL_SCENARIO_LETTERS.len() {
            bail!("il faut exactement 5 scénarios (A–E)");
        }
        let slots = ["A", "B", "C", "D", "E"];
        self.replace_scenario_kind(tournament_id, "pool", scenario_ids, &slots)
    }

    pub fn draw_pool_scenarios(
        &self,
        tournament_id: i64,
    ) -> Result<Vec<TournamentScenarioSlot>> {
        let ids = self.pick_random_scenario_ids(POOL_SCENARIO_LETTERS.len())?;
        self.set_pool_scenarios(tournament_id, &ids)
    }

    pub fn reroll_pool_scenario(
        &self,
        tournament_id: i64,
        letter: &str,
    ) -> Result<Vec<TournamentScenarioSlot>> {
        let letter = letter.trim().to_uppercase();
        if !POOL_SCENARIO_LETTERS.iter().any(|c| c.to_string() == letter) {
            bail!("lettre de scénario invalide");
        }
        let conn = self.conn.lock().unwrap();
        self.ensure_tournament_exists(&conn, tournament_id)?;
        let current = self.list_scenarios_in_conn(&conn, tournament_id, "pool")?;
        let used: Vec<i64> = current.iter().map(|s| s.scenario_id).collect();
        let new_id = self.pick_random_scenario_excluding(&conn, &used)?;
        conn.execute(
            "
            INSERT INTO tournament_scenarios (tournament_id, kind, slot, scenario_id)
            VALUES (?1, 'pool', ?2, ?3)
            ON CONFLICT(tournament_id, kind, slot) DO UPDATE SET scenario_id = excluded.scenario_id
            ",
            params![tournament_id, letter, new_id],
        )?;
        self.list_scenarios_in_conn(&conn, tournament_id, "pool")
    }

    pub fn set_bracket_scenario_pool(
        &self,
        tournament_id: i64,
        scenario_ids: &[i64],
    ) -> Result<Vec<TournamentScenarioSlot>> {
        let conn = self.conn.lock().unwrap();
        let tournament = self
            .get_in_conn(&conn, tournament_id)?
            .context("tournoi introuvable")?;
        if tournament.pools_finalized_at.is_none() {
            bail!("finalisez les poules avant de choisir les scénarios d'arbre");
        }
        let expected = bracket_scenario_phases(tournament.bracket_format).len();
        if scenario_ids.len() != expected {
            bail!("il faut exactement {expected} scénarios pour l'arbre");
        }
        drop(conn);
        let slots: Vec<String> = (0..expected).map(|i| i.to_string()).collect();
        let slot_refs: Vec<&str> = slots.iter().map(String::as_str).collect();
        // Clear any previous phase assignment when re-picking the pool.
        {
            let conn = self.conn.lock().unwrap();
            conn.execute(
                "DELETE FROM tournament_scenarios WHERE tournament_id = ?1 AND kind = 'bracket'",
                params![tournament_id],
            )?;
        }
        self.replace_scenario_kind(tournament_id, "bracket_pool", scenario_ids, &slot_refs)
    }

    pub fn draw_bracket_scenario_pool(
        &self,
        tournament_id: i64,
    ) -> Result<Vec<TournamentScenarioSlot>> {
        let conn = self.conn.lock().unwrap();
        let tournament = self
            .get_in_conn(&conn, tournament_id)?
            .context("tournoi introuvable")?;
        let count = bracket_scenario_phases(tournament.bracket_format).len();
        drop(conn);
        let ids = self.pick_random_scenario_ids(count)?;
        self.set_bracket_scenario_pool(tournament_id, &ids)
    }

    pub fn reroll_bracket_scenario_pool_slot(
        &self,
        tournament_id: i64,
        slot: &str,
    ) -> Result<Vec<TournamentScenarioSlot>> {
        let conn = self.conn.lock().unwrap();
        let tournament = self
            .get_in_conn(&conn, tournament_id)?
            .context("tournoi introuvable")?;
        let expected = bracket_scenario_phases(tournament.bracket_format).len();
        let index: usize = slot
            .parse()
            .map_err(|_| anyhow::anyhow!("slot d'arbre invalide"))?;
        if index >= expected {
            bail!("slot d'arbre invalide");
        }
        let current = self.list_scenarios_in_conn(&conn, tournament_id, "bracket_pool")?;
        let used: Vec<i64> = current.iter().map(|s| s.scenario_id).collect();
        let new_id = self.pick_random_scenario_excluding(&conn, &used)?;
        conn.execute(
            "
            INSERT INTO tournament_scenarios (tournament_id, kind, slot, scenario_id)
            VALUES (?1, 'bracket_pool', ?2, ?3)
            ON CONFLICT(tournament_id, kind, slot) DO UPDATE SET scenario_id = excluded.scenario_id
            ",
            params![tournament_id, index.to_string(), new_id],
        )?;
        // Invalidate prior phase assignment
        conn.execute(
            "DELETE FROM tournament_scenarios WHERE tournament_id = ?1 AND kind = 'bracket'",
            params![tournament_id],
        )?;
        self.list_scenarios_in_conn(&conn, tournament_id, "bracket_pool")
    }

    /// Mélange le pool de 4 scénarios et les assigne aux tours d'arbre + matchs.
    pub fn assign_bracket_scenarios(&self, tournament_id: i64) -> Result<Vec<TournamentScenarioSlot>> {
        use rand::seq::SliceRandom;

        let conn = self.conn.lock().unwrap();
        let tournament = self
            .get_in_conn(&conn, tournament_id)?
            .context("tournoi introuvable")?;
        let phases = bracket_scenario_phases(tournament.bracket_format);
        let mut pool = self.list_scenarios_in_conn(&conn, tournament_id, "bracket_pool")?;
        if pool.len() != phases.len() {
            bail!(
                "choisissez d'abord {} scénarios pour l'arbre",
                phases.len()
            );
        }
        pool.shuffle(&mut rand::rng());

        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM tournament_scenarios WHERE tournament_id = ?1 AND kind = 'bracket'",
            params![tournament_id],
        )?;
        for (phase, slot) in phases.iter().zip(pool.iter()) {
            tx.execute(
                "
                INSERT INTO tournament_scenarios (tournament_id, kind, slot, scenario_id)
                VALUES (?1, 'bracket', ?2, ?3)
                ",
                params![tournament_id, phase.as_str(), slot.scenario_id],
            )?;
            tx.execute(
                "
                UPDATE tournament_matches
                SET scenario_id = ?1, scenario_other = NULL
                WHERE tournament_id = ?2 AND phase = ?3
                ",
                params![slot.scenario_id, tournament_id, phase.as_str()],
            )?;
        }
        tx.commit()?;
        self.list_scenarios_in_conn(&conn, tournament_id, "bracket")
    }

    pub fn generate_pool_matches(&self, tournament_id: i64) -> Result<Vec<TournamentMatch>> {
        let conn = self.conn.lock().unwrap();
        let pools = self.list_pools_in_conn(&conn, tournament_id)?;
        let pool_scenarios = self.list_scenarios_in_conn(&conn, tournament_id, "pool")?;
        if pool_scenarios.len() != POOL_SCENARIO_LETTERS.len() {
            bail!("définissez les 5 scénarios de poule (A–E) avant de générer les matchs");
        }
        let letter_to_id: HashMap<char, i64> = pool_scenarios
            .iter()
            .filter_map(|s| {
                let letter = s.slot.chars().next()?;
                Some((letter, s.scenario_id))
            })
            .collect();

        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM tournament_matches WHERE tournament_id = ?1 AND phase = 'pool'",
            params![tournament_id],
        )?;

        for pool in &pools {
            let n = pool.players.len();
            let pairs = pool_round_robin_pairs(n);
            for (i, j) in pairs {
                let p1 = &pool.players[i];
                let p2 = &pool.players[j];
                let scenario_id = pool_scenario_letter(n, p1.seed as usize, p2.seed as usize)
                    .and_then(|letter| letter_to_id.get(&letter).copied());
                tx.execute(
                    "
                    INSERT INTO tournament_matches
                        (tournament_id, phase, pool_id, player1, player2, status, scenario_id)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                    ",
                    params![
                        tournament_id,
                        TournamentPhase::Pool.as_str(),
                        pool.id,
                        p1.player_name,
                        p2.player_name,
                        TournamentMatchStatus::Scheduled.as_str(),
                        scenario_id,
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

        let reg1 = self
            .registration_for_player_in_conn(&conn, tm.tournament_id, &normalize_name(&p1))?
            .context("inscription du joueur 1 introuvable")?;
        let reg2 = self
            .registration_for_player_in_conn(&conn, tm.tournament_id, &normalize_name(&p2))?
            .context("inscription du joueur 2 introuvable")?;

        let (list1, list2) = match (request.player1_list_slot, request.player2_list_slot) {
            (Some(slot1), Some(slot2)) => {
                if tm.phase != TournamentPhase::Pool {
                    if !registration_has_bracket_lists(&reg1) {
                        bail!("le joueur 1 doit saisir sa liste d'arbre avant de jouer");
                    }
                    if !registration_has_bracket_lists(&reg2) {
                        bail!("le joueur 2 doit saisir sa liste d'arbre avant de jouer");
                    }
                }
                (
                    Some(registration_list_for_slot(&reg1, tm.phase, slot1)?),
                    Some(registration_list_for_slot(&reg2, tm.phase, slot2)?),
                )
            }
            (None, None) if is_admin => (None, None),
            _ => bail!("choisissez la liste de chaque joueur (1 ou 2)"),
        };

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
        let player1_army_id = registration_army_id_in_conn(&conn, tm.tournament_id, &p1)?
            .or(request.player1_army_id);
        let player2_army_id = registration_army_id_in_conn(&conn, tm.tournament_id, &p2)?
            .or(request.player2_army_id);
        let scenario_other = request
            .scenario_other
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string);

        if request.scenario_id.is_some() && scenario_other.is_some() {
            anyhow::bail!(
                "choisissez un scénario du catalogue ou un texte libre, pas les deux"
            );
        }

        if let Some(id) = request.scenario_id {
            let exists: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM scenarios WHERE id = ?1)",
                params![id],
                |row| row.get(0),
            )?;
            if !exists {
                anyhow::bail!("scénario introuvable");
            }
            conn.execute(
                "UPDATE scenarios SET usage_count = usage_count + 1 WHERE id = ?1",
                params![id],
            )?;
        }

        conn.execute(
            "
            UPDATE tournament_matches SET
                player1_objectives = ?1, player2_objectives = ?2,
                player1_survivors = ?3, player2_survivors = ?4,
                player1_tournament_points = ?5, player2_tournament_points = ?6,
                outcome = ?7, is_forfeit = 0, is_unplayed = 0, forfeit_player = NULL,
                player1_elo_delta = ?8, player2_elo_delta = ?9,
                player1_rating_used = ?10, player2_rating_used = ?11,
                status = ?12, submitted_by_user_id = ?13, submitted_at = ?14,
                confirmed_by_user_id = ?15, confirmed_at = ?16,
                scenario_id = ?17, scenario_other = ?18,
                player1_army_id = ?19, player2_army_id = ?20,
                player1_army_list_code = ?21, player2_army_list_code = ?22,
                played_at = ?23
            WHERE id = ?24
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
                scenario_other,
                player1_army_id,
                player2_army_id,
                list1,
                list2,
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
        let player1_army_id = registration_army_id_in_conn(&conn, tm.tournament_id, &p1)?;
        let player2_army_id = registration_army_id_in_conn(&conn, tm.tournament_id, &p2)?;

        conn.execute(
            "
            UPDATE tournament_matches SET
                player1_objectives = 0, player2_objectives = 0,
                player1_survivors = 0, player2_survivors = 0,
                player1_tournament_points = ?1, player2_tournament_points = ?2,
                outcome = ?3, is_forfeit = 1, forfeit_player = ?4,
                player1_elo_delta = 0, player2_elo_delta = 0,
                status = ?5, confirmed_by_user_id = ?6, confirmed_at = ?7,
                player1_army_id = ?8, player2_army_id = ?9,
                played_at = ?7
            WHERE id = ?10
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
                player1_army_id,
                player2_army_id,
                match_id,
            ],
        )?;
        drop(conn);

        self.apply_confirmed_match(match_id, 32.0)?;
        self.get_match(match_id)?.context("match introuvable")
    }

    pub fn declare_match_unplayed(
        &self,
        match_id: i64,
        user_id: i64,
        is_admin: bool,
    ) -> Result<TournamentMatch> {
        if !is_admin {
            bail!("seul un admin peut déclarer un match non joué");
        }

        let conn = self.conn.lock().unwrap();
        let tm = self
            .get_match_in_conn(&conn, match_id)?
            .context("match introuvable")?;

        if tm.phase != TournamentPhase::Pool {
            bail!("option réservée aux matchs de poule");
        }

        if tm.status == TournamentMatchStatus::Confirmed {
            bail!("ce match est déjà confirmé");
        }

        let p1 = tm.player1.clone().context("joueur 1 manquant")?;
        let p2 = tm.player2.clone().context("joueur 2 manquant")?;
        let now = now_unix();
        let player1_army_id = registration_army_id_in_conn(&conn, tm.tournament_id, &p1)?;
        let player2_army_id = registration_army_id_in_conn(&conn, tm.tournament_id, &p2)?;

        conn.execute(
            "
            UPDATE tournament_matches SET
                player1_objectives = 0, player2_objectives = 0,
                player1_survivors = 0, player2_survivors = 0,
                player1_tournament_points = 0, player2_tournament_points = 0,
                outcome = NULL, is_forfeit = 0, is_unplayed = 1, forfeit_player = NULL,
                player1_elo_delta = 0, player2_elo_delta = 0,
                status = ?1, confirmed_by_user_id = ?2, confirmed_at = ?3,
                player1_army_id = ?4, player2_army_id = ?5,
                played_at = ?3
            WHERE id = ?6
            ",
            params![
                TournamentMatchStatus::Confirmed.as_str(),
                user_id,
                now,
                player1_army_id,
                player2_army_id,
                match_id,
            ],
        )?;
        drop(conn);

        self.apply_confirmed_match(match_id, 32.0)?;
        self.get_match(match_id)?.context("match introuvable")
    }

    pub fn correct_match_score(
        &self,
        match_id: i64,
        request: &SubmitMatchRequest,
        k_factor: f64,
    ) -> Result<(TournamentMatch, bool)> {
        if request.player1_objectives > 10 || request.player2_objectives > 10 {
            bail!("objectifs invalides");
        }

        let conn = self.conn.lock().unwrap();
        let old = self
            .get_match_in_conn(&conn, match_id)?
            .context("match introuvable")?;

        if old.status != TournamentMatchStatus::Confirmed {
            bail!("seuls les matchs confirmés peuvent être corrigés");
        }

        let p1 = old.player1.as_ref().context("joueur 1 manquant")?;
        let p2 = old.player2.as_ref().context("joueur 2 manquant")?;
        let old_winner = if old.is_unplayed {
            None
        } else {
            bracket_match_winner(&old)
        };
        let new_winner = bracket_winner_from_scores(
            p1,
            p2,
            request.player1_objectives,
            request.player2_objectives,
            request.player1_survivors,
            request.player2_survivors,
        )?;
        let winner_changed = old.phase != TournamentPhase::Pool
            && old_winner.as_ref().map(|name| normalize_name(name))
                != Some(normalize_name(&new_winner));

        if winner_changed {
            if self.bracket_downstream_confirmed(&conn, &old)? {
                bail!("impossible de changer le vainqueur : la phase suivante est déjà jouée");
            }
        }

        conn.execute(
            "
            UPDATE tournament_matches SET
                player1_objectives = ?1, player2_objectives = ?2,
                player1_survivors = ?3, player2_survivors = ?4,
                is_forfeit = 0, is_unplayed = 0, forfeit_player = NULL
            WHERE id = ?5
            ",
            params![
                request.player1_objectives,
                request.player2_objectives,
                request.player1_survivors,
                request.player2_survivors,
                match_id,
            ],
        )?;

        let tournament_id = old.tournament_id;
        if winner_changed {
            self.clear_downstream_bracket(&conn, tournament_id, &old)?;
        }
        drop(conn);

        self.recompute_tournament_standings(tournament_id, k_factor)?;

        if winner_changed {
            let conn = self.conn.lock().unwrap();
            self.rebuild_bracket_progression(&conn, tournament_id)?;
        }

        let updated = self
            .get_match(match_id)?
            .context("match introuvable")?;
        Ok((updated, winner_changed))
    }

    pub fn bracket_elo_snapshot(
        &self,
        tournament_id: i64,
    ) -> Result<HashMap<i64, (f64, f64, String, String)>> {
        let conn = self.conn.lock().unwrap();
        Ok(self
            .list_matches_in_conn(&conn, tournament_id)?
            .into_iter()
            .filter(|m| {
                m.status == TournamentMatchStatus::Confirmed
                    && m.phase != TournamentPhase::Pool
                    && !m.is_forfeit
            })
            .map(|m| {
                (
                    m.id,
                    (
                        m.player1_elo_delta,
                        m.player2_elo_delta,
                        m.player1.clone().unwrap_or_default(),
                        m.player2.clone().unwrap_or_default(),
                    ),
                )
            })
            .collect())
    }

    fn recompute_tournament_standings(&self, tournament_id: i64, k_factor: f64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let tournament = self
            .get_in_conn(&conn, tournament_id)?
            .context("tournoi introuvable")?;

        conn.execute(
            "
            UPDATE tournament_players SET
                pool_points = 0, pool_objectives = 0, pool_survivors = 0, pool_elo_delta = 0
            WHERE tournament_id = ?1
            ",
            params![tournament_id],
        )?;

        let mut matches: Vec<TournamentMatch> = self
            .list_matches_in_conn(&conn, tournament_id)?
            .into_iter()
            .filter(|m| m.status == TournamentMatchStatus::Confirmed)
            .collect();
        matches.sort_by(compare_matches_for_recompute);

        for tm in &matches {
            if tm.is_unplayed {
                continue;
            }

            if tm.is_forfeit {
                self.update_pool_standings_in_conn(&conn, tm)?;
                continue;
            }

            let outcome = outcome_from_objectives(tm.player1_objectives, tm.player2_objectives);
            let tp1 = tournament_points_for_player(
                outcome,
                true,
                tm.player1_objectives,
                tm.player2_objectives,
                false,
            );
            let tp2 = tournament_points_for_player(
                outcome,
                false,
                tm.player1_objectives,
                tm.player2_objectives,
                false,
            );

            if tm.phase == TournamentPhase::Pool {
                let (rating1, rating2) = self.ratings_for_match_in_conn(&conn, tm, k_factor)?;
                let (delta1, delta2) = compute_elo_deltas(rating1, rating2, outcome, k_factor);
                conn.execute(
                    "
                    UPDATE tournament_matches SET
                        player1_tournament_points = ?1, player2_tournament_points = ?2,
                        outcome = ?3,
                        player1_elo_delta = ?4, player2_elo_delta = ?5,
                        player1_rating_used = ?6, player2_rating_used = ?7
                    WHERE id = ?8
                    ",
                    params![
                        tp1,
                        tp2,
                        outcome_to_str(outcome),
                        delta1,
                        delta2,
                        rating1,
                        rating2,
                        tm.id,
                    ],
                )?;

                let p1_key = normalize_name(tm.player1.as_ref().unwrap());
                let p2_key = normalize_name(tm.player2.as_ref().unwrap());
                conn.execute(
                    "
                    UPDATE tournament_players SET pool_elo_delta = pool_elo_delta + ?1
                    WHERE tournament_id = ?2 AND player_name_key = ?3
                    ",
                    params![delta1, tournament_id, p1_key],
                )?;
                conn.execute(
                    "
                    UPDATE tournament_players SET pool_elo_delta = pool_elo_delta + ?2
                    WHERE tournament_id = ?1 AND player_name_key = ?3
                    ",
                    params![tournament_id, delta2, p2_key],
                )?;
            } else {
                conn.execute(
                    "
                    UPDATE tournament_matches SET
                        player1_tournament_points = ?1, player2_tournament_points = ?2,
                        outcome = ?3
                    WHERE id = ?4
                    ",
                    params![tp1, tp2, outcome_to_str(outcome), tm.id],
                )?;
            }

            let refreshed = self
                .get_match_in_conn(&conn, tm.id)?
                .context("match introuvable")?;
            self.update_pool_standings_in_conn(&conn, &refreshed)?;
        }

        if tournament.pools_finalized_at.is_some() {
            conn.execute(
                "
                UPDATE tournament_players SET
                    bracket_rating = start_rating + pool_elo_delta
                WHERE tournament_id = ?1
                ",
                params![tournament_id],
            )?;

            let bracket_matches: Vec<TournamentMatch> = matches
                .into_iter()
                .filter(|m| m.phase != TournamentPhase::Pool && !m.is_forfeit)
                .collect();

            for tm in bracket_matches {
                let (rating1, rating2) = self.ratings_for_match_in_conn(&conn, &tm, k_factor)?;
                let outcome = outcome_from_objectives(tm.player1_objectives, tm.player2_objectives);
                let (delta1, delta2) = compute_elo_deltas(rating1, rating2, outcome, k_factor);

                conn.execute(
                    "
                    UPDATE tournament_matches SET
                        player1_elo_delta = ?1, player2_elo_delta = ?2,
                        player1_rating_used = ?3, player2_rating_used = ?4,
                        outcome = ?5
                    WHERE id = ?6
                    ",
                    params![
                        delta1,
                        delta2,
                        rating1,
                        rating2,
                        outcome_to_str(outcome),
                        tm.id,
                    ],
                )?;

                let p1_key = normalize_name(tm.player1.as_ref().unwrap());
                let p2_key = normalize_name(tm.player2.as_ref().unwrap());
                conn.execute(
                    "
                    UPDATE tournament_players SET bracket_rating = bracket_rating + ?1
                    WHERE tournament_id = ?2 AND player_name_key = ?3
                    ",
                    params![delta1, tournament_id, p1_key],
                )?;
                conn.execute(
                    "
                    UPDATE tournament_players SET bracket_rating = bracket_rating + ?2
                    WHERE tournament_id = ?1 AND player_name_key = ?3
                    ",
                    params![tournament_id, delta2, p2_key],
                )?;
            }
        }

        Ok(())
    }

    fn bracket_downstream_confirmed(
        &self,
        conn: &Connection,
        tm: &TournamentMatch,
    ) -> Result<bool> {
        let Some(next_phase) = next_bracket_phase(tm.phase) else {
            return Ok(false);
        };
        let slot = tm.bracket_slot.unwrap_or(0);
        let next_slot = slot / 2;
        self.bracket_slot_confirmed(conn, tm.tournament_id, next_phase, next_slot)
    }

    fn bracket_slot_confirmed(
        &self,
        conn: &Connection,
        tournament_id: i64,
        phase: TournamentPhase,
        slot: u32,
    ) -> Result<bool> {
        let status: Option<String> = conn.query_row(
            "
            SELECT status FROM tournament_matches
            WHERE tournament_id = ?1 AND phase = ?2 AND bracket_slot = ?3
            ",
            params![tournament_id, phase.as_str(), slot],
            |row| row.get(0),
        ).optional()?;

        if status.as_deref() == Some(TournamentMatchStatus::Confirmed.as_str()) {
            return Ok(true);
        }

        let Some(next_phase) = next_bracket_phase(phase) else {
            return Ok(false);
        };
        self.bracket_slot_confirmed(conn, tournament_id, next_phase, slot / 2)
    }

    fn clear_downstream_bracket(
        &self,
        conn: &Connection,
        tournament_id: i64,
        tm: &TournamentMatch,
    ) -> Result<()> {
        self.clear_downstream_from(conn, tournament_id, tm.phase, tm.bracket_slot.unwrap_or(0))
    }

    fn clear_downstream_from(
        &self,
        conn: &Connection,
        tournament_id: i64,
        phase: TournamentPhase,
        slot: u32,
    ) -> Result<()> {
        let Some(next_phase) = next_bracket_phase(phase) else {
            return Ok(());
        };

        let next_slot = slot / 2;
        let is_player1 = slot % 2 == 0;
        let column = if is_player1 { "player1" } else { "player2" };

        conn.execute(
            &format!(
                "
                UPDATE tournament_matches SET {column} = NULL
                WHERE tournament_id = ?1 AND phase = ?2 AND bracket_slot = ?3
                  AND status != ?4
                "
            ),
            params![
                tournament_id,
                next_phase.as_str(),
                next_slot,
                TournamentMatchStatus::Confirmed.as_str(),
            ],
        )?;

        if phase == TournamentPhase::RoundOf16 {
            conn.execute(
                "
                UPDATE tournament_matches SET player2 = NULL
                WHERE tournament_id = ?1 AND phase = 'quarter' AND bracket_slot = ?2
                  AND status != ?3
                ",
                params![
                    tournament_id,
                    slot,
                    TournamentMatchStatus::Confirmed.as_str(),
                ],
            )?;
        }

        self.clear_downstream_from(conn, tournament_id, next_phase, next_slot)
    }

    fn rebuild_bracket_progression(&self, conn: &Connection, tournament_id: i64) -> Result<()> {
        let mut matches: Vec<TournamentMatch> = self
            .list_matches_in_conn(conn, tournament_id)?
            .into_iter()
            .filter(|m| {
                m.status == TournamentMatchStatus::Confirmed && m.phase != TournamentPhase::Pool
            })
            .collect();
        matches.sort_by(compare_matches_for_recompute);

        for tm in matches {
            let fresh = self
                .get_match_in_conn(conn, tm.id)?
                .context("match introuvable")?;
            self.advance_bracket_in_conn(conn, &fresh)?;
        }
        Ok(())
    }

    pub fn sync_bracket_progression(&self, tournament_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        self.rebuild_bracket_progression(&conn, tournament_id)
    }

    fn apply_confirmed_match(&self, match_id: i64, _k_factor: f64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let tm = self
            .get_match_in_conn(&conn, match_id)?
            .context("match introuvable")?;

        if tm.status != TournamentMatchStatus::Confirmed {
            return Ok(());
        }

        if tm.is_unplayed {
            return Ok(());
        }

        if tm.is_forfeit {
            self.update_pool_standings_in_conn(&conn, &tm)?;
            if tm.phase != TournamentPhase::Pool {
                self.advance_bracket_in_conn(&conn, &tm)?;
            }
            return Ok(());
        }

        if tm.phase != TournamentPhase::Pool {
            bracket_match_winner(&tm).context(
                "match nul en arbre : les points de survivants doivent départager les joueurs",
            )?;
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
        self.get(tournament_id)?.context("tournoi introuvable")
    }

    pub fn setup_bracket(
        &self,
        tournament_id: i64,
        request: &SetupBracketRequest,
    ) -> Result<Vec<TournamentMatch>> {
        let conn = self.conn.lock().unwrap();
        let tournament = self
            .get_in_conn(&conn, tournament_id)?
            .context("tournoi introuvable")?;

        if tournament.status != TournamentStatus::Started {
            bail!("tournoi non démarré");
        }
        if tournament.pools_finalized_at.is_none() {
            bail!("clôturez d'abord les poules");
        }

        let confirmed: i64 = conn.query_row(
            "
            SELECT COUNT(*) FROM tournament_matches
            WHERE tournament_id = ?1 AND phase != 'pool' AND status = 'confirmed'
            ",
            params![tournament_id],
            |row| row.get(0),
        )?;
        if confirmed > 0 {
            bail!("impossible de modifier l'arbre après des matchs confirmés");
        }

        let expected_count = match tournament.bracket_format {
            BracketFormat::QuartersDirect | BracketFormat::RoundOf16 => 4,
            BracketFormat::RoundOf16Full => 8,
        };

        if request.matches.len() != expected_count {
            bail!("{expected_count} match(s) requis pour ce format");
        }

        let mut slots: Vec<u32> = request.matches.iter().map(|m| m.bracket_slot).collect();
        slots.sort_unstable();
        slots.dedup();
        if slots.len() != expected_count {
            bail!("slots dupliqués ou invalides");
        }
        for (index, slot) in slots.iter().enumerate() {
            if *slot != index as u32 {
                bail!("slots dupliqués ou invalides");
            }
        }

        let qualified = self.qualified_players_in_conn(&conn, tournament_id)?;
        let mut used = std::collections::HashSet::new();
        for setup in &request.matches {
            for player in [&setup.player1, &setup.player2] {
                let key = normalize_name(player);
                if !qualified.contains(&key) {
                    bail!("joueur non qualifié: {player}");
                }
                if !used.insert(key) {
                    bail!("joueur utilisé plusieurs fois: {player}");
                }
            }

            if matches!(tournament.bracket_format, BracketFormat::RoundOf16) {
                let first = setup
                    .quarter_player1
                    .as_ref()
                    .context("1er de poule requis pour les quarts")?;
                let key = normalize_name(first);
                if !qualified.contains(&key) {
                    bail!("joueur non qualifié: {first}");
                }
                if !used.insert(key) {
                    bail!("joueur utilisé plusieurs fois: {first}");
                }
            }
        }

        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM tournament_matches WHERE tournament_id = ?1 AND phase != 'pool'",
            params![tournament_id],
        )?;

        match tournament.bracket_format {
            BracketFormat::QuartersDirect => {
                for setup in &request.matches {
                    tx.execute(
                        "
                        INSERT INTO tournament_matches
                            (tournament_id, phase, bracket_slot, player1, player2, status)
                        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                        ",
                        params![
                            tournament_id,
                            TournamentPhase::Quarter.as_str(),
                            setup.bracket_slot,
                            setup.player1,
                            setup.player2,
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
            }
            BracketFormat::RoundOf16 => {
                for setup in &request.matches {
                    tx.execute(
                        "
                        INSERT INTO tournament_matches
                            (tournament_id, phase, bracket_slot, player1, player2, status)
                        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                        ",
                        params![
                            tournament_id,
                            TournamentPhase::RoundOf16.as_str(),
                            setup.bracket_slot,
                            setup.player1,
                            setup.player2,
                            TournamentMatchStatus::Scheduled.as_str(),
                        ],
                    )?;
                    tx.execute(
                        "
                        INSERT INTO tournament_matches
                            (tournament_id, phase, bracket_slot, player1, status)
                        VALUES (?1, ?2, ?3, ?4, ?5)
                        ",
                        params![
                            tournament_id,
                            TournamentPhase::Quarter.as_str(),
                            setup.bracket_slot,
                            setup.quarter_player1,
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
            }
            BracketFormat::RoundOf16Full => {
                for setup in &request.matches {
                    tx.execute(
                        "
                        INSERT INTO tournament_matches
                            (tournament_id, phase, bracket_slot, player1, player2, status)
                        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                        ",
                        params![
                            tournament_id,
                            TournamentPhase::RoundOf16.as_str(),
                            setup.bracket_slot,
                            setup.player1,
                            setup.player2,
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
            }
        }

        tx.commit()?;
        drop(conn);
        // Assigne les scénarios d'arbre s'ils sont déjà choisis ; sinon l'orga pourra le faire après.
        let _ = self.assign_bracket_scenarios(tournament_id);
        let conn = self.conn.lock().unwrap();
        self.list_matches_in_conn(&conn, tournament_id)
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
        drop(conn);
        let _ = self.assign_bracket_scenarios(tournament_id);
        let conn = self.conn.lock().unwrap();
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

        let ranked: Vec<Vec<PoolPlayer>> = standings
            .iter()
            .map(|(_, players)| {
                let mut sorted = players.clone();
                sort_pool_standings(&mut sorted);
                sorted
            })
            .collect();

        let barrages = round_of_16_barrage_pairings(&ranked)
            .context("impossible de générer les barrages (2e ou 3e manquant)")?;

        for barrage in &barrages {
            tx.execute(
                "
                INSERT INTO tournament_matches
                    (tournament_id, phase, bracket_slot, player1, player2, status)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ",
                params![
                    tournament_id,
                    TournamentPhase::RoundOf16.as_str(),
                    barrage.bracket_slot,
                    barrage.player1,
                    barrage.player2,
                    TournamentMatchStatus::Scheduled.as_str(),
                ],
            )?;
            tx.execute(
                "
                INSERT INTO tournament_matches
                    (tournament_id, phase, bracket_slot, player1, status)
                VALUES (?1, ?2, ?3, ?4, ?5)
                ",
                params![
                    tournament_id,
                    TournamentPhase::Quarter.as_str(),
                    barrage.bracket_slot,
                    barrage.quarter_player1,
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
        let winner = bracket_match_winner(tm).context(
            "impossible de déterminer le vainqueur (égalité aux points de survivants sur un nul)",
        )?;

        if tm.phase == TournamentPhase::RoundOf16
            && matches!(
                self.get_in_conn(conn, tm.tournament_id)?.unwrap().bracket_format,
                BracketFormat::RoundOf16
            )
        {
            // Barrage : le 1er de poule est déjà en player1 du quart, le vainqueur va en player2.
            self.fill_quarters_after_r16(conn, tm)?;
            return Ok(());
        }

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

        Ok(())
    }

    fn fill_quarters_after_r16(&self, conn: &Connection, tm: &TournamentMatch) -> Result<()> {
        let slot = tm.bracket_slot.unwrap_or(0);
        let r16_winner = bracket_match_winner(tm).context(
            "impossible de déterminer le vainqueur (égalité aux points de survivants sur un nul)",
        )?;

        conn.execute(
            "
            UPDATE tournament_matches SET player2 = ?1
            WHERE tournament_id = ?2 AND phase = 'quarter' AND bracket_slot = ?3
            ",
            params![r16_winner, tm.tournament_id, slot],
        )?;
        Ok(())
    }

    fn qualified_players_in_conn(
        &self,
        conn: &Connection,
        tournament_id: i64,
    ) -> Result<std::collections::HashSet<String>> {
        let tournament = self
            .get_in_conn(conn, tournament_id)?
            .context("tournoi introuvable")?;
        let pools = self.list_pools_in_conn(conn, tournament_id)?;
        let top_n = match tournament.bracket_format {
            BracketFormat::QuartersDirect | BracketFormat::RoundOf16Full => 2,
            BracketFormat::RoundOf16 => 3,
        };

        let mut qualified = std::collections::HashSet::new();
        for pool in pools {
            let mut sorted = pool.players.clone();
            sorted.sort_by(|a, b| {
                b.points
                    .cmp(&a.points)
                    .then_with(|| b.objectives.cmp(&a.objectives))
                    .then_with(|| b.survivors.cmp(&a.survivors))
            });
            for player in sorted.into_iter().take(top_n) {
                qualified.insert(normalize_name(&player.player_name));
            }
        }
        Ok(qualified)
    }

    /// Recalcule les `final_placement` d'un tournoi terminé (après correction d'arbre).
    pub fn refresh_final_placements(&self, tournament_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE tournament_players SET final_placement = NULL WHERE tournament_id = ?1",
            params![tournament_id],
        )?;
        self.assign_final_placements_in_conn(&conn, tournament_id)
    }

    fn assign_final_placements_in_conn(
        &self,
        conn: &Connection,
        tournament_id: i64,
    ) -> Result<()> {
        let tournament = self
            .get_in_conn(conn, tournament_id)?
            .context("tournoi introuvable")?;

        if tournament.status != TournamentStatus::Completed {
            return Ok(());
        }

        let bracket_format = BracketFormat::parse(tournament.bracket_format.as_str())
            .unwrap_or(BracketFormat::QuartersDirect);
        let matches = self.list_matches_in_conn(conn, tournament_id)?;
        let placements = compute_bracket_placements(&matches, bracket_format);

        for (player_name, placement) in placements {
            conn.execute(
                "
                UPDATE tournament_players SET final_placement = ?1
                WHERE tournament_id = ?2 AND player_name_key = ?3
                ",
                params![placement, tournament_id, normalize_name(&player_name)],
            )?;
        }

        Ok(())
    }

    fn complete_tournament_in_conn(
        &self,
        conn: &Connection,
        tournament_id: i64,
        _winner: &str,
    ) -> Result<()> {
        let now = now_unix();
        conn.execute(
            "
            UPDATE tournaments SET status = ?1, completed_at = ?2 WHERE id = ?3
            ",
            params![TournamentStatus::Completed.as_str(), now, tournament_id],
        )?;

        self.assign_final_placements_in_conn(conn, tournament_id)?;

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

        let mut tournament_ids = conn.prepare(
            "
            SELECT DISTINCT tp.tournament_id
            FROM tournament_players tp
            JOIN tournaments t ON t.id = tp.tournament_id
            WHERE tp.player_name_key = ?1 AND t.status = 'completed'
            ",
        )?;
        let ids: Vec<i64> = tournament_ids
            .query_map(params![key], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        for tournament_id in ids {
            self.assign_final_placements_in_conn(&conn, tournament_id)?;
        }

        let mut stmt = conn.prepare(
            "
            SELECT t.id, t.name, tp.final_placement, t.completed_at, tr.army_id
            FROM tournament_players tp
            JOIN tournaments t ON t.id = tp.tournament_id
            LEFT JOIN tournament_registrations tr
                ON tr.tournament_id = tp.tournament_id
               AND tr.player_name_key = tp.player_name_key
               AND tr.status = 'approved'
            WHERE tp.player_name_key = ?1 AND t.status = 'completed'
            ORDER BY t.id DESC
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
                army_id: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn star_counts(&self) -> Result<std::collections::HashMap<String, u32>> {
        let conn = self.conn.lock().unwrap();

        let completed_ids: Vec<i64> = {
            let mut stmt = conn.prepare(
                "SELECT id FROM tournaments WHERE status = 'completed'",
            )?;
            let ids = stmt
                .query_map([], |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?;
            ids
        };
        for tournament_id in completed_ids {
            self.assign_final_placements_in_conn(&conn, tournament_id)?;
        }

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
              AND status = 'confirmed' AND is_forfeit = 0 AND is_unplayed = 0 AND elo_applied_at IS NULL
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
            SELECT id, name, description, status, pool_count, bracket_format,
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
                   waitlist_position, requested_at, reviewed_at, reviewed_by, army_id,
                   army_list_1, army_list_2, bracket_list_1, bracket_list_2
            FROM tournament_registrations
            WHERE tournament_id = ?1
            ORDER BY requested_at ASC
            ",
        )?;
        let rows = stmt.query_map(params![tournament_id], row_to_registration)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn ensure_tournament_exists(&self, conn: &Connection, tournament_id: i64) -> Result<()> {
        if self.get_in_conn(conn, tournament_id)?.is_none() {
            bail!("tournoi introuvable");
        }
        Ok(())
    }

    fn list_scenarios_in_conn(
        &self,
        conn: &Connection,
        tournament_id: i64,
        kind: &str,
    ) -> Result<Vec<TournamentScenarioSlot>> {
        let mut stmt = conn.prepare(
            "
            SELECT ts.kind, ts.slot, ts.scenario_id, s.name, COALESCE(s.slug, '')
            FROM tournament_scenarios ts
            JOIN scenarios s ON s.id = ts.scenario_id
            WHERE ts.tournament_id = ?1 AND ts.kind = ?2
            ORDER BY ts.slot
            ",
        )?;
        let rows = stmt.query_map(params![tournament_id, kind], |row| {
            Ok(TournamentScenarioSlot {
                kind: row.get(0)?,
                slot: row.get(1)?,
                scenario_id: row.get(2)?,
                scenario_name: row.get(3)?,
                scenario_slug: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn replace_scenario_kind(
        &self,
        tournament_id: i64,
        kind: &str,
        scenario_ids: &[i64],
        slots: &[&str],
    ) -> Result<Vec<TournamentScenarioSlot>> {
        if scenario_ids.len() != slots.len() {
            bail!("nombre de scénarios incohérent");
        }
        let mut unique = scenario_ids.to_vec();
        unique.sort_unstable();
        unique.dedup();
        if unique.len() != scenario_ids.len() {
            bail!("les scénarios doivent être distincts");
        }

        let conn = self.conn.lock().unwrap();
        self.ensure_tournament_exists(&conn, tournament_id)?;
        for id in scenario_ids {
            let exists: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM scenarios WHERE id = ?1)",
                params![id],
                |row| row.get(0),
            )?;
            if !exists {
                bail!("scénario introuvable ({id})");
            }
        }

        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM tournament_scenarios WHERE tournament_id = ?1 AND kind = ?2",
            params![tournament_id, kind],
        )?;
        for (slot, scenario_id) in slots.iter().zip(scenario_ids.iter()) {
            tx.execute(
                "
                INSERT INTO tournament_scenarios (tournament_id, kind, slot, scenario_id)
                VALUES (?1, ?2, ?3, ?4)
                ",
                params![tournament_id, kind, *slot, scenario_id],
            )?;
        }
        tx.commit()?;
        self.list_scenarios_in_conn(&conn, tournament_id, kind)
    }

    fn pick_random_scenario_ids(&self, count: usize) -> Result<Vec<i64>> {
        use rand::seq::SliceRandom;
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "
            SELECT id FROM scenarios
            WHERE pack_id IS NOT NULL
            ORDER BY sort_order, id
            ",
        )?;
        let mut ids: Vec<i64> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        if ids.len() < count {
            bail!("pas assez de scénarios dans le catalogue ({})", ids.len());
        }
        ids.shuffle(&mut rand::rng());
        ids.truncate(count);
        Ok(ids)
    }

    fn pick_random_scenario_excluding(
        &self,
        conn: &Connection,
        exclude: &[i64],
    ) -> Result<i64> {
        use rand::seq::SliceRandom;
        let mut stmt = conn.prepare(
            "
            SELECT id FROM scenarios
            WHERE pack_id IS NOT NULL
            ORDER BY sort_order, id
            ",
        )?;
        let mut ids: Vec<i64> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .filter(|id| !exclude.contains(id))
            .collect();
        if ids.is_empty() {
            // If everything is used, allow any pack scenario.
            let mut all: Vec<i64> = conn
                .prepare("SELECT id FROM scenarios WHERE pack_id IS NOT NULL")?
                .query_map([], |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?;
            all.shuffle(&mut rand::rng());
            return all
                .into_iter()
                .next()
                .context("aucun scénario disponible");
        }
        ids.shuffle(&mut rand::rng());
        ids.into_iter().next().context("aucun scénario disponible")
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
                   COALESCE(tp.pool_survivors, 0),
                   tr.army_id
            FROM pool_players pp
            LEFT JOIN tournament_players tp
                ON tp.tournament_id = ?2 AND tp.player_name_key = pp.player_name_key
            LEFT JOIN tournament_registrations tr
                ON tr.tournament_id = ?2 AND tr.player_name_key = pp.player_name_key
            WHERE pp.pool_id = ?1
            ORDER BY pp.seed
            ",
        )?;
        let rows = stmt.query_map(params![pool_id, tournament_id], |row| {
            Ok(PoolPlayer {
                player_name: row.get(0)?,
                player_display_name: None,
                army_id: row.get(5)?,
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

        let mut matches = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(tm) = self.get_match_in_conn(conn, id)? {
                matches.push(tm);
            }
        }
        Ok(matches)
    }

    fn get_match_in_conn(&self, conn: &Connection, id: i64) -> Result<Option<TournamentMatch>> {
        let mut stmt = conn.prepare(
            "
            SELECT tm.id, tm.tournament_id, tm.phase, tm.pool_id, tm.bracket_slot,
                   tm.player1, tm.player2,
                   tm.player1_objectives, tm.player2_objectives,
                   tm.player1_survivors, tm.player2_survivors,
                   tm.player1_tournament_points, tm.player2_tournament_points,
                   tm.outcome, tm.is_forfeit, tm.forfeit_player,
                   tm.player1_elo_delta, tm.player2_elo_delta,
                   tm.player1_rating_used, tm.player2_rating_used,
                   tm.elo_applied_at, tm.status,
                   tm.submitted_by_user_id, tm.submitted_at,
                   tm.confirmed_by_user_id, tm.confirmed_at,
                   tm.scenario_id, tm.scenario_other,
                   COALESCE(s.name, tm.scenario_other),
                   tm.player1_army_id, tm.player2_army_id, tm.played_at, tm.is_unplayed,
                   tm.player1_army_list_code, tm.player2_army_list_code
            FROM tournament_matches tm
            LEFT JOIN scenarios s ON s.id = tm.scenario_id
            WHERE tm.id = ?1
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
                   waitlist_position, requested_at, reviewed_at, reviewed_by, army_id,
                   army_list_1, army_list_2, bracket_list_1, bracket_list_2
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

fn phase_order(phase: TournamentPhase) -> u8 {
    match phase {
        TournamentPhase::Pool => 0,
        TournamentPhase::RoundOf16 => 1,
        TournamentPhase::Quarter => 2,
        TournamentPhase::Semi => 3,
        TournamentPhase::Final => 4,
    }
}

fn compare_matches_for_recompute(a: &TournamentMatch, b: &TournamentMatch) -> std::cmp::Ordering {
    phase_order(a.phase)
        .cmp(&phase_order(b.phase))
        .then(a.bracket_slot.unwrap_or(0).cmp(&b.bracket_slot.unwrap_or(0)))
        .then(a.id.cmp(&b.id))
}

fn outcome_from_objectives(player1_objectives: u8, player2_objectives: u8) -> MatchOutcome {
    if player1_objectives > player2_objectives {
        MatchOutcome::Player1Win
    } else if player2_objectives > player1_objectives {
        MatchOutcome::Player2Win
    } else {
        MatchOutcome::Draw
    }
}

fn bracket_winner_from_scores(
    player1: &str,
    player2: &str,
    p1_obj: u8,
    p2_obj: u8,
    p1_surv: u16,
    p2_surv: u16,
) -> Result<String> {
    match outcome_from_objectives(p1_obj, p2_obj) {
        MatchOutcome::Player1Win => Ok(player1.to_string()),
        MatchOutcome::Player2Win => Ok(player2.to_string()),
        MatchOutcome::Draw => {
            if p1_surv > p2_surv {
                Ok(player1.to_string())
            } else if p2_surv > p1_surv {
                Ok(player2.to_string())
            } else {
                bail!("match nul : égalité aux points de survivants")
            }
        }
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
    let status_str: String = row.get(3)?;
    let format_str: String = row.get(5)?;
    Ok(Tournament {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        status: TournamentStatus::parse(&status_str).unwrap_or(TournamentStatus::Draft),
        pool_count: row.get(4)?,
        bracket_format: BracketFormat::parse(&format_str)
            .unwrap_or(BracketFormat::QuartersDirect),
        created_at: row.get(6)?,
        started_at: row.get(7)?,
        pools_finalized_at: row.get(8)?,
        completed_at: row.get(9)?,
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
        army_list_1: row.get(10)?,
        army_list_2: row.get(11)?,
        bracket_list_1: row.get(12)?,
        bracket_list_2: row.get(13)?,
        has_army_lists: false,
        has_bracket_lists: false,
        has_army_list_2: false,
        has_bracket_list_2: false,
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
        scenario_other: row.get(27)?,
        scenario_name: row.get(28)?,
        player1_army_id: row.get(29)?,
        player2_army_id: row.get(30)?,
        played_at: row.get(31)?,
        is_unplayed: row.get::<_, i64>(32)? != 0,
        player1_army_list_code: row.get(33)?,
        player2_army_list_code: row.get(34)?,
    })
}

fn registration_army_id_in_conn(
    conn: &Connection,
    tournament_id: i64,
    player_name: &str,
) -> Result<Option<u32>> {
    let mut stmt = conn.prepare(
        "
        SELECT army_id FROM tournament_registrations
        WHERE tournament_id = ?1 AND player_name_key = ?2
        ",
    )?;
    let mut rows = stmt.query(params![tournament_id, normalize_name(player_name)])?;
    if let Some(row) = rows.next()? {
        let army_id: Option<u32> = row.get(0)?;
        return Ok(army_id);
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_copy_of_db() -> (std::path::PathBuf, TournamentStore) {
        let src = Path::new("data/poissonnerie.db");
        if !src.exists() {
            panic!("data/poissonnerie.db introuvable pour les tests");
        }
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dst = std::env::temp_dir().join(format!("poissonnerie-test-{suffix}.db"));
        fs::copy(src, &dst).unwrap();
        let store = TournamentStore::open(&dst).unwrap();
        (dst, store)
    }

    #[test]
    fn correct_match_score_persists_objectives() {
        let (db_path, store) = temp_copy_of_db();
        let before = store.get_match(1).unwrap().unwrap();
        assert_eq!(before.status, TournamentMatchStatus::Confirmed);

        let request = SubmitMatchRequest {
            player1_objectives: 3,
            player2_objectives: 7,
            player1_survivors: 42,
            player2_survivors: 84,
            player1_army_id: None,
            player2_army_id: None,
            player1_list_slot: None,
            player2_list_slot: None,
            scenario_id: None,
            scenario_other: None,
        };

        let (updated, _) = store.correct_match_score(1, &request, 32.0).unwrap();
        assert_eq!(updated.player1_objectives, 3);
        assert_eq!(updated.player2_objectives, 7);
        assert_eq!(updated.player1_survivors, 42);
        assert_eq!(updated.player2_survivors, 84);

        let reloaded = store.get_match(1).unwrap().unwrap();
        assert_eq!(reloaded.player1_objectives, 3);
        assert_eq!(reloaded.player2_objectives, 7);

        let _ = fs::remove_file(db_path);
    }

    #[test]
    fn correct_match_score_clears_forfeit_and_unplayed() {
        let (db_path, store) = temp_copy_of_db();
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "
            UPDATE tournament_matches SET
                player1_objectives = 0, player2_objectives = 0,
                player1_survivors = 0, player2_survivors = 0,
                player1_tournament_points = 5, player2_tournament_points = 0,
                outcome = 'player1_win', is_forfeit = 1, is_unplayed = 0,
                forfeit_player = 'player2', status = 'confirmed'
            WHERE id = 1
            ",
            [],
        )
        .unwrap();
        drop(conn);

        let request = SubmitMatchRequest {
            player1_objectives: 6,
            player2_objectives: 2,
            player1_survivors: 100,
            player2_survivors: 40,
            player1_army_id: None,
            player2_army_id: None,
            player1_list_slot: None,
            player2_list_slot: None,
            scenario_id: None,
            scenario_other: None,
        };

        let (updated, _) = store.correct_match_score(1, &request, 32.0).unwrap();
        assert!(!updated.is_forfeit);
        assert!(!updated.is_unplayed);
        assert!(updated.forfeit_player.is_none());
        assert_eq!(updated.player1_objectives, 6);
        assert_eq!(updated.player2_objectives, 2);
        assert_eq!(updated.outcome, Some(MatchOutcome::Player1Win));
        assert_eq!(updated.player1_tournament_points, 5);
        assert_eq!(updated.player2_tournament_points, 0);

        let conn = store.conn.lock().unwrap();
        conn.execute(
            "
            UPDATE tournament_matches SET
                player1_objectives = 0, player2_objectives = 0,
                player1_survivors = 0, player2_survivors = 0,
                player1_tournament_points = 0, player2_tournament_points = 0,
                outcome = NULL, is_forfeit = 0, is_unplayed = 1,
                forfeit_player = NULL, status = 'confirmed'
            WHERE id = 1
            ",
            [],
        )
        .unwrap();
        drop(conn);

        let request = SubmitMatchRequest {
            player1_objectives: 4,
            player2_objectives: 4,
            player1_survivors: 80,
            player2_survivors: 60,
            player1_army_id: None,
            player2_army_id: None,
            player1_list_slot: None,
            player2_list_slot: None,
            scenario_id: None,
            scenario_other: None,
        };

        let (updated, _) = store.correct_match_score(1, &request, 32.0).unwrap();
        assert!(!updated.is_forfeit);
        assert!(!updated.is_unplayed);
        assert_eq!(updated.outcome, Some(MatchOutcome::Draw));
        assert_eq!(updated.player1_tournament_points, 2);
        assert_eq!(updated.player2_tournament_points, 2);

        let _ = fs::remove_file(db_path);
    }

    #[test]
    fn sync_coupe_2_bracket_after_draw_semi() {
        let path = Path::new("data/poissonnerie.db");
        if !path.exists() {
            return;
        }

        let store = TournamentStore::open(path).unwrap();
        store.sync_bracket_progression(2).unwrap();

        let final_match = store.get_match(98).unwrap().expect("finale coupe 2");
        assert_eq!(final_match.player1.as_deref(), Some("Ayadan"));
        assert_eq!(final_match.player2.as_deref(), Some("Kantain"));
    }
}

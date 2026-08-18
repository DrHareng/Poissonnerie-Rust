use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::match_record::{
    decode_slug_list, encode_slug_list, now_unix, MatchRecord, MatchReport, MatchScores, MatchStatus,
};
use crate::migrate::migrate;
use crate::player::{MatchOutcome, Player};

const LEGACY_JSON_PATH: &str = "data/leaderboard.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Leaderboard {
    pub players: HashMap<String, Player>,
    #[serde(default)]
    pub matches: Vec<MatchRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlayerArmyUsage {
    pub army_id: u32,
    pub matches: u32,
    pub last_played_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArmyMatchStats {
    pub army_id: u32,
    pub wins: u32,
    pub draws: u32,
    pub losses: u32,
}

#[derive(Debug, Clone, Default)]
pub struct InProgressMatchUpdate {
    pub scenario_id: Option<i64>,
    pub scenario_other: Option<String>,
    pub scenario_name: Option<Option<String>>,
    /// `Some` = définir / effacer l'URL (chaîne vide → None).
    pub scenario_url: Option<String>,
    pub player1_secondary_slugs: Option<Vec<String>>,
    pub player2_secondary_slugs: Option<Vec<String>>,
    pub secondary_pool_slugs: Option<Vec<String>>,
    pub player1_chosen_secondary: Option<Option<String>>,
    pub player2_chosen_secondary: Option<Option<String>>,
    pub lieutenant_winner: Option<String>,
    pub lieutenant_winner_choice: Option<String>,
    pub lieutenant_other_choice: Option<String>,
    pub partie_step: Option<String>,
    /// Used for Combat de l'Esprit: drop the initial 3+3 draw so the draft can rewrite.
    pub clear_secondary_draws: bool,
}

impl ArmyMatchStats {
    pub fn total(&self) -> u32 {
        self.wins + self.draws + self.losses
    }

    pub fn win_rate(&self) -> f64 {
        let total = self.total();
        if total == 0 {
            0.0
        } else {
            let effective_wins = self.wins as f64 + 0.5 * self.draws as f64;
            (effective_wins / total as f64) * 100.0
        }
    }
}

impl Leaderboard {
    pub fn load(db_path: &Path) -> Result<Self> {
        if !db_path.exists() {
            if let Some(parent) = db_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("impossible de créer {}", parent.display()))?;
            }
            if Path::new(LEGACY_JSON_PATH).exists() {
                let board = Self::load_from_json(Path::new(LEGACY_JSON_PATH))?;
                board.save(db_path)?;
                return Ok(board);
            }
            return Ok(Self::default());
        }

        let conn = Connection::open(db_path)
            .with_context(|| format!("impossible d'ouvrir {}", db_path.display()))?;
        migrate(&conn)?;

        let mut players = HashMap::new();
        {
            let mut stmt = conn.prepare(
                "SELECT name_key, name, rating, wins, draws, losses, discord_username FROM players",
            )?;
            let rows = stmt.query_map([], |row| {
                let key: String = row.get(0)?;
                let player = Player {
                    name: row.get(1)?,
                    rating: row.get(2)?,
                    wins: row.get(3)?,
                    draws: row.get(4)?,
                    losses: row.get(5)?,
                    discord_username: row.get(6)?,
                };
                Ok((key, player))
            })?;
            for row in rows {
                let (key, player) = row?;
                players.insert(key, player);
            }
        }

        let mut matches = Vec::new();
        {
            let mut stmt = conn.prepare(
                "
                SELECT m.id, m.player1, m.player2, m.outcome,
                       m.player1_old, m.player1_new, m.player2_old, m.player2_new,
                       m.player1_objectives, m.player1_survivors,
                       m.player2_objectives, m.player2_survivors,
                       m.player1_army_id, m.player2_army_id,
                       m.scenario_id, m.scenario_other,
                       COALESCE(s.name, m.scenario_other),
                       m.tournament_id, m.tournament_phase, t.name,
                       m.status,
                       m.player1_secondary_slugs, m.player2_secondary_slugs,
                       m.player1_chosen_secondary, m.player2_chosen_secondary,
                       m.lieutenant_winner, m.lieutenant_winner_choice, m.lieutenant_other_choice,
                       m.partie_step, m.created_by,
                       m.player1_army_list_code, m.player2_army_list_code,
                       m.recorded_at,
                       m.secondary_pool_slugs,
                       m.counts_for_elo,
                       m.scenario_url
                FROM matches m
                LEFT JOIN scenarios s ON s.id = m.scenario_id
                LEFT JOIN tournaments t ON t.id = m.tournament_id
                ORDER BY m.recorded_at DESC, m.id DESC
                ",
            )?;
            let rows = stmt.query_map([], row_to_match)?;
            for row in rows {
                matches.push(row?);
            }
        }

        attach_match_reports(&conn, &mut matches)?;

        if players.is_empty() && matches.is_empty() && Path::new(LEGACY_JSON_PATH).exists() {
            let board = Self::load_from_json(Path::new(LEGACY_JSON_PATH))?;
            board.save(db_path)?;
            return Ok(board);
        }

        Ok(Self { players, matches })
    }

    pub fn save(&self, db_path: &Path) -> Result<()> {
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("impossible de créer {}", parent.display()))?;
        }

        let mut conn = Connection::open(db_path)
            .with_context(|| format!("impossible d'ouvrir {}", db_path.display()))?;
        migrate(&conn)?;

        let tx = conn
            .transaction()
            .context("impossible de démarrer la transaction")?;

        tx.execute("DELETE FROM match_reports", [])?;
        tx.execute("DELETE FROM matches", [])?;
        tx.execute("DELETE FROM players", [])?;

        for (key, player) in &self.players {
            tx.execute(
                "
                INSERT INTO players (name_key, name, rating, wins, draws, losses, discord_username)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                ",
                params![
                    key,
                    player.name,
                    player.rating,
                    player.wins,
                    player.draws,
                    player.losses,
                    player.discord_username,
                ],
            )?;
        }

        for record in &self.matches {
            tx.execute(
                "
                INSERT INTO matches (
                    id, player1, player2, outcome,
                    player1_old, player1_new, player2_old, player2_new,
                    player1_objectives, player1_survivors,
                    player2_objectives, player2_survivors,
                    player1_army_id, player2_army_id,
                    scenario_id, scenario_other,
                    tournament_id, tournament_phase,
                    status,
                    player1_secondary_slugs, player2_secondary_slugs,
                    player1_chosen_secondary, player2_chosen_secondary,
                    lieutenant_winner, lieutenant_winner_choice, lieutenant_other_choice,
                    partie_step, created_by,
                    player1_army_list_code, player2_army_list_code,
                    recorded_at, secondary_pool_slugs, counts_for_elo, scenario_url
                ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                    ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34
                )
                ",
                params![
                    record.id,
                    record.player1,
                    record.player2,
                    outcome_option_to_str(record.outcome),
                    record.player1_old,
                    record.player1_new,
                    record.player2_old,
                    record.player2_new,
                    record.player1_objectives,
                    record.player1_survivors,
                    record.player2_objectives,
                    record.player2_survivors,
                    record.player1_army_id,
                    record.player2_army_id,
                    record.scenario_id,
                    record.scenario_other,
                    record.tournament_id,
                    record.tournament_phase,
                    record.status.as_str(),
                    encode_slug_list(record.player1_secondary_slugs.as_deref()),
                    encode_slug_list(record.player2_secondary_slugs.as_deref()),
                    record.player1_chosen_secondary,
                    record.player2_chosen_secondary,
                    record.lieutenant_winner,
                    record.lieutenant_winner_choice,
                    record.lieutenant_other_choice,
                    record.partie_step,
                    record.created_by,
                    record.player1_army_list_code,
                    record.player2_army_list_code,
                    record.recorded_at,
                    encode_slug_list(record.secondary_pool_slugs.as_deref()),
                    if record.counts_for_elo { 1 } else { 0 },
                    record.scenario_url,
                ],
            )?;

            if let Some(report) = &record.player1_report {
                tx.execute(
                    "
                    INSERT INTO match_reports (id, match_id, player_name, body_md, created_at, updated_at)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                    ",
                    params![
                        report.id,
                        record.id,
                        record.player1,
                        report.body_md,
                        report.created_at,
                        report.updated_at,
                    ],
                )?;
            }
            if let Some(report) = &record.player2_report {
                tx.execute(
                    "
                    INSERT INTO match_reports (id, match_id, player_name, body_md, created_at, updated_at)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                    ",
                    params![
                        report.id,
                        record.id,
                        record.player2,
                        report.body_md,
                        report.created_at,
                        report.updated_at,
                    ],
                )?;
            }
        }

        tx.commit().context("impossible de valider la transaction")?;
        Ok(())
    }

    fn load_from_json(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("impossible de lire {}", path.display()))?;
        let board: Self = serde_json::from_str(&content)
            .with_context(|| format!("fichier JSON invalide : {}", path.display()))?;
        Ok(board)
    }

    pub fn add_player(&mut self, name: &str) -> Result<()> {
        self.insert_player(Player::new(name))
    }

    pub fn add_player_for_discord_username(
        &mut self,
        name: &str,
        discord_username: &str,
    ) -> Result<()> {
        if self.player_exists_for_discord_username(discord_username) {
            bail!("un joueur est déjà associé à ce pseudo Discord");
        }

        self.insert_player(Player::new_with_discord_username(name, discord_username))
    }

    fn insert_player(&mut self, player: Player) -> Result<()> {
        let key = normalize_name(&player.name);
        if self.players.contains_key(&key) {
            bail!("le joueur « {} » existe déjà", player.name);
        }

        if let Some(discord_username) = &player.discord_username {
            if self.player_exists_for_discord_username(discord_username) {
                bail!("un joueur est déjà associé à ce pseudo Discord");
            }
        }

        self.players.insert(key, player);
        Ok(())
    }

    pub fn player_exists_for_discord_username(&self, discord_username: &str) -> bool {
        self.players.values().any(|player| {
            player
                .discord_username
                .as_deref()
                .is_some_and(|stored| stored.eq_ignore_ascii_case(discord_username))
        })
    }

    pub fn get_player_by_discord_username(&self, discord_username: &str) -> Option<&Player> {
        self.players.values().find(|player| {
            player
                .discord_username
                .as_deref()
                .is_some_and(|stored| stored.eq_ignore_ascii_case(discord_username))
        })
    }

    pub fn get_player(&self, name: &str) -> Result<&Player> {
        let key = normalize_name(name);
        self.players
            .get(&key)
            .with_context(|| format!("joueur introuvable : {}", name))
    }

    pub fn get_player_mut(&mut self, name: &str) -> Result<&mut Player> {
        let key = normalize_name(name);
        self.players
            .get_mut(&key)
            .with_context(|| format!("joueur introuvable : {}", name))
    }

    pub fn record_match(
        &mut self,
        player1: &str,
        player2: &str,
        outcome: MatchOutcome,
        k_factor: f64,
        scores: MatchScores,
        player1_army_id: Option<u32>,
        player2_army_id: Option<u32>,
        scenario_id: Option<i64>,
        scenario_other: Option<String>,
        scenario_name: Option<String>,
    ) -> Result<MatchRecord> {
        let scores = scores.validate()?;
        if normalize_name(player1) == normalize_name(player2) {
            bail!("un joueur ne peut pas jouer contre lui-même");
        }

        let key1 = normalize_name(player1);
        let key2 = normalize_name(player2);

        if !self.players.contains_key(&key1) {
            bail!("joueur introuvable : {}", player1);
        }
        if !self.players.contains_key(&key2) {
            bail!("joueur introuvable : {}", player2);
        }

        let update = {
            let old1 = self.players.get(&key1).unwrap().rating;
            let old2 = self.players.get(&key2).unwrap().rating;
            let score1 = outcome.score_for_player1();
            let (new1, new2) = crate::elo::update_ratings(old1, old2, score1, k_factor);
            let score2 = match score1 {
                crate::elo::MatchScore::Win => crate::elo::MatchScore::Loss,
                crate::elo::MatchScore::Draw => crate::elo::MatchScore::Draw,
                crate::elo::MatchScore::Loss => crate::elo::MatchScore::Win,
            };
            let p1 = self.players.get_mut(&key1).unwrap();
            p1.rating = new1;
            p1.record_match(score1);
            let p2 = self.players.get_mut(&key2).unwrap();
            p2.rating = new2;
            p2.record_match(score2);
            crate::player::RatingUpdate {
                player1_old: old1,
                player1_new: new1,
                player2_old: old2,
                player2_new: new2,
            }
        };

        self.insert_match_record(
            &key1,
            &key2,
            outcome,
            update,
            scores,
            player1_army_id,
            player2_army_id,
            scenario_id,
            scenario_other,
            scenario_name,
            None,
            None,
            None,
            None,
            None,
        )
    }

    pub fn apply_match_update(
        &mut self,
        player1: &str,
        player2: &str,
        outcome: MatchOutcome,
        update: crate::player::RatingUpdate,
        scores: MatchScores,
        player1_army_id: Option<u32>,
        player2_army_id: Option<u32>,
        scenario_id: Option<i64>,
        scenario_other: Option<String>,
        scenario_name: Option<String>,
        tournament_id: Option<i64>,
        tournament_phase: Option<String>,
        tournament_name: Option<String>,
        player1_army_list_code: Option<String>,
        player2_army_list_code: Option<String>,
    ) -> Result<MatchRecord> {
        let scores = scores.validate()?;
        let key1 = normalize_name(player1);
        let key2 = normalize_name(player2);

        let p1 = self.players.get_mut(&key1).context("joueur introuvable")?;
        p1.rating = update.player1_new;
        p1.record_match(outcome.score_for_player1());

        let score2 = match outcome.score_for_player1() {
            crate::elo::MatchScore::Win => crate::elo::MatchScore::Loss,
            crate::elo::MatchScore::Draw => crate::elo::MatchScore::Draw,
            crate::elo::MatchScore::Loss => crate::elo::MatchScore::Win,
        };
        let p2 = self.players.get_mut(&key2).context("joueur introuvable")?;
        p2.rating = update.player2_new;
        p2.record_match(score2);

        self.insert_match_record(
            &key1,
            &key2,
            outcome,
            update,
            scores,
            player1_army_id,
            player2_army_id,
            scenario_id,
            scenario_other,
            scenario_name,
            tournament_id,
            tournament_phase,
            tournament_name,
            player1_army_list_code,
            player2_army_list_code,
        )
    }

    fn insert_match_record(
        &mut self,
        key1: &str,
        key2: &str,
        outcome: MatchOutcome,
        update: crate::player::RatingUpdate,
        scores: MatchScores,
        player1_army_id: Option<u32>,
        player2_army_id: Option<u32>,
        scenario_id: Option<i64>,
        scenario_other: Option<String>,
        scenario_name: Option<String>,
        tournament_id: Option<i64>,
        tournament_phase: Option<String>,
        tournament_name: Option<String>,
        player1_army_list_code: Option<String>,
        player2_army_list_code: Option<String>,
    ) -> Result<MatchRecord> {
        let next_id = self
            .matches
            .iter()
            .map(|record| record.id)
            .max()
            .unwrap_or(0)
            + 1;

        let mut record = MatchRecord::from_update(
            next_id,
            self.players.get(key1).unwrap().name.clone(),
            self.players.get(key2).unwrap().name.clone(),
            outcome,
            update,
            scores,
            player1_army_id,
            player2_army_id,
            scenario_id,
            scenario_other,
            scenario_name,
            tournament_id,
            tournament_phase,
            tournament_name,
            now_unix(),
        );
        record.player1_army_list_code = player1_army_list_code;
        record.player2_army_list_code = player2_army_list_code;

        self.matches.insert(0, record.clone());

        Ok(record)
    }

    pub fn recent_matches(&self, limit: usize) -> Vec<&MatchRecord> {
        self.matches.iter().take(limit).collect()
    }

    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    pub fn get_match(&self, id: u64) -> Option<&MatchRecord> {
        self.matches.iter().find(|record| record.id == id)
    }

    pub fn delete_match(&mut self, id: u64) -> Result<MatchRecord> {
        let index = self
            .matches
            .iter()
            .position(|record| record.id == id)
            .ok_or_else(|| anyhow::anyhow!("match introuvable"))?;

        if self.matches[index].tournament_id.is_some() {
            bail!("impossible de supprimer un match lié à un tournoi");
        }

        let record = self.matches.remove(index);

        // Les parties en cours ou amicales n'ont pas impacté l'ELO.
        if record.status == MatchStatus::InProgress
            || record.outcome.is_none()
            || !record.counts_for_elo
        {
            return Ok(record);
        }

        let outcome = record
            .outcome
            .expect("outcome présent pour un match terminé");
        let key1 = normalize_name(&record.player1);
        let key2 = normalize_name(&record.player2);

        {
            let p1 = self
                .players
                .get_mut(&key1)
                .with_context(|| format!("joueur introuvable : {}", record.player1))?;
            p1.rating = record.player1_old;
            adjust_player_match_count(p1, outcome.score_for_player1(), -1);
        }

        {
            let score2 = match outcome.score_for_player1() {
                crate::elo::MatchScore::Win => crate::elo::MatchScore::Loss,
                crate::elo::MatchScore::Draw => crate::elo::MatchScore::Draw,
                crate::elo::MatchScore::Loss => crate::elo::MatchScore::Win,
            };
            let p2 = self
                .players
                .get_mut(&key2)
                .with_context(|| format!("joueur introuvable : {}", record.player2))?;
            p2.rating = record.player2_old;
            adjust_player_match_count(p2, score2, -1);
        }

        Ok(record)
    }

    pub fn start_match(
        &mut self,
        player1: &str,
        player2: &str,
        player1_army_id: u32,
        player2_army_id: u32,
        created_by: &str,
        player1_secondary_slugs: Vec<String>,
        player2_secondary_slugs: Vec<String>,
        counts_for_elo: bool,
    ) -> Result<MatchRecord> {
        if normalize_name(player1) == normalize_name(player2) {
            bail!("un joueur ne peut pas jouer contre lui-même");
        }
        if player1_secondary_slugs.len() != 3 || player2_secondary_slugs.len() != 3 {
            bail!("chaque joueur doit recevoir exactement 3 objectifs secondaires");
        }
        let key1 = normalize_name(player1);
        let key2 = normalize_name(player2);
        if !self.players.contains_key(&key1) {
            bail!("joueur introuvable : {}", player1);
        }
        if !self.players.contains_key(&key2) {
            bail!("joueur introuvable : {}", player2);
        }

        let rating1 = self.players.get(&key1).unwrap().rating;
        let rating2 = self.players.get(&key2).unwrap().rating;
        let next_id = self
            .matches
            .iter()
            .map(|record| record.id)
            .max()
            .unwrap_or(0)
            + 1;

        let record = MatchRecord {
            id: next_id,
            player1: self.players.get(&key1).unwrap().name.clone(),
            player2: self.players.get(&key2).unwrap().name.clone(),
            status: MatchStatus::InProgress,
            outcome: None,
            player1_old: rating1,
            player1_new: rating1,
            player2_old: rating2,
            player2_new: rating2,
            player1_objectives: 0,
            player1_survivors: 0,
            player2_objectives: 0,
            player2_survivors: 0,
            player1_army_id: Some(player1_army_id),
            player2_army_id: Some(player2_army_id),
            scenario_id: None,
            scenario_other: None,
            scenario_url: None,
            scenario_name: None,
            tournament_id: None,
            tournament_phase: None,
            tournament_name: None,
            player1_report: None,
            player2_report: None,
            player1_army_list_code: None,
            player2_army_list_code: None,
            player1_secondary_slugs: Some(player1_secondary_slugs),
            player2_secondary_slugs: Some(player2_secondary_slugs),
            secondary_pool_slugs: None,
            player1_chosen_secondary: None,
            player2_chosen_secondary: None,
            lieutenant_winner: None,
            lieutenant_winner_choice: None,
            lieutenant_other_choice: None,
            partie_step: Some("scenario".to_string()),
            created_by: Some(created_by.to_string()),
            counts_for_elo,
            recorded_at: now_unix(),
        };

        self.matches.insert(0, record.clone());
        Ok(record)
    }

    pub fn update_in_progress_match(
        &mut self,
        id: u64,
        update: InProgressMatchUpdate,
    ) -> Result<MatchRecord> {
        let record = self
            .matches
            .iter_mut()
            .find(|record| record.id == id)
            .ok_or_else(|| anyhow::anyhow!("match introuvable"))?;

        if record.status != MatchStatus::InProgress {
            bail!("ce match n'est plus en cours");
        }

        if let Some(scenario_id) = update.scenario_id {
            record.scenario_id = Some(scenario_id);
            record.scenario_other = None;
            record.scenario_url = None;
        }
        if let Some(scenario_other) = update.scenario_other {
            let trimmed = scenario_other.trim().to_string();
            if trimmed.is_empty() {
                record.scenario_other = None;
            } else {
                record.scenario_other = Some(trimmed);
                record.scenario_id = None;
            }
        }
        if let Some(scenario_name) = update.scenario_name {
            record.scenario_name = scenario_name;
        }
        if let Some(scenario_url) = update.scenario_url {
            let trimmed = scenario_url.trim().to_string();
            record.scenario_url = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            };
        }
        if update.clear_secondary_draws {
            let p1_len = record.player1_secondary_slugs.as_ref().map(|s| s.len());
            let p2_len = record.player2_secondary_slugs.as_ref().map(|s| s.len());
            // Ne pas effacer un draft Combat de l'Esprit déjà figé (souvent ≠ 3 cartes).
            let looks_like_initial_draw =
                matches!((p1_len, p2_len), (None, None) | (Some(3), Some(3)));
            if looks_like_initial_draw {
                record.player1_secondary_slugs = None;
                record.player2_secondary_slugs = None;
                record.player1_chosen_secondary = None;
                record.player2_chosen_secondary = None;
                record.secondary_pool_slugs = None;
            }
        }
        if let Some(slugs) = update.player1_secondary_slugs {
            if record.player1_secondary_slugs.is_some() {
                bail!("les secondaires du joueur 1 sont déjà figés");
            }
            record.player1_secondary_slugs = Some(slugs);
        }
        if let Some(slugs) = update.player2_secondary_slugs {
            if record.player2_secondary_slugs.is_some() {
                bail!("les secondaires du joueur 2 sont déjà figés");
            }
            record.player2_secondary_slugs = Some(slugs);
        }
        if let Some(slugs) = update.secondary_pool_slugs {
            if record.secondary_pool_slugs.is_some() {
                bail!("le deck de secondaires est déjà figé");
            }
            record.secondary_pool_slugs = Some(slugs);
        }
        if let Some(chosen) = update.player1_chosen_secondary {
            record.player1_chosen_secondary = chosen;
        }
        if let Some(chosen) = update.player2_chosen_secondary {
            record.player2_chosen_secondary = chosen;
        }
        if let Some(winner) = update.lieutenant_winner {
            record.lieutenant_winner = Some(winner);
        }
        if let Some(choice) = update.lieutenant_winner_choice {
            record.lieutenant_winner_choice = Some(choice);
        }
        if let Some(choice) = update.lieutenant_other_choice {
            record.lieutenant_other_choice = Some(choice);
        }
        if let Some(step) = update.partie_step {
            record.partie_step = Some(step);
        }

        Ok(record.clone())
    }

    pub fn complete_match(
        &mut self,
        id: u64,
        outcome: MatchOutcome,
        k_factor: f64,
        scores: MatchScores,
    ) -> Result<MatchRecord> {
        let scores = scores.validate()?;
        let index = self
            .matches
            .iter()
            .position(|record| record.id == id)
            .ok_or_else(|| anyhow::anyhow!("match introuvable"))?;

        if self.matches[index].status != MatchStatus::InProgress {
            bail!("ce match n'est pas en cours");
        }

        let key1 = normalize_name(&self.matches[index].player1.clone());
        let key2 = normalize_name(&self.matches[index].player2.clone());
        let counts_for_elo = self.matches[index].counts_for_elo;

        let update = {
            let old1 = self.players.get(&key1).unwrap().rating;
            let old2 = self.players.get(&key2).unwrap().rating;
            if counts_for_elo {
                let score1 = outcome.score_for_player1();
                let (new1, new2) = crate::elo::update_ratings(old1, old2, score1, k_factor);
                let score2 = match score1 {
                    crate::elo::MatchScore::Win => crate::elo::MatchScore::Loss,
                    crate::elo::MatchScore::Draw => crate::elo::MatchScore::Draw,
                    crate::elo::MatchScore::Loss => crate::elo::MatchScore::Win,
                };
                let p1 = self.players.get_mut(&key1).unwrap();
                p1.rating = new1;
                p1.record_match(score1);
                let p2 = self.players.get_mut(&key2).unwrap();
                p2.rating = new2;
                p2.record_match(score2);
                crate::player::RatingUpdate {
                    player1_old: old1,
                    player1_new: new1,
                    player2_old: old2,
                    player2_new: new2,
                }
            } else {
                crate::player::RatingUpdate {
                    player1_old: old1,
                    player1_new: old1,
                    player2_old: old2,
                    player2_new: old2,
                }
            }
        };

        let record = &mut self.matches[index];
        record.status = MatchStatus::Completed;
        record.outcome = Some(outcome);
        record.player1_old = update.player1_old;
        record.player1_new = update.player1_new;
        record.player2_old = update.player2_old;
        record.player2_new = update.player2_new;
        record.player1_objectives = scores.player1_objectives;
        record.player1_survivors = scores.player1_survivors;
        record.player2_objectives = scores.player2_objectives;
        record.player2_survivors = scores.player2_survivors;
        record.partie_step = Some("resultat".to_string());
        record.recorded_at = now_unix();

        Ok(record.clone())
    }

    pub fn in_progress_matches(&self) -> Vec<&MatchRecord> {
        self.matches
            .iter()
            .filter(|record| record.status == MatchStatus::InProgress)
            .collect()
    }

    pub fn in_progress_matches_for_player(&self, player_name: &str) -> Vec<&MatchRecord> {
        let key = normalize_name(player_name);
        self.matches
            .iter()
            .filter(|record| {
                record.status == MatchStatus::InProgress
                    && (normalize_name(&record.player1) == key
                        || normalize_name(&record.player2) == key)
            })
            .collect()
    }

    pub fn update_match_report(
        &mut self,
        id: u64,
        player_name: &str,
        body_md: &str,
    ) -> Result<MatchRecord> {
        let key = normalize_name(player_name);
        let now = now_unix();
        let next_report_id = self.next_report_id();
        let record = self
            .matches
            .iter_mut()
            .find(|record| record.id == id)
            .ok_or_else(|| anyhow::anyhow!("match introuvable"))?;

        let report = |existing: &Option<MatchReport>| MatchReport {
            id: existing.as_ref().map(|r| r.id).unwrap_or(next_report_id),
            body_md: body_md.to_string(),
            created_at: existing.as_ref().map(|r| r.created_at).unwrap_or(now),
            updated_at: now,
        };

        if normalize_name(&record.player1) == key {
            record.player1_report = Some(report(&record.player1_report));
        } else if normalize_name(&record.player2) == key {
            record.player2_report = Some(report(&record.player2_report));
        } else {
            bail!("ce joueur ne participe pas à ce match");
        }

        Ok(record.clone())
    }

    fn next_report_id(&self) -> i64 {
        self.matches
            .iter()
            .flat_map(|record| {
                [
                    record.player1_report.as_ref().map(|r| r.id),
                    record.player2_report.as_ref().map(|r| r.id),
                ]
            })
            .flatten()
            .max()
            .unwrap_or(0)
            + 1
    }

    pub fn update_match_army_list(
        &mut self,
        id: u64,
        player_name: &str,
        army_list_code: &str,
        army_id: Option<u32>,
    ) -> Result<MatchRecord> {
        let key = normalize_name(player_name);
        let code = normalize_army_list_code(army_list_code);
        let record = self
            .matches
            .iter_mut()
            .find(|record| record.id == id)
            .ok_or_else(|| anyhow::anyhow!("match introuvable"))?;

        if record.tournament_id.is_some() {
            bail!("les listes d'un match de tournoi sont figées à la validation du résultat");
        }

        if normalize_name(&record.player1) == key {
            record.player1_army_list_code = code;
            if let Some(army_id) = army_id {
                record.player1_army_id = Some(army_id);
            }
        } else if normalize_name(&record.player2) == key {
            record.player2_army_list_code = code;
            if let Some(army_id) = army_id {
                record.player2_army_id = Some(army_id);
            }
        } else {
            bail!("ce joueur ne participe pas à ce match");
        }

        Ok(record.clone())
    }

    pub fn recent_matches_page(&self, limit: usize, offset: usize) -> Vec<&MatchRecord> {
        self.matches
            .iter()
            .skip(offset)
            .take(limit)
            .collect()
    }

    pub fn player_matches(&self, name: &str, limit: usize) -> Result<Vec<&MatchRecord>> {
        let key = normalize_name(name);
        if !self.players.contains_key(&key) {
            bail!("joueur introuvable : {}", name);
        }

        Ok(self
            .matches
            .iter()
            .filter(|record| {
                normalize_name(&record.player1) == key || normalize_name(&record.player2) == key
            })
            .take(limit)
            .collect())
    }

    pub fn player_top_armies(&self, name: &str, limit: usize) -> Result<Vec<PlayerArmyUsage>> {
        let key = normalize_name(name);
        if !self.players.contains_key(&key) {
            bail!("joueur introuvable : {}", name);
        }

        let mut stats: HashMap<u32, (u32, u64)> = HashMap::new();

        for record in &self.matches {
            let army_id = if normalize_name(&record.player1) == key {
                record.player1_army_id
            } else if normalize_name(&record.player2) == key {
                record.player2_army_id
            } else {
                None
            };

            let Some(army_id) = army_id else {
                continue;
            };

            let entry = stats.entry(army_id).or_insert((0, 0));
            entry.0 += 1;
            entry.1 = entry.1.max(record.recorded_at);
        }

        let mut top: Vec<PlayerArmyUsage> = stats
            .into_iter()
            .map(|(army_id, (matches, last_played_at))| PlayerArmyUsage {
                army_id,
                matches,
                last_played_at,
            })
            .collect();

        top.sort_by(|left, right| {
            right
                .matches
                .cmp(&left.matches)
                .then_with(|| right.last_played_at.cmp(&left.last_played_at))
        });
        top.truncate(limit);

        Ok(top)
    }

    pub fn army_ranking(&self) -> Vec<ArmyMatchStats> {
        let mut stats: HashMap<u32, ArmyMatchStats> = HashMap::new();

        for record in &self.matches {
            if record.status != MatchStatus::Completed {
                continue;
            }
            let Some(outcome) = record.outcome else {
                continue;
            };
            if let Some(army_id) = record.player1_army_id {
                let entry = stats.entry(army_id).or_insert(ArmyMatchStats {
                    army_id,
                    wins: 0,
                    draws: 0,
                    losses: 0,
                });
                match outcome {
                    MatchOutcome::Player1Win => entry.wins += 1,
                    MatchOutcome::Player2Win => entry.losses += 1,
                    MatchOutcome::Draw => entry.draws += 1,
                }
            }

            if let Some(army_id) = record.player2_army_id {
                let entry = stats.entry(army_id).or_insert(ArmyMatchStats {
                    army_id,
                    wins: 0,
                    draws: 0,
                    losses: 0,
                });
                match outcome {
                    MatchOutcome::Player1Win => entry.losses += 1,
                    MatchOutcome::Player2Win => entry.wins += 1,
                    MatchOutcome::Draw => entry.draws += 1,
                }
            }
        }

        let mut ranking: Vec<ArmyMatchStats> = stats.into_values().collect();
        ranking.sort_by(|left, right| {
            right
                .win_rate()
                .partial_cmp(&left.win_rate())
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| right.total().cmp(&left.total()))
                .then_with(|| left.army_id.cmp(&right.army_id))
        });
        ranking
    }

    pub fn army_stats(&self, army_id: u32) -> Option<ArmyMatchStats> {
        self.army_ranking()
            .into_iter()
            .find(|stats| stats.army_id == army_id)
    }

    pub fn army_matches(&self, army_id: u32, limit: usize) -> Vec<&MatchRecord> {
        self.matches
            .iter()
            .filter(|record| {
                record.player1_army_id == Some(army_id)
                    || record.player2_army_id == Some(army_id)
            })
            .take(limit)
            .collect()
    }

    pub fn ranking(&self) -> Vec<&Player> {
        let mut players: Vec<&Player> = self.players.values().collect();
        players.sort_by(|a, b| {
            b.rating
                .partial_cmp(&a.rating)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.name.cmp(&b.name))
        });
        players
    }
}

fn outcome_to_str(outcome: MatchOutcome) -> &'static str {
    match outcome {
        MatchOutcome::Player1Win => "player1_win",
        MatchOutcome::Player2Win => "player2_win",
        MatchOutcome::Draw => "draw",
    }
}

fn outcome_option_to_str(outcome: Option<MatchOutcome>) -> String {
    outcome
        .map(outcome_to_str)
        .unwrap_or("")
        .to_string()
}

fn row_to_match(row: &rusqlite::Row<'_>) -> rusqlite::Result<MatchRecord> {
    let outcome_raw: Option<String> = row.get(3)?;
    let outcome = match outcome_raw.as_deref() {
        Some("player1_win") => Some(MatchOutcome::Player1Win),
        Some("player2_win") => Some(MatchOutcome::Player2Win),
        Some("draw") => Some(MatchOutcome::Draw),
        None | Some("") => None,
        Some(other) => {
            return Err(rusqlite::Error::InvalidColumnType(
                3,
                other.to_string(),
                rusqlite::types::Type::Text,
            ));
        }
    };

    let status_raw: Option<String> = row.get(20)?;
    let status = MatchStatus::parse(status_raw.as_deref().unwrap_or("completed"));

    Ok(MatchRecord {
        id: row.get(0)?,
        player1: row.get(1)?,
        player2: row.get(2)?,
        status,
        outcome,
        player1_old: row.get(4)?,
        player1_new: row.get(5)?,
        player2_old: row.get(6)?,
        player2_new: row.get(7)?,
        player1_objectives: row.get(8)?,
        player1_survivors: row.get(9)?,
        player2_objectives: row.get(10)?,
        player2_survivors: row.get(11)?,
        player1_army_id: row.get(12)?,
        player2_army_id: row.get(13)?,
        scenario_id: row.get(14)?,
        scenario_other: row.get(15)?,
        scenario_name: row.get(16)?,
        tournament_id: row.get(17)?,
        tournament_phase: row.get(18)?,
        tournament_name: row.get(19)?,
        player1_report: None,
        player2_report: None,
        player1_secondary_slugs: decode_slug_list(row.get(21)?),
        player2_secondary_slugs: decode_slug_list(row.get(22)?),
        player1_chosen_secondary: row.get(23)?,
        player2_chosen_secondary: row.get(24)?,
        lieutenant_winner: row.get(25)?,
        lieutenant_winner_choice: row.get(26)?,
        lieutenant_other_choice: row.get(27)?,
        partie_step: row.get(28)?,
        created_by: row.get(29)?,
        player1_army_list_code: row.get(30)?,
        player2_army_list_code: row.get(31)?,
        recorded_at: row.get(32)?,
        secondary_pool_slugs: decode_slug_list(row.get(33)?),
        counts_for_elo: row.get::<_, i64>(34)? != 0,
        scenario_url: row.get(35)?,
    })
}

fn attach_match_reports(conn: &Connection, matches: &mut [MatchRecord]) -> Result<()> {
    let mut stmt = conn.prepare(
        "
        SELECT id, match_id, player_name, body_md, created_at, updated_at
        FROM match_reports
        ",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, u64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, u64>(4)?,
            row.get::<_, u64>(5)?,
        ))
    })?;

    for row in rows {
        let (id, match_id, player_name, body_md, created_at, updated_at) = row?;
        let Some(record) = matches.iter_mut().find(|record| record.id == match_id) else {
            continue;
        };
        let report = MatchReport {
            id,
            body_md,
            created_at,
            updated_at,
        };
        if normalize_name(&record.player1) == normalize_name(&player_name) {
            record.player1_report = Some(report);
        } else if normalize_name(&record.player2) == normalize_name(&player_name) {
            record.player2_report = Some(report);
        }
    }

    Ok(())
}

fn normalize_army_list_code(raw: &str) -> Option<String> {
    crate::army_list::normalize_army_list_code(raw)
}

fn adjust_player_match_count(
    player: &mut Player,
    score: crate::elo::MatchScore,
    delta: i32,
) {
    match score {
        crate::elo::MatchScore::Win => {
            player.wins = (player.wins as i32 + delta).max(0) as u32;
        }
        crate::elo::MatchScore::Draw => {
            player.draws = (player.draws as i32 + delta).max(0) as u32;
        }
        crate::elo::MatchScore::Loss => {
            player.losses = (player.losses as i32 + delta).max(0) as u32;
        }
    }
}

pub fn normalize_name(name: &str) -> String {
    name.trim().to_lowercase()
}

#[derive(Debug, Clone)]
pub struct FixTournamentArmyReport {
    pub tournament_id: i64,
    pub player_name: String,
    pub army_id: u32,
    pub registration_updated: bool,
    pub tournament_matches_updated: u32,
    pub matches_updated: u32,
}

/// Corrige l'armée d'un joueur pour un tournoi (inscription + matchs tournoi + table matches).
///
/// Si `bracket_only` est vrai, ne touche pas à l'inscription ni aux matchs de poule
/// (uniquement `round_of_16` / `quarter` / `semi` / `final`).
pub fn fix_tournament_player_army(
    db_path: &Path,
    tournament_id: i64,
    player_name: &str,
    army_id: u32,
) -> Result<FixTournamentArmyReport> {
    fix_tournament_player_army_opts(db_path, tournament_id, player_name, army_id, false)
}

pub fn fix_tournament_player_army_opts(
    db_path: &Path,
    tournament_id: i64,
    player_name: &str,
    army_id: u32,
    bracket_only: bool,
) -> Result<FixTournamentArmyReport> {
    let player_name = player_name.trim();
    if player_name.is_empty() {
        bail!("indiquez un nom de joueur");
    }
    let key = normalize_name(player_name);

    let conn = Connection::open(db_path)
        .with_context(|| format!("impossible d'ouvrir {}", db_path.display()))?;
    migrate(&conn)?;

    let canonical_name: Option<String> = conn
        .query_row(
            "
            SELECT player_name FROM tournament_registrations
            WHERE tournament_id = ?1 AND player_name_key = ?2
            ",
            params![tournament_id, key],
            |row| row.get(0),
        )
        .ok();

    if canonical_name.is_none() {
        bail!(
            "joueur « {player_name} » introuvable dans le tournoi {tournament_id}"
        );
    }

    let name = canonical_name.as_deref().unwrap_or(player_name);

    let registration_updated = if bracket_only {
        false
    } else {
        conn.execute(
            "
            UPDATE tournament_registrations SET army_id = ?1
            WHERE tournament_id = ?2 AND player_name_key = ?3
            ",
            params![army_id, tournament_id, key],
        )? > 0
    };

    let tournament_matches_updated = {
        let phase_filter = if bracket_only {
            " AND phase != 'pool'"
        } else {
            ""
        };
        let p1 = conn.execute(
            &format!(
                "
            UPDATE tournament_matches SET player1_army_id = ?1
            WHERE tournament_id = ?2 AND player1 = ?3{phase_filter}
            "
            ),
            params![army_id, tournament_id, name],
        )? as u32;
        let p2 = conn.execute(
            &format!(
                "
            UPDATE tournament_matches SET player2_army_id = ?1
            WHERE tournament_id = ?2 AND player2 = ?3{phase_filter}
            "
            ),
            params![army_id, tournament_id, name],
        )? as u32;
        p1 + p2
    };

    let matches_updated = {
        let phase_filter = if bracket_only {
            " AND tournament_phase IS NOT NULL AND tournament_phase != 'pool'"
        } else {
            ""
        };
        let p1 = conn.execute(
            &format!(
                "
            UPDATE matches SET player1_army_id = ?1
            WHERE tournament_id = ?2 AND player1 = ?3{phase_filter}
            "
            ),
            params![army_id, tournament_id, name],
        )? as u32;
        let p2 = conn.execute(
            &format!(
                "
            UPDATE matches SET player2_army_id = ?1
            WHERE tournament_id = ?2 AND player2 = ?3{phase_filter}
            "
            ),
            params![army_id, tournament_id, name],
        )? as u32;
        p1 + p2
    };

    Ok(FixTournamentArmyReport {
        tournament_id,
        player_name: name.to_string(),
        army_id,
        registration_updated,
        tournament_matches_updated,
        matches_updated,
    })
}

#[derive(Debug, Clone)]
pub struct MergePlayersReport {
    pub keep_name: String,
    pub merged_aliases: Vec<String>,
    pub matches_rewritten: u32,
    pub rating: f64,
    pub wins: u32,
    pub draws: u32,
    pub losses: u32,
}

/// Fusionne des doublons de joueurs vers `keep_name`, conserve tous les matchs,
/// puis recalcule l'ELO chronologiquement.
pub fn merge_players(
    db_path: &Path,
    keep_name: &str,
    alias_names: &[&str],
    k_factor: f64,
) -> Result<MergePlayersReport> {
    let keep_name = keep_name.trim();
    if keep_name.is_empty() {
        bail!("indiquez le nom à conserver");
    }
    if alias_names.is_empty() {
        bail!("indiquez au moins un alias à fusionner");
    }

    let keep_key = normalize_name(keep_name);
    let mut alias_keys = Vec::new();
    for alias in alias_names {
        let key = normalize_name(alias);
        if key.is_empty() {
            bail!("alias vide");
        }
        if key == keep_key {
            bail!("un alias ne peut pas être identique au nom conservé");
        }
        if alias_keys.contains(&key) {
            continue;
        }
        alias_keys.push(key);
    }

    let conn = Connection::open(db_path)
        .with_context(|| format!("impossible d'ouvrir {}", db_path.display()))?;
    migrate(&conn)?;
    let tx = conn.unchecked_transaction()?;

    let mut players: HashMap<String, Player> = HashMap::new();
    {
        let mut stmt = tx.prepare(
            "SELECT name_key, name, rating, wins, draws, losses, discord_username FROM players",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                Player {
                    name: row.get(1)?,
                    rating: row.get(2)?,
                    wins: row.get(3)?,
                    draws: row.get(4)?,
                    losses: row.get(5)?,
                    discord_username: row.get(6)?,
                },
            ))
        })?;
        for row in rows {
            let (key, player) = row?;
            players.insert(key, player);
        }
    }

    for key in &alias_keys {
        if !players.contains_key(key) {
            bail!("joueur alias introuvable : {key}");
        }
    }

    let mut discord = players
        .get(&keep_key)
        .and_then(|p| p.discord_username.clone());
    if discord.is_none() {
        for key in &alias_keys {
            if let Some(value) = players.get(key).and_then(|p| p.discord_username.clone()) {
                discord = Some(value);
                break;
            }
        }
    }

    // Libérer le pseudo Discord des aliases avant de l'attribuer au joueur conservé
    // (contrainte UNIQUE sur players.discord_username).
    for key in &alias_keys {
        tx.execute(
            "UPDATE players SET discord_username = NULL WHERE name_key = ?1",
            params![key],
        )?;
    }

    if !players.contains_key(&keep_key) {
        tx.execute(
            "
            INSERT INTO players (name_key, name, rating, wins, draws, losses, discord_username)
            VALUES (?1, ?2, ?3, 0, 0, 0, ?4)
            ",
            params![
                keep_key,
                keep_name,
                crate::player::DEFAULT_RATING,
                discord,
            ],
        )?;
    } else {
        tx.execute(
            "
            UPDATE players
            SET name = ?1, discord_username = COALESCE(?2, discord_username)
            WHERE name_key = ?3
            ",
            params![keep_name, discord, keep_key],
        )?;
    }

    let mut name_variants: HashMap<String, Vec<String>> = HashMap::new();
    let mut collect_name = |name: String| {
        let key = normalize_name(&name);
        name_variants.entry(key).or_default().push(name);
    };
    for player in players.values() {
        collect_name(player.name.clone());
    }
    {
        let mut stmt = tx.prepare(
            "
            SELECT DISTINCT player1 FROM matches
            UNION SELECT DISTINCT player2 FROM matches
            UNION SELECT DISTINCT player1 FROM tournament_matches WHERE player1 IS NOT NULL
            UNION SELECT DISTINCT player2 FROM tournament_matches WHERE player2 IS NOT NULL
            UNION SELECT DISTINCT forfeit_player FROM tournament_matches WHERE forfeit_player IS NOT NULL
            UNION SELECT DISTINCT player_name FROM tournament_registrations
            UNION SELECT DISTINCT player_name FROM tournament_players
            UNION SELECT DISTINCT player_name FROM pool_players
            ",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        for row in rows {
            collect_name(row?);
        }
    }

    let mut matches_rewritten = 0u32;
    for alias_key in &alias_keys {
        let variants = name_variants
            .get(alias_key)
            .cloned()
            .unwrap_or_else(|| vec![alias_key.clone()]);
        for variant in variants {
            matches_rewritten += tx.execute(
                "UPDATE matches SET player1 = ?1 WHERE player1 = ?2",
                params![keep_name, variant],
            )? as u32;
            matches_rewritten += tx.execute(
                "UPDATE matches SET player2 = ?1 WHERE player2 = ?2",
                params![keep_name, variant],
            )? as u32;
            tx.execute(
                "UPDATE tournament_matches SET player1 = ?1 WHERE player1 = ?2",
                params![keep_name, variant],
            )?;
            tx.execute(
                "UPDATE tournament_matches SET player2 = ?1 WHERE player2 = ?2",
                params![keep_name, variant],
            )?;
            tx.execute(
                "UPDATE tournament_matches SET forfeit_player = ?1 WHERE forfeit_player = ?2",
                params![keep_name, variant],
            )?;
        }

        rewrite_keyed_player_rows(&tx, "tournament_registrations", alias_key, keep_name, &keep_key)?;
        rewrite_keyed_player_rows(&tx, "tournament_players", alias_key, keep_name, &keep_key)?;
        rewrite_keyed_player_rows(&tx, "pool_players", alias_key, keep_name, &keep_key)?;

        tx.execute("DELETE FROM players WHERE name_key = ?1", params![alias_key])?;
    }

    tx.commit()?;

    let stats = recompute_elo_from_matches(db_path, k_factor)?;
    let keep_stats = stats
        .get(&keep_key)
        .cloned()
        .with_context(|| format!("joueur fusionné introuvable après recalcul : {keep_name}"))?;

    Ok(MergePlayersReport {
        keep_name: keep_name.to_string(),
        merged_aliases: alias_names
            .iter()
            .map(|name| name.trim().to_string())
            .collect(),
        matches_rewritten,
        rating: keep_stats.rating,
        wins: keep_stats.wins,
        draws: keep_stats.draws,
        losses: keep_stats.losses,
    })
}

fn rewrite_keyed_player_rows(
    conn: &Connection,
    table: &str,
    alias_key: &str,
    keep_name: &str,
    keep_key: &str,
) -> Result<()> {
    let scope_column = match table {
        "tournament_registrations" | "tournament_players" => "tournament_id",
        "pool_players" => "pool_id",
        _ => bail!("table non supportée pour fusion : {table}"),
    };

    let sql = format!(
        "SELECT DISTINCT {scope_column} FROM {table} WHERE player_name_key = ?1"
    );
    let scopes: Vec<i64> = {
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![alias_key], |row| row.get(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    for scope_id in scopes {
        let keep_exists: bool = conn.query_row(
            &format!(
                "SELECT EXISTS(SELECT 1 FROM {table} WHERE {scope_column} = ?1 AND player_name_key = ?2)"
            ),
            params![scope_id, keep_key],
            |row| row.get(0),
        )?;

        if keep_exists {
            // Conflit : conserver la ligne la plus « active » pour les snapshots tournoi.
            if table == "tournament_players" {
                let alias_activity: (i64, i64, i64) = conn.query_row(
                    "
                    SELECT COALESCE(pool_points, 0), COALESCE(pool_objectives, 0),
                           COALESCE(pool_survivors, 0)
                    FROM tournament_players
                    WHERE tournament_id = ?1 AND player_name_key = ?2
                    ",
                    params![scope_id, alias_key],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )?;
                let keep_activity: (i64, i64, i64) = conn.query_row(
                    "
                    SELECT COALESCE(pool_points, 0), COALESCE(pool_objectives, 0),
                           COALESCE(pool_survivors, 0)
                    FROM tournament_players
                    WHERE tournament_id = ?1 AND player_name_key = ?2
                    ",
                    params![scope_id, keep_key],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )?;
                if alias_activity > keep_activity {
                    conn.execute(
                        "DELETE FROM tournament_players WHERE tournament_id = ?1 AND player_name_key = ?2",
                        params![scope_id, keep_key],
                    )?;
                    conn.execute(
                        "
                        UPDATE tournament_players
                        SET player_name = ?1, player_name_key = ?2
                        WHERE tournament_id = ?3 AND player_name_key = ?4
                        ",
                        params![keep_name, keep_key, scope_id, alias_key],
                    )?;
                } else {
                    conn.execute(
                        "DELETE FROM tournament_players WHERE tournament_id = ?1 AND player_name_key = ?2",
                        params![scope_id, alias_key],
                    )?;
                }
            } else if table == "pool_players" {
                // Deux entrées dans la même poule = doublon : supprimer l'alias.
                conn.execute(
                    "DELETE FROM pool_players WHERE pool_id = ?1 AND player_name_key = ?2",
                    params![scope_id, alias_key],
                )?;
            } else {
                // registrations : supprimer l'alias en double
                conn.execute(
                    "DELETE FROM tournament_registrations WHERE tournament_id = ?1 AND player_name_key = ?2",
                    params![scope_id, alias_key],
                )?;
            }
        } else {
            conn.execute(
                &format!(
                    "
                    UPDATE {table}
                    SET player_name = ?1, player_name_key = ?2
                    WHERE {scope_column} = ?3 AND player_name_key = ?4
                    "
                ),
                params![keep_name, keep_key, scope_id, alias_key],
            )?;
        }
    }

    Ok(())
}

/// Recalcule rating / W-D-L de tous les joueurs à partir des outcomes, chronologiquement.
pub fn recompute_elo_from_matches(
    db_path: &Path,
    k_factor: f64,
) -> Result<HashMap<String, Player>> {
    let conn = Connection::open(db_path)
        .with_context(|| format!("impossible d'ouvrir {}", db_path.display()))?;
    migrate(&conn)?;

    let mut players: HashMap<String, Player> = HashMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT name_key, name, discord_username FROM players",
        )?;
        let rows = stmt.query_map([], |row| {
            let key: String = row.get(0)?;
            let mut player = Player::new(row.get::<_, String>(1)?);
            player.discord_username = row.get(2)?;
            Ok((key, player))
        })?;
        for row in rows {
            let (key, player) = row?;
            players.insert(key, player);
        }
    }

    let match_rows: Vec<(i64, String, String, Option<String>)> = {
        let mut stmt = conn.prepare(
            "
            SELECT id, player1, player2, outcome
            FROM matches
            WHERE outcome IS NOT NULL
              AND (status IS NULL OR status = 'completed')
              AND counts_for_elo = 1
            ORDER BY recorded_at ASC, id ASC
            ",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let tx = conn.unchecked_transaction()?;
    for (match_id, player1, player2, outcome_raw) in match_rows {
        let Some(outcome_raw) = outcome_raw else {
            continue;
        };
        let outcome = match outcome_raw.as_str() {
            "player1_win" => MatchOutcome::Player1Win,
            "player2_win" => MatchOutcome::Player2Win,
            _ => MatchOutcome::Draw,
        };
        let key1 = normalize_name(&player1);
        let key2 = normalize_name(&player2);

        if !players.contains_key(&key1) {
            players.insert(key1.clone(), Player::new(player1.clone()));
        }
        if !players.contains_key(&key2) {
            players.insert(key2.clone(), Player::new(player2.clone()));
        }

        let old1 = players.get(&key1).unwrap().rating;
        let old2 = players.get(&key2).unwrap().rating;
        let score1 = outcome.score_for_player1();
        let (new1, new2) = crate::elo::update_ratings(old1, old2, score1, k_factor);
        let score2 = match score1 {
            crate::elo::MatchScore::Win => crate::elo::MatchScore::Loss,
            crate::elo::MatchScore::Draw => crate::elo::MatchScore::Draw,
            crate::elo::MatchScore::Loss => crate::elo::MatchScore::Win,
        };

        {
            let p1 = players.get_mut(&key1).unwrap();
            p1.name = player1.clone();
            p1.rating = new1;
            p1.record_match(score1);
        }
        {
            let p2 = players.get_mut(&key2).unwrap();
            p2.name = player2.clone();
            p2.rating = new2;
            p2.record_match(score2);
        }

        tx.execute(
            "
            UPDATE matches
            SET player1_old = ?1, player1_new = ?2,
                player2_old = ?3, player2_new = ?4
            WHERE id = ?5
            ",
            params![old1, new1, old2, new2, match_id],
        )?;
    }

    for (key, player) in &players {
        tx.execute(
            "
            INSERT INTO players (name_key, name, rating, wins, draws, losses, discord_username)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(name_key) DO UPDATE SET
                name = excluded.name,
                rating = excluded.rating,
                wins = excluded.wins,
                draws = excluded.draws,
                losses = excluded.losses,
                discord_username = COALESCE(players.discord_username, excluded.discord_username)
            ",
            params![
                key,
                player.name,
                player.rating,
                player.wins,
                player.draws,
                player.losses,
                player.discord_username,
            ],
        )?;
    }

    tx.commit()?;
    Ok(players)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db_path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "poissonnerie-leaderboard-test-{}",
            std::process::id()
        ))
    }

    #[test]
    fn names_are_case_insensitive() {
        let mut board = Leaderboard::default();
        board.add_player("Alice").unwrap();
        assert!(board.add_player("alice").is_err());
        assert!(board.get_player("ALICE").is_ok());
    }

    #[test]
    fn player_matches_returns_only_player_games() {
        use crate::player::MatchOutcome;

        let mut board = Leaderboard::default();
        board.add_player("Alice").unwrap();
        board.add_player("Bob").unwrap();
        board.add_player("Charlie").unwrap();
        board.record_match("Alice", "Bob", MatchOutcome::Player1Win, 32.0, MatchScores::default(), None, None, None, None, None)
            .unwrap();
        board
            .record_match("Bob", "Charlie", MatchOutcome::Player2Win, 32.0, MatchScores::default(), None, None, None, None, None)
            .unwrap();

        let alice_matches = board.player_matches("Alice", 10).unwrap();
        assert_eq!(alice_matches.len(), 1);
        assert_eq!(alice_matches[0].player1, "Alice");

        let bob_matches = board.player_matches("Bob", 10).unwrap();
        assert_eq!(bob_matches.len(), 2);
    }

    #[test]
    fn player_top_armies_returns_most_played_with_tiebreak_on_recency() {
        use crate::player::MatchOutcome;

        let mut board = Leaderboard::default();
        board.add_player("Alice").unwrap();
        board.add_player("Bob").unwrap();

        board
            .record_match(
                "Alice",
                "Bob",
                MatchOutcome::Player1Win,
                32.0,
                MatchScores::default(),
                Some(101),
                Some(201),
                None,
                None,
                None,
            )
            .unwrap();
        board
            .record_match(
                "Alice",
                "Bob",
                MatchOutcome::Player2Win,
                32.0,
                MatchScores::default(),
                Some(101),
                Some(201),
                None,
                None,
                None,
            )
            .unwrap();
        board
            .record_match(
                "Alice",
                "Bob",
                MatchOutcome::Draw,
                32.0,
                MatchScores::default(),
                Some(102),
                Some(201),
                None,
                None,
                None,
            )
            .unwrap();

        let top = board.player_top_armies("Alice", 3).unwrap();
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].army_id, 101);
        assert_eq!(top[0].matches, 2);
        assert_eq!(top[1].army_id, 102);
        assert_eq!(top[1].matches, 1);
    }

    #[test]
    fn army_ranking_aggregates_outcomes_per_army() {
        use crate::player::MatchOutcome;

        let mut board = Leaderboard::default();
        board.add_player("Alice").unwrap();
        board.add_player("Bob").unwrap();

        board
            .record_match(
                "Alice",
                "Bob",
                MatchOutcome::Player1Win,
                32.0,
                MatchScores::default(),
                Some(101),
                Some(201),
                None,
                None,
                None,
            )
            .unwrap();
        board
            .record_match(
                "Alice",
                "Bob",
                MatchOutcome::Draw,
                32.0,
                MatchScores::default(),
                Some(101),
                Some(201),
                None,
                None,
                None,
            )
            .unwrap();
        board
            .record_match(
                "Alice",
                "Bob",
                MatchOutcome::Player2Win,
                32.0,
                MatchScores::default(),
                Some(101),
                Some(202),
                None,
                None,
                None,
            )
            .unwrap();

        let ranking = board.army_ranking();
        let army101 = ranking.iter().find(|entry| entry.army_id == 101).unwrap();
        assert_eq!(army101.wins, 1);
        assert_eq!(army101.draws, 1);
        assert_eq!(army101.losses, 1);
        assert!((army101.win_rate() - 50.0).abs() < 0.001);

        let army201 = ranking.iter().find(|entry| entry.army_id == 201).unwrap();
        assert_eq!(army201.wins, 0);
        assert_eq!(army201.draws, 1);
        assert_eq!(army201.losses, 1);
    }

    #[test]
    fn army_matches_returns_only_games_with_army() {
        use crate::player::MatchOutcome;

        let mut board = Leaderboard::default();
        board.add_player("Alice").unwrap();
        board.add_player("Bob").unwrap();

        board
            .record_match(
                "Alice",
                "Bob",
                MatchOutcome::Player1Win,
                32.0,
                MatchScores::default(),
                Some(101),
                Some(201),
                None,
                None,
                None,
            )
            .unwrap();
        board
            .record_match(
                "Alice",
                "Bob",
                MatchOutcome::Player2Win,
                32.0,
                MatchScores::default(),
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();

        let matches = board.army_matches(101, 10);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].player1_army_id, Some(101));
    }

    #[test]
    fn sqlite_roundtrip() {
        let path = temp_db_path();
        let _ = fs::remove_file(&path);

        let mut board = Leaderboard::default();
        board.add_player("Alice").unwrap();
        board.add_player("Bob").unwrap();
        board
            .record_match(
                "Alice",
                "Bob",
                MatchOutcome::Player1Win,
                32.0,
                MatchScores::default(),
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
        board.save(&path).unwrap();

        let loaded = Leaderboard::load(&path).unwrap();
        assert_eq!(loaded.players.len(), 2);
        assert_eq!(loaded.matches.len(), 1);
        assert_eq!(loaded.get_player("Alice").unwrap().wins, 1);
        assert_eq!(loaded.get_player("Bob").unwrap().losses, 1);

        let _ = fs::remove_file(path);
    }
}

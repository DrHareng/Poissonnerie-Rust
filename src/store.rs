use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::match_record::{now_unix, MatchRecord, MatchScores, MAX_MATCH_HISTORY};
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

impl ArmyMatchStats {
    pub fn total(&self) -> u32 {
        self.wins + self.draws + self.losses
    }

    pub fn win_rate(&self) -> f64 {
        let total = self.total();
        if total == 0 {
            0.0
        } else {
            (self.wins as f64 / total as f64) * 100.0
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
                SELECT id, player1, player2, outcome,
                       player1_old, player1_new, player2_old, player2_new,
                       player1_objectives, player1_survivors,
                       player2_objectives, player2_survivors,
                       player1_army_id, player2_army_id,
                       scenario_id, scenario_name, recorded_at
                FROM matches
                ORDER BY recorded_at DESC
                ",
            )?;
            let rows = stmt.query_map([], row_to_match)?;
            for row in rows {
                matches.push(row?);
            }
        }

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
                    scenario_id, scenario_name, recorded_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
                ",
                params![
                    record.id,
                    record.player1,
                    record.player2,
                    outcome_to_str(record.outcome),
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
                    record.scenario_name,
                    record.recorded_at,
                ],
            )?;
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
            scenario_name,
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
        scenario_name: Option<String>,
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
            scenario_name,
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
        scenario_name: Option<String>,
    ) -> Result<MatchRecord> {
        let next_id = self
            .matches
            .iter()
            .map(|record| record.id)
            .max()
            .unwrap_or(0)
            + 1;

        let record = MatchRecord::from_update(
            next_id,
            self.players.get(key1).unwrap().name.clone(),
            self.players.get(key2).unwrap().name.clone(),
            outcome,
            update,
            scores,
            player1_army_id,
            player2_army_id,
            scenario_id,
            scenario_name,
            now_unix(),
        );

        self.matches.insert(0, record.clone());
        if self.matches.len() > MAX_MATCH_HISTORY {
            self.matches.truncate(MAX_MATCH_HISTORY);
        }

        Ok(record)
    }

    pub fn recent_matches(&self, limit: usize) -> Vec<&MatchRecord> {
        self.matches.iter().take(limit).collect()
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
            if let Some(army_id) = record.player1_army_id {
                let entry = stats.entry(army_id).or_insert(ArmyMatchStats {
                    army_id,
                    wins: 0,
                    draws: 0,
                    losses: 0,
                });
                match record.outcome {
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
                match record.outcome {
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

fn row_to_match(row: &rusqlite::Row<'_>) -> rusqlite::Result<MatchRecord> {
    let outcome_str: String = row.get(3)?;
    let outcome = match outcome_str.as_str() {
        "player1_win" => MatchOutcome::Player1Win,
        "player2_win" => MatchOutcome::Player2Win,
        "draw" => MatchOutcome::Draw,
        _ => {
            return Err(rusqlite::Error::InvalidColumnType(
                3,
                outcome_str,
                rusqlite::types::Type::Text,
            ));
        }
    };

    Ok(MatchRecord {
        id: row.get(0)?,
        player1: row.get(1)?,
        player2: row.get(2)?,
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
        scenario_name: row.get(15)?,
        recorded_at: row.get(16)?,
    })
}

pub fn normalize_name(name: &str) -> String {
    name.trim().to_lowercase()
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
        board.record_match("Alice", "Bob", MatchOutcome::Player1Win, 32.0, MatchScores::default(), None, None, None, None)
            .unwrap();
        board
            .record_match("Bob", "Charlie", MatchOutcome::Player2Win, 32.0, MatchScores::default(), None, None, None, None)
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
            )
            .unwrap();

        let ranking = board.army_ranking();
        let army101 = ranking.iter().find(|entry| entry.army_id == 101).unwrap();
        assert_eq!(army101.wins, 1);
        assert_eq!(army101.draws, 1);
        assert_eq!(army101.losses, 1);
        assert!((army101.win_rate() - (100.0 / 3.0)).abs() < 0.001);

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

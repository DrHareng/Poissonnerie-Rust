use serde::{Deserialize, Serialize};

use crate::player::MatchOutcome;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct MatchScores {
    #[serde(default)]
    pub player1_objectives: u8,
    #[serde(default)]
    pub player1_survivors: u16,
    #[serde(default)]
    pub player2_objectives: u8,
    #[serde(default)]
    pub player2_survivors: u16,
}

impl MatchScores {
    pub fn validate(self) -> anyhow::Result<Self> {
        if self.player1_objectives > 10 || self.player2_objectives > 10 {
            anyhow::bail!("les points d'objectifs doivent être entre 0 et 10");
        }
        if self.player1_survivors > 300 || self.player2_survivors > 300 {
            anyhow::bail!("les points de survivants doivent être entre 0 et 300");
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MatchRecord {
    pub id: u64,
    pub player1: String,
    pub player2: String,
    pub outcome: MatchOutcome,
    pub player1_old: f64,
    pub player1_new: f64,
    pub player2_old: f64,
    pub player2_new: f64,
    #[serde(default)]
    pub player1_objectives: u8,
    #[serde(default)]
    pub player1_survivors: u16,
    #[serde(default)]
    pub player2_objectives: u8,
    #[serde(default)]
    pub player2_survivors: u16,
    #[serde(default)]
    pub player1_army_id: Option<u32>,
    #[serde(default)]
    pub player2_army_id: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario_name: Option<String>,
    pub recorded_at: u64,
}

impl MatchRecord {
    pub fn from_update(
        id: u64,
        player1: String,
        player2: String,
        outcome: MatchOutcome,
        update: crate::player::RatingUpdate,
        scores: MatchScores,
        player1_army_id: Option<u32>,
        player2_army_id: Option<u32>,
        scenario_id: Option<i64>,
        scenario_name: Option<String>,
        recorded_at: u64,
    ) -> Self {
        Self {
            id,
            player1,
            player2,
            outcome,
            player1_old: update.player1_old,
            player1_new: update.player1_new,
            player2_old: update.player2_old,
            player2_new: update.player2_new,
            player1_objectives: scores.player1_objectives,
            player1_survivors: scores.player1_survivors,
            player2_objectives: scores.player2_objectives,
            player2_survivors: scores.player2_survivors,
            player1_army_id,
            player2_army_id,
            scenario_id,
            scenario_name,
            recorded_at,
        }
    }
}

pub const MAX_MATCH_HISTORY: usize = 100;

pub fn now_unix() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_score_ranges() {
        assert!(MatchScores {
            player1_objectives: 10,
            player1_survivors: 300,
            player2_objectives: 0,
            player2_survivors: 0,
        }
        .validate()
        .is_ok());

        assert!(MatchScores {
            player1_objectives: 11,
            ..Default::default()
        }
        .validate()
        .is_err());
    }
}

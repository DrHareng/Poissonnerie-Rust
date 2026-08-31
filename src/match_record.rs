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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum MatchStatus {
    InProgress,
    #[default]
    Completed,
}

impl MatchStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            MatchStatus::InProgress => "in_progress",
            MatchStatus::Completed => "completed",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "in_progress" => MatchStatus::InProgress,
            _ => MatchStatus::Completed,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReportStatus {
    Draft,
    #[default]
    Published,
}

impl ReportStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Published => "published",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "draft" => Self::Draft,
            _ => Self::Published,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MatchReport {
    pub id: i64,
    pub body_md: String,
    #[serde(default)]
    pub status: ReportStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_at: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecentMatchReport {
    pub match_id: u64,
    pub report_id: i64,
    pub author_name: String,
    pub author_slot: &'static str,
    pub opponent_name: String,
    pub author_army_id: Option<u32>,
    pub opponent_army_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scenario_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tournament_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tournament_phase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tournament_name: Option<String>,
    pub counts_for_elo: bool,
    pub excerpt: String,
    pub published_at: u64,
    pub updated_at: u64,
}

pub fn report_excerpt(body: &str, max_chars: usize) -> String {
    let collapsed = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= max_chars {
        return collapsed;
    }
    let take = max_chars.saturating_sub(1);
    let truncated: String = collapsed.chars().take(take).collect();
    format!("{truncated}…")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MatchRecord {
    pub id: u64,
    pub player1: String,
    pub player2: String,
    #[serde(default)]
    pub status: MatchStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<MatchOutcome>,
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
    /// Texte libre si le scénario n'est pas choisi dans le catalogue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario_other: Option<String>,
    /// URL facultative pour un scénario saisi librement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario_url: Option<String>,
    /// Libellé d'affichage (nom catalogue ou texte libre).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tournament_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tournament_phase: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tournament_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player1_report: Option<MatchReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player2_report: Option<MatchReport>,
    /// Code de liste Infinity Army (segment d'URL après `/army/list/`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player1_army_list_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player2_army_list_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player1_army_list_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player2_army_list_id: Option<i64>,
    /// Slugs des 3 secondaires tirés (JSON array).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player1_secondary_slugs: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player2_secondary_slugs: Option<Vec<String>>,
    /// Deck figé Combat de l'Esprit (8 emplacements, ordre stable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secondary_pool_slugs: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player1_chosen_secondary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player2_chosen_secondary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lieutenant_winner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lieutenant_winner_choice: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lieutenant_other_choice: Option<String>,
    /// Étape courante du wizard (`joueurs`, `scenario`, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partie_step: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    /// Si faux : match amical (pas d'impact ELO / W-D-L).
    #[serde(default = "default_counts_for_elo")]
    pub counts_for_elo: bool,
    pub recorded_at: u64,
}

fn default_counts_for_elo() -> bool {
    true
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
        scenario_other: Option<String>,
        scenario_name: Option<String>,
        tournament_id: Option<i64>,
        tournament_phase: Option<String>,
        tournament_name: Option<String>,
        recorded_at: u64,
    ) -> Self {
        Self {
            id,
            player1,
            player2,
            status: MatchStatus::Completed,
            outcome: Some(outcome),
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
            scenario_other,
            scenario_url: None,
            scenario_name,
            tournament_id,
            tournament_phase,
            tournament_name,
            player1_report: None,
            player2_report: None,
            player1_army_list_code: None,
            player2_army_list_code: None,
            player1_army_list_id: None,
            player2_army_list_id: None,
            player1_secondary_slugs: None,
            player2_secondary_slugs: None,
            secondary_pool_slugs: None,
            player1_chosen_secondary: None,
            player2_chosen_secondary: None,
            lieutenant_winner: None,
            lieutenant_winner_choice: None,
            lieutenant_other_choice: None,
            partie_step: None,
            created_by: None,
            counts_for_elo: true,
            recorded_at,
        }
    }
}

pub fn now_unix() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

pub fn encode_slug_list(slugs: Option<&[String]>) -> Option<String> {
    slugs.map(|items| serde_json::to_string(items).unwrap_or_else(|_| "[]".to_string()))
}

pub fn decode_slug_list(raw: Option<String>) -> Option<Vec<String>> {
    let Some(raw) = raw.filter(|value| !value.trim().is_empty()) else {
        return None;
    };
    serde_json::from_str(&raw).ok()
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

    #[test]
    fn report_excerpt_collapses_and_truncates() {
        assert_eq!(report_excerpt("  hello\n\nworld  ", 80), "hello world");
        let long = "a".repeat(40);
        let excerpt = report_excerpt(&long, 10);
        assert_eq!(excerpt.chars().count(), 10);
        assert!(excerpt.ends_with('…'));
    }
}

use serde::{Deserialize, Serialize};

use crate::elo;
use crate::player::MatchOutcome;

pub const MAX_POOL_SIZE: usize = 6;
pub const POOLS_FOUR_CAPACITY: usize = 24;
pub const POOLS_EIGHT_CAPACITY: usize = 48;
pub const WAITLIST_THRESHOLD: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TournamentStatus {
    Draft,
    RegistrationOpen,
    RegistrationClosed,
    Started,
    Completed,
}

impl TournamentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::RegistrationOpen => "registration_open",
            Self::RegistrationClosed => "registration_closed",
            Self::Started => "started",
            Self::Completed => "completed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "draft" => Some(Self::Draft),
            "registration_open" => Some(Self::RegistrationOpen),
            "registration_closed" => Some(Self::RegistrationClosed),
            "started" => Some(Self::Started),
            "completed" => Some(Self::Completed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegistrationStatus {
    Pending,
    Approved,
    Waitlisted,
    Rejected,
}

impl RegistrationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Waitlisted => "waitlisted",
            Self::Rejected => "rejected",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "approved" => Some(Self::Approved),
            "waitlisted" => Some(Self::Waitlisted),
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BracketFormat {
    QuartersDirect,
    RoundOf16,
    RoundOf16Full,
}

impl BracketFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::QuartersDirect => "quarters_direct",
            Self::RoundOf16 => "round_of_16",
            Self::RoundOf16Full => "round_of_16_full",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "quarters_direct" => Some(Self::QuartersDirect),
            "round_of_16" => Some(Self::RoundOf16),
            "round_of_16_full" => Some(Self::RoundOf16Full),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TournamentPhase {
    Pool,
    RoundOf16,
    Quarter,
    Semi,
    Final,
}

impl TournamentPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pool => "pool",
            Self::RoundOf16 => "round_of_16",
            Self::Quarter => "quarter",
            Self::Semi => "semi",
            Self::Final => "final",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pool" => Some(Self::Pool),
            "round_of_16" => Some(Self::RoundOf16),
            "quarter" => Some(Self::Quarter),
            "semi" => Some(Self::Semi),
            "final" => Some(Self::Final),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TournamentMatchStatus {
    Scheduled,
    Submitted,
    Confirmed,
}

impl TournamentMatchStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Scheduled => "scheduled",
            Self::Submitted => "submitted",
            Self::Confirmed => "confirmed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "scheduled" => Some(Self::Scheduled),
            "submitted" => Some(Self::Submitted),
            "confirmed" => Some(Self::Confirmed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tournament {
    pub id: i64,
    pub name: String,
    pub status: TournamentStatus,
    pub pool_count: u8,
    pub bracket_format: BracketFormat,
    pub created_at: u64,
    pub started_at: Option<u64>,
    pub pools_finalized_at: Option<u64>,
    pub completed_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TournamentRegistration {
    pub id: i64,
    pub tournament_id: i64,
    pub player_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_display_name: Option<String>,
    pub user_id: Option<i64>,
    pub status: RegistrationStatus,
    pub waitlist_position: Option<u32>,
    pub requested_at: u64,
    pub reviewed_at: Option<u64>,
    pub reviewed_by: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub army_id: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TournamentPlayerSnapshot {
    pub player_name: String,
    pub start_rating: f64,
    pub pool_elo_delta: f64,
    pub bracket_rating: f64,
    pub pool_points: u32,
    pub pool_objectives: u32,
    pub pool_survivors: u32,
    pub final_placement: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Pool {
    pub id: i64,
    pub tournament_id: i64,
    pub name: String,
    pub position: u8,
    pub players: Vec<PoolPlayer>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PoolPlayer {
    pub player_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub army_id: Option<u32>,
    pub seed: u8,
    pub points: u32,
    pub objectives: u32,
    pub survivors: u32,
    pub wins: u32,
    pub draws: u32,
    pub losses: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TournamentMatch {
    pub id: i64,
    pub tournament_id: i64,
    pub phase: TournamentPhase,
    pub pool_id: Option<i64>,
    pub bracket_slot: Option<u32>,
    pub player1: Option<String>,
    pub player2: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player1_display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player2_display_name: Option<String>,
    pub player1_objectives: u8,
    pub player2_objectives: u8,
    pub player1_survivors: u16,
    pub player2_survivors: u16,
    pub player1_tournament_points: u8,
    pub player2_tournament_points: u8,
    pub outcome: Option<MatchOutcome>,
    pub is_forfeit: bool,
    #[serde(default)]
    pub is_unplayed: bool,
    pub forfeit_player: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forfeit_player_display_name: Option<String>,
    pub player1_elo_delta: f64,
    pub player2_elo_delta: f64,
    pub player1_rating_used: Option<f64>,
    pub player2_rating_used: Option<f64>,
    pub elo_applied_at: Option<u64>,
    pub status: TournamentMatchStatus,
    pub submitted_by_user_id: Option<i64>,
    pub submitted_at: Option<u64>,
    pub confirmed_by_user_id: Option<i64>,
    pub confirmed_at: Option<u64>,
    pub scenario_id: Option<i64>,
    pub scenario_name: Option<String>,
    pub player1_army_id: Option<u32>,
    pub player2_army_id: Option<u32>,
    pub played_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlayerTournamentResult {
    pub tournament_id: i64,
    pub tournament_name: String,
    pub placement_label: String,
    pub final_placement: Option<u32>,
    pub completed_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TournamentTopFourEntry {
    pub rank: u32,
    pub player_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TournamentListEntry {
    #[serde(flatten)]
    pub tournament: Tournament,
    pub approved_count: u32,
    pub waitlist_count: u32,
    pub display_status: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub top_four: Vec<TournamentTopFourEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TournamentDetail {
    #[serde(flatten)]
    pub tournament: Tournament,
    pub registrations: Vec<TournamentRegistration>,
    pub players: Vec<TournamentPlayerSnapshot>,
    pub pools: Vec<Pool>,
    pub matches: Vec<TournamentMatch>,
    pub approved_count: u32,
    pub waitlist_count: u32,
    pub display_status: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub top_four: Vec<TournamentTopFourEntry>,
}

pub fn tournament_points(
    outcome: MatchOutcome,
    my_objectives: u8,
    opp_objectives: u8,
    is_forfeit_winner: bool,
) -> u8 {
    if is_forfeit_winner {
        return 5;
    }

    let base = match outcome {
        MatchOutcome::Player1Win => 4,
        MatchOutcome::Draw => 2,
        MatchOutcome::Player2Win => 4,
    };

    let obj_bonus = if my_objectives >= 5 { 1 } else { 0 };
    let close_loss = if matches!(outcome, MatchOutcome::Player2Win)
        && my_objectives.saturating_add(2) >= opp_objectives
    {
        1
    } else {
        0
    };

    base + obj_bonus + close_loss
}

pub fn tournament_points_for_player(
    outcome: MatchOutcome,
    is_player1: bool,
    p1_objectives: u8,
    p2_objectives: u8,
    is_forfeit_winner: bool,
) -> u8 {
    if is_forfeit_winner {
        return 5;
    }

    let player_outcome = if is_player1 {
        outcome
    } else {
        match outcome {
            MatchOutcome::Player1Win => MatchOutcome::Player2Win,
            MatchOutcome::Player2Win => MatchOutcome::Player1Win,
            MatchOutcome::Draw => MatchOutcome::Draw,
        }
    };

    let (my_obj, opp_obj) = if is_player1 {
        (p1_objectives, p2_objectives)
    } else {
        (p2_objectives, p1_objectives)
    };

    let base = match player_outcome {
        MatchOutcome::Player1Win => 4,
        MatchOutcome::Draw => 2,
        MatchOutcome::Player2Win => 0,
    };

    let obj_bonus = if my_obj >= 5 { 1 } else { 0 };
    let close_loss = if matches!(player_outcome, MatchOutcome::Player2Win)
        && my_obj.saturating_add(2) >= opp_obj
    {
        1
    } else {
        0
    };

    base + obj_bonus + close_loss
}

pub fn compute_elo_deltas(
    rating1: f64,
    rating2: f64,
    outcome: MatchOutcome,
    k_factor: f64,
) -> (f64, f64) {
    let score1 = outcome.score_for_player1();
    let (new1, new2) = elo::update_ratings(rating1, rating2, score1, k_factor);
    (new1 - rating1, new2 - rating2)
}

pub fn placement_label(placement: u32) -> String {
    match placement {
        1 => "1er".into(),
        2 => "2ème".into(),
        3 | 4 => "Top 4".into(),
        5..=8 => "Top 8".into(),
        n => format!("Top {n}"),
    }
}

pub fn pool_round_robin_pairs(player_count: usize) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    for i in 0..player_count {
        for j in (i + 1)..player_count {
            pairs.push((i, j));
        }
    }
    pairs
}

pub fn compute_display_status(
    tournament: &Tournament,
    matches: &[TournamentMatch],
) -> String {
    match tournament.status {
        TournamentStatus::Draft => "À venir".into(),
        TournamentStatus::RegistrationOpen => "Inscription ouvertes".into(),
        TournamentStatus::RegistrationClosed => "Inscriptions closes".into(),
        TournamentStatus::Completed => "Terminé".into(),
        TournamentStatus::Started => compute_started_display_status(tournament, matches),
    }
}

fn compute_started_display_status(
    tournament: &Tournament,
    matches: &[TournamentMatch],
) -> String {
    if tournament.pools_finalized_at.is_none() {
        return "Phase de poules".into();
    }

    let phases = [
        (TournamentPhase::RoundOf16, "Seizièmes de finale"),
        (TournamentPhase::Quarter, "Quart de finale"),
        (TournamentPhase::Semi, "Demi-finale"),
        (TournamentPhase::Final, "Finale"),
    ];

    for (phase, label) in phases {
        let phase_matches: Vec<_> = matches
            .iter()
            .filter(|m| m.phase == phase)
            .collect();
        if phase_matches.is_empty() {
            continue;
        }
        if phase_matches
            .iter()
            .any(|m| m.status != TournamentMatchStatus::Confirmed)
        {
            return label.into();
        }
    }

    "Finale".into()
}

/// Vainqueur d'un match d'arbre. En cas de nul aux objectifs, le joueur avec le plus
/// de points de survivants l'emporte.
pub fn bracket_match_winner(tm: &TournamentMatch) -> Option<String> {
    let outcome = tm.outcome?;
    let p1 = tm.player1.clone()?;
    let p2 = tm.player2.clone()?;
    match outcome {
        MatchOutcome::Player1Win => Some(p1),
        MatchOutcome::Player2Win => Some(p2),
        MatchOutcome::Draw => {
            if tm.player1_survivors > tm.player2_survivors {
                Some(p1)
            } else if tm.player2_survivors > tm.player1_survivors {
                Some(p2)
            } else {
                None
            }
        }
    }
}

pub fn bracket_match_loser(tm: &TournamentMatch) -> Option<String> {
    let winner = bracket_match_winner(tm)?;
    let p1 = tm.player1.clone()?;
    let p2 = tm.player2.clone()?;
    if winner.eq_ignore_ascii_case(&p1) {
        Some(p2)
    } else {
        Some(p1)
    }
}

pub fn compute_top_four(matches: &[TournamentMatch]) -> Vec<TournamentTopFourEntry> {
    let Some(final_match) = matches.iter().find(|m| {
        m.phase == TournamentPhase::Final
            && m.status == TournamentMatchStatus::Confirmed
            && !m.is_forfeit
    }) else {
        return Vec::new();
    };

    let Some(player1) = final_match.player1.clone() else {
        return Vec::new();
    };
    let Some(player2) = final_match.player2.clone() else {
        return Vec::new();
    };
    let Some(outcome) = final_match.outcome else {
        return Vec::new();
    };

    let (first, second) = match outcome {
        MatchOutcome::Player1Win => (player1, player2),
        MatchOutcome::Player2Win => (player2, player1),
        MatchOutcome::Draw => {
            let Some(winner) = bracket_match_winner(final_match) else {
                return Vec::new();
            };
            if winner.eq_ignore_ascii_case(&player1) {
                (player1, player2)
            } else {
                (player2, player1)
            }
        }
    };

    let mut entries = vec![
        TournamentTopFourEntry {
            rank: 1,
            player_name: first,
            player_display_name: None,
        },
        TournamentTopFourEntry {
            rank: 2,
            player_name: second,
            player_display_name: None,
        },
    ];

    let semi_losers: Vec<String> = matches
        .iter()
        .filter(|m| {
            m.phase == TournamentPhase::Semi
                && m.status == TournamentMatchStatus::Confirmed
                && !m.is_forfeit
        })
        .filter_map(|m| bracket_match_loser(m))
        .collect();

    for (index, player_name) in semi_losers.into_iter().take(2).enumerate() {
        entries.push(TournamentTopFourEntry {
            rank: 3 + index as u32,
            player_name,
            player_display_name: None,
        });
    }

    entries
}

pub fn registration_counts(registrations: &[TournamentRegistration]) -> (u32, u32) {
    let approved = registrations
        .iter()
        .filter(|r| r.status == RegistrationStatus::Approved)
        .count() as u32;
    let waitlist = registrations
        .iter()
        .filter(|r| r.status == RegistrationStatus::Waitlisted)
        .count() as u32;
    (approved, waitlist)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tournament_points_examples() {
        assert_eq!(
            tournament_points_for_player(
                MatchOutcome::Player1Win,
                true,
                6,
                2,
                false
            ),
            5
        );
        assert_eq!(
            tournament_points_for_player(
                MatchOutcome::Player1Win,
                false,
                6,
                2,
                false
            ),
            0
        );
        assert_eq!(
            tournament_points_for_player(
                MatchOutcome::Player1Win,
                true,
                4,
                3,
                false
            ),
            4
        );
        assert_eq!(
            tournament_points_for_player(
                MatchOutcome::Player1Win,
                false,
                4,
                3,
                false
            ),
            1
        );
        assert_eq!(
            tournament_points_for_player(
                MatchOutcome::Draw,
                true,
                5,
                5,
                false
            ),
            3
        );
    }

    #[test]
    fn bracket_match_winner_uses_survivors_on_draw() {
        let semi_draw = TournamentMatch {
            id: 1,
            tournament_id: 1,
            phase: TournamentPhase::Semi,
            pool_id: None,
            bracket_slot: Some(0),
            player1: Some("Ayadan".into()),
            player2: Some("Shas'O Kassad".into()),
            player1_display_name: None,
            player2_display_name: None,
            player1_objectives: 3,
            player2_objectives: 3,
            player1_survivors: 149,
            player2_survivors: 45,
            player1_tournament_points: 3,
            player2_tournament_points: 3,
            outcome: Some(MatchOutcome::Draw),
            is_forfeit: false,
            is_unplayed: false,
            forfeit_player: None,
            forfeit_player_display_name: None,
            player1_elo_delta: 0.0,
            player2_elo_delta: 0.0,
            player1_rating_used: None,
            player2_rating_used: None,
            elo_applied_at: None,
            status: TournamentMatchStatus::Confirmed,
            submitted_by_user_id: None,
            submitted_at: None,
            confirmed_by_user_id: None,
            confirmed_at: None,
            scenario_id: None,
            scenario_name: None,
            player1_army_id: None,
            player2_army_id: None,
            played_at: None,
        };

        assert_eq!(bracket_match_winner(&semi_draw).as_deref(), Some("Ayadan"));
        assert_eq!(
            bracket_match_loser(&semi_draw).as_deref(),
            Some("Shas'O Kassad")
        );
    }
}

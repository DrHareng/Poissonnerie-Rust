use serde::{Deserialize, Serialize};

use crate::elo;
use crate::player::MatchOutcome;

pub const MAX_POOL_SIZE: usize = 6;
pub const POOLS_FOUR_CAPACITY: usize = 24;
pub const POOLS_EIGHT_CAPACITY: usize = 48;
/// Historique : seuil avant bascule 8 poules (désactivée — on reste à 24 + waitlist).
pub const WAITLIST_THRESHOLD: usize = 32;
pub const POOL_SCENARIO_LETTERS: &[char] = &['A', 'B', 'C', 'D', 'E'];
pub const BRACKET_SCENARIO_COUNT: usize = 4;

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
    #[serde(rename = "round_of_16")]
    RoundOf16,
    #[serde(rename = "round_of_16_full")]
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
    #[serde(rename = "round_of_16")]
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

    pub fn label(self) -> &'static str {
        match self {
            Self::Pool => "Poule",
            Self::RoundOf16 => "1/8 de finale",
            Self::Quarter => "1/4 de final",
            Self::Semi => "Demi-finale",
            Self::Final => "Finale",
        }
    }
}

pub fn phase_label(phase: &str) -> &str {
    TournamentPhase::parse(phase)
        .map(TournamentPhase::label)
        .unwrap_or(phase)
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
    #[serde(default)]
    pub description: String,
    pub status: TournamentStatus,
    pub pool_count: u8,
    pub bracket_format: BracketFormat,
    pub created_at: u64,
    pub started_at: Option<u64>,
    pub pools_finalized_at: Option<u64>,
    pub completed_at: Option<u64>,
    /// Utilisateur chargé de valider les listes d'armées (prérequis au démarrage).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_validator_user_id: Option<i64>,
    /// Affichage enrichi côté API (non stocké en base).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_validator_display_name: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub army_list_1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub army_list_2: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub army_list_1_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub army_list_2_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bracket_list_1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bracket_list_2: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bracket_list_1_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bracket_list_2_id: Option<i64>,
    /// Liste 1 d'inscription renseignée (sans révéler le code).
    #[serde(default)]
    pub has_army_lists: bool,
    /// Liste 1 d'arbre renseignée (sans révéler le code).
    #[serde(default)]
    pub has_bracket_lists: bool,
    #[serde(default)]
    pub has_army_list_2: bool,
    #[serde(default)]
    pub has_bracket_list_2: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TournamentScenarioSlot {
    /// `pool`, `bracket_pool` (4 scénarios choisis) ou `bracket` (assignés aux tours).
    pub kind: String,
    /// Lettre A–E (poules), index 0–3 (bracket_pool), ou phase (`round_of_16`, …).
    pub slot: String,
    pub scenario_id: i64,
    pub scenario_name: String,
    #[serde(default)]
    pub scenario_slug: String,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundOf16BarrageSlot {
    pub bracket_slot: u32,
    pub player1: String,
    pub player2: String,
    pub quarter_player1: String,
}

pub fn sort_pool_standings(players: &mut [PoolPlayer]) {
    players.sort_by(|left, right| {
        right
            .points
            .cmp(&left.points)
            .then_with(|| right.objectives.cmp(&left.objectives))
            .then_with(|| right.survivors.cmp(&left.survivors))
    });
}

/// Barrages 2e vs 3e entre poules (4 poules) ; les 1ers attendent en quart.
pub fn round_of_16_barrage_pairings(
    pool_standings: &[Vec<PoolPlayer>],
) -> Option<Vec<RoundOf16BarrageSlot>> {
    if pool_standings.len() != 4 {
        return None;
    }

    let first = |index: usize| -> Option<&str> {
        pool_standings[index].first().map(|player| player.player_name.as_str())
    };
    let second = |index: usize| -> Option<&str> {
        pool_standings[index]
            .get(1)
            .map(|player| player.player_name.as_str())
    };
    let third = |index: usize| -> Option<&str> {
        pool_standings[index]
            .get(2)
            .map(|player| player.player_name.as_str())
    };

    let configs = [(3, 1, 0), (2, 0, 1), (0, 3, 2), (1, 2, 3)];
    let mut slots = Vec::with_capacity(configs.len());
    for (slot, (second_pool, third_pool, first_pool)) in configs.iter().enumerate() {
        slots.push(RoundOf16BarrageSlot {
            bracket_slot: slot as u32,
            player1: second(*second_pool)?.to_string(),
            player2: third(*third_pool)?.to_string(),
            quarter_player1: first(*first_pool)?.to_string(),
        });
    }
    Some(slots)
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
    pub scenario_other: Option<String>,
    /// Libellé d'affichage (nom catalogue ou texte libre).
    pub scenario_name: Option<String>,
    pub player1_army_id: Option<u32>,
    pub player2_army_id: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player1_army_list_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player2_army_list_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player1_army_list_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player2_army_list_id: Option<i64>,
    pub played_at: Option<u64>,
    /// Partie ELO / wizard liée (en cours ou terminée).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elo_match_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlayerTournamentResult {
    pub tournament_id: i64,
    pub tournament_name: String,
    pub placement_label: String,
    pub final_placement: Option<u32>,
    pub completed_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub army_id: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TournamentTopFourEntry {
    pub rank: u32,
    pub player_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub army_id: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TournamentRegistrationPreview {
    pub player_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_display_name: Option<String>,
    pub status: RegistrationStatus,
    #[serde(default)]
    pub has_army_lists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TournamentListEntry {
    #[serde(flatten)]
    pub tournament: Tournament,
    /// Inscrits comptant pour la capacité (pending + approved).
    pub registered_count: u32,
    pub waitlist_count: u32,
    pub display_status: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub top_four: Vec<TournamentTopFourEntry>,
    /// Matchs d'arbre (hors poules), pour le mini-rendu dans la liste.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bracket_matches: Vec<TournamentMatch>,
    /// Scénarios de poules (pour affichage sous la description).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pool_scenarios: Vec<TournamentScenarioSlot>,
    /// Inscrits actifs (phase d'inscription uniquement).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub registrations: Vec<TournamentRegistrationPreview>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TournamentDetail {
    #[serde(flatten)]
    pub tournament: Tournament,
    pub registrations: Vec<TournamentRegistration>,
    pub players: Vec<TournamentPlayerSnapshot>,
    pub pools: Vec<Pool>,
    pub matches: Vec<TournamentMatch>,
    /// Inscrits comptant pour la capacité (pending + approved).
    pub registered_count: u32,
    pub waitlist_count: u32,
    pub display_status: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub top_four: Vec<TournamentTopFourEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pool_scenarios: Vec<TournamentScenarioSlot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bracket_scenario_pool: Vec<TournamentScenarioSlot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bracket_scenarios: Vec<TournamentScenarioSlot>,
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
        9..=12 => "Top 12".into(),
        n => format!("Top {n}"),
    }
}

fn phase_losers(matches: &[TournamentMatch], phase: TournamentPhase) -> Vec<String> {
    let mut losers: Vec<(u32, String)> = matches
        .iter()
        .filter(|m| {
            m.phase == phase && m.status == TournamentMatchStatus::Confirmed
        })
        .filter_map(|m| {
            let loser = bracket_match_loser(m)?;
            Some((m.bracket_slot.unwrap_or(0), loser))
        })
        .collect();
    losers.sort_by_key(|(slot, _)| *slot);
    losers.into_iter().map(|(_, name)| name).collect()
}

pub fn compute_bracket_placements(
    matches: &[TournamentMatch],
    bracket_format: BracketFormat,
) -> std::collections::HashMap<String, u32> {
    use std::collections::HashMap;

    let mut placements = HashMap::new();

    // Du tour le plus avancé au plus précoce : ne jamais écraser un meilleur classement
    // (ex. vainqueur de finale déjà placé 1er qui apparaît aussi comme perdant d'un tour antérieur
    // à cause de données d'arbre incohérentes).
    let insert_best = |placements: &mut HashMap<String, u32>, name: String, placement: u32| {
        placements.entry(name).or_insert(placement);
    };

    if let Some(final_match) = matches.iter().find(|m| {
        m.phase == TournamentPhase::Final && m.status == TournamentMatchStatus::Confirmed
    }) {
        if let (Some(winner), Some(loser)) = (
            bracket_match_winner(final_match),
            bracket_match_loser(final_match),
        ) {
            insert_best(&mut placements, winner, 1);
            insert_best(&mut placements, loser, 2);
        }
    }

    for (index, loser) in phase_losers(matches, TournamentPhase::Semi)
        .into_iter()
        .take(2)
        .enumerate()
    {
        insert_best(&mut placements, loser, 3 + index as u32);
    }

    for (index, loser) in phase_losers(matches, TournamentPhase::Quarter)
        .into_iter()
        .take(4)
        .enumerate()
    {
        insert_best(&mut placements, loser, 5 + index as u32);
    }

    let r16_slots = match bracket_format {
        BracketFormat::RoundOf16 => 4,
        BracketFormat::RoundOf16Full => 8,
        BracketFormat::QuartersDirect => 0,
    };
    if r16_slots > 0 {
        for (index, loser) in phase_losers(matches, TournamentPhase::RoundOf16)
            .into_iter()
            .take(r16_slots)
            .enumerate()
        {
            insert_best(&mut placements, loser, 9 + index as u32);
        }
    }

    placements
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

/// Lettre de mission pour un match entre deux slots (0-based) d'une poule.
/// Matrice Excel Coupe : 4 joueurs = A/B/C ; 5–6 joueurs = A–E.
pub fn pool_scenario_letter(player_count: usize, slot_a: usize, slot_b: usize) -> Option<char> {
    if slot_a == slot_b || slot_a >= player_count || slot_b >= player_count {
        return None;
    }
    let (lo, hi) = if slot_a < slot_b {
        (slot_a, slot_b)
    } else {
        (slot_b, slot_a)
    };

    match player_count {
        4 => match (lo, hi) {
            (0, 1) => Some('A'),
            (0, 2) => Some('B'),
            (0, 3) => Some('C'),
            (1, 2) => Some('C'),
            (1, 3) => Some('B'),
            (2, 3) => Some('A'),
            _ => None,
        },
        5 | 6 => match (lo, hi) {
            (0, 1) => Some('A'),
            (0, 2) => Some('B'),
            (0, 3) => Some('C'),
            (0, 4) => Some('D'),
            (0, 5) => Some('E'),
            (1, 2) => Some('E'),
            (1, 3) => Some('D'),
            (1, 4) => Some('B'),
            (1, 5) => Some('C'),
            (2, 3) => Some('A'),
            (2, 4) => Some('C'),
            (2, 5) => Some('D'),
            (3, 4) => Some('E'),
            (3, 5) => Some('B'),
            (4, 5) => Some('A'),
            _ => None,
        },
        n if n >= 2 && n <= 3 => {
            // Petites poules : cycle A/B/C.
            let letters = ['A', 'B', 'C'];
            let idx = lo * n + hi;
            Some(letters[idx % letters.len()])
        }
        _ => None,
    }
}

/// Répartit les joueurs en poules : top 1–4 séparés, top 5–8 séparés, reste au sort.
/// L'ordre dans chaque poule = slot scénario (mélangé).
pub fn draw_seeded_pools(players: &[(String, f64)], pool_count: usize) -> Vec<Vec<String>> {
    use rand::seq::SliceRandom;

    let mut ranked = players.to_vec();
    ranked.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.0.cmp(&right.0))
    });

    let mut pools: Vec<Vec<String>> = (0..pool_count).map(|_| Vec::new()).collect();
    let mut rng = rand::rng();

    let take_named = |ranked: &mut Vec<(String, f64)>, n: usize| -> Vec<String> {
        let count = n.min(ranked.len());
        ranked.drain(..count).map(|(name, _)| name).collect()
    };

    let mut top = take_named(&mut ranked, pool_count.min(4));
    top.shuffle(&mut rng);
    for (index, name) in top.into_iter().enumerate() {
        pools[index % pool_count].push(name);
    }

    let mut mid = take_named(&mut ranked, pool_count.min(4));
    mid.shuffle(&mut rng);
    for (index, name) in mid.into_iter().enumerate() {
        pools[index % pool_count].push(name);
    }

    let mut rest: Vec<String> = ranked.into_iter().map(|(name, _)| name).collect();
    rest.shuffle(&mut rng);
    for name in rest {
        let target = pools
            .iter()
            .enumerate()
            .min_by_key(|(_, pool)| pool.len())
            .map(|(index, _)| index)
            .unwrap_or(0);
        pools[target].push(name);
    }

    for pool in &mut pools {
        pool.shuffle(&mut rng);
    }

    pools
}

/// Phases d'arbre qui reçoivent chacune un scénario (format Coupe 10).
pub fn bracket_scenario_phases(format: BracketFormat) -> Vec<TournamentPhase> {
    match format {
        BracketFormat::QuartersDirect => {
            vec![TournamentPhase::Quarter, TournamentPhase::Semi, TournamentPhase::Final]
        }
        BracketFormat::RoundOf16 | BracketFormat::RoundOf16Full => vec![
            TournamentPhase::RoundOf16,
            TournamentPhase::Quarter,
            TournamentPhase::Semi,
            TournamentPhase::Final,
        ],
    }
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
        (TournamentPhase::RoundOf16, "1/8 de finale"),
        (TournamentPhase::Quarter, "1/4 de final"),
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
            army_id: None,
        },
        TournamentTopFourEntry {
            rank: 2,
            player_name: second,
            player_display_name: None,
            army_id: None,
        },
    ];

    let semi_losers = phase_losers(matches, TournamentPhase::Semi);

    for (index, player_name) in semi_losers.into_iter().take(2).enumerate() {
        entries.push(TournamentTopFourEntry {
            rank: 3 + index as u32,
            player_name,
            player_display_name: None,
            army_id: None,
        });
    }

    entries
}

pub fn enrich_top_four_armies(
    top_four: &mut [TournamentTopFourEntry],
    registrations: &[TournamentRegistration],
) {
    for entry in top_four {
        entry.army_id = registrations
            .iter()
            .find(|registration| {
                registration
                    .player_name
                    .eq_ignore_ascii_case(&entry.player_name)
            })
            .and_then(|registration| registration.army_id);
    }
}

pub fn registration_counts(registrations: &[TournamentRegistration]) -> (u32, u32) {
    // Compte les places « prises » : inscrits en cours (pending) + validés.
    let registered = registrations
        .iter()
        .filter(|r| {
            matches!(
                r.status,
                RegistrationStatus::Pending | RegistrationStatus::Approved
            )
        })
        .count() as u32;
    let waitlist = registrations
        .iter()
        .filter(|r| r.status == RegistrationStatus::Waitlisted)
        .count() as u32;
    (registered, waitlist)
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
            scenario_other: None,
            scenario_name: None,
            player1_army_id: None,
            player2_army_id: None,
            player1_army_list_code: None,
            player2_army_list_code: None,
            player1_army_list_id: None,
            player2_army_list_id: None,
            played_at: None,
            elo_match_id: None,
        };

        assert_eq!(bracket_match_winner(&semi_draw).as_deref(), Some("Ayadan"));
        assert_eq!(
            bracket_match_loser(&semi_draw).as_deref(),
            Some("Shas'O Kassad")
        );
    }

    #[test]
    fn round_of_16_barrage_pairings_cross_pools() {
        fn player(name: &str, points: u32) -> PoolPlayer {
            PoolPlayer {
                player_name: name.into(),
                player_display_name: None,
                army_id: None,
                seed: 0,
                points,
                objectives: 0,
                survivors: 0,
                wins: 0,
                draws: 0,
                losses: 0,
            }
        }

        let standings = vec![
            vec![player("1A", 30), player("2A", 20), player("3A", 10)],
            vec![player("1B", 30), player("2B", 20), player("3B", 10)],
            vec![player("1C", 30), player("2C", 20), player("3C", 10)],
            vec![player("1D", 30), player("2D", 20), player("3D", 10)],
        ];

        let slots = round_of_16_barrage_pairings(&standings).unwrap();
        assert_eq!(slots.len(), 4);
        assert_eq!(slots[0].player1, "2D");
        assert_eq!(slots[0].player2, "3B");
        assert_eq!(slots[0].quarter_player1, "1A");
        assert_eq!(slots[3].player1, "2B");
        assert_eq!(slots[3].player2, "3C");
        assert_eq!(slots[3].quarter_player1, "1D");
    }

    #[test]
    fn pool_scenario_letter_four_players_matrix() {
        assert_eq!(pool_scenario_letter(4, 0, 1), Some('A'));
        assert_eq!(pool_scenario_letter(4, 0, 2), Some('B'));
        assert_eq!(pool_scenario_letter(4, 0, 3), Some('C'));
        assert_eq!(pool_scenario_letter(4, 1, 2), Some('C'));
        assert_eq!(pool_scenario_letter(4, 1, 3), Some('B'));
        assert_eq!(pool_scenario_letter(4, 2, 3), Some('A'));
        // Distinct from the 6-player top-left corner (which would be E for 1vs2→ wait 2vs3).
        assert_ne!(pool_scenario_letter(4, 1, 2), pool_scenario_letter(6, 1, 2));
    }

    #[test]
    fn pool_scenario_letter_six_extends_five() {
        assert_eq!(pool_scenario_letter(5, 0, 1), Some('A'));
        assert_eq!(pool_scenario_letter(5, 1, 2), Some('E'));
        assert_eq!(pool_scenario_letter(5, 3, 4), Some('E'));
        assert_eq!(pool_scenario_letter(6, 0, 5), Some('E'));
        assert_eq!(pool_scenario_letter(6, 4, 5), Some('A'));
        assert_eq!(pool_scenario_letter(6, 1, 2), pool_scenario_letter(5, 1, 2));
    }

    #[test]
    fn draw_seeded_pools_separates_top_eight() {
        let players: Vec<(String, f64)> = (1..=24)
            .map(|n| (format!("P{n}"), 2000.0 - n as f64))
            .collect();
        let pools = draw_seeded_pools(&players, 4);
        assert_eq!(pools.len(), 4);
        assert_eq!(pools.iter().map(|p| p.len()).sum::<usize>(), 24);

        let top4: Vec<String> = (1..=4).map(|n| format!("P{n}")).collect();
        let top8: Vec<String> = (5..=8).map(|n| format!("P{n}")).collect();
        for name in &top4 {
            let count = pools.iter().filter(|p| p.contains(name)).count();
            assert_eq!(count, 1, "{name} should appear once");
        }
        // One top-4 per pool
        for pool in &pools {
            assert_eq!(
                pool.iter().filter(|n| top4.contains(n)).count(),
                1,
                "exactly one top-4 in {pool:?}"
            );
            assert_eq!(
                pool.iter().filter(|n| top8.contains(n)).count(),
                1,
                "exactly one top 5-8 in {pool:?}"
            );
        }
    }

    #[test]
    fn placement_label_covers_top_buckets() {
        assert_eq!(placement_label(8), "Top 8");
        assert_eq!(placement_label(9), "Top 12");
        assert_eq!(placement_label(12), "Top 12");
    }

    #[test]
    fn compute_bracket_placements_includes_forfeit_r16_losers() {
        let matches = vec![
            TournamentMatch {
                id: 1,
                tournament_id: 1,
                phase: TournamentPhase::RoundOf16,
                pool_id: None,
                bracket_slot: Some(2),
                player1: Some("Ayadan".into()),
                player2: Some("Gui Zou".into()),
                player1_display_name: None,
                player2_display_name: None,
                player1_objectives: 0,
                player2_objectives: 0,
                player1_survivors: 0,
                player2_survivors: 0,
                player1_tournament_points: 0,
                player2_tournament_points: 0,
                outcome: Some(MatchOutcome::Player1Win),
                is_forfeit: true,
                is_unplayed: false,
                forfeit_player: Some("Gui Zou".into()),
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
                scenario_other: None,
                scenario_name: None,
                player1_army_id: None,
                player2_army_id: None,
                player1_army_list_code: None,
                player2_army_list_code: None,
            player1_army_list_id: None,
            player2_army_list_id: None,
                played_at: None,
                elo_match_id: None,
            },
        ];

        let placements = compute_bracket_placements(&matches, BracketFormat::RoundOf16);
        assert_eq!(placements.get("Gui Zou"), Some(&9));
    }

    #[test]
    fn compute_bracket_placements_keeps_final_winner_over_earlier_loss() {
        let matches = vec![
            TournamentMatch {
                id: 1,
                tournament_id: 1,
                phase: TournamentPhase::Quarter,
                pool_id: None,
                bracket_slot: Some(0),
                player1: Some("Arkille".into()),
                player2: Some("Azazel".into()),
                player1_display_name: None,
                player2_display_name: None,
                player1_objectives: 10,
                player2_objectives: 3,
                player1_survivors: 191,
                player2_survivors: 105,
                player1_tournament_points: 4,
                player2_tournament_points: 0,
                outcome: Some(MatchOutcome::Player1Win),
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
                scenario_other: None,
                scenario_name: None,
                player1_army_id: None,
                player2_army_id: None,
                player1_army_list_code: None,
                player2_army_list_code: None,
            player1_army_list_id: None,
            player2_army_list_id: None,
                played_at: None,
                elo_match_id: None,
            },
            TournamentMatch {
                id: 2,
                tournament_id: 1,
                phase: TournamentPhase::Final,
                pool_id: None,
                bracket_slot: Some(0),
                player1: Some("Azazel".into()),
                player2: Some("Shas'O Kassad".into()),
                player1_display_name: None,
                player2_display_name: None,
                player1_objectives: 7,
                player2_objectives: 4,
                player1_survivors: 102,
                player2_survivors: 104,
                player1_tournament_points: 4,
                player2_tournament_points: 0,
                outcome: Some(MatchOutcome::Player1Win),
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
                scenario_other: None,
                scenario_name: None,
                player1_army_id: None,
                player2_army_id: None,
                player1_army_list_code: None,
                player2_army_list_code: None,
            player1_army_list_id: None,
            player2_army_list_id: None,
                played_at: None,
                elo_match_id: None,
            },
        ];

        let placements = compute_bracket_placements(&matches, BracketFormat::RoundOf16);
        assert_eq!(placements.get("Azazel"), Some(&1));
        assert_eq!(placements.get("Shas'O Kassad"), Some(&2));
        // Perdant du quart = Azazel, déjà classé 1er via la finale → pas d'écrasement en Top 8
        assert_eq!(placements.get("Arkille"), None);
    }

    #[test]
    fn compute_top_four_includes_forfeit_semi_losers() {
        fn semi(
            id: i64,
            slot: u32,
            p1: &str,
            p2: &str,
            outcome: MatchOutcome,
            is_forfeit: bool,
        ) -> TournamentMatch {
            TournamentMatch {
                id,
                tournament_id: 1,
                phase: TournamentPhase::Semi,
                pool_id: None,
                bracket_slot: Some(slot),
                player1: Some(p1.into()),
                player2: Some(p2.into()),
                player1_display_name: None,
                player2_display_name: None,
                player1_objectives: 0,
                player2_objectives: 0,
                player1_survivors: 0,
                player2_survivors: 0,
                player1_tournament_points: 0,
                player2_tournament_points: 0,
                outcome: Some(outcome),
                is_forfeit,
                is_unplayed: false,
                forfeit_player: if is_forfeit {
                    Some(p2.into())
                } else {
                    None
                },
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
                scenario_other: None,
                scenario_name: None,
                player1_army_id: None,
                player2_army_id: None,
                player1_army_list_code: None,
                player2_army_list_code: None,
            player1_army_list_id: None,
            player2_army_list_id: None,
                played_at: None,
                elo_match_id: None,
            }
        }

        let matches = vec![
            TournamentMatch {
                id: 100,
                tournament_id: 1,
                phase: TournamentPhase::Final,
                pool_id: None,
                bracket_slot: Some(0),
                player1: Some("Dr Hareng".into()),
                player2: Some("Badgage".into()),
                player1_display_name: None,
                player2_display_name: None,
                player1_objectives: 6,
                player2_objectives: 2,
                player1_survivors: 100,
                player2_survivors: 40,
                player1_tournament_points: 5,
                player2_tournament_points: 0,
                outcome: Some(MatchOutcome::Player1Win),
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
                scenario_other: None,
                scenario_name: None,
                player1_army_id: None,
                player2_army_id: None,
                player1_army_list_code: None,
                player2_army_list_code: None,
            player1_army_list_id: None,
            player2_army_list_id: None,
                played_at: None,
                elo_match_id: None,
            },
            semi(1, 0, "Dr Hareng", "Arkille", MatchOutcome::Player1Win, false),
            semi(2, 1, "Badgage", "Kantain", MatchOutcome::Player1Win, true),
        ];

        let top_four = compute_top_four(&matches);
        assert_eq!(top_four.len(), 4);
        assert_eq!(top_four[0].player_name, "Dr Hareng");
        assert_eq!(top_four[1].player_name, "Badgage");
        assert_eq!(top_four[2].player_name, "Arkille");
        assert_eq!(top_four[3].player_name, "Kantain");
    }
}

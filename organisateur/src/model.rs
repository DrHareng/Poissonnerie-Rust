use serde::{Deserialize, Serialize};

pub fn default_round_minutes() -> u16 {
    150
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Preparation,
    Pairing,
    InProgress,
    Finished,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Player {
    pub id: u32,
    pub name: String,
    pub sectorial_id: u32,
    pub order: u32,
    pub dropped: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dropped_round: Option<u8>,
    /// Forfait posé pendant l'appariement, avant que la ronde ne commence.
    #[serde(default)]
    pub drop_before_pairing_validated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameResult {
    pub p1_op: u8,
    pub p1_vp: u16,
    pub p2_op: u8,
    pub p2_vp: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forfeit_player: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Match {
    pub round: u8,
    pub player1: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player2: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<GameResult>,
}

impl Match {
    pub fn is_bye(&self) -> bool {
        self.player2.is_none()
    }

    pub fn is_scored(&self) -> bool {
        self.is_bye() || self.result.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tournament {
    pub id: String,
    pub name: String,
    pub rounds: u8,
    pub tables: u16,
    /// Durée d'une ronde, en minutes. 150 = 2 h 30.
    #[serde(default = "default_round_minutes")]
    pub round_minutes: u16,
    pub status: Status,
    pub round: u8,
    pub players: Vec<Player>,
    pub matches: Vec<Match>,
    /// Paires à éviter au prochain relancement, normalisées (min, max).
    pub flagged: Vec<(u32, u32)>,
    pub pending_bye: Option<u32>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Create {
        id: String,
        name: String,
        rounds: u8,
        tables: u16,
    },
    UpdateMeta {
        name: String,
        rounds: u8,
        tables: u16,
        round_minutes: u16,
    },
    SetRoundMinutes {
        minutes: u16,
    },
    AddPlayer {
        name: String,
        sectorial_id: u32,
    },
    UpdatePlayer {
        player_id: u32,
        name: String,
        sectorial_id: u32,
    },
    RemovePlayer {
        player_id: u32,
    },
    Start {
        seed: String,
    },
    SetBye {
        player_id: u32,
        seed: String,
    },
    SetFlag {
        player_a: u32,
        player_b: u32,
        flagged: bool,
    },
    Regenerate {
        seed: String,
    },
    /// Revenir à la saisie des joueurs tant que l'appariement de la première ronde n'est pas validé.
    ReturnToPlayers,
    ValidatePairing,
    SetResult {
        player1: u32,
        p1_op: u8,
        p1_vp: u16,
        p2_op: u8,
        p2_vp: u16,
    },
    DropPlayer {
        player_id: u32,
        seed: String,
    },
    UndoDrop {
        player_id: u32,
        seed: String,
    },
    ValidateRound {
        seed: String,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PlayerCard {
    pub id: u32,
    pub name: String,
    pub sectorial_id: u32,
    pub sectorial_name: String,
    pub short_name: String,
    pub dropped: bool,
    pub can_undo_drop: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MatchCard {
    pub round: u8,
    pub table: Option<u16>,
    pub bye: bool,
    pub player1: PlayerCard,
    pub player2: Option<PlayerCard>,
    pub p1_op: Option<u8>,
    pub p1_vp: Option<u16>,
    pub p1_tp: Option<u32>,
    pub p2_op: Option<u8>,
    pub p2_vp: Option<u16>,
    pub p2_tp: Option<u32>,
    pub forfeit_player: Option<u32>,
    pub scored: bool,
    pub flagged: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RoundCard {
    pub round: u8,
    pub provisional: bool,
    pub matches: Vec<MatchCard>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct StandingRow {
    pub rank: u32,
    pub player_id: u32,
    pub name: String,
    pub sectorial_name: String,
    pub short_name: String,
    pub tp: u32,
    pub op: u32,
    pub vp: u32,
    pub rounds_played: u32,
    pub op_final: f64,
    pub vp_final: f64,
    pub wins: u32,
    pub draws: u32,
    pub losses: u32,
    pub byes: u32,
    pub dropped: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ByeOption {
    pub player_id: u32,
    pub name: String,
    pub short_name: String,
    pub selected: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct View {
    pub min_tables: u16,
    pub can_start: bool,
    pub start_reason: Option<String>,
    pub can_validate_pairing: bool,
    pub can_validate_round: bool,
    pub bye_required: bool,
    pub bye_options: Vec<ByeOption>,
    /// Forfaits encore annulables, y compris les joueurs déjà sortis de l'appariement.
    pub recall: Vec<PlayerCard>,
    pub rounds: Vec<RoundCard>,
    pub standings: Vec<StandingRow>,
    pub warnings: Vec<String>,
    pub final_ranking: bool,
}

//! Domaine Dauphiné : tournoi annuel scénarisé, par équipes.
//!
//! Isolé du classement ELO et des coupes Infinity (`tournaments`).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditionStatus {
    Draft,
    Announced,
    Registration,
    Live,
    Completed,
}

impl EditionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Announced => "announced",
            Self::Registration => "registration",
            Self::Live => "live",
            Self::Completed => "completed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "draft" => Some(Self::Draft),
            "announced" => Some(Self::Announced),
            "registration" => Some(Self::Registration),
            "live" => Some(Self::Live),
            "completed" => Some(Self::Completed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeamMemberRole {
    Captain,
    Player,
}

impl TeamMemberRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Captain => "captain",
            Self::Player => "player",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "captain" => Some(Self::Captain),
            "player" => Some(Self::Player),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoundKind {
    Match,
    Narrative,
}

impl RoundKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Match => "match",
            Self::Narrative => "narrative",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "match" => Some(Self::Match),
            "narrative" => Some(Self::Narrative),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DauphineEdition {
    pub id: i64,
    pub slug: String,
    pub title: String,
    pub year: i32,
    pub status: EditionStatus,
    pub tagline: String,
    pub body_md: String,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DauphineTeam {
    pub id: i64,
    pub edition_id: i64,
    pub name: String,
    pub captain_user_id: i64,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DauphineTeamMember {
    pub id: i64,
    pub team_id: i64,
    pub user_id: i64,
    pub role: TeamMemberRole,
    pub army_id: Option<u32>,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DauphineRound {
    pub id: i64,
    pub edition_id: i64,
    pub position: u32,
    pub title: String,
    pub kind: RoundKind,
    pub body_md: String,
}

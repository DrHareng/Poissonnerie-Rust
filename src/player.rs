use serde::{Deserialize, Serialize};

use crate::elo::{self, MatchScore};

pub const DEFAULT_RATING: f64 = 1200.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Player {
    pub name: String,
    pub rating: f64,
    pub wins: u32,
    pub draws: u32,
    pub losses: u32,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "discord_id")]
    pub discord_username: Option<String>,
}

impl Player {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            rating: DEFAULT_RATING,
            wins: 0,
            draws: 0,
            losses: 0,
            discord_username: None,
        }
    }

    pub fn new_with_discord_username(
        name: impl Into<String>,
        discord_username: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            rating: DEFAULT_RATING,
            wins: 0,
            draws: 0,
            losses: 0,
            discord_username: Some(discord_username.into()),
        }
    }

    pub fn matches_played(&self) -> u32 {
        self.wins + self.draws + self.losses
    }

    pub fn record_match(&mut self, score: MatchScore) {
        match score {
            MatchScore::Win => self.wins += 1,
            MatchScore::Draw => self.draws += 1,
            MatchScore::Loss => self.losses += 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchOutcome {
    Player1Win,
    Player2Win,
    Draw,
}

impl MatchOutcome {
    pub fn score_for_player1(self) -> MatchScore {
        match self {
            MatchOutcome::Player1Win => MatchScore::Win,
            MatchOutcome::Draw => MatchScore::Draw,
            MatchOutcome::Player2Win => MatchScore::Loss,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RatingUpdate {
    pub player1_old: f64,
    pub player1_new: f64,
    pub player2_old: f64,
    pub player2_new: f64,
}

pub fn apply_match(
    player1: &mut Player,
    player2: &mut Player,
    outcome: MatchOutcome,
    k_factor: f64,
) -> RatingUpdate {
    let score1 = outcome.score_for_player1();
    let score2 = match score1 {
        MatchScore::Win => MatchScore::Loss,
        MatchScore::Draw => MatchScore::Draw,
        MatchScore::Loss => MatchScore::Win,
    };

    let old1 = player1.rating;
    let old2 = player2.rating;

    let (new1, new2) = elo::update_ratings(old1, old2, score1, k_factor);

    player1.rating = new1;
    player2.rating = new2;
    player1.record_match(score1);
    player2.record_match(score2);

    RatingUpdate {
        player1_old: old1,
        player1_new: new1,
        player2_old: old2,
        player2_new: new2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DEFAULT_K_FACTOR;

    #[test]
    fn new_player_starts_at_default_rating() {
        let player = Player::new("Alice");
        assert_eq!(player.rating, DEFAULT_RATING);
        assert_eq!(player.matches_played(), 0);
    }

    #[test]
    fn match_updates_stats_and_ratings() {
        let mut alice = Player::new("Alice");
        let mut bob = Player::new("Bob");

        let update = apply_match(&mut alice, &mut bob, MatchOutcome::Player1Win, DEFAULT_K_FACTOR);

        assert!(update.player1_new > update.player1_old);
        assert!(update.player2_new < update.player2_old);
        assert_eq!(alice.wins, 1);
        assert_eq!(bob.losses, 1);
    }
}

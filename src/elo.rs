/// Facteur K standard pour les joueurs amateurs.
pub const DEFAULT_K_FACTOR: f64 = 32.0;

/// Score réel d'un joueur après un match.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MatchScore {
    Win,
    Draw,
    Loss,
}

impl MatchScore {
    pub fn as_f64(self) -> f64 {
        match self {
            MatchScore::Win => 1.0,
            MatchScore::Draw => 0.5,
            MatchScore::Loss => 0.0,
        }
    }
}

/// Calcule le score attendu du joueur A face au joueur B.
pub fn expected_score(rating_a: f64, rating_b: f64) -> f64 {
    1.0 / (1.0 + 10_f64.powf((rating_b - rating_a) / 400.0))
}

/// Calcule le nouveau classement ELO après un match.
pub fn new_rating(current: f64, expected: f64, actual: f64, k_factor: f64) -> f64 {
    current + k_factor * (actual - expected)
}

/// Met à jour les classements des deux joueurs selon le résultat du match.
pub fn update_ratings(
    rating_a: f64,
    rating_b: f64,
    score_a: MatchScore,
    k_factor: f64,
) -> (f64, f64) {
    let expected_a = expected_score(rating_a, rating_b);
    let expected_b = expected_score(rating_b, rating_a);

    let actual_a = score_a.as_f64();
    let actual_b = 1.0 - actual_a;

    let new_a = new_rating(rating_a, expected_a, actual_a, k_factor);
    let new_b = new_rating(rating_b, expected_b, actual_b, k_factor);

    (new_a, new_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_ratings_give_fifty_percent_expected() {
        assert!((expected_score(1500.0, 1500.0) - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn higher_rated_player_wins_expected_score_increases() {
        let (new_winner, new_loser) =
            update_ratings(1600.0, 1400.0, MatchScore::Win, DEFAULT_K_FACTOR);
        assert!(new_winner > 1600.0);
        assert!(new_loser < 1400.0);
    }

    #[test]
    fn upset_gives_larger_gain_than_expected_win() {
        let (_, underdog_new) =
            update_ratings(1600.0, 1400.0, MatchScore::Loss, DEFAULT_K_FACTOR);
        let underdog_gain = underdog_new - 1400.0;

        let (favorite_new, _) =
            update_ratings(1600.0, 1400.0, MatchScore::Win, DEFAULT_K_FACTOR);
        let favorite_gain = favorite_new - 1600.0;

        assert!(underdog_gain > favorite_gain);
    }

    #[test]
    fn draw_moves_ratings_toward_each_other() {
        let (new_a, new_b) = update_ratings(1600.0, 1400.0, MatchScore::Draw, DEFAULT_K_FACTOR);
        assert!(new_a < 1600.0);
        assert!(new_b > 1400.0);
    }
}

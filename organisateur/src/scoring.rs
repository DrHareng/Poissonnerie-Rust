pub const MAX_OP: u8 = 10;
pub const MAX_VP: u16 = 300;
pub const BYE_TP: u32 = 4;
pub const FORFEIT_WIN_TP: u32 = 4;

/// Points de tournoi d'un joueur, calés sur `tournament_points_for_player`.
/// Victoire 4, nul 2, défaite 0, +1 à partir de 5 OP, +1 si défaite à 2 OP ou moins.
pub fn match_points(my_op: u8, opp_op: u8) -> u32 {
    let mut tp = if my_op > opp_op {
        4
    } else if my_op == opp_op {
        2
    } else {
        0
    };
    if my_op >= 5 {
        tp += 1;
    }
    if my_op < opp_op && my_op.saturating_add(2) >= opp_op {
        tp += 1;
    }
    tp
}

pub fn scaled(total: u32, rounds: u8, played: u32) -> f64 {
    if played == 0 {
        0.0
    } else {
        total as f64 * f64::from(rounds) / played as f64
    }
}

/// Compare deux totaux ramenés au nombre de rondes jouées. Plus grand = mieux.
pub fn cmp_scaled(total_a: u32, played_a: u32, total_b: u32, played_b: u32) -> std::cmp::Ordering {
    match (played_a == 0, played_b == 0) {
        (true, true) => std::cmp::Ordering::Equal,
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        (false, false) => (u64::from(total_a) * u64::from(played_b))
            .cmp(&(u64::from(total_b) * u64::from(played_a))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tournament_points_match_the_site() {
        assert_eq!(match_points(6, 2), 5);
        assert_eq!(match_points(2, 6), 0);
        assert_eq!(match_points(4, 3), 4);
        assert_eq!(match_points(3, 4), 1);
        assert_eq!(match_points(5, 5), 3);
        assert_eq!(match_points(0, 0), 2);
        assert_eq!(match_points(5, 7), 2);
        assert_eq!(match_points(7, 5), 5);
    }

    #[test]
    fn scale_uses_real_division() {
        assert_eq!(scaled(15, 3, 2), 22.5);
        assert_eq!(scaled(0, 3, 0), 0.0);
        assert_eq!(cmp_scaled(15, 2, 15, 2), std::cmp::Ordering::Equal);
        assert_eq!(cmp_scaled(10, 2, 16, 3), std::cmp::Ordering::Less);
    }
}

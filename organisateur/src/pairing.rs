use std::collections::{HashMap, HashSet};

use rand::seq::SliceRandom;
use rand::Rng;

pub fn pair_key(a: u32, b: u32) -> (u32, u32) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

pub struct Constraints<'a> {
    pub forbidden: &'a HashSet<(u32, u32)>,
    pub opponents: &'a HashMap<u32, HashSet<u32>>,
    pub table_history: &'a HashMap<u32, HashSet<u16>>,
    pub table_order: &'a [u16],
    pub allow_rematch: bool,
    pub allow_table_repeat: bool,
}

/// Apparie `preference` (du mieux classé au moins bien) et attribue une table.
/// Le premier succès respecte l'ordre : on éloigne un joueur de son vis-à-vis
/// naturel seulement si les contraintes l'exigent.
pub fn find_pairing(
    preference: &[u32],
    constraints: &Constraints<'_>,
    node_limit: u32,
) -> Option<Vec<(u32, u32, u16)>> {
    if preference.len() % 2 != 0 || constraints.table_order.is_empty() {
        return None;
    }
    let mut nodes = 0u32;
    let mut chosen: Vec<(u32, u32)> = Vec::new();
    search(preference, constraints, &mut chosen, &mut nodes, node_limit)
}

fn search(
    remaining: &[u32],
    constraints: &Constraints<'_>,
    chosen: &mut Vec<(u32, u32)>,
    nodes: &mut u32,
    node_limit: u32,
) -> Option<Vec<(u32, u32, u16)>> {
    *nodes = nodes.saturating_add(1);
    if *nodes > node_limit {
        return None;
    }
    if remaining.is_empty() {
        return assign_tables(chosen, constraints).map(|tables| {
            chosen
                .iter()
                .zip(tables)
                .map(|((a, b), table)| (*a, *b, table))
                .collect()
        });
    }
    if !everyone_has_partner(remaining, constraints) {
        return None;
    }

    let me = remaining[0];
    for index in 1..remaining.len() {
        let opponent = remaining[index];
        if !pair_allowed(me, opponent, constraints) {
            continue;
        }
        let mut next = Vec::with_capacity(remaining.len() - 2);
        next.extend_from_slice(&remaining[1..index]);
        next.extend_from_slice(&remaining[index + 1..]);
        chosen.push((me, opponent));
        if let Some(done) = search(&next, constraints, chosen, nodes, node_limit) {
            return Some(done);
        }
        chosen.pop();
    }
    None
}

fn pair_allowed(a: u32, b: u32, constraints: &Constraints<'_>) -> bool {
    if constraints.forbidden.contains(&pair_key(a, b)) {
        return false;
    }
    if !constraints.allow_rematch
        && constraints
            .opponents
            .get(&a)
            .is_some_and(|played| played.contains(&b))
    {
        return false;
    }
    !candidate_tables(a, b, constraints).is_empty()
}

fn everyone_has_partner(remaining: &[u32], constraints: &Constraints<'_>) -> bool {
    remaining.iter().all(|player| {
        remaining
            .iter()
            .any(|other| *other != *player && pair_allowed(*player, *other, constraints))
    })
}

fn candidate_tables(a: u32, b: u32, constraints: &Constraints<'_>) -> Vec<u16> {
    let mut fresh = Vec::new();
    let mut repeats = Vec::new();
    for table in constraints.table_order {
        let repeat = constraints
            .table_history
            .get(&a)
            .is_some_and(|used| used.contains(table))
            || constraints
                .table_history
                .get(&b)
                .is_some_and(|used| used.contains(table));
        if repeat {
            repeats.push(*table);
        } else {
            fresh.push(*table);
        }
    }
    if constraints.allow_table_repeat {
        fresh.extend(repeats);
    }
    fresh
}

fn assign_tables(pairs: &[(u32, u32)], constraints: &Constraints<'_>) -> Option<Vec<u16>> {
    let mut used = HashSet::new();
    let mut assigned = Vec::with_capacity(pairs.len());
    if assign_from(0, pairs, constraints, &mut used, &mut assigned) {
        Some(assigned)
    } else {
        None
    }
}

fn assign_from(
    index: usize,
    pairs: &[(u32, u32)],
    constraints: &Constraints<'_>,
    used: &mut HashSet<u16>,
    assigned: &mut Vec<u16>,
) -> bool {
    if index == pairs.len() {
        return true;
    }
    let (a, b) = pairs[index];
    for table in candidate_tables(a, b, constraints) {
        if !used.insert(table) {
            continue;
        }
        assigned.push(table);
        if assign_from(index + 1, pairs, constraints, used, assigned) {
            return true;
        }
        assigned.pop();
        used.remove(&table);
    }
    false
}

/// Mélange les joueurs et les pose par paires voisines, en évitant `forbidden`.
pub fn random_pairing<R: Rng>(
    players: &[u32],
    forbidden: &HashSet<(u32, u32)>,
    table_count: u16,
    rng: &mut R,
) -> Option<Vec<(u32, u32, u16)>> {
    if players.len() % 2 != 0 || table_count as usize * 2 < players.len() {
        return None;
    }
    let mut tables: Vec<u16> = (1..=table_count).collect();
    for _ in 0..48 {
        let mut order = players.to_vec();
        order.shuffle(rng);
        if order
            .chunks(2)
            .any(|pair| forbidden.contains(&pair_key(pair[0], pair[1])))
        {
            continue;
        }
        tables.shuffle(rng);
        return Some(
            order
                .chunks(2)
                .enumerate()
                .map(|(index, pair)| (pair[0], pair[1], tables[index]))
                .collect(),
        );
    }

    let mut order = players.to_vec();
    order.shuffle(rng);
    tables.shuffle(rng);
    let history = HashMap::new();
    let opponents = HashMap::new();
    let constraints = Constraints {
        forbidden,
        opponents: &opponents,
        table_history: &history,
        table_order: &tables,
        allow_rematch: true,
        allow_table_repeat: true,
    };
    find_pairing(&order, &constraints, 250_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avoids_a_forbidden_pair_when_another_matching_exists() {
        let players = [1, 2, 3, 4];
        let forbidden = HashSet::from([pair_key(1, 2)]);
        let tables = [1, 2];
        let constraints = Constraints {
            forbidden: &forbidden,
            opponents: &HashMap::new(),
            table_history: &HashMap::new(),
            table_order: &tables,
            allow_rematch: false,
            allow_table_repeat: false,
        };
        let pairs = find_pairing(&players, &constraints, 10_000).unwrap();
        assert!(pairs.iter().all(|(a, b, _)| pair_key(*a, *b) != (1, 2)));
        assert_eq!(pairs.len(), 2);
    }

    #[test]
    fn refuses_when_every_matching_is_forbidden() {
        let players = [1, 2];
        let forbidden = HashSet::from([pair_key(1, 2)]);
        let tables = [1];
        let constraints = Constraints {
            forbidden: &forbidden,
            opponents: &HashMap::new(),
            table_history: &HashMap::new(),
            table_order: &tables,
            allow_rematch: true,
            allow_table_repeat: true,
        };
        assert!(find_pairing(&players, &constraints, 1000).is_none());
    }
}

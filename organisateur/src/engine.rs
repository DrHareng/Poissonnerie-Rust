use std::collections::{HashMap, HashSet};

use rand::rngs::StdRng;
use rand::SeedableRng;

use crate::model::{
    default_round_minutes, Action, ByeOption, GameResult, Match, MatchCard, Player, PlayerCard,
    RoundCard, StandingRow, Status, Tournament, View,
};
use crate::pairing::{self, Constraints};
use crate::scoring::{self, cmp_scaled, match_points, BYE_TP, FORFEIT_WIN_TP};
use crate::sectorials;

const WARN_TABLE: &str = "Certaines tables sont rejouées pour éviter une revanche.";
const WARN_REMATCH: &str = "Certaines revanches sont inévitables.";
const NODE_LIMIT: u32 = 400_000;

struct Tally {
    tp: u32,
    op: u32,
    vp: u32,
    played: u32,
    wins: u32,
    draws: u32,
    losses: u32,
    byes: u32,
}

impl Tally {
    fn new() -> Self {
        Self {
            tp: 0,
            op: 0,
            vp: 0,
            played: 0,
            wins: 0,
            draws: 0,
            losses: 0,
            byes: 0,
        }
    }
}

struct Plan {
    bye: Option<u32>,
    pairs: Vec<(u32, u32, u16)>,
    warnings: Vec<String>,
}

pub fn apply(current: Option<Tournament>, action: Action) -> Result<Tournament, String> {
    match action {
        Action::Create {
            id,
            name,
            rounds,
            tables,
        } => create(id, name, rounds, tables),
        other => {
            let mut tournament = current.ok_or_else(|| "Aucun tournoi.".to_string())?;
            dispatch(&mut tournament, other)?;
            Ok(tournament)
        }
    }
}

pub fn inspect(tournament: &Tournament) -> View {
    let enrolled = tournament.players.len();
    let min_tables = (enrolled / 2) as u16;
    let start_reason = if tournament.status == Status::Preparation {
        start_block(tournament)
    } else {
        None
    };
    let active = active_ids(tournament);
    let bye_required = tournament.status == Status::Pairing && active.len() % 2 == 1;
    View {
        min_tables,
        can_start: tournament.status == Status::Preparation && start_reason.is_none(),
        start_reason,
        can_validate_pairing: tournament.status == Status::Pairing
            && pairing_complete(tournament).is_ok(),
        can_validate_round: tournament.status == Status::InProgress
            && round_is_complete(tournament),
        bye_required,
        bye_options: bye_options(tournament, bye_required),
        recall: tournament
            .players
            .iter()
            .filter(|player| can_undo_drop(tournament, player))
            .map(|player| player_card(tournament, player.id))
            .collect(),
        rounds: round_cards(tournament),
        standings: standings(tournament),
        warnings: tournament.warnings.clone(),
        final_ranking: tournament.status == Status::Finished,
    }
}

fn dispatch(tournament: &mut Tournament, action: Action) -> Result<(), String> {
    match action {
        Action::Create { .. } => Err("Action interne.".into()),
        Action::UpdateMeta {
            name,
            rounds,
            tables,
            round_minutes,
        } => update_meta(tournament, name, rounds, tables, round_minutes),
        Action::SetRoundMinutes { minutes } => set_round_minutes(tournament, minutes),
        Action::AddPlayer { name, sectorial_id } => add_player(tournament, name, sectorial_id),
        Action::UpdatePlayer {
            player_id,
            name,
            sectorial_id,
        } => update_player(tournament, player_id, name, sectorial_id),
        Action::RemovePlayer { player_id } => remove_player(tournament, player_id),
        Action::Start { seed } => start(tournament, &seed),
        Action::SetBye { player_id, seed } => set_bye(tournament, player_id, &seed),
        Action::SetFlag {
            player_a,
            player_b,
            flagged,
        } => set_flag(tournament, player_a, player_b, flagged),
        Action::Regenerate { seed } => regenerate(tournament, &seed),
        Action::ReturnToPlayers => return_to_players(tournament),
        Action::ValidatePairing => validate_pairing(tournament),
        Action::SetResult {
            player1,
            p1_op,
            p1_vp,
            p2_op,
            p2_vp,
        } => set_result(tournament, player1, p1_op, p1_vp, p2_op, p2_vp),
        Action::DropPlayer { player_id, seed } => drop_player(tournament, player_id, &seed),
        Action::UndoDrop { player_id, seed } => undo_drop(tournament, player_id, &seed),
        Action::ValidateRound { seed } => validate_round(tournament, &seed),
    }
}

fn create(id: String, name: String, rounds: u8, tables: u16) -> Result<Tournament, String> {
    let id = id.trim().to_string();
    if id.is_empty() {
        return Err("Identifiant manquant.".into());
    }
    let (name, rounds, tables) = clean_meta(&name, rounds, tables)?;
    Ok(Tournament {
        id,
        name,
        rounds,
        tables,
        round_minutes: default_round_minutes(),
        status: Status::Preparation,
        round: 0,
        players: Vec::new(),
        matches: Vec::new(),
        flagged: Vec::new(),
        pending_bye: None,
        warnings: Vec::new(),
    })
}

fn update_meta(
    tournament: &mut Tournament,
    name: String,
    rounds: u8,
    tables: u16,
    round_minutes: u16,
) -> Result<(), String> {
    ensure_preparation(tournament)?;
    let (name, rounds, tables) = clean_meta(&name, rounds, tables)?;
    tournament.name = name;
    tournament.rounds = rounds;
    tournament.tables = tables;
    tournament.round_minutes = clean_round_minutes(round_minutes)?;
    Ok(())
}

fn set_round_minutes(tournament: &mut Tournament, minutes: u16) -> Result<(), String> {
    if tournament.status == Status::Finished {
        return Err("Le tournoi est terminé.".into());
    }
    tournament.round_minutes = clean_round_minutes(minutes)?;
    Ok(())
}

fn add_player(tournament: &mut Tournament, name: String, sectorial_id: u32) -> Result<(), String> {
    ensure_preparation(tournament)?;
    if tournament.players.len() >= 64 {
        return Err("64 inscrits maximum.".into());
    }
    let name = clean_player_name(&name)?;
    ensure_unique_name(tournament, &name, None)?;
    ensure_sectorial(sectorial_id)?;
    let id = tournament
        .players
        .iter()
        .map(|player| player.id)
        .max()
        .unwrap_or(0)
        + 1;
    let order = tournament
        .players
        .iter()
        .map(|player| player.order)
        .max()
        .unwrap_or(0)
        + 1;
    tournament.players.push(Player {
        id,
        name,
        sectorial_id,
        order,
        dropped: false,
        dropped_round: None,
        drop_before_pairing_validated: false,
    });
    Ok(())
}

fn update_player(
    tournament: &mut Tournament,
    player_id: u32,
    name: String,
    sectorial_id: u32,
) -> Result<(), String> {
    ensure_preparation(tournament)?;
    let name = clean_player_name(&name)?;
    ensure_unique_name(tournament, &name, Some(player_id))?;
    ensure_sectorial(sectorial_id)?;
    let player = player_mut(tournament, player_id)?;
    player.name = name;
    player.sectorial_id = sectorial_id;
    Ok(())
}

fn remove_player(tournament: &mut Tournament, player_id: u32) -> Result<(), String> {
    ensure_preparation(tournament)?;
    let before = tournament.players.len();
    tournament.players.retain(|player| player.id != player_id);
    if tournament.players.len() == before {
        return Err("Joueur introuvable.".into());
    }
    Ok(())
}

fn start(tournament: &mut Tournament, seed: &str) -> Result<(), String> {
    ensure_preparation(tournament)?;
    if let Some(reason) = start_block(tournament) {
        return Err(reason);
    }
    let snapshot = tournament.clone();
    tournament.status = Status::Pairing;
    tournament.round = 1;
    tournament.flagged.clear();
    tournament.pending_bye = None;
    tournament.warnings.clear();
    if let Err(error) = regenerate(tournament, seed) {
        *tournament = snapshot;
        return Err(error);
    }
    Ok(())
}

fn set_bye(tournament: &mut Tournament, player_id: u32, seed: &str) -> Result<(), String> {
    if tournament.status != Status::Pairing {
        return Err("Le bye se choisit pendant l'appariement.".into());
    }
    let active = active_ids(tournament);
    if active.len() % 2 == 0 {
        return Err("Le nombre de joueurs est pair : pas de bye.".into());
    }
    if !active.contains(&player_id) || had_bye(tournament, player_id) {
        return Err("Ce joueur ne peut pas recevoir le bye.".into());
    }
    let previous = tournament.pending_bye;
    tournament.pending_bye = Some(player_id);
    if let Err(error) = regenerate(tournament, seed) {
        tournament.pending_bye = previous;
        return Err(error);
    }
    Ok(())
}

fn set_flag(
    tournament: &mut Tournament,
    player_a: u32,
    player_b: u32,
    flagged: bool,
) -> Result<(), String> {
    if tournament.status != Status::Pairing {
        return Err("Le marquage se fait pendant l'appariement.".into());
    }
    if player_a == player_b {
        return Err("Paire invalide.".into());
    }
    let key = pairing::pair_key(player_a, player_b);
    let listed = tournament.matches.iter().any(|game| {
        game.round == tournament.round
            && game
                .player2
                .is_some_and(|player2| pairing::pair_key(game.player1, player2) == key)
    });
    if !listed {
        return Err("Cette paire n'est pas dans l'appariement.".into());
    }
    tournament.flagged.retain(|pair| *pair != key);
    if flagged {
        tournament.flagged.push(key);
    }
    Ok(())
}

fn regenerate(tournament: &mut Tournament, seed: &str) -> Result<(), String> {
    if tournament.status != Status::Pairing {
        return Err("Il n'y a pas d'appariement à relancer.".into());
    }
    let plan = plan_round(tournament, parse_seed(seed)?)?;
    apply_plan(tournament, plan);
    Ok(())
}

fn return_to_players(tournament: &mut Tournament) -> Result<(), String> {
    if tournament.status != Status::Pairing || tournament.round != 1 {
        return Err("La saisie des joueurs est close.".into());
    }
    tournament.matches.clear();
    tournament.flagged.clear();
    tournament.pending_bye = None;
    tournament.warnings.clear();
    tournament.round = 0;
    tournament.status = Status::Preparation;
    for player in &mut tournament.players {
        if player.drop_before_pairing_validated {
            player.dropped = false;
            player.dropped_round = None;
            player.drop_before_pairing_validated = false;
        }
    }
    Ok(())
}

fn validate_pairing(tournament: &mut Tournament) -> Result<(), String> {
    if tournament.status != Status::Pairing {
        return Err("Il n'y a pas d'appariement à valider.".into());
    }
    pairing_complete(tournament)?;
    tournament.status = Status::InProgress;
    tournament.flagged.clear();
    tournament.warnings.clear();
    Ok(())
}

fn set_result(
    tournament: &mut Tournament,
    player1: u32,
    p1_op: u8,
    p1_vp: u16,
    p2_op: u8,
    p2_vp: u16,
) -> Result<(), String> {
    if tournament.status != Status::InProgress {
        return Err("Les résultats se saisissent pendant la ronde.".into());
    }
    if p1_op > scoring::MAX_OP || p2_op > scoring::MAX_OP {
        return Err("Les OP doivent être entre 0 et 10.".into());
    }
    if p1_vp > scoring::MAX_VP || p2_vp > scoring::MAX_VP {
        return Err("Les VP doivent être entre 0 et 300.".into());
    }
    let round = tournament.round;
    let game = tournament
        .matches
        .iter_mut()
        .find(|game| game.round == round && game.player1 == player1 && game.player2.is_some())
        .ok_or_else(|| "Match introuvable.".to_string())?;
    if game
        .result
        .as_ref()
        .and_then(|result| result.forfeit_player)
        .is_some()
    {
        return Err("Annulez le forfait pour saisir un résultat.".into());
    }
    game.result = Some(GameResult {
        p1_op,
        p1_vp,
        p2_op,
        p2_vp,
        forfeit_player: None,
    });
    Ok(())
}

fn drop_player(tournament: &mut Tournament, player_id: u32, seed: &str) -> Result<(), String> {
    match tournament.status {
        Status::Preparation => {
            return Err(
                "Retirez l'inscrit : le forfait se déclare une fois le tournoi démarré.".into(),
            );
        }
        Status::Finished => return Err("Le tournoi est terminé.".into()),
        Status::Pairing | Status::InProgress => {}
    }
    let index = player_index(tournament, player_id)?;
    if tournament.players[index].dropped {
        return Err("Ce joueur est déjà forfait.".into());
    }

    if tournament.status == Status::Pairing {
        let snapshot = tournament.clone();
        let player = &mut tournament.players[index];
        player.dropped = true;
        player.dropped_round = Some(tournament.round);
        player.drop_before_pairing_validated = true;
        if tournament.pending_bye == Some(player_id) {
            tournament.pending_bye = None;
        }
        if let Err(error) = regenerate(tournament, seed) {
            *tournament = snapshot;
            return Err(error);
        }
        return Ok(());
    }

    let player = &mut tournament.players[index];
    player.dropped = true;
    player.dropped_round = Some(tournament.round);
    player.drop_before_pairing_validated = false;
    let round = tournament.round;
    if let Some(game) = tournament
        .matches
        .iter_mut()
        .find(|game| game.round == round && involves(game, player_id) && game.player2.is_some())
    {
        game.result = Some(GameResult {
            p1_op: 0,
            p1_vp: 0,
            p2_op: 0,
            p2_vp: 0,
            forfeit_player: Some(player_id),
        });
    }
    Ok(())
}

fn undo_drop(tournament: &mut Tournament, player_id: u32, seed: &str) -> Result<(), String> {
    let index = player_index(tournament, player_id)?;
    if !can_undo_drop(tournament, &tournament.players[index]) {
        return Err("Ce forfait est déjà pris en compte.".into());
    }
    if tournament.status == Status::Pairing {
        let snapshot = tournament.clone();
        let player = &mut tournament.players[index];
        player.dropped = false;
        player.dropped_round = None;
        player.drop_before_pairing_validated = false;
        if let Err(error) = regenerate(tournament, seed) {
            *tournament = snapshot;
            return Err(error);
        }
        return Ok(());
    }

    let player = &mut tournament.players[index];
    player.dropped = false;
    player.dropped_round = None;
    player.drop_before_pairing_validated = false;
    let round = tournament.round;
    if let Some(game) = tournament
        .matches
        .iter_mut()
        .find(|game| game.round == round && involves(game, player_id))
    {
        if game
            .result
            .as_ref()
            .and_then(|result| result.forfeit_player)
            == Some(player_id)
        {
            game.result = None;
        }
    }
    Ok(())
}

fn validate_round(tournament: &mut Tournament, seed: &str) -> Result<(), String> {
    if tournament.status != Status::InProgress {
        return Err("Il n'y a pas de ronde à valider.".into());
    }
    if !round_is_complete(tournament) {
        return Err("Il reste des matchs sans résultat.".into());
    }
    if tournament.round >= tournament.rounds {
        tournament.status = Status::Finished;
        tournament.flagged.clear();
        tournament.pending_bye = None;
        tournament.warnings.clear();
        return Ok(());
    }

    let snapshot = tournament.clone();
    tournament.round += 1;
    tournament.status = Status::Pairing;
    tournament.flagged.clear();
    tournament.pending_bye = None;
    tournament.warnings.clear();
    if let Err(error) = regenerate(tournament, seed) {
        *tournament = snapshot;
        return Err(error);
    }
    Ok(())
}

fn plan_round(tournament: &Tournament, seed: u64) -> Result<Plan, String> {
    let mut pool = active_ids(tournament);
    if pool.is_empty() {
        return Err("Plus aucun joueur en lice.".into());
    }
    let bye = if pool.len() % 2 == 1 {
        Some(choose_bye(tournament, &pool)?)
    } else {
        None
    };
    if let Some(bye_id) = bye {
        pool.retain(|player| *player != bye_id);
    }
    if pool.is_empty() {
        return Ok(Plan {
            bye,
            pairs: Vec::new(),
            warnings: Vec::new(),
        });
    }
    let needed = pool.len() / 2;
    if needed > tournament.tables as usize {
        return Err(format!(
            "Pas assez de tables ({}) pour {needed} matchs.",
            tournament.tables
        ));
    }

    let forbidden: HashSet<(u32, u32)> = tournament.flagged.iter().copied().collect();
    let (opponents, table_history) = history(tournament);
    let mut rng = StdRng::seed_from_u64(seed);
    let (pairs, warnings) = if tournament.round <= 1 {
        let pairs = pairing::random_pairing(&pool, &forbidden, tournament.tables, &mut rng)
            .ok_or_else(|| {
                if forbidden.is_empty() {
                    "Impossible de construire l'appariement.".to_string()
                } else {
                    "Impossible d'éviter les appariements marqués.".to_string()
                }
            })?;
        (pairs, Vec::new())
    } else {
        plan_scored(tournament, &pool, &forbidden, &opponents, &table_history)?
    };
    Ok(Plan {
        bye,
        pairs,
        warnings,
    })
}

fn plan_scored(
    tournament: &Tournament,
    pool: &[u32],
    forbidden: &HashSet<(u32, u32)>,
    opponents: &HashMap<u32, HashSet<u32>>,
    table_history: &HashMap<u32, HashSet<u16>>,
) -> Result<(Vec<(u32, u32, u16)>, Vec<String>), String> {
    let mut preference = pool.to_vec();
    preference.sort_by(|left, right| cmp_best(tournament, *left, *right));
    let table_order: Vec<u16> = (1..=tournament.tables).collect();

    let try_pass = |allow_rematch: bool, allow_table_repeat: bool| {
        let constraints = Constraints {
            forbidden,
            opponents,
            table_history,
            table_order: &table_order,
            allow_rematch,
            allow_table_repeat,
        };
        pairing::find_pairing(&preference, &constraints, NODE_LIMIT)
    };

    if let Some(pairs) = try_pass(false, false) {
        return Ok((pairs, Vec::new()));
    }
    if let Some(pairs) = try_pass(false, true) {
        return Ok((pairs, vec![WARN_TABLE.into()]));
    }
    if !forbidden.is_empty() {
        return Err("Impossible d'éviter les appariements marqués.".into());
    }
    if let Some(pairs) = try_pass(true, true) {
        return Ok((pairs, vec![WARN_REMATCH.into()]));
    }
    Err("Impossible de construire l'appariement.".into())
}

fn apply_plan(tournament: &mut Tournament, plan: Plan) {
    let round = tournament.round;
    tournament.matches.retain(|game| game.round != round);
    if let Some(bye) = plan.bye {
        tournament.matches.push(Match {
            round,
            player1: bye,
            player2: None,
            table: None,
            result: None,
        });
    }
    for (player1, player2, table) in plan.pairs {
        tournament.matches.push(Match {
            round,
            player1,
            player2: Some(player2),
            table: Some(table),
            result: None,
        });
    }
    tournament.pending_bye = plan.bye;
    tournament.warnings = plan.warnings;
}

fn choose_bye(tournament: &Tournament, pool: &[u32]) -> Result<u32, String> {
    let mut eligible: Vec<u32> = pool
        .iter()
        .copied()
        .filter(|player| !had_bye(tournament, *player))
        .collect();
    if eligible.is_empty() {
        return Err(
            "Aucun joueur ne peut encore recevoir le bye. Un forfait peut rendre le tableau pair."
                .into(),
        );
    }
    if let Some(pending) = tournament.pending_bye {
        if eligible.contains(&pending) {
            return Ok(pending);
        }
    }
    eligible.sort_by(|left, right| cmp_worst(tournament, *left, *right));
    Ok(eligible[0])
}

fn history(tournament: &Tournament) -> (HashMap<u32, HashSet<u32>>, HashMap<u32, HashSet<u16>>) {
    let mut opponents: HashMap<u32, HashSet<u32>> = HashMap::new();
    let mut tables: HashMap<u32, HashSet<u16>> = HashMap::new();
    for game in &tournament.matches {
        if game.round >= tournament.round {
            continue;
        }
        let Some(player2) = game.player2 else {
            continue;
        };
        opponents.entry(game.player1).or_default().insert(player2);
        opponents.entry(player2).or_default().insert(game.player1);
        if let Some(table) = game.table {
            tables.entry(game.player1).or_default().insert(table);
            tables.entry(player2).or_default().insert(table);
        }
    }
    (opponents, tables)
}

fn tally(tournament: &Tournament, player_id: u32, previous_only: bool) -> Tally {
    let mut tally = Tally::new();
    for game in &tournament.matches {
        if previous_only && game.round >= tournament.round {
            continue;
        }
        if !previous_only && tournament.status == Status::Pairing && game.round == tournament.round
        {
            continue;
        }
        if !involves(game, player_id) {
            continue;
        }
        if game.is_bye() {
            tally.byes += 1;
            tally.tp += BYE_TP;
            continue;
        }
        let Some(result) = game.result.as_ref() else {
            continue;
        };
        tally.played += 1;
        let mine = game.player1 == player_id;
        if let Some(forfeit) = result.forfeit_player {
            if forfeit == player_id {
                tally.losses += 1;
            } else {
                tally.wins += 1;
                tally.tp += FORFEIT_WIN_TP;
            }
            continue;
        }
        let (my_op, opp_op, my_vp) = if mine {
            (result.p1_op, result.p2_op, result.p1_vp)
        } else {
            (result.p2_op, result.p1_op, result.p2_vp)
        };
        tally.tp += match_points(my_op, opp_op);
        tally.op += u32::from(my_op);
        tally.vp += u32::from(my_vp);
        if my_op > opp_op {
            tally.wins += 1;
        } else if my_op < opp_op {
            tally.losses += 1;
        } else {
            tally.draws += 1;
        }
    }
    tally
}

fn standings(tournament: &Tournament) -> Vec<StandingRow> {
    let mut rows: Vec<StandingRow> = tournament
        .players
        .iter()
        .map(|player| {
            let tally = tally(tournament, player.id, false);
            let sectorial = sectorial_label(player.sectorial_id);
            StandingRow {
                rank: 0,
                player_id: player.id,
                name: player.name.clone(),
                sectorial_name: sectorial.0,
                short_name: sectorial.1,
                tp: tally.tp,
                op: tally.op,
                vp: tally.vp,
                rounds_played: tally.played,
                op_final: scoring::scaled(tally.op, tournament.rounds, tally.played),
                vp_final: scoring::scaled(tally.vp, tournament.rounds, tally.played),
                wins: tally.wins,
                draws: tally.draws,
                losses: tally.losses,
                byes: tally.byes,
                dropped: player.dropped,
            }
        })
        .collect();
    let final_ranking = tournament.status == Status::Finished;
    rows.sort_by(|left, right| {
        let tp = right.tp.cmp(&left.tp);
        if tp != std::cmp::Ordering::Equal {
            return tp;
        }
        let objectives = if final_ranking {
            cmp_scaled(right.op, right.rounds_played, left.op, left.rounds_played)
        } else {
            right.op.cmp(&left.op)
        };
        if objectives != std::cmp::Ordering::Equal {
            return objectives;
        }
        let survivors = if final_ranking {
            cmp_scaled(right.vp, right.rounds_played, left.vp, left.rounds_played)
        } else {
            right.vp.cmp(&left.vp)
        };
        survivors.then_with(|| left.name.cmp(&right.name))
    });
    for (index, row) in rows.iter_mut().enumerate() {
        row.rank = index as u32 + 1;
    }
    rows
}

fn round_cards(tournament: &Tournament) -> Vec<RoundCard> {
    let mut numbers: Vec<u8> = tournament.matches.iter().map(|game| game.round).collect();
    numbers.sort_unstable();
    numbers.dedup();
    numbers
        .into_iter()
        .map(|round| {
            let mut matches: Vec<MatchCard> = tournament
                .matches
                .iter()
                .filter(|game| game.round == round)
                .map(|game| match_card(tournament, game))
                .collect();
            matches.sort_by(|left, right| {
                left.bye
                    .cmp(&right.bye)
                    .reverse()
                    .then(left.table.cmp(&right.table))
            });
            RoundCard {
                round,
                provisional: tournament.status == Status::Pairing && round == tournament.round,
                matches,
            }
        })
        .collect()
}

fn match_card(tournament: &Tournament, game: &Match) -> MatchCard {
    let player1 = player_card(tournament, game.player1);
    let player2 = game.player2.map(|id| player_card(tournament, id));
    let flagged = game.player2.is_some_and(|player2| {
        tournament
            .flagged
            .contains(&pairing::pair_key(game.player1, player2))
    });
    if game.is_bye() {
        return MatchCard {
            round: game.round,
            table: None,
            bye: true,
            player1,
            player2: None,
            p1_op: Some(0),
            p1_vp: Some(0),
            p1_tp: Some(BYE_TP),
            p2_op: None,
            p2_vp: None,
            p2_tp: None,
            forfeit_player: None,
            scored: true,
            flagged: false,
        };
    }
    let (p1_op, p1_vp, p1_tp, p2_op, p2_vp, p2_tp, forfeit_player) =
        if let Some(result) = &game.result {
            if let Some(forfeit) = result.forfeit_player {
                let p1_forfeit = forfeit == game.player1;
                (
                    Some(0),
                    Some(0),
                    Some(if p1_forfeit { 0 } else { FORFEIT_WIN_TP }),
                    Some(0),
                    Some(0),
                    Some(if p1_forfeit { FORFEIT_WIN_TP } else { 0 }),
                    Some(forfeit),
                )
            } else {
                (
                    Some(result.p1_op),
                    Some(result.p1_vp),
                    Some(match_points(result.p1_op, result.p2_op)),
                    Some(result.p2_op),
                    Some(result.p2_vp),
                    Some(match_points(result.p2_op, result.p1_op)),
                    None,
                )
            }
        } else {
            (None, None, None, None, None, None, None)
        };
    MatchCard {
        round: game.round,
        table: game.table,
        bye: false,
        player1,
        player2,
        p1_op,
        p1_vp,
        p1_tp,
        p2_op,
        p2_vp,
        p2_tp,
        forfeit_player,
        scored: game.result.is_some(),
        flagged,
    }
}

fn bye_options(tournament: &Tournament, bye_required: bool) -> Vec<ByeOption> {
    if !bye_required {
        return Vec::new();
    }
    let mut options: Vec<ByeOption> = tournament
        .players
        .iter()
        .filter(|player| !player.dropped && !had_bye(tournament, player.id))
        .map(|player| {
            let sectorial = sectorial_label(player.sectorial_id);
            ByeOption {
                player_id: player.id,
                name: player.name.clone(),
                short_name: sectorial.1,
                selected: tournament.pending_bye == Some(player.id),
            }
        })
        .collect();
    options.sort_by(|left, right| cmp_worst(tournament, left.player_id, right.player_id));
    options
}

fn player_card(tournament: &Tournament, player_id: u32) -> PlayerCard {
    let player = tournament
        .players
        .iter()
        .find(|player| player.id == player_id);
    let Some(player) = player else {
        return PlayerCard {
            id: player_id,
            name: format!("Joueur {player_id}"),
            sectorial_id: 0,
            sectorial_name: String::new(),
            short_name: String::new(),
            dropped: false,
            can_undo_drop: false,
        };
    };
    let sectorial = sectorial_label(player.sectorial_id);
    PlayerCard {
        id: player.id,
        name: player.name.clone(),
        sectorial_id: player.sectorial_id,
        sectorial_name: sectorial.0,
        short_name: sectorial.1,
        dropped: player.dropped,
        can_undo_drop: can_undo_drop(tournament, player),
    }
}

fn can_undo_drop(tournament: &Tournament, player: &Player) -> bool {
    if !player.dropped || player.dropped_round != Some(tournament.round) {
        return false;
    }
    match tournament.status {
        Status::Pairing => player.drop_before_pairing_validated,
        Status::InProgress => !player.drop_before_pairing_validated,
        Status::Preparation | Status::Finished => false,
    }
}

fn sectorial_label(id: u32) -> (String, String) {
    match sectorials::by_id(id) {
        Some(sectorial) => (sectorial.name.clone(), sectorial.short_name.clone()),
        None => (format!("Sectorielle {id}"), format!("{id}")),
    }
}

fn had_bye(tournament: &Tournament, player_id: u32) -> bool {
    tournament
        .matches
        .iter()
        .any(|game| game.is_bye() && game.player1 == player_id && game.round < tournament.round)
}

fn cmp_best(tournament: &Tournament, left: u32, right: u32) -> std::cmp::Ordering {
    let a = tally(tournament, left, true);
    let b = tally(tournament, right, true);
    b.tp.cmp(&a.tp)
        .then(b.op.cmp(&a.op))
        .then(b.vp.cmp(&a.vp))
        .then_with(|| order_of(tournament, left).cmp(&order_of(tournament, right)))
}

fn cmp_worst(tournament: &Tournament, left: u32, right: u32) -> std::cmp::Ordering {
    cmp_best(tournament, right, left)
}

fn order_of(tournament: &Tournament, player_id: u32) -> u32 {
    tournament
        .players
        .iter()
        .find(|player| player.id == player_id)
        .map(|player| player.order)
        .unwrap_or(u32::MAX)
}

fn active_ids(tournament: &Tournament) -> Vec<u32> {
    let mut ids: Vec<u32> = tournament
        .players
        .iter()
        .filter(|player| !player.dropped)
        .map(|player| player.id)
        .collect();
    ids.sort_by_key(|id| order_of(tournament, *id));
    ids
}

fn involves(game: &Match, player_id: u32) -> bool {
    game.player1 == player_id || game.player2 == Some(player_id)
}

fn pairing_complete(tournament: &Tournament) -> Result<(), String> {
    let active: HashSet<u32> = active_ids(tournament).into_iter().collect();
    let mut seen = HashSet::new();
    let mut tables = HashSet::new();
    for game in tournament
        .matches
        .iter()
        .filter(|game| game.round == tournament.round)
    {
        if !active.contains(&game.player1) || !seen.insert(game.player1) {
            return Err("L'appariement est incomplet.".into());
        }
        if let Some(player2) = game.player2 {
            if !active.contains(&player2) || !seen.insert(player2) {
                return Err("L'appariement est incomplet.".into());
            }
            let table = game.table.ok_or_else(|| "Table manquante.".to_string())?;
            if table == 0 || table > tournament.tables || !tables.insert(table) {
                return Err("Attribution des tables invalide.".into());
            }
        } else if game.table.is_some() {
            return Err("Le bye n'occupe pas de table.".into());
        }
    }
    if seen != active {
        return Err("L'appariement est incomplet.".into());
    }
    Ok(())
}

fn round_is_complete(tournament: &Tournament) -> bool {
    let games: Vec<&Match> = tournament
        .matches
        .iter()
        .filter(|game| game.round == tournament.round)
        .collect();
    !games.is_empty() && games.iter().all(|game| game.is_scored())
}

fn start_block(tournament: &Tournament) -> Option<String> {
    if tournament.players.len() < 2 {
        return Some("Ajoutez au moins 2 inscrits.".into());
    }
    let min = (tournament.players.len() / 2) as u16;
    if tournament.tables < min {
        return Some(format!(
            "Il faut au moins {min} tables pour {} inscrits.",
            tournament.players.len()
        ));
    }
    None
}

fn ensure_preparation(tournament: &Tournament) -> Result<(), String> {
    if tournament.status == Status::Preparation {
        Ok(())
    } else {
        Err("Le tournoi est déjà démarré.".into())
    }
}

fn clean_meta(name: &str, rounds: u8, tables: u16) -> Result<(String, u8, u16), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Le nom du tournoi est vide.".into());
    }
    if name.chars().count() > 80 {
        return Err("Le nom du tournoi est trop long.".into());
    }
    if !(1..=12).contains(&rounds) {
        return Err("Le nombre de rondes doit être entre 1 et 12.".into());
    }
    if !(1..=40).contains(&tables) {
        return Err("Le nombre de tables doit être entre 1 et 40.".into());
    }
    Ok((name.to_string(), rounds, tables))
}

fn clean_round_minutes(minutes: u16) -> Result<u16, String> {
    if (1..=12 * 60).contains(&minutes) {
        Ok(minutes)
    } else {
        Err("La durée d'une ronde doit être entre 1 minute et 12 heures.".into())
    }
}

fn clean_player_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Le nom est vide.".into());
    }
    if name.chars().count() > 40 {
        return Err("Le nom est trop long (40 caractères maximum).".into());
    }
    Ok(name.to_string())
}

fn ensure_unique_name(
    tournament: &Tournament,
    name: &str,
    except: Option<u32>,
) -> Result<(), String> {
    let key = name.to_lowercase();
    if tournament
        .players
        .iter()
        .any(|player| except != Some(player.id) && player.name.to_lowercase() == key)
    {
        return Err("Ce nom est déjà inscrit.".into());
    }
    Ok(())
}

fn ensure_sectorial(id: u32) -> Result<(), String> {
    if sectorials::by_id(id).is_some() {
        Ok(())
    } else {
        Err("Sectorielle inconnue.".into())
    }
}

fn player_index(tournament: &Tournament, player_id: u32) -> Result<usize, String> {
    tournament
        .players
        .iter()
        .position(|player| player.id == player_id)
        .ok_or_else(|| "Joueur introuvable.".to_string())
}

fn player_mut(tournament: &mut Tournament, player_id: u32) -> Result<&mut Player, String> {
    tournament
        .players
        .iter_mut()
        .find(|player| player.id == player_id)
        .ok_or_else(|| "Joueur introuvable.".to_string())
}

fn parse_seed(seed: &str) -> Result<u64, String> {
    seed.parse::<u64>()
        .map_err(|_| "Graine de tirage invalide.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn created() -> Tournament {
        apply(
            None,
            Action::Create {
                id: "t1".into(),
                name: "Coupe".into(),
                rounds: 3,
                tables: 2,
            },
        )
        .unwrap()
    }

    fn enroll(tournament: Tournament, name: &str) -> Tournament {
        apply(
            Some(tournament),
            Action::AddPlayer {
                name: name.into(),
                sectorial_id: 101,
            },
        )
        .unwrap()
    }

    fn score(
        tournament: Tournament,
        name_a: &str,
        op_a: u8,
        vp_a: u16,
        name_b: &str,
        op_b: u8,
        vp_b: u16,
    ) -> Tournament {
        let id = |name: &str| {
            tournament
                .players
                .iter()
                .find(|player| player.name == name)
                .unwrap()
                .id
        };
        let a = id(name_a);
        let b = id(name_b);
        let game = tournament
            .matches
            .iter()
            .find(|game| {
                game.round == tournament.round
                    && game.player2.is_some()
                    && involves(game, a)
                    && involves(game, b)
            })
            .unwrap();
        let seat1 = game.player1;
        let (p1_op, p1_vp, p2_op, p2_vp) = if seat1 == a {
            (op_a, vp_a, op_b, vp_b)
        } else {
            (op_b, vp_b, op_a, vp_a)
        };
        apply(
            Some(tournament),
            Action::SetResult {
                player1: seat1,
                p1_op,
                p1_vp,
                p2_op,
                p2_vp,
            },
        )
        .unwrap()
    }

    #[test]
    fn refuses_to_start_without_enough_tables() {
        let mut tournament = created();
        for name in ["A", "B", "C", "D"] {
            tournament = enroll(tournament, name);
        }
        tournament = apply(
            Some(tournament),
            Action::UpdateMeta {
                name: "Coupe".into(),
                rounds: 3,
                tables: 1,
                round_minutes: 150,
            },
        )
        .unwrap();
        let error = apply(Some(tournament), Action::Start { seed: "1".into() }).unwrap_err();
        assert!(error.contains("2 tables"));
    }

    #[test]
    fn full_swiss_with_byes_scales_the_final_tiebreak() {
        let mut tournament = apply(
            None,
            Action::Create {
                id: "t".into(),
                name: "Suisse".into(),
                rounds: 3,
                tables: 1,
            },
        )
        .unwrap();
        for name in ["A", "B", "C"] {
            tournament = enroll(tournament, name);
        }
        tournament = apply(Some(tournament), Action::Start { seed: "7".into() }).unwrap();
        assert_eq!(tournament.pending_bye, Some(3));
        tournament = apply(Some(tournament), Action::ValidatePairing).unwrap();
        tournament = score(tournament, "A", 10, 100, "B", 0, 50);
        tournament = apply(Some(tournament), Action::ValidateRound { seed: "8".into() }).unwrap();
        assert_eq!(tournament.round, 2);
        assert_eq!(tournament.pending_bye, Some(2));

        tournament = apply(Some(tournament), Action::ValidatePairing).unwrap();
        tournament = score(tournament, "A", 5, 10, "C", 5, 10);
        tournament = apply(Some(tournament), Action::ValidateRound { seed: "9".into() }).unwrap();
        assert_eq!(tournament.round, 3);
        assert_eq!(tournament.pending_bye, Some(1));

        tournament = apply(Some(tournament), Action::ValidatePairing).unwrap();
        tournament = score(tournament, "B", 0, 0, "C", 10, 0);
        tournament = apply(
            Some(tournament),
            Action::ValidateRound { seed: "10".into() },
        )
        .unwrap();
        assert_eq!(tournament.status, Status::Finished);

        let view = inspect(&tournament);
        let names: Vec<_> = view.standings.iter().map(|row| row.name.as_str()).collect();
        assert_eq!(names, ["A", "C", "B"]);
        let a = &view.standings[0];
        assert_eq!(a.tp, 12);
        assert_eq!(a.op, 15);
        assert_eq!(a.rounds_played, 2);
        assert_eq!(a.op_final, 22.5);
        assert_eq!(a.vp_final, 165.0);
        assert_eq!(a.byes, 1);
        let c = &view.standings[1];
        assert_eq!(c.tp, 12);
        assert_eq!(c.op_final, 22.5);
        assert_eq!(c.vp_final, 15.0);

        let mut seen = HashSet::new();
        for game in &tournament.matches {
            if let Some(player2) = game.player2 {
                let key = pairing::pair_key(game.player1, player2);
                assert!(seen.insert(key), "revanche");
            }
        }
    }

    #[test]
    fn flagged_pairs_are_kept_out_until_no_matching_remains() {
        let mut tournament = created();
        for name in ["A", "B", "C", "D"] {
            tournament = enroll(tournament, name);
        }
        tournament = apply(Some(tournament), Action::Start { seed: "3".into() }).unwrap();
        let pairs: Vec<(u32, u32)> = tournament
            .matches
            .iter()
            .filter_map(|game| game.player2.map(|player2| (game.player1, player2)))
            .collect();
        for (a, b) in &pairs {
            tournament = apply(
                Some(tournament),
                Action::SetFlag {
                    player_a: *a,
                    player_b: *b,
                    flagged: true,
                },
            )
            .unwrap();
        }
        let blocked = tournament.clone();
        tournament = apply(Some(tournament), Action::Regenerate { seed: "4".into() }).unwrap();
        for game in tournament
            .matches
            .iter()
            .filter(|game| game.player2.is_some())
        {
            let key = pairing::pair_key(game.player1, game.player2.unwrap());
            assert!(!blocked.flagged.contains(&key));
        }

        let mut guarded = tournament.clone();
        loop {
            let current: Vec<(u32, u32)> = guarded
                .matches
                .iter()
                .filter_map(|game| {
                    game.player2
                        .map(|player2| pairing::pair_key(game.player1, player2))
                })
                .collect();
            if current.iter().all(|key| guarded.flagged.contains(key)) {
                break;
            }
            for key in current {
                if !guarded.flagged.contains(&key) {
                    guarded = apply(
                        Some(guarded),
                        Action::SetFlag {
                            player_a: key.0,
                            player_b: key.1,
                            flagged: true,
                        },
                    )
                    .unwrap();
                }
            }
            match apply(
                Some(guarded.clone()),
                Action::Regenerate { seed: "11".into() },
            ) {
                Ok(next) => guarded = next,
                Err(error) => {
                    assert!(error.contains("appariements marqués"));
                    let unchanged = apply(
                        Some(guarded.clone()),
                        Action::Regenerate { seed: "12".into() },
                    )
                    .unwrap_err();
                    assert!(unchanged.contains("appariements marqués"));
                    let still = apply(
                        Some(guarded),
                        Action::SetFlag {
                            player_a: pairs[0].0,
                            player_b: pairs[0].1,
                            flagged: true,
                        },
                    );
                    let _ = still;
                    return;
                }
            }
        }
        panic!("l'appariement aurait dû finir par être impossible");
    }

    #[test]
    fn forfeit_scores_like_the_site_and_locks_after_the_round() {
        let mut tournament = apply(
            None,
            Action::Create {
                id: "f".into(),
                name: "Forfaits".into(),
                rounds: 2,
                tables: 2,
            },
        )
        .unwrap();
        for name in ["A", "B", "C"] {
            tournament = enroll(tournament, name);
        }
        tournament = apply(Some(tournament), Action::Start { seed: "1".into() }).unwrap();
        tournament = apply(Some(tournament), Action::ValidatePairing).unwrap();
        let playing = tournament
            .matches
            .iter()
            .find(|game| game.player2.is_some())
            .unwrap()
            .player1;
        tournament = apply(
            Some(tournament),
            Action::DropPlayer {
                player_id: playing,
                seed: "1".into(),
            },
        )
        .unwrap();
        let view = inspect(&tournament);
        let winner = view.standings.iter().find(|row| row.tp == 4).unwrap();
        assert_eq!(winner.op, 0);
        assert_eq!(winner.wins, 1);
        let loser = view
            .standings
            .iter()
            .find(|row| row.player_id == playing)
            .unwrap();
        assert_eq!(loser.tp, 0);
        assert!(loser.dropped);

        tournament = apply(
            Some(tournament),
            Action::UndoDrop {
                player_id: playing,
                seed: "1".into(),
            },
        )
        .unwrap();
        assert!(tournament
            .matches
            .iter()
            .any(|game| game.player1 == playing && game.result.is_none()
                || game.player2 == Some(playing) && game.result.is_none()));

        tournament = apply(
            Some(tournament),
            Action::DropPlayer {
                player_id: playing,
                seed: "1".into(),
            },
        )
        .unwrap();
        tournament = apply(Some(tournament), Action::ValidateRound { seed: "2".into() }).unwrap();
        let error = apply(
            Some(tournament),
            Action::UndoDrop {
                player_id: playing,
                seed: "2".into(),
            },
        )
        .unwrap_err();
        assert!(error.contains("déjà pris en compte"));
    }

    #[test]
    fn manual_bye_replaces_the_default() {
        let mut tournament = apply(
            None,
            Action::Create {
                id: "b".into(),
                name: "Bye".into(),
                rounds: 1,
                tables: 2,
            },
        )
        .unwrap();
        for name in ["A", "B", "C"] {
            tournament = enroll(tournament, name);
        }
        tournament = apply(Some(tournament), Action::Start { seed: "1".into() }).unwrap();
        assert_eq!(tournament.pending_bye, Some(3));
        tournament = apply(
            Some(tournament),
            Action::SetBye {
                player_id: 1,
                seed: "2".into(),
            },
        )
        .unwrap();
        assert_eq!(tournament.pending_bye, Some(1));
        assert!(tournament
            .matches
            .iter()
            .any(|game| { game.is_bye() && game.player1 == 1 }));
    }

    #[test]
    fn unvalidated_pairing_returns_to_player_entry() {
        let mut tournament = created();
        for name in ["A", "B", "C"] {
            tournament = enroll(tournament, name);
        }
        tournament = apply(Some(tournament), Action::Start { seed: "1".into() }).unwrap();
        let dropped = tournament.players[0].id;
        tournament = apply(
            Some(tournament),
            Action::DropPlayer {
                player_id: dropped,
                seed: "2".into(),
            },
        )
        .unwrap();
        tournament = apply(Some(tournament), Action::ReturnToPlayers).unwrap();
        assert_eq!(tournament.status, Status::Preparation);
        assert_eq!(tournament.round, 0);
        assert!(tournament.matches.is_empty());
        assert!(tournament.players.iter().all(|player| !player.dropped));
        tournament = apply(
            Some(tournament),
            Action::AddPlayer {
                name: "D".into(),
                sectorial_id: 101,
            },
        )
        .unwrap();
        assert_eq!(tournament.players.len(), 4);

        tournament = apply(Some(tournament), Action::Start { seed: "3".into() }).unwrap();
        tournament = apply(Some(tournament), Action::ValidatePairing).unwrap();
        let error = apply(Some(tournament), Action::ReturnToPlayers).unwrap_err();
        assert!(error.contains("close"));
    }

    #[test]
    fn preparation_rejects_edits_after_the_start() {
        let mut tournament = created();
        tournament = enroll(tournament, "A");
        tournament = enroll(tournament, "B");
        tournament = apply(Some(tournament), Action::Start { seed: "1".into() }).unwrap();
        let error = apply(
            Some(tournament),
            Action::AddPlayer {
                name: "C".into(),
                sectorial_id: 101,
            },
        )
        .unwrap_err();
        assert!(error.contains("déjà démarré"));
    }

    #[test]
    fn round_duration_defaults_to_two_and_a_half_hours() {
        let tournament = created();
        assert_eq!(tournament.round_minutes, 150);
        let old = r#"{"id":"old","name":"Ancien","rounds":1,"tables":1,"status":"preparation","round":0,"players":[],"matches":[],"flagged":[],"pending_bye":null,"warnings":[]}"#;
        let restored: Tournament = serde_json::from_str(old).unwrap();
        assert_eq!(restored.round_minutes, 150);

        let mut tournament = created();
        tournament = enroll(tournament, "A");
        tournament = enroll(tournament, "B");
        tournament = apply(
            Some(tournament),
            Action::UpdateMeta {
                name: "Coupe".into(),
                rounds: 3,
                tables: 1,
                round_minutes: 90,
            },
        )
        .unwrap();
        assert_eq!(tournament.round_minutes, 90);
        tournament = apply(Some(tournament), Action::Start { seed: "1".into() }).unwrap();
        tournament = apply(
            Some(tournament),
            Action::SetRoundMinutes { minutes: 120 },
        )
        .unwrap();
        assert_eq!(tournament.round_minutes, 120);
    }
}

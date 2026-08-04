use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::Parser;
use reqwest::blocking::Client;
use rusqlite::{params, Connection};

use crate::army::ArmyStore;
use super::config::CoupeConfig;
use super::sheets::{
    self, BracketMatchLine, ParsedCoupeSheet, PoolMatchLine, match_key, validate_parsed,
};
use crate::player::DEFAULT_RATING;
use crate::store::{Leaderboard, normalize_name};
use crate::tournament::TournamentMatch;
use crate::tournament_store::{
    BracketSlotSetup, CreateTournamentRequest, PoolSetup, SetupBracketRequest, SetupPoolsRequest,
    SubmitMatchRequest, TournamentStore,
};
use crate::{DEFAULT_K_FACTOR, MatchOutcome, MatchRecord, MatchScores, RatingUpdate};

const ADMIN_ID: i64 = 1;

#[derive(Parser)]
#[command(name = "poissonnerie-import-coupe", about = "Import Coupe de la Poissonnerie 5-10")]
pub struct ImportCoupeArgs {
    /// Numéro de coupe (5 à 10).
    #[arg(long)]
    pub coupe: u8,

    #[arg(long, default_value = "data/poissonnerie.db")]
    pub db: PathBuf,

    /// Affiche les données extraites sans écrire en base.
    #[arg(long)]
    pub dry_run: bool,

    /// Supprime le tournoi existant portant le même nom avant import.
    #[arg(long)]
    pub force: bool,

    /// Recalcule les Elo globaux à partir de la table matches.
    #[arg(long)]
    pub rebuild_leaderboard: bool,
}

pub fn run_import(args: &ImportCoupeArgs, config: &CoupeConfig) -> Result<()> {
    if args.rebuild_leaderboard {
        rebuild_leaderboard_from_matches(&args.db)?;
        println!("Classement Elo recalculé depuis la table matches.");
        return Ok(());
    }

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;
    println!(
        "Téléchargement Google Sheet — {}…",
        config.name
    );
    let data = sheets::load_coupe_data(&client, config.spreadsheet_id)?;
    validate_parsed(&data)?;

    if args.dry_run {
        print_summary(config, &data);
        return Ok(());
    }

    if args.force {
        cleanup_tournament_by_name(&args.db, config.name)?;
        println!("Ancien import supprimé (si présent).");
    }

    let store = TournamentStore::open(&args.db)?;
    if tournament_exists_by_name(&store, config.name)? {
        bail!(
            "le tournoi « {} » existe déjà — relancez avec --force",
            config.name
        );
    }

    let mut board = Leaderboard::load(&args.db).context("ouverture leaderboard")?;
    let army_store = ArmyStore::open(&args.db)?;
    let army_by_name = build_army_lookup(&army_store)?;

    ensure_players(&mut board, &data.inscriptions, &args.db)?;

    let tournament = store.create(&CreateTournamentRequest {
        name: config.name.into(),
        bracket_format: config.bracket_format.into(),
    })?;
    let tid = tournament.id;
    println!(
        "Tournoi créé: id={tid} ({}, format {})",
        config.name, config.bracket_format
    );

    store.open_registration(tid)?;
    for name in &data.inscriptions {
        let army_id = resolve_army_id(name, &data, &army_by_name);
        store.admin_register(tid, name, ADMIN_ID, army_id)?;
    }
    store.close_registration(tid)?;

    let ratings: Vec<(String, f64)> = board
        .ranking()
        .into_iter()
        .map(|p| (p.name.clone(), p.rating))
        .collect();
    store.start(tid, &ratings)?;

    let pool_setups: Vec<PoolSetup> = data
        .pools
        .iter()
        .enumerate()
        .map(|(idx, players)| PoolSetup {
            name: format!("Poule {}", (b'A' + idx as u8) as char),
            position: idx as u8,
            players: players.clone(),
        })
        .collect();
    store.setup_pools(tid, &SetupPoolsRequest { pools: pool_setups })?;
    store.generate_pool_matches(tid)?;

    let pool_matches: Vec<TournamentMatch> = store
        .get_detail(tid)?
        .context("tournoi introuvable")?
        .matches
        .into_iter()
        .filter(|m| m.phase.as_str() == "pool")
        .collect();

    for m in &pool_matches {
        let p1 = m.player1.as_ref().unwrap();
        let p2 = m.player2.as_ref().unwrap();
        let key = match_key(p1, p2);
        if let Some(line) = data.pool_matches.get(&key) {
            submit_pool_score(&store, m.id, p1, p2, line)?;
        } else {
            store.declare_match_unplayed(m.id, ADMIN_ID, true)?;
        }
    }

    let pending = store.pool_matches_pending_elo(tid)?;
    for tm in &pending {
        apply_to_board(&mut board, tm)?;
        store.mark_pool_elo_applied(tm.id)?;
    }
    board.save(&args.db)?;

    // Les classements officiels du sheet primont sur les totaux recalculés
    // (matchs incomplets / cases OP-VP manquantes dans les Bassins).
    apply_official_pool_standings(&args.db, tid, &data)?;
    // L'arbre historique peut diverger du tie-break officiel (ex. Coupe 8 Obakami vs Tau24).
    ensure_bracket_players_qualify(&args.db, tid, &data)?;
    store.finalize_pools(tid)?;

    if config.bracket_format == "round_of_16" && data.bracket_r16.len() == 4 {
        let bracket_setup: Vec<BracketSlotSetup> = data
            .bracket_r16
            .iter()
            .enumerate()
            .map(|(slot, m)| {
                let bye = data
                    .bracket_quarters
                    .get(slot)
                    .and_then(|qf| {
                        let r16_keys = [
                            normalize_name(&m.player1),
                            normalize_name(&m.player2),
                        ];
                        [&qf.player1, &qf.player2]
                            .into_iter()
                            .find(|p| !r16_keys.contains(&normalize_name(p)))
                            .cloned()
                    })
                    .or_else(|| data.pools.get(slot).and_then(|p| p.first().cloned()));
                BracketSlotSetup {
                    bracket_slot: slot as u32,
                    player1: m.player1.clone(),
                    player2: m.player2.clone(),
                    quarter_player1: bye,
                }
            })
            .collect();
        store.setup_bracket(tid, &SetupBracketRequest { matches: bracket_setup })?;
    } else if data.bracket_quarters.len() == 4 && data.bracket_r16.is_empty() {
        let bracket_setup: Vec<BracketSlotSetup> = data
            .bracket_quarters
            .iter()
            .enumerate()
            .map(|(slot, m)| BracketSlotSetup {
                bracket_slot: slot as u32,
                player1: m.player1.clone(),
                player2: m.player2.clone(),
                quarter_player1: None,
            })
            .collect();
        store.setup_bracket(tid, &SetupBracketRequest { matches: bracket_setup })?;
    } else {
        println!(
            "Attention: arbre ambigu (r16={}, qf={}) — génération automatique",
            data.bracket_r16.len(),
            data.bracket_quarters.len()
        );
        store.generate_bracket(tid)?;
    }

    let mut all = store.get_detail(tid)?.context("tournoi introuvable")?.matches;

    let qf_players = players_in_lines(&data.bracket_quarters);
    let sf_players = players_in_lines(&data.bracket_semis);
    let final_players = data
        .bracket_final
        .as_ref()
        .map(|f| {
            let mut s = HashSet::new();
            s.insert(normalize_name(&f.player1));
            s.insert(normalize_name(&f.player2));
            s
        })
        .unwrap_or_default();

    for line in &data.bracket_r16 {
        if let Ok(tm) = find_match(&all, "round_of_16", None, &line.player1, &line.player2) {
            let mut adv = qf_players.clone();
            adv.extend(sf_players.iter().cloned());
            adv.extend(final_players.iter().cloned());
            submit_bracket_line(&store, &mut board, tm, line, &adv)?;
        }
    }
    all = store.get_detail(tid)?.context("tournoi introuvable")?.matches;

    for line in &data.bracket_quarters {
        if let Ok(tm) = find_match(&all, "quarter", None, &line.player1, &line.player2) {
            let mut adv = sf_players.clone();
            adv.extend(final_players.iter().cloned());
            submit_bracket_line(&store, &mut board, tm, line, &adv)?;
        }
    }
    all = store.get_detail(tid)?.context("tournoi introuvable")?.matches;

    for line in &data.bracket_semis {
        if let Ok(tm) = find_match(&all, "semi", None, &line.player1, &line.player2) {
            submit_bracket_line(&store, &mut board, tm, line, &final_players)?;
        }
    }
    all = store.get_detail(tid)?.context("tournoi introuvable")?.matches;

    if let Some(line) = &data.bracket_final {
        if let Ok(tm) = find_match(&all, "final", None, &line.player1, &line.player2) {
            let podium: HashSet<String> = data
                .final_placements
                .iter()
                .filter(|(_, r)| *r == 1)
                .map(|(n, _)| normalize_name(n))
                .collect();
            submit_bracket_line(&store, &mut board, tm, line, &podium)?;
        }
    }

    {
        let conn = Connection::open(&args.db)?;
        for (name, rank) in &data.final_placements {
            if *rank <= 4 {
                conn.execute(
                    "UPDATE tournament_players SET final_placement = ?1
                     WHERE tournament_id = ?2 AND player_name_key = ?3",
                    params![rank, tid, normalize_name(name)],
                )?;
            }
        }
        for (key, standing) in &data.pool_standings {
            conn.execute(
                "UPDATE tournament_players SET pool_points = ?1, pool_objectives = ?2, pool_survivors = ?3
                 WHERE tournament_id = ?4 AND player_name_key = ?5",
                params![
                    standing.points,
                    standing.objectives,
                    standing.survivors,
                    tid,
                    key
                ],
            )?;
        }
        let pools_finalized_at = config.started_at + 2_592_000;
        conn.execute(
            "UPDATE tournaments SET started_at = ?1, pools_finalized_at = ?2, completed_at = ?3 WHERE id = ?4",
            params![
                config.started_at,
                pools_finalized_at,
                config.completed_at,
                tid
            ],
        )?;
    }

    board.save(&args.db)?;
    println!("Import terminé — tournoi {tid} ({})", config.name);
    Ok(())
}

fn print_summary(config: &CoupeConfig, data: &ParsedCoupeSheet) {
    println!("=== {} (dry-run) ===", config.name);
    println!("Format: {}", config.bracket_format);
    println!("Inscrits: {}", data.inscriptions.len());
    for (idx, pool) in data.pools.iter().enumerate() {
        println!("  Poule {}: {} joueurs — {:?}", idx + 1, pool.len(), pool);
    }
    println!("Matchs poule: {}", data.pool_matches.len());
    if !data.bracket_r16.is_empty() {
        println!("Barrages (1/8): {}", data.bracket_r16.len());
        for (i, m) in data.bracket_r16.iter().enumerate() {
            println!(
                "  R16{}: {} ({}/{}) vs {} ({}/{})",
                i + 1,
                m.player1,
                m.p1_obj,
                m.p1_surv,
                m.player2,
                m.p2_obj,
                m.p2_surv
            );
        }
    }
    println!("Quarts: {}", data.bracket_quarters.len());
    for (i, m) in data.bracket_quarters.iter().enumerate() {
        println!(
            "  QF{}: {} ({}/{}) vs {} ({}/{})",
            i + 1,
            m.player1,
            m.p1_obj,
            m.p1_surv,
            m.player2,
            m.p2_obj,
            m.p2_surv
        );
    }
    println!("Demis: {}", data.bracket_semis.len());
    for (i, m) in data.bracket_semis.iter().enumerate() {
        println!(
            "  SF{}: {} vs {} — {}-{} / {}-{}",
            i + 1,
            m.player1,
            m.player2,
            m.p1_obj,
            m.p1_surv,
            m.p2_obj,
            m.p2_surv
        );
    }
    if let Some(f) = &data.bracket_final {
        println!(
            "Finale: {} vs {} — {}-{} / {}-{}",
            f.player1, f.player2, f.p1_obj, f.p1_surv, f.p2_obj, f.p2_surv
        );
    }
    println!("Podium:");
    for (name, rank) in &data.final_placements {
        if *rank <= 4 {
            println!("  #{rank}: {name}");
        }
    }
}

fn build_army_lookup(army_store: &ArmyStore) -> Result<HashMap<String, u32>> {
    let mut map = HashMap::new();
    for army in army_store.list_selectable()? {
        map.insert(normalize_name(&army.name), army.id);
    }
    Ok(map)
}

fn resolve_army_id(
    player: &str,
    data: &ParsedCoupeSheet,
    army_by_name: &HashMap<String, u32>,
) -> u32 {
    if let Some(standing) = data.pool_standings.get(&normalize_name(player)) {
        if let Some(id) = army_by_name.get(&normalize_name(&standing.faction)) {
            return *id;
        }
    }
    army_by_name
        .get(&normalize_name(player))
        .copied()
        .unwrap_or(101)
}

fn ensure_players(board: &mut Leaderboard, names: &[String], db_path: &Path) -> Result<()> {
    for name in names {
        if board.get_player(name).is_err() {
            board.add_player(name)?;
        }
    }
    board.save(db_path)?;
    Ok(())
}

fn tournament_exists_by_name(store: &TournamentStore, name: &str) -> Result<bool> {
    Ok(store
        .list()?
        .into_iter()
        .any(|t| t.name == name))
}

fn cleanup_tournament_by_name(db_path: &Path, name: &str) -> Result<()> {
    let conn = Connection::open(db_path)?;
    let tid: Option<i64> = conn
        .query_row(
            "SELECT id FROM tournaments WHERE name = ?1",
            params![name],
            |row| row.get(0),
        )
        .ok();
    let Some(tid) = tid else {
        return Ok(());
    };
    conn.execute("DELETE FROM matches WHERE tournament_id = ?1", params![tid])?;
    conn.execute(
        "DELETE FROM tournament_matches WHERE tournament_id = ?1",
        params![tid],
    )?;
    conn.execute(
        "DELETE FROM pool_players WHERE pool_id IN (SELECT id FROM pools WHERE tournament_id = ?1)",
        params![tid],
    )?;
    conn.execute("DELETE FROM pools WHERE tournament_id = ?1", params![tid])?;
    conn.execute(
        "DELETE FROM tournament_players WHERE tournament_id = ?1",
        params![tid],
    )?;
    conn.execute(
        "DELETE FROM tournament_registrations WHERE tournament_id = ?1",
        params![tid],
    )?;
    conn.execute("DELETE FROM tournaments WHERE id = ?1", params![tid])?;
    rebuild_leaderboard_from_matches(db_path)?;
    Ok(())
}

fn apply_official_pool_standings(
    db_path: &Path,
    tid: i64,
    data: &ParsedCoupeSheet,
) -> Result<()> {
    let conn = Connection::open(db_path)?;
    for (key, standing) in &data.pool_standings {
        conn.execute(
            "UPDATE tournament_players SET pool_points = ?1, pool_objectives = ?2, pool_survivors = ?3
             WHERE tournament_id = ?4 AND player_name_key = ?5",
            params![
                standing.points,
                standing.objectives,
                standing.survivors,
                tid,
                key
            ],
        )?;
    }
    Ok(())
}

fn ensure_bracket_players_qualify(
    db_path: &Path,
    tid: i64,
    data: &ParsedCoupeSheet,
) -> Result<()> {
    let mut needed: HashMap<String, String> = HashMap::new();
    for m in data
        .bracket_r16
        .iter()
        .chain(data.bracket_quarters.iter())
        .chain(data.bracket_semis.iter())
    {
        needed.insert(normalize_name(&m.player1), m.player1.clone());
        needed.insert(normalize_name(&m.player2), m.player2.clone());
    }
    if let Some(f) = &data.bracket_final {
        needed.insert(normalize_name(&f.player1), f.player1.clone());
        needed.insert(normalize_name(&f.player2), f.player2.clone());
    }

    let conn = Connection::open(db_path)?;
    for (pool_idx, roster) in data.pools.iter().enumerate() {
        let roster_keys: Vec<String> = roster.iter().map(|n| normalize_name(n)).collect();
        let bracket_in_pool: Vec<String> = roster_keys
            .iter()
            .filter(|k| needed.contains_key(k.as_str()))
            .cloned()
            .collect();
        if bracket_in_pool.is_empty() {
            continue;
        }
        // Assure les joueurs d'arbre dans le top 3 : points décroissants 300/200/100…
        for (rank, key) in bracket_in_pool.iter().take(3).enumerate() {
            let points = 300u32 - (rank as u32) * 100;
            conn.execute(
                "UPDATE tournament_players SET pool_points = MAX(pool_points, ?1)
                 WHERE tournament_id = ?2 AND player_name_key = ?3",
                params![points, tid, key],
            )?;
        }
        // Si un joueur d'arbre est hors du top 3 du roster sheet, le forcer quand même.
        for key in needed.keys() {
            if roster_keys.contains(key) && !bracket_in_pool.iter().take(3).any(|k| k == key) {
                let _ = pool_idx;
                conn.execute(
                    "UPDATE tournament_players SET pool_points = 150
                     WHERE tournament_id = ?1 AND player_name_key = ?2",
                    params![tid, key],
                )?;
            }
        }
    }
    Ok(())
}

fn submit_pool_score(
    store: &TournamentStore,
    match_id: i64,
    p1: &str,
    _p2: &str,
    line: &PoolMatchLine,
) -> Result<TournamentMatch> {
    let (mut p1_obj, mut p2_obj, mut p1_surv, mut p2_surv) =
        (line.p1_obj, line.p2_obj, line.p1_surv, line.p2_surv);
    // Les scores de la feuille sont relatifs à line.player1 / line.player2.
    if normalize_name(p1) != normalize_name(&line.player1) {
        std::mem::swap(&mut p1_obj, &mut p2_obj);
        std::mem::swap(&mut p1_surv, &mut p2_surv);
    }
    store.submit_match(
        match_id,
        &SubmitMatchRequest {
            player1_objectives: p1_obj,
            player2_objectives: p2_obj,
            player1_survivors: p1_surv,
            player2_survivors: p2_surv,
            player1_army_id: None,
            player2_army_id: None,
            scenario_id: None,
            scenario_other: line.scenario.clone(),
        },
        ADMIN_ID,
        true,
        None,
        DEFAULT_K_FACTOR,
    )
}

fn players_in_lines(lines: &[BracketMatchLine]) -> HashSet<String> {
    let mut set = HashSet::new();
    for m in lines {
        set.insert(normalize_name(&m.player1));
        set.insert(normalize_name(&m.player2));
    }
    set
}

fn submit_bracket_line(
    store: &TournamentStore,
    board: &mut Leaderboard,
    tm: &TournamentMatch,
    line: &BracketMatchLine,
    advancers: &HashSet<String>,
) -> Result<()> {
    let db_p1 = tm.player1.as_ref().context("joueur 1 manquant")?;
    let db_p2 = tm.player2.as_ref().context("joueur 2 manquant")?;
    let (p1_obj, p2_obj, p1_surv, p2_surv) =
        if normalize_name(db_p1) == normalize_name(&line.player1) {
            (
                line.p1_obj,
                line.p2_obj,
                line.p1_surv,
                line.p2_surv,
            )
        } else {
            (
                line.p2_obj,
                line.p1_obj,
                line.p2_surv,
                line.p1_surv,
            )
        };

    // Scores absents du sheet : forfait du joueur qui n'apparaît pas au tour suivant.
    if p1_obj == 0 && p2_obj == 0 && p1_surv == 0 && p2_surv == 0 {
        let p1_adv = advancers.contains(&normalize_name(db_p1));
        let p2_adv = advancers.contains(&normalize_name(db_p2));
        let forfeit_player = match (p1_adv, p2_adv) {
            (true, false) => db_p2.as_str(),
            (false, true) => db_p1.as_str(),
            _ => {
                // À égalité / inconnu : le mieux classé au podium gagne si possible.
                let p1_rank = data_rank_hint(line, db_p1);
                let p2_rank = data_rank_hint(line, db_p2);
                if p1_rank < p2_rank {
                    db_p2.as_str()
                } else {
                    db_p1.as_str()
                }
            }
        };
        let tm = store.declare_forfeit(tm.id, forfeit_player, ADMIN_ID, true)?;
        // Pas d'Elo sur forfait.
        let _ = tm;
        return Ok(());
    }

    let tm = store.submit_match(
        tm.id,
        &SubmitMatchRequest {
            player1_objectives: p1_obj,
            player2_objectives: p2_obj,
            player1_survivors: p1_surv,
            player2_survivors: p2_surv,
            player1_army_id: None,
            player2_army_id: None,
            scenario_id: None,
            scenario_other: line.scenario.clone(),
        },
        ADMIN_ID,
        true,
        None,
        DEFAULT_K_FACTOR,
    )?;
    apply_to_board(board, &tm)?;
    store.update_bracket_rating(
        tm.tournament_id,
        tm.player1.as_ref().unwrap(),
        tm.player1_elo_delta,
    )?;
    store.update_bracket_rating(
        tm.tournament_id,
        tm.player2.as_ref().unwrap(),
        tm.player2_elo_delta,
    )?;
    Ok(())
}

fn data_rank_hint(line: &BracketMatchLine, player: &str) -> u32 {
    // Heuristique faible : le joueur avec le plus d'objectifs dans la ligne gagne.
    if normalize_name(player) == normalize_name(&line.player1) {
        100u32.saturating_sub(line.p1_obj as u32)
    } else {
        100u32.saturating_sub(line.p2_obj as u32)
    }
}

fn apply_to_board(board: &mut Leaderboard, tm: &TournamentMatch) -> Result<()> {
    if tm.is_forfeit || tm.is_unplayed {
        return Ok(());
    }
    let p1 = tm.player1.as_ref().context("joueur 1 manquant")?;
    let p2 = tm.player2.as_ref().context("joueur 2 manquant")?;
    let outcome = tm.outcome.context("résultat manquant")?;
    let update = RatingUpdate {
        player1_old: tm.player1_rating_used.unwrap_or(0.0),
        player1_new: tm.player1_rating_used.unwrap_or(0.0) + tm.player1_elo_delta,
        player2_old: tm.player2_rating_used.unwrap_or(0.0),
        player2_new: tm.player2_rating_used.unwrap_or(0.0) + tm.player2_elo_delta,
    };
    let scores = MatchScores {
        player1_objectives: tm.player1_objectives,
        player1_survivors: tm.player1_survivors,
        player2_objectives: tm.player2_objectives,
        player2_survivors: tm.player2_survivors,
    };
    board.apply_match_update(
        p1,
        p2,
        outcome,
        update,
        scores,
        tm.player1_army_id,
        tm.player2_army_id,
        tm.scenario_id,
        tm.scenario_other.clone(),
        tm.scenario_name.clone(),
        Some(tm.tournament_id),
        Some(tm.phase.as_str().to_string()),
        None,
    )?;
    Ok(())
}

fn find_match<'a>(
    matches: &'a [TournamentMatch],
    phase: &str,
    slot: Option<u32>,
    p1: &str,
    p2: &str,
) -> Result<&'a TournamentMatch> {
    let k1 = normalize_name(p1);
    let k2 = normalize_name(p2);
    matches
        .iter()
        .find(|m| {
            m.phase.as_str() == phase
                && slot.map(|s| m.bracket_slot == Some(s)).unwrap_or(true)
                && ((m.player1.as_ref().map(|n| normalize_name(n)) == Some(k1.clone())
                    && m.player2.as_ref().map(|n| normalize_name(n)) == Some(k2.clone()))
                    || (m.player1.as_ref().map(|n| normalize_name(n)) == Some(k2.clone())
                        && m.player2.as_ref().map(|n| normalize_name(n)) == Some(k1.clone())))
        })
        .with_context(|| format!("match introuvable: {phase} {p1} vs {p2}"))
}

fn rebuild_leaderboard_from_matches(db_path: &Path) -> Result<()> {
    let conn = Connection::open(db_path)?;
    let mut board = Leaderboard::load(db_path)?;
    let names: Vec<String> = board.ranking().into_iter().map(|p| p.name.clone()).collect();
    for name in &names {
        let player = board.get_player_mut(name)?;
        player.rating = DEFAULT_RATING;
        player.wins = 0;
        player.draws = 0;
        player.losses = 0;
    }

    let mut stmt = conn.prepare(
        "
        SELECT id, player1, player2, outcome,
               player1_old, player1_new, player2_old, player2_new,
               player1_objectives, player1_survivors,
               player2_objectives, player2_survivors,
               player1_army_id, player2_army_id,
               scenario_id, scenario_other,
               tournament_id, tournament_phase, recorded_at
        FROM matches
        ORDER BY recorded_at ASC, id ASC
        ",
    )?;
    let rows = stmt.query_map([], |row| {
        let scenario_other: Option<String> = row.get(15)?;
        Ok(MatchRecord {
            id: row.get(0)?,
            player1: row.get(1)?,
            player2: row.get(2)?,
            outcome: match row.get::<_, String>(3)?.as_str() {
                "player1_win" => MatchOutcome::Player1Win,
                "player2_win" => MatchOutcome::Player2Win,
                _ => MatchOutcome::Draw,
            },
            player1_old: row.get(4)?,
            player1_new: row.get(5)?,
            player2_old: row.get(6)?,
            player2_new: row.get(7)?,
            player1_objectives: row.get(8)?,
            player1_survivors: row.get(9)?,
            player2_objectives: row.get(10)?,
            player2_survivors: row.get(11)?,
            player1_army_id: row.get(12)?,
            player2_army_id: row.get(13)?,
            scenario_id: row.get(14)?,
            scenario_other: scenario_other.clone(),
            scenario_name: scenario_other,
            tournament_id: row.get(16)?,
            tournament_phase: row.get(17)?,
            tournament_name: None,
            recorded_at: row.get(18)?,
        })
    })?;

    for row in rows {
        let record = row?;
        let update = RatingUpdate {
            player1_old: record.player1_old,
            player1_new: record.player1_new,
            player2_old: record.player2_old,
            player2_new: record.player2_new,
        };
        let scores = MatchScores {
            player1_objectives: record.player1_objectives,
            player1_survivors: record.player1_survivors,
            player2_objectives: record.player2_objectives,
            player2_survivors: record.player2_survivors,
        };
        board.apply_match_update(
            &record.player1,
            &record.player2,
            record.outcome,
            update,
            scores,
            record.player1_army_id,
            record.player2_army_id,
            record.scenario_id,
            record.scenario_other,
            record.scenario_name,
            record.tournament_id,
            record.tournament_phase,
            record.tournament_name,
        )?;
    }

    for (key, player) in &board.players {
        conn.execute(
            "UPDATE players SET rating = ?1, wins = ?2, draws = ?3, losses = ?4 WHERE name_key = ?5",
            params![player.rating, player.wins, player.draws, player.losses, key],
        )?;
    }
    Ok(())
}

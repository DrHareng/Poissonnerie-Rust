use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use rusqlite::{Connection, OptionalExtension};
use poissonnerie_elo::{
    store::normalize_name, Leaderboard, MatchOutcome, MatchScores, MatchRecord, RatingUpdate,
    DEFAULT_K_FACTOR, DEFAULT_RATING,
};
use poissonnerie_elo::tournament::TournamentMatch;
use poissonnerie_elo::tournament_store::{
    BracketSlotSetup, CreateTournamentRequest, PoolSetup, SetupBracketRequest, SetupPoolsRequest,
    SubmitMatchRequest, TournamentStore,
};

const ADMIN_ID: i64 = 1;

#[derive(Parser)]
#[command(name = "poissonnerie-import-coupe4", about = "Import Coupe de la Poissonnerie 4")]
struct Args {
    #[arg(long, default_value = "data/poissonnerie.db")]
    db: PathBuf,

    /// Supprime un import partiel du tournoi 4 et réapplique les Elo globaux.
    #[arg(long)]
    force: bool,

    /// Recalcule les Elo globaux à partir de la table matches.
    #[arg(long)]
    rebuild_leaderboard: bool,
}

#[derive(Clone, Copy)]
struct ScoreLine {
    p1_obj: u8,
    p2_obj: u8,
    p1_surv: u16,
    p2_surv: u16,
    scenario: Option<&'static str>,
}

fn match_key(a: &str, b: &str) -> (String, String) {
    let mut keys = [normalize_name(a), normalize_name(b)];
    keys.sort();
    (keys[0].clone(), keys[1].clone())
}

fn pool_scores() -> HashMap<(String, String), ScoreLine> {
    let mut m = HashMap::new();

    let add = |m: &mut HashMap<_, _>, a: &str, b: &str, line: ScoreLine| {
        m.insert(match_key(a, b), line);
    };

    // Bassin A
    add(&mut m, "-Scorpion-", "Arkille", ScoreLine { p1_obj: 0, p2_obj: 5, p1_surv: 1, p2_surv: 7, scenario: Some("A : Scène de crime") });
    add(&mut m, "-Scorpion-", "Obakami", ScoreLine { p1_obj: 0, p2_obj: 4, p1_surv: 0, p2_surv: 3, scenario: Some("C : Exfiltration") });
    add(&mut m, "Arkille", "Obakami", ScoreLine { p1_obj: 5, p2_obj: 2, p1_surv: 8, p2_surv: 6, scenario: Some("B : Avant-postes") });

    // Bassin B
    add(&mut m, "Maman_Poulet", "Sitsu", ScoreLine { p1_obj: 1, p2_obj: 4, p1_surv: 2, p2_surv: 4, scenario: Some("A : Scène de crime") });
    add(&mut m, "Astellan", "Maman_Poulet", ScoreLine { p1_obj: 5, p2_obj: 0, p1_surv: 9, p2_surv: 4, scenario: Some("B : Avant-postes") });
    add(&mut m, "Kamcord", "Maman_Poulet", ScoreLine { p1_obj: 5, p2_obj: 0, p1_surv: 10, p2_surv: 2, scenario: Some("D : Églantine, ici Mirabelle !") });
    add(&mut m, "Astellan", "Sitsu", ScoreLine { p1_obj: 5, p2_obj: 0, p1_surv: 10, p2_surv: 0, scenario: Some("E : Data Mining") });
    add(&mut m, "LoGascon", "Sitsu", ScoreLine { p1_obj: 5, p2_obj: 1, p1_surv: 6, p2_surv: 4, scenario: Some("D : Églantine, ici Mirabelle !") });
    add(&mut m, "Kamcord", "Sitsu", ScoreLine { p1_obj: 3, p2_obj: 3, p1_surv: 8, p2_surv: 8, scenario: Some("B : Avant-postes") });
    add(&mut m, "Astellan", "LoGascon", ScoreLine { p1_obj: 4, p2_obj: 0, p1_surv: 4, p2_surv: 0, scenario: Some("A : Scène de crime") });
    add(&mut m, "Astellan", "Kamcord", ScoreLine { p1_obj: 0, p2_obj: 5, p1_surv: 1, p2_surv: 10, scenario: Some("C : Exfiltration") });
    add(&mut m, "Kamcord", "LoGascon", ScoreLine { p1_obj: 5, p2_obj: 0, p1_surv: 8, p2_surv: 2, scenario: Some("E : Data Mining") });

    // Bassin C
    add(&mut m, "Dr Hareng", "Stuffist", ScoreLine { p1_obj: 4, p2_obj: 1, p1_surv: 4, p2_surv: 2, scenario: Some("A : Scène de crime") });
    add(&mut m, "Dr Hareng", "wulfric", ScoreLine { p1_obj: 5, p2_obj: 2, p1_surv: 7, p2_surv: 5, scenario: Some("B : Avant-postes") });
    add(&mut m, "Dr Hareng", "Shas'O Kassad", ScoreLine { p1_obj: 5, p2_obj: 2, p1_surv: 7, p2_surv: 5, scenario: Some("C : Exfiltration") });
    add(&mut m, "Stuffist", "wulfric", ScoreLine { p1_obj: 5, p2_obj: 1, p1_surv: 8, p2_surv: 5, scenario: Some("C : Exfiltration") });
    add(&mut m, "Shas'O Kassad", "Stuffist", ScoreLine { p1_obj: 5, p2_obj: 1, p1_surv: 10, p2_surv: 5, scenario: Some("B : Avant-postes") });
    add(&mut m, "Shas'O Kassad", "wulfric", ScoreLine { p1_obj: 5, p2_obj: 0, p1_surv: 9, p2_surv: 2, scenario: Some("A : Scène de crime") });

    // Bassin D
    add(&mut m, "Ayadan", "Nico12", ScoreLine { p1_obj: 5, p2_obj: 0, p1_surv: 5, p2_surv: 1, scenario: Some("A : Scène d'Acquisition") });
    add(&mut m, "Ayadan", "Tau24", ScoreLine { p1_obj: 5, p2_obj: 2, p1_surv: 7, p2_surv: 5, scenario: Some("B : Avant-Acquisition") });
    add(&mut m, "Ayadan", "Fanfoué (MErcurE)", ScoreLine { p1_obj: 5, p2_obj: 2, p1_surv: 8, p2_surv: 7, scenario: Some("C : ExAcquisition") });
    add(&mut m, "Nico12", "Tau24", ScoreLine { p1_obj: 0, p2_obj: 5, p1_surv: 0, p2_surv: 7, scenario: Some("C : Exfiltration") });
    add(&mut m, "Fanfoué (MErcurE)", "Nico12", ScoreLine { p1_obj: 5, p2_obj: 0, p1_surv: 11, p2_surv: 4, scenario: Some("B : Avant-postes") });
    add(&mut m, "Fanfoué (MErcurE)", "Tau24", ScoreLine { p1_obj: 0, p2_obj: 5, p1_surv: 0, p2_surv: 5, scenario: Some("A : Scène de crime") });

    m
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

fn submit_score(
    store: &TournamentStore,
    match_id: i64,
    p1: &str,
    p2: &str,
    scores: &HashMap<(String, String), ScoreLine>,
) -> Result<TournamentMatch> {
    let key = match_key(p1, p2);
    let Some(line) = scores.get(&key) else {
        anyhow::bail!("score manquant pour {p1} vs {p2}");
    };
    let (mut p1_obj, mut p2_obj, mut p1_surv, mut p2_surv) =
        (line.p1_obj, line.p2_obj, line.p1_surv, line.p2_surv);
    // Score lines are keyed alphabetically; remap to match row player order.
    if normalize_name(p1) != key.0 {
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
            scenario_other: line.scenario.map(str::to_string),
        },
        ADMIN_ID,
        true,
        None,
        DEFAULT_K_FACTOR,
    )
}

fn submit_bracket(
    store: &TournamentStore,
    board: &mut Leaderboard,
    tm: &TournamentMatch,
    sheet_first: &str,
    sheet_first_obj: u8,
    sheet_second_obj: u8,
    sheet_first_surv: u16,
    sheet_second_surv: u16,
    scenario: &str,
) -> Result<()> {
    let db_p1 = tm.player1.as_ref().context("joueur 1 manquant")?;
    let (p1_obj, p2_obj, p1_surv, p2_surv) = if normalize_name(db_p1) == normalize_name(sheet_first) {
        (
            sheet_first_obj,
            sheet_second_obj,
            sheet_first_surv,
            sheet_second_surv,
        )
    } else {
        (
            sheet_second_obj,
            sheet_first_obj,
            sheet_second_surv,
            sheet_first_surv,
        )
    };
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
            scenario_other: Some(scenario.to_string()),
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
                && m.player1.as_ref().map(|n| normalize_name(n)) == Some(k1.clone())
                && m.player2.as_ref().map(|n| normalize_name(n)) == Some(k2.clone())
                || m.phase.as_str() == phase
                    && slot.map(|s| m.bracket_slot == Some(s)).unwrap_or(true)
                    && m.player1.as_ref().map(|n| normalize_name(n)) == Some(k2.clone())
                    && m.player2.as_ref().map(|n| normalize_name(n)) == Some(k1.clone())
        })
        .with_context(|| format!("match introuvable: {phase} {p1} vs {p2}"))
}

fn cleanup_partial_import(db_path: &Path) -> Result<()> {
    let conn = Connection::open(db_path)?;
    let cutoff: Option<i64> = conn
        .query_row(
            "SELECT MIN(id) FROM matches WHERE recorded_at >= 1783674980",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(cutoff) = cutoff {
        conn.execute("DELETE FROM matches WHERE id >= ?1", [cutoff])?;
    }

    conn.execute("DELETE FROM tournament_matches WHERE tournament_id = 4", [])?;
    conn.execute(
        "DELETE FROM pool_players WHERE pool_id IN (SELECT id FROM pools WHERE tournament_id = 4)",
        [],
    )?;
    conn.execute("DELETE FROM pools WHERE tournament_id = 4", [])?;
    conn.execute("DELETE FROM tournament_players WHERE tournament_id = 4", [])?;
    conn.execute("DELETE FROM tournament_registrations WHERE tournament_id = 4", [])?;
    conn.execute("DELETE FROM tournaments WHERE id = 4", [])?;
    conn.execute("DELETE FROM sqlite_sequence WHERE name = 'tournaments'", [])?;

    rebuild_leaderboard_from_matches(db_path)?;
    Ok(())
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
            status: poissonnerie_elo::MatchStatus::Completed,
            outcome: Some(match row.get::<_, String>(3)?.as_str() {
                "player1_win" => MatchOutcome::Player1Win,
                "player2_win" => MatchOutcome::Player2Win,
                _ => MatchOutcome::Draw,
            }),
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
            player1_report: None,
            player2_report: None,
            player1_army_list_code: None,
            player2_army_list_code: None,
            player1_secondary_slugs: None,
            player2_secondary_slugs: None,
            secondary_pool_slugs: None,
            player1_chosen_secondary: None,
            player2_chosen_secondary: None,
            lieutenant_winner: None,
            lieutenant_winner_choice: None,
            lieutenant_other_choice: None,
            partie_step: None,
            created_by: None,
            recorded_at: row.get(18)?,
        })
    })?;

    for row in rows {
        let record = row?;
        let Some(outcome) = record.outcome else {
            continue;
        };
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
            outcome,
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
            rusqlite::params![player.rating, player.wins, player.draws, player.losses, key],
        )?;
    }
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();
    let db_path = &args.db;

    if args.rebuild_leaderboard {
        rebuild_leaderboard_from_matches(db_path)?;
        println!("Classement Elo recalculé depuis la table matches.");
        return Ok(());
    }

    if args.force {
        cleanup_partial_import(db_path)?;
        println!("Nettoyage du tournoi 4 partiel terminé.");
    }

    let mut board = Leaderboard::load(db_path).context("ouverture leaderboard")?;
    let store = TournamentStore::open(db_path)?;

    if store.get(4)?.is_some() {
        anyhow::bail!("le tournoi id=4 existe déjà");
    }

    if board.get_player("-Scorpion-").is_err() {
        board.add_player("-Scorpion-")?;
        board.save(db_path)?;
    }

    let tournament = store.create(&CreateTournamentRequest {
        name: "Coupe de la Poissonnerie 4".into(),
        bracket_format: "quarters_direct".into(),
    })?;
    let tid = tournament.id;
    println!("Tournoi créé: id={tid}");

    store.open_registration(tid)?;
    let players = [
        ("Dr Hareng", 402u32),
        ("Tau24", 1002),
        ("Kamcord", 604),
        ("Nico12", 202),
        ("Ayadan", 1003),
        ("Sitsu", 305),
        ("LoGascon", 201),
        ("Takamura", 801),
        ("Astellan", 1003),
        ("Shas'O Kassad", 104),
        ("-Scorpion-", 603),
        ("Maman_Poulet", 402),
        ("Arkille", 303),
        ("Fanfoué (MErcurE)", 1101),
        ("Obakami", 205),
        ("Stuffist", 205),
        ("wulfric", 501),
    ];
    for (name, army_id) in players {
        store.admin_register(tid, name, ADMIN_ID, army_id)?;
    }
    store.close_registration(tid)?;

    let ratings: Vec<(String, f64)> = board
        .ranking()
        .into_iter()
        .map(|p| (p.name.clone(), p.rating))
        .collect();
    store.start(tid, &ratings)?;

    store.setup_pools(
        tid,
        &SetupPoolsRequest {
            pools: vec![
                PoolSetup {
                    name: "Poule A".into(),
                    position: 0,
                    players: vec![
                        "Arkille".into(),
                        "-Scorpion-".into(),
                        "Takamura".into(),
                        "Obakami".into(),
                    ],
                },
                PoolSetup {
                    name: "Poule B".into(),
                    position: 1,
                    players: vec![
                        "Kamcord".into(),
                        "Astellan".into(),
                        "Sitsu".into(),
                        "LoGascon".into(),
                        "Maman_Poulet".into(),
                    ],
                },
                PoolSetup {
                    name: "Poule C".into(),
                    position: 2,
                    players: vec![
                        "Shas'O Kassad".into(),
                        "Dr Hareng".into(),
                        "Stuffist".into(),
                        "wulfric".into(),
                    ],
                },
                PoolSetup {
                    name: "Poule D".into(),
                    position: 3,
                    players: vec![
                        "Ayadan".into(),
                        "Tau24".into(),
                        "Nico12".into(),
                        "Fanfoué (MErcurE)".into(),
                    ],
                },
            ],
        },
    )?;

    store.generate_pool_matches(tid)?;
    let scores = pool_scores();
    let pool_matches: Vec<TournamentMatch> = store
        .get_detail(tid)?
        .context("tournoi introuvable")?
        .matches
        .into_iter()
        .filter(|m| m.phase.as_str() == "pool")
        .collect();

    let unplayed_pairs = [
        match_key("-Scorpion-", "Takamura"),
        match_key("Arkille", "Takamura"),
        match_key("Takamura", "Obakami"),
        match_key("LoGascon", "Maman_Poulet"),
    ];

    for m in &pool_matches {
        let p1 = m.player1.as_ref().unwrap();
        let p2 = m.player2.as_ref().unwrap();
        let key = match_key(p1, p2);
        if unplayed_pairs.contains(&key) {
            store.declare_match_unplayed(m.id, ADMIN_ID, true)?;
            continue;
        }
        submit_score(&store, m.id, p1, p2, &scores)?;
    }

    let pending = store.pool_matches_pending_elo(tid)?;
    for tm in &pending {
        apply_to_board(&mut board, tm)?;
        store.mark_pool_elo_applied(tm.id)?;
    }
    board.save(db_path)?;
    store.finalize_pools(tid)?;

    store.setup_bracket(
        tid,
        &SetupBracketRequest {
            matches: vec![
                BracketSlotSetup {
                    bracket_slot: 0,
                    player1: "Arkille".into(),
                    player2: "Shas'O Kassad".into(),
                    quarter_player1: None,
                },
                BracketSlotSetup {
                    bracket_slot: 1,
                    player1: "Tau24".into(),
                    player2: "Kamcord".into(),
                    quarter_player1: None,
                },
                BracketSlotSetup {
                    bracket_slot: 2,
                    player1: "Dr Hareng".into(),
                    player2: "Astellan".into(),
                    quarter_player1: None,
                },
                BracketSlotSetup {
                    bracket_slot: 3,
                    player1: "Ayadan".into(),
                    player2: "Obakami".into(),
                    quarter_player1: None,
                },
            ],
        },
    )?;

    let all = store.get_detail(tid)?.context("tournoi introuvable")?.matches;

    // Quarts
    submit_bracket(
        &store,
        &mut board,
        find_match(&all, "quarter", Some(0), "Arkille", "Shas'O Kassad")?,
        "Arkille",
        4,
        6,
        249,
        200,
        "Églantine, ici Mirabelle !",
    )?;
    submit_bracket(
        &store,
        &mut board,
        find_match(&all, "quarter", Some(1), "Tau24", "Kamcord")?,
        "Tau24",
        9,
        3,
        148,
        169,
        "Églantine, ici Mirabelle !",
    )?;
    submit_bracket(
        &store,
        &mut board,
        find_match(&all, "quarter", Some(2), "Dr Hareng", "Astellan")?,
        "Dr Hareng",
        10,
        0,
        300,
        0,
        "Églantine, ici Mirabelle !",
    )?;
    submit_bracket(
        &store,
        &mut board,
        find_match(&all, "quarter", Some(3), "Ayadan", "Obakami")?,
        "Ayadan",
        5,
        3,
        167,
        128,
        "Églantine, ici Mirabelle !",
    )?;

    let all = store.get_detail(tid)?.context("tournoi introuvable")?.matches;

    // Demis
    submit_bracket(
        &store,
        &mut board,
        find_match(&all, "semi", Some(0), "Shas'O Kassad", "Tau24")?,
        "Shas'O Kassad",
        7,
        5,
        242,
        146,
        "Razzia",
    )?;
    submit_bracket(
        &store,
        &mut board,
        find_match(&all, "semi", Some(1), "Ayadan", "Dr Hareng")?,
        "Ayadan",
        6,
        1,
        144,
        74,
        "Razzia",
    )?;

    let all = store.get_detail(tid)?.context("tournoi introuvable")?.matches;

    // Finale
    submit_bracket(
        &store,
        &mut board,
        find_match(&all, "final", Some(0), "Shas'O Kassad", "Ayadan")?,
        "Shas'O Kassad",
        9,
        6,
        0,
        0,
        "Le combat de l'esprit",
    )?;

    {
        let conn = Connection::open(db_path)?;
        let standings = [
            ("Arkille", 10u32, 15u32, 346u32),
            ("Obakami", 6, 9, 413),
            ("-Scorpion-", 0, 1, 159),
            ("Takamura", 0, 0, 0),
            ("Kamcord", 18, 36, 1044),
            ("Astellan", 14, 24, 728),
            ("Sitsu", 8, 16, 309),
            ("LoGascon", 5, 8, 286),
            ("Maman_Poulet", 1, 8, 290),
            ("Dr Hareng", 14, 18, 493),
            ("Shas'O Kassad", 12, 24, 642),
            ("Stuffist", 7, 15, 326),
            ("wulfric", 3, 12, 437),
            ("Ayadan", 15, 20, 460),
            ("Tau24", 12, 17, 689),
            ("Fanfoué (MErcurE)", 7, 18, 252),
            ("Nico12", 0, 5, 151),
        ];
        let placements = [
            ("Shas'O Kassad", 1u32),
            ("Ayadan", 2),
            ("Tau24", 3),
            ("Dr Hareng", 4),
        ];
        for (name, rank) in placements {
            conn.execute(
                "UPDATE tournament_players SET final_placement = ?1
                 WHERE tournament_id = ?2 AND player_name_key = ?3",
                rusqlite::params![rank, tid, normalize_name(name)],
            )?;
        }
        for (name, pts, op, vp) in standings {
            conn.execute(
                "UPDATE tournament_players SET pool_points = ?1, pool_objectives = ?2, pool_survivors = ?3
                 WHERE tournament_id = ?4 AND player_name_key = ?5",
                rusqlite::params![pts, op, vp, tid, normalize_name(name)],
            )?;
        }
        let completed_at: u64 = 1783614687;
        let started_at: u64 = 1748342400;
        let pools_finalized_at: u64 = 1751328000;
        conn.execute(
            "UPDATE tournaments SET started_at = ?1, pools_finalized_at = ?2, completed_at = ?3 WHERE id = ?4",
            rusqlite::params![started_at, pools_finalized_at, completed_at, tid],
        )?;
    }

    board.save(db_path)?;

    println!("Import terminé — tournoi {tid} (Coupe 4) en base.");
    Ok(())
}

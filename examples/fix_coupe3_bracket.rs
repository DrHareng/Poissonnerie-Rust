//! Corrige les quarts de la Coupe 3 : noms de joueurs permutés alors que
//! scores / armées correspondaient déjà aux bons matchs.
//!
//! Quart 0 : Arkille vs Azazel(Bakunin) → Arkille vs Starpu
//! Quart 1 : Ayadan(Bahram) vs Kantain(Kosmoflot) → Dr Hareng vs Azazel
//!
//! Supprime aussi les doublons `matches` 500/501 (versions corrompues de 368/370).

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use rusqlite::{params, Connection};

use poissonnerie_elo::{
    army::default_db_path, recompute_elo_from_matches, TournamentStore, DEFAULT_K_FACTOR,
};

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value_os_t = default_db_path())]
    db: PathBuf,

    #[arg(long, default_value_t = DEFAULT_K_FACTOR)]
    k: f64,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let tid = 3i64;

    let conn = Connection::open(&args.db)
        .with_context(|| format!("ouverture {}", args.db.display()))?;

    let name: String = conn.query_row(
        "SELECT name FROM tournaments WHERE id = ?1",
        [tid],
        |row| row.get(0),
    )?;
    println!("Tournoi {tid}: {name}");

    // Quart 0 : vainqueur R16 Starpu (armée Bakunin 503), pas Azazel
    let n = conn.execute(
        "
        UPDATE tournament_matches
        SET player2 = 'Starpu'
        WHERE tournament_id = ?1 AND phase = 'quarter' AND bracket_slot = 0
          AND player1 = 'Arkille' AND player2 = 'Azazel' AND player2_army_id = 503
        ",
        params![tid],
    )?;
    println!("quart 0 renommé : {n}");

    // Quart 1 : bye Dr Hareng (Bahram 402) vs Azazel (Kosmoflot 306)
    let n = conn.execute(
        "
        UPDATE tournament_matches
        SET player1 = 'Dr Hareng', player2 = 'Azazel'
        WHERE tournament_id = ?1 AND phase = 'quarter' AND bracket_slot = 1
          AND player1 = 'Ayadan' AND player2 = 'Kantain'
          AND player1_army_id = 402 AND player2_army_id = 306
        ",
        params![tid],
    )?;
    println!("quart 1 renommé : {n}");

    // Corriger d'éventuelles lignes matches encore corrompues, puis supprimer les doublons.
    conn.execute(
        "
        UPDATE matches
        SET player2 = 'Starpu'
        WHERE tournament_id = ?1 AND tournament_phase = 'quarter'
          AND player1 = 'Arkille' AND player2 = 'Azazel' AND player2_army_id = 503
        ",
        params![tid],
    )?;
    conn.execute(
        "
        UPDATE matches
        SET player1 = 'Dr Hareng', player2 = 'Azazel'
        WHERE tournament_id = ?1 AND tournament_phase = 'quarter'
          AND player1 = 'Ayadan' AND player2 = 'Kantain'
          AND player1_army_id = 402 AND player2_army_id = 306
        ",
        params![tid],
    )?;

    let deleted = conn.execute(
        "
        DELETE FROM matches
        WHERE id IN (500, 501) AND tournament_id = ?1 AND tournament_phase = 'quarter'
        ",
        params![tid],
    )?;
    println!("Doublons matches 500/501 supprimés : {deleted}");

    drop(conn);

    println!("Recalcul ELO…");
    recompute_elo_from_matches(&args.db, args.k)?;

    let store = TournamentStore::open(&args.db)?;
    store.refresh_final_placements(tid)?;

    let detail = store.get_detail(tid)?.context("tournoi introuvable")?;
    println!("Placements :");
    let mut players = detail.players;
    players.sort_by_key(|p| p.final_placement.unwrap_or(999));
    for p in players.iter().filter(|p| p.final_placement.is_some()) {
        println!("  #{} {}", p.final_placement.unwrap(), p.player_name);
    }

    println!("Bracket quarts :");
    let mut qs: Vec<_> = detail
        .matches
        .iter()
        .filter(|m| m.phase.as_str() == "quarter")
        .collect();
    qs.sort_by_key(|m| m.bracket_slot.unwrap_or(0));
    for m in qs {
        println!(
            "  [{}] {} vs {}",
            m.bracket_slot.unwrap_or(0),
            m.player1.as_deref().unwrap_or("?"),
            m.player2.as_deref().unwrap_or("?")
        );
    }

    println!("OK — arbre coupe 3 corrigé.");
    Ok(())
}

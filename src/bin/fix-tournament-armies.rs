use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::Parser;
use poissonnerie_elo::army::default_db_path;
use poissonnerie_elo::fix_tournament_player_army_opts;

#[derive(Parser, Debug)]
#[command(
    name = "poissonnerie-fix-tournament-armies",
    about = "Corrige les armées d'inscription et de matchs pour un tournoi"
)]
struct Args {
    /// ID du tournoi (ex. 7 pour la coupe 5)
    tournament_id: i64,

    /// Correction « Joueur:army_id » (répétable)
    #[arg(long = "fix")]
    fixes: Vec<String>,

    /// Ne corrige que les matchs d'arbre (pas l'inscription ni les poules)
    #[arg(long)]
    bracket_only: bool,

    #[arg(long, default_value_os_t = default_db_path())]
    db: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.fixes.is_empty() {
        bail!("indiquez au moins une correction --fix \"Joueur:army_id\"");
    }

    for fix in &args.fixes {
        let (player, army_raw) = fix
            .split_once(':')
            .with_context(|| format!("format invalide (attendu Joueur:army_id) : {fix}"))?;
        let army_id: u32 = army_raw
            .trim()
            .parse()
            .with_context(|| format!("army_id invalide : {army_raw}"))?;
        let report = fix_tournament_player_army_opts(
            &args.db,
            args.tournament_id,
            player,
            army_id,
            args.bracket_only,
        )?;
        println!(
            "✓ {} → army {} (reg={}, tm={}, m={}{})",
            report.player_name,
            report.army_id,
            report.registration_updated,
            report.tournament_matches_updated,
            report.matches_updated,
            if args.bracket_only { ", bracket-only" } else { "" },
        );
    }

    Ok(())
}

use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::Parser;
use poissonnerie_elo::{army::default_db_path, merge_players, DEFAULT_K_FACTOR};

#[derive(Parser, Debug)]
#[command(
    name = "poissonnerie-merge-players",
    about = "Fusionne des doublons de joueurs et recalcule l'ELO"
)]
struct Args {
    /// Nom canonique à conserver
    #[arg(long)]
    keep: String,

    /// Alias à fusionner dans --keep (répétable)
    #[arg(long = "alias", required = true)]
    aliases: Vec<String>,

    #[arg(long, default_value_os_t = default_db_path())]
    db: PathBuf,

    #[arg(long, default_value_t = DEFAULT_K_FACTOR)]
    k: f64,
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.aliases.is_empty() {
        bail!("indiquez au moins un --alias");
    }

    let alias_refs: Vec<&str> = args.aliases.iter().map(String::as_str).collect();
    let report = merge_players(&args.db, &args.keep, &alias_refs, args.k)?;

    println!(
        "Fusion OK → « {} » (aliases : {})",
        report.keep_name,
        report.merged_aliases.join(", ")
    );
    println!(
        "Matchs réécrits : {} | ELO {:.1} | {}V / {}N / {}D",
        report.matches_rewritten,
        report.rating,
        report.wins,
        report.draws,
        report.losses
    );
    Ok(())
}

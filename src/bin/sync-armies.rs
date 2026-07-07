use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use poissonnerie_elo::army::{sync_armies, ArmyStore};

#[derive(Parser)]
#[command(
    name = "poissonnerie-sync-armies",
    about = "Synchronise les armées Infinity depuis l'API officielle vers la base SQLite"
)]
struct Args {
    /// Base SQLite (armées, joueurs et matchs)
    #[arg(long, default_value = "data/poissonnerie.db")]
    db: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let store = ArmyStore::open(&args.db)?;
    let report = sync_armies(&store)?;

    println!("Synchronisation terminée.");
    println!("  Factions récupérées     : {}", report.fetched);
    println!("  Sectorielles x99 ignorées : {}", report.skipped_reinforcement);
    println!("  Armées enregistrées     : {}", report.stored);
    println!("  Armées sélectionnables  : {}", report.selectable);
    println!("Base : {}", args.db.display());

    Ok(())
}

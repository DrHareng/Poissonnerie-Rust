use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use poissonnerie_elo::army::default_db_path;
use poissonnerie_elo::tts_figurine_isc_fix::{fix_figurine_transmute_iscs, print_report};

#[derive(Parser, Debug)]
#[command(
    name = "poissonnerie-fix-tts-figurine-iscs",
    about = "Remappe les ISC courts des figurines de transmute (COMBAT FORM → Unité COMBAT FORM). Dry-run par défaut."
)]
struct Args {
    /// Base SQLite (prod : chemin de la DB déployée).
    #[arg(long, default_value_os_t = default_db_path())]
    db: PathBuf,

    /// Écrit les corrections. Sans ce flag : rapport seulement.
    #[arg(long, default_value_t = false)]
    apply: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    println!("Base : {}", args.db.display());
    let report = fix_figurine_transmute_iscs(&args.db, args.apply)?;
    print_report(&report, args.apply);
    if !report.conflicts.is_empty() {
        println!();
        println!("Corrige les conflits à la main dans l’UI figs 3D, puis relance si besoin.");
    }
    Ok(())
}

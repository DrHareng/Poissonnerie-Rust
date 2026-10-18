use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use poissonnerie_elo::tts_profile_army::import_from_army_api;
use rusqlite::Connection;

#[derive(Parser)]
#[command(
    name = "poissonnerie-import-tts-profiles",
    about = "Importe les profils depuis l'API Infinity Army et conserve les meshes TTS"
)]
struct Args {
    /// Sauvegarde TTS utilisée seulement pour retrouver les meshes.
    #[arg(long, default_value = "data/tts/infinity-models.json")]
    input: PathBuf,

    /// Base SQLite.
    #[arg(long, default_value = "data/poissonnerie.db")]
    db: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let conn = Connection::open(&args.db)?;
    let mesh = args.input.exists().then_some(args.input.as_path());
    let report = import_from_army_api(&conn, mesh)?;

    println!("Import des profils Infinity Army terminé.");
    println!("  Sectorielles lues     : {}", report.armies);
    println!("  Sectorielles en échec : {}", report.failed_armies);
    println!("  Profils enregistrés   : {}", report.profiles);
    println!("  Formes secondaires    : {}", report.forms);
    println!("  Liens de sectorielle  : {}", report.sectorials);
    println!("Base : {}", args.db.display());
    Ok(())
}

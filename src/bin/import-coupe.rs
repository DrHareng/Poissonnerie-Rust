use anyhow::{bail, Context, Result};
use clap::Parser;

use poissonnerie_elo::import_coupe::{coupe_config, run_import, ImportCoupeArgs};

fn main() -> Result<()> {
    let args = ImportCoupeArgs::parse();
    if args.coupe < 5 || args.coupe > 10 {
        bail!("--coupe doit être entre 5 et 10");
    }
    let config = coupe_config(args.coupe).context("configuration de coupe introuvable")?;
    run_import(&args, config)
}

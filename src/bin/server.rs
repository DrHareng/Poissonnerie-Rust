use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use clap::Parser;
use poissonnerie_elo::{api, ArmyListStore, ArmyStore, AuthConfig, Leaderboard, UserStore, DEFAULT_K_FACTOR};

#[derive(Parser)]
#[command(name = "poissonnerie-server", about = "API REST pour le classement ELO")]
struct Args {
    /// Adresse d'écoute
    #[arg(long, default_value = "127.0.0.1:3000")]
    listen: SocketAddr,

    /// Base SQLite (joueurs, matchs et armées)
    #[arg(long, default_value = "data/poissonnerie.db")]
    db: PathBuf,

    /// Facteur K du système ELO
    #[arg(long, default_value_t = DEFAULT_K_FACTOR)]
    k: f64,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let args = Args::parse();
    let board = Leaderboard::load(&args.db)?;
    let armies = ArmyStore::open(&args.db)?;
    let army_lists = ArmyListStore::open(&args.db)?;
    let users = UserStore::open(&args.db)?;
    let auth = match AuthConfig::from_env() {
        Ok(config) => {
            println!("Authentification Discord activée");
            Some(config)
        }
        Err(error) => {
            eprintln!("Authentification Discord désactivée : {error}");
            None
        }
    };

    let state = api::AppState {
        board: Arc::new(Mutex::new(board)),
        armies: Arc::new(armies),
        army_lists: Arc::new(army_lists),
        users: Arc::new(users),
        tournaments: Arc::new(poissonnerie_elo::TournamentStore::open(&args.db)?),
        scenarios: Arc::new(poissonnerie_elo::ScenarioStore::open(&args.db)?),
        report_templates: Arc::new(poissonnerie_elo::ReportTemplateStore::open(&args.db)?),
        site_content: Arc::new(poissonnerie_elo::SiteContentStore::open(&args.db)?),
        auth,
        db_path: args.db,
        k_factor: args.k,
    };

    let app = api::router(state)?;
    let listener = tokio::net::TcpListener::bind(args.listen).await?;
    println!("API ELO disponible sur http://{}", args.listen);
    axum::serve(listener, app).await?;

    Ok(())
}

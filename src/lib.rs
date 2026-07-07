pub mod display_name;
pub mod api;
pub mod army;
pub mod auth;
pub mod elo;
pub mod match_record;
pub mod migrate;
pub mod player;
pub mod scenario;
pub mod store;
pub mod tournament;
pub mod tournament_api;
pub mod tournament_store;
pub mod user;

pub use api::{default_state, router, AppState};
pub use army::{default_db_path, Army, ArmyStore};
pub use auth::AuthConfig;
pub use elo::{expected_score, update_ratings, MatchScore, DEFAULT_K_FACTOR};
pub use match_record::{MatchRecord, MatchScores};
pub use player::{apply_match, MatchOutcome, Player, RatingUpdate, DEFAULT_RATING};
pub use scenario::{Scenario, ScenarioStore};
pub use store::{ArmyMatchStats, Leaderboard, PlayerArmyUsage};
pub use tournament::{
    BracketFormat, PlayerTournamentResult, Pool, Tournament, TournamentDetail,
    TournamentMatch, TournamentRegistration, TournamentStatus,
};
pub use tournament_store::TournamentStore;
pub use user::User;
pub use user::UserStore;

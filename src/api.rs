use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::{
    extract::{Path, Query, State},
    http::{HeaderValue, Method, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tower_http::cors::{AllowHeaders, AllowMethods, AllowOrigin, CorsLayer};
use tower_sessions::{MemoryStore, Session, SessionManagerLayer};

use crate::{
    auth::{self, AuthConfig, CallbackQuery},
    default_db_path, scenario::ScenarioStore, tournament_api, ArmyStore, Leaderboard,
    MatchOutcome, MatchRecord, MatchScores, Player, TournamentStore, User,
    UserStore, DEFAULT_K_FACTOR,
};
use crate::user::{LocalProfileUpdate, UserResponse};

#[derive(Clone)]
pub struct AppState {
    pub board: Arc<Mutex<Leaderboard>>,
    pub armies: Arc<ArmyStore>,
    pub users: Arc<UserStore>,
    pub tournaments: Arc<TournamentStore>,
    pub scenarios: Arc<ScenarioStore>,
    pub auth: Option<AuthConfig>,
    pub db_path: PathBuf,
    pub k_factor: f64,
}

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct AuthUserResponse {
    pub user: UserResponse,
    pub player: Option<Player>,
}

#[derive(Debug, Serialize)]
pub struct PlayerProfileResponse {
    #[serde(flatten)]
    pub player: Player,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub profile_display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discord_display_name: Option<String>,
    pub is_own_profile: bool,
}

#[derive(Debug, Deserialize)]
struct AddPlayerRequest {
    name: String,
    discord_username: String,
}

#[derive(Debug, Deserialize)]
struct RecordMatchRequest {
    player1: String,
    player2: String,
    outcome: MatchOutcome,
    #[serde(default)]
    player1_objectives: u8,
    #[serde(default)]
    player1_survivors: u16,
    #[serde(default)]
    player2_objectives: u8,
    #[serde(default)]
    player2_survivors: u16,
    #[serde(default)]
    player1_army_id: Option<u32>,
    #[serde(default)]
    player2_army_id: Option<u32>,
    #[serde(default)]
    scenario_id: Option<i64>,
    #[serde(default)]
    scenario_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MatchListQuery {
    #[serde(default = "default_match_limit")]
    limit: usize,
}

#[derive(Debug, Deserialize)]
struct UpdateProfileRequest {
    #[serde(default)]
    local_display_name: Option<String>,
    #[serde(default)]
    local_avatar_url: Option<String>,
    #[serde(default)]
    clear_local_display_name: bool,
    #[serde(default)]
    clear_local_avatar_url: bool,
}

fn default_match_limit() -> usize {
    20
}

impl ApiError {
    pub fn bad_request(error: impl Into<String>) -> Self {
        Self {
            error: error.into(),
        }
    }

    pub fn unauthorized(error: impl Into<String>) -> Self {
        Self {
            error: error.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (StatusCode::BAD_REQUEST, Json(self)).into_response()
    }
}

fn cors_layer() -> CorsLayer {
    let origin = std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://127.0.0.1:5173".into());
    CorsLayer::new()
        .allow_origin(AllowOrigin::exact(
            origin.parse::<HeaderValue>().expect("FRONTEND_URL invalide"),
        ))
        .allow_credentials(true)
        .allow_methods(AllowMethods::list([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
        ]))
        .allow_headers(AllowHeaders::mirror_request())
}

pub fn router(state: AppState) -> Router {
    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store).with_secure(false);

    Router::new()
        .route("/api/auth/discord", get(discord_login))
        .route("/api/auth/callback", get(discord_callback))
        .route("/api/auth/me", get(auth_me).patch(update_profile))
        .route("/api/auth/logout", post(auth_logout))
        .route("/api/armies", get(list_armies))
        .route("/api/armies/ranking", get(get_army_ranking))
        .route("/api/armies/{id}/matches", get(get_army_matches))
        .route("/api/armies/{id}", get(get_army))
        .route("/api/ranking", get(get_ranking))
        .route("/api/players", post(add_player))
        .route("/api/players/{name}", get(get_player))
        .route("/api/players/{name}/matches", get(get_player_matches))
        .route("/api/matches", get(list_matches).post(record_match))
        .route("/api/health", get(health))
        .merge(tournament_api::tournament_routes())
        .layer(cors_layer())
        .layer(session_layer)
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}

async fn discord_login(State(state): State<AppState>) -> Result<Redirect, ApiError> {
    let auth = state
        .auth
        .as_ref()
        .ok_or_else(|| ApiError::bad_request("authentification Discord non configurée"))?;
    Ok(Redirect::to(&auth.authorize_url()))
}

async fn discord_callback(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<CallbackQuery>,
) -> Result<Redirect, ApiError> {
    let auth = state
        .auth
        .as_ref()
        .ok_or_else(|| ApiError::bad_request("authentification Discord non configurée"))?;

    auth::login_with_code(auth, state.users.as_ref(), &session, &query.code)
        .await
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    Ok(Redirect::to(&auth.frontend_url))
}

async fn auth_me(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<AuthUserResponse>, (StatusCode, Json<ApiError>)> {
    let user = auth::current_user(state.users.as_ref(), &session)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::bad_request(error.to_string())),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(ApiError::unauthorized("non authentifié")),
            )
        })?;

    let player = {
        let board = state.board.lock().unwrap();
        board
            .get_player_by_discord_username(&user.username)
            .cloned()
    };

    Ok(Json(AuthUserResponse {
        user: user.into(),
        player,
    }))
}

async fn update_profile(
    State(state): State<AppState>,
    session: Session,
    Json(payload): Json<UpdateProfileRequest>,
) -> Result<Json<UserResponse>, ApiError> {
    let user = require_user(&state, &session).await?;

    let local_display_name = if payload.clear_local_display_name {
        Some(None)
    } else if payload.local_display_name.is_some() {
        Some(payload.local_display_name)
    } else {
        None
    };

    let local_avatar_url = if payload.clear_local_avatar_url {
        Some(None)
    } else if payload.local_avatar_url.is_some() {
        Some(payload.local_avatar_url)
    } else {
        None
    };

    let updated = state
        .users
        .update_local_profile(
            user.id,
            LocalProfileUpdate {
                local_display_name,
                local_avatar_url,
            },
        )
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    Ok(Json(updated.into()))
}

async fn auth_logout(session: Session) -> Result<StatusCode, ApiError> {
    auth::logout(&session)
        .await
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn require_user(state: &AppState, session: &Session) -> Result<User, ApiError> {
    auth::current_user(state.users.as_ref(), session)
        .await
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .ok_or_else(|| ApiError::unauthorized("non authentifié"))
}

async fn require_admin(state: &AppState, session: &Session) -> Result<User, ApiError> {
    let user = require_user(state, session).await?;
    if !user.is_admin {
        return Err(ApiError::unauthorized("droits administrateur requis"));
    }
    Ok(user)
}

async fn list_armies(State(state): State<AppState>) -> Result<Json<Vec<crate::Army>>, ApiError> {
    state
        .armies
        .list_selectable()
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

#[derive(Debug, Serialize)]
struct RankedArmy {
    rank: usize,
    army_id: u32,
    wins: u32,
    draws: u32,
    losses: u32,
    win_rate: f64,
}

async fn get_ranking(
    State(state): State<AppState>,
) -> Result<Json<Vec<tournament_api::RankedPlayerWithStars>>, ApiError> {
    let ranking = tournament_api::ranking_with_stars(&state).await?;
    Ok(Json(ranking))
}

async fn get_army_ranking(State(state): State<AppState>) -> Result<Json<Vec<RankedArmy>>, ApiError> {
    let board = state.board.lock().unwrap();
    let ranking = board
        .army_ranking()
        .into_iter()
        .enumerate()
        .map(|(index, stats)| RankedArmy {
            rank: index + 1,
            army_id: stats.army_id,
            wins: stats.wins,
            draws: stats.draws,
            losses: stats.losses,
            win_rate: stats.win_rate(),
        })
        .collect();
    Ok(Json(ranking))
}

async fn get_army(
    State(state): State<AppState>,
    Path(id): Path<u32>,
) -> Result<Json<RankedArmy>, ApiError> {
    state
        .armies
        .get(id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .ok_or_else(|| ApiError::bad_request(format!("sectorielle introuvable : {}", id)))?;

    let board = state.board.lock().unwrap();
    let ranking = board.army_ranking();
    let Some((index, stats)) = ranking
        .iter()
        .enumerate()
        .find(|(_, entry)| entry.army_id == id)
    else {
        return Err(ApiError::bad_request(
            "aucune partie enregistrée pour cette sectorielle",
        ));
    };

    Ok(Json(RankedArmy {
        rank: index + 1,
        army_id: stats.army_id,
        wins: stats.wins,
        draws: stats.draws,
        losses: stats.losses,
        win_rate: stats.win_rate(),
    }))
}

async fn get_army_matches(
    State(state): State<AppState>,
    Path(id): Path<u32>,
    Query(query): Query<MatchListQuery>,
) -> Result<Json<Vec<crate::display_name::EnrichedMatchRecord>>, ApiError> {
    state
        .armies
        .get(id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .ok_or_else(|| ApiError::bad_request(format!("sectorielle introuvable : {}", id)))?;

    let board = state.board.lock().unwrap();
    let resolver = crate::display_name::PlayerDisplayResolver::new(&board, state.users.as_ref());
    let limit = query.limit.clamp(1, 100);
    let matches = board
        .army_matches(id, limit)
        .into_iter()
        .cloned()
        .map(|record| resolver.enrich_match(record))
        .collect();
    Ok(Json(matches))
}

async fn list_matches(
    State(state): State<AppState>,
    Query(query): Query<MatchListQuery>,
) -> Json<Vec<crate::display_name::EnrichedMatchRecord>> {
    let board = state.board.lock().unwrap();
    let resolver = crate::display_name::PlayerDisplayResolver::new(&board, state.users.as_ref());
    let limit = query.limit.clamp(1, 100);
    let matches = board
        .recent_matches(limit)
        .into_iter()
        .cloned()
        .map(|record| resolver.enrich_match(record))
        .collect();
    Json(matches)
}

async fn get_player(
    State(state): State<AppState>,
    session: Session,
    Path(name): Path<String>,
) -> Result<Json<PlayerProfileResponse>, ApiError> {
    let viewer = auth::current_user(state.users.as_ref(), &session)
        .await
        .ok()
        .flatten();

    let board = state.board.lock().unwrap();
    let player = board
        .get_player(&name)
        .map(|player| player.clone())
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    Ok(Json(build_player_profile(
        state.users.as_ref(),
        &board,
        &player,
        viewer.as_ref(),
    )))
}

fn build_player_profile(
    users: &UserStore,
    board: &Leaderboard,
    player: &Player,
    viewer: Option<&User>,
) -> PlayerProfileResponse {
    let resolver = crate::display_name::PlayerDisplayResolver::new(board, users);
    let display_name = resolver.resolve_player(player);

    let mut response = PlayerProfileResponse {
        player: player.clone(),
        display_name: display_name.clone(),
        avatar_url: None,
        profile_display_name: Some(display_name),
        discord_display_name: None,
        is_own_profile: false,
    };

    let Some(discord_username) = player.discord_username.as_deref() else {
        return response;
    };

    let linked_user = users
        .get_by_username(discord_username)
        .ok()
        .flatten();

    let Some(linked_user) = linked_user else {
        return response;
    };

    response.avatar_url = Some(linked_user.effective_avatar_url().to_string());
    response.profile_display_name = Some(linked_user.effective_display_name().to_string());
    response.display_name = linked_user.effective_display_name().to_string();

    if let Some(viewer) = viewer {
        if let Some(viewer_player) = board.get_player_by_discord_username(&viewer.username) {
            response.is_own_profile =
                viewer_player.name.eq_ignore_ascii_case(&player.name);
        }

        if viewer.is_admin && linked_user.has_local_display_name() {
            response.discord_display_name = Some(linked_user.display_name.clone());
        }
    }

    response
}

async fn get_player_matches(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(query): Query<MatchListQuery>,
) -> Result<Json<Vec<crate::display_name::EnrichedMatchRecord>>, ApiError> {
    let board = state.board.lock().unwrap();
    let resolver = crate::display_name::PlayerDisplayResolver::new(&board, state.users.as_ref());
    let limit = query.limit.clamp(1, 100);
    let matches = board
        .player_matches(&name, limit)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .into_iter()
        .cloned()
        .map(|record| resolver.enrich_match(record))
        .collect();
    Ok(Json(matches))
}

async fn add_player(
    State(state): State<AppState>,
    session: Session,
    Json(payload): Json<AddPlayerRequest>,
) -> Result<(StatusCode, Json<Player>), ApiError> {
    require_admin(&state, &session).await?;

    let name = payload.name.trim().to_string();
    let discord_username = payload.discord_username.trim().to_string();

    if name.is_empty() {
        return Err(ApiError::bad_request("indiquez un pseudo"));
    }
    if discord_username.is_empty() {
        return Err(ApiError::bad_request("indiquez un pseudo Discord"));
    }

    let mut board = state.board.lock().unwrap();
    board
        .add_player_for_discord_username(&name, &discord_username)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let player = board.get_player(&name).unwrap().clone();
    board
        .save(&state.db_path)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    Ok((StatusCode::CREATED, Json(player)))
}

async fn record_match(
    State(state): State<AppState>,
    session: Session,
    Json(payload): Json<RecordMatchRequest>,
) -> Result<Json<MatchRecord>, ApiError> {
    require_user(&state, &session).await?;

    if let Some(army_id) = payload.player1_army_id {
        state
            .armies
            .validate_selectable_id(army_id)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
    }
    if let Some(army_id) = payload.player2_army_id {
        state
            .armies
            .validate_selectable_id(army_id)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
    }

    let scenario_id = if let Some(name) = payload.scenario_name.as_deref() {
        Some(state.scenarios.get_or_create(name).map(|s| s.id).map_err(
            |error| ApiError::bad_request(error.to_string()),
        )?)
    } else if let Some(id) = payload.scenario_id {
        state
            .scenarios
            .increment_usage(id)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        Some(id)
    } else {
        None
    };

    let mut board = state.board.lock().unwrap();
    let scores = MatchScores {
        player1_objectives: payload.player1_objectives,
        player1_survivors: payload.player1_survivors,
        player2_objectives: payload.player2_objectives,
        player2_survivors: payload.player2_survivors,
    };
    let record = board
        .record_match(
            &payload.player1,
            &payload.player2,
            payload.outcome,
            state.k_factor,
            scores,
            payload.player1_army_id,
            payload.player2_army_id,
            scenario_id,
            payload.scenario_name,
        )
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    board
        .save(&state.db_path)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    Ok(Json(record))
}

pub fn default_state() -> anyhow::Result<AppState> {
    let db_path = default_db_path();
    let board = Leaderboard::load(&db_path)?;
    let armies = ArmyStore::open(&db_path)?;
    let users = UserStore::open(&db_path)?;
    let tournaments = TournamentStore::open(&db_path)?;
    let scenarios = ScenarioStore::open(&db_path)?;
    let auth = AuthConfig::from_env().ok();
    Ok(AppState {
        board: Arc::new(Mutex::new(board)),
        armies: Arc::new(armies),
        users: Arc::new(users),
        tournaments: Arc::new(tournaments),
        scenarios: Arc::new(scenarios),
        auth,
        db_path,
        k_factor: DEFAULT_K_FACTOR,
    })
}

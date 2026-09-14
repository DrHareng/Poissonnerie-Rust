use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use axum::{
    extract::{Path, Query, Request, State},
    http::{header, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{delete, get, patch, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use time::Duration;
use tower_http::cors::{AllowHeaders, AllowMethods, AllowOrigin, CorsLayer};
use tower_sessions::{cookie::SameSite, Expiry, Session, SessionManagerLayer};

use crate::{
    auth::{self, AuthConfig, CallbackQuery},
    dauphine_api, default_db_path, scenario::ScenarioStore, session_store::SqliteSessionStore,
    tournament_api, ArmyListStore, ArmyStore, DauphineStore, Leaderboard, MatchOutcome,
    MatchRecord, MatchScores, Player, ReportStatus, ReportTemplateStore, SiteContentStore,
    TournamentStore, User, UserStore, DEFAULT_K_FACTOR, RESSOURCES_KEY,
};
use crate::army_list_store::ArmyListStatsGroup;
use crate::tournament::TournamentStatus;
use crate::user::{LocalProfileUpdate, UiPrefsUpdate, UserResponse};

const SESSION_INACTIVITY_DAYS: i64 = 30;

#[derive(Clone)]
pub struct AppState {
    pub board: Arc<Mutex<Leaderboard>>,
    pub armies: Arc<ArmyStore>,
    pub army_lists: Arc<ArmyListStore>,
    pub users: Arc<UserStore>,
    pub tournaments: Arc<TournamentStore>,
    pub dauphine: Arc<DauphineStore>,
    pub scenarios: Arc<ScenarioStore>,
    pub report_templates: Arc<ReportTemplateStore>,
    pub site_content: Arc<SiteContentStore>,
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
struct LinkPlayerAccountRequest {
    player_name: String,
    #[serde(default)]
    user_id: Option<i64>,
}

#[derive(Debug, Serialize)]
struct AdminAccountsResponse {
    users: Vec<AdminUserEntry>,
    players: Vec<AdminPlayerEntry>,
}

#[derive(Debug, Serialize)]
struct AdminUserEntry {
    #[serde(flatten)]
    user: UserResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    player_name: Option<String>,
}

#[derive(Debug, Serialize)]
struct AdminPlayerEntry {
    #[serde(flatten)]
    player: Player,
    #[serde(skip_serializing_if = "Option::is_none")]
    linked_user: Option<UserResponse>,
    pub can_delete: bool,
}

#[derive(Debug, Deserialize)]
struct MergePlayersRequest {
    keep: String,
    alias: String,
}

#[derive(Debug, Deserialize)]
struct ClaimPlayerRequest {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StartMatchRequest {
    player1: String,
    #[serde(default)]
    player2: String,
    #[serde(default)]
    adversaire: Option<String>,
    player1_army_id: u32,
    player2_army_id: u32,
    player1_secondary_slugs: Vec<String>,
    player2_secondary_slugs: Vec<String>,
    /// `true` = match classé (impact ELO). Défaut : amical.
    #[serde(default)]
    counts_for_elo: bool,
    #[serde(default)]
    client_uuid: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateMatchProgressRequest {
    #[serde(default)]
    scenario_id: Option<i64>,
    #[serde(default)]
    scenario_other: Option<String>,
    #[serde(default)]
    scenario_url: Option<String>,
    #[serde(default)]
    player1_secondary_slugs: Option<Vec<String>>,
    #[serde(default)]
    player2_secondary_slugs: Option<Vec<String>>,
    #[serde(default)]
    secondary_pool_slugs: Option<Vec<String>>,
    #[serde(default)]
    player1_chosen_secondary: Option<String>,
    #[serde(default)]
    player2_chosen_secondary: Option<String>,
    #[serde(default)]
    lieutenant_winner: Option<String>,
    #[serde(default)]
    lieutenant_winner_choice: Option<String>,
    #[serde(default)]
    lieutenant_other_choice: Option<String>,
    #[serde(default)]
    partie_step: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CompleteMatchRequest {
    outcome: MatchOutcome,
    #[serde(default)]
    player1_objectives: u8,
    #[serde(default)]
    player1_survivors: u16,
    #[serde(default)]
    player2_objectives: u8,
    #[serde(default)]
    player2_survivors: u16,
}

#[derive(Debug, Deserialize)]
struct SyncPartieRequest {
    client_uuid: String,
    player1: String,
    #[serde(default)]
    player2: String,
    #[serde(default)]
    adversaire: Option<String>,
    player1_army_id: u32,
    player2_army_id: u32,
    #[serde(default)]
    player1_secondary_slugs: Vec<String>,
    #[serde(default)]
    player2_secondary_slugs: Vec<String>,
    /// `true` = match classé (impact ELO). Défaut : amical.
    #[serde(default)]
    counts_for_elo: bool,
    #[serde(default)]
    scenario_id: Option<i64>,
    #[serde(default)]
    scenario_other: Option<String>,
    #[serde(default)]
    scenario_url: Option<String>,
    #[serde(default)]
    secondary_pool_slugs: Option<Vec<String>>,
    #[serde(default)]
    player1_chosen_secondary: Option<String>,
    #[serde(default)]
    player2_chosen_secondary: Option<String>,
    #[serde(default)]
    lieutenant_winner: Option<String>,
    #[serde(default)]
    lieutenant_winner_choice: Option<String>,
    #[serde(default)]
    lieutenant_other_choice: Option<String>,
    #[serde(default)]
    partie_step: Option<String>,
    #[serde(default)]
    complete: Option<CompleteMatchRequest>,
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
    scenario_other: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MatchListQuery {
    #[serde(default = "default_match_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
}

#[derive(Debug, Serialize)]
struct MatchListResponse {
    items: Vec<crate::display_name::EnrichedMatchRecord>,
    total: usize,
    limit: usize,
    offset: usize,
}

#[derive(Debug, Serialize)]
struct ReportListResponse {
    items: Vec<EnrichedRecentMatchReport>,
    total: usize,
    limit: usize,
    offset: usize,
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

#[derive(Debug, Serialize, Deserialize)]
struct PrefsResponse {
    secondary_view_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    scenario_slug: Option<String>,
    army_sort_mode: String,
    player_sort_mode: String,
    tournament_completed_view_mode: String,
}

#[derive(Debug, Deserialize)]
struct UpdatePrefsRequest {
    #[serde(default)]
    secondary_view_mode: Option<String>,
    #[serde(default)]
    scenario_slug: Option<String>,
    #[serde(default)]
    army_sort_mode: Option<String>,
    #[serde(default)]
    player_sort_mode: Option<String>,
    #[serde(default)]
    tournament_completed_view_mode: Option<String>,
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
    let frontend = std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://127.0.0.1:5173".into());
    let extra = std::env::var("CORS_ORIGINS").unwrap_or_default();
    let mut origins = Vec::new();
    for raw in extra
        .split(',')
        .chain(std::iter::once(frontend.as_str()))
        .chain([
            "https://localhost",
            "http://localhost",
            "capacitor://localhost",
        ])
    {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(value) = trimmed.parse::<HeaderValue>() {
            origins.push(value);
        }
    }
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_credentials(true)
        .allow_methods(AllowMethods::list([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ]))
        .allow_headers(AllowHeaders::mirror_request())
}

const NATIVE_SESSION_HEADER: &str = "x-poissonnerie-session";
const SESSION_COOKIE_NAME: &str = "id";

async fn inject_session_from_header(mut request: Request, next: Next) -> Response {
    let has_cookie = request.headers().get(header::COOKIE).is_some();
    if !has_cookie {
        if let Some(session_id) = request
            .headers()
            .get(NATIVE_SESSION_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if session_id
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '='))
            {
                if let Ok(cookie) =
                    HeaderValue::from_str(&format!("{SESSION_COOKIE_NAME}={session_id}"))
                {
                    request.headers_mut().insert(header::COOKIE, cookie);
                }
            }
        }
    }
    next.run(request).await
}

pub fn router(state: AppState) -> Result<Router> {
    let session_store = SqliteSessionStore::open(&state.db_path)?;
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false)
        .with_path("/")
        // Lax est requis pour OAuth Discord (retour cross-site en top-level GET).
        .with_same_site(SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(Duration::days(SESSION_INACTIVITY_DAYS)));

    Ok(Router::new()
        .route("/api/auth/discord", get(discord_login))
        .route("/api/auth/callback", get(discord_callback))
        .route("/api/auth/native-return", get(native_auth_return))
        .route("/api/auth/me", get(auth_me).patch(update_profile))
        .route("/api/auth/logout", post(auth_logout))
        .route("/api/prefs", get(get_prefs).patch(update_prefs))
        .route("/api/armies", get(list_armies))
        .route("/api/armies/ranking", get(get_army_ranking))
        .route("/api/armies/{id}/matches", get(get_army_matches))
        .route("/api/armies/{id}/players", get(get_army_players))
        .route("/api/armies/{id}", get(get_army))
        .route("/api/army-lists", get(list_army_lists))
        .route("/api/army-lists/armies", get(list_army_list_armies))
        .route("/api/army-lists/{id}/matches", get(get_army_list_matches))
        .route("/api/ranking", get(get_ranking))
        .route("/api/players", post(add_player))
        .route("/api/players/me", post(claim_player))
        .route("/api/admin/accounts", get(admin_accounts))
        .route("/api/admin/player-link", post(link_player_account))
        .route("/api/admin/players/merge", post(merge_player_accounts))
        .route("/api/admin/players/{name}", delete(delete_unused_player_account))
        .route("/api/players/{name}", get(get_player))
        .route("/api/players/{name}/armies", get(get_player_armies))
        .route("/api/players/{name}/matches", get(get_player_matches))
        .route("/api/matches", get(list_matches).post(record_match))
        .route("/api/matches/start", post(start_match))
        .route("/api/matches/sync", post(sync_partie))
        .route("/api/matches/mine/in-progress", get(list_my_in_progress_matches))
        .route(
            "/api/matches/{id}",
            get(get_match).delete(delete_match).patch(update_match_progress),
        )
        .route("/api/matches/{id}/complete", post(complete_match))
        .route("/api/matches/{id}/correct", post(correct_match))
        .route("/api/matches/{id}/report", patch(update_match_report))
        .route("/api/matches/{id}/army-list", patch(update_match_army_list))
        .route("/api/reports/recent", get(list_recent_reports))
        .route(
            "/api/me/report-templates",
            get(list_report_templates).post(create_report_template),
        )
        .route(
            "/api/me/report-templates/{id}",
            patch(update_report_template).delete(delete_report_template),
        )
        .route("/api/health", get(health))
        .route("/api/ressources", get(get_ressources).patch(update_ressources))
        .merge(tournament_api::tournament_routes())
        .merge(dauphine_api::dauphine_routes())
        .layer(cors_layer())
        .layer(session_layer)
        .layer(middleware::from_fn(inject_session_from_header))
        .with_state(state))
}

async fn health() -> &'static str {
    "ok"
}

#[derive(Debug, Serialize)]
struct RessourcesResponse {
    body_md: String,
    updated_at: u64,
}

#[derive(Debug, Deserialize)]
struct UpdateRessourcesRequest {
    body_md: String,
}

async fn get_ressources(
    State(state): State<AppState>,
) -> Result<Json<RessourcesResponse>, ApiError> {
    let content = state
        .site_content
        .get(RESSOURCES_KEY)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(RessourcesResponse {
        body_md: content.body_md,
        updated_at: content.updated_at,
    }))
}

async fn update_ressources(
    State(state): State<AppState>,
    session: Session,
    Json(payload): Json<UpdateRessourcesRequest>,
) -> Result<Json<RessourcesResponse>, ApiError> {
    require_admin(&state, &session).await?;
    let content = state
        .site_content
        .update(RESSOURCES_KEY, &payload.body_md)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(RessourcesResponse {
        body_md: content.body_md,
        updated_at: content.updated_at,
    }))
}

#[derive(Debug, Deserialize)]
struct DiscordLoginQuery {
    #[serde(default)]
    mobile: String,
}

async fn discord_login(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<DiscordLoginQuery>,
) -> Result<Redirect, ApiError> {
    let auth = state
        .auth
        .as_ref()
        .ok_or_else(|| ApiError::bad_request("authentification Discord non configurée"))?;
    let mobile = matches!(query.mobile.to_ascii_lowercase().as_str(), "1" | "true" | "yes");
    session
        .insert(auth::SESSION_OAUTH_MOBILE, mobile)
        .await
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let oauth_state = mobile.then_some("mobile");
    Ok(Redirect::to(&auth.authorize_url_with_state(oauth_state)))
}

fn native_return_url(auth: &AuthConfig) -> Result<String, ApiError> {
    let base = auth
        .redirect_uri
        .strip_suffix("/auth/callback")
        .ok_or_else(|| {
            ApiError::bad_request(
                "DISCORD_REDIRECT_URI doit se terminer par /auth/callback pour le login mobile",
            )
        })?;
    Ok(format!("{base}/auth/native-return"))
}

fn native_handoff_html(session_id: &str) -> String {
    let encoded = urlencoding::encode(session_id);
    let deep_link = format!("poissonnerie://auth?session={encoded}");
    let intent_link = format!(
        "intent://auth?session={encoded}#Intent;scheme=poissonnerie;package=fr.poissonnerie.app;end"
    );
    let deep_json = serde_json::to_string(&deep_link).unwrap_or_else(|_| "\"\"".into());
    let intent_json = serde_json::to_string(&intent_link).unwrap_or_else(|_| "\"\"".into());
    format!(
        r#"<!DOCTYPE html>
<html lang="fr">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Retour a l'application</title>
  <style>
    body {{
      margin: 0;
      min-height: 100vh;
      display: grid;
      place-items: center;
      font-family: system-ui, sans-serif;
      background: #050505;
      color: #f5f5f5;
      text-align: center;
      padding: 1.5rem;
    }}
    a {{
      color: #7dd3fc;
      font-size: 1.1rem;
    }}
  </style>
  <script>
    (function () {{
      var deep = {deep_json};
      var intent = {intent_json};
      window.location.href = deep;
      setTimeout(function () {{ window.location.href = intent; }}, 400);
    }})();
  </script>
</head>
<body>
  <div>
    <p>Connexion reussie.</p>
    <p><a href="{deep_link}">Ouvrir La Poissonnerie</a></p>
  </div>
</body>
</html>"#
    )
}

async fn discord_callback(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<CallbackQuery>,
) -> Result<Response, ApiError> {
    let auth = state
        .auth
        .as_ref()
        .ok_or_else(|| ApiError::bad_request("authentification Discord non configurée"))?;

    auth::login_with_code(auth, state.users.as_ref(), &session, &query.code)
        .await
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let mobile_from_state = query.state.eq_ignore_ascii_case("mobile");
    let mobile_from_session = session
        .get::<bool>(auth::SESSION_OAUTH_MOBILE)
        .await
        .unwrap_or(None)
        .unwrap_or(false);
    let _ = session.remove::<bool>(auth::SESSION_OAUTH_MOBILE).await;
    if mobile_from_state || mobile_from_session {
        // Important: ne jamais mettre poissonnerie:// dans un header Location
        // (nginx peut répondre 502 "upstream sent invalid header").
        // On redirige d'abord vers une URL HTTP, puis la page ouvre le deep link.
        //
        // tower-sessions n'assigne l'id qu'au `save()` (pas juste à `insert`).
        session
            .save()
            .await
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        let session_id = session
            .id()
            .ok_or_else(|| ApiError::bad_request("session mobile introuvable"))?
            .to_string();
        let target = format!(
            "{}?session={}",
            native_return_url(auth)?,
            urlencoding::encode(&session_id)
        );
        return Ok(Redirect::to(&target).into_response());
    }

    Ok(Redirect::to(&auth.frontend_url).into_response())
}

#[derive(Debug, Deserialize)]
struct NativeReturnQuery {
    #[serde(default)]
    session: String,
}

async fn native_auth_return(
    session: Session,
    Query(query): Query<NativeReturnQuery>,
) -> Result<Response, ApiError> {
    let session_id = if !query.session.trim().is_empty() {
        query.session.trim().to_string()
    } else {
        session
            .id()
            .ok_or_else(|| {
                ApiError::bad_request("session mobile introuvable — réessayez depuis l'app")
            })?
            .to_string()
    };
    Ok(Html(native_handoff_html(&session_id)).into_response())
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

async fn get_prefs(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<PrefsResponse>, ApiError> {
    resolve_prefs(&state, &session).await.map(Json)
}

async fn update_prefs(
    State(state): State<AppState>,
    session: Session,
    Json(payload): Json<UpdatePrefsRequest>,
) -> Result<Json<PrefsResponse>, ApiError> {
    let mut ui_update = UiPrefsUpdate::default();

    if let Some(raw) = payload.secondary_view_mode.as_deref() {
        let mode = auth::parse_secondary_view_mode(raw)
            .ok_or_else(|| ApiError::bad_request("mode d'affichage invalide"))?;
        session
            .insert(auth::SESSION_SECONDARY_VIEW_MODE, mode.to_string())
            .await
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        ui_update.secondary_view_mode = Some(mode.to_string());
    }

    if let Some(raw) = payload.scenario_slug.as_deref() {
        let slug = auth::normalize_scenario_slug(raw)
            .ok_or_else(|| ApiError::bad_request("scénario invalide"))?;
        session
            .insert(auth::SESSION_SCENARIO_SLUG, slug.clone())
            .await
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        ui_update.scenario_slug = Some(slug);
    }

    if let Some(raw) = payload.army_sort_mode.as_deref() {
        let mode = auth::parse_army_sort_mode(raw)
            .ok_or_else(|| ApiError::bad_request("tri sectorielles invalide"))?;
        session
            .insert(auth::SESSION_ARMY_SORT_MODE, mode.to_string())
            .await
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        ui_update.army_sort_mode = Some(mode.to_string());
    }

    if let Some(raw) = payload.player_sort_mode.as_deref() {
        let mode = auth::parse_player_sort_mode(raw)
            .ok_or_else(|| ApiError::bad_request("tri joueurs invalide"))?;
        session
            .insert(auth::SESSION_PLAYER_SORT_MODE, mode.to_string())
            .await
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        ui_update.player_sort_mode = Some(mode.to_string());
    }

    if let Some(raw) = payload.tournament_completed_view_mode.as_deref() {
        let mode = auth::parse_tournament_completed_view_mode(raw)
            .ok_or_else(|| ApiError::bad_request("affichage tournois terminés invalide"))?;
        session
            .insert(
                auth::SESSION_TOURNAMENT_COMPLETED_VIEW_MODE,
                mode.to_string(),
            )
            .await
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        ui_update.tournament_completed_view_mode = Some(mode.to_string());
    }

    if ui_update.secondary_view_mode.is_none()
        && ui_update.scenario_slug.is_none()
        && ui_update.army_sort_mode.is_none()
        && ui_update.player_sort_mode.is_none()
        && ui_update.tournament_completed_view_mode.is_none()
    {
        return Err(ApiError::bad_request("aucune préférence à mettre à jour"));
    }

    if let Some(user) = auth::current_user(state.users.as_ref(), &session)
        .await
        .map_err(|error| ApiError::bad_request(error.to_string()))?
    {
        state
            .users
            .update_ui_prefs(user.id, ui_update)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
    }

    resolve_prefs(&state, &session).await.map(Json)
}

async fn resolve_prefs(state: &AppState, session: &Session) -> Result<PrefsResponse, ApiError> {
    let user = auth::current_user(state.users.as_ref(), session)
        .await
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let secondary_view_mode = if let Some(mode) = user
        .as_ref()
        .and_then(|u| u.secondary_view_mode.as_deref())
        .and_then(auth::parse_secondary_view_mode)
    {
        mode
    } else {
        let from_session: Option<String> = session
            .get(auth::SESSION_SECONDARY_VIEW_MODE)
            .await
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        from_session
            .as_deref()
            .and_then(auth::parse_secondary_view_mode)
            .unwrap_or(auth::DEFAULT_SECONDARY_VIEW_MODE)
    };

    let scenario_slug = if let Some(slug) = user
        .as_ref()
        .and_then(|u| u.scenario_slug.as_deref())
        .and_then(auth::normalize_scenario_slug)
    {
        Some(slug)
    } else {
        let from_session: Option<String> = session
            .get(auth::SESSION_SCENARIO_SLUG)
            .await
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        from_session
            .as_deref()
            .and_then(auth::normalize_scenario_slug)
    };

    let army_sort_mode = if let Some(mode) = user
        .as_ref()
        .and_then(|u| u.army_sort_mode.as_deref())
        .and_then(auth::parse_army_sort_mode)
    {
        mode
    } else {
        let from_session: Option<String> = session
            .get(auth::SESSION_ARMY_SORT_MODE)
            .await
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        from_session
            .as_deref()
            .and_then(auth::parse_army_sort_mode)
            .unwrap_or(auth::DEFAULT_ARMY_SORT_MODE)
    };

    let player_sort_mode = if let Some(mode) = user
        .as_ref()
        .and_then(|u| u.player_sort_mode.as_deref())
        .and_then(auth::parse_player_sort_mode)
    {
        mode
    } else {
        let from_session: Option<String> = session
            .get(auth::SESSION_PLAYER_SORT_MODE)
            .await
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        from_session
            .as_deref()
            .and_then(auth::parse_player_sort_mode)
            .unwrap_or(auth::DEFAULT_PLAYER_SORT_MODE)
    };

    let tournament_completed_view_mode = if let Some(mode) = user
        .as_ref()
        .and_then(|u| u.tournament_completed_view_mode.as_deref())
        .and_then(auth::parse_tournament_completed_view_mode)
    {
        mode
    } else {
        let from_session: Option<String> = session
            .get(auth::SESSION_TOURNAMENT_COMPLETED_VIEW_MODE)
            .await
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        from_session
            .as_deref()
            .and_then(auth::parse_tournament_completed_view_mode)
            .unwrap_or(auth::DEFAULT_TOURNAMENT_COMPLETED_VIEW_MODE)
    };

    Ok(PrefsResponse {
        secondary_view_mode: secondary_view_mode.to_string(),
        scenario_slug,
        army_sort_mode: army_sort_mode.to_string(),
        player_sort_mode: player_sort_mode.to_string(),
        tournament_completed_view_mode: tournament_completed_view_mode.to_string(),
    })
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

#[derive(Debug, Serialize)]
struct PlayerArmyStatsEntry {
    army_id: u32,
    wins: u32,
    draws: u32,
    losses: u32,
    win_rate: f64,
    elo_delta: f64,
}

#[derive(Debug, Serialize)]
struct ArmyPlayerStatsEntry {
    player_name: String,
    display_name: String,
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

async fn get_army_players(
    State(state): State<AppState>,
    Path(id): Path<u32>,
) -> Result<Json<Vec<ArmyPlayerStatsEntry>>, ApiError> {
    state
        .armies
        .get(id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .ok_or_else(|| ApiError::bad_request(format!("sectorielle introuvable : {}", id)))?;

    let board = state.board.lock().unwrap();
    let resolver = crate::display_name::PlayerDisplayResolver::new(&board, state.users.as_ref());
    let players = board
        .army_player_stats(id)
        .into_iter()
        .map(|entry| ArmyPlayerStatsEntry {
            display_name: resolver.resolve(&entry.player_name),
            wins: entry.wins,
            draws: entry.draws,
            losses: entry.losses,
            win_rate: entry.win_rate(),
            player_name: entry.player_name,
        })
        .collect();
    Ok(Json(players))
}

async fn get_army_matches(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<u32>,
    Query(query): Query<MatchListQuery>,
) -> Result<Json<Vec<crate::display_name::EnrichedMatchRecord>>, ApiError> {
    state
        .armies
        .get(id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .ok_or_else(|| ApiError::bad_request(format!("sectorielle introuvable : {}", id)))?;

    let viewer = viewer_player_name(&state, &session).await;
    let mut records = {
        let board = state.board.lock().unwrap();
        let limit = query.limit.clamp(1, 100);
        board
            .army_matches(id, limit)
            .into_iter()
            .cloned()
            .collect::<Vec<_>>()
    };
    for record in &mut records {
        prepare_match_for_viewer(&state, record, viewer.as_deref());
    }
    let board = state.board.lock().unwrap();
    let resolver = crate::display_name::PlayerDisplayResolver::new(&board, state.users.as_ref());
    let matches = records
        .into_iter()
        .map(|record| resolver.enrich_match(record))
        .collect();
    Ok(Json(matches))
}

async fn viewer_player_name(state: &AppState, session: &Session) -> Option<String> {
    let user = auth::current_user(state.users.as_ref(), session)
        .await
        .ok()
        .flatten()?;
    let board = state.board.lock().unwrap();
    board
        .get_player_by_discord_username(&user.username)
        .map(|player| player.name.clone())
}

fn mask_tournament_elo_lists(
    state: &AppState,
    record: &mut MatchRecord,
    viewer_player: Option<&str>,
) {
    let Some(tournament_id) = record.tournament_id else {
        return;
    };
    let status = state
        .tournaments
        .get(tournament_id)
        .ok()
        .flatten()
        .map(|t| t.status);
    if status == Some(TournamentStatus::Completed) {
        return;
    }
    // Match Elo = résultat déjà validé ; visible seulement aux 2 joueurs tant que le tournoi n'est pas fini.
    let is_participant = viewer_player.is_some_and(|name| {
        crate::store::normalize_name(name) == crate::store::normalize_name(&record.player1)
            || crate::store::normalize_name(name) == crate::store::normalize_name(&record.player2)
    });
    if !is_participant {
        record.player1_army_list_code = None;
        record.player2_army_list_code = None;
    }
}

fn mask_draft_reports(record: &mut MatchRecord, viewer_player: Option<&str>) {
    let viewer_key = viewer_player.map(crate::normalize_name);
    let can_see = |player_name: &str| {
        viewer_key
            .as_ref()
            .is_some_and(|key| crate::normalize_name(player_name) == *key)
    };
    if record
        .player1_report
        .as_ref()
        .is_some_and(|report| report.status == ReportStatus::Draft && !can_see(&record.player1))
    {
        record.player1_report = None;
    }
    if record
        .player2_report
        .as_ref()
        .is_some_and(|report| report.status == ReportStatus::Draft && !can_see(&record.player2))
    {
        record.player2_report = None;
    }
}

fn prepare_match_for_viewer(
    state: &AppState,
    record: &mut MatchRecord,
    viewer_player: Option<&str>,
) {
    mask_tournament_elo_lists(state, record, viewer_player);
    mask_draft_reports(record, viewer_player);
}

async fn list_matches(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<MatchListQuery>,
) -> Json<MatchListResponse> {
    let viewer = viewer_player_name(&state, &session).await;
    let limit = query.limit.clamp(1, 100);
    let offset = query.offset;

    let (total, mut records) = {
        let board = state.board.lock().unwrap();
        let total = board.match_count();
        let records: Vec<_> = board
            .recent_matches_page(limit, offset)
            .into_iter()
            .cloned()
            .collect();
        (total, records)
    };

    for record in &mut records {
        prepare_match_for_viewer(&state, record, viewer.as_deref());
    }

    let board = state.board.lock().unwrap();
    let resolver = crate::display_name::PlayerDisplayResolver::new(&board, state.users.as_ref());
    let matches = records
        .into_iter()
        .map(|record| resolver.enrich_match(record))
        .collect();
    Json(MatchListResponse {
        items: matches,
        total,
        limit,
        offset,
    })
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
    session: Session,
    Path(name): Path<String>,
    Query(query): Query<MatchListQuery>,
) -> Result<Json<Vec<crate::display_name::EnrichedMatchRecord>>, ApiError> {
    let viewer = viewer_player_name(&state, &session).await;
    let limit = query.limit.clamp(1, 500);
    let mut records = {
        let board = state.board.lock().unwrap();
        board
            .player_matches(&name, limit)
            .map_err(|error| ApiError::bad_request(error.to_string()))?
            .into_iter()
            .cloned()
            .collect::<Vec<_>>()
    };
    for record in &mut records {
        prepare_match_for_viewer(&state, record, viewer.as_deref());
    }
    let board = state.board.lock().unwrap();
    let resolver = crate::display_name::PlayerDisplayResolver::new(&board, state.users.as_ref());
    let matches = records
        .into_iter()
        .map(|record| resolver.enrich_match(record))
        .collect();
    Ok(Json(matches))
}

async fn get_player_armies(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<Vec<PlayerArmyStatsEntry>>, ApiError> {
    let board = state.board.lock().unwrap();
    let stats = board
        .player_army_stats(&name)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .into_iter()
        .map(|entry| PlayerArmyStatsEntry {
            army_id: entry.army_id,
            wins: entry.wins,
            draws: entry.draws,
            losses: entry.losses,
            win_rate: entry.win_rate(),
            elo_delta: entry.elo_delta,
        })
        .collect();
    Ok(Json(stats))
}

async fn claim_player(
    State(state): State<AppState>,
    session: Session,
    Json(payload): Json<ClaimPlayerRequest>,
) -> Result<(StatusCode, Json<Player>), ApiError> {
    let user = require_user(&state, &session).await?;

    let mut board = state.board.lock().unwrap();
    if board.player_exists_for_discord_username(&user.username) {
        return Err(ApiError::bad_request(
            "un joueur est déjà associé à ce compte",
        ));
    }

    let name = payload
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| user.effective_display_name().to_string());

    if name.is_empty() {
        return Err(ApiError::bad_request("indiquez un pseudo"));
    }

    board
        .add_player_for_discord_username(&name, &user.username)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let player = board.get_player(&name).unwrap().clone();
    board
        .save(&state.db_path)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    Ok((StatusCode::CREATED, Json(player)))
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

fn admin_accounts_payload(
    board: &Leaderboard,
    users: &[User],
    tournament_keys: &HashSet<String>,
) -> AdminAccountsResponse {
    let user_entries = users
        .iter()
        .map(|user| AdminUserEntry {
            player_name: board
                .get_player_by_discord_username(&user.username)
                .map(|player| player.name.clone()),
            user: user.clone().into(),
        })
        .collect();

    let mut players: Vec<AdminPlayerEntry> = board
        .ranking()
        .into_iter()
        .map(|player| {
            let linked_user = player.discord_username.as_deref().and_then(|username| {
                users
                    .iter()
                    .find(|user| user.username.eq_ignore_ascii_case(username))
                    .cloned()
                    .map(Into::into)
            });
            let has_matches = board
                .player_matches(&player.name, 1)
                .map(|matches| !matches.is_empty())
                .unwrap_or(true);
            AdminPlayerEntry {
                player: player.clone(),
                linked_user,
                can_delete: !has_matches
                    && !tournament_keys.contains(&crate::normalize_name(&player.name)),
            }
        })
        .collect();

    players.sort_by(|left, right| {
        left.player
            .name
            .to_lowercase()
            .cmp(&right.player.name.to_lowercase())
    });

    AdminAccountsResponse {
        users: user_entries,
        players,
    }
}

fn current_admin_accounts(
    state: &AppState,
    board: &Leaderboard,
) -> Result<AdminAccountsResponse, ApiError> {
    let users = state
        .users
        .list_all()
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let tournament_keys = state
        .tournaments
        .referenced_player_keys()
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(admin_accounts_payload(board, &users, &tournament_keys))
}

async fn admin_accounts(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<AdminAccountsResponse>, ApiError> {
    require_admin(&state, &session).await?;
    let board = state.board.lock().unwrap();
    Ok(Json(current_admin_accounts(&state, &board)?))
}

async fn link_player_account(
    State(state): State<AppState>,
    session: Session,
    Json(payload): Json<LinkPlayerAccountRequest>,
) -> Result<Json<AdminAccountsResponse>, ApiError> {
    require_admin(&state, &session).await?;

    let player_name = payload.player_name.trim();
    if player_name.is_empty() {
        return Err(ApiError::bad_request("indiquez un joueur"));
    }

    let discord_username = if let Some(user_id) = payload.user_id {
        let user = state
            .users
            .get_by_id(user_id)
            .map_err(|error| ApiError::bad_request(error.to_string()))?
            .ok_or_else(|| ApiError::bad_request("utilisateur introuvable"))?;
        Some(user.username)
    } else {
        None
    };

    let mut board = state.board.lock().unwrap();
    board
        .link_player_to_discord_username(player_name, discord_username.as_deref())
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    board
        .save(&state.db_path)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    Ok(Json(current_admin_accounts(&state, &board)?))
}

async fn delete_unused_player_account(
    State(state): State<AppState>,
    session: Session,
    Path(name): Path<String>,
) -> Result<Json<AdminAccountsResponse>, ApiError> {
    require_admin(&state, &session).await?;

    let name = name.trim();
    if name.is_empty() {
        return Err(ApiError::bad_request("indiquez un joueur"));
    }

    let tournament_keys = state
        .tournaments
        .referenced_player_keys()
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    if tournament_keys.contains(&crate::normalize_name(name)) {
        return Err(ApiError::bad_request(
            "impossible de supprimer ce joueur : il apparaît encore dans un tournoi",
        ));
    }

    let mut board = state.board.lock().unwrap();
    board
        .delete_unused_player(name)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    board
        .save(&state.db_path)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    Ok(Json(current_admin_accounts(&state, &board)?))
}

async fn merge_player_accounts(
    State(state): State<AppState>,
    session: Session,
    Json(payload): Json<MergePlayersRequest>,
) -> Result<Json<AdminAccountsResponse>, ApiError> {
    require_admin(&state, &session).await?;

    let keep = payload.keep.trim().to_string();
    let alias = payload.alias.trim().to_string();
    if keep.is_empty() || alias.is_empty() {
        return Err(ApiError::bad_request("indiquez les deux joueurs à fusionner"));
    }
    if crate::normalize_name(&keep) == crate::normalize_name(&alias) {
        return Err(ApiError::bad_request(
            "choisissez deux joueurs différents",
        ));
    }

    let mut board = state.board.lock().unwrap();
    board
        .get_player(&keep)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    board
        .get_player(&alias)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    board
        .save(&state.db_path)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    crate::merge_players(&state.db_path, &keep, &[&alias], state.k_factor)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    *board = Leaderboard::load(&state.db_path)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    Ok(Json(current_admin_accounts(&state, &board)?))
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

    let scenario_other = payload
        .scenario_other
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string);

    let (scenario_id, scenario_other, scenario_name) =
        match (payload.scenario_id, scenario_other) {
            (Some(_), Some(_)) => {
                return Err(ApiError::bad_request(
                    "choisissez un scénario du catalogue ou un texte libre, pas les deux",
                ));
            }
            (Some(id), None) => {
                let scenario = state
                    .scenarios
                    .get(id)
                    .map_err(|error| ApiError::bad_request(error.to_string()))?
                    .ok_or_else(|| ApiError::bad_request("scénario introuvable"))?;
                state
                    .scenarios
                    .increment_usage(id)
                    .map_err(|error| ApiError::bad_request(error.to_string()))?;
                (Some(id), None, Some(scenario.name))
            }
            (None, Some(other)) => {
                let label = other.clone();
                (None, Some(other), Some(label))
            }
            (None, None) => (None, None, None),
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
            scenario_other,
            scenario_name,
        )
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    board
        .save(&state.db_path)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    Ok(Json(record))
}

async fn get_match(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<u64>,
) -> Result<Json<crate::display_name::EnrichedMatchRecord>, ApiError> {
    let viewer = viewer_player_name(&state, &session).await;
    let mut record = {
        let board = state.board.lock().unwrap();
        board
            .get_match(id)
            .ok_or_else(|| ApiError::bad_request("match introuvable"))?
            .clone()
    };
    prepare_match_for_viewer(&state, &mut record, viewer.as_deref());
    let board = state.board.lock().unwrap();
    let resolver = crate::display_name::PlayerDisplayResolver::new(&board, state.users.as_ref());
    Ok(Json(resolver.enrich_match(record)))
}

async fn start_match(
    State(state): State<AppState>,
    session: Session,
    Json(payload): Json<StartMatchRequest>,
) -> Result<(StatusCode, Json<crate::display_name::EnrichedMatchRecord>), ApiError> {
    let user = require_user(&state, &session).await?;
    let created_by = {
        let board = state.board.lock().unwrap();
        board
            .get_player_by_discord_username(&user.username)
            .ok_or_else(|| {
                ApiError::unauthorized("un profil joueur Poissonnerie est requis pour démarrer une partie")
            })?
            .name
            .clone()
    };

    let client_uuid = match payload.client_uuid.as_deref().map(str::trim).filter(|value| !value.is_empty())
    {
        Some(value) => Some(
            crate::match_record::normalize_client_uuid(value)
                .map_err(|error| ApiError::bad_request(error.to_string()))?,
        ),
        None => None,
    };
    if let Some(uuid) = client_uuid.as_deref() {
        if let Some(existing) = load_client_partie(&state, uuid)? {
            ensure_match_participant(&state, &user, existing.id)?;
            let board = state.board.lock().unwrap();
            let resolver =
                crate::display_name::PlayerDisplayResolver::new(&board, state.users.as_ref());
            return Ok((StatusCode::OK, Json(resolver.enrich_match(existing))));
        }
    }

    state
        .armies
        .validate_selectable_id(payload.player1_army_id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    state
        .armies
        .validate_selectable_id(payload.player2_army_id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    if !payload.player1_secondary_slugs.is_empty()
        || !payload.player2_secondary_slugs.is_empty()
    {
        if payload.player1_secondary_slugs.len() != 3
            || payload.player2_secondary_slugs.len() != 3
        {
            return Err(ApiError::bad_request(
                "chaque joueur doit recevoir exactement 3 objectifs secondaires, ou aucun (saisie manuelle)",
            ));
        }
    }

    let player2_raw = payload.player2.trim();
    let adversaire_raw = payload
        .adversaire
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty());

    let (opponent_name, guest_adversaire) = {
        let board = state.board.lock().unwrap();
        let registered = !player2_raw.is_empty()
            && board
                .get_player(player2_raw)
                .is_ok();
        if registered {
            (player2_raw.to_string(), None)
        } else {
            let label = if !player2_raw.is_empty() {
                player2_raw
            } else {
                adversaire_raw.unwrap_or("")
            };
            if label.is_empty() {
                return Err(ApiError::bad_request("indiquez un adversaire"));
            }
            if payload.counts_for_elo {
                return Err(ApiError::bad_request(
                    "un match classé requiert un adversaire inscrit",
                ));
            }
            (label.to_string(), Some(label.to_string()))
        }
    };

    let mut board = state.board.lock().unwrap();
    if crate::normalize_name(&payload.player1) != crate::normalize_name(&created_by) {
        return Err(ApiError::bad_request(
            "le joueur 1 doit être votre profil connecté",
        ));
    }
    let record = board
        .start_match_with_tournament(
            &payload.player1,
            &opponent_name,
            payload.player1_army_id,
            payload.player2_army_id,
            &created_by,
            payload.player1_secondary_slugs,
            payload.player2_secondary_slugs,
            payload.counts_for_elo,
            None,
            None,
            None,
            None,
            guest_adversaire.as_deref(),
            client_uuid,
        )
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    board
        .save(&state.db_path)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let resolver = crate::display_name::PlayerDisplayResolver::new(&board, state.users.as_ref());
    Ok((StatusCode::CREATED, Json(resolver.enrich_match(record))))
}

fn load_client_partie(state: &AppState, uuid: &str) -> Result<Option<MatchRecord>, ApiError> {
    let board = state.board.lock().unwrap();
    let Some(record) = board.get_match_by_client_uuid(uuid).cloned() else {
        return Ok(None);
    };
    if record.tournament_id.is_some() {
        return Err(ApiError::bad_request(
            "les parties de coupe se saisissent uniquement en ligne",
        ));
    }
    Ok(Some(record))
}

fn sync_progress_update(
    state: &AppState,
    payload: &SyncPartieRequest,
) -> Result<crate::store::InProgressMatchUpdate, ApiError> {
    let scenario_name = if let Some(scenario_id) = payload.scenario_id {
        Some(
            state
                .scenarios
                .get(scenario_id)
                .map_err(|error| ApiError::bad_request(error.to_string()))?
                .map(|scenario| scenario.name),
        )
    } else if let Some(other) = payload.scenario_other.as_ref() {
        let trimmed = other.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(Some(trimmed.to_string()))
        }
    } else {
        None
    };

    let clear_secondary_draws = payload
        .scenario_id
        .and_then(|id| state.scenarios.get(id).ok().flatten())
        .and_then(|scenario| scenario.slug)
        .as_deref()
        == Some("le-combat-de-lesprit");

    Ok(crate::store::InProgressMatchUpdate {
        scenario_id: payload.scenario_id,
        scenario_other: payload.scenario_other.clone(),
        scenario_name,
        scenario_url: payload.scenario_url.clone(),
        player1_secondary_slugs: if payload.player1_secondary_slugs.is_empty() {
            None
        } else {
            Some(payload.player1_secondary_slugs.clone())
        },
        player2_secondary_slugs: if payload.player2_secondary_slugs.is_empty() {
            None
        } else {
            Some(payload.player2_secondary_slugs.clone())
        },
        secondary_pool_slugs: payload.secondary_pool_slugs.clone(),
        player1_chosen_secondary: payload.player1_chosen_secondary.clone().map(Some),
        player2_chosen_secondary: payload.player2_chosen_secondary.clone().map(Some),
        lieutenant_winner: payload.lieutenant_winner.clone(),
        lieutenant_winner_choice: payload.lieutenant_winner_choice.clone(),
        lieutenant_other_choice: payload.lieutenant_other_choice.clone(),
        partie_step: payload.partie_step.clone(),
        clear_secondary_draws,
    })
}

fn complete_scores(payload: &CompleteMatchRequest) -> MatchScores {
    MatchScores {
        player1_objectives: payload.player1_objectives,
        player1_survivors: payload.player1_survivors,
        player2_objectives: payload.player2_objectives,
        player2_survivors: payload.player2_survivors,
    }
}

async fn sync_partie(
    State(state): State<AppState>,
    session: Session,
    Json(payload): Json<SyncPartieRequest>,
) -> Result<(StatusCode, Json<crate::display_name::EnrichedMatchRecord>), ApiError> {
    let user = require_user(&state, &session).await?;
    let created_by = {
        let board = state.board.lock().unwrap();
        board
            .get_player_by_discord_username(&user.username)
            .ok_or_else(|| {
                ApiError::unauthorized(
                    "un profil joueur Poissonnerie est requis pour synchroniser une partie",
                )
            })?
            .name
            .clone()
    };

    let client_uuid = crate::match_record::normalize_client_uuid(&payload.client_uuid)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    if crate::normalize_name(&payload.player1) != crate::normalize_name(&created_by) {
        return Err(ApiError::bad_request(
            "le joueur 1 doit être votre profil connecté",
        ));
    }

    if let Some(existing) = load_client_partie(&state, &client_uuid)? {
        ensure_match_participant(&state, &user, existing.id)?;
        if existing.status == crate::match_record::MatchStatus::Completed {
            if let Some(complete) = payload.complete.as_ref() {
                let same_outcome = existing.outcome == Some(complete.outcome);
                let same_scores = existing.player1_objectives == complete.player1_objectives
                    && existing.player1_survivors == complete.player1_survivors
                    && existing.player2_objectives == complete.player2_objectives
                    && existing.player2_survivors == complete.player2_survivors;
                if !same_outcome || !same_scores {
                    return Err(ApiError::bad_request("cette partie est déjà terminée"));
                }
            }
            let board = state.board.lock().unwrap();
            let resolver =
                crate::display_name::PlayerDisplayResolver::new(&board, state.users.as_ref());
            return Ok((StatusCode::OK, Json(resolver.enrich_match(existing))));
        }

        state
            .armies
            .validate_selectable_id(payload.player1_army_id)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        state
            .armies
            .validate_selectable_id(payload.player2_army_id)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;

        let update = sync_progress_update(&state, &payload)?;
        let mut board = state.board.lock().unwrap();
        let mut synced = board
            .apply_offline_partie_snapshot(existing.id, update)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        if let Some(complete) = payload.complete.as_ref() {
            synced = board
                .complete_match(existing.id, complete.outcome, state.k_factor, complete_scores(complete))
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
        }
        board
            .save(&state.db_path)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        let resolver =
            crate::display_name::PlayerDisplayResolver::new(&board, state.users.as_ref());
        return Ok((StatusCode::OK, Json(resolver.enrich_match(synced))));
    }

    state
        .armies
        .validate_selectable_id(payload.player1_army_id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    state
        .armies
        .validate_selectable_id(payload.player2_army_id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let player2_raw = payload.player2.trim();
    let adversaire_raw = payload
        .adversaire
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty());
    let (opponent_name, guest_adversaire) = {
        let board = state.board.lock().unwrap();
        let registered = !player2_raw.is_empty() && board.get_player(player2_raw).is_ok();
        if registered {
            (player2_raw.to_string(), None)
        } else {
            let label = if !player2_raw.is_empty() {
                player2_raw
            } else {
                adversaire_raw.unwrap_or("")
            };
            if label.is_empty() {
                return Err(ApiError::bad_request("indiquez un adversaire"));
            }
            if payload.counts_for_elo {
                return Err(ApiError::bad_request(
                    "un match classé requiert un adversaire inscrit",
                ));
            }
            (label.to_string(), Some(label.to_string()))
        }
    };

    let update = sync_progress_update(&state, &payload)?;
    let mut board = state.board.lock().unwrap();
    let created = board
        .start_match_with_tournament(
            &payload.player1,
            &opponent_name,
            payload.player1_army_id,
            payload.player2_army_id,
            &created_by,
            payload.player1_secondary_slugs.clone(),
            payload.player2_secondary_slugs.clone(),
            payload.counts_for_elo,
            None,
            None,
            None,
            None,
            guest_adversaire.as_deref(),
            Some(client_uuid),
        )
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let mut synced = board
        .apply_offline_partie_snapshot(created.id, update)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    if let Some(complete) = payload.complete.as_ref() {
        synced = board
            .complete_match(created.id, complete.outcome, state.k_factor, complete_scores(complete))
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
    }
    board
        .save(&state.db_path)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let resolver = crate::display_name::PlayerDisplayResolver::new(&board, state.users.as_ref());
    Ok((StatusCode::CREATED, Json(resolver.enrich_match(synced))))
}

async fn update_match_progress(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<u64>,
    Json(payload): Json<UpdateMatchProgressRequest>,
) -> Result<Json<crate::display_name::EnrichedMatchRecord>, ApiError> {
    let user = require_user(&state, &session).await?;
    ensure_match_participant(&state, &user, id)?;

    let scenario_name = if let Some(scenario_id) = payload.scenario_id {
        Some(
            state
                .scenarios
                .get(scenario_id)
                .map_err(|error| ApiError::bad_request(error.to_string()))?
                .map(|scenario| scenario.name),
        )
    } else if let Some(other) = payload.scenario_other.as_ref() {
        let trimmed = other.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(Some(trimmed.to_string()))
        }
    } else {
        None
    };

    let clear_secondaries_for_combat_esprit = payload
        .scenario_id
        .and_then(|id| state.scenarios.get(id).ok().flatten())
        .and_then(|scenario| scenario.slug)
        .as_deref()
        == Some("le-combat-de-lesprit");

    let update = crate::store::InProgressMatchUpdate {
        scenario_id: payload.scenario_id,
        scenario_other: payload.scenario_other,
        scenario_name,
        scenario_url: payload.scenario_url,
        player1_secondary_slugs: payload.player1_secondary_slugs,
        player2_secondary_slugs: payload.player2_secondary_slugs,
        secondary_pool_slugs: payload.secondary_pool_slugs,
        player1_chosen_secondary: payload
            .player1_chosen_secondary
            .map(Some),
        player2_chosen_secondary: payload
            .player2_chosen_secondary
            .map(Some),
        lieutenant_winner: payload.lieutenant_winner,
        lieutenant_winner_choice: payload.lieutenant_winner_choice,
        lieutenant_other_choice: payload.lieutenant_other_choice,
        partie_step: payload.partie_step,
        clear_secondary_draws: clear_secondaries_for_combat_esprit,
    };

    let mut board = state.board.lock().unwrap();
    let record = board
        .update_in_progress_match(id, update)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    board
        .save(&state.db_path)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let resolver = crate::display_name::PlayerDisplayResolver::new(&board, state.users.as_ref());
    Ok(Json(resolver.enrich_match(record)))
}

async fn complete_match(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<u64>,
    Json(payload): Json<CompleteMatchRequest>,
) -> Result<Json<crate::display_name::EnrichedMatchRecord>, ApiError> {
    let user = require_user(&state, &session).await?;
    ensure_match_participant(&state, &user, id)?;

    let scores = MatchScores {
        player1_objectives: payload.player1_objectives,
        player1_survivors: payload.player1_survivors,
        player2_objectives: payload.player2_objectives,
        player2_survivors: payload.player2_survivors,
    };

    let mut board = state.board.lock().unwrap();
    let record = board
        .complete_match(id, payload.outcome, state.k_factor, scores)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    board
        .save(&state.db_path)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let resolver = crate::display_name::PlayerDisplayResolver::new(&board, state.users.as_ref());
    Ok(Json(resolver.enrich_match(record)))
}

async fn correct_match(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<u64>,
    Json(payload): Json<CompleteMatchRequest>,
) -> Result<Json<crate::display_name::EnrichedMatchRecord>, ApiError> {
    require_admin(&state, &session).await?;

    let scores = MatchScores {
        player1_objectives: payload.player1_objectives,
        player1_survivors: payload.player1_survivors,
        player2_objectives: payload.player2_objectives,
        player2_survivors: payload.player2_survivors,
    };

    // Match lié à un tournoi : déléguer à la correction tournoi (classements / arbre).
    if let Ok(Some(tm)) = state.tournaments.find_match_by_elo_match_id(id) {
        if tm.status == crate::tournament::TournamentMatchStatus::Confirmed
            || tm.status == crate::tournament::TournamentMatchStatus::Submitted
        {
            let submit = crate::tournament_store::SubmitMatchRequest {
                player1_objectives: payload.player1_objectives,
                player2_objectives: payload.player2_objectives,
                player1_survivors: payload.player1_survivors,
                player2_survivors: payload.player2_survivors,
                player1_army_id: None,
                player2_army_id: None,
                player1_list_slot: None,
                player2_list_slot: None,
                scenario_id: None,
                scenario_other: None,
            };
            tournament_api::apply_tournament_match_correction(&state, tm.id, &submit)?;

            let board = state.board.lock().unwrap();
            let record = board
                .get_match(id)
                .cloned()
                .ok_or_else(|| ApiError::bad_request("match introuvable"))?;
            let resolver =
                crate::display_name::PlayerDisplayResolver::new(&board, state.users.as_ref());
            return Ok(Json(resolver.enrich_match(record)));
        }
    }

    let mut board = state.board.lock().unwrap();
    let record = board
        .correct_match(id, payload.outcome, state.k_factor, scores)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    board
        .save(&state.db_path)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let resolver = crate::display_name::PlayerDisplayResolver::new(&board, state.users.as_ref());
    Ok(Json(resolver.enrich_match(record)))
}

async fn list_my_in_progress_matches(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<Vec<crate::display_name::EnrichedMatchRecord>>, ApiError> {
    let user = require_user(&state, &session).await?;
    let board = state.board.lock().unwrap();
    let resolver = crate::display_name::PlayerDisplayResolver::new(&board, state.users.as_ref());

    let matches = if user.is_admin {
        board
            .in_progress_matches()
            .into_iter()
            .cloned()
            .map(|record| resolver.enrich_match(record))
            .collect()
    } else {
        let player = board
            .get_player_by_discord_username(&user.username)
            .ok_or_else(|| {
                ApiError::unauthorized("un profil joueur Poissonnerie est requis")
            })?;
        board
            .in_progress_matches_for_player(&player.name)
            .into_iter()
            .cloned()
            .map(|record| resolver.enrich_match(record))
            .collect()
    };

    Ok(Json(matches))
}

fn ensure_match_participant(
    state: &AppState,
    user: &User,
    match_id: u64,
) -> Result<(), ApiError> {
    if user.is_admin {
        return Ok(());
    }
    let board = state.board.lock().unwrap();
    let player = board
        .get_player_by_discord_username(&user.username)
        .ok_or_else(|| ApiError::unauthorized("profil joueur requis"))?;
    let record = board
        .get_match(match_id)
        .ok_or_else(|| ApiError::bad_request("match introuvable"))?;
    let key = crate::normalize_name(&player.name);
    if crate::normalize_name(&record.player1) != key
        && crate::normalize_name(&record.player2) != key
    {
        return Err(ApiError::unauthorized(
            "vous ne participez pas à cette partie",
        ));
    }
    Ok(())
}

async fn delete_match(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<u64>,
) -> Result<StatusCode, ApiError> {
    require_admin(&state, &session).await?;

    let mut board = state.board.lock().unwrap();
    board
        .delete_match(id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    board
        .save(&state.db_path)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct UpdateMatchReportRequest {
    body_md: String,
    #[serde(default)]
    status: ReportStatus,
}

#[derive(Debug, Deserialize)]
struct ReportTemplateBody {
    name: String,
    body_md: String,
}

#[derive(Debug, Serialize)]
struct EnrichedRecentMatchReport {
    #[serde(flatten)]
    report: crate::RecentMatchReport,
    author_display_name: String,
    opponent_display_name: String,
}

#[derive(Debug, Deserialize)]
struct UpdateMatchArmyListRequest {
    army_list_code: String,
    #[serde(default)]
    army_id: Option<u32>,
}

async fn update_match_report(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<u64>,
    Json(payload): Json<UpdateMatchReportRequest>,
) -> Result<Json<crate::display_name::EnrichedMatchRecord>, ApiError> {
    let user = require_user(&state, &session).await?;
    let player = {
        let board = state.board.lock().unwrap();
        board
            .get_player_by_discord_username(&user.username)
            .ok_or_else(|| {
                ApiError::unauthorized(
                    "un profil joueur Poissonnerie est requis pour rédiger un CR",
                )
            })?
            .clone()
    };

    let mut board = state.board.lock().unwrap();
    board
        .update_match_report(id, &player.name, &payload.body_md, payload.status)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    board
        .save(&state.db_path)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let mut record = board.get_match(id).unwrap().clone();
    prepare_match_for_viewer(&state, &mut record, Some(&player.name));
    let resolver = crate::display_name::PlayerDisplayResolver::new(&board, state.users.as_ref());
    Ok(Json(resolver.enrich_match(record)))
}

async fn update_match_army_list(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<u64>,
    Json(payload): Json<UpdateMatchArmyListRequest>,
) -> Result<Json<crate::display_name::EnrichedMatchRecord>, ApiError> {
    let user = require_user(&state, &session).await?;
    let player = {
        let board = state.board.lock().unwrap();
        board
            .get_player_by_discord_username(&user.username)
            .ok_or_else(|| {
                ApiError::unauthorized(
                    "un profil joueur Poissonnerie est requis pour enregistrer une liste",
                )
            })?
            .clone()
    };

    if let Some(army_id) = payload.army_id {
        let _ = army_id;
    }

    let list = state
        .army_lists
        .get_or_create(&payload.army_list_code)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let mut board = state.board.lock().unwrap();
    board
        .update_match_army_list(
            id,
            &player.name,
            list.id,
            &list.code,
            list.army_id,
        )
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    board
        .save(&state.db_path)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let mut record = board.get_match(id).unwrap().clone();
    prepare_match_for_viewer(&state, &mut record, Some(&player.name));
    let resolver = crate::display_name::PlayerDisplayResolver::new(&board, state.users.as_ref());
    Ok(Json(resolver.enrich_match(record)))
}

async fn list_recent_reports(
    State(state): State<AppState>,
    Query(query): Query<MatchListQuery>,
) -> Json<ReportListResponse> {
    let limit = query.limit.clamp(1, 100);
    let offset = query.offset;
    let board = state.board.lock().unwrap();
    let resolver = crate::display_name::PlayerDisplayResolver::new(&board, state.users.as_ref());
    let (reports, total) = board.recent_published_reports(limit, offset);
    let items = reports
        .into_iter()
        .map(|report| EnrichedRecentMatchReport {
            author_display_name: resolver.resolve(&report.author_name),
            opponent_display_name: resolver.resolve(&report.opponent_name),
            report,
        })
        .collect();
    Json(ReportListResponse {
        items,
        total,
        limit,
        offset,
    })
}

async fn list_report_templates(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<Vec<crate::ReportTemplate>>, ApiError> {
    let user = require_user(&state, &session).await?;
    let templates = state
        .report_templates
        .list_for_user(user.id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(templates))
}

async fn create_report_template(
    State(state): State<AppState>,
    session: Session,
    Json(payload): Json<ReportTemplateBody>,
) -> Result<(StatusCode, Json<crate::ReportTemplate>), ApiError> {
    let user = require_user(&state, &session).await?;
    let template = state
        .report_templates
        .create(user.id, &payload.name, &payload.body_md)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok((StatusCode::CREATED, Json(template)))
}

async fn update_report_template(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    Json(payload): Json<ReportTemplateBody>,
) -> Result<Json<crate::ReportTemplate>, ApiError> {
    let user = require_user(&state, &session).await?;
    let template = state
        .report_templates
        .update(user.id, id, &payload.name, &payload.body_md)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(template))
}

async fn delete_report_template(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let user = require_user(&state, &session).await?;
    state
        .report_templates
        .delete(user.id, id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct ArmyListQuery {
    #[serde(default)]
    army_ids: Option<String>,
}

async fn list_army_lists(
    State(state): State<AppState>,
    Query(query): Query<ArmyListQuery>,
) -> Result<Json<Vec<ArmyListStatsGroup>>, ApiError> {
    let army_ids = query.army_ids.as_ref().map(|raw| {
        raw.split(',')
            .filter_map(|part| part.trim().parse::<u32>().ok())
            .collect::<Vec<_>>()
    });
    let filter = army_ids.as_deref();
    let groups = state
        .army_lists
        .list_stats_by_army(filter)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(groups))
}

async fn list_army_list_armies(
    State(state): State<AppState>,
) -> Result<Json<Vec<u32>>, ApiError> {
    let ids = state
        .army_lists
        .army_ids_with_public_lists()
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(ids))
}

async fn get_army_list_matches(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    Query(query): Query<MatchListQuery>,
) -> Result<Json<Vec<crate::display_name::EnrichedMatchRecord>>, ApiError> {
    state
        .army_lists
        .get(id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .ok_or_else(|| ApiError::bad_request(format!("liste d'armée introuvable : {}", id)))?;

    let limit = query.limit.clamp(1, 500);
    let match_ids = state
        .army_lists
        .list_match_ids(id, limit)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let viewer = viewer_player_name(&state, &session).await;
    let mut records = {
        let board = state.board.lock().unwrap();
        match_ids
            .into_iter()
            .filter_map(|match_id| board.get_match(match_id).cloned())
            .collect::<Vec<_>>()
    };
    for record in &mut records {
        prepare_match_for_viewer(&state, record, viewer.as_deref());
    }
    let board = state.board.lock().unwrap();
    let resolver = crate::display_name::PlayerDisplayResolver::new(&board, state.users.as_ref());
    let matches = records
        .into_iter()
        .map(|record| resolver.enrich_match(record))
        .collect();
    Ok(Json(matches))
}

pub fn default_state() -> anyhow::Result<AppState> {
    let db_path = default_db_path();
    let board = Leaderboard::load(&db_path)?;
    let armies = ArmyStore::open(&db_path)?;
    let army_lists = ArmyListStore::open(&db_path)?;
    let users = UserStore::open(&db_path)?;
    let tournaments = TournamentStore::open(&db_path)?;
    let dauphine = DauphineStore::open(&db_path)?;
    let scenarios = ScenarioStore::open(&db_path)?;
    let report_templates = ReportTemplateStore::open(&db_path)?;
    let site_content = SiteContentStore::open(&db_path)?;
    let auth = AuthConfig::from_env().ok();
    Ok(AppState {
        board: Arc::new(Mutex::new(board)),
        armies: Arc::new(armies),
        army_lists: Arc::new(army_lists),
        users: Arc::new(users),
        tournaments: Arc::new(tournaments),
        dauphine: Arc::new(dauphine),
        scenarios: Arc::new(scenarios),
        report_templates: Arc::new(report_templates),
        site_content: Arc::new(site_content),
        auth,
        db_path,
        k_factor: DEFAULT_K_FACTOR,
    })
}

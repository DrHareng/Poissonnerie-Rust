use std::collections::{HashMap, HashSet};

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json,
};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use crate::{
    api::{ApiError, AppState},
    auth,
    match_record::MatchScores,
    player::{MatchOutcome, RatingUpdate},
    store::Leaderboard,
    tournament::TournamentMatchStatus,
    User,
};

use crate::tournament_store::{
    AdminRegisterRequest, CreateTournamentRequest, ForfeitRequest, RegisterRequest,
    SetupPoolsRequest, SubmitMatchRequest,
};

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

async fn require_player(state: &AppState, session: &Session) -> Result<(User, String), ApiError> {
    let user = require_user(state, session).await?;
    let board = state.board.lock().unwrap();
    let player = board
        .get_player_by_discord_username(&user.username)
        .ok_or_else(|| ApiError::bad_request("aucun joueur lié à ce compte"))?;
    Ok((user, player.name.clone()))
}

#[derive(Debug, Deserialize)]
struct ReviewRegistrationRequest {
    action: String,
}

struct ViewerContext {
    is_admin: bool,
    player_name: Option<String>,
}

async fn viewer_context(state: &AppState, session: &Session) -> ViewerContext {
    let user = auth::current_user(state.users.as_ref(), session)
        .await
        .ok()
        .flatten();
    let Some(user) = user else {
        return ViewerContext {
            is_admin: false,
            player_name: None,
        };
    };

    let player_name = {
        let board = state.board.lock().unwrap();
        board
            .get_player_by_discord_username(&user.username)
            .map(|player| player.name.clone())
    };

    ViewerContext {
        is_admin: user.is_admin,
        player_name,
    }
}

fn mask_registrations(
    tournament: &crate::tournament::Tournament,
    registrations: Vec<crate::tournament::TournamentRegistration>,
    viewer: &ViewerContext,
) -> Vec<crate::tournament::TournamentRegistration> {
    let armies_public = matches!(
        tournament.status,
        crate::tournament::TournamentStatus::Started | crate::tournament::TournamentStatus::Completed
    );

    registrations
        .into_iter()
        .map(|mut registration| {
            let is_own = viewer
                .player_name
                .as_ref()
                .is_some_and(|name| {
                    crate::store::normalize_name(name)
                        == crate::store::normalize_name(&registration.player_name)
                });

            if !armies_public && !viewer.is_admin && !is_own {
                registration.army_id = None;
            }
            registration
        })
        .collect()
}

#[derive(Debug, Deserialize)]
struct ScenarioQuery {
    q: Option<String>,
    #[serde(default = "default_scenario_limit")]
    limit: usize,
}

fn default_scenario_limit() -> usize {
    20
}

#[derive(Debug, Serialize)]
pub struct RankedPlayerWithStars {
    pub rank: usize,
    #[serde(flatten)]
    pub player: crate::Player,
    pub display_name: String,
    pub top_armies: Vec<crate::PlayerArmyUsage>,
    pub star_count: u32,
}

pub fn tournament_routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/api/scenarios", get(list_scenarios))
        .route("/api/tournaments", get(list_tournaments).post(create_tournament))
        .route("/api/tournaments/{id}", get(get_tournament))
        .route(
            "/api/tournaments/{id}/open-registration",
            post(open_registration),
        )
        .route(
            "/api/tournaments/{id}/close-registration",
            post(close_registration),
        )
        .route("/api/tournaments/{id}/register", post(register_tournament))
        .route(
            "/api/tournaments/{id}/registrations",
            post(admin_register).get(list_registrations),
        )
        .route(
            "/api/tournaments/{id}/registrations/{reg_id}/review",
            post(review_registration),
        )
        .route("/api/tournaments/{id}/start", post(start_tournament))
        .route("/api/tournaments/{id}/pools", post(setup_pools))
        .route(
            "/api/tournaments/{id}/generate-pool-matches",
            post(generate_pool_matches),
        )
        .route(
            "/api/tournaments/{id}/finalize-pools",
            post(finalize_pools),
        )
        .route("/api/tournaments/{id}/setup-bracket", post(setup_bracket))
        .route(
            "/api/tournaments/{id}/generate-bracket",
            post(generate_bracket),
        )
        .route(
            "/api/tournament-matches/{id}/submit",
            post(submit_tournament_match),
        )
        .route(
            "/api/tournament-matches/{id}/confirm",
            post(confirm_tournament_match),
        )
        .route(
            "/api/tournament-matches/{id}/forfeit",
            post(forfeit_tournament_match),
        )
        .route(
            "/api/tournament-matches/{id}/unplayed",
            post(unplayed_tournament_match),
        )
        .route(
            "/api/tournament-matches/{id}/correct",
            post(correct_tournament_match),
        )
        .route(
            "/api/players/{name}/tournaments",
            get(get_player_tournaments),
        )
}

async fn list_scenarios(
    State(state): State<AppState>,
    Query(query): Query<ScenarioQuery>,
) -> Result<Json<Vec<crate::scenario::Scenario>>, ApiError> {
    state
        .scenarios
        .list(query.q.as_deref(), query.limit)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn list_tournaments(
    State(state): State<AppState>,
) -> Result<Json<Vec<crate::tournament::TournamentListEntry>>, ApiError> {
    let entries = state.tournaments.list_entries().map_err(|error| {
        ApiError::bad_request(error.to_string())
    })?;

    let board = state.board.lock().unwrap();
    let resolver = crate::display_name::PlayerDisplayResolver::new(&board, state.users.as_ref());

    Ok(Json(
        entries
            .into_iter()
            .map(|mut entry| {
                for top_entry in &mut entry.top_four {
                    top_entry.player_display_name =
                        Some(resolver.resolve(&top_entry.player_name));
                }
                entry
            })
            .collect(),
    ))
}

async fn create_tournament(
    State(state): State<AppState>,
    session: Session,
    Json(payload): Json<CreateTournamentRequest>,
) -> Result<(StatusCode, Json<crate::tournament::Tournament>), ApiError> {
    require_admin(&state, &session).await?;
    let tournament = state
        .tournaments
        .create(&payload)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok((StatusCode::CREATED, Json(tournament)))
}

async fn get_tournament(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
) -> Result<Json<crate::tournament::TournamentDetail>, ApiError> {
    let viewer = viewer_context(&state, &session).await;
    let mut detail = state
        .tournaments
        .get_detail(id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .ok_or_else(|| ApiError::bad_request("tournoi introuvable"))?;

    detail.registrations =
        mask_registrations(&detail.tournament, detail.registrations, &viewer);

    let board = state.board.lock().unwrap();
    let resolver = crate::display_name::PlayerDisplayResolver::new(&board, state.users.as_ref());
    Ok(Json(resolver.enrich_tournament_detail(detail)))
}

async fn open_registration(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
) -> Result<Json<crate::tournament::Tournament>, ApiError> {
    require_admin(&state, &session).await?;
    state
        .tournaments
        .open_registration(id)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn close_registration(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
) -> Result<Json<crate::tournament::Tournament>, ApiError> {
    require_admin(&state, &session).await?;
    state
        .tournaments
        .close_registration(id)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn register_tournament(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    Json(payload): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<crate::tournament::TournamentRegistration>), ApiError> {
    let (user, player_name) = require_player(&state, &session).await?;
    state
        .armies
        .validate_selectable_id(payload.army_id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let registration = state
        .tournaments
        .register(id, &player_name, user.id, payload.army_id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok((StatusCode::CREATED, Json(registration)))
}

async fn admin_register(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    Json(payload): Json<AdminRegisterRequest>,
) -> Result<(StatusCode, Json<crate::tournament::TournamentRegistration>), ApiError> {
    let admin = require_admin(&state, &session).await?;
    let player_name = payload.player_name.trim();
    if player_name.is_empty() {
        return Err(ApiError::bad_request("indiquez un joueur"));
    }

    state
        .armies
        .validate_selectable_id(payload.army_id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    {
        let board = state.board.lock().unwrap();
        board
            .get_player(player_name)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
    }

    let registration = state
        .tournaments
        .admin_register(id, player_name, admin.id, payload.army_id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok((StatusCode::CREATED, Json(registration)))
}

async fn list_registrations(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
) -> Result<Json<Vec<crate::tournament::TournamentRegistration>>, ApiError> {
    require_admin(&state, &session).await?;
    let detail = state
        .tournaments
        .get_detail(id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .ok_or_else(|| ApiError::bad_request("tournoi introuvable"))?;
    Ok(Json(detail.registrations))
}

async fn review_registration(
    State(state): State<AppState>,
    session: Session,
    Path((_tournament_id, reg_id)): Path<(i64, i64)>,
    Json(payload): Json<ReviewRegistrationRequest>,
) -> Result<Json<crate::tournament::TournamentRegistration>, ApiError> {
    let admin = require_admin(&state, &session).await?;
    state
        .tournaments
        .review_registration(reg_id, &payload.action, admin.id)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn start_tournament(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
) -> Result<Json<crate::tournament::Tournament>, ApiError> {
    require_admin(&state, &session).await?;
    let board = state.board.lock().unwrap();
    let ratings: Vec<(String, f64)> = board
        .ranking()
        .into_iter()
        .map(|p| (p.name.clone(), p.rating))
        .collect();
    drop(board);

    let tournament = state
        .tournaments
        .start(id, &ratings)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(tournament))
}

async fn setup_pools(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    Json(payload): Json<SetupPoolsRequest>,
) -> Result<Json<Vec<crate::tournament::Pool>>, ApiError> {
    require_admin(&state, &session).await?;
    let pools = state
        .tournaments
        .setup_pools(id, &payload)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(pools))
}

async fn generate_pool_matches(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
) -> Result<Json<Vec<crate::tournament::TournamentMatch>>, ApiError> {
    require_admin(&state, &session).await?;
    let matches = state
        .tournaments
        .generate_pool_matches(id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(matches))
}

async fn finalize_pools(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
) -> Result<Json<crate::tournament::Tournament>, ApiError> {
    require_admin(&state, &session).await?;

    let pool_matches = state
        .tournaments
        .pool_matches_pending_elo(id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let tournament_name = state
        .tournaments
        .get(id)
        .ok()
        .flatten()
        .map(|tournament| tournament.name);

    {
        let mut board = state.board.lock().unwrap();
        for tm in &pool_matches {
            apply_tournament_match_to_board(&mut board, tm, tournament_name.clone())?;
            state
                .tournaments
                .mark_pool_elo_applied(tm.id)
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
        }
        board
            .save(&state.db_path)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
    }

    let tournament = state
        .tournaments
        .finalize_pools(id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(tournament))
}

async fn setup_bracket(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    Json(payload): Json<crate::tournament_store::SetupBracketRequest>,
) -> Result<Json<Vec<crate::tournament::TournamentMatch>>, ApiError> {
    require_admin(&state, &session).await?;
    let matches = state
        .tournaments
        .setup_bracket(id, &payload)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(matches))
}

async fn generate_bracket(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
) -> Result<Json<Vec<crate::tournament::TournamentMatch>>, ApiError> {
    require_admin(&state, &session).await?;
    let matches = state
        .tournaments
        .generate_bracket(id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(matches))
}

async fn submit_tournament_match(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    Json(payload): Json<SubmitMatchRequest>,
) -> Result<Json<crate::tournament::TournamentMatch>, ApiError> {
    let user = require_user(&state, &session).await?;
    let player_name = if user.is_admin {
        None
    } else {
        let board = state.board.lock().unwrap();
        Some(
            board
                .get_player_by_discord_username(&user.username)
                .ok_or_else(|| ApiError::bad_request("aucun joueur lié"))?
                .name
                .clone(),
        )
    };

    let tm = state
        .tournaments
        .submit_match(
            id,
            &payload,
            user.id,
            user.is_admin,
            player_name.as_deref(),
            state.k_factor,
        )
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    if tm.status == TournamentMatchStatus::Confirmed {
        if tm.is_forfeit {
            // standings updated in store
        } else if tm.phase != crate::tournament::TournamentPhase::Pool {
            maybe_apply_bracket_match(&state, &tm)?;
        }
    }

    Ok(Json(tm))
}

async fn confirm_tournament_match(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
) -> Result<Json<crate::tournament::TournamentMatch>, ApiError> {
    let user = require_user(&state, &session).await?;
    let player_name = if user.is_admin {
        None
    } else {
        let board = state.board.lock().unwrap();
        Some(
            board
                .get_player_by_discord_username(&user.username)
                .ok_or_else(|| ApiError::bad_request("aucun joueur lié"))?
                .name
                .clone(),
        )
    };

    let tm = state
        .tournaments
        .confirm_match(
            id,
            user.id,
            user.is_admin,
            player_name.as_deref(),
            state.k_factor,
        )
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    if !tm.is_forfeit && tm.phase != crate::tournament::TournamentPhase::Pool {
        maybe_apply_bracket_match(&state, &tm)?;
    }

    Ok(Json(tm))
}

async fn forfeit_tournament_match(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    Json(payload): Json<ForfeitRequest>,
) -> Result<Json<crate::tournament::TournamentMatch>, ApiError> {
    let admin = require_admin(&state, &session).await?;
    let tm = state
        .tournaments
        .declare_forfeit(id, &payload.forfeit_player, admin.id, true)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(tm))
}

async fn unplayed_tournament_match(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
) -> Result<Json<crate::tournament::TournamentMatch>, ApiError> {
    let admin = require_admin(&state, &session).await?;
    let tm = state
        .tournaments
        .declare_match_unplayed(id, admin.id, true)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(tm))
}

async fn correct_tournament_match(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    Json(payload): Json<SubmitMatchRequest>,
) -> Result<Json<crate::tournament::TournamentMatch>, ApiError> {
    require_admin(&state, &session).await?;

    let before = state
        .tournaments
        .get_match(id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .ok_or_else(|| ApiError::bad_request("match introuvable"))?;

    let old_outcome = before.outcome;
    let tournament_id = before.tournament_id;
    let old_bracket_snapshot = state
        .tournaments
        .bracket_elo_snapshot(tournament_id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let (tm, _winner_changed) = state
        .tournaments
        .correct_match_score(id, &payload, state.k_factor)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let new_bracket_snapshot = state
        .tournaments
        .bracket_elo_snapshot(tournament_id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    sync_board_after_tournament_correction(
        &state,
        &old_bracket_snapshot,
        &new_bracket_snapshot,
        old_outcome,
        tm.outcome,
        tm.player1.as_deref().unwrap_or(""),
        tm.player2.as_deref().unwrap_or(""),
        tm.phase != crate::tournament::TournamentPhase::Pool,
        before.is_forfeit || before.is_unplayed,
    )?;

    Ok(Json(tm))
}

async fn get_player_tournaments(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<Vec<crate::tournament::PlayerTournamentResult>>, ApiError> {
    state
        .tournaments
        .player_tournament_results(&name)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

fn apply_tournament_match_to_board(
    board: &mut Leaderboard,
    tm: &crate::tournament::TournamentMatch,
    tournament_name: Option<String>,
) -> Result<(), ApiError> {
    if tm.is_forfeit {
        return Ok(());
    }

    let p1 = tm.player1.as_ref().unwrap();
    let p2 = tm.player2.as_ref().unwrap();
    let outcome = tm.outcome.ok_or_else(|| ApiError::bad_request("résultat manquant"))?;

    let update = RatingUpdate {
        player1_old: tm.player1_rating_used.unwrap_or(0.0),
        player1_new: tm.player1_rating_used.unwrap_or(0.0) + tm.player1_elo_delta,
        player2_old: tm.player2_rating_used.unwrap_or(0.0),
        player2_new: tm.player2_rating_used.unwrap_or(0.0) + tm.player2_elo_delta,
    };

    let scores = MatchScores {
        player1_objectives: tm.player1_objectives,
        player1_survivors: tm.player1_survivors,
        player2_objectives: tm.player2_objectives,
        player2_survivors: tm.player2_survivors,
    };

    board
        .apply_match_update(
            p1,
            p2,
            outcome,
            update,
            scores,
            tm.player1_army_id,
            tm.player2_army_id,
            tm.scenario_id,
            tm.scenario_name.clone(),
            Some(tm.tournament_id),
            Some(tm.phase.as_str().to_string()),
            tournament_name,
        )
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(())
}

fn maybe_apply_bracket_match(
    state: &AppState,
    tm: &crate::tournament::TournamentMatch,
) -> Result<(), ApiError> {
    let tournament_name = state
        .tournaments
        .get(tm.tournament_id)
        .ok()
        .flatten()
        .map(|tournament| tournament.name);

    {
        let mut board = state.board.lock().unwrap();
        apply_tournament_match_to_board(&mut board, tm, tournament_name)?;
        board
            .save(&state.db_path)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
    }

    state
        .tournaments
        .update_bracket_rating(
            tm.tournament_id,
            tm.player1.as_ref().unwrap(),
            tm.player1_elo_delta,
        )
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    state
        .tournaments
        .update_bracket_rating(
            tm.tournament_id,
            tm.player2.as_ref().unwrap(),
            tm.player2_elo_delta,
        )
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    Ok(())
}

fn sync_board_after_tournament_correction(
    state: &AppState,
    old_snapshot: &HashMap<i64, (f64, f64, String, String)>,
    new_snapshot: &HashMap<i64, (f64, f64, String, String)>,
    old_outcome: Option<MatchOutcome>,
    new_outcome: Option<MatchOutcome>,
    player1: &str,
    player2: &str,
    is_bracket_match: bool,
    skip_old_outcome_removal: bool,
) -> Result<(), ApiError> {
    let mut board = state.board.lock().unwrap();
    let match_ids: HashSet<i64> = old_snapshot
        .keys()
        .chain(new_snapshot.keys())
        .copied()
        .collect();

    for match_id in match_ids {
        let (old_d1, old_d2, p1, p2) = old_snapshot
            .get(&match_id)
            .cloned()
            .unwrap_or((0.0, 0.0, String::new(), String::new()));
        let (new_d1, new_d2, new_p1, new_p2) = new_snapshot
            .get(&match_id)
            .cloned()
            .unwrap_or((0.0, 0.0, String::new(), String::new()));

        let p1 = if p1.is_empty() { new_p1 } else { p1 };
        let p2 = if p2.is_empty() { new_p2 } else { p2 };

        if let Ok(player) = board.get_player_mut(&p1) {
            player.rating += new_d1 - old_d1;
        }
        if let Ok(player) = board.get_player_mut(&p2) {
            player.rating += new_d2 - old_d2;
        }
    }

    if is_bracket_match && old_outcome != new_outcome {
        if let (Some(old_outcome), Some(new_outcome)) = (old_outcome, new_outcome) {
            if !skip_old_outcome_removal {
                adjust_board_outcome(&mut board, player1, player2, old_outcome, false)
                    .map_err(|error| ApiError::bad_request(error.to_string()))?;
            }
            adjust_board_outcome(&mut board, player1, player2, new_outcome, true)
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
        }
    }

    board
        .save(&state.db_path)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(())
}

fn adjust_board_outcome(
    board: &mut Leaderboard,
    player1: &str,
    player2: &str,
    outcome: MatchOutcome,
    apply: bool,
) -> anyhow::Result<()> {
    use crate::elo::MatchScore;

    let delta = if apply { 1i32 } else { -1i32 };
    let score1 = outcome.score_for_player1();
    let score2 = match score1 {
        MatchScore::Win => MatchScore::Loss,
        MatchScore::Draw => MatchScore::Draw,
        MatchScore::Loss => MatchScore::Win,
    };

    adjust_player_match_count(board.get_player_mut(player1)?, score1, delta);
    adjust_player_match_count(board.get_player_mut(player2)?, score2, delta);
    Ok(())
}

fn adjust_player_match_count(
    player: &mut crate::player::Player,
    score: crate::elo::MatchScore,
    delta: i32,
) {
    use crate::elo::MatchScore;

    match score {
        MatchScore::Win => {
            player.wins = (player.wins as i32 + delta).max(0) as u32;
        }
        MatchScore::Draw => {
            player.draws = (player.draws as i32 + delta).max(0) as u32;
        }
        MatchScore::Loss => {
            player.losses = (player.losses as i32 + delta).max(0) as u32;
        }
    }
}

pub async fn ranking_with_stars(state: &AppState) -> Result<Vec<RankedPlayerWithStars>, ApiError> {
    let stars = state
        .tournaments
        .star_counts()
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let board = state.board.lock().unwrap();
    let resolver = crate::display_name::PlayerDisplayResolver::new(&board, state.users.as_ref());
    let ranking = board
        .ranking()
        .into_iter()
        .enumerate()
        .map(|(index, player)| RankedPlayerWithStars {
            rank: index + 1,
            star_count: stars.get(&player.name).copied().unwrap_or(0),
            display_name: resolver.resolve_player(player),
            player: player.clone(),
            top_armies: board
                .player_top_armies(&player.name, 3)
                .unwrap_or_default(),
        })
        .collect();
    Ok(ranking)
}

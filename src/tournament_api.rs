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
    tournament::{TournamentMatchStatus, TournamentStatus},
    User,
};

use crate::tournament_store::{
    AdminRegisterRequest, CompleteRegistrationListsRequest, CreateTournamentRequest, ForfeitRequest,
    RegisterRequest, RerollScenarioRequest, SetBracketScenarioPoolRequest, SetPoolScenariosRequest,
    SetupPoolsRequest, SubmitMatchRequest, UpdateBracketListsRequest,
    UpdateTournamentDetailsRequest,
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

async fn require_list_validator(
    state: &AppState,
    session: &Session,
    tournament_id: i64,
) -> Result<User, ApiError> {
    let user = require_user(state, session).await?;
    let is_validator = state
        .tournaments
        .is_list_validator(tournament_id, user.id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    if !is_validator {
        return Err(ApiError::unauthorized(
            "droits de validateur de listes requis",
        ));
    }
    Ok(user)
}

#[derive(Debug, Deserialize)]
struct ReviewRegistrationRequest {
    action: String,
}

#[derive(Debug, Deserialize)]
struct SetListValidatorRequest {
    user_id: Option<i64>,
}

struct ViewerContext {
    is_admin: bool,
    user_id: Option<i64>,
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
            user_id: None,
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
        user_id: Some(user.id),
        player_name,
    }
}

fn is_viewer_list_validator(
    tournament: &crate::tournament::Tournament,
    viewer: &ViewerContext,
) -> bool {
    matches!(
        (tournament.list_validator_user_id, viewer.user_id),
        (Some(validator_id), Some(user_id)) if validator_id == user_id
    )
}

fn tournament_visible_to_viewer(
    tournament: &crate::tournament::Tournament,
    viewer: &ViewerContext,
) -> bool {
    viewer.is_admin
        || is_viewer_list_validator(tournament, viewer)
        || tournament.status != TournamentStatus::Draft
}

fn enrich_list_validator_display_name(
    state: &AppState,
    tournament: &mut crate::tournament::Tournament,
) {
    if let Some(user_id) = tournament.list_validator_user_id {
        tournament.list_validator_display_name = state
            .users
            .get_by_id(user_id)
            .ok()
            .flatten()
            .map(|user| user.effective_display_name().to_string());
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
    let lists_public = tournament.status == crate::tournament::TournamentStatus::Completed;
    let is_list_validator = is_viewer_list_validator(tournament, viewer);

    registrations
        .into_iter()
        .map(|mut registration| {
            registration.has_army_lists = registration
                .army_list_1
                .as_ref()
                .is_some_and(|s| !s.trim().is_empty());
            registration.has_bracket_lists = registration
                .bracket_list_1
                .as_ref()
                .is_some_and(|s| !s.trim().is_empty());
            registration.has_army_list_2 = registration
                .army_list_2
                .as_ref()
                .is_some_and(|s| !s.trim().is_empty());
            registration.has_bracket_list_2 = registration
                .bracket_list_2
                .as_ref()
                .is_some_and(|s| !s.trim().is_empty());

            let is_own = viewer
                .player_name
                .as_ref()
                .is_some_and(|name| {
                    crate::store::normalize_name(name)
                        == crate::store::normalize_name(&registration.player_name)
                });

            if !armies_public && !is_list_validator && !is_own {
                registration.army_id = None;
            }
            // Listes secrètes jusqu'à la fin du tournoi (sauf pour soi / validateur).
            let can_see_lists = lists_public || is_own || is_list_validator;
            if !can_see_lists {
                registration.army_list_1 = None;
                registration.army_list_2 = None;
                registration.bracket_list_1 = None;
                registration.bracket_list_2 = None;
            }
            registration
        })
        .collect()
}

fn mask_tournament_match_lists(
    tournament: &crate::tournament::Tournament,
    matches: &mut [crate::tournament::TournamentMatch],
    viewer: &ViewerContext,
) {
    if tournament.status == TournamentStatus::Completed {
        return;
    }
    for tm in matches {
        let is_participant = viewer.player_name.as_ref().is_some_and(|name| {
            tm.player1.as_ref().is_some_and(|p| {
                crate::store::normalize_name(name) == crate::store::normalize_name(p)
            }) || tm.player2.as_ref().is_some_and(|p| {
                crate::store::normalize_name(name) == crate::store::normalize_name(p)
            })
        });
        let revealed_to_participants =
            tm.status == crate::tournament::TournamentMatchStatus::Confirmed;
        if !revealed_to_participants || !is_participant {
            tm.player1_army_list_code = None;
            tm.player2_army_list_code = None;
        }
    }
}

#[derive(Debug, Deserialize)]
struct ScenarioQuery {
    q: Option<String>,
    #[serde(default = "default_scenario_limit")]
    limit: usize,
}

fn default_scenario_limit() -> usize {
    100
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
        .route(
            "/api/scenario-content-images",
            get(list_scenario_content_images),
        )
        .route(
            "/api/scenario-packs/{slug}",
            get(get_scenario_pack).patch(update_scenario_pack),
        )
        .route(
            "/api/scenario-packs/{slug}/secondaries",
            get(list_pack_secondaries),
        )
        .route(
            "/api/scenario-packs/{slug}/common-rules",
            get(list_pack_common_rules),
        )
        .route(
            "/api/scenario-packs/{slug}/secondaries/{secondary_slug}",
            axum::routing::patch(update_pack_secondary),
        )
        .route(
            "/api/scenario-packs/{slug}/common-rules/{rule_slug}",
            axum::routing::patch(update_pack_common_rule),
        )
        .route(
            "/api/scenario-packs/{slug}/scenarios/{scenario_slug}",
            get(get_pack_scenario).patch(update_pack_scenario),
        )
        .route("/api/tournaments", get(list_tournaments).post(create_tournament))
        .route("/api/users", get(list_users))
        .route(
            "/api/tournaments/{id}",
            get(get_tournament)
                .patch(update_tournament_details)
                .delete(delete_tournament),
        )
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
            "/api/tournaments/{id}/register/lists",
            post(complete_registration_lists),
        )
        .route(
            "/api/tournaments/{id}/unregister",
            post(unregister_tournament),
        )
        .route(
            "/api/tournaments/{id}/registrations",
            post(admin_register).get(list_registrations),
        )
        .route(
            "/api/tournaments/{id}/registrations/{reg_id}/review",
            post(review_registration),
        )
        .route(
            "/api/tournaments/{id}/list-validator",
            post(set_list_validator),
        )
        .route("/api/tournaments/{id}/start", post(start_tournament))
        .route("/api/tournaments/{id}/pools", post(setup_pools))
        .route("/api/tournaments/{id}/draw-pools", post(draw_pools))
        .route(
            "/api/tournaments/{id}/pool-scenarios",
            post(set_pool_scenarios),
        )
        .route(
            "/api/tournaments/{id}/pool-scenarios/draw",
            post(draw_pool_scenarios),
        )
        .route(
            "/api/tournaments/{id}/pool-scenarios/reroll",
            post(reroll_pool_scenario),
        )
        .route(
            "/api/tournaments/{id}/bracket-scenarios",
            post(set_bracket_scenario_pool),
        )
        .route(
            "/api/tournaments/{id}/bracket-scenarios/draw",
            post(draw_bracket_scenario_pool),
        )
        .route(
            "/api/tournaments/{id}/bracket-scenarios/reroll",
            post(reroll_bracket_scenario_pool),
        )
        .route(
            "/api/tournaments/{id}/bracket-scenarios/assign",
            post(assign_bracket_scenarios),
        )
        .route(
            "/api/tournaments/{id}/bracket-lists",
            post(update_my_bracket_lists),
        )
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
            "/api/tournament-matches/{id}/start-partie",
            post(start_tournament_partie),
        )
        .route(
            "/api/tournament-matches/{id}/submit-from-partie",
            post(submit_tournament_from_partie),
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
            "/api/tournament-matches/{id}/cancel-forfeit",
            post(cancel_tournament_forfeit),
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

async fn list_scenario_content_images() -> Result<Json<Vec<String>>, ApiError> {
    crate::scenario_pack::list_scenario_content_images()
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn get_scenario_pack(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<crate::scenario_pack::ScenarioPackPage>, ApiError> {
    state
        .scenarios
        .pack_page(&slug)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .map(Json)
        .ok_or_else(|| ApiError::bad_request("pack de scénarios introuvable"))
}

async fn list_pack_secondaries(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<Vec<crate::scenario_pack::SecondaryObjective>>, ApiError> {
    state
        .scenarios
        .pack_secondaries(&slug)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn list_pack_common_rules(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<Vec<crate::scenario_pack::CommonRule>>, ApiError> {
    state
        .scenarios
        .pack_common_rules(&slug)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn get_pack_scenario(
    State(state): State<AppState>,
    Path((_pack_slug, scenario_slug)): Path<(String, String)>,
) -> Result<Json<crate::scenario_pack::ScenarioDetail>, ApiError> {
    state
        .scenarios
        .scenario_detail(&scenario_slug)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .map(Json)
        .ok_or_else(|| ApiError::bad_request("scénario introuvable"))
}

async fn update_scenario_pack(
    State(state): State<AppState>,
    session: Session,
    Path(slug): Path<String>,
    Json(body): Json<crate::scenario_pack::UpdatePackRequest>,
) -> Result<Json<crate::scenario_pack::ScenarioPack>, ApiError> {
    require_admin(&state, &session).await?;
    state
        .scenarios
        .update_pack_preamble(&slug, &body.preamble_md)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .map(Json)
        .ok_or_else(|| ApiError::bad_request("pack de scénarios introuvable"))
}

async fn update_pack_secondary(
    State(state): State<AppState>,
    session: Session,
    Path((pack_slug, secondary_slug)): Path<(String, String)>,
    Json(body): Json<crate::scenario_pack::UpdateNamedMdRequest>,
) -> Result<Json<crate::scenario_pack::SecondaryObjective>, ApiError> {
    require_admin(&state, &session).await?;
    if body.name.trim().is_empty() {
        return Err(ApiError::bad_request("le nom est requis"));
    }
    state
        .scenarios
        .update_secondary(&pack_slug, &secondary_slug, &body.name, &body.body_md)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .map(Json)
        .ok_or_else(|| ApiError::bad_request("objectif secondaire introuvable"))
}

async fn update_pack_common_rule(
    State(state): State<AppState>,
    session: Session,
    Path((pack_slug, rule_slug)): Path<(String, String)>,
    Json(body): Json<crate::scenario_pack::UpdateNamedMdRequest>,
) -> Result<Json<crate::scenario_pack::CommonRule>, ApiError> {
    require_admin(&state, &session).await?;
    if body.name.trim().is_empty() {
        return Err(ApiError::bad_request("le nom est requis"));
    }
    state
        .scenarios
        .update_common_rule(&pack_slug, &rule_slug, &body.name, &body.body_md)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .map(Json)
        .ok_or_else(|| ApiError::bad_request("règle commune introuvable"))
}

async fn update_pack_scenario(
    State(state): State<AppState>,
    session: Session,
    Path((pack_slug, scenario_slug)): Path<(String, String)>,
    Json(body): Json<crate::scenario_pack::UpdateScenarioContentRequest>,
) -> Result<Json<crate::scenario_pack::ScenarioDetail>, ApiError> {
    require_admin(&state, &session).await?;
    state
        .scenarios
        .update_scenario_content(&pack_slug, &scenario_slug, &body)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .map(Json)
        .ok_or_else(|| ApiError::bad_request("scénario introuvable"))
}

async fn list_tournaments(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<Vec<crate::tournament::TournamentListEntry>>, ApiError> {
    let viewer = viewer_context(&state, &session).await;
    let entries = state.tournaments.list_entries().map_err(|error| {
        ApiError::bad_request(error.to_string())
    })?;

    let board = state.board.lock().unwrap();
    let resolver = crate::display_name::PlayerDisplayResolver::new(&board, state.users.as_ref());

    Ok(Json(
        entries
            .into_iter()
            .filter(|entry| tournament_visible_to_viewer(&entry.tournament, &viewer))
            .map(|mut entry| {
                enrich_list_validator_display_name(&state, &mut entry.tournament);
                for top_entry in &mut entry.top_four {
                    top_entry.player_display_name =
                        Some(resolver.resolve(&top_entry.player_name));
                }
                for registration in &mut entry.registrations {
                    registration.player_display_name =
                        Some(resolver.resolve(&registration.player_name));
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

async fn delete_tournament(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    require_admin(&state, &session).await?;
    state
        .tournaments
        .delete(id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn update_tournament_details(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateTournamentDetailsRequest>,
) -> Result<Json<crate::tournament::Tournament>, ApiError> {
    require_admin(&state, &session).await?;
    let tournament = state
        .tournaments
        .update_details(id, &payload)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(tournament))
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

    if !tournament_visible_to_viewer(&detail.tournament, &viewer) {
        return Err(ApiError::bad_request("tournoi introuvable"));
    }

    enrich_list_validator_display_name(&state, &mut detail.tournament);

    detail.registrations =
        mask_registrations(&detail.tournament, detail.registrations, &viewer);
    mask_tournament_match_lists(&detail.tournament, &mut detail.matches, &viewer);

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

fn resolve_army_id_from_lists(
    state: &AppState,
    list1: &str,
    list2: &str,
    explicit: Option<u32>,
) -> Result<u32, ApiError> {
    if let Some(army_id) = explicit {
        state
            .armies
            .validate_selectable_id(army_id)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        return Ok(army_id);
    }

    let slug1 = crate::army_list::parse_army_list_faction_slug(list1)
        .ok_or_else(|| ApiError::bad_request("impossible de lire la sectorielle depuis la liste 1"))?;
    if let Some(list2_code) = crate::army_list::normalize_army_list_code(list2) {
        let slug2 = crate::army_list::parse_army_list_faction_slug(&list2_code)
            .ok_or_else(|| ApiError::bad_request("impossible de lire la sectorielle depuis la liste 2"))?;
        if !slug1.eq_ignore_ascii_case(&slug2) {
            return Err(ApiError::bad_request(
                "les deux listes doivent appartenir à la même sectorielle",
            ));
        }
    }

    let army = state
        .armies
        .get_by_slug(&slug1)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .ok_or_else(|| {
            ApiError::bad_request(format!("sectorielle inconnue pour le slug « {slug1} »"))
        })?;
    Ok(army.id)
}

async fn register_tournament(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    Json(_payload): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<crate::tournament::TournamentRegistration>), ApiError> {
    let (user, player_name) = require_player(&state, &session).await?;
    let registration = state
        .tournaments
        .register(id, &player_name, user.id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok((StatusCode::CREATED, Json(registration)))
}

async fn complete_registration_lists(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    Json(payload): Json<CompleteRegistrationListsRequest>,
) -> Result<Json<crate::tournament::TournamentRegistration>, ApiError> {
    let (_user, player_name) = require_player(&state, &session).await?;

    let list1 = crate::army_list::normalize_army_list_code(&payload.army_list_1);
    let list2 = crate::army_list::normalize_army_list_code(&payload.army_list_2);

    if list1.is_none() {
        if list2.is_some() {
            return Err(ApiError::bad_request(
                "indiquez la liste 1, ou laissez les deux listes vides pour les supprimer",
            ));
        }
        let registration = state
            .tournaments
            .complete_registration_lists(id, &player_name, "", "", None)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        return Ok(Json(registration));
    }

    let (list1, list2) = crate::army_list::require_lists(&payload.army_list_1, &payload.army_list_2)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let list2 = list2.unwrap_or_default();
    let army_id = resolve_army_id_from_lists(&state, &list1, &list2, None)?;
    let registration = state
        .tournaments
        .complete_registration_lists(id, &player_name, &list1, &list2, Some(army_id))
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(registration))
}

async fn unregister_tournament(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let (_user, player_name) = require_player(&state, &session).await?;
    state
        .tournaments
        .unregister(id, &player_name)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
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

    let player_user_id = {
        let board = state.board.lock().unwrap();
        let player = board
            .get_player(player_name)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        player.discord_username.clone()
    };
    let player_user_id = player_user_id
        .and_then(|username| state.users.get_by_username(&username).ok().flatten())
        .map(|user| user.id);
    state
        .tournaments
        .ensure_player_is_not_list_validator(id, player_user_id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let army_id = resolve_army_id_from_lists(
        &state,
        &payload.army_list_1,
        &payload.army_list_2,
        payload.army_id,
    )?;

    let registration = state
        .tournaments
        .admin_register(
            id,
            player_name,
            admin.id,
            army_id,
            &payload.army_list_1,
            &payload.army_list_2,
        )
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok((StatusCode::CREATED, Json(registration)))
}

async fn list_registrations(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
) -> Result<Json<Vec<crate::tournament::TournamentRegistration>>, ApiError> {
    let user = require_user(&state, &session).await?;
    let is_validator = state
        .tournaments
        .is_list_validator(id, user.id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    if !user.is_admin && !is_validator {
        return Err(ApiError::unauthorized("droits insuffisants"));
    }
    let detail = state
        .tournaments
        .get_detail(id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .ok_or_else(|| ApiError::bad_request("tournoi introuvable"))?;
    let viewer = ViewerContext {
        is_admin: user.is_admin,
        user_id: Some(user.id),
        player_name: None,
    };
    Ok(Json(mask_registrations(
        &detail.tournament,
        detail.registrations,
        &viewer,
    )))
}

async fn review_registration(
    State(state): State<AppState>,
    session: Session,
    Path((tournament_id, reg_id)): Path<(i64, i64)>,
    Json(payload): Json<ReviewRegistrationRequest>,
) -> Result<Json<crate::tournament::TournamentRegistration>, ApiError> {
    let validator = require_list_validator(&state, &session, tournament_id).await?;
    state
        .tournaments
        .review_registration(tournament_id, reg_id, &payload.action, validator.id)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn set_list_validator(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    Json(payload): Json<SetListValidatorRequest>,
) -> Result<Json<crate::tournament::Tournament>, ApiError> {
    require_admin(&state, &session).await?;

    if let Some(user_id) = payload.user_id {
        let user = state
            .users
            .get_by_id(user_id)
            .map_err(|error| ApiError::bad_request(error.to_string()))?
            .ok_or_else(|| ApiError::bad_request("utilisateur introuvable"))?;

        // Bloque aussi si le joueur lié est déjà inscrit (inscriptions admin sans user_id).
        let linked_player_registered = {
            let board = state.board.lock().unwrap();
            if let Some(player) = board.get_player_by_discord_username(&user.username) {
                let detail = state
                    .tournaments
                    .get_detail(id)
                    .map_err(|error| ApiError::bad_request(error.to_string()))?
                    .ok_or_else(|| ApiError::bad_request("tournoi introuvable"))?;
                detail.registrations.iter().any(|reg| {
                    crate::store::normalize_name(&reg.player_name)
                        == crate::store::normalize_name(&player.name)
                        && matches!(
                            reg.status,
                            crate::tournament::RegistrationStatus::Pending
                                | crate::tournament::RegistrationStatus::Approved
                                | crate::tournament::RegistrationStatus::Waitlisted
                        )
                })
            } else {
                false
            }
        };
        if linked_player_registered {
            return Err(ApiError::bad_request(
                "cet utilisateur est déjà inscrit au tournoi",
            ));
        }
    }

    let mut tournament = state
        .tournaments
        .set_list_validator(id, payload.user_id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    enrich_list_validator_display_name(&state, &mut tournament);
    Ok(Json(tournament))
}

async fn list_users(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<Vec<crate::user::UserResponse>>, ApiError> {
    require_admin(&state, &session).await?;
    let users = state
        .users
        .list_all()
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(users.into_iter().map(Into::into).collect()))
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

async fn draw_pools(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
) -> Result<Json<Vec<crate::tournament::Pool>>, ApiError> {
    require_admin(&state, &session).await?;
    state
        .tournaments
        .draw_pools(id)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn set_pool_scenarios(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    Json(payload): Json<SetPoolScenariosRequest>,
) -> Result<Json<Vec<crate::tournament::TournamentScenarioSlot>>, ApiError> {
    require_admin(&state, &session).await?;
    state
        .tournaments
        .set_pool_scenarios(id, &payload.scenario_ids)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn draw_pool_scenarios(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
) -> Result<Json<Vec<crate::tournament::TournamentScenarioSlot>>, ApiError> {
    require_admin(&state, &session).await?;
    state
        .tournaments
        .draw_pool_scenarios(id)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn reroll_pool_scenario(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    Json(payload): Json<RerollScenarioRequest>,
) -> Result<Json<Vec<crate::tournament::TournamentScenarioSlot>>, ApiError> {
    require_admin(&state, &session).await?;
    state
        .tournaments
        .reroll_pool_scenario(id, &payload.slot)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn set_bracket_scenario_pool(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    Json(payload): Json<SetBracketScenarioPoolRequest>,
) -> Result<Json<Vec<crate::tournament::TournamentScenarioSlot>>, ApiError> {
    require_admin(&state, &session).await?;
    state
        .tournaments
        .set_bracket_scenario_pool(id, &payload.scenario_ids)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn draw_bracket_scenario_pool(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
) -> Result<Json<Vec<crate::tournament::TournamentScenarioSlot>>, ApiError> {
    require_admin(&state, &session).await?;
    state
        .tournaments
        .draw_bracket_scenario_pool(id)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn reroll_bracket_scenario_pool(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    Json(payload): Json<RerollScenarioRequest>,
) -> Result<Json<Vec<crate::tournament::TournamentScenarioSlot>>, ApiError> {
    require_admin(&state, &session).await?;
    state
        .tournaments
        .reroll_bracket_scenario_pool_slot(id, &payload.slot)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn assign_bracket_scenarios(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
) -> Result<Json<Vec<crate::tournament::TournamentScenarioSlot>>, ApiError> {
    require_admin(&state, &session).await?;
    state
        .tournaments
        .assign_bracket_scenarios(id)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn update_my_bracket_lists(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateBracketListsRequest>,
) -> Result<Json<crate::tournament::TournamentRegistration>, ApiError> {
    let (_user, player_name) = require_player(&state, &session).await?;
    state
        .tournaments
        .update_bracket_lists(
            id,
            &player_name,
            &payload.bracket_list_1,
            &payload.bracket_list_2,
        )
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
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

#[derive(Debug, Deserialize)]
struct SubmitFromPartieRequest {
    player1_objectives: u8,
    player2_objectives: u8,
    #[serde(default)]
    player1_survivors: u16,
    #[serde(default)]
    player2_survivors: u16,
    player1_list_slot: u8,
    player2_list_slot: u8,
}

async fn start_tournament_partie(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
) -> Result<(StatusCode, Json<crate::display_name::EnrichedMatchRecord>), ApiError> {
    let user = require_user(&state, &session).await?;
    let tm = state
        .tournaments
        .get_match(id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .ok_or_else(|| ApiError::bad_request("match introuvable"))?;

    let p1 = tm
        .player1
        .clone()
        .ok_or_else(|| ApiError::bad_request("joueurs incomplets"))?;
    let p2 = tm
        .player2
        .clone()
        .ok_or_else(|| ApiError::bad_request("joueurs incomplets"))?;

    let caller_player = {
        let board = state.board.lock().unwrap();
        board
            .get_player_by_discord_username(&user.username)
            .map(|p| p.name.clone())
    };

    if !user.is_admin {
        let name = caller_player
            .as_deref()
            .ok_or_else(|| ApiError::unauthorized("profil joueur requis"))?;
        let key = crate::normalize_name(name);
        if crate::normalize_name(&p1) != key && crate::normalize_name(&p2) != key {
            return Err(ApiError::unauthorized(
                "seul un participant ou un admin peut démarrer cette partie",
            ));
        }
    }

    if tm.status == TournamentMatchStatus::Confirmed {
        return Err(ApiError::bad_request("match déjà confirmé"));
    }
    if tm.is_forfeit || tm.is_unplayed {
        return Err(ApiError::bad_request(
            "impossible de démarrer une partie sur un forfait / non joué",
        ));
    }

    // Reprendre une partie déjà liée.
    if let Some(elo_id) = tm.elo_match_id {
        let board = state.board.lock().unwrap();
        if let Some(existing) = board.get_match(elo_id) {
            if existing.status == crate::match_record::MatchStatus::InProgress {
                let resolver =
                    crate::display_name::PlayerDisplayResolver::new(&board, state.users.as_ref());
                return Ok((StatusCode::OK, Json(resolver.enrich_match(existing.clone()))));
            }
        }
    }

    if tm.status != TournamentMatchStatus::Scheduled {
        return Err(ApiError::bad_request(
            "démarrez la partie uniquement sur un match à venir",
        ));
    }

    if tm.phase != crate::tournament::TournamentPhase::Pool {
        let r1 = state
            .tournaments
            .get_registration_for_player(tm.tournament_id, &p1)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        let r2 = state
            .tournaments
            .get_registration_for_player(tm.tournament_id, &p2)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        let ready = |reg: &Option<crate::tournament::TournamentRegistration>| {
            reg.as_ref()
                .and_then(|r| r.bracket_list_1.as_ref())
                .is_some_and(|s| !s.trim().is_empty())
        };
        if !ready(&r1) || !ready(&r2) {
            return Err(ApiError::bad_request(
                "les deux joueurs doivent avoir saisi leurs listes d'arbre",
            ));
        }
    }

    let army1 = tm.player1_army_id.or_else(|| {
        state
            .tournaments
            .get_registration_for_player(tm.tournament_id, &p1)
            .ok()
            .flatten()
            .and_then(|r| r.army_id)
    });
    let army2 = tm.player2_army_id.or_else(|| {
        state
            .tournaments
            .get_registration_for_player(tm.tournament_id, &p2)
            .ok()
            .flatten()
            .and_then(|r| r.army_id)
    });
    let army1 = army1.ok_or_else(|| ApiError::bad_request("armée joueur 1 manquante"))?;
    let army2 = army2.ok_or_else(|| ApiError::bad_request("armée joueur 2 manquante"))?;

    state
        .armies
        .validate_selectable_id(army1)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    state
        .armies
        .validate_selectable_id(army2)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let created_by = caller_player
        .clone()
        .unwrap_or_else(|| p1.clone());

    let record = {
        let mut board = state.board.lock().unwrap();
        let record = board
            .start_match_with_tournament(
                &p1,
                &p2,
                army1,
                army2,
                &created_by,
                Vec::new(),
                Vec::new(),
                false,
                Some(tm.tournament_id),
                Some(tm.phase.as_str().to_string()),
                tm.scenario_id,
                tm.scenario_name.clone(),
            )
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        board
            .save(&state.db_path)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        let resolver =
            crate::display_name::PlayerDisplayResolver::new(&board, state.users.as_ref());
        resolver.enrich_match(record)
    };

    state
        .tournaments
        .set_elo_match_id(id, Some(record.record.id))
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    Ok((StatusCode::CREATED, Json(record)))
}

async fn submit_tournament_from_partie(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    Json(payload): Json<SubmitFromPartieRequest>,
) -> Result<Json<crate::tournament::TournamentMatch>, ApiError> {
    let user = require_user(&state, &session).await?;
    let tm = state
        .tournaments
        .get_match(id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .ok_or_else(|| ApiError::bad_request("match introuvable"))?;

    let elo_id = tm
        .elo_match_id
        .ok_or_else(|| ApiError::bad_request("aucune partie liée à ce match"))?;

    let player_name = if user.is_admin {
        None
    } else {
        let board = state.board.lock().unwrap();
        let player = board
            .get_player_by_discord_username(&user.username)
            .ok_or_else(|| ApiError::unauthorized("profil joueur requis"))?;
        let key = crate::normalize_name(&player.name);
        let record = board
            .get_match(elo_id)
            .ok_or_else(|| ApiError::bad_request("partie introuvable"))?;
        if crate::normalize_name(&record.player1) != key
            && crate::normalize_name(&record.player2) != key
        {
            return Err(ApiError::unauthorized(
                "vous ne participez pas à cette partie",
            ));
        }
        Some(player.name.clone())
    };

    // Propager scénario / armées depuis la partie liée.
    let (scenario_id, scenario_other, army1, army2) = {
        let board = state.board.lock().unwrap();
        let record = board
            .get_match(elo_id)
            .ok_or_else(|| ApiError::bad_request("partie introuvable"))?;
        (
            record.scenario_id,
            record.scenario_other.clone(),
            record.player1_army_id,
            record.player2_army_id,
        )
    };

    let submit = SubmitMatchRequest {
        player1_objectives: payload.player1_objectives,
        player2_objectives: payload.player2_objectives,
        player1_survivors: payload.player1_survivors,
        player2_survivors: payload.player2_survivors,
        player1_army_id: army1,
        player2_army_id: army2,
        player1_list_slot: Some(payload.player1_list_slot),
        player2_list_slot: Some(payload.player2_list_slot),
        scenario_id,
        scenario_other,
    };

    let tm = state
        .tournaments
        .submit_match(
            id,
            &submit,
            user.id,
            user.is_admin,
            player_name.as_deref(),
            state.k_factor,
        )
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    // Clôturer la partie liée sans ELO (appliqué à la confirmation / finalisation poules).
    {
        let outcome = tm
            .outcome
            .ok_or_else(|| ApiError::bad_request("résultat manquant"))?;
        let scores = MatchScores {
            player1_objectives: tm.player1_objectives,
            player1_survivors: tm.player1_survivors,
            player2_objectives: tm.player2_objectives,
            player2_survivors: tm.player2_survivors,
        };
        let mut board = state.board.lock().unwrap();
        if let Some(code) = tm.player1_army_list_code.as_deref() {
            if let Ok(list) = state.army_lists.get_or_create(code) {
                let _ = board.update_match_army_list(
                    elo_id,
                    tm.player1.as_deref().unwrap_or(""),
                    list.id,
                    &list.code,
                    list.army_id,
                );
            }
        }
        if let Some(code) = tm.player2_army_list_code.as_deref() {
            if let Ok(list) = state.army_lists.get_or_create(code) {
                let _ = board.update_match_army_list(
                    elo_id,
                    tm.player2.as_deref().unwrap_or(""),
                    list.id,
                    &list.code,
                    list.army_id,
                );
            }
        }
        board
            .complete_match(elo_id, outcome, state.k_factor, scores)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        board
            .save(&state.db_path)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
    }

    if tm.status == TournamentMatchStatus::Confirmed
        && !tm.is_forfeit
        && tm.phase != crate::tournament::TournamentPhase::Pool
    {
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
    let user = require_user(&state, &session).await?;
    let player_name = {
        let board = state.board.lock().unwrap();
        board
            .get_player_by_discord_username(&user.username)
            .map(|p| p.name.clone())
    };

    let tm = state
        .tournaments
        .declare_forfeit(
            id,
            &payload.forfeit_player,
            user.id,
            user.is_admin,
            player_name.as_deref(),
        )
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    // Abandonner une partie en cours liée.
    if let Some(elo_id) = tm.elo_match_id {
        let mut board = state.board.lock().unwrap();
        if board
            .get_match(elo_id)
            .is_some_and(|m| m.status == crate::match_record::MatchStatus::InProgress)
        {
            let _ = board.delete_match(elo_id);
            let _ = board.save(&state.db_path);
        }
        drop(board);
        let _ = state.tournaments.set_elo_match_id(id, None);
    }

    Ok(Json(
        state
            .tournaments
            .get_match(id)
            .ok()
            .flatten()
            .unwrap_or(tm),
    ))
}

async fn cancel_tournament_forfeit(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
) -> Result<Json<crate::tournament::TournamentMatch>, ApiError> {
    require_admin(&state, &session).await?;
    state
        .tournaments
        .cancel_forfeit(id, true)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
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

    if let Some(elo_id) = tm.elo_match_id {
        if board.get_match(elo_id).is_some() {
            board
                .apply_tournament_elo_to_existing_match(
                    elo_id,
                    outcome,
                    update,
                    scores,
                    Some(tm.tournament_id),
                    Some(tm.phase.as_str().to_string()),
                    tournament_name,
                    tm.player1_army_list_code.clone(),
                    tm.player2_army_list_code.clone(),
                    tm.scenario_id,
                    tm.scenario_other.clone(),
                    tm.scenario_name.clone(),
                    tm.player1_army_id,
                    tm.player2_army_id,
                    tm.player1_army_list_id,
                    tm.player2_army_list_id,
                )
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
            return Ok(());
        }
    }

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
            tm.scenario_other.clone(),
            tm.scenario_name.clone(),
            Some(tm.tournament_id),
            Some(tm.phase.as_str().to_string()),
            tournament_name,
            tm.player1_army_list_code.clone(),
            tm.player2_army_list_code.clone(),
            tm.player1_army_list_id,
            tm.player2_army_list_id,
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

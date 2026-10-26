use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use tower_sessions::Session;

use crate::{
    api::{ApiError, AppState},
    auth,
    tts_profile::{
        ProfileFormUpdate, ProfileListQuery, ProfileUpdate, TtsProfileArmyCount, TtsProfileDetail,
        TtsProfileLabel, TtsProfileSummary,
    },
    User,
};

pub fn tts_profile_routes() -> Router<AppState> {
    Router::new()
        .route("/api/tts-profiles/armies", get(list_profile_armies))
        .route("/api/tts-profiles/labels", get(list_profile_labels))
        .route("/api/tts-profiles", get(list_profiles))
        .route(
            "/api/tts-profiles/{id}",
            get(get_profile).patch(update_profile),
        )
}

#[derive(Debug, Deserialize)]
struct ListProfilesQuery {
    army_id: Option<u32>,
    #[serde(default)]
    unassigned: bool,
    /// Inclut les profils encore rattachés à la faction parente. Vrai par défaut.
    #[serde(default = "default_true")]
    include_faction: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
struct UpdateFormBody {
    id: i64,
    #[serde(rename = "nom")]
    isc: String,
    #[serde(default)]
    loadout: String,
    nom_tts: String,
    #[serde(default)]
    description_tts: String,
    #[serde(default)]
    silhouette_size: Option<String>,
    #[serde(default)]
    marker_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateProfileBody {
    army_id: Option<i64>,
    #[serde(rename = "nom")]
    isc: String,
    #[serde(default)]
    loadout: String,
    nom_tts: String,
    #[serde(default)]
    description_tts: String,
    #[serde(default)]
    silhouette_size: Option<String>,
    #[serde(default)]
    marker_name: Option<String>,
    #[serde(default)]
    forms: Vec<UpdateFormBody>,
    #[serde(default)]
    sectorial_ids: Vec<i64>,
}

async fn require_admin(state: &AppState, session: &Session) -> Result<User, ApiError> {
    let user = auth::current_user(state.users.as_ref(), session)
        .await
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .ok_or_else(|| ApiError::unauthorized("non authentifié"))?;
    if !user.is_admin {
        return Err(ApiError::unauthorized("droits administrateur requis"));
    }
    Ok(user)
}

async fn list_profile_armies(
    State(state): State<AppState>,
) -> Result<Json<Vec<TtsProfileArmyCount>>, ApiError> {
    state
        .tts_profiles
        .army_counts()
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn list_profile_labels(
    State(state): State<AppState>,
) -> Result<Json<Vec<TtsProfileLabel>>, ApiError> {
    state
        .tts_profiles
        .list_labels()
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn list_profiles(
    State(state): State<AppState>,
    Query(query): Query<ListProfilesQuery>,
) -> Result<Json<Vec<TtsProfileSummary>>, ApiError> {
    let filter = profile_query(&state, &query)?;
    state
        .tts_profiles
        .list(&filter)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

fn profile_query(
    state: &AppState,
    query: &ListProfilesQuery,
) -> Result<ProfileListQuery, ApiError> {
    if query.unassigned {
        return Ok(ProfileListQuery {
            army_id: None,
            also_army_id: None,
            unassigned: true,
        });
    }
    let army_id = query
        .army_id
        .ok_or_else(|| ApiError::bad_request("indique une sectorielle"))?;
    let army = state
        .armies
        .get(army_id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .ok_or_else(|| ApiError::bad_request("sectorielle introuvable"))?;
    let also_army_id = if query.include_faction && army.parent_id != army.id && army.parent_id != 0
    {
        Some(i64::from(army.parent_id))
    } else {
        None
    };
    Ok(ProfileListQuery {
        army_id: Some(i64::from(army.id)),
        also_army_id,
        unassigned: false,
    })
}

async fn get_profile(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<TtsProfileDetail>, ApiError> {
    state
        .tts_profiles
        .get(id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .map(Json)
        .ok_or_else(|| ApiError::bad_request("profil introuvable"))
}

async fn update_profile(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    Json(body): Json<UpdateProfileBody>,
) -> Result<Json<TtsProfileDetail>, ApiError> {
    require_admin(&state, &session).await?;
    let update = ProfileUpdate {
        army_id: body.army_id,
        isc: body.isc,
        loadout: body.loadout,
        nom_tts: body.nom_tts,
        description_tts: body.description_tts,
        silhouette_size: body.silhouette_size,
        marker_name: body.marker_name,
        forms: body
            .forms
            .into_iter()
            .map(|form| ProfileFormUpdate {
                id: form.id,
                isc: form.isc,
                loadout: form.loadout,
                nom_tts: form.nom_tts,
                description_tts: form.description_tts,
                silhouette_size: form.silhouette_size,
                marker_name: form.marker_name,
            })
            .collect(),
        sectorial_ids: body.sectorial_ids,
    };
    state
        .tts_profiles
        .update(id, &update)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

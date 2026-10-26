use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{header, StatusCode},
    response::Response,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use crate::{
    api::{ApiError, AppState},
    auth,
    tts_figurine::{
        FigurineListFilter, FigurineListMeta, FigurineListPage, FigurineWrite, TtsFigurine,
        TtsFigurinePickPref,
    },
    tts_map::{self, MAX_PICTURE_BYTES},
    tts_object::{self, ModelForm, ModelProfile},
    tts_profile::{TtsFigurineLabel, TtsProfileForm, TtsProfileLoadout},
    User,
};

const FIGURINE_PAGE_DEFAULT: u32 = 20;
const FIGURINE_PAGE_MAX: u32 = 100;

const UPLOAD_BODY_LIMIT: usize = 12 * 1024 * 1024;

pub fn tts_figurine_routes() -> Router<AppState> {
    let uploads = Router::new()
        .route(
            "/api/tts-figurines/{id}/photo",
            axum::routing::post(upload_photo),
        )
        .layer(DefaultBodyLimit::max(UPLOAD_BODY_LIMIT));

    Router::new()
        .route(
            "/api/me/tts-figurine-picks",
            get(list_my_figurine_picks).put(upsert_my_figurine_pick),
        )
        .route(
            "/api/tts-figurines",
            get(list_figurines).post(create_figurine),
        )
        .route("/api/tts-figurines/meta", get(list_figurine_meta))
        .route("/api/tts-figurines/labels", get(list_figurine_labels))
        .route(
            "/api/tts-figurines/{id}",
            axum::routing::patch(update_figurine).delete(delete_figurine),
        )
        .route(
            "/api/tts-figurines/{id}/photo",
            get(serve_photo).delete(clear_photo),
        )
        .route(
            "/api/tts-figurines/{id}/loadouts",
            get(list_figurine_loadouts),
        )
        .route("/api/tts-sectorial-color", get(sectorial_color))
        .route("/api/tts-figurines/{id}/object", get(figurine_object))
        .merge(uploads)
}

#[derive(Debug, Deserialize)]
struct FigurineBody {
    /// Loadout TTS lié : nom / loadout sont alors recopiés depuis le profil.
    #[serde(default)]
    tts_loadout_id: Option<i64>,
    /// Requis seulement pour une figurine orpheline (sans `tts_loadout_id`).
    #[serde(rename = "nom", default)]
    isc: String,
    #[serde(default)]
    loadout: String,
    #[serde(default)]
    painter: String,
    #[serde(default)]
    asset_bundle_url: String,
}

impl FigurineBody {
    fn write(&self) -> FigurineWrite {
        FigurineWrite {
            tts_loadout_id: self.tts_loadout_id.filter(|id| *id > 0),
            isc: self.isc.clone(),
            loadout: self.loadout.clone(),
            painter: self.painter.clone(),
            asset_bundle_url: self.asset_bundle_url.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ArmyQuery {
    army_id: i64,
}

#[derive(Debug, Deserialize)]
struct FigurineListQuery {
    limit: Option<u32>,
    offset: Option<u32>,
    q: Option<String>,
    painter: Option<String>,
    army_id: Option<i64>,
    /// Catalogue complet (listes TTS) : ignore la pagination.
    all: Option<bool>,
    /// Figurines avec ISC / loadout / peintre / asset bundle / photo manquant.
    incomplete: Option<bool>,
    /// Figurines sans loadout TTS (orphelines / legacy).
    unlinked: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct ObjectQuery {
    profile_id: Option<i64>,
    form_id: Option<i64>,
    army_id: i64,
    color: Option<String>,
}

#[derive(Debug, Serialize)]
struct FactionColorBody {
    color: String,
}

#[derive(Debug, Deserialize)]
struct PhotoQuery {
    #[serde(default)]
    thumb: Option<String>,
}

impl PhotoQuery {
    fn wants_thumb(&self) -> bool {
        matches!(
            self.thumb.as_deref(),
            Some("1") | Some("true") | Some("yes") | Some("")
        )
    }
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

#[derive(Debug, Deserialize)]
struct FigurinePickBody {
    tts_loadout_id: i64,
    figurine_id: i64,
}

async fn list_my_figurine_picks(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<Vec<TtsFigurinePickPref>>, ApiError> {
    let user = require_user(&state, &session).await?;
    state
        .tts_figurines
        .list_pick_prefs(user.id)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn upsert_my_figurine_pick(
    State(state): State<AppState>,
    session: Session,
    Json(body): Json<FigurinePickBody>,
) -> Result<Json<TtsFigurinePickPref>, ApiError> {
    let user = require_user(&state, &session).await?;
    state
        .tts_figurines
        .upsert_pick_pref(user.id, body.tts_loadout_id, body.figurine_id)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn list_figurine_labels(
    State(state): State<AppState>,
    Query(query): Query<ArmyQuery>,
) -> Result<Json<Vec<TtsFigurineLabel>>, ApiError> {
    require_army(&state, query.army_id)?;
    state
        .tts_profiles
        .figurine_labels_for_army(query.army_id)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn list_figurine_loadouts(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(query): Query<ArmyQuery>,
) -> Result<Json<Vec<TtsProfileLoadout>>, ApiError> {
    require_army(&state, query.army_id)?;
    let figurine = state
        .tts_figurines
        .get(id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .ok_or_else(|| ApiError::bad_request("figurine introuvable"))?;
    let mut loadouts = state
        .tts_profiles
        .loadouts_for_isc(&figurine.isc, query.army_id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    for loadout in &mut loadouts {
        loadout.marker_name = Some(tts_object::marker_label(
            loadout.marker_name.as_deref(),
            loadout.silhouette_type.as_deref(),
            loadout.silhouette_size.as_deref(),
        ));
    }
    Ok(Json(loadouts))
}

async fn figurine_object(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(query): Query<ObjectQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let figurine = state
        .tts_figurines
        .get(id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .ok_or_else(|| ApiError::bad_request("figurine introuvable"))?;
    if figurine.asset_bundle_url.is_empty() {
        return Err(ApiError::bad_request(
            "cette figurine n'a pas d'asset bundle",
        ));
    }
    let color = diffuse_color(&state, query.army_id, query.color.as_deref())?;
    let object = if let Some(form_id) = query.form_id {
        let (form, profile_id) = state
            .tts_profiles
            .get_form(form_id)
            .map_err(|error| ApiError::bad_request(error.to_string()))?
            .ok_or_else(|| ApiError::bad_request("forme introuvable"))?;
        let in_army = state
            .tts_profiles
            .profile_in_army(profile_id, query.army_id)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        if !in_army {
            return Err(ApiError::bad_request(
                "ce loadout n'est pas dans cette sectorielle",
            ));
        }
        if figurine.tts_loadout_id != Some(form.tts_loadout_id)
            && !same_isc(&figurine.isc, &form.isc)
        {
            return Err(ApiError::bad_request(
                "ce loadout ne correspond pas à ce nom",
            ));
        }
        tts_object::saved_model(
            &figurine.asset_bundle_url,
            &model_from_form(&form),
            color,
        )
    } else {
        let profile_id = query
            .profile_id
            .ok_or_else(|| ApiError::bad_request("loadout manquant"))?;
        let in_army = state
            .tts_profiles
            .profile_in_army(profile_id, query.army_id)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        if !in_army {
            return Err(ApiError::bad_request(
                "ce loadout n'est pas dans cette sectorielle",
            ));
        }
        let profile = state
            .tts_profiles
            .get(profile_id)
            .map_err(|error| ApiError::bad_request(error.to_string()))?
            .ok_or_else(|| ApiError::bad_request("profil introuvable"))?;
        if figurine.tts_loadout_id != Some(profile.tts_loadout_id)
            && !same_isc(&figurine.isc, &profile.isc)
        {
            return Err(ApiError::bad_request(
                "ce loadout ne correspond pas à ce nom",
            ));
        }
        let all_figurines = state
            .tts_figurines
            .list()
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        let form_assets = form_asset_bundles_from_profile(
            &profile,
            &all_figurines,
            Some(figurine.painter.as_str()),
        );
        tts_object::saved_model_with_form_assets(
            &figurine.asset_bundle_url,
            &model_profile(&profile),
            color,
            &form_assets,
        )
    };
    Ok(Json(tts_object::save_file(object)))
}

fn form_asset_bundles_from_profile(
    profile: &crate::tts_profile::TtsProfileDetail,
    figurines: &[crate::tts_figurine::TtsFigurine],
    preferred_painter: Option<&str>,
) -> std::collections::HashMap<i64, String> {
    let mut assets = std::collections::HashMap::new();
    for form in profile
        .forms
        .iter()
        .filter(|form| form.state_index != crate::tts_figurine::PERIPHERAL_STATE_INDEX)
    {
        let fallback = crate::tts_figurine::parent_fallback_loadout_id(
            form.state_index,
            form.silhouette_size.as_deref(),
            profile.tts_loadout_id,
            profile.silhouette_size.as_deref(),
        );
        if let Some(url) = crate::tts_figurine::best_asset_bundle_for_form(
            figurines,
            form.tts_loadout_id,
            fallback,
            &form.isc,
            &form.loadout,
            preferred_painter,
        ) {
            assets.insert(form.id, url);
        }
    }
    assets
}

async fn sectorial_color(
    State(state): State<AppState>,
    Query(query): Query<ArmyQuery>,
) -> Result<Json<FactionColorBody>, ApiError> {
    let army = load_army(&state, query.army_id)?;
    Ok(Json(FactionColorBody {
        color: configured_color(&army.slug),
    }))
}

fn require_army(state: &AppState, army_id: i64) -> Result<(), ApiError> {
    load_army(state, army_id).map(|_| ())
}

fn load_army(state: &AppState, army_id: i64) -> Result<crate::army::Army, ApiError> {
    if army_id <= 0 || army_id > i64::from(u32::MAX) {
        return Err(ApiError::bad_request("sectorielle invalide"));
    }
    state
        .armies
        .get(army_id as u32)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .ok_or_else(|| ApiError::bad_request("sectorielle introuvable"))
}

fn diffuse_color(
    state: &AppState,
    army_id: i64,
    raw: Option<&str>,
) -> Result<tts_object::DiffuseColor, ApiError> {
    let army = load_army(state, army_id)?;
    if let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) {
        return tts_object::parse_color(raw)
            .ok_or_else(|| ApiError::bad_request("couleur invalide"));
    }
    tts_object::parse_color(&configured_color(&army.slug))
        .ok_or_else(|| ApiError::bad_request("couleur invalide"))
}

fn configured_color(slug: &str) -> String {
    tts_object::sectorial_color_hex(slug).unwrap_or_else(|| "FFFFFF".to_string())
}

fn same_isc(left: &str, right: &str) -> bool {
    left.trim().eq_ignore_ascii_case(right.trim())
}

fn model_profile(profile: &crate::tts_profile::TtsProfileDetail) -> ModelProfile {
    ModelProfile {
        nom_tts: profile.nom_tts.clone(),
        description_tts: profile.description_tts.clone(),
        marker_name: profile.marker_name.clone(),
        silhouette_size: profile.silhouette_size.clone(),
        silhouette_type: profile.silhouette_type.clone(),
        marker_mesh_url: profile.marker_mesh_url.clone(),
        forms: profile
            .forms
            .iter()
            .map(|form| ModelForm {
                id: form.id,
                state_index: form.state_index,
                nom_tts: form.nom_tts.clone(),
                description_tts: form.description_tts.clone(),
                marker_name: form.marker_name.clone(),
                silhouette_size: form.silhouette_size.clone(),
                silhouette_type: form.silhouette_type.clone(),
                mesh_url: form.mesh_url.clone(),
                marker_mesh_url: form.marker_mesh_url.clone(),
            })
            .collect(),
    }
}

fn model_from_form(form: &TtsProfileForm) -> ModelProfile {
    ModelProfile {
        nom_tts: form.nom_tts.clone(),
        description_tts: form.description_tts.clone(),
        marker_name: form.marker_name.clone(),
        silhouette_size: form.silhouette_size.clone(),
        silhouette_type: form.silhouette_type.clone(),
        marker_mesh_url: form.marker_mesh_url.clone(),
        forms: Vec::new(),
    }
}

async fn list_figurine_meta(
    State(state): State<AppState>,
) -> Result<Json<FigurineListMeta>, ApiError> {
    state
        .tts_figurines
        .list_meta()
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn list_figurines(
    State(state): State<AppState>,
    Query(query): Query<FigurineListQuery>,
) -> Result<Json<FigurineListPage>, ApiError> {
    if let Some(army_id) = query.army_id {
        if army_id > 0 {
            require_army(&state, army_id)?;
        }
    }
    let all = query.all.unwrap_or(false);
    let limit = if all {
        u32::MAX
    } else {
        query
            .limit
            .unwrap_or(FIGURINE_PAGE_DEFAULT)
            .clamp(1, FIGURINE_PAGE_MAX)
    };
    let offset = if all { 0 } else { query.offset.unwrap_or(0) };
    let mut page = state
        .tts_figurines
        .list_page(&FigurineListFilter {
            limit,
            offset,
            q: query.q,
            painter: query.painter,
            army_id: query.army_id,
            incomplete: query.incomplete.unwrap_or(false),
            unlinked: query.unlinked.unwrap_or(false),
        })
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    page.items = attach_sectorials(&state, page.items)?;
    Ok(Json(page))
}

async fn create_figurine(
    State(state): State<AppState>,
    session: Session,
    Json(body): Json<FigurineBody>,
) -> Result<Json<TtsFigurine>, ApiError> {
    require_admin(&state, &session).await?;
    let figurine = state
        .tts_figurines
        .create(&body.write())
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(attach_sectorial(&state, figurine)?))
}

async fn update_figurine(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    Json(body): Json<FigurineBody>,
) -> Result<Json<TtsFigurine>, ApiError> {
    require_admin(&state, &session).await?;
    let figurine = state
        .tts_figurines
        .update(id, &body.write())
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(attach_sectorial(&state, figurine)?))
}

async fn delete_figurine(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    require_admin(&state, &session).await?;
    state
        .tts_figurines
        .delete(id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn upload_photo(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    multipart: Multipart,
) -> Result<Json<TtsFigurine>, ApiError> {
    require_admin(&state, &session).await?;
    let bytes = read_photo(multipart).await?;
    let figurine = state
        .tts_figurines
        .set_photo(id, &bytes)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(attach_sectorial(&state, figurine)?))
}

async fn clear_photo(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
) -> Result<Json<TtsFigurine>, ApiError> {
    require_admin(&state, &session).await?;
    let figurine = state
        .tts_figurines
        .clear_photo(id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(attach_sectorial(&state, figurine)?))
}

fn attach_sectorials(
    state: &AppState,
    mut figurines: Vec<TtsFigurine>,
) -> Result<Vec<TtsFigurine>, ApiError> {
    let by_isc = state
        .tts_profiles
        .sectorial_ids_by_isc()
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    for figurine in &mut figurines {
        let key = figurine.isc.trim().to_ascii_lowercase();
        figurine.sectorial_ids = by_isc.get(&key).cloned().unwrap_or_default();
    }
    Ok(figurines)
}

fn attach_sectorial(state: &AppState, mut figurine: TtsFigurine) -> Result<TtsFigurine, ApiError> {
    let key = figurine.isc.trim().to_ascii_lowercase();
    figurine.sectorial_ids = state
        .tts_profiles
        .sectorial_ids_by_isc()
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .get(&key)
        .cloned()
        .unwrap_or_default();
    Ok(figurine)
}

async fn serve_photo(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(query): Query<PhotoQuery>,
) -> Result<Response, ApiError> {
    let Some(path) = state
        .tts_figurines
        .photo_file_path(id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
    else {
        return Err(ApiError::bad_request("image introuvable"));
    };
    if query.wants_thumb() {
        if let Ok(thumb) = tts_map::ensure_thumbnail(&path) {
            let bytes =
                std::fs::read(&thumb).map_err(|_| ApiError::bad_request("image introuvable"))?;
            return file_response(bytes, "image/jpeg");
        }
    }
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("photo");
    let bytes = std::fs::read(&path).map_err(|_| ApiError::bad_request("image introuvable"))?;
    file_response(bytes, tts_map::mime_from_filename(filename))
}

async fn read_photo(mut multipart: Multipart) -> Result<Vec<u8>, ApiError> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| ApiError::bad_request(error.to_string()))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let bytes = field
            .bytes()
            .await
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        if bytes.len() > MAX_PICTURE_BYTES {
            return Err(ApiError::bad_request(format!(
                "fichier trop volumineux (max {MAX_PICTURE_BYTES} octets)"
            )));
        }
        if bytes.is_empty() {
            return Err(ApiError::bad_request("image vide"));
        }
        return Ok(bytes.to_vec());
    }
    Err(ApiError::bad_request("image manquante"))
}

fn file_response(bytes: Vec<u8>, content_type: &'static str) -> Result<Response, ApiError> {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "private, max-age=3600")
        .body(Body::from(bytes))
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

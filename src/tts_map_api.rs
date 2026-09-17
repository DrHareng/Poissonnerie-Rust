use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{header, HeaderValue, StatusCode},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use tower_sessions::Session;

use crate::{
    api::{ApiError, AppState},
    auth,
    tts_map::{self, TtsMapDetail, TtsModuleUpdate},
    User,
};

const UPLOAD_BODY_LIMIT: usize = 20 * 1024 * 1024;

pub fn tts_map_routes() -> Router<AppState> {
    let uploads = Router::new()
        .route("/api/tts-maps/{id}/json", post(upload_map_json))
        .route("/api/tts-maps/{id}/pictures", post(upload_map_picture))
        .layer(DefaultBodyLimit::max(UPLOAD_BODY_LIMIT));

    Router::new()
        .route("/api/tts-maps", get(list_maps).post(create_map))
        .route(
            "/api/tts-maps/{id}",
            get(get_map).patch(update_map).delete(delete_map),
        )
        .route("/api/tts-maps/{id}/json", get(download_map_json))
        .route(
            "/api/tts-maps/{id}/pictures/{filename}",
            get(serve_map_picture).delete(delete_map_picture),
        )
        .route(
            "/api/tts-map-content-images",
            get(list_content_images),
        )
        .route(
            "/api/tts-module-updates",
            get(list_updates).post(create_update),
        )
        .route(
            "/api/tts-module-updates/{id}",
            axum::routing::patch(update_update).delete(delete_update),
        )
        .merge(uploads)
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

async fn list_maps(
    State(state): State<AppState>,
) -> Result<Json<Vec<tts_map::TtsMapSummary>>, ApiError> {
    state
        .tts_maps
        .list_maps()
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn get_map(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<TtsMapDetail>, ApiError> {
    state
        .tts_maps
        .get_map(id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .map(Json)
        .ok_or_else(|| ApiError::bad_request("map introuvable"))
}

#[derive(Debug, Deserialize)]
struct CreateMapRequest {
    name: String,
}

#[derive(Debug, Deserialize)]
struct RenameMapRequest {
    name: String,
}

async fn create_map(
    State(state): State<AppState>,
    session: Session,
    Json(payload): Json<CreateMapRequest>,
) -> Result<Json<TtsMapDetail>, ApiError> {
    require_admin(&state, &session).await?;
    state
        .tts_maps
        .create_map(&payload.name)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn update_map(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    Json(payload): Json<RenameMapRequest>,
) -> Result<Json<TtsMapDetail>, ApiError> {
    require_admin(&state, &session).await?;
    state
        .tts_maps
        .rename_map(id, &payload.name)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn delete_map(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    require_admin(&state, &session).await?;
    state
        .tts_maps
        .delete_map(id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn upload_map_json(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    multipart: Multipart,
) -> Result<Json<TtsMapDetail>, ApiError> {
    require_admin(&state, &session).await?;
    let (filename, bytes) = read_upload(multipart, tts_map::MAX_JSON_BYTES).await?;
    state
        .tts_maps
        .save_json(id, &filename, &bytes)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn download_map_json(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Response, ApiError> {
    let Some((path, filename)) = state
        .tts_maps
        .json_file_path(id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
    else {
        return Err(ApiError::bad_request("JSON introuvable"));
    };
    let bytes = std::fs::read(&path).map_err(|_| ApiError::bad_request("JSON introuvable"))?;
    file_response(bytes, tts_map::mime_from_filename(&filename), Some(&filename))
}

async fn upload_map_picture(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    multipart: Multipart,
) -> Result<Json<TtsMapDetail>, ApiError> {
    require_admin(&state, &session).await?;
    let (filename, bytes) = read_upload(multipart, tts_map::MAX_PICTURE_BYTES).await?;
    state
        .tts_maps
        .add_picture(id, &filename, &bytes)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn serve_map_picture(
    State(state): State<AppState>,
    Path((id, filename)): Path<(i64, String)>,
) -> Result<Response, ApiError> {
    let Some(path) = state
        .tts_maps
        .picture_file_path(id, &filename)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
    else {
        return Err(ApiError::bad_request("image introuvable"));
    };
    let bytes = std::fs::read(&path).map_err(|_| ApiError::bad_request("image introuvable"))?;
    file_response(bytes, tts_map::mime_from_filename(&filename), None)
}

async fn delete_map_picture(
    State(state): State<AppState>,
    session: Session,
    Path((id, filename)): Path<(i64, String)>,
) -> Result<Json<TtsMapDetail>, ApiError> {
    require_admin(&state, &session).await?;
    state
        .tts_maps
        .delete_picture(id, &filename)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn list_content_images(
    State(state): State<AppState>,
) -> Result<Json<Vec<tts_map::TtsContentImage>>, ApiError> {
    state
        .tts_maps
        .list_content_images()
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn list_updates(
    State(state): State<AppState>,
) -> Result<Json<Vec<TtsModuleUpdate>>, ApiError> {
    state
        .tts_maps
        .list_updates()
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

#[derive(Debug, Deserialize)]
struct UpdateBodyRequest {
    body_md: String,
}

async fn create_update(
    State(state): State<AppState>,
    session: Session,
    Json(payload): Json<UpdateBodyRequest>,
) -> Result<Json<TtsModuleUpdate>, ApiError> {
    require_admin(&state, &session).await?;
    state
        .tts_maps
        .create_update(&payload.body_md)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn update_update(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateBodyRequest>,
) -> Result<Json<TtsModuleUpdate>, ApiError> {
    require_admin(&state, &session).await?;
    state
        .tts_maps
        .update_update(id, &payload.body_md)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn delete_update(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    require_admin(&state, &session).await?;
    state
        .tts_maps
        .delete_update(id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn read_upload(
    mut multipart: Multipart,
    max_bytes: usize,
) -> Result<(String, Vec<u8>), ApiError> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| ApiError::bad_request(error.to_string()))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let filename = field
            .file_name()
            .map(str::to_string)
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| "fichier".to_string());
        let bytes = field
            .bytes()
            .await
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        if bytes.len() > max_bytes {
            return Err(ApiError::bad_request(format!(
                "fichier trop volumineux (max {max_bytes} octets)"
            )));
        }
        return Ok((filename, bytes.to_vec()));
    }
    Err(ApiError::bad_request("fichier manquant"))
}

fn file_response(
    bytes: Vec<u8>,
    content_type: &'static str,
    download_name: Option<&str>,
) -> Result<Response, ApiError> {
    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "public, max-age=3600");
    if let Some(name) = download_name {
        let value = format!("attachment; filename=\"{}\"", name.replace('"', "_"));
        let header_value = HeaderValue::from_str(&value)
            .unwrap_or_else(|_| HeaderValue::from_static("attachment"));
        builder = builder.header(header::CONTENT_DISPOSITION, header_value);
    }
    builder
        .body(Body::from(bytes))
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

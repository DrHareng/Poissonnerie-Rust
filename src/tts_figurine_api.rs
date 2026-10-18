use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{header, StatusCode},
    response::Response,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use tower_sessions::Session;

use crate::{
    api::{ApiError, AppState},
    auth,
    tts_figurine::{FigurineWrite, TtsFigurine},
    tts_map::{self, MAX_PICTURE_BYTES},
    User,
};

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
            "/api/tts-figurines",
            get(list_figurines).post(create_figurine),
        )
        .route(
            "/api/tts-figurines/{id}",
            axum::routing::patch(update_figurine).delete(delete_figurine),
        )
        .route(
            "/api/tts-figurines/{id}/photo",
            get(serve_photo).delete(clear_photo),
        )
        .merge(uploads)
}

#[derive(Debug, Deserialize)]
struct FigurineBody {
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
            isc: self.isc.clone(),
            loadout: self.loadout.clone(),
            painter: self.painter.clone(),
            asset_bundle_url: self.asset_bundle_url.clone(),
        }
    }
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

async fn list_figurines(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<Vec<TtsFigurine>>, ApiError> {
    require_admin(&state, &session).await?;
    state
        .tts_figurines
        .list()
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn create_figurine(
    State(state): State<AppState>,
    session: Session,
    Json(body): Json<FigurineBody>,
) -> Result<Json<TtsFigurine>, ApiError> {
    require_admin(&state, &session).await?;
    state
        .tts_figurines
        .create(&body.write())
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn update_figurine(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    Json(body): Json<FigurineBody>,
) -> Result<Json<TtsFigurine>, ApiError> {
    require_admin(&state, &session).await?;
    state
        .tts_figurines
        .update(id, &body.write())
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
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
    state
        .tts_figurines
        .set_photo(id, &bytes)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn clear_photo(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
) -> Result<Json<TtsFigurine>, ApiError> {
    require_admin(&state, &session).await?;
    state
        .tts_figurines
        .clear_photo(id)
        .map(Json)
        .map_err(|error| ApiError::bad_request(error.to_string()))
}

async fn serve_photo(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i64>,
    Query(query): Query<PhotoQuery>,
) -> Result<Response, ApiError> {
    require_admin(&state, &session).await?;
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

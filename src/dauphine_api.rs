use axum::{extract::State, routing::get, Json, Router};

use crate::{
    api::{ApiError, AppState},
    dauphine::DauphineEdition,
};

pub fn dauphine_routes() -> Router<AppState> {
    Router::new().route("/api/dauphine/editions", get(list_editions))
}

async fn list_editions(
    State(state): State<AppState>,
) -> Result<Json<Vec<DauphineEdition>>, ApiError> {
    let editions = state
        .dauphine
        .list_editions()
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Json(editions))
}

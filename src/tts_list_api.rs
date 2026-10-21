//! Expansion d'un code Army en slots TTS (profils + figurines candidates) et export de sac.

use std::collections::HashSet;

use axum::{extract::State, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    api::{ApiError, AppState},
    army_list::{self, parse_army_list, ArmyListMember},
    tts_figurine::TtsFigurine,
    tts_object::{self, ModelForm, ModelProfile},
    tts_profile::{TtsProfileDetail, TtsProfileSummary},
};

pub fn tts_list_routes() -> Router<AppState> {
    Router::new()
        .route("/api/tts-lists/expand", post(expand_list))
        .route("/api/tts-lists/bag", post(build_bag))
}

#[derive(Debug, Deserialize)]
struct ExpandBody {
    code: String,
    #[serde(default)]
    painter: Option<String>,
}

#[derive(Debug, Serialize)]
struct ExpandResponse {
    code: String,
    army_id: u32,
    sectoral_slug: String,
    sectoral_name: String,
    name: String,
    max_points: u32,
    default_color: String,
    entries: Vec<ExpandEntry>,
}

#[derive(Debug, Serialize)]
struct ExpandEntry {
    index: usize,
    unit_id: u32,
    group_id: u32,
    option_id: u32,
    combat_group: u32,
    reinforcements: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    customization_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    profile: Option<TtsProfileSummary>,
    figurines: Vec<RankedFigurine>,
}

#[derive(Debug, Serialize)]
struct RankedFigurine {
    #[serde(flatten)]
    figurine: TtsFigurine,
    score: i32,
    exact_loadout: bool,
}

#[derive(Debug, Deserialize)]
struct BagBody {
    bag_name: String,
    #[serde(default)]
    color_g1: Option<String>,
    #[serde(default)]
    color_g2: Option<String>,
    army_id: i64,
    picks: Vec<BagPick>,
}

#[derive(Debug, Deserialize)]
struct BagPick {
    figurine_id: i64,
    profile_id: i64,
    combat_group: u32,
}

async fn expand_list(
    State(state): State<AppState>,
    Json(body): Json<ExpandBody>,
) -> Result<Json<ExpandResponse>, ApiError> {
    let decoded =
        parse_army_list(&body.code).map_err(|error| ApiError::bad_request(error.to_string()))?;
    let army = state
        .armies
        .get(decoded.sectoral_id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .ok_or_else(|| {
            ApiError::bad_request(format!(
                "sectorielle introuvable (id {})",
                decoded.sectoral_id
            ))
        })?;

    let preferred_painter = body
        .painter
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());

    let all_figurines = state
        .tts_figurines
        .list()
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let sectorial_iscs: HashSet<String> = state
        .tts_profiles
        .figurine_labels_for_army(i64::from(decoded.sectoral_id))
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .into_iter()
        .map(|label| label.isc.trim().to_ascii_lowercase())
        .collect();

    let mut entries = Vec::with_capacity(decoded.members.len());
    for (index, member) in decoded.members.into_iter().enumerate() {
        entries.push(expand_member(
            &state,
            index,
            &member,
            decoded.sectoral_id,
            &all_figurines,
            &sectorial_iscs,
            preferred_painter.as_deref(),
        )?);
    }

    let default_color = tts_object::sectorial_color_hex(&army.slug)
        .unwrap_or_else(|| "FFFFFF".to_string());

    Ok(Json(ExpandResponse {
        code: army_list::normalize_army_list_code(&body.code).unwrap_or_default(),
        army_id: decoded.sectoral_id,
        sectoral_slug: decoded.sectoral_slug,
        sectoral_name: army.name,
        name: decoded.name,
        max_points: decoded.max_points,
        default_color,
        entries,
    }))
}

async fn build_bag(
    State(state): State<AppState>,
    Json(body): Json<BagBody>,
) -> Result<Json<Value>, ApiError> {
    if body.picks.is_empty() {
        return Err(ApiError::bad_request("aucune figurine sélectionnée"));
    }
    let army = load_army(&state, body.army_id)?;
    let color_g1 = parse_pick_color(
        body.color_g1.as_deref(),
        &tts_object::sectorial_color_hex(&army.slug).unwrap_or_else(|| "FFFFFF".to_string()),
    )?;
    let color_g2 = parse_pick_color(body.color_g2.as_deref(), "FFFFFF")?;
    let meshes = state
        .tts_profiles
        .marker_mesh_catalog()
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let mut models = Vec::with_capacity(body.picks.len());
    for pick in &body.picks {
        let figurine = state
            .tts_figurines
            .get(pick.figurine_id)
            .map_err(|error| ApiError::bad_request(error.to_string()))?
            .ok_or_else(|| ApiError::bad_request("figurine introuvable"))?;
        if figurine.asset_bundle_url.trim().is_empty() {
            return Err(ApiError::bad_request(format!(
                "figurine « {} » sans asset bundle",
                figurine.isc
            )));
        }
        let profile = state
            .tts_profiles
            .get(pick.profile_id)
            .map_err(|error| ApiError::bad_request(error.to_string()))?
            .ok_or_else(|| ApiError::bad_request("profil introuvable"))?;
        let in_army = state
            .tts_profiles
            .profile_in_army(pick.profile_id, body.army_id)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        if !in_army {
            return Err(ApiError::bad_request(format!(
                "le profil « {} » n'est pas dans cette sectorielle",
                profile.isc
            )));
        }
        let color = if pick.combat_group <= 1 {
            color_g1
        } else {
            color_g2
        };
        models.push(tts_object::saved_model(
            &figurine.asset_bundle_url,
            &model_from_detail(&profile),
            &meshes,
            color,
        ));
    }

    Ok(Json(tts_object::save_bag(&body.bag_name, models)))
}

fn expand_member(
    state: &AppState,
    index: usize,
    member: &ArmyListMember,
    army_id: u32,
    all_figurines: &[TtsFigurine],
    sectorial_iscs: &HashSet<String>,
    preferred_painter: Option<&str>,
) -> Result<ExpandEntry, ApiError> {
    let profile = state
        .tts_profiles
        .find_by_army_source(
            member.unit_id,
            member.group_id,
            member.option_id,
            Some(i64::from(army_id)),
        )
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let figurines = match profile.as_ref() {
        Some(profile) => rank_figurines(
            all_figurines,
            &profile.isc,
            &profile.loadout,
            sectorial_iscs,
            preferred_painter,
        ),
        None => Vec::new(),
    };

    Ok(ExpandEntry {
        index,
        unit_id: member.unit_id,
        group_id: member.group_id,
        option_id: member.option_id,
        combat_group: member.combat_group,
        reinforcements: member.reinforcements,
        customization_json: member.customization_json.clone(),
        profile,
        figurines,
    })
}

fn rank_figurines(
    all: &[TtsFigurine],
    isc: &str,
    loadout: &str,
    sectorial_iscs: &HashSet<String>,
    preferred_painter: Option<&str>,
) -> Vec<RankedFigurine> {
    let isc_key = isc.trim().to_ascii_lowercase();
    let loadout_key = loadout.trim().to_ascii_lowercase();
    let in_sectorial = sectorial_iscs.contains(&isc_key);

    let mut ranked: Vec<RankedFigurine> = all
        .iter()
        .filter(|figurine| figurine.isc.trim().eq_ignore_ascii_case(isc))
        .map(|figurine| {
            let exact_loadout =
                !loadout_key.is_empty() && figurine.loadout.trim().eq_ignore_ascii_case(loadout);
            let painter_match = preferred_painter
                .is_some_and(|painter| figurine.painter.trim().eq_ignore_ascii_case(painter));
            let mut score = 0;
            if exact_loadout {
                score += 100;
            }
            if in_sectorial {
                score += 20;
            }
            if painter_match {
                score += 10;
            }
            if !figurine.asset_bundle_url.trim().is_empty() {
                score += 1;
            }
            RankedFigurine {
                figurine: figurine.clone(),
                score,
                exact_loadout,
            }
        })
        .collect();

    ranked.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| {
                left.figurine
                    .painter
                    .to_ascii_lowercase()
                    .cmp(&right.figurine.painter.to_ascii_lowercase())
            })
            .then_with(|| left.figurine.id.cmp(&right.figurine.id))
    });
    ranked
}

fn model_from_detail(profile: &TtsProfileDetail) -> ModelProfile {
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

fn parse_pick_color(
    raw: Option<&str>,
    fallback: &str,
) -> Result<tts_object::DiffuseColor, ApiError> {
    let value = raw.map(str::trim).filter(|value| !value.is_empty()).unwrap_or(fallback);
    tts_object::parse_color(value).ok_or_else(|| ApiError::bad_request("couleur invalide"))
}

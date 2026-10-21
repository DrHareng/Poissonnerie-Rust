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
    tts_profile::{TtsProfileDetail, TtsProfileForm, TtsProfileSummary},
};

/// Formes périphériques (Auxbot, Crabbot, pilote…) stockées à cet index.
const PERIPHERAL_STATE_INDEX: i64 = 900;

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
    /// Présent pour un périphérique (Auxbot, Crabbot…) : id de la forme state 900.
    #[serde(skip_serializing_if = "Option::is_none")]
    form_id: Option<i64>,
    /// Slot Holomask : choisir un profil de couverture (même sectorielle / silhouette).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    holomask: bool,
    /// Taille de silhouette imposée (celle du porteur).
    #[serde(skip_serializing_if = "Option::is_none")]
    holomask_silhouette_size: Option<String>,
    /// ISC du porteur (affichage UI).
    #[serde(skip_serializing_if = "Option::is_none")]
    holomask_source_isc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    profile: Option<TtsProfileSummary>,
    figurines: Vec<RankedFigurine>,
}

#[derive(Debug, Clone, Serialize)]
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
    #[serde(default)]
    form_id: Option<i64>,
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
    let mut index = 0usize;
    for member in decoded.members {
        let expanded = expand_member(
            &state,
            &member,
            decoded.sectoral_id,
            &all_figurines,
            &sectorial_iscs,
            preferred_painter.as_deref(),
        )?;
        for mut entry in expanded {
            entry.index = index;
            index += 1;
            entries.push(entry);
        }
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
        let in_army = state
            .tts_profiles
            .profile_in_army(pick.profile_id, body.army_id)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        if !in_army {
            return Err(ApiError::bad_request(
                "ce profil n'est pas dans cette sectorielle",
            ));
        }
        let color = if pick.combat_group <= 1 {
            color_g1
        } else {
            color_g2
        };
        let (model, description, deployable_traits) = if let Some(form_id) = pick.form_id {
            let (form, parent_id) = state
                .tts_profiles
                .get_peripheral_form(form_id)
                .map_err(|error| ApiError::bad_request(error.to_string()))?
                .ok_or_else(|| ApiError::bad_request("périphérique introuvable"))?;
            if parent_id != pick.profile_id {
                return Err(ApiError::bad_request(
                    "périphérique incohérent avec le profil parent",
                ));
            }
            let description = form.description_tts.clone();
            (
                tts_object::saved_model(
                    &figurine.asset_bundle_url,
                    &model_from_form(&form),
                    color,
                ),
                description,
                // Périphérique : pas de déployables (évite le doublon avec le profil parent).
                None,
            )
        } else {
            let profile = state
                .tts_profiles
                .get(pick.profile_id)
                .map_err(|error| ApiError::bad_request(error.to_string()))?
                .ok_or_else(|| ApiError::bad_request("profil introuvable"))?;
            let description = profile.description_tts.clone();
            let traits = profile.traits.clone();
            (
                tts_object::saved_model(
                    &figurine.asset_bundle_url,
                    &model_from_detail(&profile),
                    color,
                ),
                description,
                Some(traits),
            )
        };
        models.extend(tts_object::expand_echo_models(model, &description));
        if let Some(traits) = deployable_traits {
            models.extend(crate::tts_deployable::tinted_deployables_for_traits(
                &traits, color,
            ));
        }
    }

    Ok(Json(tts_object::save_bag(&body.bag_name, models)))
}

fn expand_member(
    state: &AppState,
    member: &ArmyListMember,
    army_id: u32,
    all_figurines: &[TtsFigurine],
    sectorial_iscs: &HashSet<String>,
    preferred_painter: Option<&str>,
) -> Result<Vec<ExpandEntry>, ApiError> {
    let slots = resolve_profile_slots(state, member, army_id)?;
    let mut entries = Vec::new();
    for (slot_index, slot) in slots.iter().enumerate() {
        let customization = if slot_index == 0 {
            member.customization_json.clone()
        } else {
            None
        };
        entries.extend(expand_profile_slot(
            state,
            member,
            slot.group_id,
            slot.option_id,
            army_id,
            customization,
            all_figurines,
            sectorial_iscs,
            preferred_painter,
        )?);
    }
    Ok(entries)
}

#[derive(Debug, Clone, Copy)]
struct ProfileSlot {
    group_id: u32,
    option_id: u32,
}

/// `group_id = 0` = option d'unité (Mirage-5, Polaris…) → un slot par `includes`.
fn resolve_profile_slots(
    state: &AppState,
    member: &ArmyListMember,
    army_id: u32,
) -> Result<Vec<ProfileSlot>, ApiError> {
    if member.group_id != 0 {
        return Ok(vec![ProfileSlot {
            group_id: member.group_id,
            option_id: member.option_id,
        }]);
    }

    let mut includes = state
        .tts_profiles
        .unit_option_includes(member.unit_id, member.option_id)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;

    if includes.is_empty() {
        // Cache local absent (import antérieur) : résoudre via l'API Army.
        match crate::tts_profile_army::fetch_unit_option_includes(
            army_id,
            member.unit_id,
            member.option_id,
        ) {
            Ok(fetched) if !fetched.is_empty() => {
                let _ = state.tts_profiles.replace_unit_option_includes(&fetched);
                includes = fetched;
            }
            Ok(_) => {}
            Err(error) => {
                eprintln!(
                    "includes unit {} opt {} (army {}) : {error}",
                    member.unit_id, member.option_id, army_id
                );
            }
        }
    }

    if includes.is_empty() {
        return Ok(vec![ProfileSlot {
            group_id: member.group_id,
            option_id: member.option_id,
        }]);
    }

    let mut slots = Vec::new();
    for include in includes {
        let group_id = u32::try_from(include.group_id).unwrap_or(0);
        let option_id = u32::try_from(include.option_id).unwrap_or(0);
        let quantity = usize::try_from(include.quantity.max(1)).unwrap_or(1);
        for _ in 0..quantity {
            slots.push(ProfileSlot {
                group_id,
                option_id,
            });
        }
    }
    Ok(slots)
}

fn expand_profile_slot(
    state: &AppState,
    member: &ArmyListMember,
    group_id: u32,
    option_id: u32,
    army_id: u32,
    customization_json: Option<String>,
    all_figurines: &[TtsFigurine],
    sectorial_iscs: &HashSet<String>,
    preferred_painter: Option<&str>,
) -> Result<Vec<ExpandEntry>, ApiError> {
    let profile = state
        .tts_profiles
        .find_by_army_source(
            member.unit_id,
            group_id,
            option_id,
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

    let mut entries = vec![ExpandEntry {
        index: 0,
        unit_id: member.unit_id,
        group_id,
        option_id,
        combat_group: member.combat_group,
        reinforcements: member.reinforcements,
        customization_json,
        form_id: None,
        holomask: false,
        holomask_silhouette_size: None,
        holomask_source_isc: None,
        profile: profile.clone(),
        figurines,
    }];

    if let Some(summary) = profile.as_ref() {
        let detail = state
            .tts_profiles
            .get(summary.id)
            .map_err(|error| ApiError::bad_request(error.to_string()))?;
        if let Some(detail) = detail {
            for form in detail
                .forms
                .iter()
                .filter(|form| form.state_index == PERIPHERAL_STATE_INDEX)
            {
                let peri_summary = summary_from_peripheral(summary, form);
                let peri_figurines = rank_figurines(
                    all_figurines,
                    &form.isc,
                    &form.loadout,
                    sectorial_iscs,
                    preferred_painter,
                );
                let quantity = peripheral_quantity(&detail, form);
                for _ in 0..quantity {
                    entries.push(ExpandEntry {
                        index: 0,
                        unit_id: member.unit_id,
                        group_id,
                        option_id,
                        combat_group: member.combat_group,
                        reinforcements: member.reinforcements,
                        customization_json: None,
                        form_id: Some(form.id),
                        holomask: false,
                        holomask_silhouette_size: None,
                        holomask_source_isc: None,
                        profile: Some(peri_summary.clone()),
                        figurines: peri_figurines.clone(),
                    });
                }
            }
            if has_holomask(&detail) {
                entries.push(ExpandEntry {
                    index: 0,
                    unit_id: member.unit_id,
                    group_id,
                    option_id,
                    combat_group: member.combat_group,
                    reinforcements: member.reinforcements,
                    customization_json: None,
                    form_id: None,
                    holomask: true,
                    holomask_silhouette_size: summary
                        .silhouette_size
                        .clone()
                        .or_else(|| detail.silhouette_size.clone()),
                    holomask_source_isc: Some(summary.isc.clone()),
                    profile: None,
                    figurines: Vec::new(),
                });
            }
        }
    }

    Ok(entries)
}

fn has_holomask(detail: &TtsProfileDetail) -> bool {
    if detail.traits.iter().any(|trait_| {
        trait_.kind.eq_ignore_ascii_case("equipment")
            && trait_.name.trim().eq_ignore_ascii_case("holomask")
    }) {
        return true;
    }
    tts_object::description_plain(&detail.description_tts).contains("holomask")
}

/// `ANTIPODE (x3)` / trait `detail: x3` → 3 entrées dans la liste.
fn peripheral_quantity(detail: &TtsProfileDetail, form: &TtsProfileForm) -> usize {
    let form_isc = form.isc.trim();
    for trait_ in detail.traits.iter().chain(form.traits.iter()) {
        if !trait_.kind.eq_ignore_ascii_case("equipment") {
            continue;
        }
        if !trait_.name.trim().eq_ignore_ascii_case(form_isc) {
            continue;
        }
        return parse_x_quantity(trait_.detail.as_deref()).max(1);
    }
    quantity_from_labeled_text(&detail.loadout, form_isc)
        .or_else(|| quantity_from_labeled_text(&detail.description_tts, form_isc))
        .or_else(|| quantity_from_labeled_text(&form.loadout, form_isc))
        .or_else(|| quantity_from_labeled_text(&form.description_tts, form_isc))
        .unwrap_or(1)
        .max(1)
}

fn parse_x_quantity(raw: Option<&str>) -> usize {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return 1;
    };
    let digits = raw
        .trim_start_matches(['x', 'X', '×'])
        .trim()
        .chars()
        .take_while(|char| char.is_ascii_digit())
        .collect::<String>();
    digits.parse::<usize>().unwrap_or(1).max(1)
}

fn quantity_from_labeled_text(text: &str, name: &str) -> Option<usize> {
    let needle = name.trim();
    if needle.is_empty() {
        return None;
    }
    let lower = text.to_ascii_lowercase();
    let name_lower = needle.to_ascii_lowercase();
    let mut rest = lower.as_str();
    while let Some(pos) = rest.find(&name_lower) {
        let after = rest[pos + name_lower.len()..].trim_start();
        // « NAME x3 », « NAME(x3) », « NAME (x3) »
        let after = after.strip_prefix('(').unwrap_or(after).trim_start();
        if after.starts_with('x')
            || after.starts_with('X')
            || after.starts_with('×')
            || after.chars().next().is_some_and(|char| char.is_ascii_digit())
        {
            return Some(parse_x_quantity(Some(after)));
        }
        rest = &rest[pos + name_lower.len()..];
    }
    None
}

fn summary_from_peripheral(
    parent: &TtsProfileSummary,
    form: &TtsProfileForm,
) -> TtsProfileSummary {
    TtsProfileSummary {
        id: parent.id,
        army_id: parent.army_id,
        faction_label: parent.faction_label.clone(),
        isc: form.isc.clone(),
        loadout: form.loadout.clone(),
        nom_tts: form.nom_tts.clone(),
        silhouette_size: form.silhouette_size.clone(),
        silhouette_type: form.silhouette_type.clone(),
        marker_name: form.marker_name.clone(),
        form_count: 0,
        linked: parent.linked,
        differences: String::new(),
    }
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
            // Les périphériques sont exportés comme figurines séparées du sac.
            .filter(|form| form.state_index != PERIPHERAL_STATE_INDEX)
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

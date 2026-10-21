//! Profils TTS lus dans la sauvegarde « Infinity the Game - Models ».

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::Path;
use std::sync::Mutex;

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::migrate::migrate;

const FACTIONS: &[(&str, &str)] = &[
    ("panoceania", "panoceania"),
    ("yu jing", "yu-jing"),
    ("aleph", "aleph"),
    ("ariadna", "ariadna"),
    ("combined army", "combined-army"),
    ("dahshat company", "dahshat"),
    ("druze bayram security", "druze"),
    ("haqqislam", "haqqislam"),
    ("ikari company", "ikari"),
    ("jsa", "jsa"),
    ("nomads", "nomads"),
    ("o-12", "o-12"),
    (
        "starco free company of the star",
        "starco-free-company-of-the-star",
    ),
    ("tohaa", "tohaa"),
    ("white company", "white-company"),
    ("contracted back-up", "contracted-back-up"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedForm {
    pub state_index: i64,
    pub isc: String,
    pub loadout: String,
    pub nom_tts: String,
    pub description_tts: String,
    pub silhouette_size: Option<String>,
    pub silhouette_type: Option<String>,
    pub marker_name: Option<String>,
    pub mesh_url: Option<String>,
    pub marker_mesh_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedProfile {
    pub faction_slug: Option<String>,
    pub faction_label: String,
    pub isc: String,
    pub loadout: String,
    pub nom_tts: String,
    pub description_tts: String,
    pub silhouette_size: Option<String>,
    pub silhouette_type: Option<String>,
    pub marker_name: Option<String>,
    pub mesh_url: Option<String>,
    pub marker_mesh_url: Option<String>,
    pub source_bag: String,
    pub source_index: i64,
    pub forms: Vec<ParsedForm>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportReport {
    pub profiles: usize,
    pub forms: usize,
    pub bags: usize,
    pub without_army: usize,
}

#[derive(Debug, Deserialize)]
struct SaveFile {
    #[serde(rename = "ObjectStates", default)]
    object_states: Vec<TtsObject>,
}

#[derive(Debug, Deserialize)]
struct TtsObject {
    #[serde(rename = "Nickname", default)]
    nickname: String,
    #[serde(rename = "Description", default)]
    description: String,
    #[serde(rename = "ContainedObjects", default)]
    contained_objects: Vec<TtsObject>,
    #[serde(rename = "States", default)]
    states: BTreeMap<String, TtsObject>,
    #[serde(rename = "CustomMesh")]
    custom_mesh: Option<TtsMesh>,
}

#[derive(Debug, Deserialize)]
struct TtsMesh {
    #[serde(rename = "MeshURL", default)]
    mesh_url: String,
}

enum StateKind {
    Marker { size: Option<String>, name: String },
    Form,
}

struct OpenForm {
    state_index: i64,
    isc: String,
    loadout: String,
    nom_tts: String,
    description_tts: String,
    silhouette_size: Option<String>,
    silhouette_type: Option<String>,
    marker_name: Option<String>,
    mesh_url: Option<String>,
    marker_mesh_url: Option<String>,
    marker_set: bool,
}

pub fn strip_tts_markup(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find('[') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        if let Some(end) = after.find(']') {
            rest = &after[end + 1..];
        } else {
            out.push('[');
            rest = after;
            break;
        }
    }
    out.push_str(rest);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn faction_slug_from_bag(nickname: &str) -> Option<&'static str> {
    let label = bag_label(nickname);
    FACTIONS
        .iter()
        .find(|(name, _)| *name == label)
        .map(|(_, slug)| *slug)
}

fn bag_label(nickname: &str) -> String {
    let plain = strip_tts_markup(nickname).to_ascii_lowercase();
    let plain = plain.trim();
    plain
        .trim_end_matches(" n5")
        .trim_end_matches(" n4")
        .trim()
        .to_string()
}

fn is_imported_bag(nickname: &str) -> bool {
    let label = bag_label(nickname);
    faction_slug_from_bag(nickname).is_some() || label == "hvts" || label == "deployables"
}

pub fn split_nickname(nickname: &str) -> (String, String) {
    let nickname = nickname.trim();
    if let Some(rest) = nickname.strip_prefix('[') {
        if let Some((color, after_color)) = rest.split_once(']') {
            if color.len() == 6 && color.chars().all(|c| c.is_ascii_hexdigit()) {
                if let Some((name, loadout)) = after_color.split_once("[-]") {
                    let isc = normalize_isc(name);
                    let loadout = strip_tts_markup(loadout);
                    return (isc, loadout);
                }
            }
        }
    }
    (normalize_isc(nickname), String::new())
}

fn normalize_isc(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() <= 255 {
        return trimmed.to_string();
    }
    trimmed.chars().take(255).collect()
}

fn description_tokens(description: &str) -> Vec<String> {
    let mut plain = strip_tts_markup(description);
    // Les titres « Skills --------- » n'ont pas de puce devant la première compétence.
    let mut separated = String::with_capacity(plain.len());
    let chars: Vec<char> = plain.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '-' {
            let start = index;
            while index < chars.len() && chars[index] == '-' {
                index += 1;
            }
            if index - start >= 2 {
                separated.push_str(" ● ");
            } else {
                separated.push('-');
            }
        } else {
            separated.push(chars[index]);
            index += 1;
        }
    }
    plain = separated;
    plain
        .split('●')
        .map(|part| {
            part.chars()
                .filter(|c| !c.is_whitespace())
                .flat_map(|c| c.to_lowercase())
                .collect::<String>()
        })
        .filter(|token| !token.is_empty())
        .collect()
}

/// Une seule valeur, dans cet ordre : camo-0, camo-3, camo-6, imp-1, imp-2, normale.
pub fn silhouette_kind(description: &str) -> &'static str {
    let tokens = description_tokens(description);
    let camo = tokens.iter().any(|token| token.starts_with("camouflage"));
    let mimetism = tokens.iter().any(|token| token.starts_with("mimetism"));
    let mimetism_3 = tokens.iter().any(|token| token.starts_with("mimetism(-3)"));
    let mimetism_6 = tokens.iter().any(|token| token.starts_with("mimetism(-6)"));
    let impersonation = tokens.iter().any(|token| {
        *token == "impersonation"
            || (token.starts_with("impersonation(")
                && !token.contains("imp-2")
                && !token.contains("l2"))
    });
    let impersonation_l2 = tokens.iter().any(|token| {
        token.starts_with("impersonation") && (token.contains("imp-2") || token.contains("l2"))
    });
    let killer_hacking = tokens
        .iter()
        .any(|token| token.contains("killerhackingdevice"));
    let hacking_plus = tokens
        .iter()
        .any(|token| token.contains("hackingdeviceplus"));
    let cybermask = tokens.iter().any(|token| token.contains("cybermask"));

    if camo && !mimetism {
        return "camo-0";
    }
    if camo && mimetism_3 {
        return "camo-3";
    }
    if camo && mimetism_6 {
        return "camo-6";
    }
    if impersonation {
        return "imp-1";
    }
    if impersonation_l2 || killer_hacking || hacking_plus || cybermask {
        return "imp-2";
    }
    "normale"
}

fn has_sapper(description: &str) -> bool {
    description_tokens(description)
        .iter()
        .any(|token| token == "sapper" || token.starts_with("sapper("))
}

fn sapper_silhouette_size(base: &Option<String>) -> String {
    let size = base
        .as_deref()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0);
    size.max(4).to_string()
}

fn sapper_description(description: &str) -> String {
    let mut result = description.to_string();
    for tag in ["[B]S[/B]:", "[b]S[/b]:"] {
        let mut rebuilt = String::new();
        let mut rest = result.as_str();
        while let Some(pos) = rest.find(tag) {
            rebuilt.push_str(&rest[..pos]);
            let after = &rest[pos + tag.len()..];
            let space = after.len() - after.trim_start().len();
            let digits: String = after[space..]
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if digits == "1" || digits == "2" || digits == "3" {
                rebuilt.push_str(tag);
                rebuilt.push(' ');
                rebuilt.push('4');
                rest = &after[space + digits.len()..];
            } else {
                rebuilt.push_str(tag);
                rest = after;
            }
        }
        rebuilt.push_str(rest);
        result = rebuilt;
    }
    result
}

fn has_escape_system(description: &str) -> bool {
    description_tokens(description)
        .iter()
        .any(|token| token.starts_with("transmutation(escapesystem"))
}

fn fold_name(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ä' | 'ã' | 'Á' | 'À' | 'Â' | 'Ä' | 'Ã' => 'a',
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'Ó' | 'Ò' | 'Ô' | 'Ö' | 'Õ' => 'o',
            'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'u',
            'ñ' | 'Ñ' => 'n',
            'ç' | 'Ç' => 'c',
            other => other.to_ascii_lowercase(),
        })
        .collect()
}

fn name_words(value: &str) -> Vec<String> {
    fold_name(value)
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| word.len() >= 2)
        .map(str::to_string)
        .collect()
}

fn is_operator_isc(isc: &str) -> bool {
    name_words(isc).iter().any(|word| word == "operator")
}

fn operator_name_words(isc: &str) -> Vec<String> {
    name_words(isc)
        .into_iter()
        .filter(|word| word != "operator")
        .collect()
}

fn profile_words(
    profile: &ParsedProfile,
    drop_operator: bool,
) -> std::collections::BTreeSet<String> {
    let mut words = std::collections::BTreeSet::new();
    for word in name_words(&profile.isc) {
        if drop_operator && word == "operator" {
            continue;
        }
        if word != "specialist" && word != "multi" {
            words.insert(word);
        }
    }
    for word in name_words(&profile.loadout) {
        if word != "specialist" && word != "multi" {
            words.insert(word);
        }
    }
    words
}

fn word_overlap(left: &ParsedProfile, right: &ParsedProfile) -> usize {
    let left_words = profile_words(left, true);
    let right_words = profile_words(right, false);
    left_words.intersection(&right_words).count()
}

fn at_least_sapper_size(nom_tts: &str, size: Option<String>) -> Option<String> {
    if !nom_tts.to_ascii_lowercase().contains("(sapper)") {
        return size;
    }
    let parsed = size
        .as_deref()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0);
    if parsed < 4 {
        Some("4".to_string())
    } else {
        size
    }
}

fn silhouette_size_from_description(description: &str) -> Option<String> {
    for line in description.lines() {
        let plain = strip_tts_markup(line);
        let chars: Vec<char> = plain.chars().collect();
        for index in 0..chars.len().saturating_sub(1) {
            let is_s = chars[index].eq_ignore_ascii_case(&'s') && chars[index + 1] == ':';
            let preceded_by_letter = index > 0 && chars[index - 1].is_ascii_alphabetic();
            if !is_s || preceded_by_letter {
                continue;
            }
            let size: String = chars[index + 2..]
                .iter()
                .skip_while(|c| c.is_whitespace())
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if !size.is_empty() {
                return Some(size);
            }
        }
    }
    None
}

fn classify_state(nickname: &str) -> StateKind {
    let plain = strip_tts_markup(nickname);
    if let Some(size) = plain.strip_prefix("Silhouette") {
        let size = size.trim();
        if !size.is_empty() && size.chars().all(|c| c.is_ascii_digit()) {
            return StateKind::Marker {
                size: Some(size.to_string()),
                name: plain,
            };
        }
    }
    if plain.to_ascii_lowercase().starts_with("camouflage") {
        let size = plain
            .rsplit_once('S')
            .map(|(_, tail)| tail.trim())
            .filter(|tail| !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()))
            .map(str::to_string);
        return StateKind::Marker { size, name: plain };
    }
    if plain.to_ascii_uppercase().starts_with("IMP-") {
        return StateKind::Marker {
            size: None,
            name: plain,
        };
    }
    StateKind::Form
}

fn mesh_url(object: &TtsObject) -> Option<String> {
    object
        .custom_mesh
        .as_ref()
        .map(|mesh| mesh.mesh_url.trim().to_string())
        .filter(|url| !url.is_empty())
}

fn apply_marker(
    size: Option<String>,
    name: String,
    mesh: Option<String>,
    target_size: &mut Option<String>,
    target_name: &mut Option<String>,
    target_mesh: &mut Option<String>,
    already: &mut bool,
) {
    if *already {
        return;
    }
    if let Some(size) = size {
        *target_size = Some(size);
    }
    *target_name = Some(name);
    *target_mesh = mesh;
    *already = true;
}

fn parse_model(
    object: &TtsObject,
    bag: &str,
    faction_label: String,
    faction_slug: Option<String>,
    source_index: i64,
) -> Option<ParsedProfile> {
    let nom_tts = object.nickname.trim();
    if nom_tts.is_empty() {
        return None;
    }
    let (isc, loadout) = split_nickname(nom_tts);
    let mut silhouette_size = silhouette_size_from_description(&object.description);
    let silhouette_type = Some(silhouette_kind(&object.description).to_string());
    let mut marker_name = None;
    let mut marker_mesh_url = None;
    let mut marker_set = false;
    let mut forms: Vec<OpenForm> = Vec::new();
    let mut open_from = 0usize;

    let mut states: Vec<(i64, &TtsObject)> = object
        .states
        .iter()
        .filter_map(|(key, state)| key.parse::<i64>().ok().map(|index| (index, state)))
        .collect();
    states.sort_by_key(|(index, _)| *index);

    for (index, state) in states {
        let state_name = state.nickname.trim();
        if state_name.is_empty() || strip_tts_markup(state_name) == strip_tts_markup(nom_tts) {
            continue;
        }
        match classify_state(state_name) {
            StateKind::Marker { size, name } => {
                let mesh = mesh_url(state);
                if forms.is_empty() {
                    apply_marker(
                        size,
                        name,
                        mesh,
                        &mut silhouette_size,
                        &mut marker_name,
                        &mut marker_mesh_url,
                        &mut marker_set,
                    );
                } else {
                    for form in forms.iter_mut().skip(open_from) {
                        apply_marker(
                            size.clone(),
                            name.clone(),
                            mesh.clone(),
                            &mut form.silhouette_size,
                            &mut form.marker_name,
                            &mut form.marker_mesh_url,
                            &mut form.marker_set,
                        );
                    }
                    open_from = forms.len();
                }
            }
            StateKind::Form => {
                let (form_isc, form_loadout) = split_nickname(state_name);
                forms.push(OpenForm {
                    state_index: index,
                    isc: form_isc,
                    loadout: form_loadout,
                    nom_tts: state_name.to_string(),
                    description_tts: state.description.clone(),
                    silhouette_size: silhouette_size_from_description(&state.description),
                    silhouette_type: Some(silhouette_kind(&state.description).to_string()),
                    marker_name: None,
                    mesh_url: mesh_url(state),
                    marker_mesh_url: None,
                    marker_set: false,
                });
            }
        }
    }

    if has_sapper(&object.description)
        && !forms
            .iter()
            .any(|form| form.nom_tts.to_ascii_lowercase().contains("(sapper)"))
    {
        let size = sapper_silhouette_size(&silhouette_size);
        let description_tts = sapper_description(&object.description);
        forms.push(OpenForm {
            state_index: 1000,
            isc: isc.clone(),
            loadout: loadout.clone(),
            nom_tts: format!("{} (sapper)", nom_tts.trim()),
            description_tts,
            silhouette_size: Some(size.clone()),
            silhouette_type: Some(silhouette_kind(&object.description).to_string()),
            marker_name: Some(format!("Silhouette {size}")),
            mesh_url: mesh_url(object),
            marker_mesh_url: None,
            marker_set: true,
        });
    }

    Some(ParsedProfile {
        faction_slug,
        faction_label,
        isc,
        loadout,
        nom_tts: nom_tts.to_string(),
        description_tts: object.description.clone(),
        silhouette_size,
        silhouette_type,
        marker_name,
        mesh_url: mesh_url(object),
        marker_mesh_url,
        source_bag: bag.to_string(),
        source_index,
        forms: forms
            .into_iter()
            .map(|form| ParsedForm {
                state_index: form.state_index,
                isc: form.isc,
                loadout: form.loadout,
                nom_tts: form.nom_tts,
                description_tts: form.description_tts,
                silhouette_size: form.silhouette_size,
                silhouette_type: form.silhouette_type,
                marker_name: form.marker_name,
                mesh_url: form.mesh_url,
                marker_mesh_url: form.marker_mesh_url,
            })
            .collect(),
    })
}

fn dedup_profiles(profiles: Vec<ParsedProfile>) -> Vec<ParsedProfile> {
    let mut seen = std::collections::BTreeSet::new();
    let mut unique = Vec::new();
    for profile in profiles {
        let key = (
            profile.isc.clone(),
            profile.loadout.clone(),
            profile.nom_tts.clone(),
            profile.description_tts.clone(),
        );
        if seen.insert(key) {
            unique.push(profile);
        }
    }
    unique
}

fn pilot_form(operator: &ParsedProfile) -> ParsedForm {
    ParsedForm {
        state_index: 900,
        isc: operator.isc.clone(),
        loadout: operator.loadout.clone(),
        nom_tts: operator.nom_tts.clone(),
        description_tts: operator.description_tts.clone(),
        silhouette_size: operator.silhouette_size.clone(),
        silhouette_type: operator.silhouette_type.clone(),
        marker_name: operator.marker_name.clone(),
        mesh_url: operator.mesh_url.clone(),
        marker_mesh_url: operator.marker_mesh_url.clone(),
    }
}

fn attach_pilot(vehicle: &mut ParsedProfile, operator: &ParsedProfile) {
    let form = pilot_form(operator);
    let already = vehicle.forms.iter().any(|existing| {
        existing.isc == form.isc
            && existing.loadout == form.loadout
            && existing.nom_tts == form.nom_tts
            && existing.description_tts == form.description_tts
    });
    if !already {
        vehicle.forms.push(form);
        vehicle.forms.sort_by_key(|form| form.state_index);
    }
}

fn greedy_pairs(
    profiles: &[ParsedProfile],
    operators: &[usize],
    vehicles: &[usize],
) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    for operator in operators {
        for vehicle in vehicles {
            let score = word_overlap(&profiles[*operator], &profiles[*vehicle]);
            if score > 0 {
                pairs.push((score, *operator, *vehicle));
            }
        }
    }
    pairs.sort_by(|left, right| {
        right.0.cmp(&left.0).then(
            operator_name_words(&profiles[right.1].isc)
                .len()
                .cmp(&operator_name_words(&profiles[left.1].isc).len()),
        )
    });
    let mut used_operators = std::collections::BTreeSet::new();
    let mut used_vehicles = std::collections::BTreeSet::new();
    let mut chosen = Vec::new();
    for (_, operator, vehicle) in pairs {
        if used_operators.contains(&operator) || used_vehicles.contains(&vehicle) {
            continue;
        }
        used_operators.insert(operator);
        used_vehicles.insert(vehicle);
        chosen.push((operator, vehicle));
    }
    chosen
}

fn attach_escape_pilots(mut profiles: Vec<ParsedProfile>) -> Vec<ParsedProfile> {
    let vehicles: Vec<usize> = profiles
        .iter()
        .enumerate()
        .filter(|(_, profile)| has_escape_system(&profile.description_tts))
        .map(|(index, _)| index)
        .collect();
    let operators: Vec<usize> = profiles
        .iter()
        .enumerate()
        .filter(|(_, profile)| {
            !has_escape_system(&profile.description_tts) && is_operator_isc(&profile.isc)
        })
        .map(|(index, _)| index)
        .collect();
    if vehicles.is_empty() || operators.is_empty() {
        return profiles;
    }

    let mut consumed = vec![false; profiles.len()];
    let mut piloted = vec![false; profiles.len()];
    let mut attached_to: Vec<(String, String, usize)> = Vec::new();
    let specific: Vec<usize> = operators
        .iter()
        .copied()
        .filter(|index| !operator_name_words(&profiles[*index].isc).is_empty())
        .collect();
    let specific_vehicles: Vec<usize> = vehicles
        .iter()
        .copied()
        .filter(|vehicle| {
            specific.iter().any(|operator| {
                let keys = operator_name_words(&profiles[*operator].isc);
                keys.iter().all(|key| {
                    name_words(&profiles[*vehicle].isc)
                        .iter()
                        .any(|word| word == key)
                })
            })
        })
        .collect();
    let specific_pairs = greedy_pairs(&profiles, &specific, &specific_vehicles);
    let specific_forms: Vec<(usize, usize, ParsedForm)> = specific_pairs
        .iter()
        .map(|(operator, vehicle)| (*operator, *vehicle, pilot_form(&profiles[*operator])))
        .collect();
    for (operator, vehicle, form) in specific_forms {
        attached_to.push((
            profiles[operator].isc.to_lowercase(),
            profiles[operator].loadout.to_lowercase(),
            vehicle,
        ));
        profiles[vehicle].forms.push(form);
        profiles[vehicle].forms.sort_by_key(|form| form.state_index);
        consumed[operator] = true;
        piloted[vehicle] = true;
    }

    let bare: Vec<usize> = operators
        .iter()
        .copied()
        .filter(|index| !consumed[*index] && operator_name_words(&profiles[*index].isc).is_empty())
        .collect();
    let unmatched: Vec<usize> = vehicles
        .iter()
        .copied()
        .filter(|index| !piloted[*index])
        .collect();
    if bare.len() == 1 && !unmatched.is_empty() {
        let pilot = profiles[bare[0]].clone();
        for vehicle in unmatched {
            attached_to.push((
                pilot.isc.to_lowercase(),
                pilot.loadout.to_lowercase(),
                vehicle,
            ));
            attach_pilot(&mut profiles[vehicle], &pilot);
        }
        consumed[bare[0]] = true;
    } else if !bare.is_empty() && !unmatched.is_empty() {
        let pairs = greedy_pairs(&profiles, &bare, &unmatched);
        let forms: Vec<(usize, usize, ParsedForm)> = pairs
            .iter()
            .map(|(operator, vehicle)| (*operator, *vehicle, pilot_form(&profiles[*operator])))
            .collect();
        for (operator, vehicle, form) in forms {
            attached_to.push((
                profiles[operator].isc.to_lowercase(),
                profiles[operator].loadout.to_lowercase(),
                vehicle,
            ));
            profiles[vehicle].forms.push(form);
            profiles[vehicle].forms.sort_by_key(|form| form.state_index);
            consumed[operator] = true;
        }
    }

    let siblings: Vec<usize> = operators
        .iter()
        .copied()
        .filter(|index| !consumed[*index])
        .collect();
    for operator in siblings {
        let key = (
            profiles[operator].isc.to_lowercase(),
            profiles[operator].loadout.to_lowercase(),
        );
        let vehicles: Vec<usize> = attached_to
            .iter()
            .filter(|(isc, loadout, _)| *isc == key.0 && *loadout == key.1)
            .map(|(_, _, vehicle)| *vehicle)
            .collect();
        if vehicles.is_empty() {
            continue;
        }
        let pilot = profiles[operator].clone();
        for vehicle in vehicles {
            attach_pilot(&mut profiles[vehicle], &pilot);
        }
        consumed[operator] = true;
    }

    profiles
        .into_iter()
        .enumerate()
        .filter(|(index, _)| !consumed[*index])
        .map(|(_, profile)| profile)
        .collect()
}

pub fn parse_models_json(raw: &str) -> Result<Vec<ParsedProfile>> {
    let save: SaveFile = serde_json::from_str(raw).context("JSON TTS illisible")?;
    let mut profiles = Vec::new();
    for bag in &save.object_states {
        if !is_imported_bag(&bag.nickname) {
            continue;
        }
        let faction_label = {
            let plain = strip_tts_markup(&bag.nickname);
            plain
                .trim_end_matches(" N5")
                .trim_end_matches(" N4")
                .trim()
                .to_string()
        };
        let faction_slug = faction_slug_from_bag(&bag.nickname).map(str::to_string);
        let mut bag_profiles = Vec::new();
        for (index, model) in bag.contained_objects.iter().enumerate() {
            if let Some(profile) = parse_model(
                model,
                bag.nickname.trim(),
                faction_label.clone(),
                faction_slug.clone(),
                index as i64,
            ) {
                bag_profiles.push(profile);
            }
        }
        profiles.extend(attach_escape_pilots(dedup_profiles(bag_profiles)));
    }
    Ok(profiles)
}

pub fn import_models_file(conn: &Connection, path: &Path) -> Result<ImportReport> {
    migrate(conn)?;
    let raw = fs::read_to_string(path)
        .with_context(|| format!("impossible de lire {}", path.display()))?;
    let profiles = parse_models_json(&raw)?;
    replace_profiles(conn, &profiles)
}

fn replace_profiles(conn: &Connection, profiles: &[ParsedProfile]) -> Result<ImportReport> {
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM tts_profile_traits", [])?;
    tx.execute("DELETE FROM tts_profile_sectorials", [])?;
    tx.execute("DELETE FROM tts_profile_forms", [])?;
    tx.execute("DELETE FROM tts_profiles", [])?;

    let mut forms = 0usize;
    let mut without_army = 0usize;
    let mut bags = std::collections::BTreeSet::new();
    for profile in profiles {
        bags.insert(profile.source_bag.clone());
        let army_id = profile
            .faction_slug
            .as_deref()
            .and_then(|slug| army_id_for_slug(&tx, slug).transpose())
            .transpose()?;
        if army_id.is_none() {
            without_army += 1;
        }
        tx.execute(
            "
            INSERT INTO tts_profiles (
                army_id, faction_slug, faction_label, isc, loadout, nom_tts, description_tts,
                silhouette_size, silhouette_type, marker_name, mesh_url, marker_mesh_url,
                source_bag, source_index
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            ",
            params![
                army_id,
                profile.faction_slug,
                profile.faction_label,
                profile.isc,
                profile.loadout,
                profile.nom_tts,
                profile.description_tts,
                profile.silhouette_size,
                profile.silhouette_type,
                profile.marker_name,
                profile.mesh_url,
                profile.marker_mesh_url,
                profile.source_bag,
                profile.source_index,
            ],
        )?;
        let profile_id = tx.last_insert_rowid();
        for form in &profile.forms {
            tx.execute(
                "
                INSERT INTO tts_profile_forms (
                    profile_id, state_index, isc, loadout, nom_tts, description_tts,
                    silhouette_size, silhouette_type, marker_name, mesh_url, marker_mesh_url
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                ",
                params![
                    profile_id,
                    form.state_index,
                    form.isc,
                    form.loadout,
                    form.nom_tts,
                    form.description_tts,
                    form.silhouette_size,
                    form.silhouette_type,
                    form.marker_name,
                    form.mesh_url,
                    form.marker_mesh_url,
                ],
            )?;
            forms += 1;
        }
    }
    let stored = profiles.len();
    let bag_count = bags.len();
    tx.commit()?;
    Ok(ImportReport {
        profiles: stored,
        forms,
        bags: bag_count,
        without_army,
    })
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TtsProfileSummary {
    pub id: i64,
    pub army_id: Option<i64>,
    pub faction_label: String,
    pub isc: String,
    pub loadout: String,
    pub nom_tts: String,
    pub silhouette_size: Option<String>,
    pub silhouette_type: Option<String>,
    pub marker_name: Option<String>,
    pub form_count: i64,
    /// Vrai si le profil est lié à la sectorielle demandée, en plus du catalogue de faction.
    pub linked: bool,
    /// Jetons présents sur ce profil et absents d'au moins un autre du même ISC et loadout.
    pub differences: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TtsProfileForm {
    pub id: i64,
    pub state_index: i64,
    pub isc: String,
    pub loadout: String,
    pub nom_tts: String,
    pub description_tts: String,
    pub silhouette_size: Option<String>,
    pub silhouette_type: Option<String>,
    pub marker_name: Option<String>,
    pub mesh_url: Option<String>,
    pub marker_mesh_url: Option<String>,
    pub traits: Vec<TtsProfileTrait>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TtsProfileDetail {
    pub id: i64,
    pub army_id: Option<i64>,
    pub faction_slug: Option<String>,
    pub faction_label: String,
    pub isc: String,
    pub loadout: String,
    pub nom_tts: String,
    pub description_tts: String,
    pub silhouette_size: Option<String>,
    pub silhouette_type: Option<String>,
    pub marker_name: Option<String>,
    pub mesh_url: Option<String>,
    pub marker_mesh_url: Option<String>,
    pub forms: Vec<TtsProfileForm>,
    pub sectorial_ids: Vec<i64>,
    pub traits: Vec<TtsProfileTrait>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TtsProfileTrait {
    pub kind: String,
    pub name: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TtsProfileLabel {
    pub isc: String,
    pub loadout: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TtsProfileLoadout {
    pub profile_id: i64,
    /// Présent pour une forme (périphérique, transmute…) : id de la forme, pas du profil.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_id: Option<i64>,
    pub loadout: String,
    pub faction_label: String,
    pub marker_name: Option<String>,
    pub silhouette_size: Option<String>,
    pub silhouette_type: Option<String>,
    pub extra_forms: i64,
    pub differences: String,
}

/// ISC / loadout utilisables pour cataloguer une figurine dans une sectorielle,
/// y compris les périphériques (GO-BOT, STALDRON, …).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TtsFigurineLabel {
    pub isc: String,
    pub loadout: String,
    pub marker_name: Option<String>,
    pub silhouette_size: Option<String>,
    pub silhouette_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TtsProfileArmyCount {
    pub army_id: Option<i64>,
    pub name: Option<String>,
    pub slug: Option<String>,
    pub count: i64,
}

#[derive(Debug, Clone)]
pub struct ProfileListQuery {
    pub army_id: Option<i64>,
    /// Catalogue de la faction parente, en plus de la sectorielle demandée.
    pub also_army_id: Option<i64>,
    pub unassigned: bool,
}

#[derive(Debug, Clone)]
pub struct ProfileFormUpdate {
    pub id: i64,
    pub isc: String,
    pub loadout: String,
    pub nom_tts: String,
    pub description_tts: String,
    pub silhouette_size: Option<String>,
    pub marker_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProfileUpdate {
    pub army_id: Option<i64>,
    pub isc: String,
    pub loadout: String,
    pub nom_tts: String,
    pub description_tts: String,
    pub silhouette_size: Option<String>,
    pub marker_name: Option<String>,
    pub forms: Vec<ProfileFormUpdate>,
    pub sectorial_ids: Vec<i64>,
}

/// Sous-profil référencé par une option d'unité Army (`group_id = 0` dans les listes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitOptionInclude {
    pub unit_id: i64,
    pub unit_option_id: i64,
    pub sort_order: i64,
    pub group_id: i64,
    pub option_id: i64,
    pub quantity: i64,
}

pub struct TtsProfileStore {
    conn: Mutex<Connection>,
}

impl TtsProfileStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("impossible de créer {}", parent.display()))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("impossible d'ouvrir {}", path.display()))?;
        migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn list(&self, query: &ProfileListQuery) -> Result<Vec<TtsProfileSummary>> {
        let conn = self.conn.lock().unwrap();
        list_profiles(&conn, query)
    }

    pub fn army_counts(&self) -> Result<Vec<TtsProfileArmyCount>> {
        let conn = self.conn.lock().unwrap();
        army_counts(&conn)
    }

    pub fn loadouts_for_isc(&self, isc: &str, army_id: i64) -> Result<Vec<TtsProfileLoadout>> {
        let conn = self.conn.lock().unwrap();
        let loadouts = list_loadouts(&conn, isc, Some(army_id))?;
        if loadouts.is_empty() {
            // Périphériques (900) ou formes de transmute / sapper (ISC distinct).
            list_form_loadouts(&conn, isc, Some(army_id))
        } else {
            Ok(loadouts)
        }
    }

    pub fn figurine_labels_for_army(&self, army_id: i64) -> Result<Vec<TtsFigurineLabel>> {
        let conn = self.conn.lock().unwrap();
        figurine_labels_for_army(&conn, army_id)
    }

    /// Sectorielles (ids) indexées par ISC normalisé (trim + minuscules).
    pub fn sectorial_ids_by_isc(&self) -> Result<HashMap<String, Vec<i64>>> {
        let conn = self.conn.lock().unwrap();
        sectorial_ids_by_isc(&conn)
    }

    pub fn profile_in_army(&self, profile_id: i64, army_id: i64) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        profile_in_army(&conn, profile_id, army_id)
    }

    pub fn get_peripheral_form(&self, form_id: i64) -> Result<Option<(TtsProfileForm, i64)>> {
        let conn = self.conn.lock().unwrap();
        get_form(&conn, form_id, Some(900))
    }

    /// Forme de profil (transmutation, sapper, périphérique…).
    pub fn get_form(&self, form_id: i64) -> Result<Option<(TtsProfileForm, i64)>> {
        let conn = self.conn.lock().unwrap();
        get_form(&conn, form_id, None)
    }

    pub fn marker_mesh_catalog(&self) -> Result<BTreeMap<String, String>> {
        let conn = self.conn.lock().unwrap();
        marker_mesh_catalog(&conn)
    }

    pub fn list_labels(&self) -> Result<Vec<TtsProfileLabel>> {
        let conn = self.conn.lock().unwrap();
        let mut labels: BTreeMap<(String, String), TtsProfileLabel> = BTreeMap::new();
        let mut stmt = conn.prepare(
            "
            SELECT isc, loadout
            FROM tts_profiles
            ORDER BY isc COLLATE NOCASE, loadout COLLATE NOCASE, id
            ",
        )?;
        let profile_rows = stmt.query_map([], |row| {
            Ok(TtsProfileLabel {
                isc: row.get(0)?,
                loadout: row.get(1)?,
            })
        })?;
        for row in profile_rows {
            let label = row?;
            let key = (
                label.isc.trim().to_ascii_lowercase(),
                label.loadout.trim().to_ascii_lowercase(),
            );
            labels.entry(key).or_insert(label);
        }
        let mut stmt = conn.prepare(
            "
            SELECT isc, loadout
            FROM tts_profile_forms
            ORDER BY isc COLLATE NOCASE, loadout COLLATE NOCASE, id
            ",
        )?;
        let form_rows = stmt.query_map([], |row| {
            Ok(TtsProfileLabel {
                isc: row.get(0)?,
                loadout: row.get(1)?,
            })
        })?;
        for row in form_rows {
            let label = row?;
            let key = (
                label.isc.trim().to_ascii_lowercase(),
                label.loadout.trim().to_ascii_lowercase(),
            );
            labels.entry(key).or_insert(label);
        }
        let mut rows: Vec<_> = labels.into_values().collect();
        rows.sort_by(|left, right| {
            left.isc
                .to_ascii_lowercase()
                .cmp(&right.isc.to_ascii_lowercase())
                .then_with(|| {
                    left.loadout
                        .to_ascii_lowercase()
                        .cmp(&right.loadout.to_ascii_lowercase())
                })
        });
        Ok(rows)
    }

    pub fn get(&self, id: i64) -> Result<Option<TtsProfileDetail>> {
        let conn = self.conn.lock().unwrap();
        get_profile(&conn, id)
    }

    /// Résout un profil via `source_bag = army:{unit}:{group}` et `source_index = option`.
    /// Préfère un profil lié à `army_id` (sectorielle ou catalogue).
    pub fn find_by_army_source(
        &self,
        unit_id: u32,
        group_id: u32,
        option_id: u32,
        army_id: Option<i64>,
    ) -> Result<Option<TtsProfileSummary>> {
        let conn = self.conn.lock().unwrap();
        find_by_army_source(&conn, unit_id, group_id, option_id, army_id)
    }

    /// Sous-profils d'une option d'unité Army (`group_id = 0` dans les listes).
    pub fn unit_option_includes(
        &self,
        unit_id: u32,
        unit_option_id: u32,
    ) -> Result<Vec<UnitOptionInclude>> {
        let conn = self.conn.lock().unwrap();
        unit_option_includes(&conn, unit_id, unit_option_id)
    }

    /// Remplace le cache local des includes pour une option d'unité.
    pub fn replace_unit_option_includes(&self, includes: &[UnitOptionInclude]) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        replace_unit_option_includes(&conn, includes)
    }

    pub fn update(&self, id: i64, update: &ProfileUpdate) -> Result<TtsProfileDetail> {
        let conn = self.conn.lock().unwrap();
        update_profile(&conn, id, update)
    }
}

pub fn list_profiles(
    conn: &Connection,
    query: &ProfileListQuery,
) -> Result<Vec<TtsProfileSummary>> {
    let mut stmt = conn.prepare(
        "
        SELECT
            p.id, p.army_id, p.faction_label, p.isc, p.loadout, p.nom_tts,
            p.silhouette_size, p.silhouette_type, p.marker_name,
            (SELECT COUNT(*) FROM tts_profile_forms f WHERE f.profile_id = p.id),
            EXISTS (
                SELECT 1 FROM tts_profile_sectorials s
                WHERE s.profile_id = p.id AND s.army_id = ?1
            ),
            p.description_tts
        FROM tts_profiles p
        WHERE (
            ?3 = 1 AND p.army_id IS NULL
        ) OR (
            ?3 = 0 AND (
                p.army_id = ?1
                OR (?2 IS NOT NULL AND p.army_id = ?2)
                OR EXISTS (
                    SELECT 1 FROM tts_profile_sectorials s
                    WHERE s.profile_id = p.id AND s.army_id = ?1
                )
            )
        )
        ORDER BY p.isc COLLATE NOCASE, p.silhouette_size, p.silhouette_type,
            p.loadout COLLATE NOCASE, p.id
        ",
    )?;
    let rows = stmt.query_map(
        params![query.army_id, query.also_army_id, query.unassigned as i32,],
        |row| {
            let mut summary = map_summary(row)?;
            summary.differences.clear();
            let description: String = row.get(11)?;
            Ok((summary, description))
        },
    )?;
    let mut listed = rows
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("lecture des profils TTS")?;
    apply_description_differences(&mut listed);
    Ok(listed.into_iter().map(|(summary, _)| summary).collect())
}

fn description_pieces(description: &str) -> Vec<String> {
    strip_tts_markup(description)
        .split('●')
        .map(|piece| piece.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|piece| !piece.is_empty())
        .collect()
}

fn compact_piece(piece: &str) -> String {
    piece
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn apply_description_differences(rows: &mut [(TtsProfileSummary, String)]) {
    let mut groups: BTreeMap<(String, String), Vec<usize>> = BTreeMap::new();
    for (index, (summary, _)) in rows.iter().enumerate() {
        groups
            .entry((summary.isc.to_lowercase(), summary.loadout.to_lowercase()))
            .or_default()
            .push(index);
    }
    for indexes in groups.values() {
        if indexes.len() < 2 {
            continue;
        }
        let pieces: Vec<Vec<String>> = indexes
            .iter()
            .map(|index| description_pieces(&rows[*index].1))
            .collect();
        let mut distinct = std::collections::BTreeSet::new();
        for set in &pieces {
            distinct.insert(set.clone());
        }
        if distinct.len() < 2 {
            continue;
        }
        let mut frequency: BTreeMap<String, usize> = BTreeMap::new();
        for set in &pieces {
            let mut seen = std::collections::BTreeSet::new();
            for piece in set {
                seen.insert(compact_piece(piece));
            }
            for token in seen {
                *frequency.entry(token).or_default() += 1;
            }
        }
        let total = pieces.len();
        for (offset, index) in indexes.iter().enumerate() {
            let mut extras = Vec::new();
            let mut seen = std::collections::BTreeSet::new();
            for piece in &pieces[offset] {
                let token = compact_piece(piece);
                if !seen.insert(token.clone()) {
                    continue;
                }
                if frequency.get(&token).copied().unwrap_or(0) < total {
                    extras.push(piece.clone());
                }
            }
            rows[*index].0.differences = extras.join(" · ");
        }
    }
}

fn list_loadouts(
    conn: &Connection,
    isc: &str,
    army_id: Option<i64>,
) -> Result<Vec<TtsProfileLoadout>> {
    let isc = isc.trim();
    if isc.is_empty() {
        return Ok(Vec::new());
    }
    let mut stmt = conn.prepare(
        "
        SELECT
            p.id, p.loadout, p.faction_label, p.marker_name, p.silhouette_size,
            p.silhouette_type, p.description_tts,
            (
                SELECT COUNT(*)
                FROM tts_profile_forms f
                WHERE f.profile_id = p.id AND f.state_index != 900
            )
        FROM tts_profiles p
        WHERE p.isc = ?1 COLLATE NOCASE
          AND (
            ?2 IS NULL
            OR p.army_id = ?2
            OR EXISTS (
                SELECT 1 FROM tts_profile_sectorials s
                WHERE s.profile_id = p.id AND s.army_id = ?2
            )
          )
        ORDER BY p.loadout COLLATE NOCASE, p.faction_label COLLATE NOCASE, p.id
        ",
    )?;
    let rows = stmt
        .query_map(params![isc, army_id], |row| {
            Ok((
                TtsProfileLoadout {
                    profile_id: row.get(0)?,
                    form_id: None,
                    loadout: row.get(1)?,
                    faction_label: row.get(2)?,
                    marker_name: row.get(3)?,
                    silhouette_size: row.get(4)?,
                    silhouette_type: row.get(5)?,
                    extra_forms: row.get(7)?,
                    differences: String::new(),
                },
                row.get::<_, String>(6)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("lecture des loadouts")?;
    Ok(apply_loadout_differences(rows))
}

fn list_form_loadouts(
    conn: &Connection,
    isc: &str,
    army_id: Option<i64>,
) -> Result<Vec<TtsProfileLoadout>> {
    let isc = isc.trim();
    if isc.is_empty() {
        return Ok(Vec::new());
    }
    let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<_> {
        Ok((
            row.get::<_, String>(2)?,
            TtsProfileLoadout {
                profile_id: row.get(0)?,
                form_id: Some(row.get(1)?),
                loadout: row.get(3)?,
                faction_label: row.get(4)?,
                marker_name: row.get(5)?,
                silhouette_size: row.get(6)?,
                silhouette_type: row.get(7)?,
                extra_forms: 0,
                differences: String::new(),
            },
            row.get::<_, Option<String>>(8)?.unwrap_or_default(),
        ))
    };
    let rows = if let Some(army_id) = army_id {
        let mut stmt = conn.prepare(
            "
            SELECT
                p.id, f.id, f.isc, f.loadout, p.faction_label, f.marker_name, f.silhouette_size,
                f.silhouette_type, f.description_tts
            FROM tts_profile_forms f
            JOIN tts_profiles p ON p.id = f.profile_id
            WHERE (
                p.army_id = ?1
                OR EXISTS (
                    SELECT 1 FROM tts_profile_sectorials s
                    WHERE s.profile_id = p.id AND s.army_id = ?1
                )
              )
            ORDER BY f.loadout COLLATE NOCASE, p.faction_label COLLATE NOCASE, f.id
            ",
        )?;
        let mapped = stmt.query_map(params![army_id], map_row)?;
        mapped
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("lecture des loadouts de formes")?
    } else {
        let mut stmt = conn.prepare(
            "
            SELECT
                p.id, f.id, f.isc, f.loadout, p.faction_label, f.marker_name, f.silhouette_size,
                f.silhouette_type, f.description_tts
            FROM tts_profile_forms f
            JOIN tts_profiles p ON p.id = f.profile_id
            ORDER BY f.loadout COLLATE NOCASE, p.faction_label COLLATE NOCASE, f.id
            ",
        )?;
        let mapped = stmt.query_map([], map_row)?;
        mapped
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("lecture des loadouts de formes")?
    };
    let rows = rows
        .into_iter()
        .filter(|(form_isc, _, _)| form_isc.eq_ignore_ascii_case(isc))
        .map(|(_, loadout, description)| (loadout, description))
        .collect();
    Ok(apply_loadout_differences(rows))
}

fn apply_loadout_differences(mut rows: Vec<(TtsProfileLoadout, String)>) -> Vec<TtsProfileLoadout> {
    let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, (loadout, _)) in rows.iter().enumerate() {
        groups
            .entry(loadout.loadout.to_lowercase())
            .or_default()
            .push(index);
    }
    for indexes in groups.values() {
        let descriptions: Vec<String> =
            indexes.iter().map(|index| rows[*index].1.clone()).collect();
        let labels = differing_traits(&descriptions);
        for (offset, index) in indexes.iter().enumerate() {
            rows[*index].0.differences = labels[offset].clone();
        }
    }
    rows.into_iter().map(|(loadout, _)| loadout).collect()
}

fn figurine_labels_for_army(conn: &Connection, army_id: i64) -> Result<Vec<TtsFigurineLabel>> {
    let mut labels: BTreeMap<(String, String), TtsFigurineLabel> = BTreeMap::new();
    let mut stmt = conn.prepare(
        "
        SELECT p.isc, p.loadout, p.marker_name, p.silhouette_size, p.silhouette_type
        FROM tts_profiles p
        WHERE p.army_id = ?1
           OR EXISTS (
                SELECT 1 FROM tts_profile_sectorials s
                WHERE s.profile_id = p.id AND s.army_id = ?1
           )
        ORDER BY p.isc COLLATE NOCASE, p.loadout COLLATE NOCASE, p.id
        ",
    )?;
    let profile_rows = stmt.query_map(params![army_id], |row| {
        Ok(TtsFigurineLabel {
            isc: row.get(0)?,
            loadout: row.get(1)?,
            marker_name: row.get(2)?,
            silhouette_size: row.get(3)?,
            silhouette_type: row.get(4)?,
        })
    })?;
    for row in profile_rows {
        let label = row?;
        let key = (
            label.isc.trim().to_ascii_lowercase(),
            label.loadout.trim().to_ascii_lowercase(),
        );
        labels.entry(key).or_insert(label);
    }
    let mut stmt = conn.prepare(
        "
        SELECT f.isc, f.loadout, f.marker_name, f.silhouette_size, f.silhouette_type
        FROM tts_profile_forms f
        JOIN tts_profiles p ON p.id = f.profile_id
        WHERE (
            p.army_id = ?1
            OR EXISTS (
                SELECT 1 FROM tts_profile_sectorials s
                WHERE s.profile_id = p.id AND s.army_id = ?1
            )
          )
        ORDER BY f.isc COLLATE NOCASE, f.loadout COLLATE NOCASE, f.id
        ",
    )?;
    let form_rows = stmt.query_map(params![army_id], |row| {
        Ok(TtsFigurineLabel {
            isc: row.get(0)?,
            loadout: row.get(1)?,
            marker_name: row.get(2)?,
            silhouette_size: row.get(3)?,
            silhouette_type: row.get(4)?,
        })
    })?;
    for row in form_rows {
        let label = row?;
        let key = (
            label.isc.trim().to_ascii_lowercase(),
            label.loadout.trim().to_ascii_lowercase(),
        );
        labels.entry(key).or_insert(label);
    }
    let mut rows: Vec<_> = labels.into_values().collect();
    rows.sort_by(|left, right| {
        left.isc
            .to_ascii_lowercase()
            .cmp(&right.isc.to_ascii_lowercase())
            .then_with(|| {
                left.loadout
                    .to_ascii_lowercase()
                    .cmp(&right.loadout.to_ascii_lowercase())
            })
    });
    Ok(rows)
}

fn get_form(
    conn: &Connection,
    form_id: i64,
    only_state_index: Option<i64>,
) -> Result<Option<(TtsProfileForm, i64)>> {
    let mut stmt = conn.prepare(
        "
        SELECT
            f.id, f.state_index, f.isc, f.loadout, f.nom_tts, f.description_tts,
            f.silhouette_size, f.silhouette_type, f.marker_name, f.mesh_url,
            f.marker_mesh_url, f.profile_id
        FROM tts_profile_forms f
        WHERE f.id = ?1
          AND (?2 IS NULL OR f.state_index = ?2)
        ",
    )?;
    let mut rows = stmt.query(params![form_id, only_state_index])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };
    Ok(Some((
        TtsProfileForm {
            id: row.get(0)?,
            state_index: row.get(1)?,
            isc: row.get(2)?,
            loadout: row.get(3)?,
            nom_tts: row.get(4)?,
            description_tts: row.get(5)?,
            silhouette_size: row.get(6)?,
            silhouette_type: row.get(7)?,
            marker_name: row.get(8)?,
            mesh_url: row.get(9)?,
            marker_mesh_url: row.get(10)?,
            traits: Vec::new(),
        },
        row.get(11)?,
    )))
}

fn differing_traits(descriptions: &[String]) -> Vec<String> {
    if descriptions.len() < 2 {
        return vec![String::new(); descriptions.len()];
    }
    let pieces: Vec<Vec<String>> = descriptions
        .iter()
        .map(|text| description_pieces(text))
        .collect();
    let mut distinct = std::collections::BTreeSet::new();
    for set in &pieces {
        distinct.insert(set.clone());
    }
    if distinct.len() < 2 {
        return vec![String::new(); descriptions.len()];
    }
    let mut frequency: BTreeMap<String, usize> = BTreeMap::new();
    for set in &pieces {
        let mut seen = std::collections::BTreeSet::new();
        for piece in set {
            seen.insert(compact_piece(piece));
        }
        for token in seen {
            *frequency.entry(token).or_default() += 1;
        }
    }
    let total = pieces.len();
    pieces
        .iter()
        .map(|set| {
            let mut extras = Vec::new();
            let mut seen = std::collections::BTreeSet::new();
            for piece in set {
                let token = compact_piece(piece);
                if !seen.insert(token.clone()) {
                    continue;
                }
                if frequency.get(&token).copied().unwrap_or(0) < total {
                    extras.push(piece.clone());
                }
            }
            extras.join(" · ")
        })
        .collect()
}

fn profile_in_army(conn: &Connection, profile_id: i64, army_id: i64) -> Result<bool> {
    let found = conn
        .query_row(
            "
            SELECT 1
            FROM tts_profiles p
            WHERE p.id = ?1
              AND (
                p.army_id = ?2
                OR EXISTS (
                    SELECT 1 FROM tts_profile_sectorials s
                    WHERE s.profile_id = p.id AND s.army_id = ?2
                )
              )
            ",
            params![profile_id, army_id],
            |_| Ok(()),
        )
        .optional()?;
    Ok(found.is_some())
}

fn find_by_army_source(
    conn: &Connection,
    unit_id: u32,
    group_id: u32,
    option_id: u32,
    army_id: Option<i64>,
) -> Result<Option<TtsProfileSummary>> {
    let bag = format!("army:{unit_id}:{group_id}");
    let mut stmt = conn.prepare(
        "
        SELECT
            p.id, p.army_id, p.faction_label, p.isc, p.loadout, p.nom_tts,
            p.silhouette_size, p.silhouette_type, p.marker_name,
            (SELECT COUNT(*) FROM tts_profile_forms f WHERE f.profile_id = p.id),
            0,
            ''
        FROM tts_profiles p
        WHERE p.source_bag = ?1 AND p.source_index = ?2
        ORDER BY p.id
        ",
    )?;
    let rows = stmt
        .query_map(params![bag, i64::from(option_id)], map_summary)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("recherche de profil par source Army")?;
    if rows.is_empty() {
        return Ok(None);
    }
    let Some(army_id) = army_id else {
        return Ok(rows.into_iter().next());
    };
    let mut fallback = None;
    for mut row in rows {
        if profile_in_army(conn, row.id, army_id)? {
            row.linked = true;
            return Ok(Some(row));
        }
        if fallback.is_none() {
            fallback = Some(row);
        }
    }
    Ok(fallback)
}

fn unit_option_includes(
    conn: &Connection,
    unit_id: u32,
    unit_option_id: u32,
) -> Result<Vec<UnitOptionInclude>> {
    let mut stmt = conn.prepare(
        "
        SELECT unit_id, unit_option_id, sort_order,
               included_group_id, included_option_id, quantity
        FROM tts_army_unit_includes
        WHERE unit_id = ?1 AND unit_option_id = ?2
        ORDER BY sort_order
        ",
    )?;
    let rows = stmt
        .query_map(params![i64::from(unit_id), i64::from(unit_option_id)], |row| {
            Ok(UnitOptionInclude {
                unit_id: row.get(0)?,
                unit_option_id: row.get(1)?,
                sort_order: row.get(2)?,
                group_id: row.get(3)?,
                option_id: row.get(4)?,
                quantity: row.get(5)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("lecture des includes d'option d'unité")?;
    Ok(rows)
}

fn replace_unit_option_includes(conn: &Connection, includes: &[UnitOptionInclude]) -> Result<()> {
    if includes.is_empty() {
        return Ok(());
    }
    let unit_id = includes[0].unit_id;
    let unit_option_id = includes[0].unit_option_id;
    conn.execute(
        "DELETE FROM tts_army_unit_includes WHERE unit_id = ?1 AND unit_option_id = ?2",
        params![unit_id, unit_option_id],
    )?;
    for include in includes {
        conn.execute(
            "
            INSERT INTO tts_army_unit_includes (
                unit_id, unit_option_id, sort_order,
                included_group_id, included_option_id, quantity
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ",
            params![
                include.unit_id,
                include.unit_option_id,
                include.sort_order,
                include.group_id,
                include.option_id,
                include.quantity,
            ],
        )?;
    }
    Ok(())
}

fn marker_mesh_catalog(conn: &Connection) -> Result<BTreeMap<String, String>> {
    let mut stmt = conn.prepare(
        "
        SELECT marker_name, marker_mesh_url
        FROM tts_profiles
        WHERE marker_name IS NOT NULL AND marker_mesh_url IS NOT NULL AND marker_mesh_url != ''
        UNION ALL
        SELECT marker_name, marker_mesh_url
        FROM tts_profile_forms
        WHERE marker_name IS NOT NULL AND marker_mesh_url IS NOT NULL AND marker_mesh_url != ''
        ",
    )?;
    let pairs = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<std::result::Result<Vec<(String, String)>, _>>()
        .context("lecture des meshes de silhouettes")?;
    Ok(crate::tts_object::majority_meshes(pairs))
}

fn army_counts(conn: &Connection) -> Result<Vec<TtsProfileArmyCount>> {
    let mut stmt = conn.prepare(
        "
        SELECT p.army_id, a.name, a.slug, COUNT(*)
        FROM tts_profiles p
        LEFT JOIN armies a ON a.id = p.army_id
        GROUP BY p.army_id
        ORDER BY a.name COLLATE NOCASE, p.army_id
        ",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(TtsProfileArmyCount {
            army_id: row.get(0)?,
            name: row.get(1)?,
            slug: row.get(2)?,
            count: row.get(3)?,
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .context("comptage des profils par sectorielle")
}

fn get_profile(conn: &Connection, id: i64) -> Result<Option<TtsProfileDetail>> {
    let mut stmt = conn.prepare(
        "
        SELECT
            id, army_id, faction_slug, faction_label, isc, loadout, nom_tts, description_tts,
            silhouette_size, silhouette_type, marker_name, mesh_url, marker_mesh_url
        FROM tts_profiles
        WHERE id = ?1
        ",
    )?;
    let profile = stmt
        .query_row(params![id], map_detail_row)
        .optional()
        .context("lecture du profil TTS")?;
    let Some(mut profile) = profile else {
        return Ok(None);
    };
    profile.forms = list_forms(conn, id)?;
    profile.sectorial_ids = list_sectorial_ids(conn, id)?;
    profile.traits = list_traits(conn, id, None)?;
    for form in &mut profile.forms {
        form.traits = list_traits(conn, id, Some(form.id))?;
    }
    Ok(Some(profile))
}

fn list_traits(
    conn: &Connection,
    profile_id: i64,
    form_id: Option<i64>,
) -> Result<Vec<TtsProfileTrait>> {
    let mut stmt = conn.prepare(
        "
        SELECT kind, name, detail
        FROM tts_profile_traits
        WHERE profile_id = ?1 AND (
            (?2 IS NULL AND form_id IS NULL) OR form_id = ?2
        )
        ORDER BY sort_order, id
        ",
    )?;
    let rows = stmt.query_map(params![profile_id, form_id], |row| {
        Ok(TtsProfileTrait {
            kind: row.get(0)?,
            name: row.get(1)?,
            detail: row.get(2)?,
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .context("lecture des caractéristiques du profil")
}

fn sectorial_ids_by_isc(conn: &Connection) -> Result<HashMap<String, Vec<i64>>> {
    let mut by_isc: HashMap<String, BTreeSet<i64>> = HashMap::new();
    let mut push = |isc: String, army_id: i64| {
        let key = isc.trim().to_ascii_lowercase();
        if key.is_empty() {
            return;
        }
        by_isc.entry(key).or_default().insert(army_id);
    };
    let mut stmt = conn.prepare(
        "
        SELECT p.isc, s.army_id
        FROM tts_profiles p
        JOIN tts_profile_sectorials s ON s.profile_id = p.id
        ",
    )?;
    let profile_rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    for row in profile_rows {
        let (isc, army_id) = row?;
        push(isc, army_id);
    }
    let mut stmt = conn.prepare(
        "
        SELECT f.isc, s.army_id
        FROM tts_profile_forms f
        JOIN tts_profiles p ON p.id = f.profile_id
        JOIN tts_profile_sectorials s ON s.profile_id = p.id
        WHERE f.state_index = 900
        ",
    )?;
    let form_rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    for row in form_rows {
        let (isc, army_id) = row?;
        push(isc, army_id);
    }
    Ok(by_isc
        .into_iter()
        .map(|(isc, ids)| (isc, ids.into_iter().collect()))
        .collect())
}

fn list_sectorial_ids(conn: &Connection, profile_id: i64) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "
        SELECT army_id
        FROM tts_profile_sectorials
        WHERE profile_id = ?1
        ORDER BY army_id
        ",
    )?;
    let rows = stmt.query_map(params![profile_id], |row| row.get(0))?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .context("lecture des sectorielles du profil")
}

fn list_forms(conn: &Connection, profile_id: i64) -> Result<Vec<TtsProfileForm>> {
    let mut stmt = conn.prepare(
        "
        SELECT
            id, state_index, isc, loadout, nom_tts, description_tts,
            silhouette_size, silhouette_type, marker_name, mesh_url, marker_mesh_url
        FROM tts_profile_forms
        WHERE profile_id = ?1
        ORDER BY state_index, id
        ",
    )?;
    let rows = stmt.query_map(params![profile_id], |row| {
        Ok(TtsProfileForm {
            id: row.get(0)?,
            state_index: row.get(1)?,
            isc: row.get(2)?,
            loadout: row.get(3)?,
            nom_tts: row.get(4)?,
            description_tts: row.get(5)?,
            silhouette_size: row.get(6)?,
            silhouette_type: row.get(7)?,
            marker_name: row.get(8)?,
            mesh_url: row.get(9)?,
            marker_mesh_url: row.get(10)?,
            traits: Vec::new(),
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .context("lecture des formes TTS")
}

fn update_profile(conn: &Connection, id: i64, update: &ProfileUpdate) -> Result<TtsProfileDetail> {
    let isc = require_text(&update.isc, "ISC", 255)?;
    let nom_tts = require_text(&update.nom_tts, "nom TTS", 500)?;
    let loadout = bounded_text(&update.loadout, "loadout", 500)?;
    let description = bounded_text(&update.description_tts, "description TTS", 20_000)?;
    let silhouette_type = silhouette_kind(&description);
    if let Some(army_id) = update.army_id {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM armies WHERE id = ?1)",
            params![army_id],
            |row| row.get(0),
        )?;
        if !exists {
            bail!("sectorielle introuvable");
        }
    }
    let tx = conn.unchecked_transaction()?;
    let changed = tx.execute(
        "
        UPDATE tts_profiles
        SET army_id = ?2,
            isc = ?3,
            loadout = ?4,
            nom_tts = ?5,
            description_tts = ?6,
            silhouette_size = ?7,
            silhouette_type = ?8,
            marker_name = ?9
        WHERE id = ?1
        ",
        params![
            id,
            update.army_id,
            isc,
            loadout,
            nom_tts,
            description,
            blank(update.silhouette_size.as_deref()),
            silhouette_type,
            blank(update.marker_name.as_deref()),
        ],
    )?;
    if changed == 0 {
        bail!("profil introuvable");
    }
    for form in &update.forms {
        let form_isc = require_text(&form.isc, "ISC de la forme", 255)?;
        let form_nom = require_text(&form.nom_tts, "nom TTS de la forme", 500)?;
        let mut form_description =
            bounded_text(&form.description_tts, "description de la forme", 20_000)?;
        if form_nom.to_ascii_lowercase().contains("(sapper)") {
            form_description = sapper_description(&form_description);
        }
        let form_kind = silhouette_kind(&form_description);
        let form_size = at_least_sapper_size(&form_nom, blank(form.silhouette_size.as_deref()));
        let changed = tx.execute(
            "
            UPDATE tts_profile_forms
            SET isc = ?3,
                loadout = ?4,
                nom_tts = ?5,
                description_tts = ?6,
                silhouette_size = ?7,
                silhouette_type = ?8,
                marker_name = ?9
            WHERE id = ?1 AND profile_id = ?2
            ",
            params![
                form.id,
                id,
                form_isc,
                bounded_text(&form.loadout, "loadout de la forme", 500)?,
                form_nom,
                form_description,
                form_size,
                form_kind,
                blank(form.marker_name.as_deref()),
            ],
        )?;
        if changed == 0 {
            bail!("forme transformée introuvable");
        }
    }
    replace_sectorials(&tx, id, &update.sectorial_ids)?;
    tx.commit()?;
    get_profile(conn, id)?.context("profil introuvable après mise à jour")
}

fn map_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<TtsProfileSummary> {
    Ok(TtsProfileSummary {
        id: row.get(0)?,
        army_id: row.get(1)?,
        faction_label: row.get(2)?,
        isc: row.get(3)?,
        loadout: row.get(4)?,
        nom_tts: row.get(5)?,
        silhouette_size: row.get(6)?,
        silhouette_type: row.get(7)?,
        marker_name: row.get(8)?,
        form_count: row.get(9)?,
        linked: row.get::<_, i64>(10)? != 0,
        differences: String::new(),
    })
}

fn map_detail_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TtsProfileDetail> {
    Ok(TtsProfileDetail {
        id: row.get(0)?,
        army_id: row.get(1)?,
        faction_slug: row.get(2)?,
        faction_label: row.get(3)?,
        isc: row.get(4)?,
        loadout: row.get(5)?,
        nom_tts: row.get(6)?,
        description_tts: row.get(7)?,
        silhouette_size: row.get(8)?,
        silhouette_type: row.get(9)?,
        marker_name: row.get(10)?,
        mesh_url: row.get(11)?,
        marker_mesh_url: row.get(12)?,
        forms: Vec::new(),
        sectorial_ids: Vec::new(),
        traits: Vec::new(),
    })
}

fn replace_sectorials(conn: &Connection, profile_id: i64, army_ids: &[i64]) -> Result<()> {
    conn.execute(
        "DELETE FROM tts_profile_sectorials WHERE profile_id = ?1",
        params![profile_id],
    )?;
    let mut seen = std::collections::BTreeSet::new();
    for army_id in army_ids {
        if !seen.insert(*army_id) {
            continue;
        }
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM armies WHERE id = ?1)",
            params![army_id],
            |row| row.get(0),
        )?;
        if !exists {
            bail!("sectorielle introuvable");
        }
        conn.execute(
            "INSERT INTO tts_profile_sectorials (profile_id, army_id) VALUES (?1, ?2)",
            params![profile_id, army_id],
        )?;
    }
    Ok(())
}

fn require_text(value: &str, label: &str, max: usize) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        bail!("{label} manquant");
    }
    if trimmed.chars().count() > max {
        bail!("{label} trop long");
    }
    Ok(trimmed.to_string())
}

fn bounded_text(value: &str, label: &str, max: usize) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.chars().count() > max {
        bail!("{label} trop long");
    }
    Ok(trimmed.to_string())
}

fn blank(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

fn army_id_for_slug(conn: &Connection, slug: &str) -> Result<Option<i64>> {
    let mut stmt = conn.prepare(
        "
        SELECT id
        FROM armies
        WHERE lower(slug) = lower(?1)
        ORDER BY discontinued ASC, id ASC
        LIMIT 1
        ",
    )?;
    let mut rows = stmt.query(params![slug])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(row.get(0)?));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_colored_nickname_into_isc_and_loadout() {
        let (isc, loadout) = split_nickname("[00B0F2]FUSILIER[-] Combi Rifle");
        assert_eq!(isc, "FUSILIER");
        assert_eq!(loadout, "Combi Rifle");
    }

    #[test]
    fn reads_cube_silhouette_and_ignores_table_props() {
        let raw = r#"{
          "ObjectStates": [
            {
              "Nickname": "Faction Dice",
              "ContainedObjects": [{ "Nickname": "Red die", "Description": "" }]
            },
            {
              "Nickname": "[00B0F2]PanOceania[-] N5",
              "ContainedObjects": [{
                "Nickname": "[00B0F2]FUSILIER[-] Combi Rifle",
                "Description": "[b]LI[/b] ● Cube ● [AAFFAA]Regular[-]\n[b]BS[/b]: 12\n[B]S[/B]: 2\n",
                "CustomMesh": { "MeshURL": "https://example.test/fusilier.obj" },
                "States": {
                  "2": {
                    "Nickname": "Silhouette 2",
                    "Description": "",
                    "CustomMesh": { "MeshURL": "https://example.test/sil2.obj" }
                  }
                }
              }]
            }
          ]
        }"#;
        let profiles = parse_models_json(raw).unwrap();
        assert_eq!(profiles.len(), 1);
        let profile = &profiles[0];
        assert_eq!(profile.faction_slug.as_deref(), Some("panoceania"));
        assert_eq!(profile.faction_label, "PanOceania");
        assert_eq!(profile.isc, "FUSILIER");
        assert_eq!(profile.loadout, "Combi Rifle");
        assert_eq!(profile.silhouette_size.as_deref(), Some("2"));
        assert_eq!(profile.silhouette_type.as_deref(), Some("normale"));
        assert_eq!(profile.marker_name.as_deref(), Some("Silhouette 2"));
        assert_eq!(
            profile.mesh_url.as_deref(),
            Some("https://example.test/fusilier.obj")
        );
        assert!(profile.forms.is_empty());
    }

    #[test]
    fn attaches_transformed_form_and_its_silhouette() {
        let raw = r#"{
          "ObjectStates": [{
            "Nickname": "[007B25]Ariadna[-] N5",
            "ContainedObjects": [{
              "Nickname": "[007B25]IOANN BANN - DOGFACE FORM[-] Chain Rifle",
              "Description": "[b]WB[/b] ● Cube ● [AAFFAA]Regular[-]\n[B]S[/B]: 2\n",
              "States": {
                "2": { "Nickname": "Silhouette 2", "Description": "" },
                "3": {
                  "Nickname": "[007B25]IOANN BANN - DOG-WARRIOR FORM[-] Chain Rifle",
                  "Description": "[b]WB[/b] ● [FFFFAA]Irregular[-]\n[B]S[/B]: 6\n"
                },
                "4": { "Nickname": "Silhouette 6", "Description": "" }
              }
            }]
          }]
        }"#;
        let profiles = parse_models_json(raw).unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].silhouette_size.as_deref(), Some("2"));
        assert_eq!(profiles[0].forms.len(), 1);
        let form = &profiles[0].forms[0];
        assert_eq!(form.isc, "IOANN BANN - DOG-WARRIOR FORM");
        assert_eq!(form.silhouette_size.as_deref(), Some("6"));
        assert_eq!(form.marker_name.as_deref(), Some("Silhouette 6"));
        assert_eq!(form.silhouette_type.as_deref(), Some("normale"));
    }

    #[test]
    fn reads_camouflage_marker_size() {
        let raw = r#"{
          "ObjectStates": [{
            "Nickname": "[AFA7BC]Aleph[-] N5",
            "ContainedObjects": [{
              "Nickname": "[AFA7BC]YAKSHA[-] E/M Carbine",
              "Description": "[b]REM[/b] ● [AAFFAA]Regular[-]\n[B]S[/B]: 2\n",
              "States": {
                "2": { "Nickname": "Camouflage (-3) S2", "Description": "" }
              }
            }]
          }]
        }"#;
        let profiles = parse_models_json(raw).unwrap();
        assert_eq!(profiles[0].faction_slug.as_deref(), Some("aleph"));
        assert_eq!(profiles[0].silhouette_size.as_deref(), Some("2"));
        assert_eq!(
            profiles[0].marker_name.as_deref(),
            Some("Camouflage (-3) S2")
        );
        assert_eq!(profiles[0].silhouette_type.as_deref(), Some("normale"));
        assert!(profiles[0].forms.is_empty());
    }

    #[test]
    fn classifies_one_silhouette_kind_by_priority() {
        assert_eq!(
            silhouette_kind("Skills ● Camouflage(1 Use) ● Mimetism(-3)"),
            "camo-3"
        );
        assert_eq!(
            silhouette_kind("Skills ● Camouflage ● Mimetism(-6) ● Impersonation"),
            "camo-6"
        );
        assert_eq!(
            silhouette_kind("Skills ● Camouflage(1 Use) ● Killer Hacking Device"),
            "camo-0"
        );
        assert_eq!(silhouette_kind("Skills ● Impersonation"), "imp-1");
        assert_eq!(silhouette_kind("Skills ● Impersonation(IMP-2)"), "imp-2");
        assert_eq!(
            silhouette_kind("Equipment ● Hacking Device Plus ● Skills ● Stealth"),
            "imp-2"
        );
        assert_eq!(
            silhouette_kind("Equipment ● Hacking Device(UPGRADE: Cybermask)"),
            "imp-2"
        );
        assert_eq!(
            silhouette_kind("Equipment ● Killer Hacking Device ● Skills ● Camouflage"),
            "camo-0"
        );
        assert_eq!(silhouette_kind("Skills ● Hacker ● Cube"), "normale");
        assert_eq!(
            silhouette_kind("----------Skills --------- Camouflage ● Mimetism(-3)"),
            "camo-3"
        );
        assert_eq!(
            silhouette_kind("----------Skills --------- Impersonation(IMP-2)"),
            "imp-2"
        );
    }

    #[test]
    fn adds_a_sapper_profile_at_silhouette_four_or_more() {
        let raw = r#"{
          "ObjectStates": [{
            "Nickname": "[007B25]Ariadna[-] N5",
            "ContainedObjects": [{
              "Nickname": "[007B25]ZOUAVE[-] Rifle",
              "Description": "[b]LI[/b] ● [AAFFAA]Regular[-]\n[B]S[/B]: 2\n----------Skills --------- Sapper ● Courage\n",
              "CustomMesh": { "MeshURL": "https://example.test/zouave.obj" },
              "States": {
                "2": { "Nickname": "Silhouette 2", "Description": "" },
                "3": { "Nickname": "Silhouette 3", "Description": "" }
              }
            }]
          }]
        }"#;
        let profiles = parse_models_json(raw).unwrap();
        let profile = &profiles[0];
        assert_eq!(profile.silhouette_size.as_deref(), Some("2"));
        assert_eq!(profile.forms.len(), 1);
        let sapper = &profile.forms[0];
        assert_eq!(sapper.nom_tts, "[007B25]ZOUAVE[-] Rifle (sapper)");
        assert_eq!(sapper.isc, "ZOUAVE");
        assert_eq!(sapper.loadout, "Rifle");
        assert_eq!(sapper.silhouette_size.as_deref(), Some("4"));
        assert_eq!(sapper.marker_name.as_deref(), Some("Silhouette 4"));
        assert_eq!(sapper.silhouette_type.as_deref(), Some("normale"));
        assert!(sapper.description_tts.contains("[B]S[/B]: 4"));
        assert!(!sapper.description_tts.contains("[B]S[/B]: 2"));
    }

    #[test]
    fn drops_exact_duplicates_and_nests_escape_operator() {
        let raw = r#"{
          "ObjectStates": [{
            "Nickname": "[007B25]Ariadna[-] N5",
            "ContainedObjects": [
              {
                "Nickname": "[007B25]OPERATOR[-] Specialist, Spitfire",
                "Description": "[b]LI[/b]\n[B]S[/B]: 2\n----------Skills --------- Remote Presence\n",
                "States": { "2": { "Nickname": "Silhouette 2", "Description": "" } }
              },
              {
                "Nickname": "[007B25]OPERATOR[-] Specialist, Spitfire",
                "Description": "[b]LI[/b]\n[B]S[/B]: 2\n----------Skills --------- Remote Presence\n"
              },
              {
                "Nickname": "[007B25]ANACONDA[-] HMG",
                "Description": "[b]TAG[/b]\n[B]S[/B]: 7\n----------Skills --------- Transmutation(Escape System -2)\n"
              },
              {
                "Nickname": "[007B25]ANACONDA[-] AP Spitfire",
                "Description": "[b]TAG[/b]\n[B]S[/B]: 7\n----------Skills --------- Transmutation(Escape System -2)\n"
              }
            ]
          }]
        }"#;
        let profiles = parse_models_json(raw).unwrap();
        assert_eq!(profiles.len(), 2);
        assert!(profiles.iter().all(|profile| profile.isc == "ANACONDA"));
        for profile in &profiles {
            assert_eq!(profile.forms.len(), 1);
            assert_eq!(profile.forms[0].isc, "OPERATOR");
            assert_eq!(profile.forms[0].loadout, "Specialist, Spitfire");
            assert_eq!(profile.forms[0].silhouette_size.as_deref(), Some("2"));
        }
    }

    #[test]
    fn pairs_each_escape_vehicle_with_its_operator_loadout() {
        let raw = r#"{
          "ObjectStates": [{
            "Nickname": "[AFA7BC]Aleph[-] N5",
            "ContainedObjects": [
              {
                "Nickname": "[AFA7BC]TARKSIA OPERATOR[-] FO, Red Fury, Chain Rifle",
                "Description": "[B]S[/B]: 2\n"
              },
              {
                "Nickname": "[AFA7BC]TARKSIA OPERATOR[-] FO, MMR, Chain Rifle",
                "Description": "[B]S[/B]: 2\n"
              },
              {
                "Nickname": "[AFA7BC]TARKSIA[-] MULTI Red Fury, Chain Rifle",
                "Description": "[B]S[/B]: 7\n----------Skills --------- Transmutation(Escape System -2)\n"
              },
              {
                "Nickname": "[AFA7BC]TARKSIA[-] Multi MMR, Chain Rifle",
                "Description": "[B]S[/B]: 7\n----------Skills --------- Transmutation(Escape System -2)\n"
              },
              {
                "Nickname": "[AFA7BC]REINF. OPERATOR MAXIMUS[-] Specialist, AP Thunderbolt",
                "Description": "[B]S[/B]: 2\n"
              },
              {
                "Nickname": "[AFA7BC]OPERATOR MAXIMUS[-] Specialist, AP Heavy Machine Gun",
                "Description": "[B]S[/B]: 2\n"
              },
              {
                "Nickname": "[AFA7BC]MÁXIMUS REINF.[-] AP Thunderbolt",
                "Description": "[B]S[/B]: 6\n----------Skills --------- Transmutation(Escape System -3 )\n"
              },
              {
                "Nickname": "[AFA7BC]MAXIMUS OPTIMATE[-] AP Heavy Machine Gun",
                "Description": "[B]S[/B]: 6\n----------Skills --------- Transmutation(Escape System -3 )\n"
              }
            ]
          }]
        }"#;
        let profiles = parse_models_json(raw).unwrap();
        assert_eq!(profiles.len(), 4);
        let form_of = |loadout: &str| {
            profiles
                .iter()
                .find(|profile| profile.loadout == loadout)
                .unwrap()
                .forms[0]
                .clone()
        };
        assert_eq!(
            form_of("MULTI Red Fury, Chain Rifle").loadout,
            "FO, Red Fury, Chain Rifle"
        );
        assert_eq!(
            form_of("Multi MMR, Chain Rifle").loadout,
            "FO, MMR, Chain Rifle"
        );
        assert_eq!(form_of("AP Thunderbolt").isc, "REINF. OPERATOR MAXIMUS");
        assert_eq!(
            form_of("AP Heavy Machine Gun").loadout,
            "Specialist, AP Heavy Machine Gun"
        );
    }

    #[test]
    fn keeps_a_transmutation_profile_beside_the_base() {
        let raw = r#"{
          "ObjectStates": [{
            "Nickname": "[813CAA]Combined Army[-] N5",
            "ContainedObjects": [{
              "Nickname": "[813CAA]DIGGER REINF.[-] Rifle",
              "Description": "[b]LI[/b] ● Regular\n[B]S[/B]: 2\nSkills ● Transmutation(W) ● Camouflage ● Mimetism(-3)\n",
              "States": {
                "2": {
                  "Nickname": "[813CAA]DIGGER REINF. (BATTLE-RAVAGED)[-] Rifle",
                  "Description": "[b]LI[/b] ● Regular\n[B]S[/B]: 2\nSkills ● Transmutation(W)\n"
                },
                "3": { "Nickname": "Silhouette 2", "Description": "" }
              }
            }]
          }]
        }"#;
        let profiles = parse_models_json(raw).unwrap();
        assert_eq!(profiles[0].silhouette_type.as_deref(), Some("camo-3"));
        assert_eq!(profiles[0].forms.len(), 1);
        let form = &profiles[0].forms[0];
        assert_eq!(form.isc, "DIGGER REINF. (BATTLE-RAVAGED)");
        assert_eq!(form.silhouette_size.as_deref(), Some("2"));
        assert_eq!(form.marker_name.as_deref(), Some("Silhouette 2"));
        assert_eq!(form.silhouette_type.as_deref(), Some("normale"));
    }

    #[test]
    fn lists_a_sectorial_with_its_faction_catalog_and_updates() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS armies (
                id INTEGER PRIMARY KEY,
                parent_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                slug TEXT NOT NULL,
                logo_url TEXT NOT NULL,
                discontinued INTEGER NOT NULL DEFAULT 0,
                short_name TEXT,
                synced_at INTEGER NOT NULL
            );
            ",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO armies (id, parent_id, name, slug, logo_url, discontinued, synced_at) VALUES (101, 101, 'PanO', 'panoceania', '', 0, 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO armies (id, parent_id, name, slug, logo_url, discontinued, synced_at) VALUES (103, 101, 'MO', 'military-orders', '', 0, 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tts_profiles (army_id, faction_label, isc, loadout, nom_tts, description_tts, source_bag, source_index) VALUES (101, 'PanOceania', 'FUSILIER', 'Combi', 'FUSILIER', 'desc', 'bag', 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tts_profiles (army_id, faction_label, isc, loadout, nom_tts, description_tts, source_bag, source_index) VALUES (103, 'PanOceania', 'MAGISTER', 'HMG', 'MAGISTER', 'desc', 'bag', 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tts_profile_forms (profile_id, state_index, isc, nom_tts) VALUES (1, 3, 'DOG', 'DOG')",
            [],
        )
        .unwrap();

        let shared = list_profiles(
            &conn,
            &ProfileListQuery {
                army_id: Some(103),
                also_army_id: Some(101),
                unassigned: false,
            },
        )
        .unwrap();
        assert_eq!(shared.len(), 2);
        assert_eq!(shared[0].form_count, 1);

        let only = list_profiles(
            &conn,
            &ProfileListQuery {
                army_id: Some(103),
                also_army_id: None,
                unassigned: false,
            },
        )
        .unwrap();
        assert_eq!(only.len(), 1);
        assert_eq!(only[0].isc, "MAGISTER");

        let updated = update_profile(
            &conn,
            1,
            &ProfileUpdate {
                army_id: Some(103),
                isc: "FUSILIER".into(),
                loadout: "HMG".into(),
                nom_tts: "Fusilier HMG".into(),
                description_tts: "Skills ● Camouflage ● Mimetism(-6)".into(),
                silhouette_size: Some("2".into()),
                marker_name: None,
                forms: vec![ProfileFormUpdate {
                    id: 1,
                    isc: "WARRIOR".into(),
                    loadout: String::new(),
                    nom_tts: "Warrior (sapper)".into(),
                    description_tts: "[B]S[/B]: 2\nSkills ● Impersonation".into(),
                    silhouette_size: Some("2".into()),
                    marker_name: None,
                }],
                sectorial_ids: vec![103, 103],
            },
        )
        .unwrap();
        assert_eq!(updated.army_id, Some(103));
        assert_eq!(updated.loadout, "HMG");
        assert_eq!(updated.silhouette_type.as_deref(), Some("camo-6"));
        assert_eq!(updated.sectorial_ids, vec![103]);
        assert_eq!(updated.forms[0].silhouette_type.as_deref(), Some("imp-1"));
        assert_eq!(updated.forms[0].silhouette_size.as_deref(), Some("4"));

        let linked = list_profiles(
            &conn,
            &ProfileListQuery {
                army_id: Some(103),
                also_army_id: None,
                unassigned: false,
            },
        )
        .unwrap();
        assert_eq!(linked.len(), 2);
        assert!(linked
            .iter()
            .any(|profile| profile.isc == "FUSILIER" && profile.linked));
        assert_eq!(updated.forms[0].isc, "WARRIOR");
        assert!(updated.forms[0].description_tts.contains("[B]S[/B]: 4"));
    }

    #[test]
    fn reports_description_subtleties_for_the_same_loadout() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS armies (
                id INTEGER PRIMARY KEY,
                parent_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                slug TEXT NOT NULL,
                logo_url TEXT NOT NULL,
                discontinued INTEGER NOT NULL DEFAULT 0,
                short_name TEXT,
                synced_at INTEGER NOT NULL
            );
            INSERT INTO armies (id, parent_id, name, slug, logo_url, discontinued, synced_at)
            VALUES (101, 101, 'Aleph', 'aleph', '', 0, 0);
            ",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tts_profiles (army_id, faction_label, isc, loadout, nom_tts, description_tts, source_bag, source_index) VALUES (101, 'Aleph', 'THYREOS', 'SMG', 'THYREOS', 'Skills ● Camouflage ● Mimetism(-3)', 'bag', 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tts_profiles (army_id, faction_label, isc, loadout, nom_tts, description_tts, source_bag, source_index) VALUES (101, 'Aleph', 'THYREOS', 'SMG', 'THYREOS', 'Skills ● Camouflage ● Mimetism(-3) ● Minelayer', 'bag', 1)",
            [],
        )
        .unwrap();
        let listed = list_profiles(
            &conn,
            &ProfileListQuery {
                army_id: Some(101),
                also_army_id: None,
                unassigned: false,
            },
        )
        .unwrap();
        assert_eq!(listed.len(), 2);
        let plain = listed
            .iter()
            .find(|profile| profile.differences.is_empty())
            .unwrap();
        let mined = listed
            .iter()
            .find(|profile| !profile.differences.is_empty())
            .unwrap();
        assert!(mined.differences.contains("Minelayer"));
        assert_eq!(plain.isc, "THYREOS");
    }

    #[test]
    fn lists_loadouts_of_one_isc_and_the_usual_silhouette_mesh() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS armies (
                id INTEGER PRIMARY KEY,
                parent_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                slug TEXT NOT NULL,
                logo_url TEXT NOT NULL,
                discontinued INTEGER NOT NULL DEFAULT 0,
                short_name TEXT,
                synced_at INTEGER NOT NULL
            );
            ",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tts_profiles (faction_label, isc, loadout, nom_tts, description_tts, silhouette_size, silhouette_type, marker_name, marker_mesh_url, source_bag, source_index) VALUES ('Haqqislam', 'YUAN YUAN', 'DA', 'YUAN', 'Skills ● Booty', '2', 'normale', 'Silhouette 2', 'https://example.test/rare.obj', 'bag', 0)",
            [],
        )
        .unwrap();
        let yuan_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO tts_profiles (faction_label, isc, loadout, nom_tts, description_tts, silhouette_size, silhouette_type, marker_name, marker_mesh_url, source_bag, source_index) VALUES ('Haqqislam', 'YUAN YUAN', 'DA', 'YUAN', 'Skills ● Booty ● Mimetism(-3)', '2', 'camo-3', 'Camouflage (-3) S2', 'https://example.test/s2.obj', 'bag', 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tts_profiles (faction_label, isc, loadout, nom_tts, description_tts, marker_name, marker_mesh_url, source_bag, source_index) VALUES ('PanOceania', 'FUSILIER', 'Combi', 'FUSILIER', '', 'Silhouette 2', 'https://example.test/s2.obj', 'bag', 2)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tts_profiles (faction_label, isc, loadout, nom_tts, description_tts, marker_name, marker_mesh_url, source_bag, source_index) VALUES ('PanOceania', 'FUSILIER', 'HMG', 'FUSILIER', '', 'Silhouette 2', 'https://example.test/s2.obj', 'bag', 3)",
            [],
        )
        .unwrap();
        assert_eq!(
            conn.execute(
                "INSERT INTO tts_profile_forms (profile_id, state_index, isc, loadout, nom_tts, description_tts, marker_name) VALUES (?1, 900, 'STALDRON', 'STALDRON', 'STALDRON', '', 'Silhouette 1')",
                params![yuan_id],
            )
            .unwrap(),
            1
        );
        let staldrone_id = conn.last_insert_rowid();
        assert_eq!(
            conn.execute(
                "INSERT INTO tts_profile_forms (profile_id, state_index, isc, nom_tts, marker_name, marker_mesh_url) VALUES (?1, 1, 'DOG', 'DOG', 'Silhouette 6', 'https://example.test/s6.obj')",
                params![yuan_id],
            )
            .unwrap(),
            1
        );

        let loadouts = list_loadouts(&conn, "yuan yuan", None).unwrap();
        assert_eq!(loadouts.len(), 2);
        assert!(loadouts.iter().all(|row| row.loadout == "DA"));
        let with_form = loadouts.iter().find(|row| row.extra_forms == 1).unwrap();
        assert!(with_form.differences.is_empty());
        let camo = loadouts.iter().find(|row| row.extra_forms == 0).unwrap();
        assert!(camo.differences.contains("Mimetism(-3)"));

        let peripherals = list_form_loadouts(&conn, "staldrOn", None).unwrap();
        assert_eq!(peripherals.len(), 1);
        assert_eq!(peripherals[0].form_id, Some(staldrone_id));
        assert_eq!(peripherals[0].profile_id, yuan_id);
        assert_eq!(peripherals[0].marker_name.as_deref(), Some("Silhouette 1"));

        let meshes = marker_mesh_catalog(&conn).unwrap();
        assert_eq!(
            meshes.get("Silhouette 2").map(String::as_str),
            Some("https://example.test/s2.obj")
        );
        assert_eq!(
            meshes.get("Silhouette 6").map(String::as_str),
            Some("https://example.test/s6.obj")
        );
    }

    #[test]
    fn loadouts_stay_inside_the_chosen_sectorial() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS armies (
                id INTEGER PRIMARY KEY,
                parent_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                slug TEXT NOT NULL,
                logo_url TEXT NOT NULL,
                discontinued INTEGER NOT NULL DEFAULT 0,
                short_name TEXT,
                synced_at INTEGER NOT NULL
            );
            ",
        )
        .unwrap();
        conn.execute_batch(
            "
            INSERT INTO armies (id, parent_id, name, slug, logo_url, discontinued, synced_at) VALUES
                (401, 401, 'Haqqislam', 'haqqislam', '', 0, 0),
                (402, 401, 'Hassassin', 'hassassin', '', 0, 0),
                (101, 101, 'PanOceania', 'panoceania', '', 0, 0);
            ",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tts_profiles (army_id, faction_label, isc, loadout, nom_tts, source_bag, source_index) VALUES (401, 'Haqqislam', 'YUAN YUAN', 'DA', 'YUAN', 'bag', 0)",
            [],
        )
        .unwrap();
        let haqq_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO tts_profiles (army_id, faction_label, isc, loadout, nom_tts, source_bag, source_index) VALUES (101, 'PanOceania', 'YUAN YUAN', 'HMG', 'YUAN', 'bag', 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tts_profile_sectorials (profile_id, army_id) VALUES (?1, 402)",
            params![haqq_id],
        )
        .unwrap();

        let haqq = list_loadouts(&conn, "YUAN YUAN", Some(401)).unwrap();
        assert_eq!(haqq.len(), 1);
        assert_eq!(haqq[0].loadout, "DA");
        let linked = list_loadouts(&conn, "YUAN YUAN", Some(402)).unwrap();
        assert_eq!(linked.len(), 1);
        assert_eq!(linked[0].profile_id, haqq_id);
        assert!(list_loadouts(&conn, "YUAN YUAN", Some(103))
            .unwrap()
            .is_empty());
        assert!(profile_in_army(&conn, haqq_id, 402).unwrap());
        assert!(!profile_in_army(&conn, haqq_id, 101).unwrap());
    }
}

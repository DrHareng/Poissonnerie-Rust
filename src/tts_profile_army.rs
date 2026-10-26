//! Profils TTS construits depuis l'API Infinity Army, avec leurs sectorielles.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde_json::Value;

use crate::migrate::migrate;
use crate::tts_profile::{parse_models_json, UnitOptionInclude};

const METADATA_URL: &str = "https://api.corvusbelli.com/army/infinity/en/metadata";
const ORIGIN: &str = "https://infinityuniverse.com";
const REFERER: &str = "https://infinityuniverse.com/army/infinity";

#[derive(Debug, Clone)]
struct ArmyRow {
    id: i64,
    parent_id: i64,
    name: String,
    slug: String,
}

#[derive(Debug, Clone)]
struct TraitDraft {
    kind: &'static str,
    name: String,
    detail: Option<String>,
}

#[derive(Debug, Clone)]
struct FormDraft {
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
    traits: Vec<TraitDraft>,
}

#[derive(Debug, Clone)]
struct ProfileDraft {
    unit_id: i64,
    group_id: i64,
    option_id: i64,
    army_id: Option<i64>,
    faction_slug: Option<String>,
    faction_label: String,
    /// Nom d'unité TTS : `option.name` (NISSE), pas le titre long `profiles[0].name`.
    isc: String,
    loadout: String,
    nom_tts: String,
    description_tts: String,
    silhouette_size: Option<String>,
    silhouette_type: Option<String>,
    marker_name: Option<String>,
    mesh_url: Option<String>,
    marker_mesh_url: Option<String>,
    traits: Vec<TraitDraft>,
    forms: Vec<FormDraft>,
    sectorials: BTreeSet<i64>,
    peripherals: Vec<(String, i64)>,
    escape_pilot: bool,
    operator: bool,
}

#[derive(Debug, Clone)]
struct MeshHit {
    isc: String,
    loadout: String,
    mesh_url: Option<String>,
    marker_mesh_url: Option<String>,
    marker_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ArmyImportReport {
    pub armies: usize,
    pub profiles: usize,
    pub forms: usize,
    pub sectorials: usize,
    pub failed_armies: usize,
}

#[derive(Clone)]
struct Names {
    skills: HashMap<i64, String>,
    weapons: HashMap<i64, String>,
    equips: HashMap<i64, String>,
    extras: HashMap<i64, String>,
    chars: HashMap<i64, String>,
    types: HashMap<i64, String>,
    peripherals: HashMap<i64, String>,
}

pub fn import_from_army_api(
    conn: &Connection,
    mesh_file: Option<&Path>,
) -> Result<ArmyImportReport> {
    migrate(conn)?;
    let armies = load_armies(conn)?;
    let meshes = snapshot_meshes(conn, mesh_file)?;
    let colors = snapshot_colors(conn)?;
    let client = reqwest::blocking::Client::builder()
        .user_agent("poissonnerie-elo/0.1")
        .build()?;
    let metadata = fetch_json(&client, METADATA_URL)?;
    let mut names = names_from_metadata(&metadata);
    let mut drafts: BTreeMap<(i64, i64, i64), ProfileDraft> = BTreeMap::new();
    let mut unit_includes: BTreeMap<(i64, i64), Vec<UnitOptionInclude>> = BTreeMap::new();
    let mut failed = 0usize;
    let mut fetched = 0usize;
    for army in armies.iter().filter(|army| importable_army(army)) {
        let url = format!("https://api.corvusbelli.com/army/units/en/{}", army.id);
        let document = match fetch_json(&client, &url) {
            Ok(document) => document,
            Err(error) => {
                eprintln!(
                    "  {name} ({id}) ignorée : {error}",
                    name = army.name,
                    id = army.id
                );
                failed += 1;
                continue;
            }
        };
        fetched += 1;
        eprintln!("  {} ({})", army.name, army.id);
        absorb_filters(&mut names, &document);
        absorb_units(&mut drafts, &names, army, &armies, &document);
        absorb_unit_includes(&mut unit_includes, &document);
    }
    let mut profiles: Vec<ProfileDraft> = drafts.into_values().collect();
    if profiles.is_empty() {
        anyhow::bail!("l'API Infinity Army n'a renvoyé aucun profil");
    }
    attach_escape_pilots(&mut profiles);
    attach_sapper_forms(&mut profiles);
    apply_peripheral_names(&mut profiles);
    // AI Motorcycle : le dismounted (+ périphérique) devient forme du monté, pas un slot séparé.
    attach_ai_motorcycle_forms(&mut profiles, &mut unit_includes);
    apply_meshes_and_colors(&mut profiles, &meshes, &colors);
    if let Some(path) = mesh_file.filter(|path| path.exists()) {
        profiles.extend(mod_only_profiles(path, &armies)?);
    }
    let includes: Vec<UnitOptionInclude> = unit_includes.into_values().flatten().collect();
    let report = store_profiles(conn, &profiles, &includes)?;
    Ok(ArmyImportReport {
        armies: fetched,
        failed_armies: failed,
        ..report
    })
}

/// Récupère les `includes` d'une option d'unité depuis l'API Army (sectorielle donnée).
pub fn fetch_unit_option_includes(
    army_id: u32,
    unit_id: u32,
    unit_option_id: u32,
) -> Result<Vec<UnitOptionInclude>> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("poissonnerie-elo/0.1")
        .build()?;
    let url = format!("https://api.corvusbelli.com/army/units/en/{army_id}");
    let document = fetch_json(&client, &url)?;
    Ok(unit_option_includes_from_document(
        &document,
        i64::from(unit_id),
        i64::from(unit_option_id),
    ))
}

fn unit_option_includes_from_document(
    document: &Value,
    unit_id: i64,
    unit_option_id: i64,
) -> Vec<UnitOptionInclude> {
    let Some(units) = document.get("units").and_then(Value::as_array) else {
        return Vec::new();
    };
    let Some(unit) = units
        .iter()
        .find(|candidate| candidate.get("id").and_then(Value::as_i64) == Some(unit_id))
    else {
        return Vec::new();
    };
    parse_unit_option_includes(unit, unit_id, unit_option_id)
}

fn parse_unit_option_includes(
    unit: &Value,
    unit_id: i64,
    unit_option_id: i64,
) -> Vec<UnitOptionInclude> {
    let Some(options) = unit.get("options").and_then(Value::as_array) else {
        return Vec::new();
    };
    let Some(option) = options.iter().find(|candidate| {
        candidate.get("id").and_then(Value::as_i64) == Some(unit_option_id)
            && !candidate
                .get("disabled")
                .and_then(Value::as_bool)
                .unwrap_or(false)
    }) else {
        return Vec::new();
    };
    let Some(includes) = option.get("includes").and_then(Value::as_array) else {
        return Vec::new();
    };
    includes
        .iter()
        .enumerate()
        .filter_map(|(sort_order, include)| {
            let group_id = include.get("group").and_then(Value::as_i64)?;
            let option_id = include.get("option").and_then(Value::as_i64)?;
            let quantity = include
                .get("q")
                .and_then(Value::as_i64)
                .unwrap_or(1)
                .max(1);
            Some(UnitOptionInclude {
                unit_id,
                unit_option_id,
                sort_order: sort_order as i64,
                group_id,
                option_id,
                quantity,
            })
        })
        .collect()
}

fn absorb_unit_includes(
    dest: &mut BTreeMap<(i64, i64), Vec<UnitOptionInclude>>,
    document: &Value,
) {
    let Some(units) = document.get("units").and_then(Value::as_array) else {
        return;
    };
    for unit in units {
        let Some(unit_id) = unit.get("id").and_then(Value::as_i64) else {
            continue;
        };
        let Some(options) = unit.get("options").and_then(Value::as_array) else {
            continue;
        };
        for option in options {
            if option
                .get("disabled")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                continue;
            }
            let Some(unit_option_id) = option.get("id").and_then(Value::as_i64) else {
                continue;
            };
            let includes = parse_unit_option_includes(unit, unit_id, unit_option_id);
            if includes.is_empty() {
                continue;
            }
            dest.entry((unit_id, unit_option_id))
                .or_insert(includes);
        }
    }
}

fn fetch_json(client: &reqwest::blocking::Client, url: &str) -> Result<Value> {
    let response = client
        .get(url)
        .header("Origin", ORIGIN)
        .header("Referer", REFERER)
        .send()
        .with_context(|| format!("requête {url}"))?;
    if !response.status().is_success() {
        anyhow::bail!("{url} a répondu {}", response.status());
    }
    response.json().with_context(|| format!("JSON {url}"))
}

fn importable_army(army: &ArmyRow) -> bool {
    army.id % 100 != 99 && army.slug != "non-aligned-armies" && army.slug != "contracted-back-up"
}

fn mod_only_profiles(path: &Path, armies: &[ArmyRow]) -> Result<Vec<ProfileDraft>> {
    let parsed = parse_models_json(&std::fs::read_to_string(path)?)?;
    Ok(parsed
        .into_iter()
        .filter(|profile| {
            matches!(
                profile.faction_slug.as_deref(),
                None | Some("contracted-back-up")
            )
        })
        .map(|profile| parsed_to_draft(profile, armies))
        .collect())
}

fn parsed_to_draft(profile: crate::tts_profile::ParsedProfile, armies: &[ArmyRow]) -> ProfileDraft {
    let army = profile
        .faction_slug
        .as_ref()
        .and_then(|slug| armies.iter().find(|army| &army.slug == slug));
    let forms = profile
        .forms
        .into_iter()
        .map(|form| {
            let (isc, loadout, traits) = place_pipe_suffix(&form.isc, &form.loadout);
            FormDraft {
                state_index: form.state_index,
                isc,
                loadout,
                nom_tts: form.nom_tts,
                description_tts: form.description_tts,
                silhouette_size: form.silhouette_size,
                silhouette_type: form.silhouette_type,
                marker_name: form.marker_name,
                mesh_url: form.mesh_url,
                marker_mesh_url: form.marker_mesh_url,
                traits,
            }
        })
        .collect();
    let (isc, loadout, traits) = place_pipe_suffix(&profile.isc, &profile.loadout);
    ProfileDraft {
        unit_id: 0,
        group_id: 0,
        option_id: profile.source_index,
        army_id: army.map(|army| army.id),
        faction_slug: profile.faction_slug,
        faction_label: profile.faction_label,
        isc,
        loadout,
        nom_tts: profile.nom_tts,
        description_tts: profile.description_tts,
        silhouette_size: profile.silhouette_size,
        silhouette_type: profile.silhouette_type,
        marker_name: profile.marker_name,
        mesh_url: profile.mesh_url,
        marker_mesh_url: profile.marker_mesh_url,
        traits,
        forms,
        sectorials: BTreeSet::new(),
        peripherals: Vec::new(),
        escape_pilot: false,
        operator: false,
    }
}

fn load_armies(conn: &Connection) -> Result<Vec<ArmyRow>> {
    let mut stmt = conn.prepare(
        "
        SELECT id, parent_id, name, slug
        FROM armies
        ORDER BY id
        ",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(ArmyRow {
            id: row.get(0)?,
            parent_id: row.get(1)?,
            name: row.get(2)?,
            slug: row.get(3)?,
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .context("lecture des armées")
}

fn names_from_metadata(metadata: &Value) -> Names {
    Names {
        skills: id_name_map(metadata.get("skills")),
        weapons: id_name_map(metadata.get("weapons")),
        equips: id_name_map(metadata.get("equips")),
        extras: HashMap::new(),
        chars: HashMap::new(),
        types: HashMap::new(),
        peripherals: HashMap::new(),
    }
}

fn id_name_map(value: Option<&Value>) -> HashMap<i64, String> {
    let mut map = HashMap::new();
    let Some(items) = value.and_then(Value::as_array) else {
        return map;
    };
    for item in items {
        if let (Some(id), Some(name)) = (
            item.get("id").and_then(Value::as_i64),
            item.get("name").and_then(Value::as_str),
        ) {
            map.insert(id, name.to_string());
        }
    }
    map
}

fn absorb_filters(names: &mut Names, document: &Value) {
    let Some(filters) = document.get("filters") else {
        return;
    };
    names.extras.extend(id_name_map(filters.get("extras")));
    names.chars.extend(id_name_map(filters.get("chars")));
    names.types.extend(id_name_map(filters.get("type")));
    names.weapons.extend(id_name_map(filters.get("weapons")));
    names.skills.extend(id_name_map(filters.get("skills")));
    names.equips.extend(id_name_map(filters.get("equip")));
    names
        .peripherals
        .extend(id_name_map(filters.get("peripheral")));
}

fn absorb_units(
    drafts: &mut BTreeMap<(i64, i64, i64), ProfileDraft>,
    names: &Names,
    army: &ArmyRow,
    armies: &[ArmyRow],
    document: &Value,
) {
    let Some(units) = document.get("units").and_then(Value::as_array) else {
        return;
    };
    for unit in units {
        let Some(unit_id) = unit.get("id").and_then(Value::as_i64) else {
            continue;
        };
        let unit_name = unit
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Profil")
            .trim()
            .to_string();
        let Some(groups) = unit.get("profileGroups").and_then(Value::as_array) else {
            continue;
        };
        for group in groups {
            let group_id = group.get("id").and_then(Value::as_i64).unwrap_or(0);
            let base = group
                .get("profiles")
                .and_then(Value::as_array)
                .and_then(|profiles| profiles.first());
            let Some(base) = base else {
                continue;
            };
            // Repli si option.name est vide : titre de carte → ISC groupe → nom d'unité.
            let fallback_isc = base
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .or_else(|| {
                    group
                        .get("isc")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string)
                })
                .unwrap_or_else(|| unit_name.clone());
            let Some(options) = group.get("options").and_then(Value::as_array) else {
                continue;
            };
            let mut used_loadouts = BTreeSet::new();
            for option in options {
                if !option_is_importable(unit, group_id, option) {
                    continue;
                }
                let option_id = option.get("id").and_then(Value::as_i64).unwrap_or(0);
                // Clé applicative = option.name (NISSE / PALBOT), pas profiles[0].name
                // (Svalarheima NISSES / PALBOTS) ni l'ISC pluriel.
                let isc = option
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| fallback_isc.clone());
                let key = (unit_id, group_id, option_id);
                let factions = unit_factions(unit);
                let listed = factions.contains(&army.id);
                if let Some(existing) = drafts.get_mut(&key) {
                    if listed {
                        existing.sectorials.insert(army.id);
                    }
                    continue;
                }
                if !listed {
                    continue;
                }
                if let Some(draft) = build_profile(
                    names,
                    armies,
                    army,
                    unit,
                    unit_id,
                    group_id,
                    option_id,
                    &isc,
                    group,
                    base,
                    option,
                    &mut used_loadouts,
                ) {
                    drafts.insert(key, draft);
                }
            }
        }
    }
}

/// Options marquées `disabled` au niveau du groupe restent importables si une option
/// d'unité active les référence via `includes` (Polaris Team, Mirage-5, Scarface…).
fn option_is_importable(unit: &Value, group_id: i64, option: &Value) -> bool {
    if !option
        .get("disabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return true;
    }
    let option_id = option.get("id").and_then(Value::as_i64).unwrap_or(0);
    let Some(unit_options) = unit.get("options").and_then(Value::as_array) else {
        return false;
    };
    unit_options.iter().any(|unit_option| {
        if unit_option
            .get("disabled")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return false;
        }
        unit_option
            .get("includes")
            .and_then(Value::as_array)
            .is_some_and(|includes| {
                includes.iter().any(|include| {
                    include.get("group").and_then(Value::as_i64) == Some(group_id)
                        && include.get("option").and_then(Value::as_i64) == Some(option_id)
                })
            })
    })
}

fn unit_factions(unit: &Value) -> Vec<i64> {
    unit.get("factions")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_i64).collect())
        .unwrap_or_default()
}

fn has_named_skill(names: &Names, skills: &[RefItem], skill_name: &str) -> bool {
    skills.iter().any(|skill| {
        names
            .skills
            .get(&skill.id)
            .is_some_and(|name| name.eq_ignore_ascii_case(skill_name))
    })
}

fn build_profile(
    names: &Names,
    armies: &[ArmyRow],
    army: &ArmyRow,
    unit: &Value,
    unit_id: i64,
    group_id: i64,
    option_id: i64,
    isc: &str,
    group: &Value,
    base: &Value,
    option: &Value,
    used_loadouts: &mut BTreeSet<String>,
) -> Option<ProfileDraft> {
    let weapons = merged_refs(option.get("weapons"), base.get("weapons"));
    let skills = merged_refs(base.get("skills"), option.get("skills"));
    // Commlink = option réservée au mode renfort, sans intérêt pour les profils TTS.
    if has_named_skill(names, &skills, "Commlink") {
        return None;
    }
    let equips = merged_refs(base.get("equip"), option.get("equip"));
    let mut loadout = loadout_label(names, &weapons);
    if loadout.is_empty() {
        loadout = option
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Profil")
            .to_string();
    }
    if !used_loadouts.insert(loadout.clone()) {
        let option_skills = merged_refs(option.get("skills"), None);
        let option_equips = merged_refs(option.get("equip"), None);
        let extra = [
            labeled_list(names, &option_skills, true),
            labeled_list(names, &option_equips, false),
        ]
        .into_iter()
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
        let distinguished = if extra.is_empty() {
            format!("{loadout} #{option_id}")
        } else {
            format!("{loadout} ({extra})")
        };
        loadout = if used_loadouts.insert(distinguished.clone()) {
            distinguished
        } else {
            format!("{loadout} #{option_id}")
        };
    }
    let size = base
        .get("s")
        .and_then(Value::as_i64)
        .map(|value| value.to_string());
    let kind = silhouette_kind_from_parts(names, &skills, &equips);
    let traits = collect_traits(names, base, &weapons, &skills, &equips);
    let description = description_from_traits(&traits);
    let marker = marker_name(kind, size.as_deref());
    let operator = name_words(isc).iter().any(|word| word == "operator");
    let escape_pilot = skills.iter().any(|skill| {
        names
            .skills
            .get(&skill.id)
            .is_some_and(|name| name.eq_ignore_ascii_case("transmutation"))
            && skill.extras.iter().any(|extra| {
                names
                    .extras
                    .get(extra)
                    .is_some_and(|name| name.to_ascii_lowercase().contains("escape system"))
            })
    });
    let catalog = catalog_army(army, armies);
    Some(ProfileDraft {
        unit_id,
        group_id,
        option_id,
        army_id: Some(catalog.id),
        faction_slug: Some(catalog.slug.clone()),
        faction_label: catalog.name.clone(),
        isc: isc.to_string(),
        loadout,
        nom_tts: String::new(),
        description_tts: description,
        silhouette_size: size,
        silhouette_type: Some(kind.to_string()),
        marker_name: Some(marker),
        mesh_url: None,
        marker_mesh_url: None,
        traits,
        forms: extra_forms(names, unit, group, option, isc),
        sectorials: BTreeSet::from([army.id]),
        peripherals: peripheral_names(names, option, base),
        escape_pilot,
        operator,
    })
}

fn peripheral_names(names: &Names, option: &Value, base: &Value) -> Vec<(String, i64)> {
    let mut found: Vec<(String, i64)> = Vec::new();
    for source in [option.get("peripheral"), base.get("peripheral")] {
        let Some(items) = source.and_then(Value::as_array) else {
            continue;
        };
        for item in items {
            let Some(id) = item.get("id").and_then(Value::as_i64) else {
                continue;
            };
            let Some(name) = names.peripherals.get(&id) else {
                continue;
            };
            let quantity = item.get("q").and_then(Value::as_i64).unwrap_or(1).max(1);
            if let Some((_, existing)) = found
                .iter_mut()
                .find(|(label, _)| label.eq_ignore_ascii_case(name))
            {
                *existing = (*existing).max(quantity);
            } else {
                found.push((name.clone(), quantity));
            }
        }
    }
    found
}

fn peripheral_suffix(peripherals: &[(String, i64)]) -> String {
    peripherals
        .iter()
        .map(|(name, quantity)| {
            if *quantity > 1 {
                format!("{name} x{quantity}")
            } else {
                name.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" || ")
}

fn apply_peripheral_names(profiles: &mut [ProfileDraft]) {
    for profile in profiles {
        if profile.peripherals.is_empty() {
            continue;
        }
        let base_isc = profile.isc.clone();
        let base_loadout = profile.loadout.clone();
        let suffix = peripheral_suffix(&profile.peripherals);
        if !suffix.is_empty()
            && !profile
                .loadout
                .to_ascii_lowercase()
                .contains(&suffix.to_ascii_lowercase())
        {
            profile.loadout = append_pipe_suffix(&profile.loadout, &suffix);
        }
        let (isc, loadout) = shift_pipe_to_loadout(&profile.isc, &profile.loadout);
        profile.isc = isc;
        profile.loadout = loadout;
        for (name, quantity) in &profile.peripherals {
            add_equipment(&mut profile.traits, name, *quantity);
        }
        profile.description_tts = description_from_traits(&profile.traits);
        let loadout = profile.loadout.clone();
        let peripherals = profile.peripherals.clone();
        for form in &mut profile.forms {
            if form.isc != base_isc || form.loadout != base_loadout {
                continue;
            }
            form.loadout = loadout.clone();
            for (name, quantity) in &peripherals {
                add_equipment(&mut form.traits, name, *quantity);
            }
            form.description_tts = description_from_traits(&form.traits);
        }
    }
}

fn append_pipe_suffix(loadout: &str, suffix: &str) -> String {
    let loadout = loadout.trim();
    if loadout.is_empty() {
        suffix.to_string()
    } else {
        format!("{loadout} || {suffix}")
    }
}

fn shift_pipe_to_loadout(isc: &str, loadout: &str) -> (String, String) {
    let mut pieces = isc.split("||");
    let base = pieces.next().unwrap_or("").trim().to_string();
    let mut loadout = loadout.trim().to_string();
    for extra in pieces {
        let extra = extra.trim();
        if extra.is_empty()
            || loadout
                .to_ascii_lowercase()
                .contains(&extra.to_ascii_lowercase())
        {
            continue;
        }
        loadout = append_pipe_suffix(&loadout, extra);
    }
    (base, loadout)
}

fn place_pipe_suffix(isc: &str, loadout: &str) -> (String, String, Vec<TraitDraft>) {
    let mut traits = pipe_equipment_traits(isc);
    for item in pipe_equipment(loadout) {
        add_equipment(&mut traits, &item, 1);
    }
    let (isc, loadout) = shift_pipe_to_loadout(isc, loadout);
    (isc, loadout, traits)
}

fn add_equipment(traits: &mut Vec<TraitDraft>, name: &str, quantity: i64) {
    let name = name.trim();
    if name.is_empty() {
        return;
    }
    if traits
        .iter()
        .any(|trait_| trait_.kind == "equipment" && trait_.name.eq_ignore_ascii_case(name))
    {
        return;
    }
    traits.push(TraitDraft {
        kind: "equipment",
        name: name.to_string(),
        detail: (quantity > 1).then(|| format!("x{quantity}")),
    });
}

fn pipe_equipment_traits(name: &str) -> Vec<TraitDraft> {
    let mut traits = Vec::new();
    for item in pipe_equipment(name) {
        add_equipment(&mut traits, &item, 1);
    }
    traits
}

fn equipment_label(part: &str) -> String {
    let part = part.trim();
    if let Some(split) = part.rfind(" x") {
        let quantity = &part[split + 2..];
        if !quantity.is_empty() && quantity.chars().all(|c| c.is_ascii_digit()) {
            return part[..split].trim().to_string();
        }
    }
    part.to_string()
}

fn pipe_equipment(name: &str) -> Vec<String> {
    name.split("||")
        .skip(1)
        .flat_map(|part| part.split(','))
        .map(equipment_label)
        .filter(|part| !part.is_empty())
        .collect()
}

fn merge_figurine_suffix(name: &str, mesh_name: &str) -> String {
    let mut result = name.to_string();
    for extra in mesh_name.split("||").skip(1) {
        let extra = extra.trim();
        if extra.is_empty()
            || result
                .to_ascii_lowercase()
                .contains(&extra.to_ascii_lowercase())
        {
            continue;
        }
        result = format!("{result} || {extra}");
    }
    result
}

fn base_figurine_name(value: &str) -> &str {
    value.split("||").next().unwrap_or(value).trim()
}

fn extra_forms(
    names: &Names,
    unit: &Value,
    group: &Value,
    option: &Value,
    parent_isc: &str,
) -> Vec<FormDraft> {
    let mut forms = Vec::new();
    let current_group = group.get("id").and_then(Value::as_i64);
    if let Some(profiles) = group.get("profiles").and_then(Value::as_array) {
        for (index, profile) in profiles.iter().enumerate().skip(1) {
            // Transmutation / 2ᵉ profil du groupe : nom = « Unité COMBAT FORM ».
            if let Some(form) =
                form_from_profile(names, profile, option, index as i64, Some(parent_isc))
            {
                forms.push(form);
            }
        }
    }
    let Some(groups) = unit.get("profileGroups").and_then(Value::as_array) else {
        return forms;
    };
    let Some(includes) = option.get("includes").and_then(Value::as_array) else {
        return forms;
    };
    for include in includes {
        let group_id = include.get("group").and_then(Value::as_i64);
        if group_id.is_none() || group_id == current_group {
            continue;
        }
        let Some(target_group) = groups
            .iter()
            .find(|candidate| candidate.get("id").and_then(Value::as_i64) == group_id)
        else {
            continue;
        };
        let option_id = include.get("option").and_then(Value::as_i64);
        let Some(target_option) = target_group
            .get("options")
            .and_then(Value::as_array)
            .and_then(|options| {
                options
                    .iter()
                    .find(|candidate| candidate.get("id").and_then(Value::as_i64) == option_id)
            })
        else {
            continue;
        };
        let Some(profile) = target_group
            .get("profiles")
            .and_then(Value::as_array)
            .and_then(|profiles| profiles.first())
        else {
            continue;
        };
        // Périphérique / coéquipier : garder le nom propre (Auxbot, Operator…).
        if let Some(form) = form_from_profile(names, profile, target_option, 900, None) {
            forms.push(form);
        }
    }
    forms
}

/// Préfixe le nom parent pour les formes de transmute (« Chernobog… BATTLE RAVAGED »).
pub(crate) fn qualify_form_isc(parent_isc: &str, form_name: &str) -> String {
    let parent = parent_isc.trim();
    let form = form_name.trim();
    if form.is_empty() {
        return parent.to_string();
    }
    if parent.is_empty() || form.eq_ignore_ascii_case(parent) {
        return form.to_string();
    }
    let parent_lower = parent.to_ascii_lowercase();
    let form_lower = form.to_ascii_lowercase();
    if form_lower.starts_with(&parent_lower)
        || form_lower.starts_with(&format!("{parent_lower} "))
    {
        return form.to_string();
    }
    format!("{parent} {form}")
}

fn form_from_profile(
    names: &Names,
    profile: &Value,
    option: &Value,
    state_index: i64,
    parent_isc: Option<&str>,
) -> Option<FormDraft> {
    let form_name = profile
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("Profil")
        .trim()
        .to_string();
    let isc = match parent_isc {
        Some(parent) => qualify_form_isc(parent, &form_name),
        None => form_name,
    };
    let weapons = merged_refs(option.get("weapons"), profile.get("weapons"));
    let skills = merged_refs(profile.get("skills"), option.get("skills"));
    let equips = merged_refs(profile.get("equip"), option.get("equip"));
    let mut loadout = loadout_label(names, &weapons);
    if loadout.is_empty() {
        loadout = option
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Profil")
            .to_string();
    }
    let size = profile
        .get("s")
        .and_then(Value::as_i64)
        .map(|value| value.to_string());
    let kind = silhouette_kind_from_parts(names, &skills, &equips);
    let traits = collect_traits(names, profile, &weapons, &skills, &equips);
    Some(FormDraft {
        state_index,
        isc,
        loadout,
        nom_tts: String::new(),
        description_tts: description_from_traits(&traits),
        silhouette_size: size.clone(),
        silhouette_type: Some(kind.to_string()),
        marker_name: Some(marker_name(kind, size.as_deref())),
        mesh_url: None,
        marker_mesh_url: None,
        traits,
    })
}

struct RefItem {
    id: i64,
    extras: Vec<i64>,
}

fn merged_refs(first: Option<&Value>, second: Option<&Value>) -> Vec<RefItem> {
    let mut items = Vec::new();
    for source in [first, second].into_iter().flatten() {
        let Some(list) = source.as_array() else {
            continue;
        };
        for item in list {
            let Some(id) = item.get("id").and_then(Value::as_i64) else {
                continue;
            };
            let extras = item
                .get("extra")
                .and_then(Value::as_array)
                .map(|values| values.iter().filter_map(Value::as_i64).collect())
                .unwrap_or_default();
            items.push(RefItem { id, extras });
        }
    }
    items
}

fn loadout_label(names: &Names, weapons: &[RefItem]) -> String {
    let labels: Vec<String> = weapons
        .iter()
        .filter_map(|weapon| names.weapons.get(&weapon.id).map(|name| (name, weapon)))
        .filter(|(name, _)| {
            let lower = name.to_ascii_lowercase();
            lower != "cc weapon" && lower != "pistol"
        })
        .map(|(name, weapon)| format_ref(name, names, &weapon.extras))
        .collect();
    if !labels.is_empty() {
        return labels.join(", ");
    }
    weapons
        .iter()
        .filter_map(|weapon| {
            names
                .weapons
                .get(&weapon.id)
                .map(|name| format_ref(name, names, &weapon.extras))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn labeled_list(names: &Names, items: &[RefItem], skills: bool) -> String {
    items
        .iter()
        .filter_map(|item| {
            let catalog = if skills { &names.skills } else { &names.equips };
            catalog
                .get(&item.id)
                .map(|name| format_ref(name, names, &item.extras))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_ref(name: &str, names: &Names, extras: &[i64]) -> String {
    let extra_names: Vec<&str> = extras
        .iter()
        .filter_map(|id| names.extras.get(id).map(String::as_str))
        .collect();
    if extra_names.is_empty() {
        name.to_string()
    } else {
        format!("{name}({})", extra_names.join(", "))
    }
}

fn collect_traits(
    names: &Names,
    base: &Value,
    weapons: &[RefItem],
    skills: &[RefItem],
    equips: &[RefItem],
) -> Vec<TraitDraft> {
    let mut traits = Vec::new();
    if let Some(type_id) = base.get("type").and_then(Value::as_i64) {
        if let Some(name) = names.types.get(&type_id) {
            traits.push(TraitDraft {
                kind: "char",
                name: name.clone(),
                detail: None,
            });
        }
    }
    if let Some(chars) = base.get("chars").and_then(Value::as_array) {
        for char_id in chars.iter().filter_map(Value::as_i64) {
            if let Some(name) = names.chars.get(&char_id) {
                traits.push(TraitDraft {
                    kind: "char",
                    name: name.clone(),
                    detail: None,
                });
            }
        }
    }
    let move_value = base
        .get("move")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_i64)
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join("-")
        })
        .filter(|value| !value.is_empty());
    push_attr(&mut traits, "MOV", move_value);
    push_attr(&mut traits, "CC", json_number(base.get("cc")));
    push_attr(&mut traits, "BS", json_number(base.get("bs")));
    push_attr(&mut traits, "PH", json_number(base.get("ph")));
    push_attr(&mut traits, "WIP", json_number(base.get("wip")));
    push_attr(&mut traits, "ARM", json_number(base.get("arm")));
    push_attr(&mut traits, "BTS", json_number(base.get("bts")));
    let wounds = if base.get("str").and_then(Value::as_bool).unwrap_or(false) {
        "STR"
    } else {
        "W"
    };
    push_attr(&mut traits, wounds, json_number(base.get("w")));
    push_attr(&mut traits, "S", json_number(base.get("s")));
    for weapon in weapons {
        if let Some(name) = names.weapons.get(&weapon.id) {
            traits.push(TraitDraft {
                kind: "weapon",
                name: name.clone(),
                detail: extra_detail(names, &weapon.extras),
            });
        }
    }
    for equip in equips {
        if let Some(name) = names.equips.get(&equip.id) {
            traits.push(TraitDraft {
                kind: "equipment",
                name: name.clone(),
                detail: extra_detail(names, &equip.extras),
            });
        }
    }
    for skill in skills {
        if let Some(name) = names.skills.get(&skill.id) {
            traits.push(TraitDraft {
                kind: "skill",
                name: name.clone(),
                detail: extra_detail(names, &skill.extras),
            });
        }
    }
    traits
}

fn push_attr(traits: &mut Vec<TraitDraft>, name: &str, detail: Option<String>) {
    let Some(detail) = detail.filter(|value| !value.is_empty()) else {
        return;
    };
    traits.push(TraitDraft {
        kind: "attribute",
        name: name.to_string(),
        detail: Some(detail),
    });
}

fn json_number(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_i64)
        .map(|number| number.to_string())
}

fn extra_detail(names: &Names, extras: &[i64]) -> Option<String> {
    let text = extras
        .iter()
        .filter_map(|id| names.extras.get(id).cloned())
        .collect::<Vec<_>>()
        .join(", ");
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn description_from_traits(traits: &[TraitDraft]) -> String {
    let mut lines = Vec::new();
    let chars = traits
        .iter()
        .filter(|trait_| trait_.kind == "char")
        .map(|trait_| trait_.name.clone())
        .collect::<Vec<_>>()
        .join(" ● ");
    if !chars.is_empty() {
        lines.push(chars);
    }
    for trait_ in traits.iter().filter(|trait_| trait_.kind == "attribute") {
        let value = trait_.detail.clone().unwrap_or_default();
        if trait_.name == "S" {
            lines.push(format!("[B]S[/B]: {value}"));
        } else {
            lines.push(format!("[b]{}[/b]: {value}", trait_.name));
        }
    }
    for (kind, title) in [
        ("weapon", "Weapons"),
        ("equipment", "Equipment"),
        ("skill", "Skills"),
    ] {
        let parts = traits
            .iter()
            .filter(|trait_| trait_.kind == kind)
            .map(|trait_| match &trait_.detail {
                Some(detail) => format!("{}({detail})", trait_.name),
                None => trait_.name.clone(),
            })
            .collect::<Vec<_>>()
            .join(" ● ");
        if !parts.is_empty() {
            lines.push(format!("----------{title}---------"));
            lines.push(parts);
        }
    }
    lines.join("\n")
}

fn silhouette_kind_from_parts(
    names: &Names,
    skills: &[RefItem],
    equips: &[RefItem],
) -> &'static str {
    let skill_text = |item: &RefItem| {
        names.skills.get(&item.id).map(|name| {
            format_ref(name, names, &item.extras)
                .chars()
                .filter(|c| !c.is_whitespace())
                .flat_map(|c| c.to_lowercase())
                .collect::<String>()
        })
    };
    let skills: Vec<String> = skills.iter().filter_map(skill_text).collect();
    let equips: Vec<String> = equips
        .iter()
        .filter_map(|item| {
            names.equips.get(&item.id).map(|name| {
                format_ref(name, names, &item.extras)
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .flat_map(|c| c.to_lowercase())
                    .collect::<String>()
            })
        })
        .collect();
    let camo = skills.iter().any(|token| token.starts_with("camouflage"));
    let mimetism = skills.iter().any(|token| token.starts_with("mimetism"));
    let mimetism_3 = skills.iter().any(|token| token.contains("mimetism(-3)"));
    let mimetism_6 = skills.iter().any(|token| token.contains("mimetism(-6)"));
    let impersonation = skills
        .iter()
        .any(|token| token.starts_with("impersonation"));
    let impersonation_l2 = skills.iter().any(|token| {
        token.starts_with("impersonation") && (token.contains("imp-2") || token.contains("l2"))
    });
    let devices = equips.iter().any(|token| {
        token.contains("killerhackingdevice")
            || token.contains("hackingdeviceplus")
            || token.contains("cybermask")
    }) || skills.iter().any(|token| token.contains("cybermask"));
    if camo && mimetism_6 {
        return "camo-6";
    }
    if camo && mimetism_3 {
        return "camo-3";
    }
    if camo && !mimetism {
        return "camo-0";
    }
    if impersonation_l2 || devices {
        return "imp-2";
    }
    if impersonation {
        return "imp-1";
    }
    "normale"
}

fn marker_name(kind: &str, size: Option<&str>) -> String {
    let size = size.unwrap_or("2");
    match kind {
        "camo-0" => format!("Camouflage (0) S{size}"),
        "camo-3" => format!("Camouflage (-3) S{size}"),
        "camo-6" => format!("Camouflage (-6) S{size}"),
        "imp-1" => "IMP-1".to_string(),
        "imp-2" => "IMP-2".to_string(),
        _ => format!("Silhouette {size}"),
    }
}

fn catalog_army<'a>(army: &'a ArmyRow, armies: &'a [ArmyRow]) -> &'a ArmyRow {
    if army.parent_id == army.id {
        return army;
    }
    armies
        .iter()
        .find(|candidate| candidate.id == army.parent_id)
        .unwrap_or(army)
}

fn name_words(value: &str) -> Vec<String> {
    value
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ä' | 'ã' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            other => other.to_ascii_lowercase(),
        })
        .collect::<String>()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| word.len() >= 2 && *word != "the" && *word != "of")
        .map(str::to_string)
        .collect()
}

fn attach_sapper_forms(profiles: &mut [ProfileDraft]) {
    for profile in profiles.iter_mut() {
        let sapper = profile
            .traits
            .iter()
            .any(|trait_| trait_.kind == "skill" && trait_.name.eq_ignore_ascii_case("sapper"));
        if !sapper {
            continue;
        }
        let size = profile
            .silhouette_size
            .as_deref()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(2)
            .max(4)
            .to_string();
        let mut traits = profile.traits.clone();
        for trait_ in &mut traits {
            if trait_.kind == "attribute" && trait_.name == "S" {
                trait_.detail = Some(size.clone());
            }
        }
        let description = description_from_traits(&traits);
        profile.forms.push(FormDraft {
            state_index: 1000,
            isc: profile.isc.clone(),
            loadout: profile.loadout.clone(),
            nom_tts: String::new(),
            description_tts: description,
            silhouette_size: Some(size.clone()),
            silhouette_type: profile.silhouette_type.clone(),
            marker_name: Some(format!("Silhouette {size}")),
            mesh_url: None,
            marker_mesh_url: None,
            traits,
        });
    }
}

fn has_ai_motorcycle(profile: &ProfileDraft) -> bool {
    profile.traits.iter().any(|trait_| {
        trait_.kind == "equipment" && trait_.name.eq_ignore_ascii_case("AI Motorcycle")
    })
}

fn motorcycle_dismount_isc(mounted_isc: &str, dismounted_isc: &str) -> String {
    let mounted = mounted_isc.trim();
    let dismounted = dismounted_isc.trim();
    if dismounted.eq_ignore_ascii_case(mounted) {
        qualify_form_isc(mounted, "Dismounted")
    } else {
        dismounted.to_string()
    }
}

fn profile_key(profile: &ProfileDraft) -> (i64, i64, i64) {
    (profile.unit_id, profile.group_id, profile.option_id)
}

/// Zondnautica / AI Motorcycle : l'option d'unité référence monté + dismounted.
/// Le dismounted (et son Zondmate) devient forme du monté ; le slot table reste le S4.
fn attach_ai_motorcycle_forms(
    profiles: &mut Vec<ProfileDraft>,
    unit_includes: &mut BTreeMap<(i64, i64), Vec<UnitOptionInclude>>,
) {
    let index_by_key: BTreeMap<(i64, i64, i64), usize> = profiles
        .iter()
        .enumerate()
        .map(|(index, profile)| (profile_key(profile), index))
        .collect();
    let mut consumed = vec![false; profiles.len()];
    let mut attachments: Vec<(usize, FormDraft, Vec<FormDraft>)> = Vec::new();

    for includes in unit_includes.values() {
        if includes.len() < 2 {
            continue;
        }
        let mut mounted_index = None;
        let mut other_indices = Vec::new();
        for include in includes {
            let key = (include.unit_id, include.group_id, include.option_id);
            let Some(&index) = index_by_key.get(&key) else {
                continue;
            };
            if consumed[index] {
                continue;
            }
            if has_ai_motorcycle(&profiles[index]) {
                if mounted_index.is_some() {
                    mounted_index = None;
                    break;
                }
                mounted_index = Some(index);
            } else {
                other_indices.push(index);
            }
        }
        let Some(mounted) = mounted_index else {
            continue;
        };
        if other_indices.is_empty() {
            continue;
        }
        for other in other_indices {
            if consumed[other] {
                continue;
            }
            let dismounted = &profiles[other];
            let form = FormDraft {
                state_index: 2,
                isc: motorcycle_dismount_isc(&profiles[mounted].isc, &dismounted.isc),
                loadout: dismounted.loadout.clone(),
                nom_tts: String::new(),
                description_tts: dismounted.description_tts.clone(),
                silhouette_size: dismounted.silhouette_size.clone(),
                silhouette_type: dismounted.silhouette_type.clone(),
                marker_name: dismounted.marker_name.clone(),
                mesh_url: None,
                marker_mesh_url: None,
                traits: dismounted.traits.clone(),
            };
            let nested = dismounted.forms.clone();
            attachments.push((mounted, form, nested));
            consumed[other] = true;
        }
    }

    for (mounted, form, nested) in attachments {
        let already = profiles[mounted].forms.iter().any(|existing| {
            existing.isc == form.isc
                && existing.loadout == form.loadout
                && existing.silhouette_size == form.silhouette_size
        });
        if !already {
            profiles[mounted].forms.push(form);
        }
        for nested_form in nested {
            let already = profiles[mounted].forms.iter().any(|existing| {
                existing.isc == nested_form.isc
                    && existing.loadout == nested_form.loadout
                    && existing.state_index == nested_form.state_index
            });
            if !already {
                profiles[mounted].forms.push(nested_form);
            }
        }
    }

    // Les includes d'unité ne gardent que le profil monté (déploiement table).
    for includes in unit_includes.values_mut() {
        includes.retain(|include| {
            let key = (include.unit_id, include.group_id, include.option_id);
            index_by_key
                .get(&key)
                .is_none_or(|&index| !consumed[index])
        });
        for (sort_order, include) in includes.iter_mut().enumerate() {
            include.sort_order = sort_order as i64;
        }
    }

    let mut index = 0;
    profiles.retain(|_| {
        let keep = !consumed[index];
        index += 1;
        keep
    });
}

fn attach_escape_pilots(profiles: &mut Vec<ProfileDraft>) {
    let factions: BTreeSet<String> = profiles
        .iter()
        .filter_map(|profile| profile.faction_slug.clone())
        .collect();
    let mut consumed = vec![false; profiles.len()];
    for faction in factions {
        pair_escape_faction(profiles, &mut consumed, &faction);
    }
    let mut index = 0;
    profiles.retain(|_| {
        let keep = !consumed[index];
        index += 1;
        keep
    });
}

fn pair_escape_faction(profiles: &mut [ProfileDraft], consumed: &mut [bool], faction: &str) {
    let vehicles: Vec<usize> = profiles
        .iter()
        .enumerate()
        .filter(|(_, profile)| {
            profile.escape_pilot && profile.faction_slug.as_deref() == Some(faction)
        })
        .map(|(index, _)| index)
        .collect();
    let operators: Vec<usize> = profiles
        .iter()
        .enumerate()
        .filter(|(_, profile)| {
            profile.operator
                && !profile.escape_pilot
                && profile.faction_slug.as_deref() == Some(faction)
        })
        .map(|(index, _)| index)
        .collect();
    if vehicles.is_empty() || operators.is_empty() {
        return;
    }
    let mut piloted = vec![false; profiles.len()];
    let mut attached_to: Vec<(String, String, usize)> = Vec::new();
    let specific: Vec<usize> = operators
        .iter()
        .copied()
        .filter(|index| !operator_keys(&profiles[*index].isc).is_empty())
        .collect();
    let specific_vehicles: Vec<usize> = vehicles
        .iter()
        .copied()
        .filter(|vehicle| {
            specific.iter().any(|operator| {
                let keys = operator_keys(&profiles[*operator].isc);
                keys.iter().all(|key| {
                    name_words(&profiles[*vehicle].isc)
                        .iter()
                        .any(|word| word == key)
                })
            })
        })
        .collect();
    for (operator, vehicle) in greedy_pairs(profiles, &specific, &specific_vehicles) {
        attached_to.push((
            profiles[operator].isc.to_lowercase(),
            profiles[operator].loadout.to_lowercase(),
            vehicle,
        ));
        push_pilot(profiles, vehicle, operator);
        consumed[operator] = true;
        piloted[vehicle] = true;
    }
    let bare: Vec<usize> = operators
        .iter()
        .copied()
        .filter(|index| !consumed[*index] && operator_keys(&profiles[*index].isc).is_empty())
        .collect();
    let unmatched: Vec<usize> = vehicles
        .iter()
        .copied()
        .filter(|index| !piloted[*index])
        .collect();
    if bare.len() == 1 && !unmatched.is_empty() {
        let operator = bare[0];
        for vehicle in unmatched {
            attached_to.push((
                profiles[operator].isc.to_lowercase(),
                profiles[operator].loadout.to_lowercase(),
                vehicle,
            ));
            push_pilot(profiles, vehicle, operator);
        }
        consumed[operator] = true;
    } else if !bare.is_empty() && !unmatched.is_empty() {
        for (operator, vehicle) in greedy_pairs(profiles, &bare, &unmatched) {
            attached_to.push((
                profiles[operator].isc.to_lowercase(),
                profiles[operator].loadout.to_lowercase(),
                vehicle,
            ));
            push_pilot(profiles, vehicle, operator);
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
        let targets: Vec<usize> = attached_to
            .iter()
            .filter(|(isc, loadout, _)| *isc == key.0 && *loadout == key.1)
            .map(|(_, _, vehicle)| *vehicle)
            .collect();
        if targets.is_empty() {
            continue;
        }
        for vehicle in targets {
            push_pilot(profiles, vehicle, operator);
        }
        consumed[operator] = true;
    }
}

fn greedy_pairs(
    profiles: &[ProfileDraft],
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
            operator_keys(&profiles[right.1].isc)
                .len()
                .cmp(&operator_keys(&profiles[left.1].isc).len()),
        )
    });
    let mut used_operators = BTreeSet::new();
    let mut used_vehicles = BTreeSet::new();
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

fn word_overlap(operator: &ProfileDraft, vehicle: &ProfileDraft) -> usize {
    let left = draft_words(operator, true);
    let right = draft_words(vehicle, false);
    left.intersection(&right).count()
}

fn draft_words(profile: &ProfileDraft, drop_operator: bool) -> BTreeSet<String> {
    let mut words = BTreeSet::new();
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

fn operator_keys(isc: &str) -> Vec<String> {
    name_words(isc)
        .into_iter()
        .filter(|word| word != "operator")
        .collect()
}

fn push_pilot(profiles: &mut [ProfileDraft], vehicle: usize, operator: usize) {
    let form = FormDraft {
        state_index: 900,
        isc: profiles[operator].isc.clone(),
        loadout: profiles[operator].loadout.clone(),
        nom_tts: String::new(),
        description_tts: profiles[operator].description_tts.clone(),
        silhouette_size: profiles[operator].silhouette_size.clone(),
        silhouette_type: profiles[operator].silhouette_type.clone(),
        marker_name: profiles[operator].marker_name.clone(),
        mesh_url: None,
        marker_mesh_url: None,
        traits: profiles[operator].traits.clone(),
    };
    let already = profiles[vehicle].forms.iter().any(|existing| {
        existing.isc == form.isc
            && existing.loadout == form.loadout
            && existing.description_tts == form.description_tts
    });
    if !already {
        profiles[vehicle].forms.push(form);
    }
}

fn apply_meshes_and_colors(
    profiles: &mut [ProfileDraft],
    meshes: &[MeshHit],
    colors: &HashMap<String, String>,
) {
    for profile in profiles {
        let color = profile
            .faction_slug
            .as_deref()
            .and_then(|slug| colors.get(slug))
            .map(String::as_str)
            .unwrap_or("FFFFFF");
        if let Some(mesh) = find_mesh(meshes, &profile.isc, &profile.loadout) {
            profile.mesh_url = mesh.mesh_url.clone();
            profile.marker_mesh_url = mesh.marker_mesh_url.clone();
            if profile.marker_name.is_none() {
                profile.marker_name = mesh.marker_name.clone();
            }
            let before = profile.loadout.clone();
            let mesh_isc = mesh.isc.clone();
            let mesh_loadout = mesh.loadout.clone();
            profile.loadout = merge_figurine_suffix(&profile.loadout, &mesh_isc);
            profile.loadout = merge_figurine_suffix(&profile.loadout, &mesh_loadout);
            if profile.loadout != before {
                let added = pipe_equipment(&profile.loadout)
                    .into_iter()
                    .filter(|item| {
                        !before
                            .to_ascii_lowercase()
                            .contains(&item.to_ascii_lowercase())
                    })
                    .collect::<Vec<_>>();
                for item in &added {
                    add_equipment(&mut profile.traits, item, 1);
                }
                profile.description_tts = description_from_traits(&profile.traits);
                let loadout = profile.loadout.clone();
                let isc = profile.isc.clone();
                for form in &mut profile.forms {
                    if form.isc != isc || form.loadout != before {
                        continue;
                    }
                    form.loadout = loadout.clone();
                    for item in &added {
                        add_equipment(&mut form.traits, item, 1);
                    }
                    form.description_tts = description_from_traits(&form.traits);
                }
            }
        }
        profile.nom_tts = format!("[{color}]{}[-] {}", profile.isc, profile.loadout);
        for form in &mut profile.forms {
            if let Some(mesh) = find_mesh(meshes, &form.isc, &form.loadout) {
                form.mesh_url = mesh.mesh_url.clone();
                form.marker_mesh_url = mesh.marker_mesh_url.clone();
                let before = form.loadout.clone();
                let mesh_isc = mesh.isc.clone();
                let mesh_loadout = mesh.loadout.clone();
                form.loadout = merge_figurine_suffix(&form.loadout, &mesh_isc);
                form.loadout = merge_figurine_suffix(&form.loadout, &mesh_loadout);
                if form.loadout != before {
                    let added = pipe_equipment(&form.loadout)
                        .into_iter()
                        .filter(|item| {
                            !before
                                .to_ascii_lowercase()
                                .contains(&item.to_ascii_lowercase())
                        })
                        .collect::<Vec<_>>();
                    for item in &added {
                        add_equipment(&mut form.traits, item, 1);
                    }
                    form.description_tts = description_from_traits(&form.traits);
                }
            }
            let suffix = if form.state_index >= 1000 {
                " (sapper)"
            } else {
                ""
            };
            form.nom_tts = format!("[{color}]{}[-] {}{suffix}", form.isc, form.loadout);
        }
    }
}

fn find_mesh<'a>(meshes: &'a [MeshHit], isc: &str, loadout: &str) -> Option<&'a MeshHit> {
    let isc_key = fold_key(base_figurine_name(isc));
    let loadout_key = fold_key(base_figurine_name(loadout));
    meshes
        .iter()
        .find(|mesh| fold_key(&mesh.isc) == isc_key && fold_key(&mesh.loadout) == loadout_key)
        .or_else(|| {
            meshes.iter().find(|mesh| {
                let mesh_isc = fold_key(base_figurine_name(&mesh.isc));
                let mesh_loadout = fold_key(base_figurine_name(&mesh.loadout));
                (mesh_isc.contains(&isc_key) || isc_key.contains(&mesh_isc))
                    && !loadout_key.is_empty()
                    && (mesh_loadout.contains(&loadout_key) || loadout_key.contains(&mesh_loadout))
            })
        })
}

fn fold_key(value: &str) -> String {
    name_words(value).join(" ")
}

fn snapshot_meshes(conn: &Connection, mesh_file: Option<&Path>) -> Result<Vec<MeshHit>> {
    let mut hits = Vec::new();
    if let Some(path) = mesh_file.filter(|path| path.exists()) {
        let parsed = parse_models_json(&std::fs::read_to_string(path)?)?;
        for profile in parsed {
            hits.push(MeshHit {
                isc: profile.isc,
                loadout: profile.loadout,
                mesh_url: profile.mesh_url,
                marker_mesh_url: profile.marker_mesh_url,
                marker_name: profile.marker_name,
            });
        }
        return Ok(hits);
    }
    let mut stmt = conn.prepare(
        "
        SELECT u.nom, p.loadout, l.mesh_url, l.marker_mesh_url, l.marker_name
        FROM tts_profiles p
        JOIN tts_units u ON u.id = p.unit_id
        JOIN tts_loadouts l ON l.id = p.tts_loadout_id
        WHERE l.mesh_url IS NOT NULL AND p.parent_profile_id IS NULL
        ",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(MeshHit {
            isc: row.get(0)?,
            loadout: row.get(1)?,
            mesh_url: row.get(2)?,
            marker_mesh_url: row.get(3)?,
            marker_name: row.get(4)?,
        })
    })?;
    for row in rows {
        hits.push(row?);
    }
    Ok(hits)
}

fn snapshot_colors(conn: &Connection) -> Result<HashMap<String, String>> {
    let mut colors = HashMap::new();
    let mut stmt = conn.prepare(
        "
        SELECT p.faction_slug, l.nom_tts
        FROM tts_profiles p
        JOIN tts_loadouts l ON l.id = p.tts_loadout_id
        WHERE p.faction_slug IS NOT NULL AND l.nom_tts LIKE '[______]%'
          AND p.parent_profile_id IS NULL
        ",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (slug, nom) = row?;
        let color: String = nom.chars().skip(1).take(6).collect();
        if color.len() == 6 && color.chars().all(|c| c.is_ascii_hexdigit()) {
            colors.entry(slug).or_insert(color);
        }
    }
    Ok(colors)
}

fn has_lieutenant(traits: &[TraitDraft]) -> bool {
    traits.iter().any(|t| {
        t.kind.eq_ignore_ascii_case("char") && t.name.trim().eq_ignore_ascii_case("Lieutenant")
    })
}

fn traits_without_lieutenant(traits: &[TraitDraft]) -> Vec<TraitDraft> {
    traits
        .iter()
        .filter(|t| {
            !(t.kind.eq_ignore_ascii_case("char")
                && t.name.trim().eq_ignore_ascii_case("Lieutenant"))
        })
        .cloned()
        .collect()
}

/// Clé de partage loadout TTS : unité + groupe + loadout + silhouette, hors Lieutenant.
fn tts_share_key(profile: &ProfileDraft) -> (i64, i64, String, String, String) {
    (
        profile.unit_id,
        profile.group_id,
        profile.loadout.trim().to_ascii_lowercase(),
        profile
            .silhouette_size
            .as_deref()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase(),
        profile
            .silhouette_type
            .as_deref()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase(),
    )
}

fn ensure_unit(conn: &Connection, nom: &str) -> Result<i64> {
    let nom = nom.trim();
    if let Ok(id) = conn.query_row(
        "SELECT id FROM tts_units WHERE nom = ?1",
        params![nom],
        |row| row.get::<_, i64>(0),
    ) {
        return Ok(id);
    }
    conn.execute("INSERT INTO tts_units (nom) VALUES (?1)", params![nom])?;
    Ok(conn.last_insert_rowid())
}

fn insert_loadout(
    conn: &Connection,
    nom_tts: &str,
    description_tts: &str,
    silhouette_size: &Option<String>,
    silhouette_type: &Option<String>,
    marker_name: &Option<String>,
    mesh_url: &Option<String>,
    marker_mesh_url: &Option<String>,
) -> Result<i64> {
    conn.execute(
        "
        INSERT INTO tts_loadouts (
            nom_tts, description_tts, silhouette_size, silhouette_type,
            marker_name, mesh_url, marker_mesh_url
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ",
        params![
            nom_tts,
            description_tts,
            silhouette_size,
            silhouette_type,
            marker_name,
            mesh_url,
            marker_mesh_url,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

fn store_profiles(
    conn: &Connection,
    profiles: &[ProfileDraft],
    includes: &[UnitOptionInclude],
) -> Result<ArmyImportReport> {
    // Préserver les prefs utilisateur (user_id, figurine_id) avant wipe des loadouts.
    let prefs_snapshot: Vec<(i64, i64, i64)> = {
        let mut stmt = conn.prepare(
            "
            SELECT user_id, figurine_id, updated_at
            FROM tts_figurine_pick_prefs
            ",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap_or_default();
        rows
    };

    let tx = conn.unchecked_transaction()?;
    // Détacher les figs avant wipe des loadouts (FK).
    tx.execute("UPDATE tts_figurines SET tts_loadout_id = NULL", [])?;
    tx.execute("DELETE FROM tts_figurine_pick_prefs", [])?;
    tx.execute("DELETE FROM tts_profile_traits", [])?;
    tx.execute("DELETE FROM tts_profile_sectorials", [])?;
    tx.execute("DELETE FROM tts_profiles", [])?;
    tx.execute("DELETE FROM tts_loadouts", [])?;
    tx.execute("DELETE FROM tts_army_unit_includes", [])?;
    // Garder tts_units : on réutilise / complète.

    for include in includes {
        tx.execute(
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

    // Pré-résoudre le partage Lt : sibling non-Lt fournit le loadout TTS.
    let mut share_owner: HashMap<(i64, i64, String, String, String), usize> = HashMap::new();
    for (index, profile) in profiles.iter().enumerate() {
        if has_lieutenant(&profile.traits) {
            continue;
        }
        share_owner
            .entry(tts_share_key(profile))
            .or_insert(index);
    }
    let mut shared_loadout_ids: HashMap<(i64, i64, String, String, String), i64> = HashMap::new();

    let mut forms = 0usize;
    let mut sectorials = 0usize;
    for profile in profiles {
        let db_unit_id = ensure_unit(&tx, &profile.isc)?;
        let key = tts_share_key(profile);
        let tts_loadout_id = if has_lieutenant(&profile.traits) {
            if let Some(&owner_idx) = share_owner.get(&key) {
                if let Some(&existing) = shared_loadout_ids.get(&key) {
                    existing
                } else {
                    // Créer depuis le sibling non-Lt si pas encore inséré.
                    let owner = &profiles[owner_idx];
                    let desc = description_from_traits(&traits_without_lieutenant(&owner.traits));
                    let id = insert_loadout(
                        &tx,
                        &owner.nom_tts,
                        &desc,
                        &owner.silhouette_size,
                        &owner.silhouette_type,
                        &owner.marker_name,
                        &owner.mesh_url,
                        &owner.marker_mesh_url,
                    )?;
                    shared_loadout_ids.insert(key.clone(), id);
                    id
                }
            } else {
                // Pas de sibling non-Lt : loadout propre sans Lt dans la description TTS.
                let desc = description_from_traits(&traits_without_lieutenant(&profile.traits));
                insert_loadout(
                    &tx,
                    &profile.nom_tts,
                    &desc,
                    &profile.silhouette_size,
                    &profile.silhouette_type,
                    &profile.marker_name,
                    &profile.mesh_url,
                    &profile.marker_mesh_url,
                )?
            }
        } else if let Some(&existing) = shared_loadout_ids.get(&key) {
            existing
        } else {
            let id = insert_loadout(
                &tx,
                &profile.nom_tts,
                &profile.description_tts,
                &profile.silhouette_size,
                &profile.silhouette_type,
                &profile.marker_name,
                &profile.mesh_url,
                &profile.marker_mesh_url,
            )?;
            shared_loadout_ids.insert(key, id);
            id
        };

        tx.execute(
            "
            INSERT INTO tts_profiles (
                army_id, faction_slug, faction_label, unit_id, tts_loadout_id,
                parent_profile_id, state_index, loadout, source_bag, source_index
            )
            VALUES (?1, ?2, ?3, ?4, ?5, NULL, 0, ?6, ?7, ?8)
            ",
            params![
                profile.army_id,
                profile.faction_slug,
                profile.faction_label,
                db_unit_id,
                tts_loadout_id,
                profile.loadout,
                format!("army:{}:{}", profile.unit_id, profile.group_id),
                profile.option_id,
            ],
        )?;
        let profile_id = tx.last_insert_rowid();
        insert_traits(&tx, profile_id, &profile.traits)?;

        for form in &profile.forms {
            let form_unit_id = ensure_unit(&tx, &form.isc)?;
            let form_loadout_id = insert_loadout(
                &tx,
                &form.nom_tts,
                &form.description_tts,
                &form.silhouette_size,
                &form.silhouette_type,
                &form.marker_name,
                &form.mesh_url,
                &form.marker_mesh_url,
            )?;
            tx.execute(
                "
                INSERT INTO tts_profiles (
                    army_id, faction_slug, faction_label, unit_id, tts_loadout_id,
                    parent_profile_id, state_index, loadout, source_bag, source_index
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, '', 0)
                ",
                params![
                    profile.army_id,
                    profile.faction_slug,
                    profile.faction_label,
                    form_unit_id,
                    form_loadout_id,
                    profile_id,
                    form.state_index,
                    form.loadout,
                ],
            )?;
            let child_id = tx.last_insert_rowid();
            insert_traits(&tx, child_id, &form.traits)?;
            forms += 1;
        }

        for army_id in &profile.sectorials {
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM armies WHERE id = ?1)",
                params![army_id],
                |row| row.get(0),
            )?;
            if !exists {
                continue;
            }
            tx.execute(
                "INSERT OR IGNORE INTO tts_profile_sectorials (profile_id, army_id) VALUES (?1, ?2)",
                params![profile_id, army_id],
            )?;
            sectorials += 1;
        }
    }
    let count = profiles.len();
    tx.commit()?;

    // Rattacher les figurines après import (legacy_nom / legacy_loadout).
    let _ = crate::tts_reattach::reattach_figurines_to_loadouts(conn);

    // Restaurer les prefs sur le nouveau tts_loadout_id de chaque figurine.
    for (user_id, figurine_id, updated_at) in prefs_snapshot {
        let loadout_id: Option<i64> = conn
            .query_row(
                "SELECT tts_loadout_id FROM tts_figurines WHERE id = ?1",
                params![figurine_id],
                |row| row.get(0),
            )
            .ok()
            .flatten();
        let Some(loadout_id) = loadout_id else {
            continue;
        };
        let _ = conn.execute(
            "
            INSERT INTO tts_figurine_pick_prefs (
                user_id, tts_loadout_id, figurine_id, updated_at
            ) VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(user_id, tts_loadout_id) DO UPDATE SET
                figurine_id = excluded.figurine_id,
                updated_at = excluded.updated_at
            ",
            params![user_id, loadout_id, figurine_id, updated_at],
        );
    }

    Ok(ArmyImportReport {
        armies: 0,
        profiles: count,
        forms,
        sectorials,
        failed_armies: 0,
    })
}

fn insert_traits(conn: &Connection, profile_id: i64, traits: &[TraitDraft]) -> Result<()> {
    for (index, trait_) in traits.iter().enumerate() {
        conn.execute(
            "
            INSERT INTO tts_profile_traits (profile_id, kind, name, detail, sort_order)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ",
            params![
                profile_id,
                trait_.kind,
                trait_.name,
                trait_.detail,
                index as i64,
            ],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_documents() -> (Value, Vec<(ArmyRow, Value)>) {
        let metadata = serde_json::json!({
            "skills": [
                {"id": 28, "name": "Mimetism"},
                {"id": 29, "name": "Camouflage"},
                {"id": 89, "name": "Sapper"}
            ],
            "weapons": [
                {"id": 33, "name": "Combi Rifle"},
                {"id": 2, "name": "Heavy Machine Gun"},
                {"id": 69, "name": "Pistol"},
                {"id": 5, "name": "CC Weapon"}
            ],
            "equips": []
        });
        let units = serde_json::json!({
            "filters": {
                "extras": [{"id": 6, "name": "-3"}],
                "chars": [{"id": 1, "name": "Cube"}, {"id": 3, "name": "Regular"}],
                "type": [{"id": 1, "name": "LI"}],
                "peripheral": [{"id": 141, "name": "TURTLEMEK"}]
            },
            "units": [{
                "id": 10,
                "name": "FUSILIER",
                "factions": [101, 103],
                "profileGroups": [{
                    "id": 1,
                    "profiles": [{
                        "name": "FUSILIER",
                        "type": 1,
                        "chars": [1, 3],
                        "move": [4, 4],
                        "cc": 13,
                        "bs": 12,
                        "ph": 10,
                        "wip": 13,
                        "arm": 1,
                        "bts": 0,
                        "w": 1,
                        "str": false,
                        "s": 2,
                        "skills": [
                            {"id": 29, "order": 1},
                            {"id": 28, "extra": [6], "order": 2},
                            {"id": 89, "order": 3}
                        ],
                        "equip": [],
                        "weapons": []
                    }, {
                        "name": "BATTLE FORM",
                        "type": 1,
                        "chars": [1],
                        "move": [4, 2],
                        "cc": 13,
                        "bs": 12,
                        "ph": 10,
                        "wip": 13,
                        "arm": 1,
                        "bts": 0,
                        "w": 1,
                        "str": false,
                        "s": 6,
                        "skills": [],
                        "equip": [],
                        "weapons": []
                    }],
                    "options": [
                        {"id": 1, "name": "FUSILIER", "disabled": false, "weapons": [{"id": 33}, {"id": 69}, {"id": 5}], "skills": [], "equip": [], "includes": [{"group": 2, "option": 1}], "peripheral": [{"id": 141, "q": 2}]},
                        {"id": 2, "name": "FUSILIER", "disabled": false, "weapons": [{"id": 2}, {"id": 69}, {"id": 5}], "skills": [], "equip": [], "includes": []}
                    ]
                }, {
                    "id": 2,
                    "profiles": [{
                        "name": "FUSILIER OPERATOR",
                        "type": 1,
                        "chars": [1],
                        "move": [4, 4],
                        "cc": 13,
                        "bs": 12,
                        "ph": 10,
                        "wip": 13,
                        "arm": 0,
                        "bts": 0,
                        "w": 1,
                        "str": false,
                        "s": 2,
                        "skills": [],
                        "equip": [],
                        "weapons": []
                    }],
                    "options": [
                        {"id": 1, "name": "OPERATOR", "disabled": true, "weapons": [{"id": 69}], "skills": [], "equip": [], "includes": []}
                    ]
                }]
            }, {
                "id": 99,
                "name": "BOUNTY HUNTER",
                "factions": [],
                "profileGroups": [{
                    "id": 1,
                    "profiles": [{
                        "name": "BOUNTY HUNTER",
                        "type": 1,
                        "chars": [1, 3],
                        "move": [4, 4],
                        "cc": 13, "bs": 12, "ph": 10, "wip": 13, "arm": 1, "bts": 0, "w": 1, "str": false, "s": 2,
                        "skills": [], "equip": [], "weapons": []
                    }],
                    "options": [
                        {"id": 1, "name": "HUNTER", "disabled": false, "weapons": [{"id": 33}], "skills": [], "equip": [], "includes": []}
                    ]
                }]
            }, {
                "id": 11,
                "name": "KNIGHT",
                "factions": [103],
                "profileGroups": [{
                    "id": 1,
                    "profiles": [{
                        "name": "KNIGHT",
                        "type": 1,
                        "chars": [1, 3],
                        "move": [4, 4],
                        "cc": 13, "bs": 12, "ph": 10, "wip": 13, "arm": 1, "bts": 0, "w": 1, "str": false, "s": 2,
                        "skills": [], "equip": [], "weapons": []
                    }],
                    "options": [
                        {"id": 1, "name": "KNIGHT", "disabled": false, "weapons": [{"id": 2}], "skills": [], "equip": [], "includes": []}
                    ]
                }]
            }]
        });
        let armies = vec![
            ArmyRow {
                id: 101,
                parent_id: 101,
                name: "PanOceania".into(),
                slug: "panoceania".into(),
            },
            ArmyRow {
                id: 103,
                parent_id: 101,
                name: "Military Orders".into(),
                slug: "military-orders".into(),
            },
        ];
        let docs = vec![
            (armies[0].clone(), units.clone()),
            (armies[1].clone(), units),
        ];
        (metadata, docs)
    }

    #[test]
    fn builds_one_profile_per_loadout_with_sectorials_and_traits() {
        let (metadata, docs) = sample_documents();
        let names = names_from_metadata(&metadata);
        let armies = docs
            .iter()
            .map(|(army, _)| army.clone())
            .collect::<Vec<_>>();
        let mut drafts = BTreeMap::new();
        for (army, document) in &docs {
            let mut names = names.clone();
            absorb_filters(&mut names, document);
            absorb_units(&mut drafts, &names, army, &armies, document);
        }
        let mut profiles: Vec<_> = drafts.into_values().collect();
        attach_sapper_forms(&mut profiles);
        apply_peripheral_names(&mut profiles);
        assert_eq!(profiles.len(), 3);
        assert!(profiles
            .iter()
            .all(|profile| profile.isc != "BOUNTY HUNTER"));
        let combi = profiles
            .iter()
            .find(|profile| profile.loadout.starts_with("Combi Rifle"))
            .unwrap();
        assert_eq!(combi.isc, "FUSILIER");
        assert_eq!(combi.loadout, "Combi Rifle || TURTLEMEK x2");
        assert!(combi.traits.iter().any(|trait_| {
            trait_.kind == "equipment"
                && trait_.name == "TURTLEMEK"
                && trait_.detail.as_deref() == Some("x2")
        }));
        assert!(combi.description_tts.contains("TURTLEMEK(x2)"));
        assert!(combi.sectorials.contains(&101));
        assert!(combi.sectorials.contains(&103));
        assert_eq!(combi.army_id, Some(101));
        assert_eq!(combi.silhouette_type.as_deref(), Some("camo-3"));
        assert!(combi.traits.iter().any(|trait_| trait_.kind == "attribute"
            && trait_.name == "BS"
            && trait_.detail.as_deref() == Some("12")));
        assert!(combi
            .traits
            .iter()
            .any(|trait_| trait_.kind == "skill" && trait_.name == "Camouflage"));
        assert!(combi
            .traits
            .iter()
            .any(|trait_| trait_.kind == "weapon" && trait_.name == "Combi Rifle"));
        assert_eq!(combi.forms.len(), 3);
        assert!(combi.forms.iter().any(|form| {
            form.isc == "FUSILIER BATTLE FORM" && form.silhouette_size.as_deref() == Some("6")
        }));
        assert!(combi
            .forms
            .iter()
            .any(|form| form.isc == "FUSILIER OPERATOR" && form.loadout == "Pistol"));
        let sapper = combi
            .forms
            .iter()
            .find(|form| form.state_index == 1000)
            .unwrap();
        assert_eq!(sapper.isc, "FUSILIER");
        assert_eq!(sapper.loadout, "Combi Rifle || TURTLEMEK x2");
        assert!(sapper
            .traits
            .iter()
            .any(|trait_| trait_.kind == "equipment" && trait_.name == "TURTLEMEK"));
        assert_eq!(sapper.silhouette_size.as_deref(), Some("4"));
        assert!(sapper.description_tts.contains("[B]S[/B]: 4"));
        let knight = profiles
            .iter()
            .find(|profile| profile.isc == "KNIGHT")
            .unwrap();
        assert!(knight.sectorials.contains(&103));
        assert!(!knight.sectorials.contains(&101));
    }

    #[test]
    fn keeps_equipment_written_after_a_double_bar_in_the_figurine_name() {
        assert_eq!(
            shift_pipe_to_loadout("FUSILIER || TURTLEMEK x2", "Combi Rifle"),
            (
                "FUSILIER".to_string(),
                "Combi Rifle || TURTLEMEK x2".to_string()
            )
        );
        assert_eq!(
            merge_figurine_suffix("Combi Rifle", "FUSILIER || CrazyKoala"),
            "Combi Rifle || CrazyKoala"
        );
        assert_eq!(
            pipe_equipment("FUSILIER || TURTLEMEK x2, CrazyKoala"),
            vec!["TURTLEMEK".to_string(), "CrazyKoala".to_string()]
        );
    }

    #[test]
    fn imports_team_members_referenced_by_unit_options() {
        let metadata = serde_json::json!({
            "skills": [],
            "weapons": [
                {"id": 33, "name": "Combi Rifle"},
                {"id": 69, "name": "Pistol"},
                {"id": 5, "name": "CC Weapon"},
                {"id": 2, "name": "Heavy Machine Gun"}
            ],
            "equips": []
        });
        let army = ArmyRow {
            id: 301,
            parent_id: 301,
            name: "Ariadna".into(),
            slug: "ariadna".into(),
        };
        let document = serde_json::json!({
            "filters": {
                "extras": [],
                "chars": [],
                "type": [],
                "peripheral": []
            },
            "units": [{
                "id": 1522,
                "name": "POLARIS TEAM",
                "factions": [301, 306],
                "options": [
                    {
                        "id": 1,
                        "name": "POLARIS TEAM ALPHA",
                        "disabled": false,
                        "includes": [
                            {"q": 1, "group": 1, "option": 1},
                            {"q": 1, "group": 2, "option": 1}
                        ]
                    },
                    {
                        "id": 2,
                        "name": "POLARIS TEAM BETA",
                        "disabled": false,
                        "includes": [
                            {"q": 1, "group": 1, "option": 1},
                            {"q": 1, "group": 2, "option": 2}
                        ]
                    }
                ],
                "profileGroups": [
                    {
                        "id": 1,
                        "isc": "Masha, Polaris Handler",
                        "profiles": [{
                            "name": "MASHA",
                            "type": 1,
                            "chars": [],
                            "move": [4, 4],
                            "cc": 13, "bs": 12, "ph": 10, "wip": 13, "arm": 1, "bts": 0, "w": 1,
                            "str": false, "s": 2,
                            "skills": [], "equip": [], "weapons": []
                        }],
                        "options": [{
                            "id": 1,
                            "name": "MASHA",
                            "disabled": true,
                            "weapons": [{"id": 33}, {"id": 69}, {"id": 5}],
                            "skills": [], "equip": [], "includes": []
                        }]
                    },
                    {
                        "id": 2,
                        "isc": "Polaris Bearpode",
                        "profiles": [{
                            "name": "POLARIS BEARPODE",
                            "type": 1,
                            "chars": [],
                            "move": [6, 4],
                            "cc": 20, "bs": 10, "ph": 14, "wip": 13, "arm": 2, "bts": 0, "w": 2,
                            "str": false, "s": 5,
                            "skills": [], "equip": [], "weapons": []
                        }],
                        "options": [
                            {
                                "id": 1,
                                "name": "POLARIS BEARPODE ALPHA",
                                "disabled": true,
                                "weapons": [{"id": 33}, {"id": 5}],
                                "skills": [], "equip": [], "includes": []
                            },
                            {
                                "id": 2,
                                "name": "POLARIS BEARPODE BETA",
                                "disabled": true,
                                "weapons": [{"id": 2}, {"id": 5}],
                                "skills": [], "equip": [], "includes": []
                            }
                        ]
                    }
                ]
            }, {
                "id": 10,
                "name": "FUSILIER",
                "factions": [301],
                "profileGroups": [{
                    "id": 1,
                    "profiles": [{
                        "name": "FUSILIER",
                        "type": 1,
                        "chars": [],
                        "move": [4, 4],
                        "cc": 13, "bs": 12, "ph": 10, "wip": 13, "arm": 1, "bts": 0, "w": 1,
                        "str": false, "s": 2,
                        "skills": [], "equip": [], "weapons": []
                    }],
                    "options": [{
                        "id": 1,
                        "name": "FUSILIER",
                        "disabled": false,
                        "weapons": [{"id": 33}],
                        "skills": [], "equip": [],
                        "includes": [{"group": 2, "option": 1}]
                    }]
                }, {
                    "id": 2,
                    "profiles": [{
                        "name": "OPERATOR",
                        "type": 1,
                        "chars": [],
                        "move": [4, 4],
                        "cc": 13, "bs": 12, "ph": 10, "wip": 13, "arm": 0, "bts": 0, "w": 1,
                        "str": false, "s": 2,
                        "skills": [], "equip": [], "weapons": []
                    }],
                    "options": [{
                        "id": 1,
                        "name": "OPERATOR",
                        "disabled": true,
                        "weapons": [{"id": 69}],
                        "skills": [], "equip": [], "includes": []
                    }]
                }]
            }]
        });
        let names = names_from_metadata(&metadata);
        let armies = vec![army.clone()];
        let mut drafts = BTreeMap::new();
        absorb_units(&mut drafts, &names, &army, &armies, &document);
        let profiles: Vec<_> = drafts.into_values().collect();
        assert!(profiles.iter().any(|profile| {
            profile.isc == "MASHA" && profile.loadout.starts_with("Combi Rifle")
        }));
        assert!(profiles.iter().any(|profile| {
            profile.isc == "POLARIS BEARPODE ALPHA" && profile.loadout.starts_with("Combi Rifle")
        }));
        assert!(profiles.iter().any(|profile| {
            profile.isc == "POLARIS BEARPODE BETA"
                && profile.loadout.starts_with("Heavy Machine Gun")
        }));
        assert!(!profiles.iter().any(|profile| profile.isc == "OPERATOR"));
        assert_eq!(
            profiles
                .iter()
                .filter(|profile| {
                    profile.isc.contains("POLARIS") || profile.isc.contains("MASHA")
                })
                .count(),
            3
        );

        let mut includes = BTreeMap::new();
        absorb_unit_includes(&mut includes, &document);
        assert_eq!(
            includes.get(&(1522, 1)).map(Vec::as_slice),
            Some(
                [
                    UnitOptionInclude {
                        unit_id: 1522,
                        unit_option_id: 1,
                        sort_order: 0,
                        group_id: 1,
                        option_id: 1,
                        quantity: 1,
                    },
                    UnitOptionInclude {
                        unit_id: 1522,
                        unit_option_id: 1,
                        sort_order: 1,
                        group_id: 2,
                        option_id: 1,
                        quantity: 1,
                    },
                ]
                .as_slice()
            )
        );
        assert_eq!(
            includes.get(&(1522, 2)).map(|rows| rows
                .iter()
                .map(|row| (row.group_id, row.option_id))
                .collect::<Vec<_>>()),
            Some(vec![(1, 1), (2, 2)])
        );
    }

    #[test]
    fn qualifies_transmute_form_isc_with_parent_unit() {
        assert_eq!(
            qualify_form_isc("Chernobog Armored Detachment", "BATTLE RAVAGED"),
            "Chernobog Armored Detachment BATTLE RAVAGED"
        );
        assert_eq!(
            qualify_form_isc("Sù-Jiàn Immediate Action Unit", "COMBAT FORM"),
            "Sù-Jiàn Immediate Action Unit COMBAT FORM"
        );
        assert_eq!(
            qualify_form_isc("Dog-Warriors", "DOG-WARRIOR FORM"),
            "Dog-Warriors DOG-WARRIOR FORM"
        );
        // Déjà qualifié / même nom : inchangé.
        assert_eq!(
            qualify_form_isc(
                "Chernobog Armored Detachment",
                "Chernobog Armored Detachment BATTLE RAVAGED"
            ),
            "Chernobog Armored Detachment BATTLE RAVAGED"
        );
        assert_eq!(
            qualify_form_isc("Candy Cloud", "Candy Cloud"),
            "Candy Cloud"
        );
    }

    #[test]
    fn transmute_forms_use_qualified_isc_on_import() {
        let metadata = serde_json::json!({
            "skills": [],
            "weapons": [{"id": 2, "name": "Heavy Machine Gun"}],
            "equips": []
        });
        let army = ArmyRow {
            id: 501,
            parent_id: 501,
            name: "Combined Army".into(),
            slug: "combined-army".into(),
        };
        let document = serde_json::json!({
            "filters": {
                "extras": [], "chars": [], "type": [], "peripheral": []
            },
            "units": [{
                "id": 9001,
                "name": "CHERNOBOG ARMORED DETACHMENT",
                "factions": [501],
                "profileGroups": [{
                    "id": 1,
                    "isc": "Chernobog Armored Detachment",
                    "profiles": [
                        {
                            "name": "CHERNOBOG",
                            "type": 1, "chars": [], "move": [4,4],
                            "cc": 10, "bs": 12, "ph": 14, "wip": 13, "arm": 6, "bts": 3, "w": 3,
                            "str": true, "s": 7,
                            "skills": [], "equip": [], "weapons": []
                        },
                        {
                            "name": "BATTLE RAVAGED",
                            "type": 1, "chars": [], "move": [4,4],
                            "cc": 10, "bs": 12, "ph": 14, "wip": 13, "arm": 4, "bts": 0, "w": 2,
                            "str": true, "s": 6,
                            "skills": [], "equip": [], "weapons": []
                        }
                    ],
                    "options": [{
                        "id": 1,
                        "name": "CHERNOBOG",
                        "disabled": false,
                        "weapons": [{"id": 2}],
                        "skills": [], "equip": [], "includes": []
                    }]
                }]
            }]
        });
        let names = names_from_metadata(&metadata);
        let armies = vec![army.clone()];
        let mut drafts = BTreeMap::new();
        absorb_units(&mut drafts, &names, &army, &armies, &document);
        let profile = drafts.values().next().expect("profil");
        assert_eq!(profile.isc, "CHERNOBOG");
        assert!(
            profile.forms.iter().any(|form| {
                form.state_index == 1 && form.isc == "CHERNOBOG BATTLE RAVAGED"
            }),
            "forms = {:?}",
            profile
                .forms
                .iter()
                .map(|form| (form.state_index, form.isc.as_str()))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn parses_mirage_unit_option_includes() {
        let document = serde_json::json!({
            "units": [{
                "id": 258,
                "name": "EQUIPE MIRAGE-5",
                "options": [{
                    "id": 1,
                    "name": "EQUIPE MIRAGE-5 (MARGOT & DUROC)",
                    "disabled": false,
                    "includes": [
                        {"q": 1, "group": 1, "option": 1},
                        {"q": 1, "group": 2, "option": 1}
                    ]
                }]
            }]
        });
        let includes = unit_option_includes_from_document(&document, 258, 1);
        assert_eq!(
            includes
                .iter()
                .map(|row| (row.group_id, row.option_id, row.quantity))
                .collect::<Vec<_>>(),
            vec![(1, 1, 1), (2, 1, 1)]
        );
    }

    #[test]
    fn attaches_ai_motorcycle_dismount_and_peripheral_as_forms() {
        let metadata = serde_json::json!({
            "skills": [
                {"id": 84, "name": "Courage"},
                {"id": 246, "name": "Transmutation"}
            ],
            "weapons": [
                {"id": 74, "name": "Spitfire"},
                {"id": 69, "name": "Pistol"},
                {"id": 5, "name": "CC Weapon"},
                {"id": 18, "name": "Chain Rifle"},
                {"id": 71, "name": "PARA CC Weapon"}
            ],
            "equips": [{"id": 205, "name": "AI Motorcycle"}]
        });
        let army = ArmyRow {
            id: 501,
            parent_id: 501,
            name: "Nomads".into(),
            slug: "nomads".into(),
        };
        let document = serde_json::json!({
            "filters": {
                "extras": [{"id": 254, "name": "Auto"}],
                "chars": [],
                "type": [],
                "peripheral": [{"id": 5, "name": "ZONDMATE"}]
            },
            "units": [{
                "id": 1091,
                "name": "ZONDNAUTICA Rapid Offensive Unit",
                "factions": [501],
                "options": [{
                    "id": 1,
                    "name": "ZONDNAUTICA-A",
                    "disabled": false,
                    "includes": [
                        {"q": 1, "group": 1, "option": 1},
                        {"q": 1, "group": 2, "option": 1}
                    ]
                }],
                "profileGroups": [
                    {
                        "id": 1,
                        "isc": "Zondnautica Rapid Offensive Unit",
                        "profiles": [{
                            "name": "MOUNTED ZONDNAUTICA Rapid Offensive Unit",
                            "type": 2, "chars": [], "move": [8, 4],
                            "cc": 15, "bs": 12, "ph": 12, "wip": 13, "arm": 3, "bts": 3, "w": 1,
                            "str": false, "s": 4,
                            "skills": [{"id": 84}, {"id": 246, "extra": [254]}],
                            "equip": [{"id": 205}], "weapons": [], "peripheral": []
                        }],
                        "options": [{
                            "id": 1,
                            "name": "ZONDNAUTICA-A",
                            "disabled": true,
                            "weapons": [{"id": 74}, {"id": 69}, {"id": 5}],
                            "skills": [], "equip": [], "includes": [], "peripheral": []
                        }]
                    },
                    {
                        "id": 2,
                        "isc": "Zondnautica Rapid Offensive Unit ",
                        "profiles": [{
                            "name": "DISMOUNTED ZONDNAUTICA Rapid Offensive Unit",
                            "type": 2, "chars": [], "move": [4, 4],
                            "cc": 15, "bs": 12, "ph": 11, "wip": 13, "arm": 2, "bts": 3, "w": 1,
                            "str": false, "s": 2,
                            "skills": [{"id": 84}],
                            "equip": [], "weapons": [], "peripheral": []
                        }],
                        "options": [{
                            "id": 1,
                            "name": "ZONDNAUT-A",
                            "disabled": true,
                            "weapons": [{"id": 74}, {"id": 69}, {"id": 5}],
                            "skills": [], "equip": [],
                            "includes": [{"q": 1, "group": 3, "option": 1}],
                            "peripheral": [{"id": 5}]
                        }]
                    },
                    {
                        "id": 3,
                        "isc": "ZondMate",
                        "profiles": [{
                            "name": "ZONDMATE",
                            "type": 5, "chars": [], "move": [8, 4],
                            "cc": 13, "bs": 10, "ph": 10, "wip": 11, "arm": 1, "bts": 3, "w": 1,
                            "str": true, "s": 5,
                            "skills": [{"id": 84}],
                            "equip": [{"id": 205}], "weapons": [], "peripheral": []
                        }],
                        "options": [{
                            "id": 1,
                            "name": "ZONDMATE",
                            "disabled": true,
                            "weapons": [{"id": 18}, {"id": 71}],
                            "skills": [], "equip": [], "includes": [], "peripheral": []
                        }]
                    }
                ]
            }]
        });
        let mut names = names_from_metadata(&metadata);
        absorb_filters(&mut names, &document);
        let armies = vec![army.clone()];
        let mut drafts = BTreeMap::new();
        absorb_units(&mut drafts, &names, &army, &armies, &document);
        let mut unit_includes = BTreeMap::new();
        absorb_unit_includes(&mut unit_includes, &document);
        let mut profiles: Vec<_> = drafts.into_values().collect();
        apply_peripheral_names(&mut profiles);
        attach_ai_motorcycle_forms(&mut profiles, &mut unit_includes);

        assert_eq!(profiles.len(), 1, "seul le monté reste déployable");
        let mounted = &profiles[0];
        assert_eq!(mounted.silhouette_size.as_deref(), Some("4"));
        assert!(has_ai_motorcycle(mounted));

        let dismount = mounted
            .forms
            .iter()
            .find(|form| form.state_index == 2)
            .expect("forme dismounted");
        assert_eq!(dismount.silhouette_size.as_deref(), Some("2"));
        assert_eq!(dismount.isc, "ZONDNAUT-A");

        let zondmate = mounted
            .forms
            .iter()
            .find(|form| form.state_index == 900 && form.isc == "ZONDMATE")
            .expect("périphérique zondmate");
        assert_eq!(zondmate.silhouette_size.as_deref(), Some("5"));

        let includes = unit_includes.get(&(1091, 1)).expect("includes unitaires");
        assert_eq!(
            includes
                .iter()
                .map(|row| (row.group_id, row.option_id))
                .collect::<Vec<_>>(),
            vec![(1, 1)]
        );
    }

    #[test]
    fn skips_commlink_options_reserved_for_reinforcements() {
        let metadata = serde_json::json!({
            "skills": [
                {"id": 84, "name": "Courage"},
                {"id": 999, "name": "Commlink"}
            ],
            "weapons": [
                {"id": 33, "name": "Combi Rifle"},
                {"id": 69, "name": "Pistol"},
                {"id": 5, "name": "CC Weapon"}
            ],
            "equips": []
        });
        let army = ArmyRow {
            id: 101,
            parent_id: 101,
            name: "PanOceania".into(),
            slug: "panoceania".into(),
        };
        let document = serde_json::json!({
            "filters": {
                "extras": [],
                "chars": [],
                "type": [],
                "peripheral": []
            },
            "units": [{
                "id": 42,
                "name": "FUSILIER",
                "factions": [101],
                "profileGroups": [{
                    "id": 1,
                    "isc": "Fusilier",
                    "profiles": [{
                        "name": "FUSILIER",
                        "type": 1, "chars": [], "move": [4, 4],
                        "cc": 13, "bs": 12, "ph": 10, "wip": 13, "arm": 1, "bts": 0, "w": 1,
                        "str": false, "s": 2,
                        "skills": [{"id": 84}], "equip": [], "weapons": []
                    }],
                    "options": [
                        {
                            "id": 1,
                            "name": "FUSILIER",
                            "disabled": false,
                            "weapons": [{"id": 33}, {"id": 69}, {"id": 5}],
                            "skills": [], "equip": [], "includes": []
                        },
                        {
                            "id": 2,
                            "name": "FUSILIER",
                            "disabled": false,
                            "weapons": [{"id": 33}, {"id": 69}, {"id": 5}],
                            "skills": [{"id": 999}], "equip": [], "includes": []
                        }
                    ]
                }]
            }]
        });
        let names = names_from_metadata(&metadata);
        let armies = vec![army.clone()];
        let mut drafts = BTreeMap::new();
        absorb_units(&mut drafts, &names, &army, &armies, &document);
        let profiles: Vec<_> = drafts.into_values().collect();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].loadout, "Combi Rifle");
        assert!(!profiles[0].traits.iter().any(|trait_| {
            trait_.kind == "skill" && trait_.name.eq_ignore_ascii_case("Commlink")
        }));
    }

    #[test]
    fn lieutenant_shares_tts_loadout_with_sibling() {
        let dir = std::env::temp_dir().join(format!(
            "poissonnerie-lt-share-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("test.db");
        let conn = Connection::open(&db).unwrap();
        migrate(&conn).unwrap();

        let base = ProfileDraft {
            unit_id: 1,
            group_id: 1,
            option_id: 1,
            army_id: None,
            faction_slug: Some("ariadna".into()),
            faction_label: "Ariadna".into(),
            isc: "NISSE".into(),
            loadout: "Combi Rifle".into(),
            nom_tts: "[FFFFFF]NISSE[-] Combi Rifle".into(),
            description_tts: "Regular".into(),
            silhouette_size: Some("2".into()),
            silhouette_type: Some("normale".into()),
            marker_name: None,
            mesh_url: None,
            marker_mesh_url: None,
            traits: vec![TraitDraft {
                kind: "char",
                name: "Regular".into(),
                detail: None,
            }],
            forms: Vec::new(),
            sectorials: BTreeSet::new(),
            peripherals: Vec::new(),
            escape_pilot: false,
            operator: false,
        };
        let mut lt = base.clone();
        lt.option_id = 2;
        lt.traits.push(TraitDraft {
            kind: "char",
            name: "Lieutenant".into(),
            detail: None,
        });
        lt.description_tts = "Regular · Lieutenant".into();

        let report = store_profiles(&conn, &[base, lt], &[]).unwrap();
        assert_eq!(report.profiles, 2);

        let loadout_ids: Vec<i64> = {
            let mut stmt = conn
                .prepare("SELECT tts_loadout_id FROM tts_profiles WHERE parent_profile_id IS NULL ORDER BY source_index")
                .unwrap();
            stmt.query_map([], |row| row.get(0))
                .unwrap()
                .map(|row| row.unwrap())
                .collect()
        };
        assert_eq!(loadout_ids.len(), 2);
        assert_eq!(loadout_ids[0], loadout_ids[1]);

        let loadout_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tts_loadouts", [], |row| row.get(0))
            .unwrap();
        assert_eq!(loadout_count, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

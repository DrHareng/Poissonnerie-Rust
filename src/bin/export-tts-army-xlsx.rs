//! Exporte les données brutes de l’API Infinity Army (sans transformation TTS).
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use rusqlite::Connection;
use rust_xlsxwriter::{Format, Workbook, Worksheet};
use serde_json::Value;

const METADATA_URL: &str = "https://api.corvusbelli.com/army/infinity/en/metadata";
const ORIGIN: &str = "https://infinityuniverse.com";
const REFERER: &str = "https://infinityuniverse.com/army/infinity";

#[derive(Parser)]
#[command(
    name = "poissonnerie-export-tts-army-xlsx",
    about = "Exporte les données brutes Infinity Army vers Excel (sans traitement TTS)"
)]
struct Args {
    /// Base SQLite (liste des armées à interroger).
    #[arg(long, default_value = "data/poissonnerie.db")]
    db: PathBuf,

    /// Fichier Excel de sortie.
    #[arg(long, default_value = "data/tts-army-raw.xlsx")]
    output: PathBuf,
}

#[derive(Clone)]
struct ArmyRow {
    id: i64,
    name: String,
    slug: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let conn = Connection::open(&args.db)
        .with_context(|| format!("impossible d’ouvrir {}", args.db.display()))?;
    let armies = load_armies(&conn)?;

    let client = reqwest::blocking::Client::builder()
        .user_agent("poissonnerie-elo/0.1")
        .build()?;
    eprintln!("Metadata…");
    let metadata = fetch_json(&client, METADATA_URL)?;

    let mut workbook = Workbook::new();
    let header = Format::new().set_bold();

    write_factions_from_metadata(&metadata, &mut workbook, &header)?;
    write_catalog(&metadata, "skills", "CatalogSkills", &mut workbook, &header)?;
    write_catalog(&metadata, "weapons", "CatalogWeapons", &mut workbook, &header)?;
    write_catalog(&metadata, "equips", "CatalogEquips", &mut workbook, &header)?;

    let mut filter_catalog: BTreeMap<(String, i64), String> = BTreeMap::new();
    let mut units_rows: Vec<UnitRow> = Vec::new();
    let mut groups_rows: Vec<GroupRow> = Vec::new();
    let mut profiles_rows: Vec<ProfileRow> = Vec::new();
    let mut options_rows: Vec<OptionRow> = Vec::new();
    let mut unit_options_rows: Vec<UnitOptionRow> = Vec::new();
    let mut includes_rows: Vec<IncludeRow> = Vec::new();
    let mut seen_units = BTreeSet::new();
    let mut failed = 0usize;

    for army in armies.iter().filter(|army| importable_army(army)) {
        let url = format!("https://api.corvusbelli.com/army/units/en/{}", army.id);
        let document = match fetch_json(&client, &url) {
            Ok(document) => document,
            Err(error) => {
                eprintln!("  {} ({}) ignorée : {error}", army.name, army.id);
                failed += 1;
                continue;
            }
        };
        eprintln!("  {} ({})", army.name, army.id);
        absorb_filter_catalog(&mut filter_catalog, &document);
        absorb_document(
            army,
            &document,
            &mut seen_units,
            &mut units_rows,
            &mut groups_rows,
            &mut profiles_rows,
            &mut options_rows,
            &mut unit_options_rows,
            &mut includes_rows,
        );
    }

    write_filter_catalog(&filter_catalog, &mut workbook, &header)?;
    write_units(&units_rows, &mut workbook, &header)?;
    write_groups(&groups_rows, &mut workbook, &header)?;
    write_profiles(&profiles_rows, &mut workbook, &header)?;
    write_options(&options_rows, &mut workbook, &header)?;
    write_unit_options(&unit_options_rows, &mut workbook, &header)?;
    write_includes(&includes_rows, &mut workbook, &header)?;

    workbook
        .save(&args.output)
        .with_context(|| format!("impossible d’écrire {}", args.output.display()))?;

    println!("Export Excel écrit : {}", args.output.display());
    if failed > 0 {
        println!("Armées en échec : {failed}");
    }
    Ok(())
}

struct UnitRow {
    fetched_army_id: i64,
    fetched_army_name: String,
    unit_id: i64,
    name: String,
    factions: String,
}

struct GroupRow {
    unit_id: i64,
    group_id: i64,
    isc: String,
}

struct ProfileRow {
    unit_id: i64,
    group_id: i64,
    profile_index: i64,
    name: String,
    type_id: String,
    chars: String,
    mov: String,
    cc: String,
    bs: String,
    ph: String,
    wip: String,
    arm: String,
    bts: String,
    w: String,
    str: String,
    s: String,
    skills: String,
    equip: String,
    weapons: String,
}

struct OptionRow {
    unit_id: i64,
    group_id: i64,
    option_id: i64,
    name: String,
    disabled: bool,
    weapons: String,
    skills: String,
    equip: String,
    peripheral: String,
    includes: String,
}

struct UnitOptionRow {
    unit_id: i64,
    option_id: i64,
    name: String,
    disabled: bool,
    includes: String,
}

struct IncludeRow {
    unit_id: i64,
    unit_option_id: i64,
    included_group_id: i64,
    included_option_id: i64,
    quantity: i64,
}

fn load_armies(conn: &Connection) -> Result<Vec<ArmyRow>> {
    let mut stmt = conn.prepare(
        "
        SELECT id, name, slug
        FROM armies
        ORDER BY id
        ",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(ArmyRow {
            id: row.get(0)?,
            name: row.get(1)?,
            slug: row.get(2)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .context("lecture des armées")
}

fn importable_army(army: &ArmyRow) -> bool {
    army.id % 100 != 99 && army.slug != "non-aligned-armies" && army.slug != "contracted-back-up"
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

fn json_compact(value: Option<&Value>) -> String {
    value
        .map(|v| serde_json::to_string(v).unwrap_or_default())
        .unwrap_or_default()
}

fn value_as_display(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Array(items)) => serde_json::to_string(items).unwrap_or_default(),
        Some(other) => serde_json::to_string(other).unwrap_or_default(),
    }
}

fn write_header(sheet: &mut Worksheet, header: &Format, titles: &[&str]) -> Result<()> {
    for (col, title) in titles.iter().enumerate() {
        sheet.write_string_with_format(0, col as u16, *title, header)?;
    }
    Ok(())
}

fn write_catalog(
    metadata: &Value,
    key: &str,
    sheet_name: &str,
    workbook: &mut Workbook,
    header: &Format,
) -> Result<()> {
    let sheet = workbook.add_worksheet();
    sheet.set_name(sheet_name)?;
    write_header(sheet, header, &["id", "name"])?;
    let mut row_idx = 1u32;
    if let Some(items) = metadata.get(key).and_then(Value::as_array) {
        for item in items {
            let Some(id) = item.get("id").and_then(Value::as_i64) else {
                continue;
            };
            let name = item.get("name").and_then(Value::as_str).unwrap_or("");
            sheet.write_number(row_idx, 0, id as f64)?;
            sheet.write_string(row_idx, 1, name)?;
            row_idx += 1;
        }
    }
    println!("  {sheet_name} : {} lignes", row_idx.saturating_sub(1));
    Ok(())
}

fn write_factions_from_metadata(
    metadata: &Value,
    workbook: &mut Workbook,
    header: &Format,
) -> Result<()> {
    let sheet = workbook.add_worksheet();
    sheet.set_name("Factions")?;
    write_header(
        sheet,
        header,
        &["id", "parent", "name", "slug", "logo", "discontinued"],
    )?;
    let mut row_idx = 1u32;
    if let Some(items) = metadata.get("factions").and_then(Value::as_array) {
        for item in items {
            let Some(id) = item.get("id").and_then(Value::as_i64) else {
                continue;
            };
            sheet.write_number(row_idx, 0, id as f64)?;
            if let Some(parent) = item.get("parent").and_then(Value::as_i64) {
                sheet.write_number(row_idx, 1, parent as f64)?;
            }
            sheet.write_string(
                row_idx,
                2,
                item.get("name").and_then(Value::as_str).unwrap_or(""),
            )?;
            sheet.write_string(
                row_idx,
                3,
                item.get("slug").and_then(Value::as_str).unwrap_or(""),
            )?;
            sheet.write_string(
                row_idx,
                4,
                item.get("logo").and_then(Value::as_str).unwrap_or(""),
            )?;
            if let Some(disc) = item.get("discontinued").and_then(Value::as_bool) {
                sheet.write_boolean(row_idx, 5, disc)?;
            }
            row_idx += 1;
        }
    }
    println!("  Factions : {} lignes", row_idx.saturating_sub(1));
    Ok(())
}

fn absorb_filter_catalog(dest: &mut BTreeMap<(String, i64), String>, document: &Value) {
    let Some(filters) = document.get("filters") else {
        return;
    };
    for key in [
        "extras",
        "chars",
        "type",
        "weapons",
        "skills",
        "equip",
        "peripheral",
    ] {
        let Some(items) = filters.get(key).and_then(Value::as_array) else {
            continue;
        };
        for item in items {
            if let (Some(id), Some(name)) = (
                item.get("id").and_then(Value::as_i64),
                item.get("name").and_then(Value::as_str),
            ) {
                dest.insert((key.to_string(), id), name.to_string());
            }
        }
    }
}

fn write_filter_catalog(
    catalog: &BTreeMap<(String, i64), String>,
    workbook: &mut Workbook,
    header: &Format,
) -> Result<()> {
    let sheet = workbook.add_worksheet();
    sheet.set_name("Filters")?;
    write_header(sheet, header, &["kind", "id", "name"])?;
    let mut row_idx = 1u32;
    for ((kind, id), name) in catalog {
        sheet.write_string(row_idx, 0, kind)?;
        sheet.write_number(row_idx, 1, *id as f64)?;
        sheet.write_string(row_idx, 2, name)?;
        row_idx += 1;
    }
    println!("  Filters : {} lignes", row_idx.saturating_sub(1));
    Ok(())
}

fn absorb_document(
    army: &ArmyRow,
    document: &Value,
    seen_units: &mut BTreeSet<i64>,
    units_rows: &mut Vec<UnitRow>,
    groups_rows: &mut Vec<GroupRow>,
    profiles_rows: &mut Vec<ProfileRow>,
    options_rows: &mut Vec<OptionRow>,
    unit_options_rows: &mut Vec<UnitOptionRow>,
    includes_rows: &mut Vec<IncludeRow>,
) {
    let Some(units) = document.get("units").and_then(Value::as_array) else {
        return;
    };
    for unit in units {
        let Some(unit_id) = unit.get("id").and_then(Value::as_i64) else {
            continue;
        };
        let first_seen = seen_units.insert(unit_id);
        let name = unit
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let factions = json_compact(unit.get("factions"));

        units_rows.push(UnitRow {
            fetched_army_id: army.id,
            fetched_army_name: army.name.clone(),
            unit_id,
            name: name.clone(),
            factions,
        });

        if first_seen {
            if let Some(groups) = unit.get("profileGroups").and_then(Value::as_array) {
                for group in groups {
                    let group_id = group.get("id").and_then(Value::as_i64).unwrap_or(0);
                    let isc = group
                        .get("isc")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    groups_rows.push(GroupRow {
                        unit_id,
                        group_id,
                        isc,
                    });
                    if let Some(profiles) = group.get("profiles").and_then(Value::as_array) {
                        for (profile_index, profile) in profiles.iter().enumerate() {
                            profiles_rows.push(ProfileRow {
                                unit_id,
                                group_id,
                                profile_index: profile_index as i64,
                                name: profile
                                    .get("name")
                                    .and_then(Value::as_str)
                                    .unwrap_or("")
                                    .to_string(),
                                type_id: value_as_display(profile.get("type")),
                                chars: json_compact(profile.get("chars")),
                                mov: value_as_display(profile.get("move")),
                                cc: value_as_display(profile.get("cc")),
                                bs: value_as_display(profile.get("bs")),
                                ph: value_as_display(profile.get("ph")),
                                wip: value_as_display(profile.get("wip")),
                                arm: value_as_display(profile.get("arm")),
                                bts: value_as_display(profile.get("bts")),
                                w: value_as_display(profile.get("w")),
                                str: value_as_display(profile.get("str")),
                                s: value_as_display(profile.get("s")),
                                skills: json_compact(profile.get("skills")),
                                equip: json_compact(profile.get("equip")),
                                weapons: json_compact(profile.get("weapons")),
                            });
                        }
                    }
                    if let Some(options) = group.get("options").and_then(Value::as_array) {
                        for option in options {
                            let option_id = option.get("id").and_then(Value::as_i64).unwrap_or(0);
                            options_rows.push(OptionRow {
                                unit_id,
                                group_id,
                                option_id,
                                name: option
                                    .get("name")
                                    .and_then(Value::as_str)
                                    .unwrap_or("")
                                    .to_string(),
                                disabled: option
                                    .get("disabled")
                                    .and_then(Value::as_bool)
                                    .unwrap_or(false),
                                weapons: json_compact(option.get("weapons")),
                                skills: json_compact(option.get("skills")),
                                equip: json_compact(option.get("equip")),
                                peripheral: json_compact(option.get("peripheral")),
                                includes: json_compact(option.get("includes")),
                            });
                        }
                    }
                }
            }
        }

        if let Some(options) = unit.get("options").and_then(Value::as_array) {
            for option in options {
                let option_id = option.get("id").and_then(Value::as_i64).unwrap_or(0);
                unit_options_rows.push(UnitOptionRow {
                    unit_id,
                    option_id,
                    name: option
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    disabled: option
                        .get("disabled")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    includes: json_compact(option.get("includes")),
                });
                if let Some(includes) = option.get("includes").and_then(Value::as_array) {
                    for include in includes {
                        let Some(group) = include.get("group").and_then(Value::as_i64) else {
                            continue;
                        };
                        let Some(opt) = include.get("option").and_then(Value::as_i64) else {
                            continue;
                        };
                        let quantity = include.get("q").and_then(Value::as_i64).unwrap_or(1);
                        includes_rows.push(IncludeRow {
                            unit_id,
                            unit_option_id: option_id,
                            included_group_id: group,
                            included_option_id: opt,
                            quantity,
                        });
                    }
                }
            }
        }
    }
}

fn write_units(rows: &[UnitRow], workbook: &mut Workbook, header: &Format) -> Result<()> {
    let sheet = workbook.add_worksheet();
    sheet.set_name("Units")?;
    write_header(
        sheet,
        header,
        &[
            "fetched_army_id",
            "fetched_army_name",
            "unit_id",
            "name",
            "factions",
        ],
    )?;
    for (i, row) in rows.iter().enumerate() {
        let r = (i + 1) as u32;
        sheet.write_number(r, 0, row.fetched_army_id as f64)?;
        sheet.write_string(r, 1, &row.fetched_army_name)?;
        sheet.write_number(r, 2, row.unit_id as f64)?;
        sheet.write_string(r, 3, &row.name)?;
        sheet.write_string(r, 4, &row.factions)?;
    }
    println!("  Units : {} lignes", rows.len());
    Ok(())
}

fn write_groups(rows: &[GroupRow], workbook: &mut Workbook, header: &Format) -> Result<()> {
    let sheet = workbook.add_worksheet();
    sheet.set_name("ProfileGroups")?;
    write_header(sheet, header, &["unit_id", "group_id", "isc"])?;
    for (i, row) in rows.iter().enumerate() {
        let r = (i + 1) as u32;
        sheet.write_number(r, 0, row.unit_id as f64)?;
        sheet.write_number(r, 1, row.group_id as f64)?;
        sheet.write_string(r, 2, &row.isc)?;
    }
    println!("  ProfileGroups : {} lignes", rows.len());
    Ok(())
}

fn write_profiles(rows: &[ProfileRow], workbook: &mut Workbook, header: &Format) -> Result<()> {
    let sheet = workbook.add_worksheet();
    sheet.set_name("Profiles")?;
    write_header(
        sheet,
        header,
        &[
            "unit_id",
            "group_id",
            "profile_index",
            "name",
            "type",
            "chars",
            "move",
            "cc",
            "bs",
            "ph",
            "wip",
            "arm",
            "bts",
            "w",
            "str",
            "s",
            "skills",
            "equip",
            "weapons",
        ],
    )?;
    for (i, row) in rows.iter().enumerate() {
        let r = (i + 1) as u32;
        sheet.write_number(r, 0, row.unit_id as f64)?;
        sheet.write_number(r, 1, row.group_id as f64)?;
        sheet.write_number(r, 2, row.profile_index as f64)?;
        sheet.write_string(r, 3, &row.name)?;
        sheet.write_string(r, 4, &row.type_id)?;
        sheet.write_string(r, 5, &row.chars)?;
        sheet.write_string(r, 6, &row.mov)?;
        sheet.write_string(r, 7, &row.cc)?;
        sheet.write_string(r, 8, &row.bs)?;
        sheet.write_string(r, 9, &row.ph)?;
        sheet.write_string(r, 10, &row.wip)?;
        sheet.write_string(r, 11, &row.arm)?;
        sheet.write_string(r, 12, &row.bts)?;
        sheet.write_string(r, 13, &row.w)?;
        sheet.write_string(r, 14, &row.str)?;
        sheet.write_string(r, 15, &row.s)?;
        sheet.write_string(r, 16, &row.skills)?;
        sheet.write_string(r, 17, &row.equip)?;
        sheet.write_string(r, 18, &row.weapons)?;
    }
    println!("  Profiles : {} lignes", rows.len());
    Ok(())
}

fn write_options(rows: &[OptionRow], workbook: &mut Workbook, header: &Format) -> Result<()> {
    let sheet = workbook.add_worksheet();
    sheet.set_name("GroupOptions")?;
    write_header(
        sheet,
        header,
        &[
            "unit_id",
            "group_id",
            "option_id",
            "name",
            "disabled",
            "weapons",
            "skills",
            "equip",
            "peripheral",
            "includes",
        ],
    )?;
    for (i, row) in rows.iter().enumerate() {
        let r = (i + 1) as u32;
        sheet.write_number(r, 0, row.unit_id as f64)?;
        sheet.write_number(r, 1, row.group_id as f64)?;
        sheet.write_number(r, 2, row.option_id as f64)?;
        sheet.write_string(r, 3, &row.name)?;
        sheet.write_boolean(r, 4, row.disabled)?;
        sheet.write_string(r, 5, &row.weapons)?;
        sheet.write_string(r, 6, &row.skills)?;
        sheet.write_string(r, 7, &row.equip)?;
        sheet.write_string(r, 8, &row.peripheral)?;
        sheet.write_string(r, 9, &row.includes)?;
    }
    println!("  GroupOptions : {} lignes", rows.len());
    Ok(())
}

fn write_unit_options(
    rows: &[UnitOptionRow],
    workbook: &mut Workbook,
    header: &Format,
) -> Result<()> {
    let sheet = workbook.add_worksheet();
    sheet.set_name("UnitOptions")?;
    write_header(
        sheet,
        header,
        &["unit_id", "option_id", "name", "disabled", "includes"],
    )?;
    for (i, row) in rows.iter().enumerate() {
        let r = (i + 1) as u32;
        sheet.write_number(r, 0, row.unit_id as f64)?;
        sheet.write_number(r, 1, row.option_id as f64)?;
        sheet.write_string(r, 2, &row.name)?;
        sheet.write_boolean(r, 3, row.disabled)?;
        sheet.write_string(r, 4, &row.includes)?;
    }
    println!("  UnitOptions : {} lignes", rows.len());
    Ok(())
}

fn write_includes(rows: &[IncludeRow], workbook: &mut Workbook, header: &Format) -> Result<()> {
    let sheet = workbook.add_worksheet();
    sheet.set_name("Includes")?;
    write_header(
        sheet,
        header,
        &[
            "unit_id",
            "unit_option_id",
            "included_group_id",
            "included_option_id",
            "quantity",
        ],
    )?;
    for (i, row) in rows.iter().enumerate() {
        let r = (i + 1) as u32;
        sheet.write_number(r, 0, row.unit_id as f64)?;
        sheet.write_number(r, 1, row.unit_option_id as f64)?;
        sheet.write_number(r, 2, row.included_group_id as f64)?;
        sheet.write_number(r, 3, row.included_option_id as f64)?;
        sheet.write_number(r, 4, row.quantity as f64)?;
    }
    println!("  Includes : {} lignes", rows.len());
    Ok(())
}

//! Objet TTS d'une figurine peinte : asset bundle, profil choisi, silhouette en état 2.

use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

use serde_json::{json, Map, Value};

/// Les périphériques importés à cet index sont des figurines séparées, pas un état.
const ANCILLARY_STATE_INDEX: i64 = 900;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelForm {
    pub id: i64,
    pub state_index: i64,
    pub nom_tts: String,
    pub description_tts: String,
    pub marker_name: Option<String>,
    pub silhouette_size: Option<String>,
    pub silhouette_type: Option<String>,
    pub mesh_url: Option<String>,
    pub marker_mesh_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelProfile {
    pub nom_tts: String,
    pub description_tts: String,
    pub marker_name: Option<String>,
    pub silhouette_size: Option<String>,
    pub silhouette_type: Option<String>,
    pub marker_mesh_url: Option<String>,
    pub forms: Vec<ModelForm>,
}

pub fn marker_label(name: Option<&str>, kind: Option<&str>, size: Option<&str>) -> String {
    if let Some(name) = name.map(str::trim).filter(|name| !name.is_empty()) {
        return name.to_string();
    }
    let size = size
        .map(str::trim)
        .filter(|size| !size.is_empty())
        .unwrap_or("2");
    match kind.unwrap_or("normale") {
        "camo-0" => format!("Camouflage S{size}"),
        "camo-3" => format!("Camouflage (-3) S{size}"),
        "camo-6" => format!("Camouflage (-6) S{size}"),
        "imp-1" => "IMP-1".to_string(),
        "imp-2" => "IMP-2".to_string(),
        _ => format!("Silhouette {size}"),
    }
}

/// Mesh le plus fréquent pour chaque nom de silhouette.
pub fn majority_meshes(
    pairs: impl IntoIterator<Item = (String, String)>,
) -> BTreeMap<String, String> {
    let mut counts: BTreeMap<(String, String), usize> = BTreeMap::new();
    for (name, url) in pairs {
        let name = name.trim();
        let url = url.trim();
        if name.is_empty() || url.is_empty() {
            continue;
        }
        *counts
            .entry((name.to_string(), url.to_string()))
            .or_default() += 1;
    }
    let mut best: BTreeMap<String, (usize, String)> = BTreeMap::new();
    for ((name, url), count) in counts {
        match best.get(&name) {
            Some((winner, _)) if *winner >= count => {}
            _ => {
                best.insert(name, (count, url));
            }
        }
    }
    best.into_iter()
        .map(|(name, (_, url))| (name, url))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiffuseColor {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

impl DiffuseColor {
    pub fn hex(self) -> String {
        format!(
            "{:02X}{:02X}{:02X}",
            color_byte(self.r),
            color_byte(self.g),
            color_byte(self.b)
        )
    }
}

/// `#RGB`, `RGB`, `#RRGGBB` ou `RRGGBB`.
pub fn parse_color(raw: &str) -> Option<DiffuseColor> {
    let text = raw.trim().trim_start_matches('#');
    let expanded = match text.len() {
        3 => {
            let chars: Vec<char> = text.chars().collect();
            if !chars.iter().all(|char| char.is_ascii_hexdigit()) {
                return None;
            }
            format!("{0}{0}{1}{1}{2}{2}", chars[0], chars[1], chars[2])
        }
        6 => text.to_string(),
        _ => return None,
    };
    if !expanded.chars().all(|char| char.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |start: usize| {
        u8::from_str_radix(&expanded[start..start + 2], 16).map(|value| f64::from(value) / 255.0)
    };
    Some(DiffuseColor {
        r: channel(0).ok()?,
        g: channel(2).ok()?,
        b: channel(4).ok()?,
    })
}

/// Couleur de `config/sectorial-colors.json` pour le slug de la sectorielle choisie.
pub fn sectorial_color_hex(slug: &str) -> Option<String> {
    let key = slug.trim().to_ascii_lowercase();
    if key.is_empty() {
        return None;
    }
    sectorial_colors()
        .get(&key)
        .and_then(|raw| parse_color(raw))
        .map(|color| color.hex())
}

fn sectorial_colors() -> &'static HashMap<String, String> {
    static COLORS: OnceLock<HashMap<String, String>> = OnceLock::new();
    COLORS.get_or_init(|| {
        let raw: HashMap<String, String> =
            serde_json::from_str(include_str!("../config/sectorial-colors.json"))
                .expect("config/sectorial-colors.json");
        raw.into_iter()
            .map(|(slug, color)| (slug.to_ascii_lowercase(), color))
            .collect()
    })
}

fn color_byte(channel: f64) -> u8 {
    (channel.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn paint_nickname(nickname: &str, hex: &str) -> String {
    let Some(rest) = nickname.strip_prefix('[') else {
        return nickname.to_string();
    };
    let Some((code, after)) = rest.split_once(']') else {
        return nickname.to_string();
    };
    if code.len() == 6 && code.chars().all(|char| char.is_ascii_hexdigit()) {
        format!("[{hex}]{after}")
    } else {
        nickname.to_string()
    }
}

pub fn saved_model(
    asset_bundle_url: &str,
    profile: &ModelProfile,
    meshes: &BTreeMap<String, String>,
    color: DiffuseColor,
) -> Value {
    let mut object = asset_bundle(
        asset_bundle_url,
        &profile.nom_tts,
        &profile.description_tts,
        color,
    );
    let mut states = Map::new();
    let mut index = 2_i64;
    push(&mut states, &mut index, silhouette(profile, meshes, color));
    let mut forms: Vec<&ModelForm> = profile
        .forms
        .iter()
        .filter(|form| form.state_index != ANCILLARY_STATE_INDEX)
        .collect();
    forms.sort_by_key(|form| (form.state_index, form.id));
    for form in forms {
        let body = match blank(form.mesh_url.as_deref()) {
            Some(mesh) => custom_mesh(mesh, &form.nom_tts, &form.description_tts, color),
            None => asset_bundle(
                asset_bundle_url,
                &form.nom_tts,
                &form.description_tts,
                color,
            ),
        };
        push(&mut states, &mut index, body);
        push(&mut states, &mut index, silhouette(form, meshes, color));
    }
    object
        .as_object_mut()
        .expect("objet TTS")
        .insert("States".to_string(), Value::Object(states));
    object
}

/// TTS charge un fichier importé comme une sauvegarde : `Grid` y est un
/// objet, pas le booléen de l'objet.
pub fn save_file(model: Value) -> Value {
    let name = model
        .get("Nickname")
        .and_then(|value| value.as_str())
        .unwrap_or("Figurine");
    json!({
        "SaveName": name,
        "GameMode": "",
        "Date": "",
        "Table": "",
        "Sky": "",
        "Note": "",
        "Rules": "",
        "PlayerTurn": "",
        "Grid": {
            "Type": 0,
            "Lines": false,
            "Snapping": false,
            "Offset": false,
            "xSize": 2.0,
            "ySize": 2.0
        },
        "ObjectStates": [model]
    })
}

trait SilhouetteSource {
    fn marker_name(&self) -> Option<&str>;
    fn silhouette_type(&self) -> Option<&str>;
    fn silhouette_size(&self) -> Option<&str>;
    fn marker_mesh_url(&self) -> Option<&str>;
}

impl SilhouetteSource for ModelProfile {
    fn marker_name(&self) -> Option<&str> {
        self.marker_name.as_deref()
    }
    fn silhouette_type(&self) -> Option<&str> {
        self.silhouette_type.as_deref()
    }
    fn silhouette_size(&self) -> Option<&str> {
        self.silhouette_size.as_deref()
    }
    fn marker_mesh_url(&self) -> Option<&str> {
        self.marker_mesh_url.as_deref()
    }
}

impl SilhouetteSource for ModelForm {
    fn marker_name(&self) -> Option<&str> {
        self.marker_name.as_deref()
    }
    fn silhouette_type(&self) -> Option<&str> {
        self.silhouette_type.as_deref()
    }
    fn silhouette_size(&self) -> Option<&str> {
        self.silhouette_size.as_deref()
    }
    fn marker_mesh_url(&self) -> Option<&str> {
        self.marker_mesh_url.as_deref()
    }
}

fn silhouette(
    source: &impl SilhouetteSource,
    meshes: &BTreeMap<String, String>,
    color: DiffuseColor,
) -> Value {
    let name = marker_label(
        source.marker_name(),
        source.silhouette_type(),
        source.silhouette_size(),
    );
    let mesh = meshes
        .get(&name)
        .cloned()
        .or_else(|| blank(source.marker_mesh_url()).map(str::to_string))
        .unwrap_or_default();
    custom_mesh(&mesh, &name, "", color)
}

fn push(states: &mut Map<String, Value>, index: &mut i64, object: Value) {
    states.insert(index.to_string(), object);
    *index += 1;
}

fn blank(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn asset_bundle(url: &str, nickname: &str, description: &str, color: DiffuseColor) -> Value {
    let nickname = paint_nickname(nickname, &color.hex());
    let mut object = shell("Custom_Assetbundle", &nickname, description, color);
    object.as_object_mut().expect("objet TTS").insert(
        "CustomAssetbundle".to_string(),
        json!({
            "AssetbundleURL": url,
            "AssetbundleSecondaryURL": "",
            "MaterialIndex": 0,
            "TypeIndex": 0,
            "LoopingEffectIndex": 0
        }),
    );
    object
}

fn custom_mesh(url: &str, nickname: &str, description: &str, color: DiffuseColor) -> Value {
    let nickname = paint_nickname(nickname, &color.hex());
    let mut object = shell("Custom_Model", &nickname, description, color);
    object.as_object_mut().expect("objet TTS").insert(
        "CustomMesh".to_string(),
        json!({
            "MeshURL": url,
            "DiffuseURL": "",
            "NormalURL": "",
            "ColliderURL": "",
            "Convex": true,
            "MaterialIndex": 0,
            "TypeIndex": 0,
            "CastShadows": true
        }),
    );
    object
}

fn shell(name: &str, nickname: &str, description: &str, color: DiffuseColor) -> Value {
    json!({
        "Name": name,
        "Transform": {
            "posX": 0.0,
            "posY": 0.0,
            "posZ": 0.0,
            "rotX": 0.0,
            "rotY": 0.0,
            "rotZ": 0.0,
            "scaleX": 1.0,
            "scaleY": 1.0,
            "scaleZ": 1.0
        },
        "Nickname": nickname,
        "Description": description,
        "GMNotes": "",
        "AltLookAngle": { "x": 0.0, "y": 0.0, "z": 0.0 },
        "ColorDiffuse": { "r": color.r, "g": color.g, "b": color.b },
        "LayoutGroupSortIndex": 0,
        "Value": 0,
        "Locked": false,
        "Grid": true,
        "Snap": true,
        "IgnoreFoW": false,
        "MeasureMovement": false,
        "DragSelectable": true,
        "Autoraise": true,
        "Sticky": true,
        "Tooltip": true,
        "GridProjection": false,
        "HideWhenFaceDown": false,
        "Hands": false,
        "LuaScript": "",
        "LuaScriptState": "",
        "XmlUI": ""
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sectorial_colors_follow_the_chosen_slug() {
        assert_eq!(
            sectorial_color_hex("military-orders").as_deref(),
            Some("00B0F2")
        );
        assert_eq!(
            sectorial_color_hex("Shindenbutai").as_deref(),
            Some("E52520")
        );
        assert_eq!(sectorial_color_hex("druze").as_deref(), Some("FFFFFF"));
        assert_eq!(
            sectorial_color_hex("contracted-back-up").as_deref(),
            Some("B3B2B2")
        );
        assert_eq!(sectorial_color_hex("inconnue"), None);
        assert!(sectorial_colors().len() >= 50);
        assert!(sectorial_colors()
            .values()
            .all(|color| parse_color(color).is_some()));
    }

    fn tint() -> DiffuseColor {
        parse_color("E6DA9B").expect("couleur")
    }

    fn profile() -> ModelProfile {
        ModelProfile {
            nom_tts: "[E6DA9B]YUAN YUAN[-] Chain Rifle".to_string(),
            description_tts: "[B]S[/B]: 2\n".to_string(),
            marker_name: Some("Silhouette 2".to_string()),
            silhouette_size: Some("2".to_string()),
            silhouette_type: Some("normale".to_string()),
            marker_mesh_url: Some("https://example.test/wrong.obj".to_string()),
            forms: vec![
                ModelForm {
                    id: 2,
                    state_index: 900,
                    nom_tts: "STALDRON".to_string(),
                    description_tts: "peripheral".to_string(),
                    marker_name: Some("Silhouette 1".to_string()),
                    silhouette_size: Some("1".to_string()),
                    silhouette_type: Some("normale".to_string()),
                    mesh_url: None,
                    marker_mesh_url: None,
                },
                ModelForm {
                    id: 1,
                    state_index: 1,
                    nom_tts: "[E6DA9B]DOG-WARRIOR FORM[-] Chain Rifle".to_string(),
                    description_tts: "[B]S[/B]: 6\n".to_string(),
                    marker_name: Some("Silhouette 6".to_string()),
                    silhouette_size: Some("6".to_string()),
                    silhouette_type: Some("normale".to_string()),
                    mesh_url: Some("https://example.test/dog.obj".to_string()),
                    marker_mesh_url: None,
                },
            ],
        }
    }

    #[test]
    fn the_imported_file_is_a_save_with_a_grid_object() {
        let save = save_file(saved_model(
            "https://example.test/bundle",
            &profile(),
            &BTreeMap::new(),
            tint(),
        ));
        assert!(save["Grid"].is_object());
        assert_eq!(save["Grid"]["Type"], 0);
        assert_eq!(save["ObjectStates"][0]["Name"], "Custom_Assetbundle");
        assert_eq!(save["ObjectStates"][0]["Grid"], true);
        assert_eq!(save["SaveName"], "[E6DA9B]YUAN YUAN[-] Chain Rifle");
    }

    #[test]
    fn a_chosen_color_tints_the_model_and_its_name() {
        let color = parse_color("#00b0f2").expect("couleur");
        let object = saved_model(
            "https://example.test/bundle",
            &profile(),
            &BTreeMap::new(),
            color,
        );
        assert_eq!(object["Nickname"], "[00B0F2]YUAN YUAN[-] Chain Rifle");
        assert_eq!(object["ColorDiffuse"]["r"], color.r);
        assert_eq!(object["ColorDiffuse"]["g"], color.g);
        assert_eq!(object["ColorDiffuse"]["b"], color.b);
        assert_eq!(object["States"]["2"]["Nickname"], "Silhouette 2");
        assert_eq!(object["States"]["2"]["ColorDiffuse"]["b"], color.b);
        assert_eq!(
            object["States"]["3"]["Nickname"],
            "[00B0F2]DOG-WARRIOR FORM[-] Chain Rifle"
        );
        assert_eq!(object["States"]["4"]["ColorDiffuse"]["r"], color.r);
    }

    #[test]
    fn state_two_is_the_catalog_silhouette_and_forms_follow() {
        let meshes = majority_meshes([
            (
                "Silhouette 2".to_string(),
                "https://example.test/s2.obj".to_string(),
            ),
            (
                "Silhouette 2".to_string(),
                "https://example.test/s2.obj".to_string(),
            ),
            (
                "Silhouette 2".to_string(),
                "https://example.test/other.obj".to_string(),
            ),
            (
                "Silhouette 6".to_string(),
                "https://example.test/s6.obj".to_string(),
            ),
        ]);
        let object = saved_model("https://example.test/bundle", &profile(), &meshes, tint());

        assert_eq!(object["Name"], "Custom_Assetbundle");
        assert_eq!(object["Nickname"], "[E6DA9B]YUAN YUAN[-] Chain Rifle");
        assert_eq!(object["Description"], "[B]S[/B]: 2\n");
        assert_eq!(
            object["CustomAssetbundle"]["AssetbundleURL"],
            "https://example.test/bundle"
        );
        assert_eq!(object["States"]["2"]["Name"], "Custom_Model");
        assert_eq!(object["States"]["2"]["Nickname"], "Silhouette 2");
        assert_eq!(object["States"]["2"]["Description"], "");
        assert_eq!(
            object["States"]["2"]["CustomMesh"]["MeshURL"],
            "https://example.test/s2.obj"
        );
        assert_eq!(
            object["States"]["3"]["Nickname"],
            "[E6DA9B]DOG-WARRIOR FORM[-] Chain Rifle"
        );
        assert_eq!(
            object["States"]["3"]["CustomMesh"]["MeshURL"],
            "https://example.test/dog.obj"
        );
        assert_eq!(object["States"]["4"]["Nickname"], "Silhouette 6");
        assert_eq!(
            object["States"]["4"]["CustomMesh"]["MeshURL"],
            "https://example.test/s6.obj"
        );
        assert!(object["States"].get("5").is_none());
        assert!(object["States"]
            .as_object()
            .unwrap()
            .values()
            .all(|state| state["Nickname"] != "STALDRON"));
    }

    #[test]
    fn a_form_without_mesh_keeps_the_painted_bundle() {
        let mut profile = profile();
        profile.forms[1].mesh_url = None;
        let object = saved_model(
            "https://example.test/bundle",
            &profile,
            &BTreeMap::new(),
            tint(),
        );
        assert_eq!(object["States"]["3"]["Name"], "Custom_Assetbundle");
        assert_eq!(
            object["States"]["3"]["CustomAssetbundle"]["AssetbundleURL"],
            "https://example.test/bundle"
        );
        assert_eq!(object["States"]["4"]["CustomMesh"]["MeshURL"], "");
    }

    #[test]
    fn derives_a_camouflage_marker_when_the_name_is_missing() {
        assert_eq!(
            marker_label(None, Some("camo-3"), Some("2")),
            "Camouflage (-3) S2"
        );
        let mut profile = profile();
        profile.marker_name = None;
        profile.silhouette_type = Some("camo-3".to_string());
        profile.marker_mesh_url = Some("https://example.test/camo.obj".to_string());
        profile.forms.clear();
        let object = saved_model(
            "https://example.test/bundle",
            &profile,
            &BTreeMap::new(),
            tint(),
        );
        assert_eq!(object["States"]["2"]["Nickname"], "Camouflage (-3) S2");
        assert_eq!(
            object["States"]["2"]["CustomMesh"]["MeshURL"],
            "https://example.test/camo.obj"
        );
    }
}

//! Déployables TTS clonés depuis `config/tts-props.json`, teints à la couleur du groupe.

use std::collections::BTreeSet;

use serde_json::{json, Value};

use crate::tts_object::DiffuseColor;
use crate::tts_profile::TtsProfileTrait;
use crate::tts_props::tts_props;

/// Clés de déployables déduites des traits (max 1 occurrence par clé).
pub fn deployable_keys_from_traits(traits: &[TtsProfileTrait]) -> BTreeSet<String> {
    let templates = &tts_props().deployables;
    let mut keys = BTreeSet::new();
    for trait_ in traits {
        if trait_.kind.eq_ignore_ascii_case("skill") {
            continue;
        }
        for key in keys_for_trait(trait_) {
            if templates.contains_key(key) {
                keys.insert(key.to_string());
            }
        }
    }
    keys
}

fn keys_for_trait(trait_: &TtsProfileTrait) -> Vec<&'static str> {
    let name = normalize_label(&trait_.name);
    let detail = trait_
        .detail
        .as_deref()
        .map(normalize_label)
        .unwrap_or_default();

    // Uniquement Pitcher ou Deployable Repeater (pas Repeater / KISS! / Tinbot…).
    if name == "pitcher" || name == "deployablerepeater" {
        return vec!["Repeater"];
    }
    if name == "deployablecover" {
        return vec![
            "Deployable Cover: Vitroferro",
            "Deployable Cover: Cutting Foam",
        ];
    }
    if name == "fastpanda" {
        return vec!["FastPanda"];
    }
    if name == "dazer" {
        return vec!["Dazer"];
    }
    if name == "crazykoala" {
        return vec!["CrazyKoala"];
    }
    if name == "wildparrot" {
        return vec!["WildParrot"];
    }
    if name == "madtraps" || name == "madtrap" {
        return vec!["MadTrap"];
    }
    if name == "dropbears" || name == "dropbear" {
        return vec!["Drop Bear"];
    }
    if name == "discoballer" || name == "discoball" {
        return vec!["Disco Ball"];
    }
    if name == "minedispenser" {
        return mine_from_dispenser_detail(&detail)
            .into_iter()
            .collect();
    }
    if let Some(mine) = mine_from_weapon_name(&name) {
        return vec![mine];
    }
    Vec::new()
}

fn mine_from_weapon_name(name: &str) -> Option<&'static str> {
    match name {
        "apmine" => Some("AP Mine"),
        "shockmine" => Some("Shock Mine"),
        "viralmine" => Some("Viral Mine"),
        "emmine" | "e/mmine" => Some("E/M Mine"),
        "paramine" | "para" => Some("Para Mine"),
        "monofilamentmine" => Some("Monofilament Mine"),
        "cybermine" | "cybermines" => Some("CyberMine"),
        _ => None,
    }
}

fn mine_from_dispenser_detail(detail: &str) -> Option<&'static str> {
    if detail.is_empty() {
        return None;
    }
    if detail.contains("viral") {
        return Some("Viral Mine");
    }
    if detail.contains("shock") {
        return Some("Shock Mine");
    }
    if detail.contains("cyber") {
        return Some("CyberMine");
    }
    if detail.contains("para") {
        return Some("Para Mine");
    }
    if detail.contains("mono") {
        return Some("Monofilament Mine");
    }
    if detail.contains("em") || detail.contains("e/m") {
        return Some("E/M Mine");
    }
    if detail.contains("ap") {
        return Some("AP Mine");
    }
    None
}

fn normalize_label(raw: &str) -> String {
    raw.chars()
        .filter(|char| char.is_ascii_alphanumeric() || *char == '/')
        .flat_map(char::to_lowercase)
        .collect()
}

/// Clone teinté des déployables pour un profil (au plus un objet par type).
pub fn tinted_deployables_for_traits(
    traits: &[TtsProfileTrait],
    color: DiffuseColor,
) -> Vec<Value> {
    let templates = &tts_props().deployables;
    deployable_keys_from_traits(traits)
        .into_iter()
        .filter_map(|key| {
            let template = templates.get(&key)?;
            Some(tint_deployable(template, color))
        })
        .collect()
}

fn tint_deployable(template: &Value, color: DiffuseColor) -> Value {
    let mut object = template.clone();
    apply_color_diffuse(&mut object, color);
    object
}

fn apply_color_diffuse(value: &mut Value, color: DiffuseColor) {
    match value {
        Value::Object(map) => {
            if map.contains_key("ColorDiffuse") {
                map.insert(
                    "ColorDiffuse".to_string(),
                    json!({ "r": color.r, "g": color.g, "b": color.b }),
                );
            }
            for child in map.values_mut() {
                apply_color_diffuse(child, color);
            }
        }
        Value::Array(items) => {
            for item in items {
                apply_color_diffuse(item, color);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trait_(kind: &str, name: &str, detail: Option<&str>) -> TtsProfileTrait {
        TtsProfileTrait {
            kind: kind.into(),
            name: name.into(),
            detail: detail.map(str::to_string),
        }
    }

    #[test]
    fn loads_deployable_catalog_without_speedballs() {
        let templates = &tts_props().deployables;
        assert!(templates.contains_key("Repeater"));
        assert!(templates.contains_key("AP Mine"));
        assert!(templates.contains_key("CrazyKoala"));
        assert!(templates.contains_key("Disco Ball"));
        assert!(templates.contains_key("Deployable Cover: Vitroferro"));
        assert!(templates.contains_key("Deployable Cover: Cutting Foam"));
        assert!(!templates.contains_key("Speedball A"));
        assert!(!templates.contains_key("Speedball B"));
        // Nouveau Repeater (avec état silhouette), pas l'ancien du sac Deployables.
        assert!(templates["Repeater"].get("States").and_then(Value::as_object).is_some());
    }

    #[test]
    fn pitcher_and_cover_and_dispenser_map_correctly() {
        let keys = deployable_keys_from_traits(&[
            trait_("weapon", "Pitcher", Some("+1B")),
            trait_("equipment", "Deployable Cover", None),
            trait_("weapon", "Mine Dispenser", Some("Viral")),
            trait_("skill", "Minelayer", Some("2")),
            trait_("weapon", "Crazykoala", None),
            trait_("equipment", "FastPanda", None),
            trait_("equipment", "Deployable Repeater", None),
            trait_("equipment", "KISS! (Repeater)", None),
            trait_("equipment", "Tinbot (Repeater)", None),
            trait_("equipment", "Repeater", None),
        ]);
        assert!(keys.contains("Repeater"));
        assert!(keys.contains("Deployable Cover: Vitroferro"));
        assert!(keys.contains("Deployable Cover: Cutting Foam"));
        assert!(keys.contains("Viral Mine"));
        assert!(keys.contains("CrazyKoala"));
        assert!(keys.contains("FastPanda"));
        assert!(!keys.iter().any(|key| key.contains("Speedball")));
        // Max 1 clé Repeater même avec Pitcher + Deployable Repeater.
        assert_eq!(keys.iter().filter(|key| *key == "Repeater").count(), 1);
    }

    #[test]
    fn only_pitcher_or_deployable_repeater_add_a_repeater() {
        assert!(deployable_keys_from_traits(&[trait_("weapon", "Pitcher", None)])
            .contains("Repeater"));
        assert!(deployable_keys_from_traits(&[trait_(
            "equipment",
            "Deployable Repeater",
            None
        )])
        .contains("Repeater"));
        assert!(!deployable_keys_from_traits(&[trait_("equipment", "Repeater", None)])
            .contains("Repeater"));
        assert!(!deployable_keys_from_traits(&[trait_(
            "equipment",
            "KISS! (Repeater)",
            None
        )])
        .contains("Repeater"));
        assert!(!deployable_keys_from_traits(&[trait_(
            "equipment",
            "Tinbot (Repeater)",
            None
        )])
        .contains("Repeater"));
    }

    #[test]
    fn ignores_minelayer_skill_alone() {
        let keys = deployable_keys_from_traits(&[trait_("skill", "Minelayer", Some("2"))]);
        assert!(keys.is_empty());
    }

    #[test]
    fn tints_silhouette_state_color() {
        let color = DiffuseColor {
            r: 1.0,
            g: 0.0,
            b: 0.0,
        };
        let objects = tinted_deployables_for_traits(&[trait_("weapon", "AP Mine", None)], color);
        assert_eq!(objects.len(), 1);
        let state2 = &objects[0]["States"]["2"]["ColorDiffuse"];
        assert_eq!(state2["r"], 1.0);
        assert_eq!(state2["g"], 0.0);
        assert_eq!(state2["b"], 0.0);
        assert_eq!(objects[0]["ColorDiffuse"]["r"], 1.0);
    }
}

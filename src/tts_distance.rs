//! Conversion des distances (MOV, Dodge(+…), etc.) pour l'export TTS.
//!
//! Les descriptions stockées sont en centimètres (1″ = 2,5 cm). On n'écrit
//! l'unité qu'à la génération du sac, selon le choix cm / ″.

/// Unité d'affichage pour les distances dans les descriptions TTS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistanceUnit {
    Inch,
    Cm,
}

const CM_PER_INCH: f64 = 2.5;

impl DistanceUnit {
    /// `"cm"` (défaut) ou `"inch"` / `"\""`.
    pub fn parse(raw: Option<&str>) -> Result<Self, String> {
        match raw.map(str::trim).filter(|value| !value.is_empty()) {
            None => Ok(Self::Cm),
            Some("cm") | Some("centimeter") | Some("centimetre") | Some("centimeters")
            | Some("centimetres") => Ok(Self::Cm),
            Some("inch") | Some("inches") | Some("in") | Some("\"") | Some("″") => Ok(Self::Inch),
            Some(other) => Err(format!(
                "unité de distance invalide « {other} » (cm ou inch)"
            )),
        }
    }
}

/// Réécrit MOV et les extras de distance (`(+2")`, `(+5cm)`, …) dans `unit`.
pub fn apply_distance_unit(description: &str, unit: DistanceUnit) -> String {
    let with_mov = rewrite_mov_lines(description, unit);
    rewrite_parenthetical_distances(&with_mov, unit)
}

fn rewrite_mov_lines(description: &str, unit: DistanceUnit) -> String {
    let ends_with_newline = description.ends_with('\n');
    let rewritten = description
        .lines()
        .map(|line| rewrite_mov_line(line, unit))
        .collect::<Vec<_>>()
        .join("\n");
    if ends_with_newline {
        format!("{rewritten}\n")
    } else {
        rewritten
    }
}

fn rewrite_mov_line(line: &str, unit: DistanceUnit) -> String {
    // [b]MOV[/b]: ou [B]MOV[/B]:
    let lower = line.to_ascii_lowercase();
    let Some(tag_pos) = lower.find("[b]mov[/b]:") else {
        return line.to_string();
    };
    let tag_len = "[b]mov[/b]:".len();
    let prefix = &line[..tag_pos + tag_len];
    let rest = line[tag_pos + tag_len..].trim_start();
    let Some(formatted) = rewrite_mov_value(rest, unit) else {
        return line.to_string();
    };
    format!("{prefix} {formatted}")
}

fn rewrite_mov_value(raw: &str, unit: DistanceUnit) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    // Unité commune en fin de valeur : 4-4" / 10-10cm
    let (body, shared_unit) = split_trailing_unit(raw);
    let parts: Vec<&str> = body.split('-').map(str::trim).collect();
    if parts.is_empty() || parts.iter().any(|part| part.is_empty()) {
        return None;
    }
    let mut centimetres = Vec::with_capacity(parts.len());
    for part in parts {
        let (number_part, part_unit) = split_trailing_unit(part);
        let token_unit = part_unit.or(shared_unit);
        let value = parse_number(number_part)?;
        // Profils stockés en cm : valeur nue = cm.
        let as_cm = match token_unit {
            Some(UnitKind::Inch) => value * CM_PER_INCH,
            Some(UnitKind::Cm) | None => value,
        };
        centimetres.push(as_cm);
    }
    let formatted = centimetres
        .iter()
        .map(|value| format_number(*value, unit))
        .collect::<Vec<_>>()
        .join("-");
    Some(format!("{}{}", formatted, unit_suffix(unit)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnitKind {
    Inch,
    Cm,
}

fn split_trailing_unit(raw: &str) -> (&str, Option<UnitKind>) {
    let trimmed = raw.trim_end();
    let lower = trimmed.to_ascii_lowercase();
    if lower.ends_with("cm") {
        return (trimmed[..trimmed.len() - 2].trim_end(), Some(UnitKind::Cm));
    }
    if trimmed.ends_with('″') || trimmed.ends_with('"') {
        return (&trimmed[..trimmed.len() - 1], Some(UnitKind::Inch));
    }
    if trimmed.ends_with("''") {
        return (&trimmed[..trimmed.len() - 2], Some(UnitKind::Inch));
    }
    if lower.ends_with("inches") {
        return (
            trimmed[..trimmed.len() - "inches".len()].trim_end(),
            Some(UnitKind::Inch),
        );
    }
    if lower.ends_with("inch") {
        return (
            trimmed[..trimmed.len() - "inch".len()].trim_end(),
            Some(UnitKind::Inch),
        );
    }
    if lower.ends_with("in")
        && trimmed
            .chars()
            .rev()
            .nth(2)
            .is_some_and(|c| c.is_ascii_digit() || c.is_whitespace())
    {
        return (
            trimmed[..trimmed.len() - 2].trim_end(),
            Some(UnitKind::Inch),
        );
    }
    (trimmed, None)
}

fn rewrite_parenthetical_distances(description: &str, unit: DistanceUnit) -> String {
    let mut result = String::with_capacity(description.len());
    let chars: Vec<char> = description.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '(' {
            result.push(chars[i]);
            i += 1;
            continue;
        }
        let mut depth = 1usize;
        let mut j = i + 1;
        while j < chars.len() {
            match chars[j] {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            j += 1;
        }
        if depth != 0 || j >= chars.len() {
            result.push(chars[i]);
            i += 1;
            continue;
        }
        let skill = skill_name_before(&chars, i);
        let inner: String = chars[i + 1..j].iter().collect();
        if let Some(rewritten) = rewrite_distance_extra(&inner, unit, skill.as_deref()) {
            result.push('(');
            result.push_str(&rewritten);
            result.push(')');
        } else {
            result.extend(chars[i..=j].iter().copied());
        }
        i = j + 1;
    }
    result
}

fn skill_name_before(chars: &[char], open_paren_idx: usize) -> Option<String> {
    let mut end = open_paren_idx;
    while end > 0 && chars[end - 1].is_whitespace() {
        end -= 1;
    }
    if end == 0 {
        return None;
    }
    let mut start = end;
    while start > 0 {
        let c = chars[start - 1];
        if c.is_alphabetic() || c == '-' || c == '_' || c == ' ' || c == '\'' {
            start -= 1;
        } else {
            break;
        }
    }
    let name: String = chars[start..end].iter().collect();
    let name = name.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn normalize_skill_name(name: &str) -> String {
    name.to_ascii_lowercase()
        .replace(['-', '_'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_cm_distance_skill(name: &str) -> bool {
    matches!(
        normalize_skill_name(name).as_str(),
        "dodge" | "super jump" | "jump"
    )
}

/// Valeurs Dodge / Super-Jump / Jump sans unité qui sont des distances en cm.
fn is_implicit_cm_distance(value: f64) -> bool {
    [2.5, 5.0, 7.5, 10.0]
        .iter()
        .any(|expected| (value - expected).abs() < 1e-6)
}

fn rewrite_distance_extra(
    inner: &str,
    unit: DistanceUnit,
    skill: Option<&str>,
) -> Option<String> {
    let trimmed = inner.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (sign, rest) = match trimmed.chars().next() {
        Some('+') => ("+", trimmed[1..].trim_start()),
        Some('-') => ("-", trimmed[1..].trim_start()),
        _ => ("", trimmed),
    };
    let (number_part, kind) = split_trailing_unit(rest);
    let value = parse_number(number_part)?;
    let centimetres = match kind {
        Some(UnitKind::Cm) => value,
        Some(UnitKind::Inch) => value * CM_PER_INCH,
        None => {
            // Dodge(+3), Mimetism(-3), Jump(PH=11)… : laisser tel quel.
            // Dodge/Super-Jump/Jump avec 2.5/5/7.5/10 : distances en cm.
            let skill = skill?;
            if !is_cm_distance_skill(skill) || !is_implicit_cm_distance(value) {
                return None;
            }
            value
        }
    };
    Some(format!(
        "{}{}{}",
        sign,
        format_number(centimetres, unit),
        unit_suffix(unit)
    ))
}

fn parse_number(raw: &str) -> Option<f64> {
    let normalized = raw.trim().replace(',', ".");
    if normalized.is_empty() {
        return None;
    }
    normalized.parse().ok()
}

/// `centimetres` : valeur canonique (stockage).
fn format_number(centimetres: f64, unit: DistanceUnit) -> String {
    let value = match unit {
        DistanceUnit::Cm => centimetres,
        DistanceUnit::Inch => centimetres / CM_PER_INCH,
    };
    let rounded = (value * 100.0).round() / 100.0;
    if (rounded - rounded.round()).abs() < 1e-9 {
        format!("{}", rounded.round() as i64)
    } else {
        let text = format!("{rounded:.2}");
        text.trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

fn unit_suffix(unit: DistanceUnit) -> &'static str {
    match unit {
        DistanceUnit::Inch => "\"",
        DistanceUnit::Cm => "cm",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_mov_from_cm_storage() {
        let raw = "[b]LI[/b]\n[b]MOV[/b]: 10-10\n[B]S[/B]: 2\n";
        assert_eq!(
            apply_distance_unit(raw, DistanceUnit::Cm),
            "[b]LI[/b]\n[b]MOV[/b]: 10-10cm\n[B]S[/B]: 2\n"
        );
        assert_eq!(
            apply_distance_unit(raw, DistanceUnit::Inch),
            "[b]LI[/b]\n[b]MOV[/b]: 4-4\"\n[B]S[/B]: 2\n"
        );
    }

    #[test]
    fn converts_dodge_distance_extras_both_ways() {
        let inches = "Skills --------- Dodge(+2\") ● Courage";
        assert_eq!(
            apply_distance_unit(inches, DistanceUnit::Cm),
            "Skills --------- Dodge(+5cm) ● Courage"
        );
        let cm = "Skills --------- Dodge(+5cm) ● Courage";
        assert_eq!(
            apply_distance_unit(cm, DistanceUnit::Inch),
            "Skills --------- Dodge(+2\") ● Courage"
        );
        assert_eq!(
            apply_distance_unit("Dodge(+2.5cm)", DistanceUnit::Inch),
            "Dodge(+1\")"
        );
    }

    #[test]
    fn converts_unlabelled_dodge_and_super_jump_cm_distances() {
        assert_eq!(
            apply_distance_unit("Dodge(+2.5)", DistanceUnit::Cm),
            "Dodge(+2.5cm)"
        );
        assert_eq!(
            apply_distance_unit("Dodge(+5)", DistanceUnit::Inch),
            "Dodge(+2\")"
        );
        assert_eq!(
            apply_distance_unit("Dodge(+7.5)", DistanceUnit::Inch),
            "Dodge(+3\")"
        );
        assert_eq!(
            apply_distance_unit("Dodge(+10)", DistanceUnit::Cm),
            "Dodge(+10cm)"
        );
        assert_eq!(
            apply_distance_unit("Super-Jump(7.5)", DistanceUnit::Cm),
            "Super-Jump(7.5cm)"
        );
        assert_eq!(
            apply_distance_unit("Super-Jump(+7.5)", DistanceUnit::Inch),
            "Super-Jump(+3\")"
        );
        assert_eq!(
            apply_distance_unit("Super Jump(5)", DistanceUnit::Inch),
            "Super Jump(2\")"
        );
        assert_eq!(
            apply_distance_unit("Jump(7.5)", DistanceUnit::Cm),
            "Jump(7.5cm)"
        );
        assert_eq!(
            apply_distance_unit("Jump(+7.5)", DistanceUnit::Inch),
            "Jump(+3\")"
        );
    }

    #[test]
    fn leaves_non_distance_mods_alone() {
        let raw =
            "Dodge(+3) ● Mimetism(-3) ● BS Attack(+1B) ● Dodge(+1SD) ● Super-Jump(Jet Propulsion) ● Jump(PH=11) ● Jump(+3)";
        assert_eq!(apply_distance_unit(raw, DistanceUnit::Cm), raw);
        assert_eq!(apply_distance_unit(raw, DistanceUnit::Inch), raw);
    }

    #[test]
    fn converts_mov_already_labelled() {
        assert_eq!(
            apply_distance_unit("[b]MOV[/b]: 4-4\"", DistanceUnit::Cm),
            "[b]MOV[/b]: 10-10cm"
        );
        assert_eq!(
            apply_distance_unit("[b]MOV[/b]: 10-10cm", DistanceUnit::Inch),
            "[b]MOV[/b]: 4-4\""
        );
    }

    #[test]
    fn parses_unit_aliases() {
        assert_eq!(DistanceUnit::parse(None).unwrap(), DistanceUnit::Cm);
        assert_eq!(DistanceUnit::parse(Some("cm")).unwrap(), DistanceUnit::Cm);
        assert_eq!(
            DistanceUnit::parse(Some("inch")).unwrap(),
            DistanceUnit::Inch
        );
        assert_eq!(DistanceUnit::parse(Some("\"")).unwrap(), DistanceUnit::Inch);
        assert!(DistanceUnit::parse(Some("yard")).is_err());
    }
}

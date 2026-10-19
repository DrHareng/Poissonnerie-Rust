//! Normalisation et parsing des codes de liste Infinity Army.
//!
//! Format binaire (Army 7), d'après Infinity-Army-Tools / Docs/Armycodes.md :
//! sectoral_id (VLI) · slug · nom · points · N groupes ·
//! chaque groupe : numéro, field_A, field_B (renfort), taille ·
//! chaque membre : 00 · unit_id · group · option · new_field · [mod_flag · custom?]
//!
//! Certaines listes omettent le `mod_flag` quand il vaut 0 : on tente d'abord le
//! format complet, puis le format sans `mod_flag`.

use anyhow::{bail, Context, Result};
use serde::Serialize;

const URL_MARKERS: &[&str] = &["army/list/", "army/infinity/list/"];

/// Entrée d'une liste décodée (une ligne Army Builder).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArmyListMember {
    pub unit_id: u32,
    pub group_id: u32,
    pub option_id: u32,
    pub combat_group: u32,
    pub reinforcements: bool,
    /// JSON brut de customisation Spec-Ops / Team-Ops, si présent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customization_json: Option<String>,
}

/// Liste Army entièrement décodée.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecodedArmyList {
    pub sectoral_id: u32,
    pub sectoral_slug: String,
    pub name: String,
    pub max_points: u32,
    pub members: Vec<ArmyListMember>,
}

/// Décode le roster complet (unités / options) depuis un code ou une URL Army.
pub fn parse_army_list(raw: &str) -> Result<DecodedArmyList> {
    let normalized =
        normalize_army_list_code(raw).context("code de liste Army manquant ou invalide")?;
    let binary =
        decode_army_payload(&normalized).context("impossible de décoder le code Army (base64)")?;
    decode_army_binary(&binary).context("format de liste Army non reconnu")
}

/// Extrait le code depuis un code brut ou une URL Army (query/hash ignorés).
/// Décode aussi les séquences percent-encoding (`%3D` → `=`).
pub fn normalize_army_list_code(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let lower = trimmed.to_ascii_lowercase();
    let mut code = trimmed;
    for marker in URL_MARKERS {
        if let Some(idx) = lower.find(marker) {
            code = &trimmed[idx + marker.len()..];
            break;
        }
    }

    let code = code
        .split(['?', '#'])
        .next()
        .unwrap_or(code)
        .trim()
        .trim_start_matches('/');
    if code.is_empty() {
        return None;
    }
    let code = percent_decode(code);
    let code = code.trim();
    if code.is_empty() {
        None
    } else {
        Some(code.to_string())
    }
}

/// Liste 1 obligatoire, liste 2 optionnelle. Si les deux sont présentes, elles doivent différer.
pub fn require_lists(list1: &str, list2: &str) -> Result<(String, Option<String>)> {
    let a = normalize_army_list_code(list1).context("indiquez le code de la liste 1")?;
    let b = normalize_army_list_code(list2);
    if let Some(ref code2) = b {
        if *code2 == a {
            bail!("les deux listes doivent être différentes");
        }
    }
    Ok((a, b))
}

/// Décode le code Army (base64) et lit le slug de faction.
pub fn parse_army_list_faction_slug(raw: &str) -> Option<String> {
    let normalized = normalize_army_list_code(raw)?;
    let binary = decode_army_payload(&normalized)?;
    find_faction_slug_in_binary(&binary).map(|header| header.slug)
}

/// Lit le nom de liste encodé après le slug de faction.
pub fn parse_army_list_name(raw: &str) -> Option<String> {
    let normalized = normalize_army_list_code(raw)?;
    let binary = decode_army_payload(&normalized)?;
    let header = find_faction_slug_in_binary(&binary)?;
    let name_offset = header.slug_offset + 1 + header.slug_len;
    read_length_prefixed_string(&binary, name_offset, 128)
}

fn decode_army_binary(data: &[u8]) -> Result<DecodedArmyList> {
    if let Ok(list) = decode_army_binary_with(data, true) {
        return Ok(list);
    }
    decode_army_binary_with(data, false)
}

fn decode_army_binary_with(data: &[u8], with_mod_flag: bool) -> Result<DecodedArmyList> {
    let mut cur = Cursor::new(data);
    let sectoral_id = cur.read_vli().context("ID de sectorielle")?;
    let sectoral_slug = cur
        .read_length_string(64)
        .context("slug de sectorielle")?;
    if !is_faction_slug(&sectoral_slug) {
        bail!("slug de sectorielle invalide");
    }
    let name = cur.read_byte_string(128).context("nom de liste")?;
    let max_points = cur.read_vli().context("limite de points")?;
    let group_count = cur.read_vli().context("nombre de groupes")?;
    if group_count == 0 || group_count > 8 {
        bail!("nombre de combat groups invalide ({group_count})");
    }

    let mut members = Vec::new();
    for _ in 0..group_count {
        let combat_group = cur.read_vli().context("numéro de combat group")?;
        let _field_a = cur.read_vli().context("field_A")?;
        let field_b = cur.read_vli().context("field_B")?;
        let size = cur.read_vli().context("taille du combat group")?;
        if size > 40 {
            bail!("combat group trop grand ({size})");
        }
        let reinforcements = field_b != 0;
        for _ in 0..size {
            members.push(
                cur.read_member(combat_group, reinforcements, with_mod_flag)
                    .context("membre de liste")?,
            );
        }
    }
    if !cur.is_empty() {
        bail!("octets restants après décodage ({})", cur.remaining());
    }
    Ok(DecodedArmyList {
        sectoral_id,
        sectoral_slug,
        name,
        max_points,
        members,
    })
}

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn is_empty(&self) -> bool {
        self.pos >= self.data.len()
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn read_u8(&mut self) -> Result<u8> {
        let byte = *self
            .data
            .get(self.pos)
            .context("fin inattendue du code Army")?;
        self.pos += 1;
        Ok(byte)
    }

    /// Entier VLQ Army : 1 octet si < 128, sinon short BE avec bit 15 levé.
    fn read_vli(&mut self) -> Result<u32> {
        let first = self.read_u8()?;
        if first < 0x80 {
            return Ok(u32::from(first));
        }
        let second = self.read_u8()?;
        let value = (u16::from(first) << 8) | u16::from(second);
        Ok(u32::from(value & 0x7fff))
    }

    fn read_length_string(&mut self, max_len: usize) -> Result<String> {
        let length = self.read_vli()? as usize;
        self.read_exact_string(length, max_len)
    }

    fn read_byte_string(&mut self, max_len: usize) -> Result<String> {
        let length = usize::from(self.read_u8()?);
        self.read_exact_string(length, max_len)
    }

    fn read_exact_string(&mut self, length: usize, max_len: usize) -> Result<String> {
        if length > max_len {
            bail!("chaîne trop longue ({length})");
        }
        if self.pos + length > self.data.len() {
            bail!("chaîne tronquée");
        }
        let bytes = &self.data[self.pos..self.pos + length];
        self.pos += length;
        let value = std::str::from_utf8(bytes)
            .context("chaîne UTF-8 invalide")?
            .trim()
            .to_string();
        Ok(value)
    }

    fn read_member(
        &mut self,
        combat_group: u32,
        reinforcements: bool,
        with_mod_flag: bool,
    ) -> Result<ArmyListMember> {
        let lead = self.read_u8()?;
        if lead != 0 {
            bail!("séparateur de membre invalide (0x{lead:02x})");
        }
        let unit_id = self.read_vli()?;
        let group_id = self.read_vli()?;
        let option_id = self.read_vli()?;
        let _new_field = self.read_vli()?;
        let mut customization_json = None;
        if with_mod_flag {
            let mod_flag = self.read_vli()?;
            if mod_flag != 0 {
                let _byte_e = self.read_vli()?;
                let length = self.read_vli()? as usize;
                if length > 8_192 {
                    bail!("customisation trop longue ({length})");
                }
                if self.pos + length > self.data.len() {
                    bail!("customisation tronquée");
                }
                let bytes = &self.data[self.pos..self.pos + length];
                self.pos += length;
                let json = std::str::from_utf8(bytes)
                    .context("customisation UTF-8 invalide")?
                    .to_string();
                customization_json = Some(json);
            }
        }
        Ok(ArmyListMember {
            unit_id,
            group_id,
            option_id,
            combat_group,
            reinforcements,
            customization_json,
        })
    }
}

struct FactionSlugHeader {
    slug_offset: usize,
    slug_len: usize,
    slug: String,
}

fn find_faction_slug_in_binary(binary: &[u8]) -> Option<FactionSlugHeader> {
    if binary.len() < 5 {
        return None;
    }

    for offset in 0..=std::cmp::min(6, binary.len().saturating_sub(2)) {
        let length = binary[offset] as usize;
        if !(3..=64).contains(&length) {
            continue;
        }
        if offset + 1 + length > binary.len() {
            continue;
        }
        let bytes = &binary[offset + 1..offset + 1 + length];
        if !bytes.iter().all(|b| (0x20..=0x7e).contains(b)) {
            continue;
        }
        let slug = String::from_utf8_lossy(bytes).into_owned();
        if is_faction_slug(&slug) {
            return Some(FactionSlugHeader {
                slug_offset: offset,
                slug_len: length,
                slug,
            });
        }
    }
    None
}

fn read_length_prefixed_string(binary: &[u8], offset: usize, max_len: usize) -> Option<String> {
    if offset >= binary.len() {
        return None;
    }
    let length = binary[offset] as usize;
    if length == 0 || length > max_len {
        return None;
    }
    if offset + 1 + length > binary.len() {
        return None;
    }
    let bytes = &binary[offset + 1..offset + 1 + length];
    std::str::from_utf8(bytes)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn is_faction_slug(slug: &str) -> bool {
    if slug.is_empty() {
        return false;
    }
    let mut chars = slug.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphanumeric() {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn decode_army_payload(value: &str) -> Option<Vec<u8>> {
    let decoded = percent_decode(value);
    if let Some(bytes) = base64_decode(&decoded) {
        return Some(bytes);
    }
    base64_decode(value)
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(a), Some(b)) = (from_hex(bytes[i + 1]), from_hex(bytes[i + 2])) {
                out.push((a << 4) | b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn from_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn base64_decode(value: &str) -> Option<Vec<u8>> {
    let cleaned: String = value
        .chars()
        .filter(|c| {
            c.is_ascii_alphanumeric()
                || *c == '+'
                || *c == '/'
                || *c == '='
                || *c == '-'
                || *c == '_'
        })
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            other => other,
        })
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    let pad = (4 - (cleaned.len() % 4)) % 4;
    let padded = format!("{cleaned}{}", "=".repeat(pad));
    base64_std_decode(&padded)
}

fn base64_std_decode(input: &str) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }

    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    let mut buf = [0u8; 4];
    let mut buf_len = 0;
    for &b in bytes {
        if b == b'=' {
            break;
        }
        buf[buf_len] = val(b)?;
        buf_len += 1;
        if buf_len == 4 {
            out.push((buf[0] << 2) | (buf[1] >> 4));
            out.push((buf[1] << 4) | (buf[2] >> 2));
            out.push((buf[2] << 6) | buf[3]);
            buf_len = 0;
        }
    }
    if buf_len == 2 {
        out.push((buf[0] << 2) | (buf[1] >> 4));
    } else if buf_len == 3 {
        out.push((buf[0] << 2) | (buf[1] >> 4));
        out.push((buf[1] << 4) | (buf[2] >> 2));
    } else if buf_len == 1 {
        return None;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_code_from_url() {
        let code =
            normalize_army_list_code("https://infinitytheuniverse.com/army/list/abc123?x=1#y");
        assert_eq!(code.as_deref(), Some("abc123"));
    }

    #[test]
    fn percent_decodes_and_rejects_identical_lists() {
        let url = "https://infinitytheuniverse.com/army/list/gZIQaGFzc2Fzc2luLWJhaHJhbRBOb3V2ZWF1IGdvIHRvIHYygSwCAQEACgCBMAECAACGNQECAACBRwEGAACBTgEBAACBLQELAACBLQEOAACBUQECAACBGwEBAACBTAECAACGCwEDAAIBAAUAgUgBAwAAgTABBgAAgT4BAQAAgVQBAwAAhgkBAgA%3D";
        let code = "gZIQaGFzc2Fzc2luLWJhaHJhbRBOb3V2ZWF1IGdvIHRvIHYygSwCAQEACgCBMAECAACGNQECAACBRwEGAACBTgEBAACBLQELAACBLQEOAACBUQECAACBGwEBAACBTAECAACGCwEDAAIBAAUAgUgBAwAAgTABBgAAgT4BAQAAgVQBAwAAhgkBAgA=";
        let a = normalize_army_list_code(url).unwrap();
        let b = normalize_army_list_code(code).unwrap();
        assert_eq!(a, b);
        assert!(require_lists(url, code).is_err());
    }

    #[test]
    fn extracts_list_name_from_code() {
        let code = "gZIQaGFzc2Fzc2luLWJhaHJhbRBOb3V2ZWF1IGdvIHRvIHYygSwCAQEACgCBMAECAACGNQECAACBRwEGAACBTgEBAACBLQELAACBLQEOAACBUQECAACBGwEBAACBTAECAACGCwEDAAIBAAUAgUgBAwAAgTABBgAAgT4BAQAAgVQBAwAAhgkBAgA=";
        assert_eq!(
            parse_army_list_faction_slug(code).as_deref(),
            Some("hassassin-bahram")
        );
        assert_eq!(
            parse_army_list_name(code).as_deref(),
            Some("Nouveau go to v2")
        );
    }

    #[test]
    fn decodes_hassassin_roster_without_mod_flag() {
        let code = "gZIQaGFzc2Fzc2luLWJhaHJhbRBOb3V2ZWF1IGdvIHRvIHYygSwCAQEACgCBMAECAACGNQECAACBRwEGAACBTgEBAACBLQELAACBLQEOAACBUQECAACBGwEBAACBTAECAACGCwEDAAIBAAUAgUgBAwAAgTABBgAAgT4BAQAAgVQBAwAAhgkBAgA=";
        let list = parse_army_list(code).unwrap();
        assert_eq!(list.sectoral_id, 402);
        assert_eq!(list.sectoral_slug, "hassassin-bahram");
        assert_eq!(list.name, "Nouveau go to v2");
        assert_eq!(list.max_points, 300);
        assert_eq!(list.members.len(), 15);
        assert_eq!(list.members[0].unit_id, 304);
        assert_eq!(list.members[0].group_id, 1);
        assert_eq!(list.members[0].option_id, 2);
        assert_eq!(list.members[0].combat_group, 1);
        assert_eq!(list.members[10].combat_group, 2);
        assert_eq!(list.members[10].unit_id, 328);
    }

    #[test]
    fn decodes_operations_roster_with_mod_flag() {
        // CODE_1 from Infinity-Army-Tools ArmylistDecodeTest
        let code = "gr8Kb3BlcmF0aW9ucwtTSW1wbGUgbGlzdIEsAQEBAAYAgkgBBgAAAIJTAQEAAACCUwEBAAAAglUBAgAAAIJVAgIAAACCVQQCAAA=";
        let list = parse_army_list(code).unwrap();
        assert_eq!(list.sectoral_id, 703);
        assert_eq!(list.sectoral_slug, "operations");
        assert_eq!(list.name, "SImple list");
        assert_eq!(list.max_points, 300);
        assert_eq!(list.members.len(), 6);
        assert_eq!(list.members[0].unit_id, 584);
        assert_eq!(list.members[0].option_id, 6);
        assert_eq!(
            list.members
                .iter()
                .map(|member| member.unit_id)
                .collect::<Vec<_>>(),
            vec![584, 595, 595, 597, 597, 597]
        );
    }

    #[test]
    fn decodes_customization_block() {
        // CODE_4 from Infinity-Army-Tools — un Spec-Ops customisé
        let code = "gr8Kb3BlcmF0aW9ucwtTSW1wbGUgbGlzdIEsAQEBAAEAh5ABAQABAXJbeyJ0eXBlIjoic3RhdCIsInN0YXQiOiJtb3ZlMCIsInEiOjE1fSx7InR5cGUiOiJzdGF0Iiwic3RhdCI6Im1vdmUxIiwicSI6MTB9LHsidHlwZSI6InNraWxsIiwiaWQiOjQwLCJleHRyYSI6WzFdfV0=";
        let list = parse_army_list(code).unwrap();
        assert_eq!(list.members.len(), 1);
        assert_eq!(list.members[0].unit_id, 1936);
        assert_eq!(list.members[0].group_id, 1);
        assert_eq!(list.members[0].option_id, 1);
        let json = list.members[0].customization_json.as_deref().unwrap();
        assert!(json.contains("move0"));
        assert!(json.contains("\"id\":40"));
    }
}

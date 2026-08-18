//! Normalisation et parsing des codes de liste Infinity Army.

use anyhow::{bail, Context, Result};

const URL_MARKERS: &[&str] = &[
    "army/list/",
    "army/infinity/list/",
];

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
            return Some(slug);
        }
    }
    None
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
        .filter(|c| c.is_ascii_alphanumeric() || *c == '+' || *c == '/' || *c == '=' || *c == '-' || *c == '_')
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
        let code = normalize_army_list_code(
            "https://infinitytheuniverse.com/army/list/abc123?x=1#y",
        );
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
}

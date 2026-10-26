//! Rattachement des figurines / prefs au schéma unités + loadouts TTS.

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};

/// Cible de matching : nom d'unité (ou forme) + loadout Army → id loadout TTS.
#[derive(Debug, Clone)]
struct LoadoutTarget {
    tts_loadout_id: i64,
    nom: String,
    loadout: String,
}

/// Relie les figurines orphelines (ou toutes) aux loadouts TTS.
/// Matching : exact (casse ignorée) puis flou (pluriel simple).
pub fn reattach_figurines_to_loadouts(conn: &Connection) -> Result<usize> {
    if !table_exists(conn, "tts_figurines")? || !table_exists(conn, "tts_loadouts")? {
        return Ok(0);
    }
    if !column_exists(conn, "tts_figurines", "tts_loadout_id")? {
        return Ok(0);
    }

    let targets = load_targets(conn)?;
    let mut stmt = conn.prepare(
        "
        SELECT id, COALESCE(legacy_nom, ''), COALESCE(legacy_loadout, ''), tts_loadout_id
        FROM tts_figurines
        ",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<i64>>(3)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("lecture figurines pour reattach")?;

    let mut linked = 0usize;
    for (id, legacy_nom, legacy_loadout, current) in rows {
        if current.is_some() {
            continue;
        }
        let Some(target) = match_target(&targets, &legacy_nom, &legacy_loadout) else {
            continue;
        };
        conn.execute(
            "UPDATE tts_figurines SET tts_loadout_id = ?1 WHERE id = ?2",
            params![target.tts_loadout_id, id],
        )?;
        linked += 1;
    }
    Ok(linked)
}

/// Migre les prefs `(isc_key, loadout_key)` → `(tts_loadout_id)` quand possible.
pub fn migrate_pick_prefs_to_loadouts(conn: &Connection) -> Result<usize> {
    if !table_exists(conn, "tts_figurine_pick_prefs")? {
        return Ok(0);
    }
    // Ancienne forme encore présente ?
    if !column_exists(conn, "tts_figurine_pick_prefs", "isc_key")? {
        return Ok(0);
    }
    if !column_exists(conn, "tts_figurine_pick_prefs", "tts_loadout_id")? {
        return Ok(0);
    }

    let targets = load_targets(conn)?;
    let mut stmt = conn.prepare(
        "
        SELECT user_id, isc_key, loadout_key, figurine_id, updated_at
        FROM tts_figurine_pick_prefs
        WHERE tts_loadout_id IS NULL
        ",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut migrated = 0usize;
    for (user_id, isc_key, loadout_key, _figurine_id, _updated_at) in rows {
        let Some(target) = match_target(&targets, &isc_key, &loadout_key) else {
            continue;
        };
        conn.execute(
            "
            UPDATE tts_figurine_pick_prefs
            SET tts_loadout_id = ?1
            WHERE user_id = ?2 AND isc_key = ?3 AND loadout_key = ?4
              AND tts_loadout_id IS NULL
            ",
            params![target.tts_loadout_id, user_id, isc_key, loadout_key],
        )?;
        migrated += 1;
    }
    Ok(migrated)
}

fn load_targets(conn: &Connection) -> Result<Vec<LoadoutTarget>> {
    let mut stmt = conn.prepare(
        "
        SELECT DISTINCT p.tts_loadout_id, u.nom, p.loadout
        FROM tts_profiles p
        JOIN tts_units u ON u.id = p.unit_id
        WHERE p.tts_loadout_id IS NOT NULL
        ",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(LoadoutTarget {
                tts_loadout_id: row.get(0)?,
                nom: row.get(1)?,
                loadout: row.get(2)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("lecture cibles loadout TTS")?;
    Ok(rows)
}

fn match_target<'a>(
    targets: &'a [LoadoutTarget],
    nom: &str,
    loadout: &str,
) -> Option<&'a LoadoutTarget> {
    let nom_key = nom.trim().to_ascii_lowercase();
    let loadout_key = loadout.trim().to_ascii_lowercase();
    if nom_key.is_empty() {
        return None;
    }

    // 1. Exact nom + loadout
    if let Some(hit) = targets.iter().find(|t| {
        t.nom.trim().eq_ignore_ascii_case(&nom_key)
            && t.loadout.trim().eq_ignore_ascii_case(&loadout_key)
    }) {
        return Some(hit);
    }

    // 2. Flou : même loadout + noms équivalents (pluriel / casse)
    let fuzzy: Vec<_> = targets
        .iter()
        .filter(|t| {
            names_fuzzy_equal(&nom_key, &t.nom.trim().to_ascii_lowercase())
                && t.loadout.trim().eq_ignore_ascii_case(&loadout_key)
        })
        .collect();
    if fuzzy.len() == 1 {
        return Some(fuzzy[0]);
    }

    // 3. Flou nom seul si un seul loadout TTS pour ce nom
    if loadout_key.is_empty() {
        let by_name: Vec<_> = targets
            .iter()
            .filter(|t| names_fuzzy_equal(&nom_key, &t.nom.trim().to_ascii_lowercase()))
            .collect();
        if by_name.len() == 1 {
            return Some(by_name[0]);
        }
    }

    None
}

/// Égalité floue : casse, pluriel anglais simple (s / ies).
pub fn names_fuzzy_equal(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let na = normalize_plural(a);
    let nb = normalize_plural(b);
    na == nb
}

fn normalize_plural(value: &str) -> String {
    let v = value.trim().to_ascii_lowercase();
    if v.ends_with("ies") && v.len() > 3 {
        return format!("{}y", &v[..v.len() - 3]);
    }
    if v.ends_with('s') && !v.ends_with("ss") && v.len() > 1 {
        return v[..v.len() - 1].to_string();
    }
    v
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
    let found: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            params![table],
            |row| row.get(0),
        )
        .optional()?;
    Ok(found.is_some())
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plural_fuzzy() {
        assert!(names_fuzzy_equal("nisses", "nisse"));
        assert!(names_fuzzy_equal("NISSE", "Nisses"));
        assert!(names_fuzzy_equal("fusiliers", "fusilier"));
        assert!(names_fuzzy_equal("palbots", "palbot"));
        assert!(!names_fuzzy_equal("nisse", "palbot"));
    }

    #[test]
    fn reattach_exact_fuzzy_and_orphan() {
        let dir = std::env::temp_dir().join(format!(
            "poissonnerie-reattach-{}-{}",
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
        crate::migrate::migrate(&conn).unwrap();

        conn.execute("INSERT INTO tts_units (nom) VALUES ('NISSE')", [])
            .unwrap();
        let unit_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO tts_loadouts (nom_tts, description_tts) VALUES ('n', 'd')",
            [],
        )
        .unwrap();
        let loadout_id = conn.last_insert_rowid();
        conn.execute(
            "
            INSERT INTO tts_profiles (
                faction_label, unit_id, tts_loadout_id, parent_profile_id,
                state_index, loadout, source_bag, source_index
            ) VALUES ('Ariadna', ?1, ?2, NULL, 0, 'Combi Rifle', 'army:1:1', 1)
            ",
            rusqlite::params![unit_id, loadout_id],
        )
        .unwrap();

        let now = 1i64;
        conn.execute(
            "
            INSERT INTO tts_figurines (
                isc, loadout, legacy_nom, legacy_loadout, painter,
                asset_bundle_url, created_at, updated_at
            ) VALUES
                ('NISSE', 'Combi Rifle', 'NISSE', 'Combi Rifle', 'A', 'https://a', ?1, ?1),
                ('Nisses', 'Combi Rifle', 'Nisses', 'Combi Rifle', 'B', 'https://b', ?1, ?1),
                ('ORPHAN', 'X', 'ORPHAN', 'X', 'C', 'https://c', ?1, ?1)
            ",
            rusqlite::params![now],
        )
        .unwrap();

        let linked = reattach_figurines_to_loadouts(&conn).unwrap();
        assert_eq!(linked, 2);
        let orphan: Option<i64> = conn
            .query_row(
                "SELECT tts_loadout_id FROM tts_figurines WHERE legacy_nom = 'ORPHAN'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(orphan.is_none());
        let exact: i64 = conn
            .query_row(
                "SELECT tts_loadout_id FROM tts_figurines WHERE legacy_nom = 'NISSE'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(exact, loadout_id);
        let fuzzy: i64 = conn
            .query_row(
                "SELECT tts_loadout_id FROM tts_figurines WHERE legacy_nom = 'Nisses'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(fuzzy, loadout_id);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

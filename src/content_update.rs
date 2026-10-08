use std::path::Path;
use std::sync::Mutex;

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::match_record::now_unix;
use crate::migrate::{migrate, row_text};

pub const KIND_MAP: &str = "map";
pub const KIND_SCENARIO: &str = "scenario";
pub const MAX_DESCRIPTION_CHARS: usize = 4_000;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ContentUpdate {
    pub id: i64,
    pub description: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MapContentUpdate {
    pub id: i64,
    pub map_id: i64,
    pub map_name: String,
    pub map_slug: String,
    pub description: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ScenarioContentUpdate {
    pub id: i64,
    pub scenario_id: i64,
    pub scenario_slug: String,
    pub scenario_name: String,
    pub description: String,
    pub created_at: u64,
}

pub const RECENT_SCENARIO_UPDATE_SECS: u64 = 30 * 24 * 60 * 60;

pub struct ContentUpdateStore {
    conn: Mutex<Connection>,
}

impl ContentUpdateStore {
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

    pub fn list_for_map(&self, map_id: i64) -> Result<Vec<ContentUpdate>> {
        let conn = self.conn.lock().unwrap();
        ensure_map(&conn, map_id)?;
        list_updates(&conn, KIND_MAP, map_id)
    }

    pub fn add_for_map(&self, map_id: i64, description: &str) -> Result<ContentUpdate> {
        let description = normalize_description(description)?;
        let conn = self.conn.lock().unwrap();
        ensure_map(&conn, map_id)?;
        insert_update(&conn, KIND_MAP, map_id, &description)
    }

    pub fn list_recent_maps(&self, limit: usize) -> Result<Vec<MapContentUpdate>> {
        let limit = limit.clamp(1, 50) as i64;
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "
            SELECT u.id, u.target_id, m.name, m.slug, u.description, u.created_at
            FROM content_updates u
            JOIN tts_maps m ON m.id = u.target_id
            WHERE u.target_kind = ?1
            ORDER BY u.created_at DESC, u.id DESC
            LIMIT ?2
            ",
        )?;
        let rows = stmt.query_map(params![KIND_MAP, limit], |row| {
            Ok(MapContentUpdate {
                id: row.get(0)?,
                map_id: row.get(1)?,
                map_name: row.get(2)?,
                map_slug: row_text(row, 3)?,
                description: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn delete_for_map(&self, map_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM content_updates WHERE target_kind = ?1 AND target_id = ?2",
            params![KIND_MAP, map_id],
        )?;
        Ok(())
    }

    pub fn list_recent_scenarios(
        &self,
        pack_slug: &str,
        max_age_secs: u64,
    ) -> Result<Vec<ScenarioContentUpdate>> {
        let cutoff = now_unix().saturating_sub(max_age_secs);
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "
            SELECT u.id, s.id, s.slug, s.name, u.description, u.created_at
            FROM content_updates u
            JOIN scenarios s ON s.id = u.target_id
            JOIN scenario_packs p ON p.id = s.pack_id
            WHERE u.target_kind = ?1
              AND p.slug = ?2
              AND u.created_at >= ?3
            ORDER BY u.created_at DESC, u.id DESC
            ",
        )?;
        let rows = stmt.query_map(params![KIND_SCENARIO, pack_slug, cutoff], |row| {
            Ok(ScenarioContentUpdate {
                id: row.get(0)?,
                scenario_id: row.get(1)?,
                scenario_slug: row_text(row, 2)?,
                scenario_name: row.get(3)?,
                description: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn list_for_scenario(&self, slug: &str) -> Result<Vec<ContentUpdate>> {
        let conn = self.conn.lock().unwrap();
        let scenario_id = scenario_id_by_slug(&conn, slug)?;
        list_updates(&conn, KIND_SCENARIO, scenario_id)
    }

    pub fn add_for_scenario(&self, slug: &str, description: &str) -> Result<ContentUpdate> {
        let description = normalize_description(description)?;
        let conn = self.conn.lock().unwrap();
        let scenario_id = scenario_id_by_slug(&conn, slug)?;
        insert_update(&conn, KIND_SCENARIO, scenario_id, &description)
    }
}

fn normalize_description(description: &str) -> Result<String> {
    let trimmed = description.trim();
    if trimmed.is_empty() {
        bail!("la description est requise");
    }
    if trimmed.chars().count() > MAX_DESCRIPTION_CHARS {
        bail!("la description est trop longue");
    }
    Ok(trimmed.to_string())
}

fn ensure_map(conn: &Connection, map_id: i64) -> Result<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM tts_maps WHERE id = ?1)",
        params![map_id],
        |row| row.get(0),
    )?;
    if !exists {
        bail!("map introuvable");
    }
    Ok(())
}

fn scenario_id_by_slug(conn: &Connection, slug: &str) -> Result<i64> {
    let slug = slug.trim();
    if slug.is_empty() {
        bail!("scénario introuvable");
    }
    conn.query_row(
        "SELECT id FROM scenarios WHERE slug = ?1",
        params![slug],
        |row| row.get(0),
    )
    .optional()?
    .context("scénario introuvable")
}

fn list_updates(conn: &Connection, kind: &str, target_id: i64) -> Result<Vec<ContentUpdate>> {
    let mut stmt = conn.prepare(
        "
        SELECT id, description, created_at
        FROM content_updates
        WHERE target_kind = ?1 AND target_id = ?2
        ORDER BY created_at DESC, id DESC
        ",
    )?;
    let rows = stmt.query_map(params![kind, target_id], |row| {
        Ok(ContentUpdate {
            id: row.get(0)?,
            description: row.get(1)?,
            created_at: row.get(2)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn insert_update(
    conn: &Connection,
    kind: &str,
    target_id: i64,
    description: &str,
) -> Result<ContentUpdate> {
    let created_at = now_unix();
    conn.execute(
        "
        INSERT INTO content_updates (target_kind, target_id, description, created_at)
        VALUES (?1, ?2, ?3, ?4)
        ",
        params![kind, target_id, description, created_at],
    )?;
    Ok(ContentUpdate {
        id: conn.last_insert_rowid(),
        description: description.to_string(),
        created_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

    fn temp_store() -> (ContentUpdateStore, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "poissonnerie-content-updates-{}-{}-{}.db",
            std::process::id(),
            now_unix(),
            TEST_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_file(&path);
        let store = ContentUpdateStore::open(&path).unwrap();
        (store, path)
    }

    #[test]
    fn stores_updates_newest_first_and_rejects_blank() {
        let (store, path) = temp_store();
        let conn = Connection::open(&path).unwrap();
        conn.execute(
            "
            INSERT INTO tts_maps (slug, name, sort_order, created_at, updated_at)
            VALUES ('quai', 'Quai', 0, 1, 1)
            ",
            [],
        )
        .unwrap();
        let map_id = conn.last_insert_rowid();
        drop(conn);

        let first = store.add_for_map(map_id, "  Première note  ").unwrap();
        assert_eq!(first.description, "Première note");
        let second = store.add_for_map(map_id, "Correctif du JSON").unwrap();

        let listed = store.list_for_map(map_id).unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].id, second.id);
        assert_eq!(listed[1].id, first.id);

        assert!(store.add_for_map(map_id, "   ").is_err());
        assert!(store.add_for_map(9_999, "note").is_err());

        let recent = store.list_recent_maps(10).unwrap();
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].id, second.id);
        assert_eq!(recent[0].map_name, "Quai");
        assert_eq!(recent[0].map_slug, "quai");

        let scenario = store
            .add_for_scenario("avant-poste", "Déploiement clarifié")
            .unwrap();
        assert_eq!(store.list_recent_maps(10).unwrap().len(), 2);

        let conn = Connection::open(&path).unwrap();
        let scenario_id: i64 = conn
            .query_row(
                "SELECT id FROM scenarios WHERE slug = 'avant-poste'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        conn.execute(
            "
            INSERT INTO content_updates (target_kind, target_id, description, created_at)
            VALUES ('scenario', ?1, 'ancienne', 10)
            ",
            params![scenario_id],
        )
        .unwrap();
        drop(conn);
        let recent_scenarios = store
            .list_recent_scenarios("poissonnerie-v2", RECENT_SCENARIO_UPDATE_SECS)
            .unwrap();
        assert!(recent_scenarios
            .iter()
            .any(|item| item.description == "Déploiement clarifié"));
        assert!(recent_scenarios
            .iter()
            .all(|item| item.description != "ancienne"));
        let scenarios = store.list_for_scenario("avant-poste").unwrap();
        assert_eq!(scenarios[0], scenario);
        assert_eq!(scenarios[1].description, "ancienne");
        assert!(store.list_for_scenario("inconnu").is_err());

        store.delete_for_map(map_id).unwrap();
        assert!(store.list_for_map(map_id).unwrap().is_empty());

        drop(store);
        let _ = std::fs::remove_file(path);
    }
}

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::Deserialize;

use crate::migrate::migrate;

const METADATA_URL: &str = "https://api.corvusbelli.com/army/infinity/fr/metadata";
const ORIGIN: &str = "https://infinityuniverse.com";
const REFERER: &str = "https://infinityuniverse.com/army/infinity";
const SHORT_NAMES_PATH: &str = "data/army-short-names.json";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct Army {
    pub id: u32,
    pub parent_id: u32,
    pub name: String,
    pub slug: String,
    pub logo_url: String,
    pub discontinued: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_name: Option<String>,
}

/// Sectorielles de renfort (id se terminant par 99) — exclues des listes de saisie.
pub fn is_reinforcement_sectorial(id: u32) -> bool {
    id % 100 == 99
}

/// Factions généralistes (id se terminant par 01).
pub fn is_generalist(id: u32) -> bool {
    id % 100 == 1
}

pub fn is_listable(army: &Army) -> bool {
    !is_reinforcement_sectorial(army.id)
}

pub fn default_db_path() -> PathBuf {
    PathBuf::from("data/poissonnerie.db")
}

#[derive(Debug, Deserialize)]
struct MetadataResponse {
    factions: Vec<FactionResponse>,
}

#[derive(Debug, Deserialize)]
struct FactionResponse {
    id: u32,
    parent: u32,
    name: String,
    slug: String,
    logo: String,
    #[serde(default)]
    discontinued: bool,
}

pub struct ArmyStore {
    conn: Mutex<Connection>,
}

impl ArmyStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("impossible de créer {}", parent.display()))?;
        }

        let conn = Connection::open(path)
            .with_context(|| format!("impossible d'ouvrir {}", path.display()))?;
        migrate(&conn)?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_schema()?;
        store.seed_short_names()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
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
            CREATE INDEX IF NOT EXISTS idx_armies_parent ON armies(parent_id);
            ",
        )?;
        if !army_column_exists(&conn, "short_name")? {
            conn.execute("ALTER TABLE armies ADD COLUMN short_name TEXT", [])?;
        }
        Ok(())
    }

    fn seed_short_names(&self) -> Result<()> {
        let path = Path::new(SHORT_NAMES_PATH);
        if !path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("impossible de lire {}", path.display()))?;
        let seeds: HashMap<String, String> =
            serde_json::from_str(&content).context("JSON invalide dans army-short-names.json")?;

        let conn = self.conn.lock().unwrap();
        for (slug, short_name) in seeds {
            let short = short_name.trim();
            if short.is_empty() {
                continue;
            }
            conn.execute(
                "
                UPDATE armies
                SET short_name = ?1
                WHERE lower(slug) = lower(?2) AND short_name IS NULL
                ",
                params![short, slug],
            )?;
        }
        Ok(())
    }

    pub fn upsert(&self, army: &Army, synced_at: u64) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "
            INSERT INTO armies (id, parent_id, name, slug, logo_url, discontinued, synced_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(id) DO UPDATE SET
                parent_id = excluded.parent_id,
                name = excluded.name,
                slug = excluded.slug,
                logo_url = excluded.logo_url,
                discontinued = excluded.discontinued,
                synced_at = excluded.synced_at
            ",
            params![
                army.id,
                army.parent_id,
                army.name,
                army.slug,
                army.logo_url,
                army.discontinued as i32,
                synced_at,
            ],
        )?;
        Ok(())
    }

    pub fn get(&self, id: u32) -> Result<Option<Army>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "
            SELECT id, parent_id, name, slug, logo_url, discontinued, short_name
            FROM armies
            WHERE id = ?1
            ",
        )?;

        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(row_to_army(row)?));
        }
        Ok(None)
    }

    pub fn get_by_slug(&self, slug: &str) -> Result<Option<Army>> {
        let key = slug.trim().to_ascii_lowercase();
        if key.is_empty() {
            return Ok(None);
        }
        Ok(self
            .list_selectable()?
            .into_iter()
            .find(|army| army.slug.eq_ignore_ascii_case(&key)))
    }

    pub fn list_all(&self) -> Result<Vec<Army>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "
            SELECT id, parent_id, name, slug, logo_url, discontinued, short_name
            FROM armies
            ORDER BY parent_id, name
            ",
        )?;

        let armies = stmt
            .query_map([], row_to_army)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(armies)
    }

    pub fn list_selectable(&self) -> Result<Vec<Army>> {
        Ok(self
            .list_all()?
            .into_iter()
            .filter(is_listable)
            .collect())
    }

    pub fn validate_selectable_id(&self, id: u32) -> Result<()> {
        let army = self
            .get(id)?
            .with_context(|| format!("armée introuvable : id {}", id))?;

        if !is_listable(&army) {
            anyhow::bail!(
                "l'armée « {} » (id {}) n'est pas sélectionnable",
                army.name,
                army.id
            );
        }

        Ok(())
    }
}

fn row_to_army(row: &rusqlite::Row<'_>) -> rusqlite::Result<Army> {
    Ok(Army {
        id: row.get(0)?,
        parent_id: row.get(1)?,
        name: row.get(2)?,
        slug: row.get(3)?,
        logo_url: row.get(4)?,
        discontinued: row.get::<_, i32>(5)? != 0,
        short_name: row.get(6)?,
    })
}

fn army_column_exists(conn: &Connection, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare("PRAGMA table_info(armies)")?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn fetch_factions_from_api() -> Result<Vec<Army>> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("poissonnerie-elo/0.1")
        .build()?;

    let response = client
        .get(METADATA_URL)
        .header("Origin", ORIGIN)
        .header("Referer", REFERER)
        .send()
        .context("échec de la requête vers l'API Infinity Army")?;

    if !response.status().is_success() {
        anyhow::bail!(
            "l'API Infinity Army a répondu avec le statut {}",
            response.status()
        );
    }

    let metadata: MetadataResponse = response
        .json()
        .context("réponse JSON invalide de l'API Infinity Army")?;

    Ok(metadata
        .factions
        .into_iter()
        .map(|faction| Army {
            id: faction.id,
            parent_id: faction.parent,
            name: faction.name,
            slug: faction.slug,
            logo_url: faction.logo,
            discontinued: faction.discontinued,
            short_name: None,
        })
        .collect())
}

pub fn sync_armies(store: &ArmyStore) -> Result<SyncReport> {
    let factions = fetch_factions_from_api()?;
    let synced_at = crate::match_record::now_unix();
    let mut stored = 0usize;
    let mut skipped_reinforcement = 0usize;

    for army in &factions {
        if is_reinforcement_sectorial(army.id) {
            skipped_reinforcement += 1;
            continue;
        }
        store.upsert(army, synced_at)?;
        stored += 1;
    }

    store.seed_short_names()?;

    Ok(SyncReport {
        fetched: factions.len(),
        stored,
        skipped_reinforcement,
        selectable: store.list_selectable()?.len(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncReport {
    pub fetched: usize,
    pub stored: usize,
    pub skipped_reinforcement: usize,
    pub selectable: usize,
}

#[cfg(test)]
impl ArmyStore {
    fn set_short_name(&self, id: u32, short_name: &str) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "UPDATE armies SET short_name = ?1 WHERE id = ?2",
            params![short_name, id],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reinforcement_sectorials_end_with_99() {
        assert!(is_reinforcement_sectorial(199));
        assert!(is_reinforcement_sectorial(1099));
        assert!(is_reinforcement_sectorial(1199));
        assert!(!is_reinforcement_sectorial(101));
        assert!(!is_reinforcement_sectorial(998));
    }

    #[test]
    fn generalists_end_with_01() {
        assert!(is_generalist(101));
        assert!(is_generalist(1101));
        assert!(!is_generalist(102));
    }

    #[test]
    fn listable_excludes_only_reinforcement_sectorials() {
        let reinforcement = Army {
            id: 199,
            parent_id: 191,
            name: "Code: Capital".into(),
            slug: "code-capital".into(),
            logo_url: "https://example.com/logo.svg".into(),
            discontinued: false,
            short_name: None,
        };
        assert!(!is_listable(&reinforcement));

        let discontinued = Army {
            id: 102,
            parent_id: 101,
            name: "Test".into(),
            slug: "test".into(),
            logo_url: "https://example.com/logo.svg".into(),
            discontinued: true,
            short_name: None,
        };
        assert!(is_listable(&discontinued));
    }

    #[test]
    fn store_roundtrip() {
        let dir = std::env::temp_dir().join(format!(
            "poissonnerie-army-test-{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("armies.db");

        let store = ArmyStore::open(&path).unwrap();
        let army = Army {
            id: 101,
            parent_id: 101,
            name: "PanOcéanie".into(),
            slug: "panoceania".into(),
            logo_url: "https://example.com/panoceania.svg".into(),
            discontinued: false,
            short_name: None,
        };
        store.upsert(&army, 1).unwrap();

        let loaded = store.get(101).unwrap().unwrap();
        assert_eq!(loaded, army);

        store.set_short_name(101, "Pano").unwrap();
        let updated = Army {
            name: "PanOcéanie (API)".into(),
            short_name: Some("Pano".into()),
            ..army.clone()
        };
        store.upsert(&updated, 2).unwrap();
        let after_sync = store.get(101).unwrap().unwrap();
        assert_eq!(after_sync.name, "PanOcéanie (API)");
        assert_eq!(after_sync.short_name.as_deref(), Some("Pano"));
        assert_eq!(store.list_selectable().unwrap().len(), 1);

        let _ = std::fs::remove_dir_all(dir);
    }
}

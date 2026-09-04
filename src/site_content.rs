use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::match_record::now_unix;
use crate::migrate::migrate;

pub const RESSOURCES_KEY: &str = "ressources";

const DEFAULT_RESSOURCES_MD: &str = "\
## Liens utiles

Ajoutez ici les ressources de la communauté (liens, guides, outils…).

- [Infinity the Game](https://infinitythegame.com)
- [Army](https://army.infinitythegame.com)
";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SiteContent {
    pub key: String,
    pub body_md: String,
    pub updated_at: u64,
}

pub struct SiteContentStore {
    conn: Mutex<Connection>,
}

impl SiteContentStore {
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

    pub fn get(&self, key: &str) -> Result<SiteContent> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "
            SELECT key, body_md, updated_at
            FROM site_content
            WHERE key = ?1
            ",
        )?;
        let row = stmt
            .query_row(params![key], |row| {
                Ok(SiteContent {
                    key: row.get(0)?,
                    body_md: row.get(1)?,
                    updated_at: row.get(2)?,
                })
            })
            .optional_or_default(key)?;
        Ok(row)
    }

    pub fn update(&self, key: &str, body_md: &str) -> Result<SiteContent> {
        let now = now_unix();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "
            INSERT INTO site_content (key, body_md, updated_at)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(key) DO UPDATE SET
                body_md = excluded.body_md,
                updated_at = excluded.updated_at
            ",
            params![key, body_md, now],
        )?;
        Ok(SiteContent {
            key: key.to_string(),
            body_md: body_md.to_string(),
            updated_at: now,
        })
    }
}

trait OptionalOrDefault {
    fn optional_or_default(self, key: &str) -> Result<SiteContent>;
}

impl OptionalOrDefault for Result<SiteContent, rusqlite::Error> {
    fn optional_or_default(self, key: &str) -> Result<SiteContent> {
        match self {
            Ok(content) => Ok(content),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(SiteContent {
                key: key.to_string(),
                body_md: String::new(),
                updated_at: 0,
            }),
            Err(error) => Err(error.into()),
        }
    }
}

pub fn seed_ressources_content(conn: &Connection) -> Result<()> {
    let now = now_unix();
    conn.execute(
        "
        INSERT OR IGNORE INTO site_content (key, body_md, updated_at)
        VALUES (?1, ?2, ?3)
        ",
        params![RESSOURCES_KEY, DEFAULT_RESSOURCES_MD, now],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_db() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "poissonnerie-site-content-{}-{}.db",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn get_and_update_ressources() {
        let path = temp_db();
        let _ = std::fs::remove_file(&path);
        let store = SiteContentStore::open(&path).unwrap();

        let initial = store.get(RESSOURCES_KEY).unwrap();
        assert!(!initial.body_md.is_empty());

        let updated = store
            .update(RESSOURCES_KEY, "## Nouveau\n\n- [Lien](https://example.com)")
            .unwrap();
        assert!(updated.body_md.contains("Nouveau"));
        assert_eq!(store.get(RESSOURCES_KEY).unwrap().body_md, updated.body_md);

        let _ = std::fs::remove_file(&path);
    }
}

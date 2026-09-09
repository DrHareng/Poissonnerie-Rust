use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use rusqlite::Connection;

use crate::dauphine::{DauphineEdition, EditionStatus};
use crate::migrate::migrate;

pub struct DauphineStore {
    conn: Mutex<Connection>,
}

impl DauphineStore {
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

    pub fn list_editions(&self) -> Result<Vec<DauphineEdition>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "
            SELECT id, slug, title, year, status, tagline, body_md, created_at, updated_at
            FROM dauphine_editions
            ORDER BY year DESC, id DESC
            ",
        )?;
        let rows = stmt.query_map([], |row| {
            let status_raw: String = row.get(4)?;
            Ok(DauphineEdition {
                id: row.get(0)?,
                slug: row.get(1)?,
                title: row.get(2)?,
                year: row.get(3)?,
                status: EditionStatus::parse(&status_raw).unwrap_or(EditionStatus::Draft),
                tagline: row.get(5)?,
                body_md: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })?;

        let mut editions = Vec::new();
        for row in rows {
            editions.push(row?);
        }
        Ok(editions)
    }
}


use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::migrate::migrate;
use crate::store::normalize_name;

pub fn strip_scenario_prefix(name: &str) -> String {
    let trimmed = name.trim();
    let Some((prefix, rest)) = trimmed.split_once(':') else {
        return trimmed.to_string();
    };

    if prefix.trim().len() == 1 {
        return rest.trim().to_string();
    }

    trimmed.to_string()
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Scenario {
    pub id: i64,
    pub name: String,
    pub usage_count: u32,
}

pub struct ScenarioStore {
    conn: Mutex<Connection>,
}

impl ScenarioStore {
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

    pub fn list(&self, query: Option<&str>, limit: usize) -> Result<Vec<Scenario>> {
        let conn = self.conn.lock().unwrap();
        let limit = limit.clamp(1, 100) as i64;

        if let Some(q) = query.filter(|value| !value.trim().is_empty()) {
            let pattern = format!("%{}%", q.trim().to_lowercase());
            let mut stmt = conn.prepare(
                "
                SELECT id, name, usage_count
                FROM scenarios
                WHERE name_key LIKE ?1
                ORDER BY usage_count DESC, name ASC
                LIMIT ?2
                ",
            )?;
            let rows = stmt.query_map(params![pattern, limit], row_to_scenario)?;
            return rows.collect::<Result<Vec<_>, _>>().map_err(Into::into);
        }

        let mut stmt = conn.prepare(
            "
            SELECT id, name, usage_count
            FROM scenarios
            ORDER BY usage_count DESC, name ASC
            LIMIT ?1
            ",
        )?;
        let rows = stmt.query_map(params![limit], row_to_scenario)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_or_create(&self, name: &str) -> Result<Scenario> {
        let trimmed = strip_scenario_prefix(name);
        if trimmed.is_empty() {
            anyhow::bail!("indiquez un nom de scénario");
        }

        let key = normalize_name(&trimmed);
        let conn = self.conn.lock().unwrap();

        if let Some(existing) = self.get_by_key_in_conn(&conn, &key)? {
            conn.execute(
                "UPDATE scenarios SET usage_count = usage_count + 1 WHERE id = ?1",
                params![existing.id],
            )?;
            return Ok(Scenario {
                usage_count: existing.usage_count + 1,
                ..existing
            });
        }

        conn.execute(
            "INSERT INTO scenarios (name, name_key, usage_count) VALUES (?1, ?2, 1)",
            params![&trimmed, key],
        )?;
        let id = conn.last_insert_rowid();
        Ok(Scenario {
            id,
            name: trimmed,
            usage_count: 1,
        })
    }

    pub fn increment_usage(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE scenarios SET usage_count = usage_count + 1 WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    fn get_by_key_in_conn(&self, conn: &Connection, key: &str) -> Result<Option<Scenario>> {
        let mut stmt = conn.prepare(
            "SELECT id, name, usage_count FROM scenarios WHERE name_key = ?1",
        )?;
        let mut rows = stmt.query(params![key])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(row_to_scenario(row)?));
        }
        Ok(None)
    }
}

fn row_to_scenario(row: &rusqlite::Row<'_>) -> rusqlite::Result<Scenario> {
    Ok(Scenario {
        id: row.get(0)?,
        name: row.get(1)?,
        usage_count: row.get(2)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_scenario_prefix_removes_letter_prefix() {
        assert_eq!(
            strip_scenario_prefix("A : Scène de crime"),
            "Scène de crime"
        );
        assert_eq!(
            strip_scenario_prefix("Le combat de l'esprit"),
            "Le combat de l'esprit"
        );
    }
}

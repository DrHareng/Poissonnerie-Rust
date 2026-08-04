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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub map_filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pack_id: Option<i64>,
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
        let limit = limit.clamp(1, 200) as i64;

        if let Some(q) = query.filter(|value| !value.trim().is_empty()) {
            let pattern = format!("%{}%", q.trim().to_lowercase());
            let mut stmt = conn.prepare(
                "
                SELECT id, name, usage_count, slug, map_filename, pack_id
                FROM scenarios
                WHERE name_key LIKE ?1
                ORDER BY sort_order ASC, name ASC
                LIMIT ?2
                ",
            )?;
            let rows = stmt.query_map(params![pattern, limit], row_to_scenario)?;
            return rows.collect::<Result<Vec<_>, _>>().map_err(Into::into);
        }

        let mut stmt = conn.prepare(
            "
            SELECT id, name, usage_count, slug, map_filename, pack_id
            FROM scenarios
            ORDER BY sort_order ASC, name ASC
            LIMIT ?1
            ",
        )?;
        let rows = stmt.query_map(params![limit], row_to_scenario)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get(&self, id: i64) -> Result<Option<Scenario>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "
            SELECT id, name, usage_count, slug, map_filename, pack_id
            FROM scenarios
            WHERE id = ?1
            ",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(row_to_scenario(row)?));
        }
        Ok(None)
    }

    pub fn pack_page(
        &self,
        slug: &str,
    ) -> Result<Option<crate::scenario_pack::ScenarioPackPage>> {
        let conn = self.conn.lock().unwrap();
        crate::scenario_pack::get_pack_page(&conn, slug)
    }

    pub fn pack_secondaries(
        &self,
        pack_slug: &str,
    ) -> Result<Vec<crate::scenario_pack::SecondaryObjective>> {
        let conn = self.conn.lock().unwrap();
        crate::scenario_pack::list_secondaries(&conn, pack_slug)
    }

    pub fn pack_common_rules(
        &self,
        pack_slug: &str,
    ) -> Result<Vec<crate::scenario_pack::CommonRule>> {
        let conn = self.conn.lock().unwrap();
        crate::scenario_pack::list_common_rules(&conn, pack_slug)
    }

    pub fn scenario_detail(
        &self,
        slug: &str,
    ) -> Result<Option<crate::scenario_pack::ScenarioDetail>> {
        let conn = self.conn.lock().unwrap();
        crate::scenario_pack::get_scenario_detail(&conn, slug)
    }

    pub fn update_pack_preamble(
        &self,
        slug: &str,
        preamble_md: &str,
    ) -> Result<Option<crate::scenario_pack::ScenarioPack>> {
        let conn = self.conn.lock().unwrap();
        crate::scenario_pack::update_pack_preamble(&conn, slug, preamble_md)
    }

    pub fn update_secondary(
        &self,
        pack_slug: &str,
        secondary_slug: &str,
        name: &str,
        body_md: &str,
    ) -> Result<Option<crate::scenario_pack::SecondaryObjective>> {
        let conn = self.conn.lock().unwrap();
        crate::scenario_pack::update_secondary(&conn, pack_slug, secondary_slug, name, body_md)
    }

    pub fn update_common_rule(
        &self,
        pack_slug: &str,
        rule_slug: &str,
        name: &str,
        body_md: &str,
    ) -> Result<Option<crate::scenario_pack::CommonRule>> {
        let conn = self.conn.lock().unwrap();
        crate::scenario_pack::update_common_rule(&conn, pack_slug, rule_slug, name, body_md)
    }

    pub fn update_scenario_content(
        &self,
        pack_slug: &str,
        scenario_slug: &str,
        patch: &crate::scenario_pack::UpdateScenarioContentRequest,
    ) -> Result<Option<crate::scenario_pack::ScenarioDetail>> {
        let conn = self.conn.lock().unwrap();
        crate::scenario_pack::update_scenario_content(&conn, pack_slug, scenario_slug, patch)
    }

    pub fn increment_usage(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let updated = conn.execute(
            "UPDATE scenarios SET usage_count = usage_count + 1 WHERE id = ?1",
            params![id],
        )?;
        if updated == 0 {
            anyhow::bail!("scénario introuvable");
        }
        Ok(())
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
            slug: None,
            map_filename: None,
            pack_id: None,
        })
    }

    fn get_by_key_in_conn(&self, conn: &Connection, key: &str) -> Result<Option<Scenario>> {
        let mut stmt = conn.prepare(
            "
            SELECT id, name, usage_count, slug, map_filename, pack_id
            FROM scenarios
            WHERE name_key = ?1
            ",
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
        slug: row.get(3)?,
        map_filename: row.get(4)?,
        pack_id: row.get(5)?,
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

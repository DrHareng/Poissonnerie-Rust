use std::path::Path;
use std::sync::Mutex;

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::match_record::now_unix;
use crate::migrate::migrate;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ReportTemplate {
    pub id: i64,
    pub name: String,
    pub body_md: String,
    pub created_at: u64,
    pub updated_at: u64,
}

pub struct ReportTemplateStore {
    conn: Mutex<Connection>,
}

impl ReportTemplateStore {
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

    pub fn list_for_user(&self, user_id: i64) -> Result<Vec<ReportTemplate>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "
            SELECT id, name, body_md, created_at, updated_at
            FROM report_templates
            WHERE user_id = ?1
            ORDER BY updated_at DESC, id DESC
            ",
        )?;
        let rows = stmt.query_map(params![user_id], |row| {
            Ok(ReportTemplate {
                id: row.get(0)?,
                name: row.get(1)?,
                body_md: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;
        let mut templates = Vec::new();
        for row in rows {
            templates.push(row?);
        }
        Ok(templates)
    }

    pub fn create(&self, user_id: i64, name: &str, body_md: &str) -> Result<ReportTemplate> {
        let name = validate_template_name(name)?;
        let body_md = validate_template_body(body_md)?;
        let now = now_unix();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "
            INSERT INTO report_templates (user_id, name, body_md, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?4)
            ",
            params![user_id, name, body_md, now],
        )?;
        let id = conn.last_insert_rowid();
        Ok(ReportTemplate {
            id,
            name,
            body_md,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn update(
        &self,
        user_id: i64,
        id: i64,
        name: &str,
        body_md: &str,
    ) -> Result<ReportTemplate> {
        let name = validate_template_name(name)?;
        let body_md = validate_template_body(body_md)?;
        let now = now_unix();
        let conn = self.conn.lock().unwrap();
        let n = conn.execute(
            "
            UPDATE report_templates
            SET name = ?1, body_md = ?2, updated_at = ?3
            WHERE id = ?4 AND user_id = ?5
            ",
            params![name, body_md, now, id, user_id],
        )?;
        if n == 0 {
            bail!("modèle introuvable");
        }
        let created_at: u64 = conn.query_row(
            "SELECT created_at FROM report_templates WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        Ok(ReportTemplate {
            id,
            name,
            body_md,
            created_at,
            updated_at: now,
        })
    }

    pub fn delete(&self, user_id: i64, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let n = conn.execute(
            "DELETE FROM report_templates WHERE id = ?1 AND user_id = ?2",
            params![id, user_id],
        )?;
        if n == 0 {
            bail!("modèle introuvable");
        }
        Ok(())
    }
}

fn validate_template_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        bail!("le nom du modèle est requis");
    }
    if name.chars().count() > 80 {
        bail!("le nom du modèle est trop long");
    }
    Ok(name.to_string())
}

fn validate_template_body(body_md: &str) -> Result<String> {
    if body_md.chars().count() > 80_000 {
        bail!("le modèle est trop long");
    }
    Ok(body_md.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_list_update_delete_roundtrip() {
        let path = std::env::temp_dir().join(format!(
            "poissonnerie-report-templates-{}-{}",
            std::process::id(),
            now_unix()
        ));
        let _ = std::fs::remove_file(&path);
        let store = ReportTemplateStore::open(&path).unwrap();
        let created = store.create(1, " Classique ", "## T1\n").unwrap();
        assert_eq!(created.name, "Classique");
        assert_eq!(store.list_for_user(1).unwrap().len(), 1);
        assert!(store.list_for_user(2).unwrap().is_empty());

        let updated = store
            .update(1, created.id, "Court", "## Résumé\n")
            .unwrap();
        assert_eq!(updated.body_md, "## Résumé\n");
        store.delete(1, created.id).unwrap();
        assert!(store.list_for_user(1).unwrap().is_empty());
        let _ = std::fs::remove_file(path);
    }
}

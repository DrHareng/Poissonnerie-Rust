use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::migrate::{migrate, row_opt_text};
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

pub const MAX_SECONDARY_IMAGE_BYTES: usize = 8 * 1024 * 1024;

pub struct ScenarioStore {
    conn: Mutex<Connection>,
    uploads_root: PathBuf,
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
            uploads_root: uploads_root_from_db(path),
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
        image: Option<&[u8]>,
    ) -> Result<Option<crate::scenario_pack::SecondaryObjective>> {
        if let Some(bytes) = image {
            self.replace_secondary_image(pack_slug, secondary_slug, name, body_md, bytes)
        } else {
            let conn = self.conn.lock().unwrap();
            crate::scenario_pack::update_secondary(
                &conn,
                pack_slug,
                secondary_slug,
                name,
                body_md,
                None,
            )
        }
    }

    pub fn secondary_image_path(
        &self,
        pack_slug: &str,
        secondary_slug: &str,
    ) -> Result<Option<PathBuf>> {
        safe_path_slug(pack_slug)?;
        safe_path_slug(secondary_slug)?;
        let filename = {
            let conn = self.conn.lock().unwrap();
            match crate::scenario_pack::find_secondary_image_filename(
                &conn,
                pack_slug,
                secondary_slug,
            )? {
                Some(Some(name)) => name,
                _ => return Ok(None),
            }
        };
        let filename = safe_stored_filename(&filename)?;
        let path = self
            .uploads_root
            .join("secondaries")
            .join(pack_slug)
            .join(filename);
        if path.is_file() {
            Ok(Some(path))
        } else {
            Ok(None)
        }
    }

    fn replace_secondary_image(
        &self,
        pack_slug: &str,
        secondary_slug: &str,
        name: &str,
        body_md: &str,
        bytes: &[u8],
    ) -> Result<Option<crate::scenario_pack::SecondaryObjective>> {
        safe_path_slug(pack_slug)?;
        safe_path_slug(secondary_slug)?;
        let ext = sniff_secondary_image(bytes)?;
        let filename = format!("{secondary_slug}.{ext}");
        let previous = {
            let conn = self.conn.lock().unwrap();
            match crate::scenario_pack::find_secondary_image_filename(
                &conn,
                pack_slug,
                secondary_slug,
            )? {
                None => return Ok(None),
                Some(name) => name,
            }
        };

        let dir = self.uploads_root.join("secondaries").join(pack_slug);
        fs::create_dir_all(&dir)
            .with_context(|| format!("impossible de créer {}", dir.display()))?;
        let path = dir.join(&filename);
        fs::write(&path, bytes).with_context(|| format!("impossible d'écrire {}", path.display()))?;

        let updated_at = now_unix();
        let result = {
            let conn = self.conn.lock().unwrap();
            crate::scenario_pack::update_secondary(
                &conn,
                pack_slug,
                secondary_slug,
                name,
                body_md,
                Some((&filename, updated_at)),
            )
        };
        match result {
            Ok(value) => {
                if value.is_none() {
                    if previous.as_deref() != Some(filename.as_str()) {
                        let _ = fs::remove_file(&path);
                    }
                } else if let Some(old) = previous.as_deref() {
                    if old != filename {
                        if let Ok(old_name) = safe_stored_filename(old) {
                            let _ = fs::remove_file(dir.join(old_name));
                        }
                    }
                }
                Ok(value)
            }
            Err(error) => {
                if previous.as_deref() != Some(filename.as_str()) {
                    let _ = fs::remove_file(&path);
                }
                Err(error)
            }
        }
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

fn uploads_root_from_db(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join("uploads")
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn safe_path_slug(slug: &str) -> Result<()> {
    if slug.is_empty()
        || slug.len() > 80
        || !slug
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    {
        anyhow::bail!("identifiant invalide");
    }
    Ok(())
}

fn safe_stored_filename(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty()
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains("..")
        || trimmed.contains('\0')
    {
        anyhow::bail!("nom de fichier invalide");
    }
    Ok(trimmed.to_string())
}

fn sniff_secondary_image(bytes: &[u8]) -> Result<&'static str> {
    if bytes.is_empty() {
        anyhow::bail!("image vide");
    }
    if bytes.len() > MAX_SECONDARY_IMAGE_BYTES {
        anyhow::bail!("l'image dépasse {MAX_SECONDARY_IMAGE_BYTES} octets");
    }
    let format = image::guess_format(bytes).map_err(|_| {
        anyhow::anyhow!("format d'image non supporté (png, jpg, gif, webp)")
    })?;
    match format {
        image::ImageFormat::Png => Ok("png"),
        image::ImageFormat::Jpeg => Ok("jpg"),
        image::ImageFormat::Gif => Ok("gif"),
        image::ImageFormat::WebP => Ok("webp"),
        _ => anyhow::bail!("format d'image non supporté (png, jpg, gif, webp)"),
    }
}

fn row_to_scenario(row: &rusqlite::Row<'_>) -> rusqlite::Result<Scenario> {
    Ok(Scenario {
        id: row.get(0)?,
        name: row.get(1)?,
        usage_count: row.get(2)?,
        slug: row_opt_text(row, 3)?,
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

    #[test]
    fn update_secondary_overwrites_text_and_image() {
        let dir = std::env::temp_dir().join(format!(
            "poissonnerie-secondary-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let store = ScenarioStore::open(&dir.join("test.db")).unwrap();
        let pack = crate::scenario_pack::DEFAULT_PACK_SLUG;

        let png = encode_image(image::ImageFormat::Png);
        let updated = store
            .update_secondary(pack, "enlevement", "Enlèvement modifié", "nouveau texte", Some(&png))
            .unwrap()
            .unwrap();
        assert_eq!(updated.name, "Enlèvement modifié");
        assert_eq!(updated.body_md, "nouveau texte");
        let image_url = updated.image_url.expect("image");
        assert!(image_url.contains("/secondaries/enlevement/image?v="));

        let png_path = store.secondary_image_path(pack, "enlevement").unwrap().unwrap();
        assert_eq!(
            png_path.extension().and_then(|ext| ext.to_str()),
            Some("png")
        );

        let jpeg = encode_image(image::ImageFormat::Jpeg);
        let replaced = store
            .update_secondary(pack, "enlevement", "Encore", "texte jpeg", Some(&jpeg))
            .unwrap()
            .unwrap();
        assert!(replaced.image_url.is_some());
        assert!(!png_path.exists());
        let jpeg_path = store.secondary_image_path(pack, "enlevement").unwrap().unwrap();
        assert_eq!(
            jpeg_path.extension().and_then(|ext| ext.to_str()),
            Some("jpg")
        );

        let text_only = store
            .update_secondary(pack, "enlevement", "Texte seul", "inchangé", None)
            .unwrap()
            .unwrap();
        assert_eq!(text_only.name, "Texte seul");
        assert_eq!(text_only.body_md, "inchangé");
        assert!(text_only.image_url.is_some());
        assert!(jpeg_path.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn encode_image(format: image::ImageFormat) -> Vec<u8> {
        let image = if format == image::ImageFormat::Jpeg {
            image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
                1,
                1,
                image::Rgb([4, 5, 6]),
            ))
        } else {
            image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
                1,
                1,
                image::Rgba([1, 2, 3, 255]),
            ))
        };
        let mut bytes = Vec::new();
        image
            .write_to(&mut std::io::Cursor::new(&mut bytes), format)
            .unwrap();
        bytes
    }
}

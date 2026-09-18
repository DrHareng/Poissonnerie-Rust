use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::match_record::now_unix;
use crate::migrate::migrate;

pub const MAX_JSON_BYTES: usize = 15 * 1024 * 1024;
pub const MAX_PICTURE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_UPDATE_CHARS: usize = 80_000;
pub const MAX_REPORT_CHARS: usize = 8_000;
pub const MAX_NAME_CHARS: usize = 80;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TtsMapSummary {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub has_json: bool,
    pub json_filename: Option<String>,
    pub picture_count: i64,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TtsMapPicture {
    pub id: i64,
    pub filename: String,
    pub original_name: String,
    pub url: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TtsMapDetail {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub json_filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_url: Option<String>,
    pub pictures: Vec<TtsMapPicture>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TtsModuleUpdate {
    pub id: i64,
    pub body_md: String,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TtsContentImage {
    pub label: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TtsMapReport {
    pub id: i64,
    pub map_id: i64,
    pub map_name: String,
    pub map_slug: String,
    pub reporter_user_id: i64,
    pub reporter_display_name: String,
    pub description: String,
    pub image_filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TtsMapVariant {
    pub id: i64,
    pub map_id: i64,
    pub map_name: String,
    pub map_slug: String,
    pub scenario_id: i64,
    pub scenario_name: String,
    pub scenario_slug: String,
    pub tournament_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tournament_name: Option<String>,
    pub json_filename: String,
    pub json_url: String,
    pub created_at: u64,
    pub updated_at: u64,
}

pub struct TtsMapStore {
    conn: Mutex<Connection>,
    uploads_root: PathBuf,
}

impl TtsMapStore {
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

    pub fn list_maps(&self) -> Result<Vec<TtsMapSummary>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "
            SELECT
                m.id, m.slug, m.name, m.json_filename,
                (SELECT COUNT(*) FROM tts_map_pictures p WHERE p.map_id = m.id),
                m.created_at, m.updated_at
            FROM tts_maps m
            ORDER BY m.sort_order ASC, m.name COLLATE NOCASE ASC
            ",
        )?;
        let rows = stmt.query_map([], |row| {
            let json_filename: Option<String> = row.get(3)?;
            Ok(TtsMapSummary {
                id: row.get(0)?,
                slug: row.get(1)?,
                name: row.get(2)?,
                has_json: json_filename
                    .as_deref()
                    .is_some_and(|name| !name.trim().is_empty()),
                json_filename,
                picture_count: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_map(&self, id: i64) -> Result<Option<TtsMapDetail>> {
        let conn = self.conn.lock().unwrap();
        self.get_map_in_conn(&conn, id)
    }

    pub fn get_map_by_slug(&self, slug: &str) -> Result<Option<TtsMapDetail>> {
        let conn = self.conn.lock().unwrap();
        let id: Option<i64> = conn
            .query_row(
                "SELECT id FROM tts_maps WHERE slug = ?1",
                params![slug],
                |row| row.get(0),
            )
            .optional()?;
        let Some(id) = id else {
            return Ok(None);
        };
        self.get_map_in_conn(&conn, id)
    }

    pub fn create_map(&self, name: &str) -> Result<TtsMapDetail> {
        let name = validate_map_name(name)?;
        let base_slug = slugify(&name);
        let now = now_unix();
        let conn = self.conn.lock().unwrap();
        let slug = unique_slug(&conn, &base_slug)?;
        let sort_order: i64 = conn.query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM tts_maps",
            [],
            |row| row.get(0),
        )?;
        conn.execute(
            "
            INSERT INTO tts_maps (slug, name, sort_order, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?4)
            ",
            params![slug, name, sort_order, now],
        )?;
        let id = conn.last_insert_rowid();
        self.get_map_in_conn(&conn, id)?
            .with_context(|| "map introuvable après création")
    }

    pub fn rename_map(&self, id: i64, name: &str) -> Result<TtsMapDetail> {
        let name = validate_map_name(name)?;
        let now = now_unix();
        let conn = self.conn.lock().unwrap();
        let n = conn.execute(
            "
            UPDATE tts_maps
            SET name = ?1, updated_at = ?2
            WHERE id = ?3
            ",
            params![name, now, id],
        )?;
        if n == 0 {
            bail!("map introuvable");
        }
        self.get_map_in_conn(&conn, id)?
            .with_context(|| "map introuvable après mise à jour")
    }

    pub fn delete_map(&self, id: i64) -> Result<()> {
        let dir = self.map_dir(id);
        let conn = self.conn.lock().unwrap();
        let report_ids: Vec<i64> = {
            let mut stmt = conn.prepare("SELECT id FROM tts_map_reports WHERE map_id = ?1")?;
            let rows = stmt.query_map(params![id], |row| row.get(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        let variant_ids: Vec<i64> = {
            let mut stmt = conn.prepare("SELECT id FROM tts_map_variants WHERE map_id = ?1")?;
            let rows = stmt.query_map(params![id], |row| row.get(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        conn.execute("DELETE FROM tts_map_reports WHERE map_id = ?1", params![id])?;
        conn.execute("DELETE FROM tts_map_variants WHERE map_id = ?1", params![id])?;
        conn.execute("DELETE FROM tts_map_pictures WHERE map_id = ?1", params![id])?;
        let n = conn.execute("DELETE FROM tts_maps WHERE id = ?1", params![id])?;
        if n == 0 {
            bail!("map introuvable");
        }
        drop(conn);
        for variant_id in variant_ids {
            let dir = self.variant_dir(variant_id);
            if dir.exists() {
                let _ = fs::remove_dir_all(&dir);
            }
        }
        for report_id in report_ids {
            let report_dir = self.report_dir(report_id);
            if report_dir.exists() {
                let _ = fs::remove_dir_all(&report_dir);
            }
        }
        if dir.exists() {
            let _ = fs::remove_dir_all(&dir);
        }
        Ok(())
    }

    pub fn save_json(&self, id: i64, original_name: &str, bytes: &[u8]) -> Result<TtsMapDetail> {
        if bytes.len() > MAX_JSON_BYTES {
            bail!("le JSON dépasse {MAX_JSON_BYTES} octets");
        }
        serde_json::from_slice::<serde_json::Value>(bytes)
            .context("le fichier n'est pas un JSON valide")?;
        let filename = sanitize_filename(original_name, "map.json")?;
        if !filename.to_ascii_lowercase().ends_with(".json") {
            bail!("le fichier JSON doit avoir l'extension .json");
        }
        self.ensure_map_exists(id)?;
        let dir = self.json_dir(id);
        fs::create_dir_all(&dir)
            .with_context(|| format!("impossible de créer {}", dir.display()))?;
        if dir.exists() {
            for entry in fs::read_dir(&dir)? {
                let entry = entry?;
                if entry.path().is_file() {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
        let path = dir.join(&filename);
        fs::write(&path, bytes)
            .with_context(|| format!("impossible d'écrire {}", path.display()))?;
        let now = now_unix();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "
            UPDATE tts_maps
            SET json_filename = ?1, updated_at = ?2
            WHERE id = ?3
            ",
            params![filename, now, id],
        )?;
        self.get_map_in_conn(&conn, id)?
            .with_context(|| "map introuvable après upload JSON")
    }

    pub fn json_file_path(&self, id: i64) -> Result<Option<(PathBuf, String)>> {
        let conn = self.conn.lock().unwrap();
        let filename: Option<String> = match conn.query_row(
            "SELECT json_filename FROM tts_maps WHERE id = ?1",
            params![id],
            |row| row.get(0),
        ) {
            Ok(value) => value,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let Some(filename) = filename.filter(|name| !name.trim().is_empty()) else {
            return Ok(None);
        };
        Ok(Some((self.json_dir(id).join(&filename), filename)))
    }

    pub fn add_picture(
        &self,
        id: i64,
        original_name: &str,
        bytes: &[u8],
    ) -> Result<TtsMapDetail> {
        if bytes.len() > MAX_PICTURE_BYTES {
            bail!("l'image dépasse {MAX_PICTURE_BYTES} octets");
        }
        let filename = unique_picture_filename(original_name)?;
        self.ensure_map_exists(id)?;
        let dir = self.pictures_dir(id);
        fs::create_dir_all(&dir)
            .with_context(|| format!("impossible de créer {}", dir.display()))?;
        let mut stored = filename.clone();
        let mut n = 2;
        while dir.join(&stored).exists() {
            stored = append_filename_suffix(&filename, n);
            n += 1;
            if n > 1000 {
                bail!("impossible de stocker l'image");
            }
        }
        let path = dir.join(&stored);
        fs::write(&path, bytes)
            .with_context(|| format!("impossible d'écrire {}", path.display()))?;
        let now = now_unix();
        let conn = self.conn.lock().unwrap();
        let original = Path::new(original_name)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&stored);
        conn.execute(
            "
            INSERT INTO tts_map_pictures (map_id, filename, original_name, created_at)
            VALUES (?1, ?2, ?3, ?4)
            ",
            params![id, stored, original, now],
        )?;
        conn.execute(
            "UPDATE tts_maps SET updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        self.get_map_in_conn(&conn, id)?
            .with_context(|| "map introuvable après upload photo")
    }

    pub fn picture_file_path(&self, map_id: i64, filename: &str) -> Result<Option<PathBuf>> {
        let filename = sanitize_existing_filename(filename)?;
        let conn = self.conn.lock().unwrap();
        let exists: bool = conn.query_row(
            "
            SELECT EXISTS(
                SELECT 1 FROM tts_map_pictures
                WHERE map_id = ?1 AND filename = ?2
            )
            ",
            params![map_id, filename],
            |row| row.get(0),
        )?;
        if !exists {
            return Ok(None);
        }
        Ok(Some(self.pictures_dir(map_id).join(filename)))
    }

    pub fn delete_picture(&self, map_id: i64, filename: &str) -> Result<TtsMapDetail> {
        let filename = sanitize_existing_filename(filename)?;
        let path = self.pictures_dir(map_id).join(&filename);
        let now = now_unix();
        let conn = self.conn.lock().unwrap();
        let n = conn.execute(
            "DELETE FROM tts_map_pictures WHERE map_id = ?1 AND filename = ?2",
            params![map_id, filename],
        )?;
        if n == 0 {
            bail!("image introuvable");
        }
        conn.execute(
            "UPDATE tts_maps SET updated_at = ?1 WHERE id = ?2",
            params![now, map_id],
        )?;
        let detail = self
            .get_map_in_conn(&conn, map_id)?
            .with_context(|| "map introuvable")?;
        drop(conn);
        let _ = fs::remove_file(path);
        Ok(detail)
    }

    pub fn list_content_images(&self) -> Result<Vec<TtsContentImage>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "
            SELECT m.id, m.name, p.filename
            FROM tts_map_pictures p
            JOIN tts_maps m ON m.id = p.map_id
            ORDER BY m.name COLLATE NOCASE ASC, p.filename COLLATE NOCASE ASC
            ",
        )?;
        let rows = stmt.query_map([], |row| {
            let map_id: i64 = row.get(0)?;
            let map_name: String = row.get(1)?;
            let filename: String = row.get(2)?;
            Ok(TtsContentImage {
                label: format!("{map_name} / {filename}"),
                path: picture_url(map_id, &filename),
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn list_updates(&self) -> Result<Vec<TtsModuleUpdate>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "
            SELECT id, body_md, created_at, updated_at
            FROM tts_module_updates
            ORDER BY created_at DESC, id DESC
            ",
        )?;
        let rows = stmt.query_map([], row_to_update)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn create_update(&self, body_md: &str) -> Result<TtsModuleUpdate> {
        let body_md = validate_update_body(body_md)?;
        let now = now_unix();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "
            INSERT INTO tts_module_updates (body_md, created_at, updated_at)
            VALUES (?1, ?2, ?2)
            ",
            params![body_md, now],
        )?;
        let id = conn.last_insert_rowid();
        Ok(TtsModuleUpdate {
            id,
            body_md,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn update_update(&self, id: i64, body_md: &str) -> Result<TtsModuleUpdate> {
        let body_md = validate_update_body(body_md)?;
        let now = now_unix();
        let conn = self.conn.lock().unwrap();
        let n = conn.execute(
            "
            UPDATE tts_module_updates
            SET body_md = ?1, updated_at = ?2
            WHERE id = ?3
            ",
            params![body_md, now, id],
        )?;
        if n == 0 {
            bail!("mise à jour introuvable");
        }
        let created_at: u64 = conn.query_row(
            "SELECT created_at FROM tts_module_updates WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        Ok(TtsModuleUpdate {
            id,
            body_md,
            created_at,
            updated_at: now,
        })
    }

    pub fn delete_update(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let n = conn.execute("DELETE FROM tts_module_updates WHERE id = ?1", params![id])?;
        if n == 0 {
            bail!("mise à jour introuvable");
        }
        Ok(())
    }

    pub fn create_report(
        &self,
        map_id: i64,
        reporter_user_id: i64,
        description: &str,
        image: Option<(&str, &[u8])>,
    ) -> Result<TtsMapReport> {
        let description = validate_report_description(description)?;
        if let Some((name, bytes)) = image {
            if bytes.len() > MAX_PICTURE_BYTES {
                bail!("l'image dépasse {MAX_PICTURE_BYTES} octets");
            }
            unique_picture_filename(name)?;
        }
        self.ensure_map_exists(map_id)?;
        let now = now_unix();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "
            INSERT INTO tts_map_reports (map_id, reporter_user_id, description, created_at)
            VALUES (?1, ?2, ?3, ?4)
            ",
            params![map_id, reporter_user_id, description, now],
        )?;
        let id = conn.last_insert_rowid();
        drop(conn);

        if let Some((original_name, bytes)) = image {
            let filename = unique_picture_filename(original_name)?;
            let dir = self.report_dir(id);
            fs::create_dir_all(&dir)
                .with_context(|| format!("impossible de créer {}", dir.display()))?;
            let path = dir.join(&filename);
            fs::write(&path, bytes)
                .with_context(|| format!("impossible d'écrire {}", path.display()))?;
            let conn = self.conn.lock().unwrap();
            conn.execute(
                "UPDATE tts_map_reports SET image_filename = ?1 WHERE id = ?2",
                params![filename, id],
            )?;
        }

        self.get_report(id)?
            .with_context(|| "signalement introuvable après création")
    }

    pub fn list_reports(&self) -> Result<Vec<TtsMapReport>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "
            SELECT
                r.id, r.map_id, m.name, m.slug,
                r.reporter_user_id,
                COALESCE(NULLIF(TRIM(u.local_display_name), ''), u.display_name, ''),
                r.description, r.image_filename, r.created_at
            FROM tts_map_reports r
            JOIN tts_maps m ON m.id = r.map_id
            LEFT JOIN users u ON u.id = r.reporter_user_id
            ORDER BY r.created_at DESC, r.id DESC
            ",
        )?;
        let rows = stmt.query_map([], row_to_report)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn count_reports(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM tts_map_reports", [], |row| row.get(0))
            .map_err(Into::into)
    }

    pub fn get_report(&self, id: i64) -> Result<Option<TtsMapReport>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "
            SELECT
                r.id, r.map_id, m.name, m.slug,
                r.reporter_user_id,
                COALESCE(NULLIF(TRIM(u.local_display_name), ''), u.display_name, ''),
                r.description, r.image_filename, r.created_at
            FROM tts_map_reports r
            JOIN tts_maps m ON m.id = r.map_id
            LEFT JOIN users u ON u.id = r.reporter_user_id
            WHERE r.id = ?1
            ",
        )?;
        match stmt.query_row(params![id], row_to_report) {
            Ok(report) => Ok(Some(report)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn report_image_path(&self, id: i64) -> Result<Option<(PathBuf, String)>> {
        let conn = self.conn.lock().unwrap();
        let filename: Option<String> = match conn.query_row(
            "SELECT image_filename FROM tts_map_reports WHERE id = ?1",
            params![id],
            |row| row.get(0),
        ) {
            Ok(value) => value,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let Some(filename) = filename.filter(|name| !name.trim().is_empty()) else {
            return Ok(None);
        };
        let filename = sanitize_existing_filename(&filename)?;
        Ok(Some((self.report_dir(id).join(&filename), filename)))
    }

    pub fn delete_report(&self, id: i64) -> Result<()> {
        let dir = self.report_dir(id);
        let conn = self.conn.lock().unwrap();
        let n = conn.execute("DELETE FROM tts_map_reports WHERE id = ?1", params![id])?;
        if n == 0 {
            bail!("signalement introuvable");
        }
        drop(conn);
        if dir.exists() {
            let _ = fs::remove_dir_all(&dir);
        }
        Ok(())
    }

    pub fn create_variant(
        &self,
        map_id: i64,
        scenario_id: i64,
        tournament_id: Option<i64>,
        original_name: &str,
        bytes: &[u8],
    ) -> Result<TtsMapVariant> {
        let filename = validate_variant_json(original_name, bytes)?;
        self.ensure_map_exists(map_id)?;
        self.ensure_scenario_exists(scenario_id)?;
        if let Some(tournament_id) = tournament_id {
            self.ensure_tournament_exists(tournament_id)?;
        }
        self.ensure_variant_unique(map_id, scenario_id, tournament_id, None)?;

        let now = now_unix();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "
            INSERT INTO tts_map_variants (
                map_id, scenario_id, tournament_id, json_filename, created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?5)
            ",
            params![map_id, scenario_id, tournament_id, filename, now],
        )?;
        let id = conn.last_insert_rowid();
        drop(conn);

        let dir = self.variant_dir(id);
        if let Err(error) = (|| -> Result<()> {
            fs::create_dir_all(&dir)
                .with_context(|| format!("impossible de créer {}", dir.display()))?;
            fs::write(dir.join(&filename), bytes)
                .with_context(|| format!("impossible d'écrire {}", dir.join(&filename).display()))?;
            Ok(())
        })() {
            let conn = self.conn.lock().unwrap();
            let _ = conn.execute("DELETE FROM tts_map_variants WHERE id = ?1", params![id]);
            let _ = fs::remove_dir_all(&dir);
            return Err(error);
        }

        self.get_variant(id)?
            .with_context(|| "dérivée introuvable après création")
    }

    pub fn list_variants(
        &self,
        map_id: Option<i64>,
        scenario_id: Option<i64>,
        tournament_id: Option<i64>,
    ) -> Result<Vec<TtsMapVariant>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "
            SELECT
                v.id, v.map_id, m.name, m.slug,
                v.scenario_id, s.name, COALESCE(s.slug, ''),
                v.tournament_id, t.name,
                v.json_filename, v.created_at, v.updated_at
            FROM tts_map_variants v
            JOIN tts_maps m ON m.id = v.map_id
            JOIN scenarios s ON s.id = v.scenario_id
            LEFT JOIN tournaments t ON t.id = v.tournament_id
            WHERE (?1 IS NULL OR v.map_id = ?1)
              AND (?2 IS NULL OR v.scenario_id = ?2)
              AND (?3 IS NULL OR v.tournament_id = ?3)
            ORDER BY s.sort_order ASC, s.name COLLATE NOCASE ASC,
                     m.name COLLATE NOCASE ASC, v.id ASC
            ",
        )?;
        let rows = stmt.query_map(params![map_id, scenario_id, tournament_id], row_to_variant)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_variant(&self, id: i64) -> Result<Option<TtsMapVariant>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "
            SELECT
                v.id, v.map_id, m.name, m.slug,
                v.scenario_id, s.name, COALESCE(s.slug, ''),
                v.tournament_id, t.name,
                v.json_filename, v.created_at, v.updated_at
            FROM tts_map_variants v
            JOIN tts_maps m ON m.id = v.map_id
            JOIN scenarios s ON s.id = v.scenario_id
            LEFT JOIN tournaments t ON t.id = v.tournament_id
            WHERE v.id = ?1
            ",
        )?;
        match stmt.query_row(params![id], row_to_variant) {
            Ok(variant) => Ok(Some(variant)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn variant_json_path(&self, id: i64) -> Result<Option<(PathBuf, String)>> {
        let conn = self.conn.lock().unwrap();
        let filename: Option<String> = match conn.query_row(
            "SELECT json_filename FROM tts_map_variants WHERE id = ?1",
            params![id],
            |row| row.get(0),
        ) {
            Ok(value) => Some(value),
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let Some(filename) = filename.filter(|name| !name.trim().is_empty()) else {
            return Ok(None);
        };
        let filename = sanitize_existing_filename(&filename)?;
        Ok(Some((self.variant_dir(id).join(&filename), filename)))
    }

    pub fn save_variant_json(
        &self,
        id: i64,
        original_name: &str,
        bytes: &[u8],
    ) -> Result<TtsMapVariant> {
        let filename = validate_variant_json(original_name, bytes)?;
        let dir = self.variant_dir(id);
        let conn = self.conn.lock().unwrap();
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM tts_map_variants WHERE id = ?1)",
            params![id],
            |row| row.get(0),
        )?;
        if !exists {
            bail!("dérivée introuvable");
        }
        drop(conn);

        fs::create_dir_all(&dir)
            .with_context(|| format!("impossible de créer {}", dir.display()))?;
        if dir.exists() {
            for entry in fs::read_dir(&dir)? {
                let entry = entry?;
                if entry.path().is_file() {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
        fs::write(dir.join(&filename), bytes)
            .with_context(|| format!("impossible d'écrire {}", dir.join(&filename).display()))?;
        let now = now_unix();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE tts_map_variants SET json_filename = ?1, updated_at = ?2 WHERE id = ?3",
            params![filename, now, id],
        )?;
        drop(conn);
        self.get_variant(id)?
            .with_context(|| "dérivée introuvable après upload JSON")
    }

    pub fn delete_variant(&self, id: i64) -> Result<()> {
        let dir = self.variant_dir(id);
        let conn = self.conn.lock().unwrap();
        let n = conn.execute("DELETE FROM tts_map_variants WHERE id = ?1", params![id])?;
        if n == 0 {
            bail!("dérivée introuvable");
        }
        drop(conn);
        if dir.exists() {
            let _ = fs::remove_dir_all(&dir);
        }
        Ok(())
    }

    pub fn delete_variants_for_tournament(&self, tournament_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let ids: Vec<i64> = {
            let mut stmt =
                conn.prepare("SELECT id FROM tts_map_variants WHERE tournament_id = ?1")?;
            let rows = stmt.query_map(params![tournament_id], |row| row.get(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        conn.execute(
            "DELETE FROM tts_map_variants WHERE tournament_id = ?1",
            params![tournament_id],
        )?;
        drop(conn);
        for id in ids {
            let dir = self.variant_dir(id);
            if dir.exists() {
                let _ = fs::remove_dir_all(&dir);
            }
        }
        Ok(())
    }

    fn ensure_variant_unique(
        &self,
        map_id: i64,
        scenario_id: i64,
        tournament_id: Option<i64>,
        ignore_id: Option<i64>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let exists: bool = if let Some(tournament_id) = tournament_id {
            conn.query_row(
                "
                SELECT EXISTS(
                    SELECT 1 FROM tts_map_variants
                    WHERE tournament_id = ?1 AND scenario_id = ?2
                      AND (?3 IS NULL OR id != ?3)
                )
                ",
                params![tournament_id, scenario_id, ignore_id],
                |row| row.get(0),
            )?
        } else {
            conn.query_row(
                "
                SELECT EXISTS(
                    SELECT 1 FROM tts_map_variants
                    WHERE map_id = ?1 AND scenario_id = ?2 AND tournament_id IS NULL
                      AND (?3 IS NULL OR id != ?3)
                )
                ",
                params![map_id, scenario_id, ignore_id],
                |row| row.get(0),
            )?
        };
        if exists {
            if tournament_id.is_some() {
                bail!("ce scénario a déjà une dérivée TTS pour ce tournoi");
            }
            bail!("cette map a déjà une dérivée TTS pour ce scénario");
        }
        Ok(())
    }

    fn ensure_map_exists(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM tts_maps WHERE id = ?1)",
            params![id],
            |row| row.get(0),
        )?;
        if !exists {
            bail!("map introuvable");
        }
        Ok(())
    }

    fn ensure_scenario_exists(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM scenarios WHERE id = ?1)",
            params![id],
            |row| row.get(0),
        )?;
        if !exists {
            bail!("scénario introuvable");
        }
        Ok(())
    }

    fn ensure_tournament_exists(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM tournaments WHERE id = ?1)",
            params![id],
            |row| row.get(0),
        )?;
        if !exists {
            bail!("tournoi introuvable");
        }
        Ok(())
    }

    fn get_map_in_conn(&self, conn: &Connection, id: i64) -> Result<Option<TtsMapDetail>> {
        let mut stmt = conn.prepare(
            "
            SELECT id, slug, name, json_filename, created_at, updated_at
            FROM tts_maps
            WHERE id = ?1
            ",
        )?;
        let map = match stmt.query_row(params![id], |row| {
            let json_filename: Option<String> = row.get(3)?;
            let json_filename = json_filename.filter(|name| !name.trim().is_empty());
            let json_url = json_filename
                .as_ref()
                .map(|_| format!("/api/tts-maps/{id}/json"));
            Ok(TtsMapDetail {
                id: row.get(0)?,
                slug: row.get(1)?,
                name: row.get(2)?,
                json_filename,
                json_url,
                pictures: Vec::new(),
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        }) {
            Ok(map) => map,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(error) => return Err(error.into()),
        };

        let mut pic_stmt = conn.prepare(
            "
            SELECT id, filename, original_name, created_at
            FROM tts_map_pictures
            WHERE map_id = ?1
            ORDER BY created_at ASC, id ASC
            ",
        )?;
        let pictures = pic_stmt
            .query_map(params![id], |row| {
                let filename: String = row.get(1)?;
                Ok(TtsMapPicture {
                    id: row.get(0)?,
                    url: picture_url(id, &filename),
                    filename,
                    original_name: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Some(TtsMapDetail {
            pictures,
            ..map
        }))
    }

    fn map_dir(&self, id: i64) -> PathBuf {
        self.uploads_root.join("maps").join(id.to_string())
    }

    fn pictures_dir(&self, id: i64) -> PathBuf {
        self.map_dir(id).join("pictures")
    }

    fn json_dir(&self, id: i64) -> PathBuf {
        self.map_dir(id).join("json")
    }

    fn report_dir(&self, id: i64) -> PathBuf {
        self.uploads_root.join("map-reports").join(id.to_string())
    }

    fn variant_dir(&self, id: i64) -> PathBuf {
        self.uploads_root.join("map-variants").join(id.to_string())
    }
}

pub fn picture_url(map_id: i64, filename: &str) -> String {
    format!(
        "/api/tts-maps/{map_id}/pictures/{}",
        urlencoding::encode(filename)
    )
}

pub fn mime_from_filename(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".json") {
        "application/json"
    } else {
        "application/octet-stream"
    }
}

fn uploads_root_from_db(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join("uploads")
}

fn validate_map_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        bail!("le nom de la map est requis");
    }
    if name.chars().count() > MAX_NAME_CHARS {
        bail!("le nom de la map est trop long");
    }
    Ok(name.to_string())
}

fn validate_update_body(body_md: &str) -> Result<String> {
    let body_md = body_md.trim();
    if body_md.is_empty() {
        bail!("la description est requise");
    }
    if body_md.chars().count() > MAX_UPDATE_CHARS {
        bail!("la description est trop longue");
    }
    Ok(body_md.to_string())
}

fn validate_report_description(description: &str) -> Result<String> {
    let description = description.trim();
    if description.is_empty() {
        bail!("la description est requise");
    }
    if description.chars().count() > MAX_REPORT_CHARS {
        bail!("la description est trop longue");
    }
    Ok(description.to_string())
}

fn validate_variant_json(original_name: &str, bytes: &[u8]) -> Result<String> {
    if bytes.len() > MAX_JSON_BYTES {
        bail!("le JSON dépasse {MAX_JSON_BYTES} octets");
    }
    serde_json::from_slice::<serde_json::Value>(bytes)
        .context("le fichier n'est pas un JSON valide")?;
    let filename = sanitize_filename(original_name, "map.json")?;
    if !filename.to_ascii_lowercase().ends_with(".json") {
        bail!("le fichier JSON doit avoir l'extension .json");
    }
    Ok(filename)
}

fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut pending_hyphen = false;
    for ch in name.trim().chars() {
        let mapped = match ch {
            'à' | 'á' | 'â' | 'ä' | 'ã' | 'À' | 'Á' | 'Â' | 'Ä' | 'Ã' => Some('a'),
            'è' | 'é' | 'ê' | 'ë' | 'È' | 'É' | 'Ê' | 'Ë' => Some('e'),
            'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' | 'Î' | 'Ï' => Some('i'),
            'ò' | 'ó' | 'ô' | 'ö' | 'õ' | 'Ò' | 'Ó' | 'Ô' | 'Ö' | 'Õ' => Some('o'),
            'ù' | 'ú' | 'û' | 'ü' | 'Ù' | 'Ú' | 'Û' | 'Ü' => Some('u'),
            'ç' | 'Ç' => Some('c'),
            'ñ' | 'Ñ' => Some('n'),
            'œ' | 'Œ' => {
                flush_hyphen(&mut out, &mut pending_hyphen);
                out.push_str("oe");
                None
            }
            'æ' | 'Æ' => {
                flush_hyphen(&mut out, &mut pending_hyphen);
                out.push_str("ae");
                None
            }
            c if c.is_ascii_alphanumeric() => Some(c.to_ascii_lowercase()),
            c if c.is_whitespace() || c == '_' || c == '-' => {
                pending_hyphen = !out.is_empty();
                None
            }
            _ => None,
        };
        if let Some(mapped) = mapped {
            flush_hyphen(&mut out, &mut pending_hyphen);
            out.push(mapped);
        }
    }
    if out.is_empty() {
        "map".to_string()
    } else {
        out
    }
}

fn flush_hyphen(out: &mut String, pending: &mut bool) {
    if *pending && !out.is_empty() && !out.ends_with('-') {
        out.push('-');
    }
    *pending = false;
}

fn unique_slug(conn: &Connection, base: &str) -> Result<String> {
    let mut slug = base.to_string();
    let mut n = 2;
    loop {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM tts_maps WHERE slug = ?1)",
            params![slug],
            |row| row.get(0),
        )?;
        if !exists {
            return Ok(slug);
        }
        slug = format!("{base}-{n}");
        n += 1;
        if n > 1000 {
            bail!("impossible de générer un identifiant unique");
        }
    }
}

fn unique_picture_filename(original_name: &str) -> Result<String> {
    let filename = sanitize_filename(original_name, "image.png")?;
    let lower = filename.to_ascii_lowercase();
    if !(lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
        || lower.ends_with(".webp"))
    {
        bail!("format d'image non supporté (png, jpg, gif, webp)");
    }
    Ok(filename)
}

fn sanitize_filename(name: &str, fallback: &str) -> Result<String> {
    let raw = Path::new(name)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(fallback)
        .trim();
    let mut out = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
            out.push(ch);
        } else if ch.is_whitespace() {
            out.push('_');
        }
    }
    while out.contains("..") {
        out = out.replace("..", ".");
    }
    if out.starts_with('.') {
        out.remove(0);
    }
    if out.is_empty() || out == "." {
        out = fallback.to_string();
    }
    Ok(out)
}

fn sanitize_existing_filename(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty()
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains("..")
    {
        bail!("nom de fichier invalide");
    }
    Ok(trimmed.to_string())
}

fn append_filename_suffix(filename: &str, n: usize) -> String {
    if let Some((stem, ext)) = filename.rsplit_once('.') {
        format!("{stem}-{n}.{ext}")
    } else {
        format!("{filename}-{n}")
    }
}

fn row_to_update(row: &rusqlite::Row<'_>) -> rusqlite::Result<TtsModuleUpdate> {
    Ok(TtsModuleUpdate {
        id: row.get(0)?,
        body_md: row.get(1)?,
        created_at: row.get(2)?,
        updated_at: row.get(3)?,
    })
}

fn row_to_report(row: &rusqlite::Row<'_>) -> rusqlite::Result<TtsMapReport> {
    let id: i64 = row.get(0)?;
    let reporter_user_id: i64 = row.get(4)?;
    let reporter_name: String = row.get(5)?;
    let image_filename: Option<String> = row.get(7)?;
    let image_filename = image_filename.filter(|name| !name.trim().is_empty());
    let image_url = image_filename
        .as_ref()
        .map(|_| format!("/api/tts-map-reports/{id}/image"));
    Ok(TtsMapReport {
        id,
        map_id: row.get(1)?,
        map_name: row.get(2)?,
        map_slug: row.get(3)?,
        reporter_user_id,
        reporter_display_name: if reporter_name.trim().is_empty() {
            format!("Utilisateur {reporter_user_id}")
        } else {
            reporter_name
        },
        description: row.get(6)?,
        image_filename,
        image_url,
        created_at: row.get(8)?,
    })
}

fn row_to_variant(row: &rusqlite::Row<'_>) -> rusqlite::Result<TtsMapVariant> {
    let id: i64 = row.get(0)?;
    let json_filename: String = row.get(9)?;
    Ok(TtsMapVariant {
        id,
        map_id: row.get(1)?,
        map_name: row.get(2)?,
        map_slug: row.get(3)?,
        scenario_id: row.get(4)?,
        scenario_name: row.get(5)?,
        scenario_slug: row.get(6)?,
        tournament_id: row.get(7)?,
        tournament_name: row.get(8)?,
        json_url: format!("/api/tts-map-variants/{id}/json"),
        json_filename,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

    fn temp_store() -> (TtsMapStore, PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "poissonnerie-tts-maps-{}-{}-{}",
            std::process::id(),
            now_unix(),
            TEST_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_file(&path);
        let store = TtsMapStore::open(&path).unwrap();
        (store, path)
    }

    fn insert_user(path: &Path, name: &str) -> i64 {
        let conn = Connection::open(path).unwrap();
        let now = now_unix() as i64;
        conn.execute(
            "
            INSERT INTO users (discord_id, username, display_name, avatar_url, created_at, last_login_at)
            VALUES (?1, ?2, ?3, '', ?4, ?4)
            ",
            params![format!("d-{name}-{now}"), name, name, now],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn first_scenario(path: &Path) -> (i64, String) {
        let conn = Connection::open(path).unwrap();
        conn.query_row(
            "SELECT id, name FROM scenarios ORDER BY id ASC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
    }

    fn insert_tournament(path: &Path, name: &str) -> i64 {
        let conn = Connection::open(path).unwrap();
        let now = now_unix() as i64;
        conn.execute(
            "
            INSERT INTO tournaments (name, created_at)
            VALUES (?1, ?2)
            ",
            params![name, now],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn slugify_french_name() {
        assert_eq!(slugify("Âge de glace"), "age-de-glace");
        assert_eq!(slugify("  "), "map");
    }

    #[test]
    fn create_upload_update_delete_roundtrip() {
        let (store, path) = temp_store();
        let created = store.create_map(" Carte test ").unwrap();
        assert_eq!(created.name, "Carte test");
        assert_eq!(created.slug, "carte-test");
        assert!(created.json_url.is_none());

        let json = br#"{"Tabletop":true}"#;
        let with_json = store.save_json(created.id, "table.json", json).unwrap();
        assert_eq!(with_json.json_filename.as_deref(), Some("table.json"));
        let json_path = store.json_file_path(created.id).unwrap().unwrap().0;
        assert_eq!(fs::read(&json_path).unwrap(), json);

        let png = [0x89, b'P', b'N', b'G', 0, 1, 2, 3];
        let with_pic = store
            .add_picture(created.id, "Vue nord.png", &png)
            .unwrap();
        assert_eq!(with_pic.pictures.len(), 1);
        assert_eq!(with_pic.pictures[0].filename, "Vue_nord.png");

        let update = store.create_update("Nouveau pack TTS").unwrap();
        assert_eq!(store.list_updates().unwrap().len(), 1);
        store
            .update_update(update.id, "Pack TTS corrigé")
            .unwrap();
        store.delete_update(update.id).unwrap();
        assert!(store.list_updates().unwrap().is_empty());

        let map_dir = json_path.parent().unwrap().parent().unwrap().to_path_buf();
        store.delete_map(created.id).unwrap();
        assert!(store.list_maps().unwrap().is_empty());
        assert!(!map_dir.exists());

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn report_create_list_delete() {
        let (store, path) = temp_store();
        let user_id = insert_user(&path, "Alex");
        let map = store.create_map("Carte bug").unwrap();
        let png = [0x89, b'P', b'N', b'G', 0, 1, 2, 3];
        let report = store
            .create_report(map.id, user_id, " Texture cassée ", Some(("bug.png", &png)))
            .unwrap();
        assert_eq!(report.map_name, "Carte bug");
        assert_eq!(report.reporter_display_name, "Alex");
        assert_eq!(report.description, "Texture cassée");
        assert_eq!(report.image_filename.as_deref(), Some("bug.png"));
        assert_eq!(store.count_reports().unwrap(), 1);
        assert_eq!(store.list_reports().unwrap().len(), 1);
        let image_path = store.report_image_path(report.id).unwrap().unwrap().0;
        assert_eq!(fs::read(&image_path).unwrap(), png);

        store.delete_report(report.id).unwrap();
        assert_eq!(store.count_reports().unwrap(), 0);
        assert!(!image_path.exists());

        let report = store
            .create_report(map.id, user_id, "Encore cassé", Some(("bug.png", &png)))
            .unwrap();
        let image_path = store.report_image_path(report.id).unwrap().unwrap().0;
        store.delete_map(map.id).unwrap();
        assert_eq!(store.count_reports().unwrap(), 0);
        assert!(!image_path.exists());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn variant_create_list_delete() {
        let (store, path) = temp_store();
        let map = store.create_map("Age de glace").unwrap();
        let (scenario_id, scenario_name) = first_scenario(&path);
        let tournament_id = insert_tournament(&path, "Coupe 4");
        let json = br#"{"Tabletop":true,"variant":1}"#;

        let generic = store
            .create_variant(map.id, scenario_id, None, "frontline.json", json)
            .unwrap();
        assert_eq!(generic.map_name, "Age de glace");
        assert_eq!(generic.scenario_name, scenario_name);
        assert!(generic.tournament_id.is_none());
        assert_eq!(generic.json_filename, "frontline.json");
        let json_path = store.variant_json_path(generic.id).unwrap().unwrap().0;
        assert_eq!(fs::read(&json_path).unwrap(), json);

        assert!(
            store
                .create_variant(map.id, scenario_id, None, "dup.json", json)
                .is_err()
        );

        let tournament_variant = store
            .create_variant(
                map.id,
                scenario_id,
                Some(tournament_id),
                "coupe.json",
                json,
            )
            .unwrap();
        assert_eq!(tournament_variant.tournament_id, Some(tournament_id));
        assert_eq!(
            tournament_variant.tournament_name.as_deref(),
            Some("Coupe 4")
        );
        assert_eq!(
            store
                .list_variants(Some(map.id), None, None)
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            store
                .list_variants(None, None, Some(tournament_id))
                .unwrap()
                .len(),
            1
        );

        store.delete_variants_for_tournament(tournament_id).unwrap();
        assert!(
            store
                .list_variants(None, None, Some(tournament_id))
                .unwrap()
                .is_empty()
        );

        store.delete_map(map.id).unwrap();
        assert!(store.list_variants(None, None, None).unwrap().is_empty());
        assert!(!json_path.exists());
        let _ = fs::remove_file(&path);
    }
}

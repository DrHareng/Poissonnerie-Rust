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
        conn.execute("DELETE FROM tts_map_pictures WHERE map_id = ?1", params![id])?;
        let n = conn.execute("DELETE FROM tts_maps WHERE id = ?1", params![id])?;
        if n == 0 {
            bail!("map introuvable");
        }
        drop(conn);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> (TtsMapStore, PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "poissonnerie-tts-maps-{}-{}",
            std::process::id(),
            now_unix()
        ));
        let _ = fs::remove_file(&path);
        let store = TtsMapStore::open(&path).unwrap();
        (store, path)
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
}

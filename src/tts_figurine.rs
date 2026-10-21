//! Figurines peintes : ISC, loadout, peintre, lien d'asset bundle et photo.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::match_record::now_unix;
use crate::migrate::migrate;
use crate::tts_map::{self, MAX_PICTURE_BYTES};

const MAX_ISC_CHARS: usize = 200;
const MAX_LOADOUT_CHARS: usize = 200;
const MAX_PAINTER_CHARS: usize = 120;
const MAX_URL_CHARS: usize = 2_000;

#[derive(Debug, Clone)]
pub struct FigurineWrite {
    pub isc: String,
    pub loadout: String,
    pub painter: String,
    pub asset_bundle_url: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TtsFigurine {
    pub id: i64,
    pub isc: String,
    pub loadout: String,
    pub painter: String,
    pub asset_bundle_url: String,
    pub photo_url: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(default)]
    pub sectorial_ids: Vec<i64>,
}

/// Meilleur asset bundle pour un ISC/loadout (peintre préféré, loadout exact, URL non vide).
pub fn best_asset_bundle(
    figurines: &[TtsFigurine],
    isc: &str,
    loadout: &str,
    preferred_painter: Option<&str>,
) -> Option<String> {
    let isc_key = isc.trim().to_ascii_lowercase();
    let loadout_key = loadout.trim().to_ascii_lowercase();
    let painter_key = preferred_painter
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());

    let mut best: Option<(i32, &TtsFigurine)> = None;
    for figurine in figurines {
        if figurine.isc.trim().to_ascii_lowercase() != isc_key {
            continue;
        }
        if figurine.asset_bundle_url.trim().is_empty() {
            continue;
        }
        let mut score = 1;
        if !loadout_key.is_empty()
            && figurine.loadout.trim().to_ascii_lowercase() == loadout_key
        {
            score += 100;
        }
        if painter_key.as_ref().is_some_and(|painter| {
            figurine.painter.trim().to_ascii_lowercase() == *painter
        }) {
            score += 10;
        }
        match best {
            Some((best_score, _)) if best_score >= score => {}
            _ => best = Some((score, figurine)),
        }
    }
    best.map(|(_, figurine)| figurine.asset_bundle_url.clone())
}

pub struct TtsFigurineStore {
    conn: Mutex<Connection>,
    uploads_root: PathBuf,
}

impl TtsFigurineStore {
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
            uploads_root: uploads_root(path),
        })
    }

    pub fn list(&self) -> Result<Vec<TtsFigurine>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "
            SELECT id, isc, loadout, painter, asset_bundle_url, photo_filename, created_at, updated_at
            FROM tts_figurines
            ORDER BY isc COLLATE NOCASE, loadout COLLATE NOCASE, id
            ",
        )?;
        let rows = stmt
            .query_map([], map_row)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("lecture des figurines")?;
        Ok(rows)
    }

    pub fn get(&self, id: i64) -> Result<Option<TtsFigurine>> {
        let conn = self.conn.lock().unwrap();
        get_in_conn(&conn, id)
    }

    pub fn create(&self, input: &FigurineWrite) -> Result<TtsFigurine> {
        let input = normalize_write(input)?;
        let now = now_unix();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "
            INSERT INTO tts_figurines (
                isc, loadout, painter, asset_bundle_url, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?5)
            ",
            params![
                input.isc,
                input.loadout,
                input.painter,
                input.asset_bundle_url,
                now
            ],
        )?;
        let id = conn.last_insert_rowid();
        get_in_conn(&conn, id)?.context("figurine introuvable après création")
    }

    pub fn update(&self, id: i64, input: &FigurineWrite) -> Result<TtsFigurine> {
        let input = normalize_write(input)?;
        let now = now_unix();
        let conn = self.conn.lock().unwrap();
        let changed = conn.execute(
            "
            UPDATE tts_figurines
            SET isc = ?1, loadout = ?2, painter = ?3, asset_bundle_url = ?4, updated_at = ?5
            WHERE id = ?6
            ",
            params![
                input.isc,
                input.loadout,
                input.painter,
                input.asset_bundle_url,
                now,
                id
            ],
        )?;
        if changed == 0 {
            bail!("figurine introuvable");
        }
        get_in_conn(&conn, id)?.context("figurine introuvable après mise à jour")
    }

    pub fn delete(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let changed = conn.execute("DELETE FROM tts_figurines WHERE id = ?1", params![id])?;
        drop(conn);
        if changed == 0 {
            bail!("figurine introuvable");
        }
        let _ = fs::remove_dir_all(self.dir(id));
        Ok(())
    }

    pub fn set_photo(&self, id: i64, bytes: &[u8]) -> Result<TtsFigurine> {
        if bytes.is_empty() {
            bail!("image vide");
        }
        if bytes.len() > MAX_PICTURE_BYTES {
            bail!("l'image dépasse {MAX_PICTURE_BYTES} octets");
        }
        let format = image::guess_format(bytes).context("image illisible")?;
        let ext = match format {
            image::ImageFormat::Png => "png",
            image::ImageFormat::Jpeg => "jpg",
            image::ImageFormat::Gif => "gif",
            image::ImageFormat::WebP => "webp",
            _ => bail!("format d'image non supporté (png, jpg, gif, webp)"),
        };
        image::load_from_memory(bytes).context("image illisible")?;

        {
            let conn = self.conn.lock().unwrap();
            if get_in_conn(&conn, id)?.is_none() {
                bail!("figurine introuvable");
            }
        }

        let filename = format!("photo.{ext}");
        let dir = self.dir(id);
        fs::create_dir_all(&dir)
            .with_context(|| format!("impossible de créer {}", dir.display()))?;
        clear_dir_except(&dir, &filename)?;
        let path = dir.join(&filename);
        fs::write(&path, bytes)
            .with_context(|| format!("impossible d'écrire {}", path.display()))?;
        let _ = tts_map::ensure_thumbnail(&path);

        let now = now_unix();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE tts_figurines SET photo_filename = ?1, updated_at = ?2 WHERE id = ?3",
            params![filename, now, id],
        )?;
        get_in_conn(&conn, id)?.context("figurine introuvable après photo")
    }

    pub fn clear_photo(&self, id: i64) -> Result<TtsFigurine> {
        let now = now_unix();
        let conn = self.conn.lock().unwrap();
        let changed = conn.execute(
            "UPDATE tts_figurines SET photo_filename = NULL, updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        drop(conn);
        if changed == 0 {
            bail!("figurine introuvable");
        }
        let _ = fs::remove_dir_all(self.dir(id));
        self.get(id)?
            .context("figurine introuvable après retrait de la photo")
    }

    pub fn photo_file_path(&self, id: i64) -> Result<Option<PathBuf>> {
        let conn = self.conn.lock().unwrap();
        let filename: Option<Option<String>> = conn
            .query_row(
                "SELECT photo_filename FROM tts_figurines WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(filename) = filename.flatten().filter(|name| !name.trim().is_empty()) else {
            return Ok(None);
        };
        let safe = sanitize_filename(&filename)?;
        let path = self.dir(id).join(safe);
        if path.is_file() {
            Ok(Some(path))
        } else {
            Ok(None)
        }
    }

    fn dir(&self, id: i64) -> PathBuf {
        self.uploads_root.join("figurines").join(id.to_string())
    }
}

fn get_in_conn(conn: &Connection, id: i64) -> Result<Option<TtsFigurine>> {
    conn.query_row(
        "
        SELECT id, isc, loadout, painter, asset_bundle_url, photo_filename, created_at, updated_at
        FROM tts_figurines
        WHERE id = ?1
        ",
        params![id],
        map_row,
    )
    .optional()
    .context("lecture de la figurine")
}

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TtsFigurine> {
    let id: i64 = row.get(0)?;
    let photo_filename: Option<String> = row.get(5)?;
    let has_photo = photo_filename
        .as_deref()
        .is_some_and(|name| !name.trim().is_empty());
    Ok(TtsFigurine {
        id,
        isc: row.get(1)?,
        loadout: row.get(2)?,
        painter: row.get(3)?,
        asset_bundle_url: row.get(4)?,
        photo_url: has_photo.then(|| format!("/api/tts-figurines/{id}/photo")),
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        sectorial_ids: Vec::new(),
    })
}

fn normalize_write(input: &FigurineWrite) -> Result<FigurineWrite> {
    let isc = bounded("l'ISC", &input.isc, 1, MAX_ISC_CHARS)?;
    let loadout = bounded("le loadout", &input.loadout, 0, MAX_LOADOUT_CHARS)?;
    let painter = bounded("le nom du peintre", &input.painter, 0, MAX_PAINTER_CHARS)?;
    Ok(FigurineWrite {
        isc,
        loadout,
        painter,
        asset_bundle_url: normalize_bundle_url(&input.asset_bundle_url)?,
    })
}

fn bounded(label: &str, raw: &str, min_chars: usize, max_chars: usize) -> Result<String> {
    let value = raw.trim();
    let count = value.chars().count();
    if count < min_chars {
        bail!("{label} est requis");
    }
    if count > max_chars {
        bail!("{label} est trop long");
    }
    Ok(value.to_string())
}

fn normalize_bundle_url(raw: &str) -> Result<String> {
    let url = raw.trim();
    if url.is_empty() {
        return Ok(String::new());
    }
    if url.chars().count() > MAX_URL_CHARS {
        bail!("le lien d'asset bundle est trop long");
    }
    if url.chars().any(|ch| ch.is_whitespace() || ch.is_control()) {
        bail!("le lien d'asset bundle est invalide");
    }
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        bail!("le lien d'asset bundle doit commencer par http:// ou https://");
    }
    Ok(url.to_string())
}

fn sanitize_filename(name: &str) -> Result<String> {
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

fn clear_dir_except(dir: &Path, keep: &str) -> Result<()> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        if name == keep {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            fs::remove_dir_all(&path)
                .with_context(|| format!("impossible de supprimer {}", path.display()))?;
        } else {
            fs::remove_file(&path)
                .with_context(|| format!("impossible de supprimer {}", path.display()))?;
        }
    }
    Ok(())
}

fn uploads_root(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join("uploads")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    use image::ImageEncoder;

    static SEQ: AtomicU64 = AtomicU64::new(0);

    fn temp_store() -> (TtsFigurineStore, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "poissonnerie-figs-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let store = TtsFigurineStore::open(&dir.join("test.db")).unwrap();
        (store, dir)
    }

    fn sample() -> FigurineWrite {
        FigurineWrite {
            isc: " FUSILIER ".to_string(),
            loadout: " Combi ".to_string(),
            painter: " Alex ".to_string(),
            asset_bundle_url: " https://example.com/bundle ".to_string(),
        }
    }

    fn png_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(&[9, 8, 7, 6, 5, 4], 2, 1, image::ExtendedColorType::Rgb8)
            .unwrap();
        bytes
    }

    #[test]
    fn creates_updates_photo_and_deletes() {
        let (store, dir) = temp_store();
        let created = store.create(&sample()).unwrap();
        assert_eq!(created.isc, "FUSILIER");
        assert_eq!(created.loadout, "Combi");
        assert_eq!(created.painter, "Alex");
        assert_eq!(created.asset_bundle_url, "https://example.com/bundle");
        assert!(created.photo_url.is_none());

        let with_photo = store.set_photo(created.id, &png_bytes()).unwrap();
        assert_eq!(
            with_photo.photo_url.as_deref(),
            Some(format!("/api/tts-figurines/{}/photo", created.id).as_str())
        );
        let path = store.photo_file_path(created.id).unwrap().unwrap();
        assert!(path.ends_with("photo.png"));
        assert!(tts_map::ensure_thumbnail(&path).unwrap().is_file());

        let updated = store
            .update(
                created.id,
                &FigurineWrite {
                    isc: "MAGISTER".into(),
                    loadout: "HMG".into(),
                    painter: "Léa".into(),
                    asset_bundle_url: String::new(),
                },
            )
            .unwrap();
        assert_eq!(updated.isc, "MAGISTER");
        assert!(updated.asset_bundle_url.is_empty());
        assert!(updated.photo_url.is_some());
        assert_eq!(store.list().unwrap().len(), 1);

        store.clear_photo(created.id).unwrap();
        assert!(store.photo_file_path(created.id).unwrap().is_none());
        store.delete(created.id).unwrap();
        assert!(store.list().unwrap().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn best_asset_prefers_painter_and_loadout() {
        let figurines = vec![
            TtsFigurine {
                id: 1,
                isc: "DOG-WARRIOR FORM".into(),
                loadout: "Chain Rifle".into(),
                painter: "Alex".into(),
                asset_bundle_url: "https://a".into(),
                photo_url: None,
                created_at: 0,
                updated_at: 0,
                sectorial_ids: Vec::new(),
            },
            TtsFigurine {
                id: 2,
                isc: "DOG-WARRIOR FORM".into(),
                loadout: "Chain Rifle".into(),
                painter: "Léa".into(),
                asset_bundle_url: "https://b".into(),
                photo_url: None,
                created_at: 0,
                updated_at: 0,
                sectorial_ids: Vec::new(),
            },
        ];
        assert_eq!(
            best_asset_bundle(&figurines, "dog-warrior form", "Chain Rifle", Some("Léa"))
                .as_deref(),
            Some("https://b")
        );
    }

    #[test]
    fn rejects_bad_bundle_and_empty_isc() {
        let (store, dir) = temp_store();
        let mut input = sample();
        input.isc = "  ".into();
        assert!(store.create(&input).is_err());
        input.isc = "FUSILIER".into();
        input.asset_bundle_url = "steam://bundle".into();
        assert!(store.create(&input).is_err());
        assert!(store.set_photo(1, b"not an image").is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}

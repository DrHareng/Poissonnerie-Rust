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

#[derive(Debug, Clone, Default)]
pub struct FigurineListFilter {
    pub limit: u32,
    pub offset: u32,
    pub q: Option<String>,
    pub painter: Option<String>,
    pub army_id: Option<i64>,
    /// ISC, loadout, peintre, asset bundle ou photo manquant.
    pub incomplete: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FigurineListPage {
    pub items: Vec<TtsFigurine>,
    pub total: u64,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FigurineListMeta {
    pub painters: Vec<String>,
    pub iscs: Vec<String>,
}

/// Figurine mémorisée pour une paire ISC + loadout de profil.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TtsFigurinePickPref {
    pub isc: String,
    pub loadout: String,
    pub figurine_id: i64,
    pub updated_at: u64,
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
        let page = self.list_page(&FigurineListFilter {
            limit: u32::MAX,
            offset: 0,
            q: None,
            painter: None,
            army_id: None,
            incomplete: false,
        })?;
        Ok(page.items)
    }

    /// Page de figurines (filtres optionnels peintre / recherche / sectorielle).
    pub fn list_page(&self, filter: &FigurineListFilter) -> Result<FigurineListPage> {
        let conn = self.conn.lock().unwrap();
        let q = filter
            .q
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(fold_search_text);
        let painter = filter
            .painter
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_ascii_lowercase());
        let army_id = filter.army_id.filter(|id| *id > 0);

        let mut where_parts = Vec::new();
        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(painter) = painter.as_ref() {
            where_parts.push("lower(trim(f.painter)) = ?");
            params_vec.push(Box::new(painter.clone()));
        }
        if filter.incomplete {
            where_parts.push(
                "(
                    trim(f.isc) = ''
                    OR trim(f.loadout) = ''
                    OR trim(f.painter) = ''
                    OR trim(f.asset_bundle_url) = ''
                    OR f.photo_filename IS NULL
                    OR trim(COALESCE(f.photo_filename, '')) = ''
                )",
            );
        }
        if let Some(army_id) = army_id {
            where_parts.push(
                "lower(trim(f.isc)) IN (
                    SELECT lower(trim(p.isc))
                    FROM tts_profiles p
                    WHERE p.army_id = ?
                       OR EXISTS (
                            SELECT 1 FROM tts_profile_sectorials s
                            WHERE s.profile_id = p.id AND s.army_id = ?
                       )
                    UNION
                    SELECT lower(trim(ff.isc))
                    FROM tts_profile_forms ff
                    JOIN tts_profiles p ON p.id = ff.profile_id
                    WHERE p.army_id = ?
                       OR EXISTS (
                            SELECT 1 FROM tts_profile_sectorials s
                            WHERE s.profile_id = p.id AND s.army_id = ?
                       )
                )",
            );
            params_vec.push(Box::new(army_id));
            params_vec.push(Box::new(army_id));
            params_vec.push(Box::new(army_id));
            params_vec.push(Box::new(army_id));
        }

        let where_sql = if where_parts.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_parts.join(" AND "))
        };

        // Recherche texte : filtre en Rust pour ignorer les accents (í→i…).
        if let Some(needle) = q.as_ref() {
            let list_sql = format!(
                "
                SELECT f.id, f.isc, f.loadout, f.painter, f.asset_bundle_url,
                       f.photo_filename, f.created_at, f.updated_at
                FROM tts_figurines f
                {where_sql}
                ORDER BY f.isc COLLATE NOCASE, f.loadout COLLATE NOCASE, f.id
                "
            );
            let params_ref: Vec<&dyn rusqlite::types::ToSql> =
                params_vec.iter().map(|p| p.as_ref()).collect();
            let mut stmt = conn.prepare(&list_sql)?;
            let rows = stmt
                .query_map(params_ref.as_slice(), map_row)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .context("lecture des figurines")?;
            let matched: Vec<TtsFigurine> = rows
                .into_iter()
                .filter(|figurine| {
                    let haystack = fold_search_text(&format!(
                        "{} {} {} {}",
                        figurine.isc,
                        figurine.loadout,
                        figurine.painter,
                        figurine.asset_bundle_url
                    ));
                    haystack.contains(needle)
                })
                .collect();
            let total = matched.len() as u64;
            let start = filter.offset as usize;
            let items = matched
                .into_iter()
                .skip(start)
                .take(filter.limit as usize)
                .collect();
            return Ok(FigurineListPage {
                items,
                total,
                limit: filter.limit,
                offset: filter.offset,
            });
        }

        let count_sql = format!("SELECT COUNT(*) FROM tts_figurines f {where_sql}");
        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let total: u64 = conn
            .query_row(&count_sql, params_ref.as_slice(), |row| row.get(0))
            .context("comptage des figurines")?;

        let list_sql = format!(
            "
            SELECT f.id, f.isc, f.loadout, f.painter, f.asset_bundle_url,
                   f.photo_filename, f.created_at, f.updated_at
            FROM tts_figurines f
            {where_sql}
            ORDER BY f.isc COLLATE NOCASE, f.loadout COLLATE NOCASE, f.id
            LIMIT ? OFFSET ?
            "
        );
        let mut list_params = params_vec;
        list_params.push(Box::new(filter.limit as i64));
        list_params.push(Box::new(filter.offset as i64));
        let list_ref: Vec<&dyn rusqlite::types::ToSql> =
            list_params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&list_sql)?;
        let rows = stmt
            .query_map(list_ref.as_slice(), map_row)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("lecture des figurines")?;
        Ok(FigurineListPage {
            items: rows,
            total,
            limit: filter.limit,
            offset: filter.offset,
        })
    }

    /// Peintres et ISC distincts (filtres / onglet profils).
    pub fn list_meta(&self) -> Result<FigurineListMeta> {
        let conn = self.conn.lock().unwrap();
        let mut painters = Vec::new();
        let mut stmt = conn.prepare(
            "
            SELECT DISTINCT trim(painter)
            FROM tts_figurines
            WHERE trim(painter) != ''
            ORDER BY trim(painter) COLLATE NOCASE
            ",
        )?;
        let painter_rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        for row in painter_rows {
            let name = row?;
            if painters
                .last()
                .is_some_and(|prev: &String| prev.eq_ignore_ascii_case(&name))
            {
                continue;
            }
            painters.push(name);
        }

        let mut iscs = Vec::new();
        let mut stmt = conn.prepare(
            "
            SELECT DISTINCT lower(trim(isc))
            FROM tts_figurines
            WHERE trim(isc) != ''
            ORDER BY lower(trim(isc))
            ",
        )?;
        let isc_rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        for row in isc_rows {
            iscs.push(row?);
        }
        Ok(FigurineListMeta { painters, iscs })
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

    pub fn list_pick_prefs(&self, user_id: i64) -> Result<Vec<TtsFigurinePickPref>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "
            SELECT isc_key, loadout_key, figurine_id, updated_at
            FROM tts_figurine_pick_prefs
            WHERE user_id = ?1
            ORDER BY updated_at DESC, isc_key, loadout_key
            ",
        )?;
        let rows = stmt
            .query_map(params![user_id], |row| {
                Ok(TtsFigurinePickPref {
                    isc: row.get(0)?,
                    loadout: row.get(1)?,
                    figurine_id: row.get(2)?,
                    updated_at: row.get::<_, i64>(3)? as u64,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("lecture des préférences de figurines")?;
        Ok(rows)
    }

    pub fn upsert_pick_pref(
        &self,
        user_id: i64,
        isc: &str,
        loadout: &str,
        figurine_id: i64,
    ) -> Result<TtsFigurinePickPref> {
        let isc_key = normalize_pref_key(isc, "l'ISC")?;
        let loadout_key = normalize_pref_key(loadout, "le loadout")?;
        let now = now_unix();
        let conn = self.conn.lock().unwrap();
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM tts_figurines WHERE id = ?1)",
            params![figurine_id],
            |row| row.get(0),
        )?;
        if !exists {
            bail!("figurine introuvable");
        }
        conn.execute(
            "
            INSERT INTO tts_figurine_pick_prefs (user_id, isc_key, loadout_key, figurine_id, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(user_id, isc_key, loadout_key) DO UPDATE SET
                figurine_id = excluded.figurine_id,
                updated_at = excluded.updated_at
            ",
            params![user_id, isc_key, loadout_key, figurine_id, now],
        )?;
        Ok(TtsFigurinePickPref {
            isc: isc_key,
            loadout: loadout_key,
            figurine_id,
            updated_at: now as u64,
        })
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

fn normalize_pref_key(raw: &str, label: &str) -> Result<String> {
    let min = if label.contains("ISC") { 1 } else { 0 };
    let max = if label.contains("ISC") {
        MAX_ISC_CHARS
    } else {
        MAX_LOADOUT_CHARS
    };
    Ok(bounded(label, raw, min, max)?.to_ascii_lowercase())
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

/// Minuscules sans accents (í→i, á→a, ö→o…) pour les recherches ISC / texte.
pub fn fold_search_text(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        for c in ch.to_lowercase() {
            match c {
                'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => out.push('a'),
                'ç' | 'ć' | 'č' => out.push('c'),
                'ď' | 'đ' => out.push('d'),
                'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ė' | 'ę' => out.push('e'),
                'ì' | 'í' | 'î' | 'ï' | 'ī' | 'į' => out.push('i'),
                'ł' => out.push('l'),
                'ñ' | 'ń' => out.push('n'),
                'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ő' => out.push('o'),
                'ř' => out.push('r'),
                'ś' | 'š' | 'ş' | 'ș' => out.push('s'),
                'ť' | 'ț' => out.push('t'),
                'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' => out.push('u'),
                'ý' | 'ÿ' => out.push('y'),
                'ź' | 'ż' | 'ž' => out.push('z'),
                'ß' => out.push_str("ss"),
                'æ' => out.push_str("ae"),
                'œ' => out.push_str("oe"),
                // Marques combinantes éventuelles (texte déjà en NFD).
                '\u{0300}'..='\u{036F}' => {}
                other => out.push(other),
            }
        }
    }
    out
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
    fn list_page_filters_and_paginates() {
        let (store, dir) = temp_store();
        for (isc, painter) in [
            ("Alpha", "Alex"),
            ("Bravo", "Alex"),
            ("Charlie", "Léa"),
            ("Delta", "Alex"),
        ] {
            store
                .create(&FigurineWrite {
                    isc: isc.into(),
                    loadout: "Combi".into(),
                    painter: painter.into(),
                    asset_bundle_url: "https://example.com/x".into(),
                })
                .unwrap();
        }
        let page = store
            .list_page(&FigurineListFilter {
                limit: 2,
                offset: 0,
                q: None,
                painter: Some("Alex".into()),
                army_id: None,
                incomplete: false,
            })
            .unwrap();
        assert_eq!(page.total, 3);
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.items[0].isc, "Alpha");
        let page2 = store
            .list_page(&FigurineListFilter {
                limit: 2,
                offset: 2,
                q: None,
                painter: Some("Alex".into()),
                army_id: None,
                incomplete: false,
            })
            .unwrap();
        assert_eq!(page2.items.len(), 1);
        assert_eq!(page2.items[0].isc, "Delta");
        let search = store
            .list_page(&FigurineListFilter {
                limit: 20,
                offset: 0,
                q: Some("char".into()),
                painter: None,
                army_id: None,
                incomplete: false,
            })
            .unwrap();
        assert_eq!(search.total, 1);
        assert_eq!(search.items[0].isc, "Charlie");
        store
            .create(&FigurineWrite {
                isc: "Síomón".into(),
                loadout: "Combi".into(),
                painter: "Alex".into(),
                asset_bundle_url: "https://example.com/x".into(),
            })
            .unwrap();
        let accent = store
            .list_page(&FigurineListFilter {
                limit: 20,
                offset: 0,
                q: Some("siomon".into()),
                painter: None,
                army_id: None,
                incomplete: false,
            })
            .unwrap();
        assert_eq!(accent.total, 1);
        assert_eq!(accent.items[0].isc, "Síomón");
        store
            .create(&FigurineWrite {
                isc: "Incomplete".into(),
                loadout: "".into(),
                painter: "Alex".into(),
                asset_bundle_url: "https://example.com/x".into(),
            })
            .unwrap();
        let incomplete = store
            .list_page(&FigurineListFilter {
                limit: 20,
                offset: 0,
                q: None,
                painter: None,
                army_id: None,
                incomplete: true,
            })
            .unwrap();
        assert!(incomplete.total >= 1);
        assert!(incomplete.items.iter().any(|row| row.isc == "Incomplete"));
        let meta = store.list_meta().unwrap();
        assert!(meta.painters.iter().any(|p| p == "Alex"));
        assert!(meta.iscs.iter().any(|i| i == "alpha"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn fold_search_text_strips_accents() {
        assert_eq!(fold_search_text("Síomón Águila"), "siomon aguila");
        assert_eq!(fold_search_text("Ötzi"), "otzi");
        assert_eq!(fold_search_text("Ægir"), "aegir");
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
    fn remembers_pick_pref_per_isc_loadout() {
        let (store, dir) = temp_store();
        let fig = store.create(&sample()).unwrap();
        {
            let conn = rusqlite::Connection::open(dir.join("test.db")).unwrap();
            conn.execute(
                "INSERT INTO users (discord_id, username, display_name, avatar_url, created_at, last_login_at)
                 VALUES ('u1', 'tester', 'Tester', '', 0, 0)",
                [],
            )
            .unwrap();
        }
        let saved = store
            .upsert_pick_pref(1, "Fusilier", "Combi Rifle", fig.id)
            .unwrap();
        assert_eq!(saved.isc, "fusilier");
        assert_eq!(saved.loadout, "combi rifle");
        let other = store
            .create(&FigurineWrite {
                isc: "OTHER".into(),
                loadout: "HMG".into(),
                painter: "Alex".into(),
                asset_bundle_url: "https://example.test/b".into(),
            })
            .unwrap();
        store
            .upsert_pick_pref(1, "Fusilier", "Combi Rifle", other.id)
            .unwrap();
        let prefs = store.list_pick_prefs(1).unwrap();
        assert_eq!(prefs.len(), 1);
        assert_eq!(prefs[0].figurine_id, other.id);
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

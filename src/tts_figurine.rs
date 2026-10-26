//! Figurines peintes : lien loadout TTS (ou orphelin legacy), peintre, asset bundle et photo.

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

/// Écriture d'une figurine.
///
/// Si `tts_loadout_id` est renseigné, `isc` / `loadout` sont recopiés depuis le profil lié.
/// Sinon la figurine est orpheline (legacy) : `isc` est requis.
#[derive(Debug, Clone, Default)]
pub struct FigurineWrite {
    pub tts_loadout_id: Option<i64>,
    pub isc: String,
    pub loadout: String,
    pub painter: String,
    pub asset_bundle_url: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TtsFigurine {
    pub id: i64,
    /// Loadout TTS lié (`NULL` = orpheline / legacy).
    pub tts_loadout_id: Option<i64>,
    /// Nom d'unité affiché (profil lié, sinon legacy).
    #[serde(rename = "nom")]
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
    /// Non liée à un loadout TTS, ou nom, loadout, peintre, asset bundle ou photo manquant.
    pub incomplete: bool,
    /// Uniquement les figurines sans loadout TTS (`tts_loadout_id IS NULL`).
    pub unlinked: bool,
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
    #[serde(rename = "noms")]
    pub iscs: Vec<String>,
}

/// Figurine mémorisée pour un loadout TTS (par utilisateur).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TtsFigurinePickPref {
    pub tts_loadout_id: i64,
    /// Affichage : unité du profil lié.
    #[serde(rename = "nom")]
    pub isc: String,
    /// Affichage : loadout du profil lié.
    pub loadout: String,
    pub figurine_id: i64,
    pub updated_at: u64,
}

/// Meilleur asset bundle pour un ISC/loadout (peintre préféré, loadout exact, URL non vide).
/// Correspondance par nom uniquement (tests / legacy) ; voir [`best_asset_bundle_for_loadout`].
pub fn best_asset_bundle(
    figurines: &[TtsFigurine],
    isc: &str,
    loadout: &str,
    preferred_painter: Option<&str>,
) -> Option<String> {
    best_asset_bundle_for_loadout(figurines, None, isc, loadout, preferred_painter)
}

/// Comme [`best_asset_bundle`], mais préfère les figurines liées à `tts_loadout_id`.
///
/// Avec un id : une figurine liée au même loadout l'emporte ; une figurine liée à un autre
/// loadout est ignorée ; une orpheline reste candidate par nom (fallback ISC).
pub fn best_asset_bundle_for_loadout(
    figurines: &[TtsFigurine],
    tts_loadout_id: Option<i64>,
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
        if figurine.asset_bundle_url.trim().is_empty() {
            continue;
        }
        let mut score = 1;
        match (tts_loadout_id, figurine.tts_loadout_id) {
            (Some(wanted), Some(linked)) if wanted == linked => score += 1000,
            (Some(_), Some(_)) => continue,
            _ => {
                if figurine.isc.trim().to_ascii_lowercase() != isc_key {
                    continue;
                }
            }
        }
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

/// Index d'état des périphériques : jamais de repli sur les figurines du parent.
pub const PERIPHERAL_STATE_INDEX: i64 = 900;

/// Loadout du parent dont les figurines servent de repli pour une forme enfant :
/// uniquement hors périphérique (état ≠ 900) et à silhouette identique.
pub fn parent_fallback_loadout_id(
    form_state_index: i64,
    form_silhouette_size: Option<&str>,
    parent_tts_loadout_id: i64,
    parent_silhouette_size: Option<&str>,
) -> Option<i64> {
    if form_state_index == PERIPHERAL_STATE_INDEX {
        return None;
    }
    let norm = |value: Option<&str>| value.map(|v| v.trim().to_ascii_lowercase());
    (norm(form_silhouette_size) == norm(parent_silhouette_size)).then_some(parent_tts_loadout_id)
}

/// Asset bundle d'une forme : figurine liée à son loadout, sinon (repli) à celui du parent,
/// sinon figurine orpheline par nom.
pub fn best_asset_bundle_for_form(
    figurines: &[TtsFigurine],
    form_tts_loadout_id: i64,
    parent_fallback: Option<i64>,
    isc: &str,
    loadout: &str,
    preferred_painter: Option<&str>,
) -> Option<String> {
    let has_linked = |id: i64| {
        figurines
            .iter()
            .any(|f| f.tts_loadout_id == Some(id) && !f.asset_bundle_url.trim().is_empty())
    };
    let wanted = if has_linked(form_tts_loadout_id) {
        form_tts_loadout_id
    } else if let Some(parent) = parent_fallback.filter(|id| has_linked(*id)) {
        parent
    } else {
        form_tts_loadout_id
    };
    best_asset_bundle_for_loadout(figurines, Some(wanted), isc, loadout, preferred_painter)
}

/// Profil (parent d'abord, puis le plus ancien) portant ce loadout : colonne `u.nom`.
fn profile_nom_sql(loadout_col: &str) -> String {
    format!(
        "(SELECT u.nom FROM tts_profiles p JOIN tts_units u ON u.id = p.unit_id \
         WHERE p.tts_loadout_id = {loadout_col} \
         ORDER BY p.parent_profile_id IS NOT NULL, p.id LIMIT 1)"
    )
}

/// Profil (parent d'abord, puis le plus ancien) portant ce loadout : colonne `p.loadout`.
fn profile_loadout_sql(loadout_col: &str) -> String {
    format!(
        "(SELECT p.loadout FROM tts_profiles p \
         WHERE p.tts_loadout_id = {loadout_col} \
         ORDER BY p.parent_profile_id IS NOT NULL, p.id LIMIT 1)"
    )
}

/// Nom affiché d'une figurine `f` : profil lié, puis legacy.
fn display_nom_sql() -> String {
    format!(
        "COALESCE({}, NULLIF(trim(f.legacy_nom), ''), f.isc)",
        profile_nom_sql("f.tts_loadout_id")
    )
}

/// Loadout affiché d'une figurine `f` : profil lié, puis legacy.
fn display_loadout_sql() -> String {
    format!(
        "COALESCE({}, NULLIF(trim(f.legacy_loadout), ''), f.loadout)",
        profile_loadout_sql("f.tts_loadout_id")
    )
}

fn figurine_select_sql() -> String {
    format!(
        "f.id, {nom} AS disp_nom, {loadout} AS disp_loadout, f.painter, f.asset_bundle_url, \
         f.photo_filename, f.created_at, f.updated_at, f.tts_loadout_id",
        nom = display_nom_sql(),
        loadout = display_loadout_sql(),
    )
}

/// Loadouts TTS des profils (et formes enfants) d'une sectorielle (`?` ×2 : army_id).
const ARMY_LOADOUTS_SQL: &str = "
    SELECT p.tts_loadout_id
    FROM tts_profiles p
    LEFT JOIN tts_profiles par ON par.id = p.parent_profile_id
    WHERE COALESCE(par.army_id, p.army_id) = ?
       OR EXISTS (
            SELECT 1 FROM tts_profile_sectorials s
            WHERE s.profile_id = COALESCE(p.parent_profile_id, p.id) AND s.army_id = ?
       )";

/// Noms d'unités d'une sectorielle (`?` ×2 : army_id).
const ARMY_UNIT_NAMES_SQL: &str = "
    SELECT lower(trim(u.nom))
    FROM tts_profiles p
    JOIN tts_units u ON u.id = p.unit_id
    LEFT JOIN tts_profiles par ON par.id = p.parent_profile_id
    WHERE COALESCE(par.army_id, p.army_id) = ?
       OR EXISTS (
            SELECT 1 FROM tts_profile_sectorials s
            WHERE s.profile_id = COALESCE(p.parent_profile_id, p.id) AND s.army_id = ?
       )";

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
            ..Default::default()
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
        let nom_sql = display_nom_sql();
        let loadout_sql = display_loadout_sql();

        let mut where_parts: Vec<String> = Vec::new();
        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(painter) = painter.as_ref() {
            where_parts.push("lower(trim(f.painter)) = ?".to_string());
            params_vec.push(Box::new(painter.clone()));
        }
        if filter.unlinked {
            where_parts.push("f.tts_loadout_id IS NULL".to_string());
        }
        if filter.incomplete {
            where_parts.push(format!(
                "(
                    f.tts_loadout_id IS NULL
                    OR trim({nom_sql}) = ''
                    OR trim({loadout_sql}) = ''
                    OR trim(f.painter) = ''
                    OR trim(f.asset_bundle_url) = ''
                    OR trim(COALESCE(f.photo_filename, '')) = ''
                )"
            ));
        }
        if let Some(army_id) = army_id {
            where_parts.push(format!(
                "(
                    f.tts_loadout_id IN ({ARMY_LOADOUTS_SQL})
                    OR (
                        f.tts_loadout_id IS NULL
                        AND lower(trim(COALESCE(NULLIF(trim(f.legacy_nom), ''), f.isc)))
                            IN ({ARMY_UNIT_NAMES_SQL})
                    )
                )"
            ));
            for _ in 0..4 {
                params_vec.push(Box::new(army_id));
            }
        }

        let where_sql = if where_parts.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_parts.join(" AND "))
        };
        let select_sql = figurine_select_sql();
        let order_sql = format!(
            "ORDER BY {nom_sql} COLLATE NOCASE, {loadout_sql} COLLATE NOCASE, f.id"
        );

        // Recherche texte : filtre en Rust pour ignorer les accents (í→i…).
        if let Some(needle) = q.as_ref() {
            let list_sql = format!(
                "SELECT {select_sql} FROM tts_figurines f {where_sql} {order_sql}"
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
            "SELECT {select_sql} FROM tts_figurines f {where_sql} {order_sql} LIMIT ? OFFSET ?"
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

    /// Peintres et noms d'unité distincts (filtres / onglet profils).
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
        let nom_sql = display_nom_sql();
        let mut stmt = conn.prepare(&format!(
            "
            SELECT DISTINCT lower(trim({nom_sql})) AS k
            FROM tts_figurines f
            WHERE trim({nom_sql}) != ''
            ORDER BY k
            "
        ))?;
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
        let now = now_unix();
        let conn = self.conn.lock().unwrap();
        let input = resolve_write(&conn, input)?;
        conn.execute(
            "
            INSERT INTO tts_figurines (
                isc, loadout, legacy_nom, legacy_loadout, tts_loadout_id,
                painter, asset_bundle_url, created_at, updated_at
            ) VALUES (?1, ?2, ?1, ?2, ?3, ?4, ?5, ?6, ?6)
            ",
            params![
                input.isc,
                input.loadout,
                input.tts_loadout_id,
                input.painter,
                input.asset_bundle_url,
                now
            ],
        )?;
        let id = conn.last_insert_rowid();
        get_in_conn(&conn, id)?.context("figurine introuvable après création")
    }

    pub fn update(&self, id: i64, input: &FigurineWrite) -> Result<TtsFigurine> {
        let now = now_unix();
        let conn = self.conn.lock().unwrap();
        let input = resolve_write(&conn, input)?;
        let changed = conn.execute(
            "
            UPDATE tts_figurines
            SET isc = ?1, loadout = ?2, legacy_nom = ?1, legacy_loadout = ?2,
                tts_loadout_id = ?3, painter = ?4, asset_bundle_url = ?5, updated_at = ?6
            WHERE id = ?7
            ",
            params![
                input.isc,
                input.loadout,
                input.tts_loadout_id,
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
        let sql = format!(
            "
            SELECT pp.tts_loadout_id,
                   COALESCE({nom}, ''),
                   COALESCE({loadout}, ''),
                   pp.figurine_id, pp.updated_at
            FROM tts_figurine_pick_prefs pp
            WHERE pp.user_id = ?1
            ORDER BY pp.updated_at DESC, pp.tts_loadout_id
            ",
            nom = profile_nom_sql("pp.tts_loadout_id"),
            loadout = profile_loadout_sql("pp.tts_loadout_id"),
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(params![user_id], map_pref_row)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("lecture des préférences de figurines")?;
        Ok(rows)
    }

    pub fn upsert_pick_pref(
        &self,
        user_id: i64,
        tts_loadout_id: i64,
        figurine_id: i64,
    ) -> Result<TtsFigurinePickPref> {
        let now = now_unix();
        let conn = self.conn.lock().unwrap();
        if !loadout_exists(&conn, tts_loadout_id)? {
            bail!("loadout TTS introuvable");
        }
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
            INSERT INTO tts_figurine_pick_prefs (user_id, tts_loadout_id, figurine_id, updated_at)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(user_id, tts_loadout_id) DO UPDATE SET
                figurine_id = excluded.figurine_id,
                updated_at = excluded.updated_at
            ",
            params![user_id, tts_loadout_id, figurine_id, now],
        )?;
        let sql = format!(
            "
            SELECT pp.tts_loadout_id,
                   COALESCE({nom}, ''),
                   COALESCE({loadout}, ''),
                   pp.figurine_id, pp.updated_at
            FROM tts_figurine_pick_prefs pp
            WHERE pp.user_id = ?1 AND pp.tts_loadout_id = ?2
            ",
            nom = profile_nom_sql("pp.tts_loadout_id"),
            loadout = profile_loadout_sql("pp.tts_loadout_id"),
        );
        conn.query_row(&sql, params![user_id, tts_loadout_id], map_pref_row)
            .context("lecture de la préférence de figurine")
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
    let sql = format!(
        "SELECT {} FROM tts_figurines f WHERE f.id = ?1",
        figurine_select_sql()
    );
    conn.query_row(&sql, params![id], map_row)
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
        tts_loadout_id: row.get(8)?,
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

fn map_pref_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TtsFigurinePickPref> {
    Ok(TtsFigurinePickPref {
        tts_loadout_id: row.get(0)?,
        isc: row.get(1)?,
        loadout: row.get(2)?,
        figurine_id: row.get(3)?,
        updated_at: row.get::<_, i64>(4)? as u64,
    })
}

fn loadout_exists(conn: &Connection, tts_loadout_id: i64) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM tts_loadouts WHERE id = ?1)",
        params![tts_loadout_id],
        |row| row.get(0),
    )
    .context("vérification du loadout TTS")
}

/// Valide l'entrée et, si un loadout TTS est lié, recopie nom / loadout depuis son profil.
fn resolve_write(conn: &Connection, input: &FigurineWrite) -> Result<FigurineWrite> {
    let painter = bounded("le nom du peintre", &input.painter, 0, MAX_PAINTER_CHARS)?;
    let asset_bundle_url = normalize_bundle_url(&input.asset_bundle_url)?;
    let typed_isc = bounded("le nom", &input.isc, 0, MAX_ISC_CHARS)?;
    let typed_loadout = bounded("le loadout", &input.loadout, 0, MAX_LOADOUT_CHARS)?;

    let Some(tts_loadout_id) = input.tts_loadout_id else {
        if typed_isc.is_empty() {
            bail!("le nom est requis");
        }
        return Ok(FigurineWrite {
            tts_loadout_id: None,
            isc: typed_isc,
            loadout: typed_loadout,
            painter,
            asset_bundle_url,
        });
    };

    if !loadout_exists(conn, tts_loadout_id)? {
        bail!("loadout TTS introuvable");
    }
    let linked: Option<(String, String)> = conn
        .query_row(
            "
            SELECT u.nom, p.loadout
            FROM tts_profiles p
            JOIN tts_units u ON u.id = p.unit_id
            WHERE p.tts_loadout_id = ?1
            ORDER BY p.parent_profile_id IS NOT NULL, p.id
            LIMIT 1
            ",
            params![tts_loadout_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .context("lecture du profil lié au loadout")?;
    let (isc, loadout) = match linked {
        Some((nom, loadout)) => (nom.trim().to_string(), loadout.trim().to_string()),
        // Loadout sans profil : on garde les libellés saisis.
        None if !typed_isc.is_empty() => (typed_isc, typed_loadout),
        None => bail!("aucun profil n'utilise ce loadout TTS"),
    };
    Ok(FigurineWrite {
        tts_loadout_id: Some(tts_loadout_id),
        isc,
        loadout,
        painter,
        asset_bundle_url,
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

    /// Orpheline (legacy) : `tts_loadout_id` NULL, `isc` seul requis.
    fn orphan(isc: &str, loadout: &str, painter: &str) -> FigurineWrite {
        FigurineWrite {
            tts_loadout_id: None,
            isc: isc.into(),
            loadout: loadout.into(),
            painter: painter.into(),
            asset_bundle_url: "https://example.com/x".into(),
        }
    }

    fn sample() -> FigurineWrite {
        FigurineWrite {
            tts_loadout_id: None,
            isc: " FUSILIER ".to_string(),
            loadout: " Combi ".to_string(),
            painter: " Alex ".to_string(),
            asset_bundle_url: " https://example.com/bundle ".to_string(),
        }
    }

    /// Crée unité + loadout TTS + profil (optionnellement rattaché à une sectorielle).
    /// Retourne `(tts_loadout_id, profile_id)`.
    fn seed_loadout(
        dir: &Path,
        unit: &str,
        loadout: &str,
        army_id: Option<i64>,
    ) -> (i64, i64) {
        let conn = rusqlite::Connection::open(dir.join("test.db")).unwrap();
        if let Some(army_id) = army_id {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS armies (
                    id INTEGER PRIMARY KEY, parent_id INTEGER NOT NULL DEFAULT 0,
                    name TEXT NOT NULL DEFAULT '', slug TEXT NOT NULL DEFAULT '',
                    logo_url TEXT NOT NULL DEFAULT '', discontinued INTEGER NOT NULL DEFAULT 0,
                    synced_at INTEGER NOT NULL DEFAULT 0
                 );",
            )
            .unwrap();
            conn.execute(
                "INSERT OR IGNORE INTO armies (id, name, slug) VALUES (?1, 'A', 'a')",
                params![army_id],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT OR IGNORE INTO tts_units (nom) VALUES (?1)",
            params![unit],
        )
        .unwrap();
        let unit_id: i64 = conn
            .query_row("SELECT id FROM tts_units WHERE nom = ?1", params![unit], |r| r.get(0))
            .unwrap();
        conn.execute(
            "INSERT INTO tts_loadouts (nom_tts, description_tts) VALUES (?1, '')",
            params![unit],
        )
        .unwrap();
        let loadout_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO tts_profiles (
                army_id, faction_label, unit_id, tts_loadout_id, parent_profile_id,
                state_index, loadout, source_bag, source_index
             ) VALUES (?1, 'Test', ?2, ?3, NULL, 0, ?4, '', 0)",
            params![army_id, unit_id, loadout_id, loadout],
        )
        .unwrap();
        (loadout_id, conn.last_insert_rowid())
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
            store.create(&orphan(isc, "Combi", painter)).unwrap();
        }
        let page = store
            .list_page(&FigurineListFilter {
                limit: 2,
                painter: Some("Alex".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(page.total, 3);
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.items[0].isc, "Alpha");
        let page2 = store
            .list_page(&FigurineListFilter {
                limit: 2,
                offset: 2,
                painter: Some("Alex".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(page2.items.len(), 1);
        assert_eq!(page2.items[0].isc, "Delta");
        let search = store
            .list_page(&FigurineListFilter {
                limit: 20,
                q: Some("char".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(search.total, 1);
        assert_eq!(search.items[0].isc, "Charlie");
        store.create(&orphan("Síomón", "Combi", "Alex")).unwrap();
        let accent = store
            .list_page(&FigurineListFilter {
                limit: 20,
                q: Some("siomon".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(accent.total, 1);
        assert_eq!(accent.items[0].isc, "Síomón");
        store.create(&orphan("Incomplete", "", "Alex")).unwrap();
        let incomplete = store
            .list_page(&FigurineListFilter {
                limit: 20,
                incomplete: true,
                ..Default::default()
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
        assert!(created.tts_loadout_id.is_none());
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
                    tts_loadout_id: None,
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
    fn linked_figurine_syncs_from_profile_and_filters_unlinked() {
        let (store, dir) = temp_store();
        let (loadout_id, _) = seed_loadout(&dir, "Fusiliers", "Combi Rifle", None);

        let linked = store
            .create(&FigurineWrite {
                tts_loadout_id: Some(loadout_id),
                isc: "ignored".into(),
                loadout: "ignored".into(),
                painter: "Alex".into(),
                asset_bundle_url: "https://example.com/x".into(),
            })
            .unwrap();
        assert_eq!(linked.tts_loadout_id, Some(loadout_id));
        assert_eq!(linked.isc, "Fusiliers");
        assert_eq!(linked.loadout, "Combi Rifle");
        let orphan_fig = store.create(&orphan("Legacy", "Old", "Léa")).unwrap();
        assert!(orphan_fig.tts_loadout_id.is_none());

        let unlinked = store
            .list_page(&FigurineListFilter {
                limit: 20,
                unlinked: true,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(unlinked.total, 1);
        assert_eq!(unlinked.items[0].id, orphan_fig.id);

        let incomplete = store
            .list_page(&FigurineListFilter {
                limit: 20,
                incomplete: true,
                ..Default::default()
            })
            .unwrap();
        assert!(incomplete.items.iter().any(|f| f.id == orphan_fig.id));
        // Le lié n'a pas de photo : incomplet lui aussi, mais pas « non lié ».
        assert!(incomplete.items.iter().any(|f| f.id == linked.id));

        // Les noms affichés suivent le profil, même si le legacy diverge.
        {
            let conn = rusqlite::Connection::open(dir.join("test.db")).unwrap();
            conn.execute("UPDATE tts_units SET nom = 'Fusiliers V2'", []).unwrap();
        }
        assert_eq!(store.get(linked.id).unwrap().unwrap().isc, "Fusiliers V2");

        assert!(store
            .create(&FigurineWrite {
                tts_loadout_id: Some(9999),
                ..orphan("X", "", "")
            })
            .is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn army_filter_matches_linked_loadouts_and_legacy_orphans() {
        let (store, dir) = temp_store();
        let (in_army, _) = seed_loadout(&dir, "Fusiliers", "Combi Rifle", Some(7));
        let (other_army, _) = seed_loadout(&dir, "Hackers", "Pitcher", Some(8));
        let linked = store
            .create(&FigurineWrite {
                tts_loadout_id: Some(in_army),
                ..orphan("", "", "Alex")
            })
            .unwrap();
        store
            .create(&FigurineWrite {
                tts_loadout_id: Some(other_army),
                ..orphan("", "", "Alex")
            })
            .unwrap();
        let legacy = store.create(&orphan("FUSILIERS", "Whatever", "Alex")).unwrap();
        store.create(&orphan("Nobody", "", "Alex")).unwrap();

        let page = store
            .list_page(&FigurineListFilter {
                limit: 20,
                army_id: Some(7),
                ..Default::default()
            })
            .unwrap();
        let ids: Vec<i64> = page.items.iter().map(|f| f.id).collect();
        assert_eq!(page.total, 2);
        assert!(ids.contains(&linked.id));
        assert!(ids.contains(&legacy.id));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn remembers_pick_pref_per_loadout() {
        let (store, dir) = temp_store();
        let (loadout_id, _) = seed_loadout(&dir, "Fusilier", "Combi Rifle", None);
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
        let saved = store.upsert_pick_pref(1, loadout_id, fig.id).unwrap();
        assert_eq!(saved.tts_loadout_id, loadout_id);
        assert_eq!(saved.isc, "Fusilier");
        assert_eq!(saved.loadout, "Combi Rifle");
        let other = store.create(&orphan("OTHER", "HMG", "Alex")).unwrap();
        store.upsert_pick_pref(1, loadout_id, other.id).unwrap();
        let prefs = store.list_pick_prefs(1).unwrap();
        assert_eq!(prefs.len(), 1);
        assert_eq!(prefs[0].figurine_id, other.id);
        assert_eq!(prefs[0].isc, "Fusilier");
        assert!(store.upsert_pick_pref(1, 9999, fig.id).is_err());
        assert!(store.upsert_pick_pref(1, loadout_id, 9999).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    fn fig(id: i64, loadout_id: Option<i64>, isc: &str, loadout: &str, painter: &str, url: &str) -> TtsFigurine {
        TtsFigurine {
            id,
            tts_loadout_id: loadout_id,
            isc: isc.into(),
            loadout: loadout.into(),
            painter: painter.into(),
            asset_bundle_url: url.into(),
            photo_url: None,
            created_at: 0,
            updated_at: 0,
            sectorial_ids: Vec::new(),
        }
    }

    #[test]
    fn best_asset_prefers_painter_and_loadout() {
        let figurines = vec![
            fig(1, None, "DOG-WARRIOR FORM", "Chain Rifle", "Alex", "https://a"),
            fig(2, None, "DOG-WARRIOR FORM", "Chain Rifle", "Léa", "https://b"),
        ];
        assert_eq!(
            best_asset_bundle(&figurines, "dog-warrior form", "Chain Rifle", Some("Léa"))
                .as_deref(),
            Some("https://b")
        );
    }

    #[test]
    fn best_asset_prefers_matching_loadout_id() {
        let figurines = vec![
            fig(1, Some(10), "Dog", "Rifle", "Alex", "https://linked-other"),
            fig(2, Some(11), "Dog", "Rifle", "Léa", "https://linked-match"),
            fig(3, None, "Dog", "Rifle", "Léa", "https://orphan"),
        ];
        assert_eq!(
            best_asset_bundle_for_loadout(&figurines, Some(11), "Dog", "Rifle", Some("Alex"))
                .as_deref(),
            Some("https://linked-match")
        );
        // Sans figurine liée : repli ISC sur les orphelines, jamais sur un autre loadout lié.
        assert_eq!(
            best_asset_bundle_for_loadout(&figurines, Some(12), "Dog", "Rifle", None).as_deref(),
            Some("https://orphan")
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

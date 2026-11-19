//! Administration des profils d'unité : profil TTS partagé et loadout de figurine.

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::tts_profile::{split_nickname, strip_tts_markup};

#[derive(Debug, Serialize)]
pub struct AdminUnit {
    pub id: i64,
    pub nom: String,
    pub profile_count: i64,
}

#[derive(Debug, Serialize)]
pub struct AdminProfile {
    pub id: i64,
    pub loadout: String,
    pub faction_label: String,
    pub parent_profile_id: Option<i64>,
    pub state_index: i64,
    pub tts_id: i64,
    pub nom_tts: String,
    pub description_tts: String,
    pub loadout_id: i64,
    pub loadout_nom: String,
    pub figurine_count: i64,
}

#[derive(Debug, Serialize)]
pub struct TtsChoice {
    pub id: i64,
    pub nom_tts: String,
    pub description_tts: String,
    pub profile_count: i64,
}

#[derive(Debug, Serialize)]
pub struct LoadoutChoice {
    pub id: i64,
    pub nom: String,
    pub silhouette_size: Option<String>,
    pub silhouette_type: Option<String>,
    pub marker_name: Option<String>,
    pub profile_count: i64,
    pub figurine_count: i64,
}

#[derive(Debug, Serialize)]
pub struct AdminFigurine {
    pub id: i64,
    pub painter: String,
    pub photo_filename: Option<String>,
    pub loadout_id: i64,
    pub loadout_nom: String,
}

#[derive(Debug, Serialize)]
pub struct AdminUnitDetail {
    pub unit: AdminUnit,
    pub profiles: Vec<AdminProfile>,
    pub tts_profiles: Vec<TtsChoice>,
    pub loadouts: Vec<LoadoutChoice>,
    pub figurines: Vec<AdminFigurine>,
}

#[derive(Debug, Deserialize)]
pub struct AssignProfileRequest {
    pub tts_id: i64,
    pub loadout_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct AssignFigurineLoadoutRequest {
    pub loadout_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct CreateTtsRequest {
    pub nom_tts: String,
    #[serde(default)]
    pub description_tts: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTtsRequest {
    pub nom_tts: String,
    #[serde(default)]
    pub description_tts: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateLoadoutRequest {
    pub nom: String,
    #[serde(default)]
    pub silhouette_size: Option<String>,
    #[serde(default)]
    pub silhouette_type: Option<String>,
    #[serde(default)]
    pub marker_name: Option<String>,
}

/// Nom de loadout de figurine : uniquement armes / équipements, sans `[FFFFFF]UNITÉ[-]`.
pub fn figurine_loadout_label(nom: &str) -> String {
    let (_, equipment) = split_nickname(nom);
    if !equipment.is_empty() {
        return equipment;
    }
    strip_tts_markup(nom).trim().to_string()
}

/// Loadouts d'une unité (même ensemble que l'onglet Loadout admin).
pub fn loadouts_for_unit(conn: &Connection, unit_id: i64) -> Result<Vec<LoadoutChoice>> {
    let mut loadout_stmt = conn.prepare(
        "
        SELECT l.id, l.nom, l.silhouette_size, l.silhouette_type, l.marker_name,
               (SELECT COUNT(*) FROM unit_profiles p WHERE p.loadout_id = l.id),
               (SELECT COUNT(*) FROM tts_figurine_rows g WHERE g.loadout_id = l.id)
        FROM unit_loadouts l
        WHERE l.id IN (
            SELECT p.loadout_id FROM unit_profiles p WHERE p.unit_id = ?1
            UNION
            SELECT map.loadout_id
            FROM unit_loadout_tts map
            WHERE map.tts_id IN (
                SELECT p.tts_id FROM unit_profiles p WHERE p.unit_id = ?1
            )
            UNION
            SELECT g.loadout_id
            FROM tts_figurine_rows g
            WHERE g.loadout_id IN (
                SELECT p.loadout_id FROM unit_profiles p WHERE p.unit_id = ?1
                UNION
                SELECT map.loadout_id
                FROM unit_loadout_tts map
                WHERE map.tts_id IN (
                    SELECT p.tts_id FROM unit_profiles p WHERE p.unit_id = ?1
                )
            )
        )
        ORDER BY l.nom COLLATE NOCASE, l.id
        ",
    )?;
    let loadouts = loadout_stmt
        .query_map(params![unit_id], |row| {
            let nom: String = row.get(1)?;
            Ok(LoadoutChoice {
                id: row.get(0)?,
                nom: figurine_loadout_label(&nom),
                silhouette_size: row.get(2)?,
                silhouette_type: row.get(3)?,
                marker_name: row.get(4)?,
                profile_count: row.get(5)?,
                figurine_count: row.get(6)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("lecture des loadouts")?;
    Ok(loadouts)
}

/// Unités présentes dans une sectorielle (profils catalogue ou liés).
pub fn unit_ids_for_army(conn: &Connection, army_id: i64) -> Result<Vec<(i64, String)>> {
    let mut stmt = conn.prepare(
        "
        SELECT DISTINCT u.id, u.nom
        FROM units u
        JOIN unit_profiles p ON p.unit_id = u.id
        LEFT JOIN unit_profiles par ON par.id = p.parent_profile_id
        WHERE COALESCE(par.army_id, p.army_id) = ?1
           OR EXISTS (
                SELECT 1 FROM unit_profile_sectorials s
                WHERE s.profile_id = COALESCE(p.parent_profile_id, p.id) AND s.army_id = ?1
           )
        ORDER BY u.nom COLLATE NOCASE, u.id
        ",
    )?;
    let units = stmt
        .query_map(params![army_id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("lecture des unités de la sectorielle")?;
    Ok(units)
}

/// Toutes les unités ayant au moins un loadout / profil.
pub fn all_units_with_profiles(conn: &Connection) -> Result<Vec<(i64, String)>> {
    let mut stmt = conn.prepare(
        "
        SELECT DISTINCT u.id, u.nom
        FROM units u
        JOIN unit_profiles p ON p.unit_id = u.id
        ORDER BY u.nom COLLATE NOCASE, u.id
        ",
    )?;
    let units = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("lecture des unités")?;
    Ok(units)
}

pub fn clean_figurine_loadout_names(conn: &Connection) -> Result<()> {
    if !table_exists(conn, "unit_loadouts")? {
        return Ok(());
    }
    let mut stmt = conn.prepare("SELECT id, nom FROM unit_loadouts")?;
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for (id, nom) in rows {
        let cleaned = figurine_loadout_label(&nom);
        if cleaned != nom {
            conn.execute(
                "UPDATE unit_loadouts SET nom = ?1 WHERE id = ?2",
                params![cleaned, id],
            )?;
        }
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct AdminUnitPage {
    pub items: Vec<AdminUnit>,
    pub total: i64,
    pub limit: u32,
    pub offset: u32,
}

pub fn list_units(
    conn: &Connection,
    query: &str,
    army_id: Option<i64>,
    limit: u32,
    offset: u32,
) -> Result<AdminUnitPage> {
    let needle = query.trim();
    let limit = limit.clamp(1, 100);
    let total: i64 = conn.query_row(
        "
        SELECT COUNT(*)
        FROM (
            SELECT u.id
            FROM units u
            JOIN unit_profiles p ON p.unit_id = u.id
            WHERE (?1 = '' OR u.nom LIKE '%' || ?1 || '%' COLLATE NOCASE)
              AND (
                ?2 IS NULL
                OR p.army_id = ?2
                OR EXISTS (
                    SELECT 1
                    FROM tts_profile_sectorials s
                    WHERE s.army_id = ?2
                      AND s.profile_id = COALESCE(p.parent_profile_id, p.id)
                )
              )
            GROUP BY u.id
            HAVING COUNT(p.id) > 0
        )
        ",
        params![needle, army_id],
        |row| row.get(0),
    )?;
    let mut stmt = conn.prepare(
        "
        SELECT u.id, u.nom, COUNT(p.id)
        FROM units u
        JOIN unit_profiles p ON p.unit_id = u.id
        WHERE (?1 = '' OR u.nom LIKE '%' || ?1 || '%' COLLATE NOCASE)
          AND (
            ?2 IS NULL
            OR p.army_id = ?2
            OR EXISTS (
                SELECT 1
                FROM tts_profile_sectorials s
                WHERE s.army_id = ?2
                  AND s.profile_id = COALESCE(p.parent_profile_id, p.id)
            )
          )
        GROUP BY u.id
        HAVING COUNT(p.id) > 0
        ORDER BY u.nom COLLATE NOCASE
        LIMIT ?3 OFFSET ?4
        ",
    )?;
    let items = stmt
        .query_map(params![needle, army_id, limit, offset], |row| {
            Ok(AdminUnit {
                id: row.get(0)?,
                nom: row.get(1)?,
                profile_count: row.get(2)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("lecture des unités")?;
    Ok(AdminUnitPage {
        items,
        total,
        limit,
        offset,
    })
}

pub fn unit_detail(conn: &Connection, unit_id: i64) -> Result<AdminUnitDetail> {
    let unit = conn
        .query_row(
            "
            SELECT u.id, u.nom, COUNT(p.id)
            FROM units u
            LEFT JOIN unit_profiles p ON p.unit_id = u.id
            WHERE u.id = ?1
            GROUP BY u.id
            ",
            params![unit_id],
            |row| {
                Ok(AdminUnit {
                    id: row.get(0)?,
                    nom: row.get(1)?,
                    profile_count: row.get(2)?,
                })
            },
        )
        .optional()
        .context("lecture de l'unité")?
        .context("unité introuvable")?;

    let mut profiles_stmt = conn.prepare(
        "
        SELECT p.id, p.loadout, f.label, p.parent_profile_id, p.state_index,
               p.tts_id, t.nom_tts, t.description_tts, p.loadout_id, l.nom,
               (SELECT COUNT(*) FROM tts_figurine_rows g WHERE g.loadout_id = p.loadout_id)
        FROM unit_profiles p
        JOIN unit_factions f ON f.id = p.faction_id
        JOIN unit_tts t ON t.id = p.tts_id
        JOIN unit_loadouts l ON l.id = p.loadout_id
        WHERE p.unit_id = ?1
        ORDER BY p.parent_profile_id IS NOT NULL, p.loadout COLLATE NOCASE, p.state_index, p.id
        ",
    )?;
    let profiles = profiles_stmt
        .query_map(params![unit_id], |row| {
            let loadout_nom: String = row.get(9)?;
            Ok(AdminProfile {
                id: row.get(0)?,
                loadout: row.get(1)?,
                faction_label: row.get(2)?,
                parent_profile_id: row.get(3)?,
                state_index: row.get(4)?,
                tts_id: row.get(5)?,
                nom_tts: row.get(6)?,
                description_tts: row.get(7)?,
                loadout_id: row.get(8)?,
                loadout_nom: figurine_loadout_label(&loadout_nom),
                figurine_count: row.get(10)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("lecture des profils")?;

    // Profils TTS liés à l'unité (FK actuelles + graines des loadouts liés).
    let mut tts_stmt = conn.prepare(
        "
        SELECT t.id, t.nom_tts, t.description_tts,
               (SELECT COUNT(*) FROM unit_profiles p WHERE p.tts_id = t.id)
        FROM unit_tts t
        WHERE t.id IN (
            SELECT p.tts_id FROM unit_profiles p WHERE p.unit_id = ?1
            UNION
            SELECT map.tts_id
            FROM unit_loadout_tts map
            WHERE map.loadout_id IN (
                SELECT p.loadout_id FROM unit_profiles p WHERE p.unit_id = ?1
            )
            UNION
            SELECT map.tts_id
            FROM unit_loadout_tts map
            WHERE map.loadout_id IN (
                SELECT g.loadout_id FROM tts_figurine_rows g
                WHERE g.loadout_id IN (
                    SELECT p.loadout_id FROM unit_profiles p WHERE p.unit_id = ?1
                    UNION
                    SELECT map2.loadout_id
                    FROM unit_loadout_tts map2
                    WHERE map2.tts_id IN (
                        SELECT p.tts_id FROM unit_profiles p WHERE p.unit_id = ?1
                    )
                )
            )
        )
        ORDER BY t.nom_tts COLLATE NOCASE, t.id
        ",
    )?;
    let tts_profiles = tts_stmt
        .query_map(params![unit_id], |row| {
            Ok(TtsChoice {
                id: row.get(0)?,
                nom_tts: row.get(1)?,
                description_tts: row.get(2)?,
                profile_count: row.get(3)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("lecture des profils TTS")?;

    let loadouts = loadouts_for_unit(conn, unit_id)?;

    let mut figurines_stmt = conn.prepare(
        "
        SELECT g.id, g.painter, g.photo_filename, g.loadout_id, l.nom
        FROM tts_figurine_rows g
        JOIN unit_loadouts l ON l.id = g.loadout_id
        WHERE g.loadout_id IN (
            SELECT p.loadout_id FROM unit_profiles p WHERE p.unit_id = ?1
            UNION
            SELECT map.loadout_id
            FROM unit_loadout_tts map
            WHERE map.tts_id IN (
                SELECT p.tts_id FROM unit_profiles p WHERE p.unit_id = ?1
            )
            UNION
            SELECT g2.loadout_id
            FROM tts_figurine_rows g2
            WHERE g2.loadout_id IN (
                SELECT p.loadout_id FROM unit_profiles p WHERE p.unit_id = ?1
                UNION
                SELECT map.loadout_id
                FROM unit_loadout_tts map
                WHERE map.tts_id IN (
                    SELECT p.tts_id FROM unit_profiles p WHERE p.unit_id = ?1
                )
            )
        )
        ORDER BY l.nom COLLATE NOCASE, g.painter COLLATE NOCASE, g.id
        ",
    )?;
    let figurines = figurines_stmt
        .query_map(params![unit_id], |row| {
            let loadout_nom: String = row.get(4)?;
            Ok(AdminFigurine {
                id: row.get(0)?,
                painter: row.get(1)?,
                photo_filename: row.get(2)?,
                loadout_id: row.get(3)?,
                loadout_nom: figurine_loadout_label(&loadout_nom),
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("lecture des figurines")?;

    Ok(AdminUnitDetail {
        unit,
        profiles,
        tts_profiles,
        loadouts,
        figurines,
    })
}

pub fn assign_profile(
    conn: &Connection,
    profile_id: i64,
    body: &AssignProfileRequest,
) -> Result<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM unit_profiles WHERE id = ?1)",
        params![profile_id],
        |row| row.get(0),
    )?;
    if !exists {
        bail!("profil introuvable");
    }
    let tts_exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM unit_tts WHERE id = ?1)",
        params![body.tts_id],
        |row| row.get(0),
    )?;
    if !tts_exists {
        bail!("profil TTS introuvable");
    }
    let loadout_exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM unit_loadouts WHERE id = ?1)",
        params![body.loadout_id],
        |row| row.get(0),
    )?;
    if !loadout_exists {
        bail!("loadout de figurine introuvable");
    }

    // Pas de suppression automatique : les orphelins restent listés pour nettoyage manuel.
    conn.execute(
        "UPDATE unit_profiles SET tts_id = ?1, loadout_id = ?2 WHERE id = ?3",
        params![body.tts_id, body.loadout_id, profile_id],
    )?;
    Ok(())
}

pub fn assign_figurine_loadout(
    conn: &Connection,
    figurine_id: i64,
    body: &AssignFigurineLoadoutRequest,
) -> Result<i64> {
    let old_loadout: i64 = conn
        .query_row(
            "SELECT loadout_id FROM tts_figurine_rows WHERE id = ?1",
            params![figurine_id],
            |row| row.get(0),
        )
        .optional()
        .context("lecture de la figurine")?
        .context("figurine introuvable")?;
    let loadout_exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM unit_loadouts WHERE id = ?1)",
        params![body.loadout_id],
        |row| row.get(0),
    )?;
    if !loadout_exists {
        bail!("loadout de figurine introuvable");
    }

    let unit_id: i64 = conn
        .query_row(
            "
            SELECT p.unit_id
            FROM unit_profiles p
            WHERE p.loadout_id IN (?1, ?2)
            ORDER BY CASE WHEN p.loadout_id = ?1 THEN 0 ELSE 1 END, p.id
            LIMIT 1
            ",
            params![old_loadout, body.loadout_id],
            |row| row.get(0),
        )
        .optional()
        .context("unité liée à la figurine")?
        .context("aucun profil d'unité n'utilise ce loadout")?;

    conn.execute(
        "
        UPDATE tts_figurine_rows
        SET loadout_id = ?1, updated_at = CAST(strftime('%s','now') AS INTEGER)
        WHERE id = ?2
        ",
        params![body.loadout_id, figurine_id],
    )?;
    Ok(unit_id)
}

pub fn delete_tts(conn: &Connection, tts_id: i64) -> Result<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM unit_tts WHERE id = ?1)",
        params![tts_id],
        |row| row.get(0),
    )?;
    if !exists {
        bail!("profil TTS introuvable");
    }
    let profile_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM unit_profiles WHERE tts_id = ?1",
        params![tts_id],
        |row| row.get(0),
    )?;
    if profile_count > 0 {
        bail!("ce profil TTS est encore utilisé par {profile_count} profil(s)");
    }
    conn.execute(
        "DELETE FROM unit_loadout_tts WHERE tts_id = ?1",
        params![tts_id],
    )?;
    conn.execute("DELETE FROM unit_tts WHERE id = ?1", params![tts_id])?;
    Ok(())
}

pub fn delete_loadout(conn: &Connection, loadout_id: i64) -> Result<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM unit_loadouts WHERE id = ?1)",
        params![loadout_id],
        |row| row.get(0),
    )?;
    if !exists {
        bail!("loadout introuvable");
    }
    let profile_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM unit_profiles WHERE loadout_id = ?1",
        params![loadout_id],
        |row| row.get(0),
    )?;
    if profile_count > 0 {
        bail!("ce loadout est encore utilisé par {profile_count} profil(s)");
    }
    let figurine_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tts_figurine_rows WHERE loadout_id = ?1",
        params![loadout_id],
        |row| row.get(0),
    )?;
    if figurine_count > 0 {
        bail!("ce loadout a encore {figurine_count} figurine(s)");
    }
    conn.execute(
        "DELETE FROM unit_loadout_tts WHERE loadout_id = ?1",
        params![loadout_id],
    )?;
    conn.execute("DELETE FROM unit_loadouts WHERE id = ?1", params![loadout_id])?;
    Ok(())
}

pub fn create_tts(conn: &Connection, body: &CreateTtsRequest) -> Result<i64> {
    let nom = body.nom_tts.trim();
    if nom.is_empty() {
        bail!("indiquez un nom de profil TTS");
    }
    conn.execute(
        "INSERT INTO unit_tts (nom_tts, description_tts) VALUES (?1, ?2)",
        params![nom, body.description_tts.trim()],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_tts(conn: &Connection, tts_id: i64, body: &UpdateTtsRequest) -> Result<()> {
    let nom = body.nom_tts.trim();
    if nom.is_empty() {
        bail!("indiquez un nom de profil TTS");
    }
    let changed = conn.execute(
        "UPDATE unit_tts SET nom_tts = ?1, description_tts = ?2 WHERE id = ?3",
        params![nom, body.description_tts.trim(), tts_id],
    )?;
    if changed == 0 {
        bail!("profil TTS introuvable");
    }
    Ok(())
}

pub fn create_loadout(conn: &Connection, body: &CreateLoadoutRequest) -> Result<i64> {
    let nom = figurine_loadout_label(&body.nom);
    if nom.is_empty() {
        bail!("indiquez un nom de loadout");
    }
    conn.execute(
        "
        INSERT INTO unit_loadouts (nom, silhouette_size, silhouette_type, marker_name)
        VALUES (?1, ?2, ?3, ?4)
        ",
        params![
            nom,
            blank_to_none(body.silhouette_size.as_deref()),
            blank_to_none(body.silhouette_type.as_deref()),
            blank_to_none(body.marker_name.as_deref()),
        ],
    )?;
    let loadout_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO unit_tts (nom_tts, description_tts) VALUES (?1, '')",
        params![nom],
    )?;
    let tts_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO unit_loadout_tts (loadout_id, tts_id) VALUES (?1, ?2)",
        params![loadout_id, tts_id],
    )?;
    Ok(loadout_id)
}

fn table_exists(conn: &Connection, name: &str) -> Result<bool> {
    let found: Option<String> = conn
        .query_row(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?1",
            params![name],
            |row| row.get(0),
        )
        .optional()?;
    Ok(found.is_some())
}

fn blank_to_none(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::figurine_loadout_label;

    #[test]
    fn figurine_loadout_keeps_only_equipment() {
        assert_eq!(
            figurine_loadout_label("[FFFFFF]FUSILIER[-] Combi Rifle"),
            "Combi Rifle"
        );
        assert_eq!(
            figurine_loadout_label("[00B0F2]FUSILIER[-] Combi Rifle, Flash Pulse"),
            "Combi Rifle, Flash Pulse"
        );
        assert_eq!(figurine_loadout_label("Missile Launcher"), "Missile Launcher");
    }
}

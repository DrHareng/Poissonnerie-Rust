//! Table canonique `army_lists` et statistiques dérivées des matchs éligibles.

use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::army_list::{normalize_army_list_code, parse_army_list_faction_slug, parse_army_list_name};
use crate::migrate::migrate;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ArmyList {
    pub id: i64,
    pub code: String,
    pub army_id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ArmyListStatsEntry {
    pub id: i64,
    pub code: String,
    pub army_id: u32,
    pub name: Option<String>,
    pub wins: u32,
    pub draws: u32,
    pub losses: u32,
    pub games: u32,
    pub win_rate: f64,
    pub last_used_at: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ArmyListStatsGroup {
    pub army_id: u32,
    pub lists: Vec<ArmyListStatsEntry>,
}

pub struct ArmyListStore {
    conn: Mutex<Connection>,
}

impl ArmyListStore {
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

    /// Normalise le code, déduit la sectorielle, get-or-create.
    pub fn get_or_create(&self, raw: &str) -> Result<ArmyList> {
        let conn = self.conn.lock().unwrap();
        get_or_create_in_conn(&conn, raw)
    }

    pub fn get(&self, id: i64) -> Result<Option<ArmyList>> {
        let conn = self.conn.lock().unwrap();
        get_in_conn(&conn, id)
    }

    pub fn list_stats_by_army(&self, army_ids: Option<&[u32]>) -> Result<Vec<ArmyListStatsGroup>> {
        let conn = self.conn.lock().unwrap();
        list_stats_by_army_in_conn(&conn, army_ids)
    }

    pub fn army_ids_with_public_lists(&self) -> Result<Vec<u32>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "
            SELECT DISTINCT al.army_id
            FROM army_lists al
            INNER JOIN matches m ON (
                (m.player1_army_list_id = al.id OR m.player2_army_list_id = al.id)
                AND m.status = 'completed'
                AND (
                    m.tournament_id IS NULL
                    OR EXISTS (
                        SELECT 1 FROM tournaments t
                        WHERE t.id = m.tournament_id AND t.status = 'completed'
                    )
                )
            )
            ORDER BY al.army_id
            ",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, u32>(0))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn list_match_ids(&self, army_list_id: i64, limit: usize) -> Result<Vec<u64>> {
        let conn = self.conn.lock().unwrap();
        list_match_ids_in_conn(&conn, army_list_id, limit)
    }
}

pub fn get_or_create_in_conn(conn: &Connection, raw: &str) -> Result<ArmyList> {
    let code = normalize_army_list_code(raw).context("code de liste invalide")?;
    let slug = parse_army_list_faction_slug(&code).with_context(|| {
        "impossible de déduire la sectorielle depuis ce code — vérifiez la liste avec l'orga ou le vérificateur"
    })?;
    let army_id: u32 = conn
        .query_row(
            "SELECT id FROM armies WHERE lower(slug) = lower(?1)",
            params![slug],
            |row| row.get(0),
        )
        .with_context(|| format!("sectorielle inconnue pour le slug « {slug} »"))?;

    if let Some(existing) = get_by_code_in_conn(conn, &code)? {
        if existing.name.is_none() {
            if let Some(name) = parse_army_list_name(&code) {
                conn.execute(
                    "UPDATE army_lists SET name = ?1 WHERE id = ?2",
                    params![name, existing.id],
                )?;
                return Ok(ArmyList {
                    name: Some(name),
                    ..existing
                });
            }
        }
        return Ok(existing);
    }

    let name = parse_army_list_name(&code);
    conn.execute(
        "INSERT INTO army_lists (code, army_id, name) VALUES (?1, ?2, ?3)",
        params![code, army_id, name],
    )?;
    let id = conn.last_insert_rowid();
    Ok(ArmyList {
        id,
        code,
        army_id,
        name,
    })
}

fn get_by_code_in_conn(conn: &Connection, code: &str) -> Result<Option<ArmyList>> {
    let mut stmt = conn.prepare("SELECT id, code, army_id, name FROM army_lists WHERE code = ?1")?;
    let mut rows = stmt.query(params![code])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(map_army_list_row(row)?));
    }
    Ok(None)
}

fn get_in_conn(conn: &Connection, id: i64) -> Result<Option<ArmyList>> {
    let mut stmt = conn.prepare("SELECT id, code, army_id, name FROM army_lists WHERE id = ?1")?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(map_army_list_row(row)?));
    }
    Ok(None)
}

fn map_army_list_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ArmyList> {
    Ok(ArmyList {
        id: row.get(0)?,
        code: row.get(1)?,
        army_id: row.get(2)?,
        name: row.get(3)?,
    })
}

fn list_stats_by_army_in_conn(
    conn: &Connection,
    army_ids: Option<&[u32]>,
) -> Result<Vec<ArmyListStatsGroup>> {
    const BASE_SQL: &str = "
        WITH eligible AS (
            SELECT
                m.outcome,
                m.recorded_at,
                m.player1_army_list_id AS p1_list,
                m.player2_army_list_id AS p2_list
            FROM matches m
            WHERE m.status = 'completed'
              AND (
                m.tournament_id IS NULL
                OR EXISTS (
                    SELECT 1 FROM tournaments t
                    WHERE t.id = m.tournament_id AND t.status = 'completed'
                )
              )
              AND (m.player1_army_list_id IS NOT NULL OR m.player2_army_list_id IS NOT NULL)
        ),
        appearances AS (
            SELECT p1_list AS army_list_id, outcome, recorded_at, 1 AS side
            FROM eligible WHERE p1_list IS NOT NULL
            UNION ALL
            SELECT p2_list AS army_list_id, outcome, recorded_at, 2 AS side
            FROM eligible WHERE p2_list IS NOT NULL
        ),
        scored AS (
            SELECT
                army_list_id,
                recorded_at,
                CASE
                    WHEN side = 1 AND outcome = 'player1_win' THEN 'win'
                    WHEN side = 1 AND outcome = 'player2_win' THEN 'loss'
                    WHEN side = 2 AND outcome = 'player2_win' THEN 'win'
                    WHEN side = 2 AND outcome = 'player1_win' THEN 'loss'
                    ELSE 'draw'
                END AS result
            FROM appearances
        )
        SELECT
            al.id,
            al.code,
            al.army_id,
            al.name,
            SUM(CASE WHEN s.result = 'win' THEN 1 ELSE 0 END) AS wins,
            SUM(CASE WHEN s.result = 'draw' THEN 1 ELSE 0 END) AS draws,
            SUM(CASE WHEN s.result = 'loss' THEN 1 ELSE 0 END) AS losses,
            COUNT(*) AS games,
            MAX(s.recorded_at) AS last_used_at
        FROM army_lists al
        INNER JOIN scored s ON s.army_list_id = al.id
    ";

    let entries = if let Some(ids) = army_ids {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let sql = format!(
            "{BASE_SQL} WHERE al.army_id IN ({placeholders}) GROUP BY al.id ORDER BY al.army_id ASC, last_used_at DESC"
        );
        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<rusqlite::types::Value> = ids
            .iter()
            .map(|id| rusqlite::types::Value::from(*id))
            .collect();
        let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), map_stats_row)?;
        rows.collect::<Result<Vec<_>, _>>()?
    } else {
        let sql =
            format!("{BASE_SQL} GROUP BY al.id ORDER BY al.army_id ASC, last_used_at DESC");
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], map_stats_row)?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let mut groups: Vec<ArmyListStatsGroup> = Vec::new();
    for entry in entries {
        if let Some(group) = groups.last_mut() {
            if group.army_id == entry.army_id {
                group.lists.push(entry);
                continue;
            }
        }
        groups.push(ArmyListStatsGroup {
            army_id: entry.army_id,
            lists: vec![entry],
        });
    }
    Ok(groups)
}

fn list_match_ids_in_conn(
    conn: &Connection,
    army_list_id: i64,
    limit: usize,
) -> Result<Vec<u64>> {
    let limit = limit.clamp(1, 500) as i64;
    let mut stmt = conn.prepare(
        "
        SELECT m.id
        FROM matches m
        WHERE m.status = 'completed'
          AND (m.player1_army_list_id = ?1 OR m.player2_army_list_id = ?1)
          AND (
            m.tournament_id IS NULL
            OR EXISTS (
                SELECT 1 FROM tournaments t
                WHERE t.id = m.tournament_id AND t.status = 'completed'
            )
          )
        ORDER BY m.recorded_at DESC
        LIMIT ?2
        ",
    )?;
    let rows = stmt.query_map(params![army_list_id, limit], |row| {
        row.get::<_, i64>(0).map(|id| id as u64)
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn map_stats_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ArmyListStatsEntry> {
    let wins: u32 = row.get(4)?;
    let draws: u32 = row.get(5)?;
    let losses: u32 = row.get(6)?;
    let games: u32 = row.get(7)?;
    let win_rate = if games == 0 {
        0.0
    } else {
        (wins as f64 + 0.5 * draws as f64) / games as f64 * 100.0
    };
    Ok(ArmyListStatsEntry {
        id: row.get(0)?,
        code: row.get(1)?,
        army_id: row.get(2)?,
        name: row.get(3)?,
        wins,
        draws,
        losses,
        games,
        win_rate,
        last_used_at: row.get(8)?,
    })
}

/// Remplit `army_lists.name` depuis le code encodé.
pub fn backfill_army_list_names(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("SELECT id, code FROM army_lists WHERE name IS NULL OR trim(name) = ''")?;
    let rows: Vec<(i64, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .filter_map(|row| row.ok())
        .collect();

    for (id, code) in rows {
        let Some(name) = parse_army_list_name(&code) else {
            continue;
        };
        conn.execute(
            "UPDATE army_lists SET name = ?1 WHERE id = ?2",
            params![name, id],
        )?;
    }
    Ok(())
}

/// Migration : backfill `army_lists` et FK depuis les codes TEXT existants.
pub fn backfill_army_list_references(conn: &Connection) -> Result<()> {
    backfill_codes_from_column(conn, "matches", "player1_army_list_code", "player1_army_list_id")?;
    backfill_codes_from_column(conn, "matches", "player2_army_list_code", "player2_army_list_id")?;
    backfill_codes_from_column(
        conn,
        "tournament_matches",
        "player1_army_list_code",
        "player1_army_list_id",
    )?;
    backfill_codes_from_column(
        conn,
        "tournament_matches",
        "player2_army_list_code",
        "player2_army_list_id",
    )?;
    backfill_codes_from_column(
        conn,
        "tournament_registrations",
        "army_list_1",
        "army_list_1_id",
    )?;
    backfill_codes_from_column(
        conn,
        "tournament_registrations",
        "army_list_2",
        "army_list_2_id",
    )?;
    backfill_codes_from_column(
        conn,
        "tournament_registrations",
        "bracket_list_1",
        "bracket_list_1_id",
    )?;
    backfill_codes_from_column(
        conn,
        "tournament_registrations",
        "bracket_list_2",
        "bracket_list_2_id",
    )?;
    Ok(())
}

fn backfill_codes_from_column(
    conn: &Connection,
    table: &str,
    code_col: &str,
    id_col: &str,
) -> Result<()> {
    let sql = format!(
        "SELECT id, {code_col} FROM {table} WHERE {code_col} IS NOT NULL AND trim({code_col}) != '' AND ({id_col} IS NULL)"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows: Vec<(i64, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();

    for (row_id, raw) in rows {
        let Ok(list) = get_or_create_in_conn(conn, &raw) else {
            continue;
        };
        let update = format!("UPDATE {table} SET {id_col} = ?1 WHERE id = ?2");
        conn.execute(&update, params![list.id, row_id])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::army::ArmyStore;

    fn test_conn() -> Connection {
        let path = std::env::temp_dir().join(format!(
            "poissonnerie-army-lists-{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_file(&path);
        ArmyStore::open(&path).unwrap();
        Connection::open(&path).unwrap()
    }

    #[test]
    fn get_or_create_is_idempotent() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO armies (id, parent_id, name, slug, logo_url, discontinued, synced_at)
             VALUES (201, 0, 'Hassassin Bahram', 'hassassin-bahram', '', 0, 0)",
            [],
        )
        .unwrap();
        let code = "gZIQaGFzc2Fzc2luLWJhaHJhbRBOb3V2ZWF1IGdvIHRvIHYygSwCAQEACgCBMAECAACGNQECAACBRwEGAACBTgEBAACBLQELAACBLQEOAACBUQECAACBGwEBAACBTAECAACGCwEDAAIBAAUAgUgBAwAAgTABBgAAgT4BAQAAgVQBAwAAhgkBAgA=";
        let a = get_or_create_in_conn(&conn, code).unwrap();
        let b = get_or_create_in_conn(&conn, code).unwrap();
        assert_eq!(a.id, b.id);
        assert_eq!(a.code, b.code);
        assert_eq!(a.army_id, 201);
    }

    #[test]
    fn rejects_unparseable_code() {
        let conn = test_conn();
        assert!(get_or_create_in_conn(&conn, "not-a-real-list").is_err());
    }
}

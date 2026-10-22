//! Remap des ISC courts de figurines (ex. `COMBAT FORM`) vers les ISC qualifiés
//! des formes de transmute (`Sù-Jiàn… COMBAT FORM`).

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{params, Connection};

use crate::migrate::migrate;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FigurineIscFix {
    pub id: i64,
    pub painter: String,
    pub loadout: String,
    pub from_isc: String,
    pub to_isc: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FigurineIscConflict {
    pub id: i64,
    pub painter: String,
    pub loadout: String,
    pub from_isc: String,
    pub candidates: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FigurineIscFixReport {
    pub fixes: Vec<FigurineIscFix>,
    pub conflicts: Vec<FigurineIscConflict>,
    pub skipped_ok: usize,
    pub applied: usize,
}

/// Planifie (et optionnellement applique) le remap ISC des figurines orphelines.
pub fn fix_figurine_transmute_iscs(db: &Path, apply: bool) -> Result<FigurineIscFixReport> {
    let conn = Connection::open(db).with_context(|| format!("ouverture {}", db.display()))?;
    migrate(&conn)?;
    plan_and_maybe_apply(&conn, apply)
}

fn plan_and_maybe_apply(conn: &Connection, apply: bool) -> Result<FigurineIscFixReport> {
    let forms = load_transmute_forms(conn)?;
    let figurines = load_figurines(conn)?;
    let known_iscs = load_known_iscs(conn)?;

    let mut report = FigurineIscFixReport::default();
    for fig in &figurines {
        let current = fig.isc.trim();
        if current.is_empty() {
            continue;
        }
        if known_iscs.contains(&current.to_ascii_lowercase()) {
            report.skipped_ok += 1;
            continue;
        }

        let matching_forms: Vec<&TransmuteForm> = forms
            .iter()
            .filter(|form| form_suffix_matches(&form.isc, current))
            .collect();
        if matching_forms.is_empty() {
            continue;
        }

        let loadout_hits: Vec<&TransmuteForm> = matching_forms
            .iter()
            .copied()
            .filter(|form| form.loadout.trim().eq_ignore_ascii_case(fig.loadout.trim()))
            .collect();

        let (pool, reason_if_ambiguous) = if !loadout_hits.is_empty() {
            (loadout_hits, "plusieurs unités partagent ce suffixe + loadout")
        } else {
            (
                matching_forms,
                "plusieurs unités partagent ce suffixe (précisez le loadout ou corrigez à la main)",
            )
        };

        let targets: BTreeSet<String> = pool.iter().map(|form| form.isc.clone()).collect();
        if targets.len() == 1 {
            let to_isc = targets.into_iter().next().expect("un ISC");
            report.fixes.push(FigurineIscFix {
                id: fig.id,
                painter: fig.painter.clone(),
                loadout: fig.loadout.clone(),
                from_isc: fig.isc.clone(),
                to_isc,
            });
        } else {
            report.conflicts.push(FigurineIscConflict {
                id: fig.id,
                painter: fig.painter.clone(),
                loadout: fig.loadout.clone(),
                from_isc: fig.isc.clone(),
                candidates: targets.into_iter().collect(),
                reason: reason_if_ambiguous.to_string(),
            });
        }
    }

    if apply && !report.fixes.is_empty() {
        let tx = conn.unchecked_transaction()?;
        for fix in &report.fixes {
            tx.execute(
                "UPDATE tts_figurines SET isc = ?1, updated_at = ?2 WHERE id = ?3",
                params![fix.to_isc, now_unix(), fix.id],
            )?;
            report.applied += 1;
        }
        tx.commit()?;
    }

    Ok(report)
}

#[derive(Debug, Clone)]
struct TransmuteForm {
    isc: String,
    loadout: String,
}

#[derive(Debug, Clone)]
struct FigurineRow {
    id: i64,
    isc: String,
    loadout: String,
    painter: String,
}

fn load_transmute_forms(conn: &Connection) -> Result<Vec<TransmuteForm>> {
    let mut stmt = conn.prepare(
        "
        SELECT isc, loadout
        FROM tts_profile_forms
        WHERE state_index != 900 AND state_index != 1000
        ORDER BY id
        ",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(TransmuteForm {
                isc: row.get(0)?,
                loadout: row.get(1)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("lecture des formes de transmute")?;
    Ok(rows)
}

fn load_figurines(conn: &Connection) -> Result<Vec<FigurineRow>> {
    let mut stmt = conn.prepare(
        "
        SELECT id, isc, loadout, painter
        FROM tts_figurines
        ORDER BY id
        ",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(FigurineRow {
                id: row.get(0)?,
                isc: row.get(1)?,
                loadout: row.get(2)?,
                painter: row.get(3)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("lecture des figurines")?;
    Ok(rows)
}

fn load_known_iscs(conn: &Connection) -> Result<BTreeSet<String>> {
    let mut known = BTreeSet::new();
    let mut stmt = conn.prepare("SELECT isc FROM tts_profiles")?;
    for row in stmt.query_map([], |row| row.get::<_, String>(0))? {
        known.insert(row?.trim().to_ascii_lowercase());
    }
    let mut stmt = conn.prepare("SELECT isc FROM tts_profile_forms")?;
    for row in stmt.query_map([], |row| row.get::<_, String>(0))? {
        known.insert(row?.trim().to_ascii_lowercase());
    }
    Ok(known)
}

/// `Chernobog Armored Detachment BATTLE RAVAGED` se termine par ` BATTLE RAVAGED`.
fn form_suffix_matches(qualified_isc: &str, bare_isc: &str) -> bool {
    let qualified = qualified_isc.trim();
    let bare = bare_isc.trim();
    if qualified.is_empty() || bare.is_empty() {
        return false;
    }
    let q = qualified.to_ascii_lowercase();
    let b = bare.to_ascii_lowercase();
    if q == b {
        return false;
    }
    q.ends_with(&format!(" {b}"))
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or(0)
}

/// Affichage console du rapport (dry-run ou apply).
pub fn print_report(report: &FigurineIscFixReport, apply: bool) {
    println!(
        "Déjà OK (ISC connu)     : {}",
        report.skipped_ok
    );
    println!("À corriger              : {}", report.fixes.len());
    println!("Conflits (manuel)       : {}", report.conflicts.len());
    if apply {
        println!("Appliqués               : {}", report.applied);
    } else {
        println!("Mode                    : dry-run (ajoute --apply pour écrire)");
    }

    if !report.fixes.is_empty() {
        println!();
        println!("Corrections :");
        for fix in &report.fixes {
            println!(
                "  #{id} [{painter}] {from} → {to}",
                id = fix.id,
                painter = if fix.painter.trim().is_empty() {
                    "—"
                } else {
                    fix.painter.as_str()
                },
                from = fix.from_isc,
                to = fix.to_isc
            );
            if !fix.loadout.trim().is_empty() {
                println!("           loadout: {}", fix.loadout);
            }
        }
    }

    if !report.conflicts.is_empty() {
        println!();
        println!("Conflits (non touchés) :");
        for conflict in &report.conflicts {
            println!(
                "  #{id} [{painter}] « {from} » — {reason}",
                id = conflict.id,
                painter = if conflict.painter.trim().is_empty() {
                    "—"
                } else {
                    conflict.painter.as_str()
                },
                from = conflict.from_isc,
                reason = conflict.reason
            );
            if !conflict.loadout.trim().is_empty() {
                println!("           loadout: {}", conflict.loadout);
            }
            for candidate in &conflict.candidates {
                println!("           ? {candidate}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        // Référencé par tts_profiles.army_id (FK) même si on laisse army_id NULL ailleurs.
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS armies (
                id INTEGER PRIMARY KEY,
                parent_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                slug TEXT NOT NULL,
                logo_url TEXT NOT NULL DEFAULT '',
                discontinued INTEGER NOT NULL DEFAULT 0,
                synced_at INTEGER NOT NULL DEFAULT 0
            );
            ",
        )
        .unwrap();
        let insert_profile = |conn: &Connection, isc: &str, loadout: &str, bag: &str| -> i64 {
            conn.execute(
                "INSERT INTO tts_profiles (
                    faction_label, isc, loadout, nom_tts, description_tts,
                    source_bag, source_index
                 ) VALUES ('Test', ?1, ?2, 'X', '', ?3, 1)",
                params![isc, loadout, bag],
            )
            .unwrap();
            conn.last_insert_rowid()
        };
        let insert_form = |conn: &Connection, profile_id: i64, isc: &str, loadout: &str| {
            conn.execute(
                "INSERT INTO tts_profile_forms (
                    profile_id, state_index, isc, loadout, nom_tts, description_tts
                 ) VALUES (?1, 1, ?2, ?3, '', '')",
                params![profile_id, isc, loadout],
            )
            .unwrap();
        };

        let dog_id = insert_profile(&conn, "Dog-Warriors", "Chain Rifle", "army:1:1");
        insert_form(
            &conn,
            dog_id,
            "Dog-Warriors DOG-WARRIOR FORM",
            "Chain Rifle",
        );
        let su_id = insert_profile(
            &conn,
            "Sù-Jiàn Immediate Action Unit",
            "Spitfire",
            "army:2:1",
        );
        insert_form(
            &conn,
            su_id,
            "Sù-Jiàn Immediate Action Unit COMBAT FORM",
            "Spitfire",
        );
        let rb_id = insert_profile(&conn, "RoadBots Highway Patrol", "Red Fury", "army:3:1");
        insert_form(
            &conn,
            rb_id,
            "RoadBots Highway Patrol COMBAT FORM",
            "Red Fury",
        );
        conn
    }

    fn insert_fig(conn: &Connection, isc: &str, loadout: &str) -> i64 {
        let now = 1_i64;
        conn.execute(
            "INSERT INTO tts_figurines (isc, loadout, painter, asset_bundle_url, created_at, updated_at)
             VALUES (?1, ?2, 'Alex', 'https://example.test/a', ?3, ?3)",
            params![isc, loadout, now],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn form_suffix_matching() {
        assert!(form_suffix_matches(
            "Dog-Warriors DOG-WARRIOR FORM",
            "DOG-WARRIOR FORM"
        ));
        assert!(!form_suffix_matches(
            "Dog-Warriors DOG-WARRIOR FORM",
            "Dog-Warriors"
        ));
        assert!(!form_suffix_matches("COMBAT FORM", "COMBAT FORM"));
    }

    #[test]
    fn remaps_unique_suffix_and_disambiguates_by_loadout() {
        let conn = setup();
        let dog = insert_fig(&conn, "DOG-WARRIOR FORM", "Chain Rifle");
        let su = insert_fig(&conn, "COMBAT FORM", "Spitfire");
        let ambiguous = insert_fig(&conn, "COMBAT FORM", "Unknown Gun");
        let ok = insert_fig(&conn, "Dog-Warriors", "Chain Rifle");

        let dry = plan_and_maybe_apply(&conn, false).unwrap();
        assert_eq!(dry.skipped_ok, 1);
        assert_eq!(dry.fixes.len(), 2);
        assert_eq!(dry.conflicts.len(), 1);
        assert!(dry.fixes.iter().any(|fix| {
            fix.id == dog && fix.to_isc == "Dog-Warriors DOG-WARRIOR FORM"
        }));
        assert!(dry.fixes.iter().any(|fix| {
            fix.id == su && fix.to_isc == "Sù-Jiàn Immediate Action Unit COMBAT FORM"
        }));
        assert_eq!(dry.conflicts[0].id, ambiguous);
        let _ = ok;

        let applied = plan_and_maybe_apply(&conn, true).unwrap();
        assert_eq!(applied.applied, 2);
        let dog_isc: String = conn
            .query_row("SELECT isc FROM tts_figurines WHERE id = ?1", params![dog], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(dog_isc, "Dog-Warriors DOG-WARRIOR FORM");
    }

    #[test]
    fn apply_is_idempotent_after_success() {
        let conn = setup();
        insert_fig(&conn, "DOG-WARRIOR FORM", "Chain Rifle");
        let first = plan_and_maybe_apply(&conn, true).unwrap();
        assert_eq!(first.applied, 1);
        let second = plan_and_maybe_apply(&conn, true).unwrap();
        assert_eq!(second.fixes.len(), 0);
        assert_eq!(second.applied, 0);
        assert!(second.skipped_ok >= 1);
    }
}

//! Passe la base sur le schéma joueurs / listes / matchs / unités TTS.
//!
//! Les tables historiques (`tournament_registrations`, `tournament_matches`,
//! `tts_profiles`, …) deviennent des vues. Les écritures existantes passent
//! par des triggers, et l'administration écrit directement dans `unit_*`.

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};

const MIGRATION_ID: &str = "rework_v1";

/// Identifiant écrit par le trigger d'une vue. `last_insert_rowid()` reprend
/// sa valeur d'avant le trigger, donc on lit la ligne laissée par celui-ci.
pub(crate) fn echoed_id(conn: &Connection) -> Result<i64> {
    conn.query_row("SELECT id FROM schema_rowid_echo LIMIT 1", [], |row| {
        row.get(0)
    })
    .context("identifiant d'insertion")
}

pub fn rework(conn: &Connection) -> Result<()> {
    if migration_done(conn)? {
        upgrade_rework(conn)?;
        return Ok(());
    }

    conn.pragma_update(None, "foreign_keys", "OFF")?;
    let tx = conn
        .unchecked_transaction()
        .context("transaction de reprise du schéma")?;

    tx.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS schema_migrations (
            id TEXT PRIMARY KEY
        );
        CREATE TABLE IF NOT EXISTS schema_id_hold (
            k TEXT PRIMARY KEY,
            v INTEGER
        ) WITHOUT ROWID;
        CREATE TABLE IF NOT EXISTS schema_rowid_echo (
            id INTEGER PRIMARY KEY
        );
        ",
    )?;

    rework_players(&tx)?;
    rework_tournament_people(&tx)?;
    rework_tournament_matches(&tx)?;
    rework_tts(&tx)?;

    tx.execute(
        "INSERT INTO schema_migrations (id) VALUES (?1)",
        params![MIGRATION_ID],
    )?;
    tx.commit().context("validation de la reprise du schéma")?;
    Ok(())
}

fn upgrade_rework(conn: &Connection) -> Result<()> {
    if column_exists(conn, "tts_figurine_rows", "id")?
        && !column_exists(conn, "tts_figurine_rows", "label_nom")?
    {
        conn.execute_batch(
            "
            ALTER TABLE tts_figurine_rows ADD COLUMN label_nom TEXT NOT NULL DEFAULT '';
            ALTER TABLE tts_figurine_rows ADD COLUMN label_loadout TEXT NOT NULL DEFAULT '';
            ",
        )?;
    }
    if column_exists(conn, "tts_figurine_rows", "label_nom")? {
        conn.execute_batch(
            "
            DROP VIEW IF EXISTS tts_figurines;
            CREATE VIEW tts_figurines AS
            SELECT
                f.id,
                COALESCE(
                    (SELECT u.nom FROM unit_profiles p JOIN units u ON u.id = p.unit_id
                     WHERE p.loadout_id = f.loadout_id
                     ORDER BY p.parent_profile_id IS NOT NULL, p.id LIMIT 1),
                    NULLIF(f.label_nom, ''),
                    l.nom
                ) AS isc,
                COALESCE(
                    (SELECT p.loadout FROM unit_profiles p
                     WHERE p.loadout_id = f.loadout_id
                     ORDER BY p.parent_profile_id IS NOT NULL, p.id LIMIT 1),
                    f.label_loadout
                ) AS loadout,
                f.painter,
                f.asset_bundle_url,
                f.photo_filename,
                f.created_at,
                f.updated_at,
                f.label_nom AS legacy_nom,
                f.label_loadout AS legacy_loadout,
                CASE
                    WHEN EXISTS (
                        SELECT 1 FROM unit_profiles p WHERE p.loadout_id = f.loadout_id
                    ) THEN f.loadout_id
                    ELSE NULL
                END AS tts_loadout_id
            FROM tts_figurine_rows f
            JOIN unit_loadouts l ON l.id = f.loadout_id;
            ",
        )?;
    }
    conn.execute_batch(
        "
        DROP TRIGGER IF EXISTS tournament_matches_insert;
        DROP TRIGGER IF EXISTS tournament_matches_update;
        DROP TRIGGER IF EXISTS tournament_matches_delete;
        ",
    )?;
    conn.execute_batch(MATCH_TRIGGERS)?;
    if column_exists(conn, "units", "id")? {
        conn.execute_batch(
            "
            DROP TRIGGER IF EXISTS tts_units_insert;
            DROP TRIGGER IF EXISTS tts_units_update;
            DROP TRIGGER IF EXISTS tts_loadouts_insert;
            DROP TRIGGER IF EXISTS tts_loadouts_update;
            DROP TRIGGER IF EXISTS tts_loadouts_delete;
            DROP TRIGGER IF EXISTS tts_profiles_insert;
            DROP TRIGGER IF EXISTS tts_profiles_update;
            DROP TRIGGER IF EXISTS tts_profiles_delete;
            DROP TRIGGER IF EXISTS tts_profile_traits_insert;
            DROP TRIGGER IF EXISTS tts_profile_traits_delete;
            DROP TRIGGER IF EXISTS tts_profile_sectorials_insert;
            DROP TRIGGER IF EXISTS tts_profile_sectorials_delete;
            DROP TRIGGER IF EXISTS tts_figurines_insert;
            DROP TRIGGER IF EXISTS tts_figurines_update;
            DROP TRIGGER IF EXISTS tts_figurines_delete;
            ",
        )?;
        conn.execute_batch(TTS_TRIGGERS)?;
        crate::unit_admin::clean_figurine_loadout_names(conn)?;
    }
    Ok(())
}

fn migration_done(conn: &Connection) -> Result<bool> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if exists.is_none() {
        return Ok(false);
    }
    let done: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM schema_migrations WHERE id = ?1",
            params![MIGRATION_ID],
            |row| row.get(0),
        )
        .optional()?;
    Ok(done.is_some())
}

fn bump_sequence(conn: &Connection, table: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO sqlite_sequence (name, seq) SELECT ?1, 0 WHERE NOT EXISTS (SELECT 1 FROM sqlite_sequence WHERE name = ?1)",
        params![table],
    )?;
    conn.execute(
        &format!(
            "UPDATE sqlite_sequence SET seq = COALESCE((SELECT MAX(id) FROM {table}), 0) WHERE name = ?1"
        ),
        params![table],
    )?;
    Ok(())
}

fn rework_players(conn: &Connection) -> Result<()> {
    if column_exists(conn, "players", "id")? {
        return Ok(());
    }
    conn.execute_batch(
        "
        CREATE TABLE players_v2 (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name_key TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            rating REAL NOT NULL,
            wins INTEGER NOT NULL DEFAULT 0,
            draws INTEGER NOT NULL DEFAULT 0,
            losses INTEGER NOT NULL DEFAULT 0,
            discord_username TEXT
        );
        INSERT INTO players_v2 (name_key, name, rating, wins, draws, losses, discord_username)
        SELECT name_key, name, rating, wins, draws, losses, discord_username FROM players;
        DROP TABLE players;
        ALTER TABLE players_v2 RENAME TO players;
        UPDATE sqlite_sequence SET name = 'players' WHERE name = 'players_v2';
        CREATE UNIQUE INDEX IF NOT EXISTS idx_players_discord_username
            ON players(discord_username)
            WHERE discord_username IS NOT NULL;
        ",
    )?;
    bump_sequence(conn, "players")?;
    Ok(())
}

fn rework_tournament_people(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE tournament_player_army_lists (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            registration_id INTEGER NOT NULL,
            army_list_id INTEGER NOT NULL REFERENCES army_lists(id),
            kind TEXT NOT NULL CHECK (kind IN ('pools', 'final')),
            slot INTEGER NOT NULL CHECK (slot IN (1, 2)),
            validated INTEGER NOT NULL DEFAULT 0,
            UNIQUE (registration_id, kind, slot)
        );

        INSERT INTO players (name_key, name, rating, wins, draws, losses)
        SELECT DISTINCT player_name_key, player_name, 1200, 0, 0, 0
        FROM tournament_registrations r
        WHERE NOT EXISTS (SELECT 1 FROM players p WHERE p.name_key = r.player_name_key);

        INSERT INTO players (name_key, name, rating, wins, draws, losses)
        SELECT DISTINCT player_name_key, player_name, 1200, 0, 0, 0
        FROM tournament_players r
        WHERE NOT EXISTS (SELECT 1 FROM players p WHERE p.name_key = r.player_name_key);

        INSERT INTO players (name_key, name, rating, wins, draws, losses)
        SELECT DISTINCT player_name_key, player_name, 1200, 0, 0, 0
        FROM pool_players r
        WHERE NOT EXISTS (SELECT 1 FROM players p WHERE p.name_key = r.player_name_key);

        INSERT OR IGNORE INTO army_lists (code, army_id)
        SELECT DISTINCT code, army_id FROM (
            SELECT army_list_1 AS code, army_id FROM tournament_registrations
            WHERE army_list_1_id IS NULL AND army_list_1 IS NOT NULL AND trim(army_list_1) != '' AND army_id IS NOT NULL
            UNION ALL
            SELECT army_list_2, army_id FROM tournament_registrations
            WHERE army_list_2_id IS NULL AND army_list_2 IS NOT NULL AND trim(army_list_2) != '' AND army_id IS NOT NULL
            UNION ALL
            SELECT bracket_list_1, army_id FROM tournament_registrations
            WHERE bracket_list_1_id IS NULL AND bracket_list_1 IS NOT NULL AND trim(bracket_list_1) != '' AND army_id IS NOT NULL
            UNION ALL
            SELECT bracket_list_2, army_id FROM tournament_registrations
            WHERE bracket_list_2_id IS NULL AND bracket_list_2 IS NOT NULL AND trim(bracket_list_2) != '' AND army_id IS NOT NULL
        ) AS missing
        WHERE NOT EXISTS (SELECT 1 FROM army_lists al WHERE al.code = missing.code);

        CREATE TABLE tournament_registration_rows (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tournament_id INTEGER NOT NULL REFERENCES tournaments(id),
            player_id INTEGER NOT NULL REFERENCES players(id),
            user_id INTEGER,
            status TEXT NOT NULL DEFAULT 'pending',
            waitlist_position INTEGER,
            requested_at INTEGER NOT NULL,
            reviewed_at INTEGER,
            reviewed_by INTEGER,
            army_id INTEGER,
            UNIQUE (tournament_id, player_id)
        );

        INSERT INTO tournament_registration_rows (
            id, tournament_id, player_id, user_id, status, waitlist_position,
            requested_at, reviewed_at, reviewed_by, army_id
        )
        SELECT r.id, r.tournament_id, p.id, r.user_id, r.status, r.waitlist_position,
               r.requested_at, r.reviewed_at, r.reviewed_by, r.army_id
        FROM tournament_registrations r
        JOIN players p ON p.name_key = r.player_name_key;

        INSERT INTO tournament_player_army_lists (registration_id, army_list_id, kind, slot, validated)
        SELECT r.id, COALESCE(r.army_list_1_id, (SELECT id FROM army_lists WHERE code = r.army_list_1 LIMIT 1)),
               'pools', 1, COALESCE(r.army_list_1_validated, 0)
        FROM tournament_registrations r
        WHERE COALESCE(r.army_list_1_id, (SELECT id FROM army_lists WHERE code = r.army_list_1 LIMIT 1)) IS NOT NULL;

        INSERT INTO tournament_player_army_lists (registration_id, army_list_id, kind, slot, validated)
        SELECT r.id, COALESCE(r.army_list_2_id, (SELECT id FROM army_lists WHERE code = r.army_list_2 LIMIT 1)),
               'pools', 2, COALESCE(r.army_list_2_validated, 0)
        FROM tournament_registrations r
        WHERE COALESCE(r.army_list_2_id, (SELECT id FROM army_lists WHERE code = r.army_list_2 LIMIT 1)) IS NOT NULL
          AND NOT (r.army_list_2_id IS NULL AND trim(COALESCE(r.army_list_2, '')) = '');

        INSERT INTO tournament_player_army_lists (registration_id, army_list_id, kind, slot, validated)
        SELECT r.id, COALESCE(r.bracket_list_1_id, (SELECT id FROM army_lists WHERE code = r.bracket_list_1 LIMIT 1)),
               'final', 1, 0
        FROM tournament_registrations r
        WHERE COALESCE(r.bracket_list_1_id, (SELECT id FROM army_lists WHERE code = r.bracket_list_1 LIMIT 1)) IS NOT NULL;

        INSERT INTO tournament_player_army_lists (registration_id, army_list_id, kind, slot, validated)
        SELECT r.id, COALESCE(r.bracket_list_2_id, (SELECT id FROM army_lists WHERE code = r.bracket_list_2 LIMIT 1)),
               'final', 2, 0
        FROM tournament_registrations r
        WHERE COALESCE(r.bracket_list_2_id, (SELECT id FROM army_lists WHERE code = r.bracket_list_2 LIMIT 1)) IS NOT NULL
          AND NOT (r.bracket_list_2_id IS NULL AND trim(COALESCE(r.bracket_list_2, '')) = '');

        DROP TABLE tournament_registrations;

        CREATE TABLE tournament_player_rows (
            tournament_id INTEGER NOT NULL REFERENCES tournaments(id),
            player_id INTEGER NOT NULL REFERENCES players(id),
            start_rating REAL NOT NULL,
            pool_elo_delta REAL NOT NULL DEFAULT 0,
            bracket_rating REAL NOT NULL DEFAULT 0,
            pool_points INTEGER NOT NULL DEFAULT 0,
            pool_objectives INTEGER NOT NULL DEFAULT 0,
            pool_survivors INTEGER NOT NULL DEFAULT 0,
            final_placement INTEGER,
            PRIMARY KEY (tournament_id, player_id)
        );
        INSERT INTO tournament_player_rows (
            tournament_id, player_id, start_rating, pool_elo_delta, bracket_rating,
            pool_points, pool_objectives, pool_survivors, final_placement
        )
        SELECT t.tournament_id, p.id, t.start_rating, t.pool_elo_delta, t.bracket_rating,
               t.pool_points, t.pool_objectives, t.pool_survivors, t.final_placement
        FROM tournament_players t
        JOIN players p ON p.name_key = t.player_name_key;
        DROP TABLE tournament_players;

        CREATE TABLE pool_player_rows (
            pool_id INTEGER NOT NULL REFERENCES pools(id),
            player_id INTEGER NOT NULL REFERENCES players(id),
            seed INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (pool_id, player_id)
        );
        INSERT INTO pool_player_rows (pool_id, player_id, seed)
        SELECT pp.pool_id, p.id, pp.seed
        FROM pool_players pp
        JOIN players p ON p.name_key = pp.player_name_key;
        DROP TABLE pool_players;
        ",
    )?;
    bump_sequence(conn, "tournament_registration_rows")?;
    bump_sequence(conn, "tournament_player_army_lists")?;
    create_people_views(conn)?;
    Ok(())
}

fn create_people_views(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE VIEW tournament_registrations AS
        SELECT
            r.id,
            r.tournament_id,
            p.name_key AS player_name_key,
            p.name AS player_name,
            r.user_id,
            r.status,
            r.waitlist_position,
            r.requested_at,
            r.reviewed_at,
            r.reviewed_by,
            r.army_id,
            (SELECT al.code FROM tournament_player_army_lists l
                JOIN army_lists al ON al.id = l.army_list_id
                WHERE l.registration_id = r.id AND l.kind = 'pools' AND l.slot = 1) AS army_list_1,
            (SELECT al.code FROM tournament_player_army_lists l
                JOIN army_lists al ON al.id = l.army_list_id
                WHERE l.registration_id = r.id AND l.kind = 'pools' AND l.slot = 2) AS army_list_2,
            (SELECT al.code FROM tournament_player_army_lists l
                JOIN army_lists al ON al.id = l.army_list_id
                WHERE l.registration_id = r.id AND l.kind = 'final' AND l.slot = 1) AS bracket_list_1,
            (SELECT al.code FROM tournament_player_army_lists l
                JOIN army_lists al ON al.id = l.army_list_id
                WHERE l.registration_id = r.id AND l.kind = 'final' AND l.slot = 2) AS bracket_list_2,
            (SELECT l.army_list_id FROM tournament_player_army_lists l
                WHERE l.registration_id = r.id AND l.kind = 'pools' AND l.slot = 1) AS army_list_1_id,
            (SELECT l.army_list_id FROM tournament_player_army_lists l
                WHERE l.registration_id = r.id AND l.kind = 'pools' AND l.slot = 2) AS army_list_2_id,
            (SELECT l.army_list_id FROM tournament_player_army_lists l
                WHERE l.registration_id = r.id AND l.kind = 'final' AND l.slot = 1) AS bracket_list_1_id,
            (SELECT l.army_list_id FROM tournament_player_army_lists l
                WHERE l.registration_id = r.id AND l.kind = 'final' AND l.slot = 2) AS bracket_list_2_id,
            COALESCE((SELECT l.validated FROM tournament_player_army_lists l
                WHERE l.registration_id = r.id AND l.kind = 'pools' AND l.slot = 1), 0) AS army_list_1_validated,
            COALESCE((SELECT l.validated FROM tournament_player_army_lists l
                WHERE l.registration_id = r.id AND l.kind = 'pools' AND l.slot = 2), 0) AS army_list_2_validated
        FROM tournament_registration_rows r
        JOIN players p ON p.id = r.player_id;

        CREATE VIEW tournament_players AS
        SELECT
            t.tournament_id,
            p.name_key AS player_name_key,
            p.name AS player_name,
            t.start_rating,
            t.pool_elo_delta,
            t.bracket_rating,
            t.pool_points,
            t.pool_objectives,
            t.pool_survivors,
            t.final_placement
        FROM tournament_player_rows t
        JOIN players p ON p.id = t.player_id;

        CREATE VIEW pool_players AS
        SELECT
            pp.pool_id,
            p.name_key AS player_name_key,
            p.name AS player_name,
            pp.seed
        FROM pool_player_rows pp
        JOIN players p ON p.id = pp.player_id;
        ",
    )?;
    conn.execute_batch(REGISTRATION_TRIGGERS)?;
    conn.execute_batch(PLAYER_TRIGGERS)?;
    Ok(())
}

const REGISTRATION_TRIGGERS: &str = r#"
CREATE TRIGGER tournament_registrations_insert
INSTEAD OF INSERT ON tournament_registrations
BEGIN
    INSERT INTO players (name_key, name, rating, wins, draws, losses)
    SELECT NEW.player_name_key,
           COALESCE(NULLIF(trim(NEW.player_name), ''), NEW.player_name_key),
           1200, 0, 0, 0
    WHERE NEW.player_name_key IS NOT NULL
      AND NOT EXISTS (SELECT 1 FROM players WHERE name_key = NEW.player_name_key);

    INSERT INTO tournament_registration_rows (
        tournament_id, player_id, user_id, status, waitlist_position,
        requested_at, reviewed_at, reviewed_by, army_id
    ) VALUES (
        NEW.tournament_id,
        (SELECT id FROM players WHERE name_key = NEW.player_name_key),
        NEW.user_id,
        COALESCE(NEW.status, 'pending'),
        NEW.waitlist_position,
        COALESCE(NEW.requested_at, CAST(strftime('%s','now') AS INTEGER)),
        NEW.reviewed_at,
        NEW.reviewed_by,
        NEW.army_id
    );

    INSERT OR REPLACE INTO schema_id_hold (k, v) VALUES ('reg', last_insert_rowid());

    INSERT INTO tournament_player_army_lists (registration_id, army_list_id, kind, slot, validated)
    SELECT v, NEW.army_list_1_id, 'pools', 1, COALESCE(NEW.army_list_1_validated, 0)
    FROM schema_id_hold WHERE k = 'reg' AND NEW.army_list_1_id IS NOT NULL;

    INSERT INTO tournament_player_army_lists (registration_id, army_list_id, kind, slot, validated)
    SELECT v, NEW.army_list_2_id, 'pools', 2, COALESCE(NEW.army_list_2_validated, 0)
    FROM schema_id_hold WHERE k = 'reg' AND NEW.army_list_2_id IS NOT NULL;

    INSERT INTO tournament_player_army_lists (registration_id, army_list_id, kind, slot, validated)
    SELECT v, NEW.bracket_list_1_id, 'final', 1, 0
    FROM schema_id_hold WHERE k = 'reg' AND NEW.bracket_list_1_id IS NOT NULL;

    INSERT INTO tournament_player_army_lists (registration_id, army_list_id, kind, slot, validated)
    SELECT v, NEW.bracket_list_2_id, 'final', 2, 0
    FROM schema_id_hold WHERE k = 'reg' AND NEW.bracket_list_2_id IS NOT NULL;

    DELETE FROM schema_rowid_echo;
    INSERT INTO schema_rowid_echo (id) SELECT v FROM schema_id_hold WHERE k = 'reg';
END;

CREATE TRIGGER tournament_registrations_update
INSTEAD OF UPDATE ON tournament_registrations
BEGIN
    INSERT INTO players (name_key, name, rating, wins, draws, losses)
    SELECT NEW.player_name_key,
           COALESCE(NULLIF(trim(NEW.player_name), ''), NEW.player_name_key),
           1200, 0, 0, 0
    WHERE NEW.player_name_key IS NOT NULL
      AND NOT EXISTS (SELECT 1 FROM players WHERE name_key = NEW.player_name_key);

    UPDATE tournament_registration_rows SET
        tournament_id = NEW.tournament_id,
        player_id = (SELECT id FROM players WHERE name_key = NEW.player_name_key),
        user_id = NEW.user_id,
        status = COALESCE(NEW.status, status),
        waitlist_position = NEW.waitlist_position,
        requested_at = COALESCE(NEW.requested_at, requested_at),
        reviewed_at = NEW.reviewed_at,
        reviewed_by = NEW.reviewed_by,
        army_id = NEW.army_id
    WHERE id = OLD.id;

    DELETE FROM tournament_player_army_lists WHERE registration_id = OLD.id;

    INSERT INTO tournament_player_army_lists (registration_id, army_list_id, kind, slot, validated)
    SELECT OLD.id, NEW.army_list_1_id, 'pools', 1, COALESCE(NEW.army_list_1_validated, 0)
    WHERE NEW.army_list_1_id IS NOT NULL;

    INSERT INTO tournament_player_army_lists (registration_id, army_list_id, kind, slot, validated)
    SELECT OLD.id, NEW.army_list_2_id, 'pools', 2, COALESCE(NEW.army_list_2_validated, 0)
    WHERE NEW.army_list_2_id IS NOT NULL;

    INSERT INTO tournament_player_army_lists (registration_id, army_list_id, kind, slot, validated)
    SELECT OLD.id, NEW.bracket_list_1_id, 'final', 1, 0
    WHERE NEW.bracket_list_1_id IS NOT NULL;

    INSERT INTO tournament_player_army_lists (registration_id, army_list_id, kind, slot, validated)
    SELECT OLD.id, NEW.bracket_list_2_id, 'final', 2, 0
    WHERE NEW.bracket_list_2_id IS NOT NULL;
END;

CREATE TRIGGER tournament_registrations_delete
INSTEAD OF DELETE ON tournament_registrations
BEGIN
    DELETE FROM tournament_player_army_lists WHERE registration_id = OLD.id;
    DELETE FROM tournament_registration_rows WHERE id = OLD.id;
END;
"#;

const PLAYER_TRIGGERS: &str = r#"
CREATE TRIGGER tournament_players_insert
INSTEAD OF INSERT ON tournament_players
BEGIN
    INSERT INTO players (name_key, name, rating, wins, draws, losses)
    SELECT NEW.player_name_key,
           COALESCE(NULLIF(trim(NEW.player_name), ''), NEW.player_name_key),
           1200, 0, 0, 0
    WHERE NEW.player_name_key IS NOT NULL
      AND NOT EXISTS (SELECT 1 FROM players WHERE name_key = NEW.player_name_key);

    INSERT INTO tournament_player_rows (
        tournament_id, player_id, start_rating, pool_elo_delta, bracket_rating,
        pool_points, pool_objectives, pool_survivors, final_placement
    ) VALUES (
        NEW.tournament_id,
        (SELECT id FROM players WHERE name_key = NEW.player_name_key),
        COALESCE(NEW.start_rating, 1200),
        COALESCE(NEW.pool_elo_delta, 0),
        COALESCE(NEW.bracket_rating, NEW.start_rating, 1200),
        COALESCE(NEW.pool_points, 0),
        COALESCE(NEW.pool_objectives, 0),
        COALESCE(NEW.pool_survivors, 0),
        NEW.final_placement
    )
    ON CONFLICT (tournament_id, player_id) DO UPDATE SET
        start_rating = COALESCE(NEW.start_rating, tournament_player_rows.start_rating),
        pool_elo_delta = COALESCE(NEW.pool_elo_delta, tournament_player_rows.pool_elo_delta),
        bracket_rating = COALESCE(NEW.bracket_rating, tournament_player_rows.bracket_rating),
        pool_points = COALESCE(NEW.pool_points, tournament_player_rows.pool_points),
        pool_objectives = COALESCE(NEW.pool_objectives, tournament_player_rows.pool_objectives),
        pool_survivors = COALESCE(NEW.pool_survivors, tournament_player_rows.pool_survivors),
        final_placement = COALESCE(NEW.final_placement, tournament_player_rows.final_placement);
END;

CREATE TRIGGER tournament_players_update
INSTEAD OF UPDATE ON tournament_players
BEGIN
    UPDATE tournament_player_rows SET
        start_rating = COALESCE(NEW.start_rating, start_rating),
        pool_elo_delta = COALESCE(NEW.pool_elo_delta, pool_elo_delta),
        bracket_rating = COALESCE(NEW.bracket_rating, bracket_rating),
        pool_points = COALESCE(NEW.pool_points, pool_points),
        pool_objectives = COALESCE(NEW.pool_objectives, pool_objectives),
        pool_survivors = COALESCE(NEW.pool_survivors, pool_survivors),
        final_placement = NEW.final_placement
    WHERE tournament_id = OLD.tournament_id
      AND player_id = (SELECT id FROM players WHERE name_key = OLD.player_name_key);
END;

CREATE TRIGGER tournament_players_delete
INSTEAD OF DELETE ON tournament_players
BEGIN
    DELETE FROM tournament_player_rows
    WHERE tournament_id = OLD.tournament_id
      AND player_id = (SELECT id FROM players WHERE name_key = OLD.player_name_key);
END;

CREATE TRIGGER pool_players_insert
INSTEAD OF INSERT ON pool_players
BEGIN
    INSERT INTO players (name_key, name, rating, wins, draws, losses)
    SELECT NEW.player_name_key,
           COALESCE(NULLIF(trim(NEW.player_name), ''), NEW.player_name_key),
           1200, 0, 0, 0
    WHERE NEW.player_name_key IS NOT NULL
      AND NOT EXISTS (SELECT 1 FROM players WHERE name_key = NEW.player_name_key);

    INSERT INTO pool_player_rows (pool_id, player_id, seed)
    VALUES (
        NEW.pool_id,
        (SELECT id FROM players WHERE name_key = NEW.player_name_key),
        COALESCE(NEW.seed, 0)
    );
END;

CREATE TRIGGER pool_players_update
INSTEAD OF UPDATE ON pool_players
BEGIN
    UPDATE pool_player_rows SET seed = COALESCE(NEW.seed, seed)
    WHERE pool_id = OLD.pool_id
      AND player_id = (SELECT id FROM players WHERE name_key = OLD.player_name_key);
END;

CREATE TRIGGER pool_players_delete
INSTEAD OF DELETE ON pool_players
BEGIN
    DELETE FROM pool_player_rows
    WHERE pool_id = OLD.pool_id
      AND player_id = (SELECT id FROM players WHERE name_key = OLD.player_name_key);
END;
"#;

fn rework_tournament_matches(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        UPDATE matches SET
            player1 = COALESCE((SELECT tm.player1 FROM tournament_matches tm WHERE tm.elo_match_id = matches.id AND tm.player1 IS NOT NULL LIMIT 1), player1),
            player2 = COALESCE((SELECT tm.player2 FROM tournament_matches tm WHERE tm.elo_match_id = matches.id AND tm.player2 IS NOT NULL LIMIT 1), player2),
            player1_objectives = COALESCE((SELECT tm.player1_objectives FROM tournament_matches tm WHERE tm.elo_match_id = matches.id LIMIT 1), player1_objectives),
            player2_objectives = COALESCE((SELECT tm.player2_objectives FROM tournament_matches tm WHERE tm.elo_match_id = matches.id LIMIT 1), player2_objectives),
            player1_survivors = COALESCE((SELECT tm.player1_survivors FROM tournament_matches tm WHERE tm.elo_match_id = matches.id LIMIT 1), player1_survivors),
            player2_survivors = COALESCE((SELECT tm.player2_survivors FROM tournament_matches tm WHERE tm.elo_match_id = matches.id LIMIT 1), player2_survivors),
            player1_army_id = (SELECT tm.player1_army_id FROM tournament_matches tm WHERE tm.elo_match_id = matches.id LIMIT 1),
            player2_army_id = (SELECT tm.player2_army_id FROM tournament_matches tm WHERE tm.elo_match_id = matches.id LIMIT 1),
            scenario_id = COALESCE((SELECT tm.scenario_id FROM tournament_matches tm WHERE tm.elo_match_id = matches.id LIMIT 1), scenario_id),
            scenario_other = COALESCE((SELECT tm.scenario_other FROM tournament_matches tm WHERE tm.elo_match_id = matches.id LIMIT 1), scenario_other),
            player1_army_list_code = COALESCE((SELECT tm.player1_army_list_code FROM tournament_matches tm WHERE tm.elo_match_id = matches.id LIMIT 1), player1_army_list_code),
            player2_army_list_code = COALESCE((SELECT tm.player2_army_list_code FROM tournament_matches tm WHERE tm.elo_match_id = matches.id LIMIT 1), player2_army_list_code),
            player1_army_list_id = COALESCE((SELECT tm.player1_army_list_id FROM tournament_matches tm WHERE tm.elo_match_id = matches.id LIMIT 1), player1_army_list_id),
            player2_army_list_id = COALESCE((SELECT tm.player2_army_list_id FROM tournament_matches tm WHERE tm.elo_match_id = matches.id LIMIT 1), player2_army_list_id),
            tournament_id = COALESCE(tournament_id, (SELECT tm.tournament_id FROM tournament_matches tm WHERE tm.elo_match_id = matches.id LIMIT 1)),
            tournament_phase = COALESCE(tournament_phase, (SELECT tm.phase FROM tournament_matches tm WHERE tm.elo_match_id = matches.id LIMIT 1))
        WHERE id IN (SELECT elo_match_id FROM tournament_matches WHERE elo_match_id IS NOT NULL);

        INSERT INTO matches (
            player1, player2, outcome,
            player1_old, player1_new, player2_old, player2_new,
            player1_objectives, player1_survivors, player2_objectives, player2_survivors,
            player1_army_id, player2_army_id,
            scenario_id, scenario_other,
            tournament_id, tournament_phase,
            status, counts_for_elo, recorded_at,
            player1_army_list_code, player2_army_list_code,
            player1_army_list_id, player2_army_list_id,
            client_uuid
        )
        SELECT
            tm.player1, tm.player2,
            CASE WHEN tm.status = 'scheduled' THEN '' ELSE COALESCE(tm.outcome, '') END,
            0, 0, 0, 0,
            tm.player1_objectives, tm.player1_survivors, tm.player2_objectives, tm.player2_survivors,
            tm.player1_army_id, tm.player2_army_id,
            tm.scenario_id, tm.scenario_other,
            tm.tournament_id, tm.phase,
            CASE
                WHEN tm.status = 'scheduled' THEN 'to_be_played'
                WHEN tm.status = 'confirmed' THEN 'completed'
                ELSE 'in_progress'
            END,
            0,
            COALESCE(tm.played_at, tm.submitted_at, tm.confirmed_at, 0),
            tm.player1_army_list_code, tm.player2_army_list_code,
            tm.player1_army_list_id, tm.player2_army_list_id,
            'tournament-match:' || tm.id
        FROM tournament_matches tm
        WHERE tm.elo_match_id IS NULL
          AND tm.player1 IS NOT NULL AND trim(tm.player1) != ''
          AND tm.player2 IS NOT NULL AND trim(tm.player2) != '';

        CREATE TABLE tournament_match_rows (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tournament_id INTEGER NOT NULL REFERENCES tournaments(id),
            phase TEXT NOT NULL,
            pool_id INTEGER REFERENCES pools(id),
            bracket_slot INTEGER,
            match_id INTEGER REFERENCES matches(id),
            player1_tournament_points INTEGER NOT NULL DEFAULT 0,
            player2_tournament_points INTEGER NOT NULL DEFAULT 0,
            is_forfeit INTEGER NOT NULL DEFAULT 0,
            forfeit_player TEXT,
            is_unplayed INTEGER NOT NULL DEFAULT 0,
            player1_elo_delta REAL NOT NULL DEFAULT 0,
            player2_elo_delta REAL NOT NULL DEFAULT 0,
            player1_rating_used REAL,
            player2_rating_used REAL,
            elo_applied_at INTEGER,
            status TEXT NOT NULL DEFAULT 'scheduled',
            submitted_by_user_id INTEGER,
            submitted_at INTEGER,
            confirmed_by_user_id INTEGER,
            confirmed_at INTEGER,
            played_at INTEGER
        );

        INSERT INTO tournament_match_rows (
            id, tournament_id, phase, pool_id, bracket_slot, match_id,
            player1_tournament_points, player2_tournament_points,
            is_forfeit, forfeit_player, is_unplayed,
            player1_elo_delta, player2_elo_delta,
            player1_rating_used, player2_rating_used, elo_applied_at,
            status, submitted_by_user_id, submitted_at,
            confirmed_by_user_id, confirmed_at, played_at
        )
        SELECT
            tm.id, tm.tournament_id, tm.phase, tm.pool_id, tm.bracket_slot,
            COALESCE(tm.elo_match_id, m.id),
            tm.player1_tournament_points, tm.player2_tournament_points,
            tm.is_forfeit, tm.forfeit_player, tm.is_unplayed,
            tm.player1_elo_delta, tm.player2_elo_delta,
            tm.player1_rating_used, tm.player2_rating_used, tm.elo_applied_at,
            tm.status, tm.submitted_by_user_id, tm.submitted_at,
            tm.confirmed_by_user_id, tm.confirmed_at, tm.played_at
        FROM tournament_matches tm
        LEFT JOIN matches m ON m.client_uuid = 'tournament-match:' || tm.id;

        UPDATE matches SET client_uuid = NULL WHERE client_uuid LIKE 'tournament-match:%';

        DROP TABLE tournament_matches;

        CREATE VIEW tournament_matches AS
        SELECT
            t.id,
            t.tournament_id,
            t.phase,
            t.pool_id,
            t.bracket_slot,
            m.player1,
            m.player2,
            COALESCE(m.player1_objectives, 0) AS player1_objectives,
            COALESCE(m.player2_objectives, 0) AS player2_objectives,
            COALESCE(m.player1_survivors, 0) AS player1_survivors,
            COALESCE(m.player2_survivors, 0) AS player2_survivors,
            t.player1_tournament_points,
            t.player2_tournament_points,
            m.outcome,
            t.is_forfeit,
            t.forfeit_player,
            t.player1_elo_delta,
            t.player2_elo_delta,
            t.player1_rating_used,
            t.player2_rating_used,
            t.elo_applied_at,
            t.status,
            t.submitted_by_user_id,
            t.submitted_at,
            t.confirmed_by_user_id,
            t.confirmed_at,
            m.scenario_id,
            m.scenario_other,
            m.player1_army_id,
            m.player2_army_id,
            t.played_at,
            t.is_unplayed,
            m.player1_army_list_code,
            m.player2_army_list_code,
            m.player1_army_list_id,
            m.player2_army_list_id,
            CASE
                WHEN m.id IS NULL OR m.status = 'to_be_played' THEN NULL
                ELSE m.id
            END AS elo_match_id
        FROM tournament_match_rows t
        LEFT JOIN matches m ON m.id = t.match_id;

        CREATE INDEX IF NOT EXISTS idx_tournament_match_rows_tournament
            ON tournament_match_rows(tournament_id);
        CREATE INDEX IF NOT EXISTS idx_tournament_match_rows_match
            ON tournament_match_rows(match_id);
        ",
    )?;
    bump_sequence(conn, "tournament_match_rows")?;
    bump_sequence(conn, "matches")?;
    conn.execute_batch(MATCH_TRIGGERS)?;
    Ok(())
}

const MATCH_TRIGGERS: &str = r#"
CREATE TRIGGER tournament_matches_insert
INSTEAD OF INSERT ON tournament_matches
BEGIN
    INSERT INTO matches (
        player1, player2, outcome,
        player1_old, player1_new, player2_old, player2_new,
        player1_objectives, player1_survivors, player2_objectives, player2_survivors,
        player1_army_id, player2_army_id,
        scenario_id, scenario_other,
        tournament_id, tournament_phase,
        status, counts_for_elo, recorded_at,
        player1_army_list_code, player2_army_list_code,
        player1_army_list_id, player2_army_list_id
    )
    SELECT
        NEW.player1, NEW.player2,
        CASE WHEN COALESCE(NEW.status, 'scheduled') = 'scheduled' THEN '' ELSE COALESCE(NEW.outcome, '') END,
        0, 0, 0, 0,
        COALESCE(NEW.player1_objectives, 0), COALESCE(NEW.player1_survivors, 0),
        COALESCE(NEW.player2_objectives, 0), COALESCE(NEW.player2_survivors, 0),
        NEW.player1_army_id, NEW.player2_army_id,
        NEW.scenario_id, NEW.scenario_other,
        NEW.tournament_id, NEW.phase,
        CASE
            WHEN COALESCE(NEW.status, 'scheduled') = 'scheduled' THEN 'to_be_played'
            WHEN NEW.status = 'confirmed' THEN 'completed'
            ELSE 'in_progress'
        END,
        0,
        COALESCE(NEW.played_at, NEW.submitted_at, 0),
        NEW.player1_army_list_code, NEW.player2_army_list_code,
        NEW.player1_army_list_id, NEW.player2_army_list_id
    WHERE NEW.elo_match_id IS NULL
      AND NEW.player1 IS NOT NULL AND trim(NEW.player1) != ''
      AND NEW.player2 IS NOT NULL AND trim(NEW.player2) != '';

    INSERT OR REPLACE INTO schema_id_hold (k, v) VALUES (
        'tm_match',
        CASE
            WHEN NEW.elo_match_id IS NOT NULL THEN NEW.elo_match_id
            WHEN NEW.player1 IS NOT NULL AND trim(NEW.player1) != ''
             AND NEW.player2 IS NOT NULL AND trim(NEW.player2) != '' THEN last_insert_rowid()
            ELSE NULL
        END
    );

    INSERT INTO tournament_match_rows (
        tournament_id, phase, pool_id, bracket_slot, match_id,
        player1_tournament_points, player2_tournament_points,
        is_forfeit, forfeit_player, is_unplayed,
        player1_elo_delta, player2_elo_delta,
        player1_rating_used, player2_rating_used, elo_applied_at,
        status, submitted_by_user_id, submitted_at,
        confirmed_by_user_id, confirmed_at, played_at
    ) VALUES (
        NEW.tournament_id, NEW.phase, NEW.pool_id, NEW.bracket_slot,
        (SELECT v FROM schema_id_hold WHERE k = 'tm_match'),
        COALESCE(NEW.player1_tournament_points, 0),
        COALESCE(NEW.player2_tournament_points, 0),
        COALESCE(NEW.is_forfeit, 0),
        NEW.forfeit_player,
        COALESCE(NEW.is_unplayed, 0),
        COALESCE(NEW.player1_elo_delta, 0),
        COALESCE(NEW.player2_elo_delta, 0),
        NEW.player1_rating_used,
        NEW.player2_rating_used,
        NEW.elo_applied_at,
        COALESCE(NEW.status, 'scheduled'),
        NEW.submitted_by_user_id,
        NEW.submitted_at,
        NEW.confirmed_by_user_id,
        NEW.confirmed_at,
        NEW.played_at
    );

    DELETE FROM schema_rowid_echo;
    INSERT INTO schema_rowid_echo (id) VALUES (last_insert_rowid());
END;

CREATE TRIGGER tournament_matches_delete
INSTEAD OF DELETE ON tournament_matches
BEGIN
    DELETE FROM matches
    WHERE id = (SELECT match_id FROM tournament_match_rows WHERE id = OLD.id)
      AND status = 'to_be_played';
    DELETE FROM tournament_match_rows WHERE id = OLD.id;
END;

CREATE TRIGGER tournament_matches_update
INSTEAD OF UPDATE ON tournament_matches
BEGIN
    DELETE FROM matches
    WHERE NEW.elo_match_id IS NOT NULL
      AND id = (SELECT match_id FROM tournament_match_rows WHERE id = OLD.id)
      AND id != NEW.elo_match_id
      AND status = 'to_be_played';

    INSERT OR REPLACE INTO schema_id_hold (k, v) VALUES (
        'tm_match',
        CASE
            WHEN NEW.elo_match_id IS NOT NULL THEN NEW.elo_match_id
            WHEN OLD.elo_match_id IS NOT NULL AND NEW.elo_match_id IS NULL THEN NULL
            ELSE (SELECT match_id FROM tournament_match_rows WHERE id = OLD.id)
        END
    );

    INSERT INTO matches (
        player1, player2, outcome,
        player1_old, player1_new, player2_old, player2_new,
        player1_objectives, player1_survivors, player2_objectives, player2_survivors,
        player1_army_id, player2_army_id,
        scenario_id, scenario_other,
        tournament_id, tournament_phase,
        status, counts_for_elo, recorded_at,
        player1_army_list_code, player2_army_list_code,
        player1_army_list_id, player2_army_list_id
    )
    SELECT
        NEW.player1, NEW.player2, COALESCE(NEW.outcome, ''),
        0, 0, 0, 0,
        COALESCE(NEW.player1_objectives, 0), COALESCE(NEW.player1_survivors, 0),
        COALESCE(NEW.player2_objectives, 0), COALESCE(NEW.player2_survivors, 0),
        NEW.player1_army_id, NEW.player2_army_id,
        NEW.scenario_id, NEW.scenario_other,
        NEW.tournament_id, NEW.phase,
        'to_be_played', 0, COALESCE(NEW.played_at, 0),
        NEW.player1_army_list_code, NEW.player2_army_list_code,
        NEW.player1_army_list_id, NEW.player2_army_list_id
    WHERE (SELECT v FROM schema_id_hold WHERE k = 'tm_match') IS NULL
      AND NEW.player1 IS NOT NULL AND trim(NEW.player1) != ''
      AND NEW.player2 IS NOT NULL AND trim(NEW.player2) != '';

    UPDATE schema_id_hold SET v = last_insert_rowid()
    WHERE k = 'tm_match'
      AND v IS NULL
      AND NEW.player1 IS NOT NULL AND trim(NEW.player1) != ''
      AND NEW.player2 IS NOT NULL AND trim(NEW.player2) != '';

    UPDATE matches SET
        player1 = NEW.player1,
        player2 = NEW.player2,
        outcome = CASE
            WHEN NEW.outcome IS NULL OR trim(NEW.outcome) = '' THEN ''
            ELSE NEW.outcome
        END,
        player1_objectives = COALESCE(NEW.player1_objectives, 0),
        player2_objectives = COALESCE(NEW.player2_objectives, 0),
        player1_survivors = COALESCE(NEW.player1_survivors, 0),
        player2_survivors = COALESCE(NEW.player2_survivors, 0),
        player1_army_id = NEW.player1_army_id,
        player2_army_id = NEW.player2_army_id,
        scenario_id = NEW.scenario_id,
        scenario_other = NEW.scenario_other,
        player1_army_list_code = NEW.player1_army_list_code,
        player2_army_list_code = NEW.player2_army_list_code,
        player1_army_list_id = NEW.player1_army_list_id,
        player2_army_list_id = NEW.player2_army_list_id,
        tournament_id = NEW.tournament_id,
        tournament_phase = NEW.phase
    WHERE id = (SELECT v FROM schema_id_hold WHERE k = 'tm_match');

    DELETE FROM matches
    WHERE id = (SELECT v FROM schema_id_hold WHERE k = 'tm_match')
      AND status = 'to_be_played'
      AND (
        NEW.player1 IS NULL OR trim(NEW.player1) = ''
        OR NEW.player2 IS NULL OR trim(NEW.player2) = ''
      );

    UPDATE schema_id_hold SET v = NULL
    WHERE k = 'tm_match'
      AND (
        NEW.player1 IS NULL OR trim(COALESCE(NEW.player1, '')) = ''
        OR NEW.player2 IS NULL OR trim(COALESCE(NEW.player2, '')) = ''
      )
      AND NOT EXISTS (
        SELECT 1 FROM matches
        WHERE id = schema_id_hold.v AND status != 'to_be_played'
      );

    UPDATE tournament_match_rows SET
        tournament_id = NEW.tournament_id,
        phase = NEW.phase,
        pool_id = NEW.pool_id,
        bracket_slot = NEW.bracket_slot,
        match_id = (SELECT v FROM schema_id_hold WHERE k = 'tm_match'),
        player1_tournament_points = COALESCE(NEW.player1_tournament_points, 0),
        player2_tournament_points = COALESCE(NEW.player2_tournament_points, 0),
        is_forfeit = COALESCE(NEW.is_forfeit, 0),
        forfeit_player = NEW.forfeit_player,
        is_unplayed = COALESCE(NEW.is_unplayed, 0),
        player1_elo_delta = COALESCE(NEW.player1_elo_delta, 0),
        player2_elo_delta = COALESCE(NEW.player2_elo_delta, 0),
        player1_rating_used = NEW.player1_rating_used,
        player2_rating_used = NEW.player2_rating_used,
        elo_applied_at = NEW.elo_applied_at,
        status = COALESCE(NEW.status, 'scheduled'),
        submitted_by_user_id = NEW.submitted_by_user_id,
        submitted_at = NEW.submitted_at,
        confirmed_by_user_id = NEW.confirmed_by_user_id,
        confirmed_at = NEW.confirmed_at,
        played_at = NEW.played_at
    WHERE id = OLD.id;
END;
"#;

fn rework_tts(conn: &Connection) -> Result<()> {
    let kind: Option<String> = conn
        .query_row(
            "SELECT type FROM sqlite_master WHERE name = 'tts_profiles'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if kind.as_deref() != Some("table") {
        return Ok(());
    }
    if !column_exists(conn, "tts_profiles", "tts_loadout_id")? {
        return Ok(());
    }

    conn.execute_batch(
        "
        CREATE TABLE units (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            nom TEXT NOT NULL UNIQUE
        );
        INSERT INTO units (id, nom) SELECT id, nom FROM tts_units;

        CREATE TABLE unit_factions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            slug TEXT NOT NULL UNIQUE,
            label TEXT NOT NULL
        );
        INSERT INTO unit_factions (slug, label)
        SELECT COALESCE(faction_slug, ''), MIN(faction_label)
        FROM tts_profiles
        GROUP BY COALESCE(faction_slug, '');
        INSERT INTO unit_factions (slug, label)
        SELECT '', 'Inconnue'
        WHERE NOT EXISTS (SELECT 1 FROM unit_factions WHERE slug = '');

        CREATE TABLE unit_tts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            nom_tts TEXT NOT NULL,
            description_tts TEXT NOT NULL DEFAULT ''
        );
        INSERT INTO unit_tts (id, nom_tts, description_tts)
        SELECT id, nom_tts, description_tts FROM tts_loadouts;

        CREATE TABLE unit_loadouts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            nom TEXT NOT NULL,
            silhouette_size TEXT,
            silhouette_type TEXT,
            marker_name TEXT,
            mesh_url TEXT,
            marker_mesh_url TEXT
        );
        INSERT INTO unit_loadouts (
            id, nom, silhouette_size, silhouette_type, marker_name, mesh_url, marker_mesh_url
        )
        SELECT id, nom_tts, silhouette_size, silhouette_type, marker_name, mesh_url, marker_mesh_url
        FROM tts_loadouts;

        CREATE TABLE unit_loadout_tts (
            loadout_id INTEGER PRIMARY KEY REFERENCES unit_loadouts(id) ON DELETE CASCADE,
            tts_id INTEGER NOT NULL REFERENCES unit_tts(id)
        );
        INSERT INTO unit_loadout_tts (loadout_id, tts_id)
        SELECT id, id FROM tts_loadouts;

        CREATE TABLE unit_profiles (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            army_id INTEGER,
            unit_id INTEGER NOT NULL REFERENCES units(id),
            faction_id INTEGER NOT NULL REFERENCES unit_factions(id),
            tts_id INTEGER NOT NULL REFERENCES unit_tts(id),
            loadout_id INTEGER NOT NULL REFERENCES unit_loadouts(id),
            parent_profile_id INTEGER REFERENCES unit_profiles(id) ON DELETE CASCADE,
            state_index INTEGER NOT NULL DEFAULT 0,
            loadout TEXT NOT NULL DEFAULT '',
            source_bag TEXT NOT NULL DEFAULT '',
            source_index INTEGER NOT NULL DEFAULT 0
        );
        INSERT INTO unit_profiles (
            id, army_id, unit_id, faction_id, tts_id, loadout_id, parent_profile_id,
            state_index, loadout, source_bag, source_index
        )
        SELECT p.id, p.army_id, p.unit_id, f.id, p.tts_loadout_id, p.tts_loadout_id,
               p.parent_profile_id, p.state_index, p.loadout, p.source_bag, p.source_index
        FROM tts_profiles p
        JOIN unit_factions f ON f.slug = COALESCE(p.faction_slug, '');

        CREATE TABLE unit_profile_traits (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            profile_id INTEGER NOT NULL REFERENCES unit_profiles(id) ON DELETE CASCADE,
            kind TEXT NOT NULL,
            name TEXT NOT NULL,
            detail TEXT,
            sort_order INTEGER NOT NULL DEFAULT 0
        );
        INSERT INTO unit_profile_traits (id, profile_id, kind, name, detail, sort_order)
        SELECT id, profile_id, kind, name, detail, sort_order FROM tts_profile_traits;

        CREATE TABLE unit_profile_sectorials (
            profile_id INTEGER NOT NULL REFERENCES unit_profiles(id) ON DELETE CASCADE,
            army_id INTEGER NOT NULL,
            PRIMARY KEY (profile_id, army_id)
        );
        INSERT INTO unit_profile_sectorials (profile_id, army_id)
        SELECT profile_id, army_id FROM tts_profile_sectorials;
        ",
    )?;

    link_orphan_figurines(conn)?;

    conn.execute_batch(
        "
        CREATE TABLE tts_figurine_rows (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            painter TEXT NOT NULL DEFAULT '',
            asset_bundle_url TEXT NOT NULL DEFAULT '',
            photo_filename TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            loadout_id INTEGER NOT NULL REFERENCES unit_loadouts(id),
            label_nom TEXT NOT NULL DEFAULT '',
            label_loadout TEXT NOT NULL DEFAULT ''
        );
        INSERT INTO tts_figurine_rows (
            id, painter, asset_bundle_url, photo_filename, created_at, updated_at, loadout_id,
            label_nom, label_loadout
        )
        SELECT f.id, f.painter, f.asset_bundle_url, f.photo_filename, f.created_at, f.updated_at,
               COALESCE(
                   f.tts_loadout_id,
                   (
                       SELECT m.loadout_id FROM orphan_loadout_map m
                       WHERE m.nom = COALESCE(NULLIF(trim(f.legacy_nom), ''), NULLIF(trim(f.isc), ''), 'Sans nom')
                   )
               ),
               COALESCE(NULLIF(trim(f.legacy_nom), ''), f.isc, ''),
               COALESCE(NULLIF(trim(f.legacy_loadout), ''), f.loadout, '')
        FROM tts_figurines f
        WHERE COALESCE(
            f.tts_loadout_id,
            (
                SELECT m.loadout_id FROM orphan_loadout_map m
                WHERE m.nom = COALESCE(NULLIF(trim(f.legacy_nom), ''), NULLIF(trim(f.isc), ''), 'Sans nom')
            )
        ) IS NOT NULL;

        DROP TABLE tts_figurines;
        DROP TABLE tts_profile_traits;
        DROP TABLE tts_profile_sectorials;
        DROP TABLE tts_profiles;
        DROP TABLE tts_loadouts;
        DROP TABLE tts_units;
        DROP TABLE IF EXISTS orphan_loadout_map;

        CREATE VIEW tts_units AS
        SELECT id, nom FROM units;

        CREATE VIEW tts_loadouts AS
        SELECT
            l.id,
            COALESCE(t.nom_tts, l.nom) AS nom_tts,
            COALESCE(t.description_tts, '') AS description_tts,
            l.silhouette_size,
            l.silhouette_type,
            l.marker_name,
            l.mesh_url,
            l.marker_mesh_url
        FROM unit_loadouts l
        LEFT JOIN unit_loadout_tts map ON map.loadout_id = l.id
        LEFT JOIN unit_tts t ON t.id = map.tts_id;

        CREATE VIEW tts_profiles AS
        SELECT
            p.id,
            p.army_id,
            NULLIF(f.slug, '') AS faction_slug,
            f.label AS faction_label,
            p.unit_id,
            p.loadout_id AS tts_loadout_id,
            p.tts_id,
            t.nom_tts,
            t.description_tts,
            p.parent_profile_id,
            p.state_index,
            p.loadout,
            p.source_bag,
            p.source_index
        FROM unit_profiles p
        JOIN unit_factions f ON f.id = p.faction_id
        JOIN unit_tts t ON t.id = p.tts_id;

        CREATE VIEW tts_profile_traits AS
        SELECT id, profile_id, kind, name, detail, sort_order
        FROM unit_profile_traits;

        CREATE VIEW tts_profile_sectorials AS
        SELECT profile_id, army_id FROM unit_profile_sectorials;

        CREATE VIEW tts_figurines AS
        SELECT
            f.id,
            COALESCE(
                (SELECT u.nom FROM unit_profiles p JOIN units u ON u.id = p.unit_id
                 WHERE p.loadout_id = f.loadout_id
                 ORDER BY p.parent_profile_id IS NOT NULL, p.id LIMIT 1),
                NULLIF(f.label_nom, ''),
                l.nom
            ) AS isc,
            COALESCE(
                (SELECT p.loadout FROM unit_profiles p
                 WHERE p.loadout_id = f.loadout_id
                 ORDER BY p.parent_profile_id IS NOT NULL, p.id LIMIT 1),
                f.label_loadout
            ) AS loadout,
            f.painter,
            f.asset_bundle_url,
            f.photo_filename,
            f.created_at,
            f.updated_at,
            f.label_nom AS legacy_nom,
            f.label_loadout AS legacy_loadout,
            CASE
                WHEN EXISTS (
                    SELECT 1 FROM unit_profiles p WHERE p.loadout_id = f.loadout_id
                ) THEN f.loadout_id
                ELSE NULL
            END AS tts_loadout_id
        FROM tts_figurine_rows f
        JOIN unit_loadouts l ON l.id = f.loadout_id;

        CREATE INDEX IF NOT EXISTS idx_units_nom ON units(nom);
        CREATE INDEX IF NOT EXISTS idx_unit_profiles_unit ON unit_profiles(unit_id);
        CREATE INDEX IF NOT EXISTS idx_unit_profiles_loadout ON unit_profiles(loadout_id);
        CREATE INDEX IF NOT EXISTS idx_unit_profiles_tts ON unit_profiles(tts_id);
        CREATE INDEX IF NOT EXISTS idx_unit_profiles_parent ON unit_profiles(parent_profile_id, state_index);
        CREATE INDEX IF NOT EXISTS idx_unit_profile_traits_profile ON unit_profile_traits(profile_id, sort_order);
        CREATE INDEX IF NOT EXISTS idx_tts_figurine_rows_loadout ON tts_figurine_rows(loadout_id);
        ",
    )?;
    bump_sequence(conn, "units")?;
    bump_sequence(conn, "unit_factions")?;
    bump_sequence(conn, "unit_tts")?;
    bump_sequence(conn, "unit_loadouts")?;
    bump_sequence(conn, "unit_profiles")?;
    bump_sequence(conn, "unit_profile_traits")?;
    bump_sequence(conn, "tts_figurine_rows")?;
    conn.execute_batch(TTS_TRIGGERS)?;
    crate::unit_admin::clean_figurine_loadout_names(conn)?;
    Ok(())
}

fn link_orphan_figurines(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TEMP TABLE orphan_loadout_map (
            nom TEXT PRIMARY KEY,
            loadout_id INTEGER NOT NULL
        );
        ",
    )?;
    let mut stmt = conn.prepare(
        "
        SELECT DISTINCT COALESCE(NULLIF(trim(legacy_nom), ''), NULLIF(trim(isc), ''), 'Sans nom')
        FROM tts_figurines
        WHERE tts_loadout_id IS NULL
        ",
    )?;
    let names: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);
    for name in names {
        conn.execute("INSERT INTO unit_loadouts (nom) VALUES (?1)", params![name])?;
        let loadout_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO unit_tts (nom_tts, description_tts) VALUES (?1, '')",
            params![name],
        )?;
        let tts_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO unit_loadout_tts (loadout_id, tts_id) VALUES (?1, ?2)",
            params![loadout_id, tts_id],
        )?;
        conn.execute(
            "INSERT INTO orphan_loadout_map (nom, loadout_id) VALUES (?1, ?2)",
            params![name, loadout_id],
        )?;
    }
    Ok(())
}

const TTS_TRIGGERS: &str = r#"
CREATE TRIGGER tts_units_insert
INSTEAD OF INSERT ON tts_units
BEGIN
    INSERT OR IGNORE INTO units (nom) VALUES (trim(NEW.nom));
    DELETE FROM schema_rowid_echo;
    INSERT INTO schema_rowid_echo (id)
    SELECT id FROM units WHERE nom = trim(NEW.nom);
END;

CREATE TRIGGER tts_units_update
INSTEAD OF UPDATE ON tts_units
BEGIN
    UPDATE units SET nom = trim(NEW.nom) WHERE id = OLD.id;
END;

CREATE TRIGGER tts_loadouts_insert
INSTEAD OF INSERT ON tts_loadouts
BEGIN
    INSERT INTO unit_tts (nom_tts, description_tts)
    VALUES (NEW.nom_tts, COALESCE(NEW.description_tts, ''));
    INSERT OR REPLACE INTO schema_id_hold (k, v) VALUES ('tts', last_insert_rowid());
    INSERT INTO unit_loadouts (
        nom, silhouette_size, silhouette_type, marker_name, mesh_url, marker_mesh_url
    ) VALUES (
        NEW.nom_tts, NEW.silhouette_size, NEW.silhouette_type, NEW.marker_name,
        NEW.mesh_url, NEW.marker_mesh_url
    );
    INSERT OR REPLACE INTO schema_id_hold (k, v) VALUES ('loadout', last_insert_rowid());
    INSERT INTO unit_loadout_tts (loadout_id, tts_id)
    SELECT
        (SELECT v FROM schema_id_hold WHERE k = 'loadout'),
        (SELECT v FROM schema_id_hold WHERE k = 'tts');
    DELETE FROM schema_rowid_echo;
    INSERT INTO schema_rowid_echo (id)
    SELECT v FROM schema_id_hold WHERE k = 'loadout';
END;

CREATE TRIGGER tts_loadouts_update
INSTEAD OF UPDATE ON tts_loadouts
BEGIN
    UPDATE unit_loadouts SET
        nom = COALESCE(NEW.nom_tts, nom),
        silhouette_size = NEW.silhouette_size,
        silhouette_type = NEW.silhouette_type,
        marker_name = NEW.marker_name,
        mesh_url = NEW.mesh_url,
        marker_mesh_url = NEW.marker_mesh_url
    WHERE id = OLD.id;
    UPDATE unit_tts SET
        nom_tts = COALESCE(NEW.nom_tts, nom_tts),
        description_tts = COALESCE(NEW.description_tts, description_tts)
    WHERE id = (SELECT tts_id FROM unit_loadout_tts WHERE loadout_id = OLD.id);
END;

CREATE TRIGGER tts_loadouts_delete
INSTEAD OF DELETE ON tts_loadouts
BEGIN
    DELETE FROM unit_tts
    WHERE id = (SELECT tts_id FROM unit_loadout_tts WHERE loadout_id = OLD.id)
      AND NOT EXISTS (SELECT 1 FROM unit_profiles WHERE tts_id = unit_tts.id);
    DELETE FROM unit_loadout_tts WHERE loadout_id = OLD.id;
    DELETE FROM unit_loadouts
    WHERE id = OLD.id
      AND NOT EXISTS (SELECT 1 FROM tts_figurine_rows WHERE loadout_id = OLD.id);
END;

CREATE TRIGGER tts_profiles_insert
INSTEAD OF INSERT ON tts_profiles
BEGIN
    INSERT INTO unit_factions (slug, label)
    SELECT COALESCE(NEW.faction_slug, ''), COALESCE(NEW.faction_label, '')
    WHERE NOT EXISTS (
        SELECT 1 FROM unit_factions WHERE slug = COALESCE(NEW.faction_slug, '')
    );
    INSERT INTO unit_profiles (
        army_id, unit_id, faction_id, tts_id, loadout_id, parent_profile_id,
        state_index, loadout, source_bag, source_index
    ) VALUES (
        NEW.army_id,
        NEW.unit_id,
        (SELECT id FROM unit_factions WHERE slug = COALESCE(NEW.faction_slug, '')),
        COALESCE(
            (SELECT tts_id FROM unit_loadout_tts WHERE loadout_id = NEW.tts_loadout_id),
            (SELECT tts_id FROM unit_profiles WHERE loadout_id = NEW.tts_loadout_id LIMIT 1)
        ),
        NEW.tts_loadout_id,
        NEW.parent_profile_id,
        COALESCE(NEW.state_index, 0),
        COALESCE(NEW.loadout, ''),
        COALESCE(NEW.source_bag, ''),
        COALESCE(NEW.source_index, 0)
    );
    DELETE FROM schema_rowid_echo;
    INSERT INTO schema_rowid_echo (id) VALUES (last_insert_rowid());
END;

CREATE TRIGGER tts_profiles_update
INSTEAD OF UPDATE ON tts_profiles
BEGIN
    UPDATE unit_profiles SET
        army_id = NEW.army_id,
        unit_id = COALESCE(NEW.unit_id, unit_id),
        loadout_id = COALESCE(NEW.tts_loadout_id, loadout_id),
        tts_id = CASE
            WHEN NEW.tts_loadout_id IS NOT NULL AND NEW.tts_loadout_id != OLD.tts_loadout_id
            THEN COALESCE(
                (SELECT tts_id FROM unit_loadout_tts WHERE loadout_id = NEW.tts_loadout_id),
                tts_id
            )
            ELSE tts_id
        END,
        parent_profile_id = NEW.parent_profile_id,
        state_index = COALESCE(NEW.state_index, state_index),
        loadout = COALESCE(NEW.loadout, loadout),
        source_bag = COALESCE(NEW.source_bag, source_bag),
        source_index = COALESCE(NEW.source_index, source_index)
    WHERE id = OLD.id;
END;

CREATE TRIGGER tts_profiles_delete
INSTEAD OF DELETE ON tts_profiles
BEGIN
    DELETE FROM unit_profiles WHERE id = OLD.id;
END;

CREATE TRIGGER tts_profile_traits_insert
INSTEAD OF INSERT ON tts_profile_traits
BEGIN
    INSERT INTO unit_profile_traits (profile_id, kind, name, detail, sort_order)
    VALUES (NEW.profile_id, NEW.kind, NEW.name, NEW.detail, COALESCE(NEW.sort_order, 0));
END;

CREATE TRIGGER tts_profile_traits_delete
INSTEAD OF DELETE ON tts_profile_traits
BEGIN
    DELETE FROM unit_profile_traits WHERE id = OLD.id;
END;

CREATE TRIGGER tts_profile_sectorials_insert
INSTEAD OF INSERT ON tts_profile_sectorials
BEGIN
    INSERT OR IGNORE INTO unit_profile_sectorials (profile_id, army_id)
    VALUES (NEW.profile_id, NEW.army_id);
END;

CREATE TRIGGER tts_profile_sectorials_delete
INSTEAD OF DELETE ON tts_profile_sectorials
BEGIN
    DELETE FROM unit_profile_sectorials
    WHERE profile_id = OLD.profile_id AND army_id = OLD.army_id;
END;

CREATE TRIGGER tts_figurines_insert
INSTEAD OF INSERT ON tts_figurines
BEGIN
    INSERT INTO unit_loadouts (nom)
    SELECT COALESCE(NULLIF(trim(NEW.legacy_nom), ''), NULLIF(trim(NEW.isc), ''), 'Sans nom')
    WHERE NEW.tts_loadout_id IS NULL;
    INSERT OR REPLACE INTO schema_id_hold (k, v)
    SELECT 'fig_loadout', last_insert_rowid()
    WHERE NEW.tts_loadout_id IS NULL;

    INSERT INTO unit_tts (nom_tts, description_tts)
    SELECT COALESCE(NULLIF(trim(NEW.legacy_nom), ''), NULLIF(trim(NEW.isc), ''), 'Sans nom'), ''
    WHERE NEW.tts_loadout_id IS NULL;
    INSERT OR REPLACE INTO schema_id_hold (k, v)
    SELECT 'fig_tts', last_insert_rowid()
    WHERE NEW.tts_loadout_id IS NULL;

    INSERT INTO unit_loadout_tts (loadout_id, tts_id)
    SELECT
        (SELECT v FROM schema_id_hold WHERE k = 'fig_loadout'),
        (SELECT v FROM schema_id_hold WHERE k = 'fig_tts')
    WHERE NEW.tts_loadout_id IS NULL;

    INSERT INTO tts_figurine_rows (
        painter, asset_bundle_url, photo_filename, created_at, updated_at, loadout_id,
        label_nom, label_loadout
    ) VALUES (
        COALESCE(NEW.painter, ''),
        COALESCE(NEW.asset_bundle_url, ''),
        NEW.photo_filename,
        COALESCE(NEW.created_at, CAST(strftime('%s','now') AS INTEGER)),
        COALESCE(NEW.updated_at, CAST(strftime('%s','now') AS INTEGER)),
        COALESCE(
            NEW.tts_loadout_id,
            (SELECT v FROM schema_id_hold WHERE k = 'fig_loadout')
        ),
        COALESCE(NULLIF(trim(NEW.legacy_nom), ''), NULLIF(trim(NEW.isc), ''), ''),
        COALESCE(NULLIF(trim(NEW.legacy_loadout), ''), NULLIF(trim(NEW.loadout), ''), '')
    );
    DELETE FROM schema_rowid_echo;
    INSERT INTO schema_rowid_echo (id) VALUES (last_insert_rowid());
END;

CREATE TRIGGER tts_figurines_update
INSTEAD OF UPDATE ON tts_figurines
BEGIN
    UPDATE tts_figurine_rows SET
        painter = COALESCE(NEW.painter, painter),
        asset_bundle_url = COALESCE(NEW.asset_bundle_url, asset_bundle_url),
        photo_filename = NEW.photo_filename,
        updated_at = COALESCE(NEW.updated_at, updated_at),
        loadout_id = COALESCE(NEW.tts_loadout_id, loadout_id),
        label_nom = COALESCE(NULLIF(trim(NEW.legacy_nom), ''), NULLIF(trim(NEW.isc), ''), label_nom),
        label_loadout = COALESCE(
            NULLIF(trim(NEW.legacy_loadout), ''),
            NULLIF(trim(NEW.loadout), ''),
            label_loadout
        )
    WHERE id = OLD.id;
END;

CREATE TRIGGER tts_figurines_delete
INSTEAD OF DELETE ON tts_figurines
BEGIN
    DELETE FROM tts_figurine_rows WHERE id = OLD.id;
END;
"#;

fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

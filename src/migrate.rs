use anyhow::Result;
use rusqlite::Connection;

pub fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS players (
            name_key TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            rating REAL NOT NULL,
            wins INTEGER NOT NULL DEFAULT 0,
            draws INTEGER NOT NULL DEFAULT 0,
            losses INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS matches (
            id INTEGER PRIMARY KEY,
            player1 TEXT NOT NULL,
            player2 TEXT NOT NULL,
            outcome TEXT NOT NULL,
            player1_old REAL NOT NULL,
            player1_new REAL NOT NULL,
            player2_old REAL NOT NULL,
            player2_new REAL NOT NULL,
            player1_objectives INTEGER NOT NULL DEFAULT 0,
            player1_survivors INTEGER NOT NULL DEFAULT 0,
            player2_objectives INTEGER NOT NULL DEFAULT 0,
            player2_survivors INTEGER NOT NULL DEFAULT 0,
            player1_army_id INTEGER,
            player2_army_id INTEGER,
            recorded_at INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_matches_recorded_at ON matches(recorded_at DESC);

        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            discord_id TEXT NOT NULL UNIQUE,
            username TEXT NOT NULL,
            display_name TEXT NOT NULL,
            avatar_url TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            last_login_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY NOT NULL,
            data BLOB NOT NULL,
            expiry_date INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_sessions_expiry_date ON sessions(expiry_date);

        CREATE TABLE IF NOT EXISTS scenarios (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            name_key TEXT NOT NULL UNIQUE,
            usage_count INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS tournaments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'draft',
            pool_count INTEGER NOT NULL DEFAULT 4,
            bracket_format TEXT NOT NULL DEFAULT 'quarters_direct',
            created_at INTEGER NOT NULL,
            started_at INTEGER,
            pools_finalized_at INTEGER,
            completed_at INTEGER
        );

        CREATE TABLE IF NOT EXISTS tournament_registrations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tournament_id INTEGER NOT NULL REFERENCES tournaments(id),
            player_name_key TEXT NOT NULL,
            player_name TEXT NOT NULL,
            user_id INTEGER,
            status TEXT NOT NULL DEFAULT 'pending',
            waitlist_position INTEGER,
            requested_at INTEGER NOT NULL,
            reviewed_at INTEGER,
            reviewed_by INTEGER,
            army_id INTEGER,
            UNIQUE(tournament_id, player_name_key)
        );

        CREATE TABLE IF NOT EXISTS tournament_players (
            tournament_id INTEGER NOT NULL REFERENCES tournaments(id),
            player_name_key TEXT NOT NULL,
            player_name TEXT NOT NULL,
            start_rating REAL NOT NULL,
            pool_elo_delta REAL NOT NULL DEFAULT 0,
            bracket_rating REAL NOT NULL DEFAULT 0,
            pool_points INTEGER NOT NULL DEFAULT 0,
            pool_objectives INTEGER NOT NULL DEFAULT 0,
            pool_survivors INTEGER NOT NULL DEFAULT 0,
            final_placement INTEGER,
            PRIMARY KEY (tournament_id, player_name_key)
        );

        CREATE TABLE IF NOT EXISTS pools (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tournament_id INTEGER NOT NULL REFERENCES tournaments(id),
            name TEXT NOT NULL,
            position INTEGER NOT NULL,
            UNIQUE(tournament_id, position)
        );

        CREATE TABLE IF NOT EXISTS pool_players (
            pool_id INTEGER NOT NULL REFERENCES pools(id),
            player_name_key TEXT NOT NULL,
            player_name TEXT NOT NULL,
            seed INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (pool_id, player_name_key)
        );

        CREATE TABLE IF NOT EXISTS tournament_matches (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tournament_id INTEGER NOT NULL REFERENCES tournaments(id),
            phase TEXT NOT NULL,
            pool_id INTEGER REFERENCES pools(id),
            bracket_slot INTEGER,
            player1 TEXT,
            player2 TEXT,
            player1_objectives INTEGER NOT NULL DEFAULT 0,
            player2_objectives INTEGER NOT NULL DEFAULT 0,
            player1_survivors INTEGER NOT NULL DEFAULT 0,
            player2_survivors INTEGER NOT NULL DEFAULT 0,
            player1_tournament_points INTEGER NOT NULL DEFAULT 0,
            player2_tournament_points INTEGER NOT NULL DEFAULT 0,
            outcome TEXT,
            is_forfeit INTEGER NOT NULL DEFAULT 0,
            forfeit_player TEXT,
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
            scenario_id INTEGER REFERENCES scenarios(id),
            scenario_other TEXT,
            player1_army_id INTEGER,
            player2_army_id INTEGER,
            player1_army_list_code TEXT,
            player2_army_list_code TEXT,
            played_at INTEGER
        );

        CREATE INDEX IF NOT EXISTS idx_tournament_matches_tournament
            ON tournament_matches(tournament_id);
        CREATE INDEX IF NOT EXISTS idx_tournament_registrations_tournament
            ON tournament_registrations(tournament_id);
        ",
    )?;

    if column_exists(conn, "players", "discord_id")?
        && !column_exists(conn, "players", "discord_username")?
    {
        conn.execute(
            "ALTER TABLE players RENAME COLUMN discord_id TO discord_username",
            [],
        )?;
    } else if !column_exists(conn, "players", "discord_username")? {
        conn.execute("ALTER TABLE players ADD COLUMN discord_username TEXT", [])?;
    }

    if !column_exists(conn, "users", "is_admin")? {
        conn.execute(
            "ALTER TABLE users ADD COLUMN is_admin INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }

    if !column_exists(conn, "matches", "scenario_id")? {
        conn.execute("ALTER TABLE matches ADD COLUMN scenario_id INTEGER", [])?;
    }

    rename_scenario_name_to_other(conn, "matches")?;
    rename_scenario_name_to_other(conn, "tournament_matches")?;

    if !column_exists(conn, "matches", "tournament_id")? {
        conn.execute("ALTER TABLE matches ADD COLUMN tournament_id INTEGER", [])?;
    }

    if !column_exists(conn, "matches", "tournament_phase")? {
        conn.execute("ALTER TABLE matches ADD COLUMN tournament_phase TEXT", [])?;
    }

    if !column_exists(conn, "matches", "player1_report_md")? {
        conn.execute("ALTER TABLE matches ADD COLUMN player1_report_md TEXT", [])?;
    }

    if !column_exists(conn, "matches", "player2_report_md")? {
        conn.execute("ALTER TABLE matches ADD COLUMN player2_report_md TEXT", [])?;
    }

    if !column_exists(conn, "matches", "player1_army_list_code")? {
        conn.execute(
            "ALTER TABLE matches ADD COLUMN player1_army_list_code TEXT",
            [],
        )?;
    }
    if !column_exists(conn, "matches", "player2_army_list_code")? {
        conn.execute(
            "ALTER TABLE matches ADD COLUMN player2_army_list_code TEXT",
            [],
        )?;
    }

    if !column_exists(conn, "matches", "status")? {
        conn.execute(
            "ALTER TABLE matches ADD COLUMN status TEXT NOT NULL DEFAULT 'completed'",
            [],
        )?;
    }

    if !column_exists(conn, "matches", "player1_secondary_slugs")? {
        conn.execute(
            "ALTER TABLE matches ADD COLUMN player1_secondary_slugs TEXT",
            [],
        )?;
    }
    if !column_exists(conn, "matches", "player2_secondary_slugs")? {
        conn.execute(
            "ALTER TABLE matches ADD COLUMN player2_secondary_slugs TEXT",
            [],
        )?;
    }
    if !column_exists(conn, "matches", "player1_chosen_secondary")? {
        conn.execute(
            "ALTER TABLE matches ADD COLUMN player1_chosen_secondary TEXT",
            [],
        )?;
    }
    if !column_exists(conn, "matches", "player2_chosen_secondary")? {
        conn.execute(
            "ALTER TABLE matches ADD COLUMN player2_chosen_secondary TEXT",
            [],
        )?;
    }
    if !column_exists(conn, "matches", "secondary_pool_slugs")? {
        conn.execute(
            "ALTER TABLE matches ADD COLUMN secondary_pool_slugs TEXT",
            [],
        )?;
    }
    if !column_exists(conn, "matches", "lieutenant_winner")? {
        conn.execute("ALTER TABLE matches ADD COLUMN lieutenant_winner TEXT", [])?;
    }
    if !column_exists(conn, "matches", "lieutenant_winner_choice")? {
        conn.execute(
            "ALTER TABLE matches ADD COLUMN lieutenant_winner_choice TEXT",
            [],
        )?;
    }
    if !column_exists(conn, "matches", "lieutenant_other_choice")? {
        conn.execute(
            "ALTER TABLE matches ADD COLUMN lieutenant_other_choice TEXT",
            [],
        )?;
    }
    if !column_exists(conn, "matches", "partie_step")? {
        conn.execute("ALTER TABLE matches ADD COLUMN partie_step TEXT", [])?;
    }
    if !column_exists(conn, "matches", "created_by")? {
        conn.execute("ALTER TABLE matches ADD COLUMN created_by TEXT", [])?;
    }

    if !column_exists(conn, "tournament_matches", "is_unplayed")? {
        conn.execute(
            "ALTER TABLE tournament_matches ADD COLUMN is_unplayed INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }

    if !column_exists(conn, "tournament_matches", "elo_match_id")? {
        conn.execute(
            "ALTER TABLE tournament_matches ADD COLUMN elo_match_id INTEGER REFERENCES matches(id)",
            [],
        )?;
    }

    migrate_match_reports(conn)?;

    if column_exists(conn, "matches", "tournament_id")?
        && column_exists(conn, "matches", "tournament_phase")?
    {
        conn.execute_batch(
            "
            UPDATE matches SET
                tournament_id = (
                    SELECT tm.tournament_id FROM tournament_matches tm
                    WHERE tm.status = 'confirmed'
                      AND tm.is_forfeit = 0
                      AND tm.is_unplayed = 0
                      AND lower(trim(matches.player1)) = lower(trim(tm.player1))
                      AND lower(trim(matches.player2)) = lower(trim(tm.player2))
                      AND matches.player1_objectives = tm.player1_objectives
                      AND matches.player2_objectives = tm.player2_objectives
                      AND matches.player1_survivors = tm.player1_survivors
                      AND matches.player2_survivors = tm.player2_survivors
                    LIMIT 1
                ),
                tournament_phase = (
                    SELECT tm.phase FROM tournament_matches tm
                    WHERE tm.status = 'confirmed'
                      AND tm.is_forfeit = 0
                      AND tm.is_unplayed = 0
                      AND lower(trim(matches.player1)) = lower(trim(tm.player1))
                      AND lower(trim(matches.player2)) = lower(trim(tm.player2))
                      AND matches.player1_objectives = tm.player1_objectives
                      AND matches.player2_objectives = tm.player2_objectives
                      AND matches.player1_survivors = tm.player1_survivors
                      AND matches.player2_survivors = tm.player2_survivors
                    LIMIT 1
                )
            WHERE tournament_id IS NULL
              AND EXISTS (
                SELECT 1 FROM tournament_matches tm
                WHERE tm.status = 'confirmed'
                  AND tm.is_forfeit = 0
                  AND tm.is_unplayed = 0
                  AND lower(trim(matches.player1)) = lower(trim(tm.player1))
                  AND lower(trim(matches.player2)) = lower(trim(tm.player2))
                  AND matches.player1_objectives = tm.player1_objectives
                  AND matches.player2_objectives = tm.player2_objectives
                  AND matches.player1_survivors = tm.player1_survivors
                  AND matches.player2_survivors = tm.player2_survivors
              );

            UPDATE matches SET
                tournament_id = (
                    SELECT tm.tournament_id FROM tournament_matches tm
                    WHERE tm.status = 'confirmed'
                      AND tm.is_forfeit = 0
                      AND tm.is_unplayed = 0
                      AND lower(trim(matches.player1)) = lower(trim(tm.player2))
                      AND lower(trim(matches.player2)) = lower(trim(tm.player1))
                      AND matches.player1_objectives = tm.player2_objectives
                      AND matches.player2_objectives = tm.player1_objectives
                      AND matches.player1_survivors = tm.player2_survivors
                      AND matches.player2_survivors = tm.player1_survivors
                    LIMIT 1
                ),
                tournament_phase = (
                    SELECT tm.phase FROM tournament_matches tm
                    WHERE tm.status = 'confirmed'
                      AND tm.is_forfeit = 0
                      AND tm.is_unplayed = 0
                      AND lower(trim(matches.player1)) = lower(trim(tm.player2))
                      AND lower(trim(matches.player2)) = lower(trim(tm.player1))
                      AND matches.player1_objectives = tm.player2_objectives
                      AND matches.player2_objectives = tm.player1_objectives
                      AND matches.player1_survivors = tm.player2_survivors
                      AND matches.player2_survivors = tm.player1_survivors
                    LIMIT 1
                )
            WHERE tournament_id IS NULL
              AND EXISTS (
                SELECT 1 FROM tournament_matches tm
                WHERE tm.status = 'confirmed'
                  AND tm.is_forfeit = 0
                  AND tm.is_unplayed = 0
                  AND lower(trim(matches.player1)) = lower(trim(tm.player2))
                  AND lower(trim(matches.player2)) = lower(trim(tm.player1))
                  AND matches.player1_objectives = tm.player2_objectives
                  AND matches.player2_objectives = tm.player1_objectives
                  AND matches.player1_survivors = tm.player2_survivors
                  AND matches.player2_survivors = tm.player1_survivors
              );
            ",
        )?;
    }

    if !column_exists(conn, "tournament_registrations", "army_id")? {
        conn.execute(
            "ALTER TABLE tournament_registrations ADD COLUMN army_id INTEGER",
            [],
        )?;
    }

    if !column_exists(conn, "users", "local_display_name")? {
        conn.execute("ALTER TABLE users ADD COLUMN local_display_name TEXT", [])?;
    }

    if !column_exists(conn, "users", "local_avatar_url")? {
        conn.execute("ALTER TABLE users ADD COLUMN local_avatar_url TEXT", [])?;
    }

    if !column_exists(conn, "users", "secondary_view_mode")? {
        conn.execute("ALTER TABLE users ADD COLUMN secondary_view_mode TEXT", [])?;
    }

    if !column_exists(conn, "users", "scenario_slug")? {
        conn.execute("ALTER TABLE users ADD COLUMN scenario_slug TEXT", [])?;
    }

    if !column_exists(conn, "users", "army_sort_mode")? {
        conn.execute("ALTER TABLE users ADD COLUMN army_sort_mode TEXT", [])?;
    }

    if !column_exists(conn, "users", "tournament_completed_view_mode")? {
        conn.execute(
            "ALTER TABLE users ADD COLUMN tournament_completed_view_mode TEXT",
            [],
        )?;
    }

    if !column_exists(conn, "matches", "counts_for_elo")? {
        conn.execute(
            "ALTER TABLE matches ADD COLUMN counts_for_elo INTEGER NOT NULL DEFAULT 1",
            [],
        )?;
    }

    if !column_exists(conn, "matches", "scenario_url")? {
        conn.execute("ALTER TABLE matches ADD COLUMN scenario_url TEXT", [])?;
    }

    if !column_exists(conn, "tournaments", "description")? {
        conn.execute(
            "ALTER TABLE tournaments ADD COLUMN description TEXT NOT NULL DEFAULT ''",
            [],
        )?;
    }

    if !column_exists(conn, "tournaments", "list_validator_user_id")? {
        conn.execute(
            "ALTER TABLE tournaments ADD COLUMN list_validator_user_id INTEGER REFERENCES users(id)",
            [],
        )?;
    }

    if !column_exists(conn, "tournament_matches", "player1_army_list_code")? {
        conn.execute(
            "ALTER TABLE tournament_matches ADD COLUMN player1_army_list_code TEXT",
            [],
        )?;
    }
    if !column_exists(conn, "tournament_matches", "player2_army_list_code")? {
        conn.execute(
            "ALTER TABLE tournament_matches ADD COLUMN player2_army_list_code TEXT",
            [],
        )?;
    }

    if !column_exists(conn, "tournament_registrations", "army_list_1")? {
        conn.execute(
            "ALTER TABLE tournament_registrations ADD COLUMN army_list_1 TEXT",
            [],
        )?;
    }
    if !column_exists(conn, "tournament_registrations", "army_list_2")? {
        conn.execute(
            "ALTER TABLE tournament_registrations ADD COLUMN army_list_2 TEXT",
            [],
        )?;
    }
    if !column_exists(conn, "tournament_registrations", "bracket_list_1")? {
        conn.execute(
            "ALTER TABLE tournament_registrations ADD COLUMN bracket_list_1 TEXT",
            [],
        )?;
    }
    if !column_exists(conn, "tournament_registrations", "bracket_list_2")? {
        conn.execute(
            "ALTER TABLE tournament_registrations ADD COLUMN bracket_list_2 TEXT",
            [],
        )?;
    }

    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS army_lists (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            code TEXT NOT NULL UNIQUE,
            army_id INTEGER NOT NULL REFERENCES armies(id)
        );
        CREATE INDEX IF NOT EXISTS idx_army_lists_army_id ON army_lists(army_id);
        ",
    )?;

    if !column_exists(conn, "matches", "player1_army_list_id")? {
        conn.execute(
            "ALTER TABLE matches ADD COLUMN player1_army_list_id INTEGER REFERENCES army_lists(id)",
            [],
        )?;
    }
    if !column_exists(conn, "matches", "player2_army_list_id")? {
        conn.execute(
            "ALTER TABLE matches ADD COLUMN player2_army_list_id INTEGER REFERENCES army_lists(id)",
            [],
        )?;
    }
    if !column_exists(conn, "tournament_matches", "player1_army_list_id")? {
        conn.execute(
            "ALTER TABLE tournament_matches ADD COLUMN player1_army_list_id INTEGER REFERENCES army_lists(id)",
            [],
        )?;
    }
    if !column_exists(conn, "tournament_matches", "player2_army_list_id")? {
        conn.execute(
            "ALTER TABLE tournament_matches ADD COLUMN player2_army_list_id INTEGER REFERENCES army_lists(id)",
            [],
        )?;
    }
    if !column_exists(conn, "tournament_registrations", "army_list_1_id")? {
        conn.execute(
            "ALTER TABLE tournament_registrations ADD COLUMN army_list_1_id INTEGER REFERENCES army_lists(id)",
            [],
        )?;
    }
    if !column_exists(conn, "tournament_registrations", "army_list_2_id")? {
        conn.execute(
            "ALTER TABLE tournament_registrations ADD COLUMN army_list_2_id INTEGER REFERENCES army_lists(id)",
            [],
        )?;
    }
    if !column_exists(conn, "tournament_registrations", "bracket_list_1_id")? {
        conn.execute(
            "ALTER TABLE tournament_registrations ADD COLUMN bracket_list_1_id INTEGER REFERENCES army_lists(id)",
            [],
        )?;
    }
    if !column_exists(conn, "tournament_registrations", "bracket_list_2_id")? {
        conn.execute(
            "ALTER TABLE tournament_registrations ADD COLUMN bracket_list_2_id INTEGER REFERENCES army_lists(id)",
            [],
        )?;
    }

    crate::army_list_store::backfill_army_list_references(conn)?;

    if !column_exists(conn, "army_lists", "name")? {
        conn.execute("ALTER TABLE army_lists ADD COLUMN name TEXT", [])?;
    }
    crate::army_list_store::backfill_army_list_names(conn)?;

    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS tournament_scenarios (
            tournament_id INTEGER NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
            kind TEXT NOT NULL,
            slot TEXT NOT NULL,
            scenario_id INTEGER NOT NULL REFERENCES scenarios(id),
            PRIMARY KEY (tournament_id, kind, slot)
        );
        ",
    )?;

    conn.execute_batch(
        "
        UPDATE matches
        SET scenario_other = TRIM(SUBSTR(scenario_other, 5))
        WHERE scenario_other IS NOT NULL
          AND LENGTH(scenario_other) > 4
          AND SUBSTR(scenario_other, 2, 3) = ' : ';

        UPDATE tournament_matches
        SET scenario_other = TRIM(SUBSTR(scenario_other, 5))
        WHERE scenario_other IS NOT NULL
          AND LENGTH(scenario_other) > 4
          AND SUBSTR(scenario_other, 2, 3) = ' : ';

        UPDATE scenarios
        SET name = TRIM(SUBSTR(name, 5)),
            name_key = lower(trim(TRIM(SUBSTR(name, 5))))
        WHERE name IS NOT NULL
          AND LENGTH(name) > 4
          AND SUBSTR(name, 2, 3) = ' : ';
        ",
    )?;

    migrate_scenario_pack_schema(conn)?;

    backfill_matches_from_tournaments(conn)?;

    normalize_stored_army_list_urls(conn)?;

    conn.execute_batch(
        "
        DROP INDEX IF EXISTS idx_players_discord_id;

        CREATE UNIQUE INDEX IF NOT EXISTS idx_players_discord_username
            ON players(discord_username)
            WHERE discord_username IS NOT NULL;

        UPDATE users SET is_admin = 1 WHERE discord_id = '494953088180813825';
        ",
    )?;

    Ok(())
}

fn backfill_matches_from_tournaments(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        UPDATE matches
        SET recorded_at = COALESCE((
            SELECT COALESCE(tm.played_at, tm.confirmed_at, tm.submitted_at)
            FROM tournament_matches tm
            WHERE tm.status = 'confirmed'
              AND tm.is_forfeit = 0
              AND tm.is_unplayed = 0
              AND (
                (
                  lower(trim(matches.player1)) = lower(trim(tm.player1))
                  AND lower(trim(matches.player2)) = lower(trim(tm.player2))
                  AND matches.player1_objectives = tm.player1_objectives
                  AND matches.player2_objectives = tm.player2_objectives
                  AND matches.player1_survivors = tm.player1_survivors
                  AND matches.player2_survivors = tm.player2_survivors
                )
                OR (
                  lower(trim(matches.player1)) = lower(trim(tm.player2))
                  AND lower(trim(matches.player2)) = lower(trim(tm.player1))
                  AND matches.player1_objectives = tm.player2_objectives
                  AND matches.player2_objectives = tm.player1_objectives
                  AND matches.player1_survivors = tm.player2_survivors
                  AND matches.player2_survivors = tm.player1_survivors
                )
              )
            ORDER BY tm.confirmed_at DESC
            LIMIT 1
        ), recorded_at)
        WHERE recorded_at < 31536000;

        INSERT INTO matches (
            player1, player2, outcome,
            player1_old, player1_new, player2_old, player2_new,
            player1_objectives, player1_survivors,
            player2_objectives, player2_survivors,
            player1_army_id, player2_army_id,
            scenario_id, scenario_other,
            tournament_id, tournament_phase,
            recorded_at
        )
        SELECT
            tm.player1,
            tm.player2,
            tm.outcome,
            COALESCE(tm.player1_rating_used, 1200.0),
            COALESCE(tm.player1_rating_used, 1200.0) + tm.player1_elo_delta,
            COALESCE(tm.player2_rating_used, 1200.0),
            COALESCE(tm.player2_rating_used, 1200.0) + tm.player2_elo_delta,
            tm.player1_objectives,
            tm.player1_survivors,
            tm.player2_objectives,
            tm.player2_survivors,
            tm.player1_army_id,
            tm.player2_army_id,
            tm.scenario_id,
            tm.scenario_other,
            tm.tournament_id,
            tm.phase,
            COALESCE(tm.played_at, tm.confirmed_at, tm.submitted_at, t.completed_at, t.started_at, 0)
        FROM tournament_matches tm
        JOIN tournaments t ON t.id = tm.tournament_id
        WHERE tm.status = 'confirmed'
          AND tm.is_forfeit = 0
          AND tm.is_unplayed = 0
          AND tm.player1 IS NOT NULL
          AND tm.player2 IS NOT NULL
          AND tm.outcome IS NOT NULL
          AND NOT EXISTS (
            SELECT 1
            FROM matches m
            WHERE (
              lower(trim(m.player1)) = lower(trim(tm.player1))
              AND lower(trim(m.player2)) = lower(trim(tm.player2))
              AND m.player1_objectives = tm.player1_objectives
              AND m.player2_objectives = tm.player2_objectives
              AND m.player1_survivors = tm.player1_survivors
              AND m.player2_survivors = tm.player2_survivors
            )
            OR (
              lower(trim(m.player1)) = lower(trim(tm.player2))
              AND lower(trim(m.player2)) = lower(trim(tm.player1))
              AND m.player1_objectives = tm.player2_objectives
              AND m.player2_objectives = tm.player1_objectives
              AND m.player1_survivors = tm.player2_survivors
              AND m.player2_survivors = tm.player1_survivors
            )
          );
        ",
    )?;
    Ok(())
}

fn rename_scenario_name_to_other(conn: &Connection, table: &str) -> Result<()> {
    if column_exists(conn, table, "scenario_name")?
        && !column_exists(conn, table, "scenario_other")?
    {
        conn.execute(
            &format!("ALTER TABLE {table} RENAME COLUMN scenario_name TO scenario_other"),
            [],
        )?;
    } else if !column_exists(conn, table, "scenario_other")? {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN scenario_other TEXT"),
            [],
        )?;
    }
    Ok(())
}

fn migrate_scenario_pack_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS scenario_packs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            slug TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            version TEXT,
            preamble_md TEXT NOT NULL DEFAULT '',
            sort_order INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS common_rules (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            pack_id INTEGER NOT NULL REFERENCES scenario_packs(id),
            slug TEXT NOT NULL,
            name TEXT NOT NULL,
            body_md TEXT NOT NULL DEFAULT '',
            sort_order INTEGER NOT NULL DEFAULT 0,
            UNIQUE(pack_id, slug)
        );

        CREATE TABLE IF NOT EXISTS secondary_objectives (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            pack_id INTEGER NOT NULL REFERENCES scenario_packs(id),
            slug TEXT NOT NULL,
            name TEXT NOT NULL,
            body_md TEXT NOT NULL DEFAULT '',
            sort_order INTEGER NOT NULL DEFAULT 0,
            UNIQUE(pack_id, slug)
        );

        CREATE TABLE IF NOT EXISTS scenario_common_rules (
            scenario_id INTEGER NOT NULL REFERENCES scenarios(id) ON DELETE CASCADE,
            common_rule_id INTEGER NOT NULL REFERENCES common_rules(id) ON DELETE CASCADE,
            PRIMARY KEY (scenario_id, common_rule_id)
        );
        ",
    )?;

    let upgrading_scenarios = !column_exists(conn, "scenarios", "map_filename")?;

    for (column, ddl) in [
        ("pack_id", "ALTER TABLE scenarios ADD COLUMN pack_id INTEGER REFERENCES scenario_packs(id)"),
        ("slug", "ALTER TABLE scenarios ADD COLUMN slug TEXT"),
        ("map_filename", "ALTER TABLE scenarios ADD COLUMN map_filename TEXT"),
        ("flavor_text", "ALTER TABLE scenarios ADD COLUMN flavor_text TEXT"),
        ("end_condition_md", "ALTER TABLE scenarios ADD COLUMN end_condition_md TEXT"),
        ("objectives_md", "ALTER TABLE scenarios ADD COLUMN objectives_md TEXT"),
        ("deployment_notes_md", "ALTER TABLE scenarios ADD COLUMN deployment_notes_md TEXT"),
        ("exclusion_zones_md", "ALTER TABLE scenarios ADD COLUMN exclusion_zones_md TEXT"),
        ("elements_md", "ALTER TABLE scenarios ADD COLUMN elements_md TEXT"),
        ("special_rules_md", "ALTER TABLE scenarios ADD COLUMN special_rules_md TEXT"),
        ("sort_order", "ALTER TABLE scenarios ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0"),
    ] {
        if !column_exists(conn, "scenarios", column)? {
            conn.execute(ddl, [])?;
        }
    }

    if upgrading_scenarios {
        // Purge du catalogue hérité avant intégration du pack de scénarios.
        // Les libellés historiques restent dans scenario_other.
        conn.execute_batch(
            "
            UPDATE matches SET scenario_id = NULL;
            UPDATE tournament_matches SET scenario_id = NULL;
            DELETE FROM scenario_common_rules;
            DELETE FROM scenarios;
            ",
        )?;
    }

    crate::scenario_pack::seed_default_pack_if_needed(conn)?;
    crate::scenario_pack::sync_map_filenames(conn)?;
    crate::scenario_pack::sync_exclusion_rule_links(conn)?;

    Ok(())
}

fn migrate_match_reports(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS match_reports (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            match_id INTEGER NOT NULL,
            player_name TEXT NOT NULL,
            body_md TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            status TEXT NOT NULL DEFAULT 'published',
            published_at INTEGER,
            UNIQUE(match_id, player_name)
        );
        CREATE INDEX IF NOT EXISTS idx_match_reports_match_id
            ON match_reports(match_id);
        CREATE TABLE IF NOT EXISTS report_templates (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            body_md TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_report_templates_user
            ON report_templates(user_id);
        ",
    )?;

    if !column_exists(conn, "match_reports", "status")? {
        conn.execute(
            "ALTER TABLE match_reports ADD COLUMN status TEXT NOT NULL DEFAULT 'published'",
            [],
        )?;
    }
    if !column_exists(conn, "match_reports", "published_at")? {
        conn.execute(
            "ALTER TABLE match_reports ADD COLUMN published_at INTEGER",
            [],
        )?;
    }

    // Migration one-shot depuis les anciennes colonnes matches.player*_report_md.
    if column_exists(conn, "matches", "player1_report_md")? {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        conn.execute(
            "
            INSERT OR IGNORE INTO match_reports (match_id, player_name, body_md, created_at, updated_at)
            SELECT id, player1, player1_report_md, ?1, ?1
            FROM matches
            WHERE player1_report_md IS NOT NULL AND trim(player1_report_md) != ''
            ",
            rusqlite::params![now],
        )?;
        conn.execute(
            "
            INSERT OR IGNORE INTO match_reports (match_id, player_name, body_md, created_at, updated_at)
            SELECT id, player2, player2_report_md, ?1, ?1
            FROM matches
            WHERE player2_report_md IS NOT NULL AND trim(player2_report_md) != ''
            ",
            rusqlite::params![now],
        )?;
    }

    conn.execute(
        "
        UPDATE match_reports
        SET published_at = created_at
        WHERE published_at IS NULL AND status = 'published'
        ",
        [],
    )?;

    Ok(())
}

/// Retire le préfixe URL Army des codes déjà stockés (ne garde que le code).
fn normalize_stored_army_list_urls(conn: &Connection) -> Result<()> {
    use crate::army_list::normalize_army_list_code;

    let targets: &[(&str, &str)] = &[
        ("tournament_registrations", "army_list_1"),
        ("tournament_registrations", "army_list_2"),
        ("tournament_registrations", "bracket_list_1"),
        ("tournament_registrations", "bracket_list_2"),
        ("matches", "player1_army_list_code"),
        ("matches", "player2_army_list_code"),
        ("tournament_matches", "player1_army_list_code"),
        ("tournament_matches", "player2_army_list_code"),
    ];

    for &(table, column) in targets {
        if !column_exists(conn, table, column)? {
            continue;
        }
        let sql = format!(
            "SELECT rowid, {column} FROM {table}
             WHERE {column} IS NOT NULL
               AND ({column} LIKE '%army/list/%' OR {column} LIKE '%army/infinity/list/%')"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (rowid, raw) in rows {
            let Some(normalized) = normalize_army_list_code(&raw) else {
                continue;
            };
            if normalized == raw {
                continue;
            }
            conn.execute(
                &format!("UPDATE {table} SET {column} = ?1 WHERE rowid = ?2"),
                rusqlite::params![normalized, rowid],
            )?;
        }
    }

    Ok(())
}

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

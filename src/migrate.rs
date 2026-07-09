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

        CREATE TABLE IF NOT EXISTS scenarios (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            name_key TEXT NOT NULL UNIQUE,
            usage_count INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS tournaments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
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
            scenario_name TEXT,
            player1_army_id INTEGER,
            player2_army_id INTEGER,
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

    if !column_exists(conn, "matches", "scenario_name")? {
        conn.execute("ALTER TABLE matches ADD COLUMN scenario_name TEXT", [])?;
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

    if !column_exists(conn, "tournament_matches", "is_unplayed")? {
        conn.execute(
            "ALTER TABLE tournament_matches ADD COLUMN is_unplayed INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }

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

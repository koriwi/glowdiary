pub mod goals;
pub mod meals;
pub mod users;

use rusqlite::Connection;

// ---------------------------------------------------------------------------
// Default nutrition goals — single source of truth for the application.
// ---------------------------------------------------------------------------

pub const DEFAULT_KCAL_TARGET: f64 = 2000.0;
pub const DEFAULT_FAT_G_TARGET: f64 = 65.0;
pub const DEFAULT_PROTEIN_G_TARGET: f64 = 75.0;
pub const DEFAULT_CARBS_G_TARGET: f64 = 275.0;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Current UTC timestamp in ISO 8601 format (e.g. "2026-05-13T12:30:00.123Z").
pub fn timestamp_now() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

// ---------------------------------------------------------------------------
// DB setup
// ---------------------------------------------------------------------------

/// Open (or create) the SQLite database and run migrations.
pub fn open(path: &str) -> anyhow::Result<Connection> {
    let conn = Connection::open(path)?;

    // Enable WAL mode for better concurrent access
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;

    run_migrations(&conn)?;

    Ok(conn)
}

fn run_migrations(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS users (
            uuid TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS goals (
            user_uuid TEXT PRIMARY KEY REFERENCES users(uuid),
            kcal_target REAL NOT NULL DEFAULT 2000,
            fat_g_target REAL NOT NULL DEFAULT 65,
            protein_g_target REAL NOT NULL DEFAULT 75,
            carbs_g_target REAL NOT NULL DEFAULT 275
        );

        CREATE TABLE IF NOT EXISTS meals (
            uuid TEXT PRIMARY KEY,
            user_uuid TEXT NOT NULL REFERENCES users(uuid),
            name TEXT NOT NULL,
            eaten_at TEXT NOT NULL,
            kcal REAL NOT NULL,
            fat_g REAL NOT NULL,
            protein_g REAL NOT NULL,
            carbs_g REAL NOT NULL,
            fddb_source TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE INDEX IF NOT EXISTS idx_meals_user_eaten
            ON meals(user_uuid, eaten_at);
        ",
    )?;

    Ok(())
}

use crate::error::{AppError, AppResult};
use rusqlite::Connection;
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct User {
    pub uuid: String,
    pub name: String,
    pub created_at: String,
}

/// Create a new user with a generated UUID and default goals.
pub fn create_user(conn: &Connection, name: &str) -> AppResult<User> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::InvalidInput("Name cannot be empty".into()));
    }

    let uuid = Uuid::now_v7().to_string();
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

    conn.execute(
        "INSERT INTO users (uuid, name, created_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![uuid, name, now],
    )?;

    // Insert default goals
    conn.execute(
        "INSERT INTO goals (user_uuid) VALUES (?1)",
        rusqlite::params![uuid],
    )?;

    Ok(User {
        uuid,
        name,
        created_at: now,
    })
}

/// Get a user by UUID.
pub fn get_user(conn: &Connection, uuid: &str) -> AppResult<Option<User>> {
    let mut stmt = conn.prepare(
        "SELECT uuid, name, created_at FROM users WHERE uuid = ?1",
    )?;

    let mut rows = stmt.query_map(rusqlite::params![uuid], |row| {
        Ok(User {
            uuid: row.get(0)?,
            name: row.get(1)?,
            created_at: row.get(2)?,
        })
    })?;

    match rows.next() {
        Some(Ok(user)) => Ok(Some(user)),
        Some(Err(e)) => Err(AppError::Database(e)),
        None => Ok(None),
    }
}

/// Check if a user exists (returns Ok if found, Err(UserNotFound) otherwise).
pub fn require_user(conn: &Connection, uuid: &str) -> AppResult<()> {
    let exists: bool =
        conn.query_row(
            "SELECT COUNT(*) FROM users WHERE uuid = ?1",
            rusqlite::params![uuid],
            |row| row.get::<_, i64>(0),
        )? > 0;

    if exists {
        Ok(())
    } else {
        Err(AppError::UserNotFound(uuid.to_string()))
    }
}

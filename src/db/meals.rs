use crate::error::{AppError, AppResult};
use rusqlite::Connection;
use serde::Serialize;
use uuid::Uuid;

use super::timestamp_now;

#[derive(Debug, Clone, Serialize)]
pub struct Meal {
    pub uuid: String,
    pub user_uuid: String,
    pub name: String,
    pub eaten_at: String,
    pub kcal: f64,
    pub fat_g: f64,
    pub protein_g: f64,
    pub carbs_g: f64,
    pub fddb_source: Option<String>,
    pub created_at: String,
}

/// Daily aggregated stats.
#[derive(Debug, Clone, Serialize)]
pub struct DailyStats {
    pub date: String,
    pub total_kcal: f64,
    pub total_fat_g: f64,
    pub total_protein_g: f64,
    pub total_carbs_g: f64,
    pub meal_count: u64,
}

/// Weekly aggregated stats with per-day breakdown.
#[derive(Debug, Clone, Serialize)]
pub struct WeeklyStats {
    pub week_start: String,  // ISO Monday
    pub week_end: String,    // ISO Sunday
    pub totals: DailyStats,
    pub daily_averages: DailyStats,
    pub per_day: Vec<DailyStats>,
}

// ---------------------------------------------------------------------------

/// Add a meal.
pub fn add_meal(
    conn: &Connection,
    user_uuid: &str,
    name: &str,
    eaten_at: &str,
    kcal: f64,
    fat_g: f64,
    protein_g: f64,
    carbs_g: f64,
    fddb_source: Option<String>,
) -> AppResult<Meal> {
    let uuid = Uuid::now_v7().to_string();
    let now = timestamp_now();

    conn.execute(
        "INSERT INTO meals (uuid, user_uuid, name, eaten_at, kcal, fat_g, protein_g, carbs_g, fddb_source, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![uuid, user_uuid, name, eaten_at, kcal, fat_g, protein_g, carbs_g, fddb_source, now],
    )?;

    Ok(Meal {
        uuid,
        user_uuid: user_uuid.to_string(),
        name: name.to_string(),
        eaten_at: eaten_at.to_string(),
        kcal,
        fat_g,
        protein_g,
        carbs_g,
        fddb_source,
        created_at: now,
    })
}

// ---------------------------------------------------------------------------

/// Get a single meal by UUID.
pub fn get_meal(conn: &Connection, uuid: &str) -> AppResult<Meal> {
    conn.query_row(
        "SELECT uuid, user_uuid, name, eaten_at, kcal, fat_g, protein_g, carbs_g, fddb_source, created_at
         FROM meals WHERE uuid = ?1",
        rusqlite::params![uuid],
        |row| {
            Ok(Meal {
                uuid: row.get(0)?,
                user_uuid: row.get(1)?,
                name: row.get(2)?,
                eaten_at: row.get(3)?,
                kcal: row.get(4)?,
                fat_g: row.get(5)?,
                protein_g: row.get(6)?,
                carbs_g: row.get(7)?,
                fddb_source: row.get(8)?,
                created_at: row.get(9)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            AppError::MealNotFound(uuid.to_string())
        }
        other => AppError::Database(other),
    })
}

// ---------------------------------------------------------------------------

/// Get all meals for a user on a specific day (ISO date string "2026-05-13").
/// Results sorted ascending by eaten_at.
pub fn get_meals_by_day(conn: &Connection, user_uuid: &str, date: &str) -> AppResult<Vec<Meal>> {
    // Match any timestamp on that date
    let start = format!("{}T00:00:00", date);
    let end = format!("{}T23:59:59", date);

    let mut stmt = conn.prepare(
        "SELECT uuid, user_uuid, name, eaten_at, kcal, fat_g, protein_g, carbs_g, fddb_source, created_at
         FROM meals
         WHERE user_uuid = ?1 AND eaten_at >= ?2 AND eaten_at <= ?3
         ORDER BY eaten_at ASC",
    )?;

    let rows = stmt.query_map(rusqlite::params![user_uuid, start, end], map_meal)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(AppError::Database)
}

// ---------------------------------------------------------------------------

/// Get all meals for a user in the ISO week containing `date`.
pub fn get_meals_by_week(conn: &Connection, user_uuid: &str, date: &str) -> AppResult<Vec<Meal>> {
    let (monday, sunday) = week_bounds(date)?;

    let start = format!("{}T00:00:00", monday.format("%Y-%m-%d"));
    let end = format!("{}T23:59:59", sunday.format("%Y-%m-%d"));

    let mut stmt = conn.prepare(
        "SELECT uuid, user_uuid, name, eaten_at, kcal, fat_g, protein_g, carbs_g, fddb_source, created_at
         FROM meals
         WHERE user_uuid = ?1 AND eaten_at >= ?2 AND eaten_at <= ?3
         ORDER BY eaten_at ASC",
    )?;

    let rows = stmt.query_map(rusqlite::params![user_uuid, start, end], map_meal)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(AppError::Database)
}

// ---------------------------------------------------------------------------

/// Delete a meal by UUID. Returns the deleted meal.
pub fn delete_meal(conn: &Connection, uuid: &str) -> AppResult<Meal> {
    let meal = get_meal(conn, uuid)?;
    conn.execute("DELETE FROM meals WHERE uuid = ?1", rusqlite::params![uuid])?;
    Ok(meal)
}

// ---------------------------------------------------------------------------

/// Compute daily stats for a user on a given date.
pub fn get_daily_stats(conn: &Connection, user_uuid: &str, date: &str) -> AppResult<DailyStats> {
    let meals = get_meals_by_day(conn, user_uuid, date)?;

    let total_kcal = meals.iter().map(|m| m.kcal).sum();
    let total_fat_g = meals.iter().map(|m| m.fat_g).sum();
    let total_protein_g = meals.iter().map(|m| m.protein_g).sum();
    let total_carbs_g = meals.iter().map(|m| m.carbs_g).sum();
    let meal_count = meals.len() as u64;

    Ok(DailyStats {
        date: date.to_string(),
        total_kcal,
        total_fat_g,
        total_protein_g,
        total_carbs_g,
        meal_count,
    })
}

// ---------------------------------------------------------------------------

/// Compute weekly stats for a user, with per-day breakdown.
pub fn get_weekly_stats(conn: &Connection, user_uuid: &str, date: &str) -> AppResult<WeeklyStats> {
    let (monday, sunday) = week_bounds(date)?;

    let mut per_day = Vec::with_capacity(7);
    let mut totals = DailyStats {
        date: format!("{} - {}", monday.format("%Y-%m-%d"), sunday.format("%Y-%m-%d")),
        total_kcal: 0.0,
        total_fat_g: 0.0,
        total_protein_g: 0.0,
        total_carbs_g: 0.0,
        meal_count: 0,
    };

    for i in 0..7 {
        let day = monday + chrono::Duration::days(i);
        let day_str = day.format("%Y-%m-%d").to_string();
        let ds = get_daily_stats(conn, user_uuid, &day_str)?;

        totals.total_kcal += ds.total_kcal;
        totals.total_fat_g += ds.total_fat_g;
        totals.total_protein_g += ds.total_protein_g;
        totals.total_carbs_g += ds.total_carbs_g;
        totals.meal_count += ds.meal_count;

        per_day.push(ds);
    }

    let day_count = per_day.iter().filter(|d| d.meal_count > 0).count().max(1) as f64;

    let daily_averages = DailyStats {
        date: "daily_average".to_string(),
        total_kcal: totals.total_kcal / day_count,
        total_fat_g: totals.total_fat_g / day_count,
        total_protein_g: totals.total_protein_g / day_count,
        total_carbs_g: totals.total_carbs_g / day_count,
        meal_count: (totals.meal_count as f64 / day_count).round() as u64,
    };

    Ok(WeeklyStats {
        week_start: monday.format("%Y-%m-%d").to_string(),
        week_end: sunday.format("%Y-%m-%d").to_string(),
        totals,
        daily_averages,
        per_day,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return (monday, sunday) for the ISO week containing `date`.
fn week_bounds(date: &str) -> AppResult<(chrono::NaiveDate, chrono::NaiveDate)> {
    let parsed = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|e| AppError::InvalidInput(format!("Invalid date '{date}': {e}")))?;

    let weekday: u32 = parsed
        .format("%u")
        .to_string()
        .parse()
        .map_err(|_| AppError::InvalidInput(format!("Failed to determine weekday for '{date}'")))?;

    let days_from_monday = weekday.saturating_sub(1);
    let monday = parsed - chrono::Duration::days(days_from_monday as i64);
    let sunday = monday + chrono::Duration::days(6);

    Ok((monday, sunday))
}

fn map_meal(row: &rusqlite::Row<'_>) -> rusqlite::Result<Meal> {
    Ok(Meal {
        uuid: row.get(0)?,
        user_uuid: row.get(1)?,
        name: row.get(2)?,
        eaten_at: row.get(3)?,
        kcal: row.get(4)?,
        fat_g: row.get(5)?,
        protein_g: row.get(6)?,
        carbs_g: row.get(7)?,
        fddb_source: row.get(8)?,
        created_at: row.get(9)?,
    })
}

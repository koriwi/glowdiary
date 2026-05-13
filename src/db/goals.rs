use crate::error::{AppError, AppResult};
use rusqlite::Connection;
use serde::Serialize;

use super::{DEFAULT_CARBS_G_TARGET, DEFAULT_FAT_G_TARGET, DEFAULT_KCAL_TARGET, DEFAULT_PROTEIN_G_TARGET};

#[derive(Debug, Clone, Serialize)]
pub struct Goals {
    pub kcal_target: f64,
    pub fat_g_target: f64,
    pub protein_g_target: f64,
    pub carbs_g_target: f64,
}

/// Get goals for a user. Returns the default fallback if no custom goals exist.
pub fn get_goals(conn: &Connection, user_uuid: &str) -> AppResult<Goals> {
    let result = conn.query_row(
        "SELECT kcal_target, fat_g_target, protein_g_target, carbs_g_target
         FROM goals WHERE user_uuid = ?1",
        rusqlite::params![user_uuid],
        |row| {
            Ok(Goals {
                kcal_target: row.get(0)?,
                fat_g_target: row.get(1)?,
                protein_g_target: row.get(2)?,
                carbs_g_target: row.get(3)?,
            })
        },
    );

    match result {
        Ok(goals) => Ok(goals),
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            // No goals row exists (shouldn't happen if user was created properly),
            // return sensible defaults
            Ok(Goals {
                kcal_target: DEFAULT_KCAL_TARGET,
                fat_g_target: DEFAULT_FAT_G_TARGET,
                protein_g_target: DEFAULT_PROTEIN_G_TARGET,
                carbs_g_target: DEFAULT_CARBS_G_TARGET,
            })
        }
        Err(e) => Err(AppError::Database(e)),
    }
}

/// Update goals for a user. Returns the updated goals.
pub fn set_goals(
    conn: &Connection,
    user_uuid: &str,
    kcal_target: f64,
    fat_g_target: f64,
    protein_g_target: f64,
    carbs_g_target: f64,
) -> AppResult<Goals> {
    if kcal_target <= 0.0 {
        return Err(AppError::InvalidInput("kcal_target must be > 0".into()));
    }
    if fat_g_target < 0.0 || protein_g_target < 0.0 || carbs_g_target < 0.0 {
        return Err(AppError::InvalidInput(
            "Target values must be >= 0".into(),
        ));
    }

    let affected = conn.execute(
        "UPDATE goals SET kcal_target = ?1, fat_g_target = ?2,
         protein_g_target = ?3, carbs_g_target = ?4
         WHERE user_uuid = ?5",
        rusqlite::params![kcal_target, fat_g_target, protein_g_target, carbs_g_target, user_uuid],
    )?;

    if affected == 0 {
        // No goals row yet — insert one
        conn.execute(
            "INSERT INTO goals (user_uuid, kcal_target, fat_g_target, protein_g_target, carbs_g_target)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![user_uuid, kcal_target, fat_g_target, protein_g_target, carbs_g_target],
        )?;
    }

    Ok(Goals {
        kcal_target,
        fat_g_target,
        protein_g_target,
        carbs_g_target,
    })
}

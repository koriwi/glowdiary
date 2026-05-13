use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("User not found: {0}")]
    UserNotFound(String),

    #[error("Meal not found: {0}")]
    MealNotFound(String),

    #[error("Open Food Facts API error: {0}")]
    FoodApi(String),

    #[error("HTTP request failed: {0}")]
    Http(#[from] ureq::Error),

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

pub type AppResult<T> = Result<T, AppError>;

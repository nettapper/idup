use sqlx::SqlitePool;

/// Application state shared across all handlers.
pub type AppState = SqlitePool;

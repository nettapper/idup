use axum::extract::State;
use axum::response::Html;
use sqlx::SqlitePool;
use askama::Template;

use crate::error::AppError;

#[derive(Template)]
#[template(path = "fragments/clean_result.html")]
struct CleanResultTemplate;

pub async fn clean(State(pool): State<SqlitePool>) -> Result<Html<String>, Html<String>> {
    match idup::db::wipe_db(&pool).await {
        Ok(()) => {
            let tmpl = CleanResultTemplate;
            tmpl.render()
                .map(Html)
                .map_err(|e| AppError::from(e).into_fragment())
        }
        Err(e) => Err(AppError::from(e).into_fragment()),
    }
}

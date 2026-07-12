use axum::extract::State;
use axum::response::Html;
use axum::Form;
use serde::Deserialize;
use sqlx::SqlitePool;
use std::path::PathBuf;
use askama::Template;

use crate::error::AppError;

#[derive(Deserialize)]
pub struct UpdateForm {
    path: Option<String>,
    cleanup: Option<String>,
}

#[derive(Template)]
#[template(path = "fragments/update_result.html")]
struct UpdateResultTemplate {
    verified: usize,
    updated: usize,
    missing: usize,
    total: usize,
    elapsed_secs: String,
    show_cleaned: bool,
    cleaned: usize,
}

pub async fn update(
    State(pool): State<SqlitePool>,
    Form(form): Form<UpdateForm>,
) -> Result<Html<String>, Html<String>> {
    let path = form.path.filter(|p| !p.trim().is_empty()).map(PathBuf::from);
    let cleanup = form.cleanup.is_some();

    let stats = idup::update::process_update(path, cleanup, &pool).await;

    let tmpl = UpdateResultTemplate {
        verified: stats.verified,
        updated: stats.updated,
        missing: stats.missing,
        total: stats.total,
        elapsed_secs: format!("{:.2}", stats.elapsed_secs),
        show_cleaned: cleanup,
        cleaned: stats.cleaned,
    };

    tmpl.render()
        .map(Html)
        .map_err(|e| AppError::from(e).into_fragment())
}

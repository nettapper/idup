use axum::extract::State;
use axum::response::Html;
use axum::Form;
use serde::Deserialize;
use sqlx::SqlitePool;
use std::path::PathBuf;
use askama::Template;
use crate::error::AppError;

#[derive(Deserialize)]
pub struct ScanForm {
    path: String,
    recursive: Option<String>,
    unzip: Option<String>,
    remove_archive: Option<String>,
    exact: Option<String>,
    all: Option<String>,
    phash: Option<String>,
    rotations: Option<String>,
    flips: Option<String>,
}

#[derive(Template)]
#[template(path = "fragments/scan_result.html")]
struct ScanResultTemplate {
    path: String,
    suffix: String,
    processed: usize,
    elapsed_secs: String,
}

pub async fn scan(
    State(pool): State<SqlitePool>,
    Form(form): Form<ScanForm>,
) -> Result<Html<String>, Html<String>> {
    let path = PathBuf::from(&form.path);
    let recursive = form.recursive.is_some();

    let mut opts = if form.all.is_some() {
        idup::scan::ScanOptions::all()
    } else if form.exact.is_some() {
        idup::scan::ScanOptions::exact()
    } else {
        idup::scan::ScanOptions {
            rotations: form.rotations.is_some(),
            flips: form.flips.is_some(),
            phash: form.phash.is_some(),
            unzip: false,
            remove_archive: false,
        }
    };
    opts.unzip = form.unzip.is_some();
    opts.remove_archive = form.remove_archive.is_some();

    let stats = idup::scan::process_path(path, recursive, &opts, &pool).await;

    let mut notes: Vec<&str> = Vec::new();
    if recursive { notes.push("recursive"); }
    if opts.unzip { notes.push("unzip"); }
    if opts.remove_archive { notes.push("remove-archive"); }
    if opts.rotations { notes.push("rotations"); }
    if opts.flips { notes.push("flips"); }
    if opts.phash { notes.push("phash"); }
    let suffix = if notes.is_empty() {
        String::new()
    } else {
        format!(" ({})", notes.join(", "))
    };

    let tmpl = ScanResultTemplate {
        path: form.path,
        suffix,
        processed: stats.processed,
        elapsed_secs: format!("{:.2}", stats.elapsed_secs),
    };

    tmpl.render()
        .map(Html)
        .map_err(|e| AppError::from(e).into_fragment())
}

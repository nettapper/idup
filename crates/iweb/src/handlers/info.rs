use axum::extract::Query;
use axum::response::Html;
use serde::Deserialize;
use std::path::PathBuf;
use askama::Template;

use crate::error::AppError;

#[derive(Deserialize)]
pub struct InfoQuery {
    file: String,
}

#[derive(Template)]
#[template(path = "fragments/info_result.html")]
struct InfoResultTemplate {
    phash: Option<String>,
    phash_error: Option<String>,
    sha256: Option<String>,
    sha256_error: Option<String>,
}

pub async fn info(Query(params): Query<InfoQuery>) -> Result<Html<String>, Html<String>> {
    let path = PathBuf::from(&params.file);

    let (phash, phash_error) = match idup::hash::phash::hash_path(&path) {
        Ok(ph) => (Some(ph.hash), None),
        Err(e) => (None, Some(e.to_string())),
    };

    let (sha256, sha256_error) = match idup::hash::sha256::hash_path(&path) {
        Ok(sh) => (Some(sh.hash), None),
        Err(e) => (None, Some(e.to_string())),
    };

    let tmpl = InfoResultTemplate {
        phash,
        phash_error,
        sha256,
        sha256_error,
    };

    tmpl.render()
        .map(Html)
        .map_err(|e| AppError::from(e).into_fragment())
}

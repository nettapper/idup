use axum::extract::{Query, State};
use axum::response::Html;
use serde::Deserialize;
use sqlx::SqlitePool;
use std::path::Path;
use askama::Template;

use crate::error::AppError;
use crate::services;
use super::url_encode;

#[derive(Deserialize)]
pub struct ExploreQuery {
    dir: Option<String>,
    filter: Option<String>,
    hash: Option<String>,
    seed: Option<u64>,
    n: Option<u32>,
}

pub struct Breadcrumb {
    pub label: String,
    pub href: Option<String>,
}

pub struct SubdirEntry {
    pub name: String,
    pub href: String,
    pub full_path: String,
}

pub struct ImageEntry {
    pub path: String,
    pub filename: String,
    pub parent_dir: String,
    pub img_src: String,
}

#[derive(Template)]
#[template(path = "explore.html")]
struct ExploreTemplate {
    title: String,
    dir_value: String,
    filter_value: String,
    breadcrumbs: Vec<Breadcrumb>,
    show_breadcrumbs: bool,
    subdirs: Vec<SubdirEntry>,
    images: Vec<ImageEntry>,
    show_headers: bool,
    empty_message: Option<String>,
}

pub async fn explore(
    State(pool): State<SqlitePool>,
    Query(params): Query<ExploreQuery>,
) -> Result<Html<String>, Html<String>> {
    let dir = params.dir.as_deref().filter(|s| !s.trim().is_empty());
    let filter = params.filter.as_deref().filter(|s| !s.trim().is_empty());
    let hash = params.hash.as_deref().filter(|s| !s.trim().is_empty());
    let seed = params.seed;
    let n = params.n.unwrap_or(20);

    let data = services::explore::fetch_explore_data(&pool, dir, filter, hash, seed, n)
        .await
        .map_err(|e| e.into_page())?;

    let dir_value = data.dir.as_deref().unwrap_or("").to_string();
    let filter_value = data.filter.as_deref().unwrap_or("").to_string();

    // Build breadcrumbs
    let mut breadcrumbs = Vec::new();
    let mut show_breadcrumbs = false;
    if let Some(ref d) = data.dir {
        show_breadcrumbs = true;
        let parts: Vec<&str> = d.split('/').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            breadcrumbs.push(Breadcrumb {
                label: "root".to_string(),
                href: None,
            });
        } else {
            breadcrumbs.push(Breadcrumb {
                label: "root".to_string(),
                href: Some("/explore".to_string()),
            });
            let mut cumulative = String::new();
            for (i, part) in parts.iter().enumerate() {
                cumulative.push('/');
                cumulative.push_str(part);
                if i + 1 == parts.len() {
                    breadcrumbs.push(Breadcrumb {
                        label: part.to_string(),
                        href: None,
                    });
                } else {
                    breadcrumbs.push(Breadcrumb {
                        label: part.to_string(),
                        href: Some(format!("/explore?dir={}", url_encode(&cumulative))),
                    });
                }
            }
        }
    }

    let show_headers = !data.subdirs.is_empty() && !data.paths.is_empty();
    let empty_message = if data.subdirs.is_empty() && data.paths.is_empty() {
        if params.dir.is_none() && params.filter.is_none() {
            Some("Enter a directory path or glob filter above to start exploring.".to_string())
        } else {
            Some("No images found.".to_string())
        }
    } else {
        None
    };

    let subdirs = data.subdirs.iter().map(|d| {
        let name = Path::new(d)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| d.clone());
        let href = format!("/explore?dir={}", url_encode(d));
        SubdirEntry {
            name,
            href,
            full_path: d.clone(),
        }
    }).collect::<Vec<_>>();

    let images = data.paths.iter().map(|p| {
        let filename = Path::new(p)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| p.clone());
        let parent_dir = Path::new(p)
            .parent()
            .map(|d| d.to_string_lossy().to_string())
            .unwrap_or_default();
        let img_src = format!("/api/image?path={}", url_encode(p));
        ImageEntry {
            path: p.clone(),
            filename,
            parent_dir,
            img_src,
        }
    }).collect::<Vec<_>>();

    let tmpl = ExploreTemplate {
        title: data.title,
        dir_value,
        filter_value,
        breadcrumbs,
        show_breadcrumbs,
        subdirs,
        images,
        show_headers,
        empty_message,
    };

    let rendered = tmpl.render().map_err(|e| AppError::from(e).into_page())?;
    Ok(Html(rendered))
}

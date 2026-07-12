use axum::extract::{Query, State};
use axum::response::Html;
use serde::Deserialize;
use sqlx::SqlitePool;
use askama::Template;

use crate::error::AppError;
use super::explore_href_seeded;

#[derive(Deserialize)]
pub struct RandomQuery {
    n: Option<u32>,
    filter: Option<String>,
}

#[derive(Template)]
#[template(path = "fragments/empty.html")]
struct EmptyTemplate {
    message: String,
}

#[derive(Template)]
#[template(path = "fragments/random_result.html")]
struct RandomResultTemplate {
    explore_href: String,
    count: usize,
    paths: Vec<String>,
}

pub async fn random(
    State(pool): State<SqlitePool>,
    Query(params): Query<RandomQuery>,
) -> Result<Html<String>, Html<String>> {
    let n = params.n.unwrap_or(20);
    let filter = params.filter.as_deref().filter(|s| !s.trim().is_empty());

    // Generate a seed so /explore can reproduce this exact random selection.
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(42);

    match idup::db::random_images_seeded(&pool, n, filter, seed).await {
        Err(e) => Err(AppError::from(e).into_fragment()),
        Ok(data) if data.is_empty() => {
            let tmpl = EmptyTemplate {
                message: "No images in db.".to_string(),
            };
            tmpl.render()
                .map(Html)
                .map_err(|e| AppError::from(e).into_fragment())
        }
        Ok(data) => {
            let paths: Vec<String> = data.iter().map(|d| d.path.clone()).collect();
            let explore_href = explore_href_seeded(seed, n, filter);
            let count = paths.len();

            let tmpl = RandomResultTemplate {
                explore_href,
                count,
                paths,
            };

            tmpl.render()
                .map(Html)
                .map_err(|e| AppError::from(e).into_fragment())
        }
    }
}

use axum::extract::{Query, State};
use axum::response::Html;
use serde::Deserialize;
use sqlx::SqlitePool;
use std::path::PathBuf;
use askama::Template;

use crate::error::AppError;
use super::{url_encode, explore_href_for_hash};

#[derive(Deserialize)]
pub struct ListQuery {
    path: Option<String>,
}

#[derive(Template)]
#[template(path = "fragments/empty.html")]
struct EmptyTemplate {
    message: String,
}

struct DupItem {
    encoded_path: String,
    display_path: String,
}

struct DupGroup {
    num: usize,
    explore_href: String,
    items: Vec<DupItem>,
}

#[derive(Template)]
#[template(path = "fragments/list_groups.html")]
struct ListGroupsTemplate {
    groups: Vec<DupGroup>,
}

#[derive(Template)]
#[template(path = "fragments/list_single.html")]
struct ListSingleTemplate {
    items: Vec<DupItem>,
}

pub async fn list(
    State(pool): State<SqlitePool>,
    Query(params): Query<ListQuery>,
) -> Result<Html<String>, Html<String>> {
    let path = params.path.filter(|p| !p.trim().is_empty());

    match path {
        None => match idup::db::exact_matches_grouped(&pool).await {
            Err(e) => Err(AppError::from(e).into_fragment()),
            Ok(data) if data.is_empty() => {
                let tmpl = EmptyTemplate {
                    message: "No duplicates found.".to_string(),
                };
                tmpl.render()
                    .map(Html)
                    .map_err(|e| AppError::from(e).into_fragment())
            }
            Ok(data) => {
                let mut groups = Vec::new();
                let mut current_hash = String::new();
                let mut group_num = 0usize;
                let mut current_items = Vec::new();

                for item in &data {
                    if item.group_hash != current_hash {
                        if group_num > 0 {
                            groups.push(DupGroup {
                                num: group_num,
                                explore_href: explore_href_for_hash(&current_hash),
                                items: current_items,
                            });
                        }
                        group_num += 1;
                        current_hash = item.group_hash.clone();
                        current_items = Vec::new();
                    }
                    current_items.push(DupItem {
                        encoded_path: url_encode(&item.path),
                        display_path: item.path.clone(),
                    });
                }
                if group_num > 0 {
                    groups.push(DupGroup {
                        num: group_num,
                        explore_href: explore_href_for_hash(&current_hash),
                        items: current_items,
                    });
                }

                let tmpl = ListGroupsTemplate { groups };
                tmpl.render()
                    .map(Html)
                    .map_err(|e| AppError::from(e).into_fragment())
            }
        },

        Some(path_str) => {
            let path = PathBuf::from(&path_str);
            match idup::db::exact_match(&pool, &path).await {
                Err(e) => Err(AppError::from(e).into_fragment()),
                Ok(data) if data.is_empty() => {
                    let tmpl = EmptyTemplate {
                        message: "No duplicates found.".to_string(),
                    };
                    tmpl.render()
                        .map(Html)
                        .map_err(|e| AppError::from(e).into_fragment())
                }
                Ok(data) => {
                    let items = data.iter().map(|item| {
                        DupItem {
                            encoded_path: url_encode(&item.path),
                            display_path: item.path.clone(),
                        }
                    }).collect::<Vec<_>>();

                    let tmpl = ListSingleTemplate { items };
                    tmpl.render()
                        .map(Html)
                        .map_err(|e| AppError::from(e).into_fragment())
                }
            }
        }
    }
}

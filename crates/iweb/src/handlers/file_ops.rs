use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::services;

#[derive(Deserialize)]
pub struct DeleteRequest {
    paths: Vec<String>,
}

#[derive(Serialize)]
pub struct DeleteResponse {
    success: bool,
    message: String,
    deleted_count: usize,
}

#[derive(Deserialize)]
pub struct MoveRequest {
    paths: Vec<String>,
    destination_dir: String,
    auto_create: bool,
    collision_behavior: String, // "rename", "overwrite", "skip"
}

#[derive(Serialize)]
pub struct MoveResponse {
    success: bool,
    message: String,
    moved_count: usize,
    skipped_count: usize,
    errors: Vec<String>,
}

pub async fn explore_delete(
    State(pool): State<SqlitePool>,
    Json(req): Json<DeleteRequest>,
) -> Json<DeleteResponse> {
    let result = services::file_ops::delete_images(&pool, &req.paths).await;

    let success = result.errors.is_empty();
    let message = if success {
        format!("Successfully deleted {} file(s).", result.deleted_count)
    } else {
        format!(
            "Deleted {} file(s) with errors:\n{}",
            result.deleted_count,
            result.errors.join("\n")
        )
    };

    Json(DeleteResponse {
        success,
        message,
        deleted_count: result.deleted_count,
    })
}

pub async fn explore_move(
    State(pool): State<SqlitePool>,
    Json(req): Json<MoveRequest>,
) -> Json<MoveResponse> {
    let params = services::file_ops::MoveParams {
        paths: req.paths,
        destination_dir: req.destination_dir,
        auto_create: req.auto_create,
        collision_behavior: req.collision_behavior,
    };

    match services::file_ops::move_images(&pool, params).await {
        Ok(result) => {
            let success = result.errors.is_empty();
            let message = if success {
                format!(
                    "Successfully moved {} file(s) ({} skipped).",
                    result.moved_count, result.skipped_count
                )
            } else {
                format!(
                    "Moved {} file(s) ({} skipped) with errors:\n{}",
                    result.moved_count,
                    result.skipped_count,
                    result.errors.join("\n")
                )
            };
            Json(MoveResponse {
                success,
                message,
                moved_count: result.moved_count,
                skipped_count: result.skipped_count,
                errors: result.errors,
            })
        }
        Err(e) => {
            let message = e.message();
            Json(MoveResponse {
                success: false,
                message,
                moved_count: 0,
                skipped_count: 0,
                errors: vec!["See message".to_string()],
            })
        }
    }
}

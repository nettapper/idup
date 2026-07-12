use crate::error::AppError;
use sqlx::SqlitePool;
use std::path::Path;

pub struct DeleteResult {
    pub deleted_count: usize,
    pub errors: Vec<String>,
}

pub struct MoveParams {
    pub paths: Vec<String>,
    pub destination_dir: String,
    pub auto_create: bool,
    pub collision_behavior: String,
}

pub struct MoveResult {
    pub moved_count: usize,
    pub skipped_count: usize,
    pub errors: Vec<String>,
}

pub async fn delete_images(pool: &SqlitePool, paths: &[String]) -> DeleteResult {
    let mut deleted_count = 0;
    let mut errors = Vec::new();

    for path_str in paths {
        let path = Path::new(path_str);
        if path.exists() {
            if let Err(e) = tokio::fs::remove_file(path).await {
                errors.push(format!("Failed to delete file {}: {}", path_str, e));
                continue;
            }
        }
        if let Err(e) = idup::db::delete_image(pool, path_str).await {
            errors.push(format!("Failed to remove from DB {}: {}", path_str, e));
        } else {
            deleted_count += 1;
        }
    }

    DeleteResult {
        deleted_count,
        errors,
    }
}

pub async fn move_images(
    pool: &SqlitePool,
    params: MoveParams,
) -> Result<MoveResult, AppError> {
    let dest_dir = Path::new(&params.destination_dir);

    if !dest_dir.exists() {
        if params.auto_create {
            if let Err(e) = tokio::fs::create_dir_all(dest_dir).await {
                return Err(AppError::Internal(format!(
                    "Failed to create destination directory: {}",
                    e
                )));
            }
        } else {
            return Err(AppError::NotFound(format!(
                "Destination directory does not exist: {}",
                params.destination_dir
            )));
        }
    } else if !dest_dir.is_dir() {
        return Err(AppError::Internal(format!(
            "Destination path is not a directory: {}",
            params.destination_dir
        )));
    }

    let mut moved_count = 0;
    let mut skipped_count = 0;
    let mut errors = Vec::new();

    for path_str in &params.paths {
        let old_path = Path::new(path_str);
        if !old_path.exists() {
            errors.push(format!("Source file does not exist: {}", path_str));
            continue;
        }

        let filename = match old_path.file_name() {
            Some(name) => name,
            None => {
                errors.push(format!("Invalid source path: {}", path_str));
                continue;
            }
        };

        let mut dest_path = dest_dir.join(filename);

        if dest_path.exists() {
            match params.collision_behavior.as_str() {
                "skip" => {
                    skipped_count += 1;
                    continue;
                }
                "overwrite" => {}
                "rename" => {
                    let stem = dest_path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("file");
                    let ext = dest_path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");
                    let mut counter = 1;
                    loop {
                        let new_name = if ext.is_empty() {
                            format!("{}_{}", stem, counter)
                        } else {
                            format!("{}_{}.{}", stem, counter, ext)
                        };
                        let candidate = dest_dir.join(new_name);
                        if !candidate.exists() {
                            dest_path = candidate;
                            break;
                        }
                        counter += 1;
                    }
                }
                _ => {
                    errors.push(format!(
                        "Unknown collision behavior: {}",
                        params.collision_behavior
                    ));
                    continue;
                }
            }
        }

        let dest_path_str = dest_path.to_string_lossy().to_string();

        if let Err(e) = tokio::fs::rename(old_path, &dest_path).await {
            errors.push(format!(
                "Failed to move file on disk from {} to {}: {}",
                path_str, dest_path_str, e
            ));
            continue;
        }

        if let Err(e) = idup::db::rename_image_path(pool, path_str, &dest_path_str).await {
            errors.push(format!(
                "Failed to update database for {} -> {}: {}",
                path_str, dest_path_str, e
            ));
        } else {
            moved_count += 1;
        }
    }

    Ok(MoveResult {
        moved_count,
        skipped_count,
        errors,
    })
}

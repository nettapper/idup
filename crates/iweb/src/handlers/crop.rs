use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::Path;

use crate::services;

#[derive(Deserialize)]
pub struct CropRect {
    x: i32,
    y: i32,
    w: u32,
    h: u32,
}

#[derive(Deserialize)]
pub struct CropRequest {
    input_path: String,
    overwrite: bool,
    new_filename: Option<String>,
    rotate_deg: Option<f64>,
    crop_rect: Option<CropRect>,
}

#[derive(Serialize)]
pub struct CropResponse {
    success: bool,
    message: String,
    new_path: Option<String>,
    img_src: Option<String>,
}

pub async fn crop(
    State(pool): State<SqlitePool>,
    Json(req): Json<CropRequest>,
) -> Json<CropResponse> {
    let params = services::crop::CropParams {
        input_path: req.input_path.clone(),
        overwrite: req.overwrite,
        new_filename: req.new_filename,
        rotate_deg: req.rotate_deg,
        crop_rect: req.crop_rect.map(|r| (r.x, r.y, r.w, r.h)),
    };

    match services::crop::process_crop(&pool, params).await {
        Ok(result) => {
            let message = if result.overwritten {
                "Image cropped and updated in-place.".to_string()
            } else {
                let filename = Path::new(&result.out_path)
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or("");
                format!("Image cropped and saved as new file: {}", filename)
            };
            Json(CropResponse {
                success: true,
                message,
                new_path: Some(result.out_path),
                img_src: Some(result.img_src),
            })
        }
        Err(e) => Json(CropResponse {
            success: false,
            message: e.message(),
            new_path: None,
            img_src: None,
        }),
    }
}

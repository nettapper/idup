use crate::error::AppError;
use sqlx::SqlitePool;
use std::path::Path;
use crate::handlers::url_encode;

pub struct CropParams {
    pub input_path: String,
    pub overwrite: bool,
    pub new_filename: Option<String>,
    pub rotate_deg: Option<f64>,
    pub crop_rect: Option<(i32, i32, u32, u32)>,
}

pub struct CropResult {
    pub out_path: String,
    pub img_src: String,
    pub overwritten: bool,
}

pub async fn process_crop(
    pool: &SqlitePool,
    params: CropParams,
) -> Result<CropResult, AppError> {
    let in_path = Path::new(&params.input_path);
    if !in_path.exists() {
        return Err(AppError::NotFound(format!(
            "Input file does not exist: {}",
            params.input_path
        )));
    }

    let out_path = if params.overwrite {
        in_path.to_path_buf()
    } else {
        let parent = in_path.parent().unwrap_or_else(|| Path::new("."));
        let name = match &params.new_filename {
            Some(n) if !n.trim().is_empty() => n.trim().to_string(),
            _ => {
                let stem = in_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("image");
                let ext = in_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("png");
                format!("{}_cropped.{}", stem, ext)
            }
        };
        parent.join(name)
    };

    if let Err(err) =
        icrop::rotate_and_crop(in_path, &out_path, params.crop_rect, params.rotate_deg)
    {
        return Err(AppError::Internal(format!(
            "Image processing failed: {}",
            err
        )));
    }

    // Immediately scan/hash the output path to update the database
    let scan_opts = idup::scan::ScanOptions::all();
    idup::scan::process_path(out_path.clone(), false, &scan_opts, pool).await;

    let out_path_str = out_path.to_string_lossy().into_owned();
    let img_src = format!("/api/image?path={}", url_encode(&out_path_str));

    Ok(CropResult {
        out_path: out_path_str,
        img_src,
        overwritten: params.overwrite,
    })
}

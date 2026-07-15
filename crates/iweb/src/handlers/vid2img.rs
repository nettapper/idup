use axum::extract::{Multipart, State};
use axum::response::Html;
use axum::Json;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tokio::io::AsyncWriteExt;
use askama::Template;

use crate::error::AppError;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FrameMetadata {
    pub filename: String,
    pub path: String,
    pub timestamp: f64,
    pub start_time: f64,
    pub end_time: f64,
}

impl FrameMetadata {
    pub fn formatted_time(&self) -> String {
        let secs = self.timestamp;
        let h = (secs / 3600.0).floor() as u32;
        let m = ((secs % 3600.0) / 60.0).floor() as u32;
        let s = (secs % 60.0).floor() as u32;
        let ms = ((secs - secs.floor()) * 1000.0).round() as u32;
        format!("{:02}:{:02}:{:02}.{:03}", h, m, s, ms)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VideoMetadata {
    pub video_id: String,
    pub video_name: String,
    pub video_path: String,
    pub destination_dir: String,
    pub extract_dir: String,
    pub frames: Vec<FrameMetadata>,
    pub last_interval: f64,
}

#[derive(Template)]
#[template(path = "fragments/vid2img_list.html")]
struct Vid2ImgListTemplate {
    reviews: Vec<VideoMetadata>,
}

// ── GET /api/vid2img/list ───────────────────────────────────────────────────
pub async fn list() -> Result<Html<String>, Html<String>> {
    let reviews = list_active_reviews().await.map_err(|e| e.into_fragment())?;
    let tmpl = Vid2ImgListTemplate { reviews };
    tmpl.render()
        .map(Html)
        .map_err(|e| AppError::from(e).into_fragment())
}

// ── POST /api/vid2img/upload ────────────────────────────────────────────────
pub async fn upload(mut multipart: Multipart) -> Result<Html<String>, Html<String>> {
    let mut interval = 5.0;
    let mut destination_dir = String::new();
    let mut uploaded_videos = Vec::new();

    while let Some(mut field) = multipart.next_field().await.map_err(|e: axum::extract::multipart::MultipartError| AppError::Internal(e.to_string()).into_fragment())? {
        let name = field.name().unwrap_or("").to_string();
        if name == "interval" {
            let text = field.text().await.unwrap_or_default();
            if let Ok(val) = text.parse::<f64>() {
                interval = val;
            }
        } else if name == "destination_dir" {
            destination_dir = field.text().await.unwrap_or_default().trim().to_string();
        } else if name == "videos" {
            let file_name = field.file_name().unwrap_or("video.mp4").to_string();
            if file_name.is_empty() {
                continue;
            }
            
            // Generate a unique ID for this upload
            let now = SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let video_id = format!("{}_{}", now, uploaded_videos.len());
            
            let video_dir = PathBuf::from(format!("/tmp/nettapper/vid2img/videos/{}", video_id));
            tokio::fs::create_dir_all(&video_dir).await.map_err(|e: std::io::Error| AppError::Internal(e.to_string()).into_fragment())?;
            let video_path = video_dir.join(&file_name);

            let mut file = tokio::fs::File::create(&video_path).await.map_err(|e: std::io::Error| AppError::Internal(e.to_string()).into_fragment())?;
            while let Some(chunk) = field.chunk().await.map_err(|e: axum::extract::multipart::MultipartError| AppError::Internal(e.to_string()).into_fragment())? {
                file.write_all(&chunk).await.map_err(|e: std::io::Error| AppError::Internal(e.to_string()).into_fragment())?;
            }
            uploaded_videos.push((video_id, file_name, video_path));
        }
    }

    if destination_dir.is_empty() {
        return Err(AppError::NotFound("Destination directory is required".to_string()).into_fragment());
    }
    if interval <= 0.0 {
        return Err(AppError::NotFound("Interval must be greater than 0".to_string()).into_fragment());
    }

    // Process each video
    for (video_id, video_name, video_path) in uploaded_videos {
        let extract_dir = PathBuf::from(format!("/tmp/nettapper/vid2img/extract/{}", video_id));
        tokio::fs::create_dir_all(&extract_dir).await.map_err(|e: std::io::Error| AppError::Internal(e.to_string()).into_fragment())?;

        let config = ivid::extract::ExtractConfig {
            video: video_path.clone(),
            output_dir: extract_dir.clone(),
            interval,
            interval_mode: ivid::extract::IntervalMode::Time,
            start: None,
            stop: None,
            force: true,
            mkdir: true,
        };

        let video_path_clone = video_path.clone();
        let extract_dir_clone = extract_dir.clone();
        let config_clone = config;
        
        let _result = tokio::task::spawn_blocking(move || ivid::extract::run_extraction(&config_clone))
            .await
            .map_err(|e| AppError::Internal(e.to_string()).into_fragment())?
            .map_err(|e| AppError::Internal(e).into_fragment())?;

        // Find extracted frames, sort and rename
        let mut entries = Vec::new();
        let mut dir_reader = tokio::fs::read_dir(&extract_dir_clone).await.map_err(|e: std::io::Error| AppError::Internal(e.to_string()).into_fragment())?;
        while let Some(entry) = dir_reader.next_entry().await.map_err(|e: std::io::Error| AppError::Internal(e.to_string()).into_fragment())? {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ext == "png" {
                        entries.push(path);
                    }
                }
            }
        }
        entries.sort();

        let mut frames = Vec::new();
        for (i, old_path) in entries.into_iter().enumerate() {
            let frame_num = i + 1;
            let new_filename = format!("frame_{:05}.png", frame_num);
            let new_path = extract_dir_clone.join(&new_filename);

            tokio::fs::rename(&old_path, &new_path).await.map_err(|e: std::io::Error| AppError::Internal(e.to_string()).into_fragment())?;

            let timestamp = (frame_num as f64 - 0.5) * interval;
            let start_time = (frame_num as f64 - 1.0) * interval;
            let end_time = (frame_num as f64) * interval;

            frames.push(FrameMetadata {
                filename: new_filename,
                path: new_path.to_string_lossy().to_string(),
                timestamp,
                start_time,
                end_time,
            });
        }

        let metadata = VideoMetadata {
            video_id,
            video_name,
            video_path: video_path_clone.to_string_lossy().to_string(),
            destination_dir: destination_dir.clone(),
            extract_dir: extract_dir_clone.to_string_lossy().to_string(),
            frames,
            last_interval: interval,
        };

        let meta_json_path = extract_dir_clone.join("metadata.json");
        let meta_content = serde_json::to_string_pretty(&metadata).map_err(|e: serde_json::Error| AppError::Internal(e.to_string()).into_fragment())?;
        tokio::fs::write(&meta_json_path, meta_content).await.map_err(|e: std::io::Error| AppError::Internal(e.to_string()).into_fragment())?;
    }

    let reviews = list_active_reviews().await.map_err(|e| e.into_fragment())?;
    let tmpl = Vid2ImgListTemplate { reviews };
    tmpl.render()
        .map(Html)
        .map_err(|e| AppError::from(e).into_fragment())
}

// ── POST /api/vid2img/reextract ─────────────────────────────────────────────
#[derive(Deserialize)]
pub struct ReextractRequest {
    video_id: String,
    selected_indexes: Vec<usize>,
    new_interval: f64,
}

pub async fn reextract(Json(req): Json<ReextractRequest>) -> Result<Html<String>, Html<String>> {
    if req.selected_indexes.is_empty() {
        return Err(AppError::NotFound("No frames selected".to_string()).into_fragment());
    }
    if req.new_interval <= 0.0 {
        return Err(AppError::NotFound("Interval must be greater than 0".to_string()).into_fragment());
    }

    let extract_dir = PathBuf::from(format!("/tmp/nettapper/vid2img/extract/{}", req.video_id));
    let meta_json_path = extract_dir.join("metadata.json");
    if !meta_json_path.exists() {
        return Err(AppError::NotFound(format!("Metadata not found for video {}", req.video_id)).into_fragment());
    }

    let meta_content = tokio::fs::read_to_string(&meta_json_path).await.map_err(|e: std::io::Error| AppError::Internal(e.to_string()).into_fragment())?;
    let mut metadata: VideoMetadata = serde_json::from_str(&meta_content).map_err(|e: serde_json::Error| AppError::Internal(e.to_string()).into_fragment())?;

    // Group selected indices to contiguous runs
    let groups = group_contiguous_indices(&req.selected_indexes);

    let temp_extract_dir = extract_dir.join("temp_extract");
    if temp_extract_dir.exists() {
        tokio::fs::remove_dir_all(&temp_extract_dir).await.ok();
    }
    tokio::fs::create_dir_all(&temp_extract_dir).await.map_err(|e: std::io::Error| AppError::Internal(e.to_string()).into_fragment())?;

    let mut new_extracted_frames = Vec::new();

    for (group_idx, group_range) in groups.iter().enumerate() {
        let start_idx = *group_range.start();
        let end_idx = *group_range.end();
        if start_idx >= metadata.frames.len() || end_idx >= metadata.frames.len() {
            continue;
        }

        let group_start = metadata.frames[start_idx].start_time;
        let group_end = metadata.frames[end_idx].end_time;

        // Perform sub-extraction for this group
        let group_temp_dir = temp_extract_dir.join(format!("g_{}", group_idx));
        tokio::fs::create_dir_all(&group_temp_dir).await.map_err(|e: std::io::Error| AppError::Internal(e.to_string()).into_fragment())?;

        let config = ivid::extract::ExtractConfig {
            video: PathBuf::from(&metadata.video_path),
            output_dir: group_temp_dir.clone(),
            interval: req.new_interval,
            interval_mode: ivid::extract::IntervalMode::Time,
            start: Some(group_start),
            stop: Some(group_end),
            force: true,
            mkdir: true,
        };

        let _result = tokio::task::spawn_blocking(move || ivid::extract::run_extraction(&config))
            .await
            .map_err(|e| AppError::Internal(e.to_string()).into_fragment())?
            .map_err(|e| AppError::Internal(e).into_fragment())?;

        // Collect new frames from this group's extraction
        let mut entries = Vec::new();
        let mut dir_reader = tokio::fs::read_dir(&group_temp_dir).await.map_err(|e: std::io::Error| AppError::Internal(e.to_string()).into_fragment())?;
        while let Some(entry) = dir_reader.next_entry().await.map_err(|e: std::io::Error| AppError::Internal(e.to_string()).into_fragment())? {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ext == "png" {
                        entries.push(path);
                    }
                }
            }
        }
        entries.sort();

        for (i, old_path) in entries.into_iter().enumerate() {
            let frame_num = i + 1;
            let timestamp = group_start + (frame_num as f64 - 0.5) * req.new_interval;
            let start_time = group_start + (frame_num as f64 - 1.0) * req.new_interval;
            let end_time = group_start + (frame_num as f64) * req.new_interval;

            // Move to temporary filename in main extract_dir to avoid collision
            let temp_uuid_name = format!("temp_re_{}_{}_{}.png", group_idx, frame_num, SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
            let temp_dest_path = extract_dir.join(&temp_uuid_name);
            tokio::fs::rename(&old_path, &temp_dest_path).await.map_err(|e: std::io::Error| AppError::Internal(e.to_string()).into_fragment())?;

            new_extracted_frames.push(FrameMetadata {
                filename: temp_uuid_name,
                path: temp_dest_path.to_string_lossy().to_string(),
                timestamp,
                start_time,
                end_time,
            });
        }
    }

    // Remove old selected frames on disk
    let selected_set: std::collections::HashSet<usize> = req.selected_indexes.iter().cloned().collect();
    for (i, frame) in metadata.frames.iter().enumerate() {
        if selected_set.contains(&i) {
            tokio::fs::remove_file(&frame.path).await.ok();
        }
    }

    // Keep unselected old frames
    let mut combined_frames = Vec::new();
    for (i, frame) in metadata.frames.into_iter().enumerate() {
        if !selected_set.contains(&i) {
            combined_frames.push(frame);
        }
    }

    // Add newly extracted frames
    combined_frames.extend(new_extracted_frames);

    // Sort by timestamp
    combined_frames.sort_by(|a, b| a.timestamp.partial_cmp(&b.timestamp).unwrap_or(std::cmp::Ordering::Equal));

    // Rename all files sequentially to avoid conflicts, rename to temporary names first
    let mut temp_renames = Vec::new();
    for (i, mut frame) in combined_frames.into_iter().enumerate() {
        let temp_name = format!("sort_temp_{:05}.png", i + 1);
        let temp_path = extract_dir.join(&temp_name);
        tokio::fs::rename(&frame.path, &temp_path).await.map_err(|e: std::io::Error| AppError::Internal(e.to_string()).into_fragment())?;
        frame.filename = temp_name;
        frame.path = temp_path.to_string_lossy().to_string();
        temp_renames.push(frame);
    }

    // Now rename to final frame_xxxxx.png names
    let mut final_frames = Vec::new();
    for (i, mut frame) in temp_renames.into_iter().enumerate() {
        let final_name = format!("frame_{:05}.png", i + 1);
        let final_path = extract_dir.join(&final_name);
        tokio::fs::rename(&frame.path, &final_path).await.map_err(|e: std::io::Error| AppError::Internal(e.to_string()).into_fragment())?;
        frame.filename = final_name;
        frame.path = final_path.to_string_lossy().to_string();
        final_frames.push(frame);
    }

    metadata.frames = final_frames;
    metadata.last_interval = req.new_interval;

    // Rewrite metadata.json
    let meta_content = serde_json::to_string_pretty(&metadata).map_err(|e: serde_json::Error| AppError::Internal(e.to_string()).into_fragment())?;
    tokio::fs::write(&meta_json_path, meta_content).await.map_err(|e: std::io::Error| AppError::Internal(e.to_string()).into_fragment())?;

    // Cleanup temp directory
    tokio::fs::remove_dir_all(&temp_extract_dir).await.ok();

    let reviews = list_active_reviews().await.map_err(|e| e.into_fragment())?;
    let tmpl = Vid2ImgListTemplate { reviews };
    tmpl.render()
        .map(Html)
        .map_err(|e| AppError::from(e).into_fragment())
}

// ── POST /api/vid2img/save ──────────────────────────────────────────────────
#[derive(Deserialize)]
pub struct SaveRequest {
    video_id: String,
    save_all: bool,
    paths: Vec<String>,
}

pub async fn save(
    State(pool): State<SqlitePool>,
    Json(req): Json<SaveRequest>,
) -> Result<Html<String>, Html<String>> {
    let extract_dir = PathBuf::from(format!("/tmp/nettapper/vid2img/extract/{}", req.video_id));
    let meta_json_path = extract_dir.join("metadata.json");
    if !meta_json_path.exists() {
        return Err(AppError::NotFound("Video review session not found".to_string()).into_fragment());
    }

    let meta_content = tokio::fs::read_to_string(&meta_json_path).await.map_err(|e: std::io::Error| AppError::Internal(e.to_string()).into_fragment())?;
    let mut metadata: VideoMetadata = serde_json::from_str(&meta_content).map_err(|e: serde_json::Error| AppError::Internal(e.to_string()).into_fragment())?;

    let dest_dir_str = metadata.destination_dir.clone();
    let dest_dir = Path::new(&dest_dir_str);
    tokio::fs::create_dir_all(&dest_dir).await.map_err(|e: std::io::Error| AppError::Internal(format!("Failed to create destination dir: {e}")).into_fragment())?;

    let path_set: std::collections::HashSet<String> = req.paths.iter().cloned().collect();
    let mut kept_frames = Vec::new();
    let mut moved_any = false;

    for frame in metadata.frames {
        let should_move = req.save_all || path_set.contains(&frame.path);
        if should_move {
            let src_path = Path::new(&frame.path);
            if src_path.exists() {
                let stem = src_path.file_stem().and_then(|s| s.to_str()).unwrap_or("frame");
                let ext = src_path.extension().and_then(|e| e.to_str()).unwrap_or("png");
                let mut dest_path = dest_dir.join(format!("{}.{}", stem, ext));
                let mut counter = 1;
                while dest_path.exists() {
                    dest_path = dest_dir.join(format!("{}_{}.{}", stem, counter, ext));
                    counter += 1;
                }
                
                tokio::fs::rename(src_path, &dest_path).await.map_err(|e: std::io::Error| AppError::Internal(format!("Failed to move file: {e}")).into_fragment())?;
                moved_any = true;
            }
        } else {
            kept_frames.push(frame);
        }
    }

    if moved_any {
        // Trigger database scan in background
        let dest_dir_str_clone = dest_dir_str.clone();
        let pool_clone = pool.clone();
        tokio::spawn(async move {
            let opts = idup::scan::ScanOptions::all();
            let stats = idup::scan::process_path(PathBuf::from(&dest_dir_str_clone), true, &opts, &pool_clone).await;
            println!("Background scan of {} processed {} files in {}s", dest_dir_str_clone, stats.processed, stats.elapsed_secs);
        });
    }

    if req.save_all || kept_frames.is_empty() {
        // Clean up everything
        tokio::fs::remove_dir_all(&extract_dir).await.ok();
        let video_dir = PathBuf::from(format!("/tmp/nettapper/vid2img/videos/{}", req.video_id));
        tokio::fs::remove_dir_all(&video_dir).await.ok();
    } else {
        // Update metadata.json with kept frames
        metadata.frames = kept_frames;
        let meta_content = serde_json::to_string_pretty(&metadata).map_err(|e: serde_json::Error| AppError::Internal(e.to_string()).into_fragment())?;
        tokio::fs::write(&meta_json_path, meta_content).await.map_err(|e: std::io::Error| AppError::Internal(e.to_string()).into_fragment())?;
    }

    let reviews = list_active_reviews().await.map_err(|e| e.into_fragment())?;
    let tmpl = Vid2ImgListTemplate { reviews };
    tmpl.render()
        .map(Html)
        .map_err(|e| AppError::from(e).into_fragment())
}

// ── POST /api/vid2img/delete ────────────────────────────────────────────────
#[derive(Deserialize)]
pub struct DeleteRequest {
    video_id: String,
}

pub async fn delete(axum::extract::Form(req): axum::extract::Form<DeleteRequest>) -> Result<Html<String>, Html<String>> {
    let extract_dir = PathBuf::from(format!("/tmp/nettapper/vid2img/extract/{}", req.video_id));
    tokio::fs::remove_dir_all(&extract_dir).await.ok();

    let video_dir = PathBuf::from(format!("/tmp/nettapper/vid2img/videos/{}", req.video_id));
    tokio::fs::remove_dir_all(&video_dir).await.ok();

    let reviews = list_active_reviews().await.map_err(|e| e.into_fragment())?;
    let tmpl = Vid2ImgListTemplate { reviews };
    tmpl.render()
        .map(Html)
        .map_err(|e| AppError::from(e).into_fragment())
}

// ── Helpers ────────────────────────────────────────────────────────────────
pub async fn list_active_reviews() -> Result<Vec<VideoMetadata>, AppError> {
    let extract_root = Path::new("/tmp/nettapper/vid2img/extract");
    if !extract_root.exists() {
        return Ok(Vec::new());
    }

    let mut list = Vec::new();
    let mut dir_reader = tokio::fs::read_dir(extract_root).await.map_err(|e: std::io::Error| AppError::Internal(e.to_string()))?;
    while let Some(entry) = dir_reader.next_entry().await.map_err(|e: std::io::Error| AppError::Internal(e.to_string()))? {
        let path = entry.path();
        if path.is_dir() {
            let meta_path = path.join("metadata.json");
            if meta_path.exists() {
                if let Ok(content) = tokio::fs::read_to_string(&meta_path).await {
                    if let Ok(meta) = serde_json::from_str::<VideoMetadata>(&content) {
                        list.push(meta);
                    }
                }
            }
        }
    }
    list.sort_by(|a, b| a.video_name.cmp(&b.video_name));
    Ok(list)
}

fn group_contiguous_indices(indices: &[usize]) -> Vec<std::ops::RangeInclusive<usize>> {
    if indices.is_empty() {
        return Vec::new();
    }
    let mut sorted = indices.to_vec();
    sorted.sort();

    let mut groups = Vec::new();
    let mut start = sorted[0];
    let mut prev = sorted[0];

    for &idx in &sorted[1..] {
        if idx == prev + 1 {
            prev = idx;
        } else {
            groups.push(start..=prev);
            start = idx;
            prev = idx;
        }
    }
    groups.push(start..=prev);
    groups
}

#[derive(Deserialize)]
pub struct CheckDirQuery {
    path: String,
}

#[derive(Serialize)]
pub struct CheckDirResponse {
    exists: bool,
}

pub async fn check_dir(
    axum::extract::Query(params): axum::extract::Query<CheckDirQuery>,
) -> Json<CheckDirResponse> {
    let path = Path::new(&params.path);
    Json(CheckDirResponse {
        exists: path.exists() && path.is_dir(),
    })
}

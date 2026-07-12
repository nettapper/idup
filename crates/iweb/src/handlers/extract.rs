use axum::response::Html;
use axum::Form;
use serde::Deserialize;
use std::path::PathBuf;
use askama::Template;

use crate::error::AppError;
use super::url_encode;

#[derive(Deserialize)]
pub struct ExtractForm {
    video: String,
    interval: Option<f64>,
    interval_mode: Option<String>,
    start: Option<String>,
    stop: Option<String>,
    output: Option<String>,
    mkdir: Option<String>,
    force: Option<String>,
}

#[derive(Template)]
#[template(path = "fragments/extract_result.html")]
struct ExtractResultTemplate {
    frame_count: usize,
    elapsed_secs: String,
    output_dir: String,
    encoded_path: String,
}

pub async fn extract(Form(form): Form<ExtractForm>) -> Result<Html<String>, Html<String>> {
    let video_path = PathBuf::from(form.video.trim());
    if form.video.trim().is_empty() {
        return Err(AppError::NotFound("Video file path is required".to_string()).into_fragment());
    }

    let interval = form.interval.unwrap_or(1.0);
    if interval <= 0.0 {
        return Err(AppError::NotFound("Interval must be greater than 0".to_string()).into_fragment());
    }

    let interval_mode = match form.interval_mode.as_deref() {
        Some("frame") => ivid::extract::IntervalMode::Frame,
        _ => ivid::extract::IntervalMode::Time,
    };

    let start_secs = if let Some(ref s) = form.start.filter(|s| !s.trim().is_empty()) {
        match ivid::time::parse_hhmmss(s) {
            Ok(secs) => Some(secs),
            Err(e) => return Err(AppError::NotFound(format!("Invalid start time: {e}")).into_fragment()),
        }
    } else {
        None
    };

    let stop_secs = if let Some(ref s) = form.stop.filter(|s| !s.trim().is_empty()) {
        match ivid::time::parse_hhmmss(s) {
            Ok(secs) => Some(secs),
            Err(e) => return Err(AppError::NotFound(format!("Invalid stop time: {e}")).into_fragment()),
        }
    } else {
        None
    };

    let video_stem = video_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("video");

    let output_dir = if let Some(ref out) = form.output.filter(|o| !o.trim().is_empty()) {
        PathBuf::from(out.trim())
    } else {
        PathBuf::from(format!("ivid_{video_stem}"))
    };

    let mkdir = form.mkdir.is_some();
    let force = form.force.is_some();

    let config = ivid::extract::ExtractConfig {
        video: video_path,
        output_dir,
        interval,
        interval_mode,
        start: start_secs,
        stop: stop_secs,
        force,
        mkdir,
    };

    match tokio::task::spawn_blocking(move || ivid::extract::run_extraction(&config)).await {
        Ok(Ok(result)) => {
            let output_dir_str = result.output_dir.to_string_lossy().into_owned();
            let encoded_path = url_encode(&output_dir_str);

            let tmpl = ExtractResultTemplate {
                frame_count: result.frame_count,
                elapsed_secs: format!("{:.2}", result.elapsed_secs),
                output_dir: output_dir_str,
                encoded_path,
            };

            tmpl.render()
                .map(Html)
                .map_err(|e| AppError::from(e).into_fragment())
        }
        Ok(Err(e)) => Err(AppError::Internal(e).into_fragment()),
        Err(e) => Err(AppError::Internal(format!("Internal task execution error: {e}")).into_fragment()),
    }
}

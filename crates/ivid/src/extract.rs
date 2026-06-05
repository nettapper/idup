use std::path::{PathBuf};
use std::time::Instant;

use crate::ffmpeg;

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum IntervalMode {
    Time,
    Frame,
}


pub struct ExtractConfig {
    pub video: PathBuf,
    pub output_dir: PathBuf,
    pub interval: f64,
    pub interval_mode: IntervalMode,
    pub start: Option<f64>,
    pub stop: Option<f64>,
    pub force: bool,
    pub mkdir: bool,
}

pub struct ExtractResult {
    pub frame_count: usize,
    pub output_dir: PathBuf,
    pub elapsed_secs: f64,
}

/// Run the extraction using a single ffmpeg command.
pub fn run_extraction(config: &ExtractConfig) -> Result<ExtractResult, String> {
    // 1. Check ffmpeg/ffprobe availability
    ffmpeg::check_ffmpeg()?;
    ffmpeg::check_ffprobe()?;

    // 2. Validate input file
    if !config.video.exists() {
        return Err(format!("Video file not found: {}", config.video.display()));
    }

    // 3. Validate interval
    if config.interval <= 0.0 {
        return Err("Interval must be greater than 0".to_string());
    }

    // 4. Probe video duration and fps
    let duration = ffmpeg::probe_duration(&config.video)?;

    // 5. Parse and validate start/stop
    let start_secs = config.start.unwrap_or(0.0);
    let stop_secs = config.stop.unwrap_or(duration);

    if start_secs >= stop_secs {
        return Err(format!(
            "Start time ({start_secs:.2}s) must be before stop time ({stop_secs:.2}s)"
        ));
    }
    if start_secs >= duration {
        return Err(format!(
            "Start time ({start_secs:.2}s) is at or past video duration ({duration:.2}s)"
        ));
    }
    if stop_secs > duration {
        return Err(format!(
            "Stop time ({stop_secs:.2}s) exceeds video duration ({duration:.2}s)"
        ));
    }

    // 6. Resolve output directory
    if !config.output_dir.exists() {
        if config.mkdir {
            std::fs::create_dir_all(&config.output_dir)
                .map_err(|e| format!("Failed to create output directory: {e}"))?;
        } else {
            return Err(format!(
                "Output directory does not exist: {}. Use --mkdir to create it.",
                config.output_dir.display()
            ));
        }
    }

    // 7. Check if output directory is not empty
    if !config.force {
        let is_empty = std::fs::read_dir(&config.output_dir)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(true);
        if !is_empty {
            return Err(format!(
                "Output directory is not empty: {}. Use --force to overwrite.",
                config.output_dir.display()
            ));
        }
    }

    // 8. Build output pattern
    let video_stem = config.video.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("video");
    let output_pattern = config.output_dir.join(format!("{video_stem}_%04d.png"));

    let wall_start = Instant::now();

    // 9. Call ffmpeg
    match config.interval_mode {
        IntervalMode::Time => {
            let fps_value = 1.0 / config.interval;
            ffmpeg::extract_frames_by_time(
                &config.video,
                &output_pattern,
                fps_value,
                config.start,
                config.stop,
            )?;
        }
        IntervalMode::Frame => {
            let frame_interval = config.interval.round() as u64;
            let frame_interval = frame_interval.max(1);
            ffmpeg::extract_frames_by_frame_interval(
                &config.video,
                &output_pattern,
                frame_interval,
                config.start,
                config.stop,
            )?;
        }
    }

    let elapsed_secs = wall_start.elapsed().as_secs_f64();

    // 10. Count output files
    let mut frame_count = 0;
    if let Ok(entries) = std::fs::read_dir(&config.output_dir) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_file() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if name.starts_with(video_stem) && name.ends_with(".png") {
                        frame_count += 1;
                    }
                }
            }
        }
    }

    Ok(ExtractResult {
        frame_count,
        output_dir: config.output_dir.clone(),
        elapsed_secs,
    })
}

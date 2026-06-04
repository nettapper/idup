use std::path::Path;
use std::time::Instant;

use crate::ffmpeg;
use crate::time;

/// Configuration for a frame extraction run.
pub struct ExtractConfig<'a> {
    pub video: &'a Path,
    pub output_dir: &'a Path,
    pub video_stem: &'a str,
    pub timestamps: &'a [f64],
    pub sub_second: bool,
    pub start_secs: f64,
    pub stop_secs: f64,
}

/// Compute extraction timestamps for time-based mode.
///
/// Generates timestamps from `start` to `stop` (exclusive), stepping by
/// `interval` seconds.
pub fn compute_timestamps_time(start: f64, stop: f64, interval: f64) -> Vec<f64> {
    let mut timestamps = Vec::new();
    let mut t = start;
    while t < stop {
        timestamps.push(t);
        t += interval;
    }
    timestamps
}

/// Compute extraction timestamps for frame-based mode.
///
/// Walks frame numbers 0, interval, 2*interval, … and converts each to a
/// timestamp via the video's fps. Only timestamps within [start, stop) are
/// included.
pub fn compute_timestamps_frame(start: f64, stop: f64, frame_interval: f64, fps: f64) -> Vec<f64> {
    let frame_interval = (frame_interval.round() as u64).max(1);
    let mut timestamps = Vec::new();

    let mut frame = 0u64;
    loop {
        let t = frame as f64 / fps;
        if t >= stop {
            break;
        }
        if t >= start {
            timestamps.push(t);
        }
        frame += frame_interval;
    }
    timestamps
}

/// Build the output filename for a given timestamp.
pub fn output_filename(stem: &str, timestamp: f64, sub_second: bool) -> String {
    let ts = time::format_timestamp(timestamp, sub_second);
    format!("{stem}_{ts}.png")
}

/// Run the extraction loop, calling ffmpeg once per frame.
///
/// Prints progress every 10 seconds and a summary on completion.
pub fn run_extraction(config: ExtractConfig) -> Result<(), String> {
    let total = config.timestamps.len();
    if total == 0 {
        println!("[ivid] No frames to extract in the specified range.");
        return Ok(());
    }

    println!(
        "[ivid] Extracting {total} frames from {} ({}–{})...",
        config.video.display(),
        time::format_hhmmss(config.start_secs),
        time::format_hhmmss(config.stop_secs),
    );

    let wall_start = Instant::now();
    let mut last_progress = Instant::now();

    for (i, &ts) in config.timestamps.iter().enumerate() {
        let filename = output_filename(config.video_stem, ts, config.sub_second);
        let output_path = config.output_dir.join(&filename);

        ffmpeg::extract_frame(config.video, ts, &output_path).map_err(|e| {
            format!(
                "Failed to extract frame at {}: {e}",
                time::format_hhmmss(ts)
            )
        })?;

        if last_progress.elapsed().as_secs() >= 10 {
            println!(
                "[ivid] Extracting... {} of {total} frames captured (at {} of {})",
                i + 1,
                time::format_hhmmss(ts),
                time::format_hhmmss(config.stop_secs),
            );
            last_progress = Instant::now();
        }
    }

    let elapsed = wall_start.elapsed().as_secs_f64();
    println!(
        "[ivid] Done. {total} frames extracted covering {}–{} in {elapsed:.1}s.",
        time::format_hhmmss(config.start_secs),
        time::format_hhmmss(config.stop_secs),
    );
    println!("[ivid] Output: {}", config.output_dir.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_time_basic() {
        let ts = compute_timestamps_time(0.0, 5.0, 1.0);
        assert_eq!(ts, vec![0.0, 1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn timestamps_time_sub_second() {
        let ts = compute_timestamps_time(0.0, 1.0, 0.5);
        assert_eq!(ts, vec![0.0, 0.5]);
    }

    #[test]
    fn timestamps_time_with_start_offset() {
        let ts = compute_timestamps_time(10.0, 13.0, 1.0);
        assert_eq!(ts, vec![10.0, 11.0, 12.0]);
    }

    #[test]
    fn timestamps_time_empty_range() {
        let ts = compute_timestamps_time(5.0, 5.0, 1.0);
        assert!(ts.is_empty());
    }

    #[test]
    fn timestamps_frame_basic() {
        // 30 fps, every 30 frames = 1 frame per second
        let ts = compute_timestamps_frame(0.0, 3.0, 30.0, 30.0);
        assert_eq!(ts.len(), 3);
        assert!((ts[0] - 0.0).abs() < 0.01);
        assert!((ts[1] - 1.0).abs() < 0.01);
        assert!((ts[2] - 2.0).abs() < 0.01);
    }

    #[test]
    fn timestamps_frame_with_start() {
        // 30 fps, every 30 frames, start at 2s
        let ts = compute_timestamps_frame(2.0, 5.0, 30.0, 30.0);
        assert_eq!(ts.len(), 3);
        assert!((ts[0] - 2.0).abs() < 0.01);
    }

    #[test]
    fn output_filename_whole_second() {
        assert_eq!(
            output_filename("myvideo", 90.0, false),
            "myvideo_00h01m30s.png"
        );
    }

    #[test]
    fn output_filename_sub_second() {
        assert_eq!(
            output_filename("myvideo", 0.5, true),
            "myvideo_00h00m00s500ms.png"
        );
    }
}

use std::path::PathBuf;
use std::process;

use clap::{Parser, ValueEnum};

mod extract;
mod ffmpeg;
mod time;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum IntervalMode {
    /// Interval is in seconds (default)
    Time,
    /// Interval is in frames
    Frame,
}

#[derive(Parser, Debug)]
#[command(
    name = "ivid",
    about = "Extract screenshots from a video at configurable intervals"
)]
struct Cli {
    /// Path to the input video file
    video: PathBuf,

    /// Interval between frame captures (seconds or frames depending on --interval-mode).
    /// Accepts fractional values (e.g. 0.5 for twice per second).
    #[arg(long, default_value = "1.0")]
    interval: f64,

    /// Whether --interval is in seconds (time) or frames (frame)
    #[arg(long, default_value = "time", value_name = "MODE")]
    interval_mode: IntervalMode,

    /// Start time for extraction (HH:MM:SS)
    #[arg(long)]
    start: Option<String>,

    /// Stop time for extraction (HH:MM:SS). Defaults to end of video.
    #[arg(long)]
    stop: Option<String>,

    /// Output directory for extracted frames.
    /// Defaults to ./ivid_<video_stem>/ in the current directory.
    #[arg(long, short)]
    output: Option<PathBuf>,

    /// Create the output directory if it doesn't exist
    #[arg(long)]
    mkdir: bool,

    /// Overwrite existing files in the output directory
    #[arg(long)]
    force: bool,
}

fn main() {
    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        eprintln!("[ivid] Error: {e}");
        process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    // --- 1. Check ffmpeg/ffprobe availability ---
    ffmpeg::check_ffmpeg()?;
    ffmpeg::check_ffprobe()?;

    // --- 2. Validate input file ---
    if !cli.video.exists() {
        return Err(format!(
            "Video file not found: {}",
            cli.video.display()
        ));
    }

    // --- 3. Validate interval ---
    if cli.interval <= 0.0 {
        return Err("--interval must be greater than 0".to_string());
    }

    // --- 4. Probe video metadata ---
    let duration = ffmpeg::probe_duration(&cli.video)?;
    let fps = ffmpeg::probe_fps(&cli.video)?;

    println!(
        "[ivid] Video: {} ({}, {fps:.2} fps)",
        cli.video.display(),
        time::format_hhmmss(duration),
    );

    // --- 5. Parse and validate start/stop ---
    let start_secs = match &cli.start {
        Some(s) => time::parse_hhmmss(s)?,
        None => 0.0,
    };
    let stop_secs = match &cli.stop {
        Some(s) => time::parse_hhmmss(s)?,
        None => duration,
    };

    if start_secs >= stop_secs {
        return Err(format!(
            "--start ({}) must be before --stop ({})",
            time::format_hhmmss(start_secs),
            time::format_hhmmss(stop_secs),
        ));
    }
    if start_secs >= duration {
        return Err(format!(
            "--start ({}) is at or past video duration ({})",
            time::format_hhmmss(start_secs),
            time::format_hhmmss(duration),
        ));
    }
    if cli.stop.is_some() && stop_secs > duration {
        return Err(format!(
            "--stop ({}) exceeds video duration ({})",
            time::format_hhmmss(stop_secs),
            time::format_hhmmss(duration),
        ));
    }

    // --- 6. Determine output directory ---
    let video_stem = cli
        .video
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("video");

    let output_dir = cli
        .output
        .unwrap_or_else(|| PathBuf::from(format!("ivid_{video_stem}")));

    // --- 7. Handle output directory existence ---
    if !output_dir.exists() {
        if cli.mkdir {
            std::fs::create_dir_all(&output_dir)
                .map_err(|e| format!("Failed to create output directory: {e}"))?;
        } else {
            return Err(format!(
                "Output directory does not exist: {}. Use --mkdir to create it.",
                output_dir.display()
            ));
        }
    }

    // --- 8. Determine if interval is sub-second ---
    let sub_second = match cli.interval_mode {
        IntervalMode::Time => cli.interval.fract() != 0.0,
        IntervalMode::Frame => {
            // In frame mode, sub-second depends on whether frame interval
            // produces sub-second timestamps (it usually does)
            let interval_secs = cli.interval / fps;
            interval_secs.fract() != 0.0
        }
    };

    // --- 9. Compute extraction timestamps ---
    let timestamps = match cli.interval_mode {
        IntervalMode::Time => {
            extract::compute_timestamps_time(start_secs, stop_secs, cli.interval)
        }
        IntervalMode::Frame => {
            extract::compute_timestamps_frame(start_secs, stop_secs, cli.interval, fps)
        }
    };

    // --- 10. Check for existing files (unless --force) ---
    if !cli.force {
        let existing: Vec<_> = timestamps
            .iter()
            .map(|&t| extract::output_filename(video_stem, t, sub_second))
            .map(|name| output_dir.join(name))
            .filter(|p| p.exists())
            .collect();

        if !existing.is_empty() {
            return Err(format!(
                "{} output file(s) already exist in {}. Use --force to overwrite.",
                existing.len(),
                output_dir.display()
            ));
        }
    }

    // --- 11. Extract frames ---
    let config = extract::ExtractConfig {
        video: &cli.video,
        output_dir: &output_dir,
        video_stem,
        timestamps: &timestamps,
        sub_second,
        start_secs,
        stop_secs,
    };

    extract::run_extraction(config)?;

    Ok(())
}

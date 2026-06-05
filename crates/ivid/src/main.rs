use std::path::PathBuf;
use std::process;

use clap::Parser;

use ivid::extract::{ExtractConfig, IntervalMode, run_extraction};
use ivid::time;


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
    let video_stem = cli
        .video
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("video");

    let user_specified_output = cli.output.is_some();
    let output_dir = cli
        .output
        .unwrap_or_else(|| PathBuf::from(format!("ivid_{video_stem}")));

    let start_secs = match &cli.start {
        Some(s) => Some(time::parse_hhmmss(s)?),
        None => None,
    };
    let stop_secs = match &cli.stop {
        Some(s) => Some(time::parse_hhmmss(s)?),
        None => None,
    };

    // For CLI, default output directory (ivid_<video_stem>) is always auto-created (mkdir = true).
    // User-specified output requires either mkdir flag or output_dir to already exist.
    let mkdir = if !user_specified_output {
        true
    } else {
        cli.mkdir
    };

    let config = ExtractConfig {
        video: cli.video,
        output_dir,
        interval: cli.interval,
        interval_mode: cli.interval_mode,
        start: start_secs,
        stop: stop_secs,
        force: cli.force,
        mkdir,
    };

    println!(
        "[ivid] Extracting frames from {}...",
        config.video.display()
    );

    let result = run_extraction(&config)?;

    println!(
        "[ivid] Done. {} frames extracted in {:.1}s.",
        result.frame_count,
        result.elapsed_secs
    );
    println!("[ivid] Output directory: {}", result.output_dir.display());

    Ok(())
}

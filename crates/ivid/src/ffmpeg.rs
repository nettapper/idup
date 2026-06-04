use std::path::Path;
use std::process::Command;

/// Verify that ffmpeg is installed and reachable.
pub fn check_ffmpeg() -> Result<(), String> {
    Command::new("ffmpeg")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|_| {
            "ffmpeg not found. Install ffmpeg and ensure it's in your PATH.".to_string()
        })
        .and_then(|s| {
            if s.success() {
                Ok(())
            } else {
                Err("ffmpeg returned an error.".to_string())
            }
        })
}

/// Verify that ffprobe is installed and reachable.
pub fn check_ffprobe() -> Result<(), String> {
    Command::new("ffprobe")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|_| {
            "ffprobe not found. Install ffmpeg (includes ffprobe) and ensure it's in your PATH."
                .to_string()
        })
        .and_then(|s| {
            if s.success() {
                Ok(())
            } else {
                Err("ffprobe returned an error.".to_string())
            }
        })
}

/// Probe the video duration in seconds using ffprobe.
pub fn probe_duration(path: &Path) -> Result<f64, String> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .map_err(|e| format!("Failed to run ffprobe: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Not a recognized video format: {}\n{stderr}",
            path.display()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().next().unwrap_or("").trim();

    line.parse::<f64>().map_err(|_| {
        format!(
            "Failed to parse video duration from ffprobe output: '{line}'"
        )
    })
}

/// Probe the video frame rate using ffprobe.
///
/// Returns the fps as an f64 (e.g. 29.97 for 30000/1001).
pub fn probe_fps(path: &Path) -> Result<f64, String> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=r_frame_rate",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .map_err(|e| format!("Failed to run ffprobe: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Failed to probe video fps: {}\n{stderr}",
            path.display()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().next().unwrap_or("").trim();

    // ffprobe returns fps as a fraction like "30/1" or "24000/1001"
    if let Some((num, den)) = line.split_once('/') {
        let n: f64 = num
            .parse()
            .map_err(|_| format!("Failed to parse fps numerator: '{num}'"))?;
        let d: f64 = den
            .parse()
            .map_err(|_| format!("Failed to parse fps denominator: '{den}'"))?;
        if d == 0.0 {
            return Err("Video has zero fps denominator".to_string());
        }
        Ok(n / d)
    } else {
        line.parse::<f64>()
            .map_err(|_| format!("Failed to parse fps from ffprobe output: '{line}'"))
    }
}

/// Extract a single frame from a video at the given timestamp (in seconds).
pub fn extract_frame(video: &Path, timestamp_secs: f64, output_path: &Path) -> Result<(), String> {
    let result = Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error"])
        .arg("-ss")
        .arg(format!("{timestamp_secs:.3}"))
        .arg("-i")
        .arg(video)
        .args(["-frames:v", "1"])
        .arg(output_path)
        .output()
        .map_err(|e| format!("Failed to run ffmpeg: {e}"))?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(format!("ffmpeg failed: {}", stderr.trim()));
    }

    Ok(())
}

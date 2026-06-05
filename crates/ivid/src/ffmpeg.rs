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

/// Extract frames at a fixed FPS rate (time-based interval).
/// fps_value = 1.0 / interval_secs (e.g., interval=2s -> fps=0.5)
/// Command: ffmpeg -y -loglevel error [-ss start] [-to stop] -i video -vf "fps=<fps_value>" output_pattern
pub fn extract_frames_by_time(
    video: &Path,
    output_pattern: &Path,
    fps_value: f64,
    start: Option<f64>,
    stop: Option<f64>,
) -> Result<(), String> {
    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-y").args(["-loglevel", "error"]);

    if let Some(s) = start {
        cmd.args(["-ss", &format!("{s:.3}")]);
    }
    if let Some(t) = stop {
        cmd.args(["-to", &format!("{t:.3}")]);
    }

    cmd.arg("-i").arg(video);
    cmd.args(["-vf", &format!("fps={fps_value}")]);
    cmd.arg(output_pattern);

    let output = cmd.output().map_err(|e| format!("Failed to run ffmpeg: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffmpeg failed: {}", stderr.trim()));
    }
    Ok(())
}

/// Extract every Nth frame (frame-based interval).
/// Command: ffmpeg -y -loglevel error [-ss start] [-to stop] -i video -vf "select='not(mod(n\,<N>))'" -vsync vfr output_pattern
pub fn extract_frames_by_frame_interval(
    video: &Path,
    output_pattern: &Path,
    frame_interval: u64,
    start: Option<f64>,
    stop: Option<f64>,
) -> Result<(), String> {
    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-y").args(["-loglevel", "error"]);

    if let Some(s) = start {
        cmd.args(["-ss", &format!("{s:.3}")]);
    }
    if let Some(t) = stop {
        cmd.args(["-to", &format!("{t:.3}")]);
    }

    cmd.arg("-i").arg(video);
    cmd.args(["-vf", &format!("select='not(mod(n\\,{}))'", frame_interval)]);
    cmd.args(["-vsync", "vfr"]);
    cmd.arg(output_pattern);

    let output = cmd.output().map_err(|e| format!("Failed to run ffmpeg: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffmpeg failed: {}", stderr.trim()));
    }
    Ok(())
}


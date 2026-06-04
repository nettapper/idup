/// Parse a "HH:MM:SS" string into total seconds.
pub fn parse_hhmmss(s: &str) -> Result<f64, String> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        return Err(format!("Invalid time format '{s}': expected HH:MM:SS"));
    }

    let hours: f64 = parts[0]
        .parse()
        .map_err(|_| format!("Invalid hours in '{s}'"))?;
    let minutes: f64 = parts[1]
        .parse()
        .map_err(|_| format!("Invalid minutes in '{s}'"))?;
    let seconds: f64 = parts[2]
        .parse()
        .map_err(|_| format!("Invalid seconds in '{s}'"))?;

    if minutes >= 60.0 || seconds >= 60.0 {
        return Err(format!(
            "Invalid time '{s}': minutes and seconds must be < 60"
        ));
    }

    Ok(hours * 3600.0 + minutes * 60.0 + seconds)
}

/// Format seconds as "HH:MM:SS".
pub fn format_hhmmss(secs: f64) -> String {
    let total = secs.floor() as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

/// Format seconds as a filename-safe timestamp.
///
/// When `sub_second` is true, includes milliseconds: `00h01m30s500ms`.
/// Otherwise: `00h01m30s`.
pub fn format_timestamp(secs: f64, sub_second: bool) -> String {
    let total_ms = (secs * 1000.0).round() as u64;
    let h = total_ms / 3_600_000;
    let m = (total_ms % 3_600_000) / 60_000;
    let s = (total_ms % 60_000) / 1000;
    let ms = total_ms % 1000;

    if sub_second {
        format!("{h:02}h{m:02}m{s:02}s{ms:03}ms")
    } else {
        format!("{h:02}h{m:02}m{s:02}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hhmmss_basic() {
        assert!((parse_hhmmss("00:00:00").unwrap() - 0.0).abs() < f64::EPSILON);
        assert!((parse_hhmmss("00:01:30").unwrap() - 90.0).abs() < f64::EPSILON);
        assert!((parse_hhmmss("01:00:00").unwrap() - 3600.0).abs() < f64::EPSILON);
        assert!((parse_hhmmss("01:30:45").unwrap() - 5445.0).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_hhmmss_rejects_bad_input() {
        assert!(parse_hhmmss("00:00").is_err());
        assert!(parse_hhmmss("abc").is_err());
        assert!(parse_hhmmss("00:60:00").is_err());
        assert!(parse_hhmmss("00:00:60").is_err());
    }

    #[test]
    fn format_hhmmss_basic() {
        assert_eq!(format_hhmmss(0.0), "00:00:00");
        assert_eq!(format_hhmmss(90.0), "00:01:30");
        assert_eq!(format_hhmmss(3661.0), "01:01:01");
    }

    #[test]
    fn format_timestamp_whole_seconds() {
        assert_eq!(format_timestamp(0.0, false), "00h00m00s");
        assert_eq!(format_timestamp(5.0, false), "00h00m05s");
        assert_eq!(format_timestamp(90.0, false), "00h01m30s");
        assert_eq!(format_timestamp(3661.0, false), "01h01m01s");
    }

    #[test]
    fn format_timestamp_sub_second() {
        assert_eq!(format_timestamp(0.0, true), "00h00m00s000ms");
        assert_eq!(format_timestamp(0.5, true), "00h00m00s500ms");
        assert_eq!(format_timestamp(1.0, true), "00h00m01s000ms");
        assert_eq!(format_timestamp(90.123, true), "00h01m30s123ms");
    }
}

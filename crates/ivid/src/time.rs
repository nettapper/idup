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
}

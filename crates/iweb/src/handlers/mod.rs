pub mod assets;
pub mod clean;
pub mod crop;
pub mod explore;
pub mod extract;
pub mod file_ops;
pub mod image;
pub mod info;
pub mod list;
pub mod random;
pub mod scan;
pub mod update;

// ── Shared Helpers ────────────────────────────────────────────────────────────

/// Percent-encode characters that are not safe as a URL query-parameter value.
pub(crate) fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for b in s.bytes() {
        match b {
            // Unreserved characters: leave as-is
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'~'
            | b'/' => out.push(b as char),
            // Everything else: percent-encode
            _ => {
                out.push('%');
                out.push(
                    char::from_digit((b >> 4) as u32, 16)
                        .unwrap()
                        .to_ascii_uppercase(),
                );
                out.push(
                    char::from_digit((b & 0xf) as u32, 16)
                        .unwrap()
                        .to_ascii_uppercase(),
                );
            }
        }
    }
    out
}

/// Build an /explore URL for a duplicate group hash.
pub(crate) fn explore_href_for_hash(hash: &str) -> String {
    format!("/explore?hash={}", url_encode(hash))
}

/// Build an /explore URL for a seeded random result.
pub(crate) fn explore_href_seeded(seed: u64, n: u32, filter: Option<&str>) -> String {
    let mut href = format!("/explore?seed={}&n={}", seed, n);
    if let Some(f) = filter {
        href.push_str(&format!("&filter={}", url_encode(f)));
    }
    href
}

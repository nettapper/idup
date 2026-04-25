use image::{ImageFormat, RgbImage};
use std::path::{Path, PathBuf};

/// Advance one step of a simple LCG (Knuth multiplicative constants).
/// Returns the next state and a byte derived from the high bits.
fn lcg_next(state: u64) -> (u64, u8) {
    let next = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    let byte = (next >> 56) as u8;
    (next, byte)
}

/// Generate a deterministic `RgbImage` from `seed`.
///
/// Given the same `seed`, `width`, and `height` this always produces
/// identical pixel data — and therefore an identical SHA-256 hash.
pub fn generate_image(seed: u64, width: u32, height: u32) -> RgbImage {
    let mut state = seed;
    let mut img = RgbImage::new(width, height);

    for pixel in img.pixels_mut() {
        let (s0, r) = lcg_next(state);
        let (s1, g) = lcg_next(s0);
        let (s2, b) = lcg_next(s1);
        state = s2;
        *pixel = image::Rgb([r, g, b]);
    }

    img
}

/// Write a seeded PNG to `dir/<filename>` and return the full path.
///
/// The image is always 64×64 pixels unless overridden by the caller via
/// `generate_image` directly.
pub fn write_png(dir: &Path, filename: &str, seed: u64) -> PathBuf {
    let path = dir.join(filename);
    let img = generate_image(seed, 64, 64);
    img.save_with_format(&path, ImageFormat::Png)
        .unwrap_or_else(|e| panic!("failed to write test image {filename}: {e}"));
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn same_seed_produces_same_pixels() {
        let a = generate_image(42, 8, 8);
        let b = generate_image(42, 8, 8);
        assert_eq!(a.into_raw(), b.into_raw());
    }

    #[test]
    fn different_seeds_produce_different_pixels() {
        let a = generate_image(1, 8, 8);
        let b = generate_image(2, 8, 8);
        assert_ne!(a.into_raw(), b.into_raw());
    }

    #[test]
    fn write_png_creates_file() {
        let dir = tempdir().unwrap();
        let path = write_png(dir.path(), "test.png", 99);
        assert!(path.exists());
        assert!(path.metadata().unwrap().len() > 0);
    }
}

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
fn generate_image(seed: u64, width: u32, height: u32) -> RgbImage {
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

/// Write a seeded PNG to `dir/<filename>` with given dimensions.
fn write_png(dir: &Path, filename: &str, seed: u64, width: u32, height: u32) -> PathBuf {
    let path = dir.join(filename);
    let img = generate_image(seed, width, height);
    img.save_with_format(&path, ImageFormat::Png)
        .unwrap_or_else(|e| panic!("failed to write {filename}: {e}"));
    path
}

fn main() {
    use std::env;

    let args: Vec<String> = env::args().collect();

    let mut dir: Option<String> = None;
    let mut count: usize = 200;
    let mut dupe_pct: u32 = 20;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--dir" => {
                i += 1;
                if i < args.len() {
                    dir = Some(args[i].clone());
                }
            }
            "--count" => {
                i += 1;
                if i < args.len() {
                    count = args[i].parse().unwrap_or(200);
                }
            }
            "--dupe-pct" => {
                i += 1;
                if i < args.len() {
                    dupe_pct = args[i].parse().unwrap_or(20);
                }
            }
            _ => {}
        }
        i += 1;
    }

    let dir = match dir {
        Some(d) => PathBuf::from(d),
        None => {
            eprintln!("Usage: cargo run --example igen -- --dir <path> [--count <n>] [--dupe-pct <pct>]");
            eprintln!("\nDefaults: --count 200 --dupe-pct 20");
            std::process::exit(1);
        }
    };

    if !dir.exists() {
        std::fs::create_dir_all(&dir).expect("failed to create output directory");
    }

    // Image size distribution: ~1/3 each (64x64, 256x256, 1024x1024)
    let sizes = [
        (64, 64),
        (256, 256),
        (1024, 1024),
    ];

    let num_dupes = (count as u32 * dupe_pct / 100) as usize;
    let num_unique = count - num_dupes;

    let mut generated = 0;

    // Generate unique images
    for i in 0..num_unique {
        let seed = (i as u64).wrapping_mul(7919); // Prime-based seed divergence
        let (width, height) = sizes[i % 3];
        let filename = format!("img_{:05}.png", i);
        write_png(&dir, &filename, seed, width, height);
        generated += 1;
    }

    // Generate duplicates (same seed as first num_dupes unique images, different filename)
    for i in 0..num_dupes {
        let seed = (i as u64).wrapping_mul(7919); // Same seed as corresponding unique image
        let (width, height) = sizes[i % 3];
        let filename = format!("img_{:05}_dup.png", i);
        write_png(&dir, &filename, seed, width, height);
        generated += 1;
    }

    println!("{}", dir.display());
    println!("{}", generated);
}

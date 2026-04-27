use crate::{db, hash};
use infer::{get_from_path, MatcherType};
use sqlx::SqlitePool;
use std::fs::read_dir;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::time::{interval, Duration};

#[derive(Debug, Clone)]
pub struct ScanStats {
    pub processed: usize,
    pub elapsed_secs: f64,
}

/// Controls which hash variants are computed during a scan.
///
/// The base exact hash of raw pixel data (`imgdata`) is always included for
/// whichever exact-hash algorithms are compiled in (see feature flags `xxh3`
/// and `sha256`).  All orientation variants are opt-in.
#[derive(Debug, Clone)]
pub struct ScanOptions {
    /// Include the 3 rotation variants (rot90, rot180, rot270)
    pub rotations: bool,
    /// Include the vertical flip and its 3 rotations
    pub flips: bool,
    /// Perceptual hash (average hash)
    pub phash: bool,
}

impl ScanOptions {
    /// Only the base exact hash (`imgdata`). Fastest; good for exact-duplicate detection.
    pub fn default() -> Self {
        Self {
            rotations: false,
            flips: false,
            phash: false,
        }
    }

    /// Base exact hash plus the 3 rotation variants — detects rotated exact duplicates.
    pub fn exact() -> Self {
        Self {
            rotations: true,
            flips: false,
            phash: false,
        }
    }

    /// Every hash variant: all 8 orientation hashes plus phash.
    pub fn all() -> Self {
        Self {
            rotations: true,
            flips: true,
            phash: true,
        }
    }
}

pub async fn process_path(path: PathBuf, recursive: bool, opts: &ScanOptions, pool: &SqlitePool) -> ScanStats {
    let mut stack: Vec<PathBuf> = Vec::new();
    match path.canonicalize() {
        Ok(path) => stack.push(path),
        Err(err) => {
            println!("Cannot process path due to err={}", err);
            return ScanStats {
                processed: 0,
                elapsed_secs: 0.0,
            };
        }
    }

    let start = Instant::now();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_for_task = Arc::clone(&counter);

    let progress_task = tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(10));
        ticker.tick().await; // skip the immediate first tick
        loop {
            ticker.tick().await;
            let elapsed = start.elapsed();
            println!("[progress] {} images processed so far in {:.2}s", counter_for_task.load(Ordering::Relaxed), elapsed.as_secs_f64());
        }
    });

    while let Some(curr) = stack.pop() {
        if curr.is_dir() {
            if recursive {
                for entry in read_dir(&curr)
                    .unwrap_or_else(|_| panic!("Failed to read contents of dir={:?}", &curr))
                {
                    match entry {
                        Ok(path_buf) => stack.push(path_buf.path()),
                        Err(err) => println!("Cannot process entry with err={}", err),
                    }
                }
            }
            continue;
        }

        let file_name = curr
            .to_str()
            .unwrap_or("cannot print path due to non-UTF8 chars");

        if is_img(&curr).unwrap_or(false) {
            if let Err(err) = db::clear_hashes_for_path(pool, &curr).await {
                println!("Cannot clear old hashes for {}: {}", file_name, err);
                continue;
            }

            // --- xxh3 exact hashes (compiled in when the `xxh3` feature is active) ---
            #[cfg(feature = "xxh3")]
            match hash::xxh3::selected_hashes_of_img_data(&curr, opts.rotations, opts.flips) {
                Ok(hashes) => {
                    for h in hashes {
                        if let Err(err) = db::save(pool, &h).await {
                            println!("Cannot save xxh3 hash for {}: {}", file_name, err);
                        }
                    }
                }
                Err(err) => println!("Cannot compute xxh3 for {}: {}", file_name, err),
            }

            // --- sha256 exact hashes (compiled in when the `sha256` feature is active) ---
            #[cfg(feature = "sha256")]
            match hash::sha256::selected_hashes_of_img_data(&curr, opts.rotations, opts.flips) {
                Ok(hashes) => {
                    for h in hashes {
                        if let Err(err) = db::save(pool, &h).await {
                            println!("Cannot save sha256 hash for {}: {}", file_name, err);
                        }
                    }
                }
                Err(err) => println!("Cannot compute sha256 for {}: {}", file_name, err),
            }

            // --- perceptual hash ---
            if opts.phash {
                match hash::phash::hash_path(&curr) {
                    Ok(ph) => {
                        if let Err(err) = db::save(pool, &ph).await {
                            println!("Cannot save phash for {}: {}", file_name, err);
                        }
                    }
                    Err(err) => println!("Cannot hash phash for {}: {}", file_name, err),
                }
            }

            counter.fetch_add(1, Ordering::Relaxed);
        } else {
            println!("skipping file={}", file_name);
        }
    }

    progress_task.abort();
    let elapsed = start.elapsed();
    let total = counter.load(Ordering::Relaxed);
    let elapsed_secs = elapsed.as_secs_f64();

    let stats = ScanStats {
        processed: total,
        elapsed_secs,
    };

    let imgs_per_sec = if elapsed_secs > 0.0 {
        total as f64 / elapsed_secs
    } else {
        0.0
    };

    println!(
        "Done. {} files scanned in {:.2}s ({:.2} imgs/sec).",
        total,
        elapsed_secs,
        imgs_per_sec
    );

    stats
}

fn is_img(path: &Path) -> Option<bool> {
    Some(get_from_path(path).ok()??.matcher_type() == MatcherType::Image)
}

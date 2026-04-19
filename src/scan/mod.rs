use crate::{db, hash};
use infer::{get_from_path, MatcherType};
use sqlx::SqlitePool;
use std::fs::read_dir;
use std::path::{Path, PathBuf};

/// Controls which hash variants are computed during a scan.
///
/// The base SHA-256 of raw pixel data (`imgdata`) is always included.
/// All other variants are opt-in.
#[derive(Debug, Clone)]
pub struct ScanOptions {
    /// SHA-256 of the 3 rotations (rot90, rot180, rot270)
    pub sha256_rotations: bool,
    /// SHA-256 of the vertical flip and its 3 rotations
    pub sha256_flips: bool,
    /// Perceptual hash (average hash)
    pub phash: bool,
}

impl ScanOptions {
    /// Only the base SHA-256 (`imgdata`). Fastest; good for exact-duplicate detection.
    pub fn default() -> Self {
        Self {
            sha256_rotations: false,
            sha256_flips: false,
            phash: false,
        }
    }

    /// Base SHA-256 plus the 3 rotation variants — detects rotated exact duplicates.
    /// No flip variants, no phash.
    pub fn exact() -> Self {
        Self {
            sha256_rotations: true,
            sha256_flips: false,
            phash: false,
        }
    }

    /// Every hash variant: all 8 SHA-256 orientations plus phash.
    pub fn all() -> Self {
        Self {
            sha256_rotations: true,
            sha256_flips: true,
            phash: true,
        }
    }
}

pub async fn process_path(path: PathBuf, recursive: bool, opts: &ScanOptions, pool: &SqlitePool) {
    let mut stack: Vec<PathBuf> = Vec::new();
    match path.canonicalize() {
        Ok(path) => stack.push(path),
        Err(err) => {
            println!("Cannot process path due to err={}", err);
            return;
        }
    }

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

            match hash::sha256::selected_hashes_of_img_data(
                &curr,
                opts.sha256_rotations,
                opts.sha256_flips,
            ) {
                Ok(shs) => {
                    for sh in shs {
                        if let Err(err) = db::save(pool, &sh).await {
                            println!("Cannot save sha256 hash for {}: {}", file_name, err);
                        }
                    }
                }
                Err(err) => println!("Cannot hash sha256 for {}: {}", file_name, err),
            }

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
        } else {
            println!("skipping file={}", file_name);
        }
    }
}

fn is_img(path: &Path) -> Option<bool> {
    Some(get_from_path(path).ok()??.matcher_type() == MatcherType::Image)
}

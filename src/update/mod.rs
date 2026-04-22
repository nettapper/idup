use crate::{db, hash};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::time::{interval, Duration};

#[derive(Debug, Clone)]
pub struct UpdateStats {
    pub verified: usize,
    pub updated: usize,
    pub missing: usize,
    pub cleaned: usize,
    pub total: usize,
    pub elapsed_secs: f64,
}

pub async fn process_update(path: Option<PathBuf>, cleanup: bool, pool: &SqlitePool) -> UpdateStats {
    let filter_path = path.as_ref().map(|p| {
        p.canonicalize()
            .unwrap_or_else(|_| p.clone())
            .to_string_lossy()
            .to_string()
    });

    // Get all images from DB (filtered by path if provided)
    let images = match db::get_all_images(pool, filter_path.as_deref()).await {
        Ok(imgs) => imgs,
        Err(err) => {
            eprintln!("Error fetching images from database: {}", err);
            return UpdateStats {
                verified: 0,
                updated: 0,
                missing: 0,
                cleaned: 0,
                total: 0,
                elapsed_secs: 0.0,
            };
        }
    };

    if images.is_empty() {
        println!("No images found in database.");
        return UpdateStats {
            verified: 0,
            updated: 0,
            missing: 0,
            cleaned: 0,
            total: 0,
            elapsed_secs: 0.0,
        };
    }

    let start = Instant::now();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_for_task = Arc::clone(&counter);

    // Progress reporting task (same as scan, every 10 seconds)
    let progress_task = tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(10));
        ticker.tick().await; // skip the immediate first tick
        loop {
            ticker.tick().await;
            let elapsed = start.elapsed();
            println!(
                "[progress] {} images processed so far in {:.2}s",
                counter_for_task.load(Ordering::Relaxed),
                elapsed.as_secs_f64()
            );
        }
    });

    let mut updated_count = 0;
    let mut verified_count = 0;
    let mut missing_count = 0;
    let mut cleaned_count = 0;

    for img in images {
        let path = Path::new(&img.path);

        // Check if file exists
        if !path.exists() {
            missing_count += 1;
            eprintln!("File not found: {}", img.path);

            if cleanup {
                match db::delete_image(pool, &img.path).await {
                    Ok(_) => {
                        cleaned_count += 1;
                    }
                    Err(err) => {
                        eprintln!("Error deleting image from DB: {}", err);
                    }
                }
            }

            counter.fetch_add(1, Ordering::Relaxed);
            continue;
        }

        // Compute base SHA-256 hash
        let base_sha256 = match hash::sha256::hash_path(path) {
            Ok(h) => h,
            Err(err) => {
                eprintln!("Error computing SHA-256 for {}: {}", img.path, err);
                counter.fetch_add(1, Ordering::Relaxed);
                continue;
            }
        };

        // Get the current base SHA-256 from DB
        let db_base_hash = match db::get_single_hash(pool, &img.path, "sha256 imgdata").await {
            Ok(Some(h)) => h,
            Ok(None) => {
                eprintln!("No base SHA-256 found in DB for: {}", img.path);
                counter.fetch_add(1, Ordering::Relaxed);
                continue;
            }
            Err(err) => {
                eprintln!("Error fetching hash from DB: {}", err);
                counter.fetch_add(1, Ordering::Relaxed);
                continue;
            }
        };

        // Check if hashes match
        if base_sha256.hash == db_base_hash {
            // Hashes match, file is verified
            verified_count += 1;
        } else {
            // Hash mismatch: recompute ALL hash kinds that exist in DB for this image
            match recompute_all_hashes(pool, path, &img.path).await {
                Ok(count) => {
                    updated_count += count;
                }
                Err(err) => {
                    eprintln!("Error recomputing hashes for {}: {}", img.path, err);
                }
            }
        }

        counter.fetch_add(1, Ordering::Relaxed);
    }

    progress_task.abort();
    let elapsed = start.elapsed();
    let total = counter.load(Ordering::Relaxed);
    let elapsed_secs = elapsed.as_secs_f64();

    let stats = UpdateStats {
        verified: verified_count,
        updated: updated_count,
        missing: missing_count,
        cleaned: cleaned_count,
        total,
        elapsed_secs,
    };

    // Print stats to stdout
    println!();
    println!("Done. {} files checked in {:.2}s.", total, elapsed_secs);
    println!("  Verified: {}", verified_count);
    println!("  Updated:  {}", updated_count);
    println!("  Missing:  {}", missing_count);
    if cleanup {
        println!("  Cleaned:  {}", cleaned_count);
    }

    stats
}

/// Recompute all hash kinds that exist in the DB for the given image.
/// Returns the count of hashes that were updated.
async fn recompute_all_hashes(
    pool: &SqlitePool,
    file_path: &Path,
    db_path: &str,
) -> Result<usize, Box<dyn std::error::Error>> {
    // Get all hash kinds stored for this image
    let hash_kinds = db::get_hash_kinds_for_image(pool, db_path).await?;

    let mut count = 0;
    let mut sha256_processed = false;

    for kind in &hash_kinds {
        if kind == "phash" {
            // Recompute perceptual hash
            match hash::phash::hash_path(file_path) {
                Ok(ph) => {
                    db::save(pool, &ph).await?;
                    count += 1;
                }
                Err(err) => {
                    eprintln!("Error computing phash for {}: {}", db_path, err);
                }
            }
        } else if kind.starts_with("sha256") && !sha256_processed {
            // This is a SHA-256 variant, we need to recompute all SHA-256 variants
            // based on the kinds that exist in the DB
            // We'll do this only once per file to avoid recomputing multiple times

            // Collect all sha256 kinds
            let sha256_kinds: Vec<_> = hash_kinds
                .iter()
                .filter(|k| k.starts_with("sha256"))
                .collect();

            if sha256_kinds.is_empty() {
                continue;
            }

            // Determine which options were used originally
            let needs_rotations = sha256_kinds.iter().any(|k| k.contains("rot90") || k.contains("rot180") || k.contains("rot270"));
            let needs_flips = sha256_kinds.iter().any(|k| k.contains("flipv") || k.contains("fliph"));

            // Recompute SHA-256 hashes with the original options
            match hash::sha256::selected_hashes_of_img_data(file_path, needs_rotations, needs_flips) {
                Ok(shs) => {
                    for sh in shs {
                        db::save(pool, &sh).await?;
                        count += 1;
                    }
                }
                Err(err) => {
                    eprintln!("Error computing SHA-256 for {}: {}", db_path, err);
                }
            }

            // Only process SHA-256 once per file
            sha256_processed = true;
        }
    }

    Ok(count)
}

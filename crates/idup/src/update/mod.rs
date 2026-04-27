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

        // Verify the image against whatever base hashes exist in the DB.
        // We check each active algorithm independently and consider the image
        // verified only if every present base hash matches.
        let verified = verify_and_maybe_update(pool, path, &img.path, &mut updated_count).await;
        if verified {
            verified_count += 1;
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

/// Check a single image against all base hashes stored in the DB.
/// If any algorithm's hash has changed, recompute all hash kinds for that
/// algorithm and update the DB.
///
/// Returns `true` if every present base hash matched (image unchanged).
async fn verify_and_maybe_update(
    pool: &SqlitePool,
    file_path: &Path,
    db_path: &str,
    updated_count: &mut usize,
) -> bool {
    let mut all_verified = true;

    // --- xxh3 ---
    #[cfg(feature = "xxh3")]
    {
        if let Some(verified) =
            check_and_update_xxh3(pool, file_path, db_path, updated_count).await
        {
            if !verified {
                all_verified = false;
            }
        }
        // None means no xxh3 base hash existed in DB — skip silently.
    }

    // --- sha256 ---
    #[cfg(feature = "sha256")]
    {
        if let Some(verified) =
            check_and_update_sha256(pool, file_path, db_path, updated_count).await
        {
            if !verified {
                all_verified = false;
            }
        }
        // None means no sha256 base hash existed in DB — skip silently.
    }

    // --- phash (always present if it was scanned) ---
    if let Err(err) = maybe_update_phash(pool, file_path, db_path, updated_count).await {
        eprintln!("Error handling phash for {}: {}", db_path, err);
    }

    all_verified
}

/// Returns `Some(true)` if xxh3 base hash matched, `Some(false)` if it changed
/// (and hashes were recomputed), or `None` if no xxh3 base hash was in the DB.
#[cfg(feature = "xxh3")]
async fn check_and_update_xxh3(
    pool: &SqlitePool,
    file_path: &Path,
    db_path: &str,
    updated_count: &mut usize,
) -> Option<bool> {
    let db_hash = db::get_single_hash(pool, db_path, "xxh3 imgdata").await.ok()??;

    // Recompute base xxh3 from pixel data
    let raw_bytes = image::ImageReader::open(file_path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?
        .into_bytes();
    let current = hash::xxh3::hash(&raw_bytes);

    if current == db_hash {
        return Some(true);
    }

    // Hash changed — recompute all xxh3 orientation variants present in DB
    let hash_kinds = db::get_hash_kinds_for_image(pool, db_path).await.ok()?;
    let needs_rotations = hash_kinds.iter().any(|k| k.starts_with("xxh3") && k.contains("rot"));
    let needs_flips = hash_kinds.iter().any(|k| k.starts_with("xxh3") && k.contains("flip"));

    match hash::xxh3::selected_hashes_of_img_data(file_path, needs_rotations, needs_flips) {
        Ok(hashes) => {
            for h in hashes {
                if let Err(err) = db::save(pool, &h).await {
                    eprintln!("Error saving xxh3 hash for {}: {}", db_path, err);
                } else {
                    *updated_count += 1;
                }
            }
        }
        Err(err) => eprintln!("Error recomputing xxh3 for {}: {}", db_path, err),
    }

    Some(false)
}

/// Returns `Some(true)` if sha256 base hash matched, `Some(false)` if it changed
/// (and hashes were recomputed), or `None` if no sha256 base hash was in the DB.
#[cfg(feature = "sha256")]
async fn check_and_update_sha256(
    pool: &SqlitePool,
    file_path: &Path,
    db_path: &str,
    updated_count: &mut usize,
) -> Option<bool> {
    let db_hash = db::get_single_hash(pool, db_path, "sha256 imgdata").await.ok()??;

    // Recompute base sha256 from pixel data
    let raw_bytes = image::ImageReader::open(file_path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?
        .into_bytes();
    let current = hash::sha256::hash(raw_bytes);

    if current == db_hash {
        return Some(true);
    }

    // Hash changed — recompute all sha256 orientation variants present in DB
    let hash_kinds = db::get_hash_kinds_for_image(pool, db_path).await.ok()?;
    let needs_rotations = hash_kinds.iter().any(|k| k.starts_with("sha256") && k.contains("rot"));
    let needs_flips = hash_kinds.iter().any(|k| k.starts_with("sha256") && k.contains("flip"));

    match hash::sha256::selected_hashes_of_img_data(file_path, needs_rotations, needs_flips) {
        Ok(hashes) => {
            for h in hashes {
                if let Err(err) = db::save(pool, &h).await {
                    eprintln!("Error saving sha256 hash for {}: {}", db_path, err);
                } else {
                    *updated_count += 1;
                }
            }
        }
        Err(err) => eprintln!("Error recomputing sha256 for {}: {}", db_path, err),
    }

    Some(false)
}

/// If a phash exists in the DB for this image, verify it is still current.
/// (pHash doesn't change unless the image content itself changes, so we only
/// recompute it when one of the exact-hash checks already found a mismatch —
/// for simplicity here we just leave it as-is; phash is checked separately
/// via `idup compare`.)
async fn maybe_update_phash(
    pool: &SqlitePool,
    file_path: &Path,
    db_path: &str,
    updated_count: &mut usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let hash_kinds = db::get_hash_kinds_for_image(pool, db_path).await?;
    if !hash_kinds.iter().any(|k| k == "phash") {
        return Ok(());
    }

    match hash::phash::hash_path(file_path) {
        Ok(ph) => {
            db::save(pool, &ph).await?;
            *updated_count += 1;
        }
        Err(err) => eprintln!("Error computing phash for {}: {}", db_path, err),
    }

    Ok(())
}

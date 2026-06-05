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
    /// Unzip zip files and scan their contents
    pub unzip: bool,
    /// Remove the zip archive after successful extraction
    pub remove_archive: bool,
}

impl ScanOptions {
    /// Only the base exact hash (`imgdata`). Fastest; good for exact-duplicate detection.
    pub fn default() -> Self {
        Self {
            rotations: false,
            flips: false,
            phash: false,
            unzip: false,
            remove_archive: false,
        }
    }

    /// Base exact hash plus the 3 rotation variants — detects rotated exact duplicates.
    pub fn exact() -> Self {
        Self {
            rotations: true,
            flips: false,
            phash: false,
            unzip: false,
            remove_archive: false,
        }
    }

    /// Every hash variant: all 8 orientation hashes plus phash.
    pub fn all() -> Self {
        Self {
            rotations: true,
            flips: true,
            phash: true,
            unzip: false,
            remove_archive: false,
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
            // Always enumerate direct children of a directory.
            for entry in read_dir(&curr)
                .unwrap_or_else(|_| panic!("Failed to read contents of dir={:?}", &curr))
            {
                match entry {
                    Ok(path_buf) => {
                        let child = path_buf.path();
                        // When not recursive, only push immediate files.
                        if recursive || child.is_file() {
                            stack.push(child);
                        }
                    }
                    Err(err) => println!("Cannot process entry with err={}", err),
                }
            }
            continue;
        }

        if opts.unzip && is_zip(&curr) {
            let dest_dir = get_non_colliding_sibling_dir(&curr);
            println!("Extracting {:?} to {:?}", curr, dest_dir);
            match unzip_file(&curr, &dest_dir) {
                Ok(()) => {
                    stack.push(dest_dir);
                    if opts.remove_archive {
                        println!("Removing archive {:?}", curr);
                        if let Err(err) = std::fs::remove_file(&curr) {
                            println!("Failed to remove archive {:?}: {}", curr, err);
                        }
                    }
                }
                Err(err) => {
                    println!("Failed to unzip {:?}: {}", curr, err);
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

fn is_zip(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
}

fn get_non_colliding_sibling_dir(zip_path: &Path) -> PathBuf {
    let parent = zip_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = zip_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("extracted_zip");

    let dest_dir = parent.join(stem);
    if !dest_dir.exists() {
        return dest_dir;
    }

    let mut counter = 1;
    loop {
        let candidate = parent.join(format!("{}_{}", stem, counter));
        if !candidate.exists() {
            return candidate;
        }
        counter += 1;
    }
}

fn unzip_file(zip_path: &Path, dest_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs::{create_dir_all, File};
    use std::io;
    use zip::ZipArchive;

    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;

    // Detect if there is a single top-level directory root
    let single_root = get_zip_single_root_dir(&mut archive);

    create_dir_all(dest_dir)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = match file.enclosed_name() {
            Some(path) => path.to_owned(),
            None => continue,
        };

        // If there is a single root, strip the root prefix from outpath
        let final_path = if let Some(ref root) = single_root {
            if let Ok(stripped) = outpath.strip_prefix(root) {
                if stripped.as_os_str().is_empty() {
                    continue;
                }
                stripped.to_owned()
            } else {
                outpath
            }
        } else {
            outpath
        };

        let target_path = dest_dir.join(final_path);
        if file.is_dir() {
            create_dir_all(&target_path)?;
        } else {
            if let Some(p) = target_path.parent() {
                if !p.exists() {
                    create_dir_all(p)?;
                }
            }
            let mut outfile = File::create(&target_path)?;
            io::copy(&mut file, &mut outfile)?;
        }
    }

    Ok(())
}

fn get_zip_single_root_dir(archive: &mut zip::ZipArchive<std::fs::File>) -> Option<String> {
    let mut root_dir: Option<String> = None;
    for i in 0..archive.len() {
        let file = match archive.by_index(i) {
            Ok(f) => f,
            Err(_) => return None,
        };
        let outpath = match file.enclosed_name() {
            Some(path) => path.to_owned(),
            None => continue,
        };

        // Get the first component of the path
        let first_component = outpath
            .components()
            .next()
            .and_then(|c| c.as_os_str().to_str())
            .map(|s| s.to_string())?;

        // Check if there are more components or if the entry is a directory
        let is_dir = file.is_dir();
        let mut components = outpath.components();
        components.next();
        let has_more = components.next().is_some();

        if !has_more && !is_dir {
            // It's a file at the root level, so no single root dir
            return None;
        }

        if let Some(ref root) = root_dir {
            if root != &first_component {
                return None;
            }
        } else {
            root_dir = Some(first_component);
        }
    }
    root_dir
}

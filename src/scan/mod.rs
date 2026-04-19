use crate::{db, hash};
use infer::{get_from_path, MatcherType};
use sqlx::SqlitePool;
use std::fs::read_dir;
use std::path::{Path, PathBuf};

pub async fn process_path(path: PathBuf, recursive: bool, pool: &SqlitePool) {
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

            match hash::sha256::all_hashes_of_img_data(&curr) {
                Ok(shs) => {
                    for sh in shs {
                        if let Err(err) = db::save(pool, &sh).await {
                            println!("Cannot save sha256 hash for {}: {}", file_name, err);
                        }
                    }
                }
                Err(err) => println!("Cannot hash sha256 for {}: {}", file_name, err),
            }

            match hash::phash::hash_path(&curr) {
                Ok(ph) => {
                    if let Err(err) = db::save(pool, &ph).await {
                        println!("Cannot save phash for {}: {}", file_name, err);
                    }
                }
                Err(err) => println!("Cannot hash phash for {}: {}", file_name, err),
            }
        } else {
            println!("skipping file={}", file_name);
        }
    }
}

fn is_img(path: &Path) -> Option<bool> {
    Some(get_from_path(path).ok()??.matcher_type() == MatcherType::Image)
}

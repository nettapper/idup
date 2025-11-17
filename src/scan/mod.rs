use crate::db;
use crate::hash;
use infer::{get_from_path, MatcherType};
use log::{debug, error, info, warn};
use std::fs::read_dir;
use std::path::{Path, PathBuf};

pub fn process_path(path: PathBuf, recursive: bool) {
    let mut stack: Vec<PathBuf> = Vec::new();
    // SAFETY: all paths passed to db::save need to be absolute
    stack.push(path.canonicalize().unwrap());

    while !stack.is_empty() {
        let curr = stack.pop().expect("Failed to process item becuase the stack is empty");
        if curr.is_dir() {
            if recursive {
                for entry in read_dir(&curr).unwrap_or_else(|_| panic!("Failed to read contents of dir={:?}", &curr)) {
                    match entry {
                        Ok(path_buf) => stack.push(path_buf.path()),
                        Err(err) => warn!("Cannot process entry with err={}", err),
                    }
                }
            }
        } else {
            let file_name = curr.to_str().unwrap_or("cannot print path due to non-UTF8 chars");
            // TODO mv this to a seperate fn
            if is_img(&curr).unwrap_or(false) {
                let shs = hash::sha256::all_hashes_of_img_data(&curr).unwrap();
                let ph = hash::phash::hash_path(&curr).unwrap();
                info!("file={} sha256s={:?} phash={:?}", file_name, shs, ph);
                for sh in shs {
                    if let Err(e) = db::save(&sh) {
                        error!("Failed to save sha256 hash for {}: {}", file_name, e);
                    }
                }
                if let Err(e) = db::save(&ph) {
                    error!("Failed to save phash for {}: {}", file_name, e);
                }
            } else {
                debug!("skipping file={}", file_name);
            }
        }
    }
}

fn is_img(path: &Path) -> Option<bool> {
    Some(get_from_path(path).ok()??.matcher_type() == MatcherType::Image)
}

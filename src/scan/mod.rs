use std::path::{Path, PathBuf};
use std::fs::read_dir;
use infer::{get_from_path, MatcherType};
use crate::hash;
use crate::db;

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
                        Err(err) => println!("Cannot process entry with err={}", err),
                    }
                }
            }
        } else {
            let file_name = curr.to_str().unwrap_or("cannot print path due to non-UTF8 chars");
            // TODO mv this to a seperate fn
            if is_img(&curr).unwrap_or(false) {
                // TODO save all hashes
                // println!("{:?}", hash::sha256::all_hashes_of_img_data(&curr).unwrap());
                let sh = hash::sha256::hash_path(&curr).unwrap();
                let ph = hash::phash::hash_path(&curr).unwrap();
                // TODO can i use some logging lib everywhere?
                println!("file={} sha256={:?} phash={:?}", file_name, sh, ph);
                db::save(&sh);
                db::save(&ph);
            } else {
                println!("skipping file={}", file_name);
            }
        }
    }
}

fn is_img(path: &Path) -> Option<bool> {
    Some(get_from_path(path).ok()??
        .matcher_type() == MatcherType::Image)
}

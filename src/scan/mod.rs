use std::path::PathBuf;
use std::fs::read_dir;
use infer::{get_from_path, MatcherType};

use crate::hash;

pub fn process_path(path: PathBuf, recursive: bool) {
    let mut stack: Vec<PathBuf> = Vec::new();
    stack.push(path);

    while !stack.is_empty() {
        let curr = stack.pop().expect("Failed to process item becuase the stack is empty");
        if curr.is_dir() {
            if recursive {
                for entry in read_dir(&curr).expect(format!("Failed to read contents of dir={:?}", &curr).as_str()) {
                    match entry {
                        Ok(path_buf) => stack.push(path_buf.path()),
                        Err(err) => println!("Cannot process entry with err={}", err),
                    }
                }
            }
        } else {
            let file_name = curr.to_str().unwrap_or("cannot print path due to non-UTF8 chars");
            if is_img(&curr).unwrap_or(false) {
                let sh = hash::sha256::hash_path(&curr);
                let ph = hash::phash::hash_path(&curr);
                println!("file={} sha256={} phash={}", file_name, sh, ph);
            } else {
                println!("skipping file={}", file_name);
            }
        }
    }
}

fn is_img(path: &PathBuf) -> Option<bool> {
    return Some(get_from_path(path).ok()??
        .matcher_type() == MatcherType::Image);

}

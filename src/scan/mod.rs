use std::path::PathBuf;
use std::fs::{metadata, read, read_dir};

use crate::hash;

pub fn process_path(path: PathBuf, recursive: bool) {
    let meta = metadata(&path).expect("Failed to look up file metadata");
    if meta.is_dir() {
        let mut stack: Vec<PathBuf> = Vec::new();
        stack.push(path);

        while !stack.is_empty() {
            let curr_dir = stack.pop().unwrap();
            for entry in read_dir(&curr_dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    println!("dir={}", path.to_str().unwrap_or("cannot print path due to non-UTF8 chars"));
                    if recursive {
                        stack.push(path);
                    }
                } else {
                    println!("file={}", path.to_str().unwrap_or("cannot print path due to non-UTF8 chars"));
                    let data = read(&path).expect("Failed to open the file for the sha256 hash");
                    let sh = hash::sha256::hash(data);
                    println!("\tsha256: {}", sh);
                }
            }
        }
    } else {
        println!("file={}", path.to_str().unwrap_or("cannot print path due to non-UTF8 chars"));
    }
}

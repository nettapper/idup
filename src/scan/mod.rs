use std::path::PathBuf;
use std::fs::{metadata, read, read_dir};

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
            println!("processing file={}", curr.to_str().unwrap_or("cannot print path due to non-UTF8 chars"));
            let data = read(&curr).expect("Failed to open the file for the sha256 hash");
            let sh = hash::sha256::hash(data);
            println!("\tsha256: {}", sh);
        }
    }
}

// fn is_img(path: PathBuf) {
//     let meta = metadata(&path).expect("Failed to look up file metadata");
//     let ft = meta.file_type();
// }

use super::ImgHash;
use super::ImgHashKind;
use image::{ImageError, ImageReader};
use std::path::Path;
use xxhash_rust::xxh3::xxh3_64;

// NOTE: hashing the bytes from a DynamicImage isn't the same as
// hashing the bytes from a file on disk
pub fn selected_hashes_of_img_data(
    path: &Path,
    include_rotations: bool,
    include_flips: bool,
) -> Result<Vec<ImgHash>, ImageError> {
    let img = ImageReader::open(path)?.with_guessed_format()?.decode()?;
    let mut results = Vec::new();

    results.push(ImgHash {
        path: path.to_path_buf(),
        kind: ImgHashKind::Xxh3("imgdata".to_string()),
        hash: hash(img.as_bytes()),
    });

    if include_rotations {
        results.push(ImgHash {
            path: path.to_path_buf(),
            kind: ImgHashKind::Xxh3("imgdata rot90".to_string()),
            hash: hash(img.rotate90().as_bytes()),
        });
        results.push(ImgHash {
            path: path.to_path_buf(),
            kind: ImgHashKind::Xxh3("imgdata rot180".to_string()),
            hash: hash(img.rotate180().as_bytes()),
        });
        results.push(ImgHash {
            path: path.to_path_buf(),
            kind: ImgHashKind::Xxh3("imgdata rot270".to_string()),
            hash: hash(img.rotate270().as_bytes()),
        });
    }

    if include_flips {
        let flipped = img.flipv();
        results.push(ImgHash {
            path: path.to_path_buf(),
            kind: ImgHashKind::Xxh3("imgdata flipv".to_string()),
            hash: hash(flipped.as_bytes()),
        });
        results.push(ImgHash {
            path: path.to_path_buf(),
            kind: ImgHashKind::Xxh3("imgdata flipv rot90".to_string()),
            hash: hash(flipped.rotate90().as_bytes()),
        });
        results.push(ImgHash {
            path: path.to_path_buf(),
            kind: ImgHashKind::Xxh3("imgdata flipv rot180".to_string()),
            hash: hash(flipped.rotate180().as_bytes()),
        });
        results.push(ImgHash {
            path: path.to_path_buf(),
            kind: ImgHashKind::Xxh3("imgdata flipv rot270".to_string()),
            hash: hash(flipped.rotate270().as_bytes()),
        });
    }

    Ok(results)
}

pub fn hash(data: &[u8]) -> String {
    format!("{:x}", xxh3_64(data))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Reference values generated with the xxHash3 reference implementation.
    // $ echo -n "" | xxh3sum            -> 2d06800538d394c2
    // $ echo -n "abc" | xxh3sum         -> 78af5f94892f3950

    #[test]
    fn test_empty() {
        let data: &[u8] = b"";
        assert_eq!(hash(data), "2d06800538d394c2");
    }

    #[test]
    fn test_abc() {
        let data: &[u8] = b"abc";
        assert_eq!(hash(data), "78af5f94892f3950");
    }
}

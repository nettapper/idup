use super::ImgHash;
use super::ImgHashKind;
use image::{ImageError, ImageReader};
use sha2::{Digest, Sha256};
use std::fs::read;
use std::path::Path;

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
        kind: ImgHashKind::Sha256("imgdata".to_string()),
        hash: hash(img.clone().into_bytes()),
    });

    if include_rotations {
        results.push(ImgHash {
            path: path.to_path_buf(),
            kind: ImgHashKind::Sha256("imgdata rot90".to_string()),
            hash: hash(img.rotate90().into_bytes()),
        });
        results.push(ImgHash {
            path: path.to_path_buf(),
            kind: ImgHashKind::Sha256("imgdata rot180".to_string()),
            hash: hash(img.rotate180().into_bytes()),
        });
        results.push(ImgHash {
            path: path.to_path_buf(),
            kind: ImgHashKind::Sha256("imgdata rot270".to_string()),
            hash: hash(img.rotate270().into_bytes()),
        });
    }

    if include_flips {
        results.push(ImgHash {
            path: path.to_path_buf(),
            kind: ImgHashKind::Sha256("imgdata flipv".to_string()),
            hash: hash(img.flipv().into_bytes()),
        });
        results.push(ImgHash {
            path: path.to_path_buf(),
            kind: ImgHashKind::Sha256("imgdata flipv rot90".to_string()),
            hash: hash(img.flipv().rotate90().into_bytes()),
        });
        results.push(ImgHash {
            path: path.to_path_buf(),
            kind: ImgHashKind::Sha256("imgdata flipv rot180".to_string()),
            hash: hash(img.flipv().rotate180().into_bytes()),
        });
        results.push(ImgHash {
            path: path.to_path_buf(),
            kind: ImgHashKind::Sha256("imgdata flipv rot270".to_string()),
            hash: hash(img.flipv().rotate270().into_bytes()),
        });
    }

    Ok(results)
}

// NOTE: hashing the bytes from a DynamicImage isn't the same as
// hashing the bytes from a file on disk
pub fn hash_path(path: &Path) -> Result<ImgHash, std::io::Error> {
    let data = read(path)?;
    Ok(ImgHash {
        path: path.to_path_buf(),
        kind: ImgHashKind::Sha256("sha256".to_string()),
        hash: hash(data),
    })
}

pub fn hash(data: Vec<u8>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let result = hasher.finalize();
    format!("{:x}", result)
}

#[cfg(test)]
mod tests {
    use super::*;

    // NOTE: be careful when generating test data as EOL will change the hash
    // echo -n  "abc" | sha256sum

    #[test]
    fn test_empty() {
        let data = String::from("").into_bytes();
        assert_eq!(
            hash(data),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_abcs() {
        let data = String::from("abc").into_bytes();
        assert_eq!(
            hash(data),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn test_abcs_repeat() {
        let data = String::from("aaaabbbbcccc").into_bytes();
        assert_eq!(
            hash(data),
            "11c85195ae99540ac07f80e2905e6e39aaefc4ac94cd380f366e79ba83560566"
        );
    }

    #[test]
    fn test_multiple_chunks() {
        // 10 1's, then 10 2's, then ..., then 10 7's
        // 70 byes = 70 * 8 = 560 bits > 512 bit chunk size
        let data =
            String::from("1111111111222222222233333333334444444444555555555566666666667777777777")
                .into_bytes();
        assert_eq!(
            hash(data),
            "7c3bfca2e1355c1dd2c1343e490625b4a59a5c0aefb9d2177a55a6f5d464f369"
        );
    }

    #[test]
    fn test_exactly_one_chunk() {
        // 512 bits (chunk size) - 64 bits (for the u64 len) - 8 bits (for the append 1 bit) = 440 bits
        // so I'll add 440 / 8 = 55 ascii a's
        let data: Vec<u8> = [97].repeat(55);
        assert_eq!(
            hash(data),
            "9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318"
        );
    }
}

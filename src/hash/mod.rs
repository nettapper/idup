use std::fmt;
use std::num::ParseIntError;
use std::path::PathBuf;

pub mod phash;
pub mod sha256;

#[derive(Debug)]
pub enum ImgHashKind {
    Phash,
    Sha256(String), // this describes the rotation & flip performed on the image
}

impl fmt::Display for ImgHashKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ImgHashKind::Phash => write!(f, "phash"),
            ImgHashKind::Sha256(s) => write!(f, "sha256 {}", s),
        }
    }
}

#[derive(Debug)]
pub struct ImgHash {
    pub path: PathBuf,
    pub kind: ImgHashKind,
    pub hash: String,
}

// counts the number of bits that are different using the hash as a number
pub fn hamming_dist(a: ImgHash, b: ImgHash) -> Result<u8, ParseIntError> {
    let x: u64 = a.hash.parse()?;
    let y: u64 = b.hash.parse()?;
    Ok(hamming_dist_internal(x, y))
}

// counts the number of bits that are different
fn hamming_dist_internal(mut a: u64, mut b: u64) -> u8 {
    let mut count: u8 = 0;
    while a > 0 && b > 0 {
        if a & 1 != b & 1 {
            count += 1;
        }
        a >>= 1; // div by 2
        b >>= 1; // div by 2
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hamming_dist_same() {
        let x = 0x8f8f978589f9f1c0;
        assert_eq!(hamming_dist_internal(x, x), 0);
    }

    #[test]
    fn hamming_dist_off_by_one() {
        let x = 0x8f8f978589f9f1c0; // last 4 bits are 0's
        let y = x + 1;
        assert_eq!(hamming_dist_internal(x, y), 1);
        let z = x + 8; // any pow of 2 should only change on bit (assuming no carry bit)
        assert_eq!(hamming_dist_internal(x, z), 1);
    }
}

pub mod phash;
pub mod sha512;

// counts the number of bits that are different
pub fn hamming_dist(mut a: u64, mut b: u64) -> u8 {
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
        assert_eq!(hamming_dist(x, x), 0);
    }

    #[test]
    fn hamming_dist_off_by_one() {
        let x = 0x8f8f978589f9f1c0; // last 4 bits are 0's
        let y = x + 1;
        assert_eq!(hamming_dist(x, y), 1);
        let z = x + 8; // any pow of 2 should only change on bit (assming no carry bit)
        assert_eq!(hamming_dist(x, z), 1);
    }
}

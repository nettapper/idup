use super::ImgHash;
use super::ImgHashKind;
use image::{ImageError, ImageReader};
use std::fs::read;
use std::path::Path;

// NOTE: hashing the bytes from a DynamicImage isn't the same as
// hashing the bytes from a file on disk
pub fn all_hashes_of_img_data(path: &Path) -> Result<Vec<ImgHash>, ImageError> {
    let img = ImageReader::open(path)?.with_guessed_format()?.decode()?;
    let mut results = Vec::new();

    results.push(ImgHash {
        path: path.to_path_buf(),
        kind: ImgHashKind::Sha256("imgdata".to_string()),
        hash: hash(img.clone().into_bytes()),
    });
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

// Pseudocode taken from [Wikipedia](https://en.wikipedia.org/wiki/SHA-2#Pseudocode)
pub fn hash(mut data: Vec<u8>) -> String {
    // Note 1: All variables are 32 bit unsigned integers and addition is calculated modulo 2^32
    // Note 2: For each round, there is one round constant k[i] and one entry in the message schedule array w[i], 0 ≤ i ≤ 63
    // Note 3: The compression function uses 8 working variables, a through h
    // Note 4: Big-endian convention is used when expressing the constants in this pseudocode,
    // and when parsing message block data from bytes to words, for example,
    // the first word of the input message "abc" after padding is 0x61626380

    // TODO do I need to u32::from_be(0x6a...) for all constants?

    // Initialize hash values:
    // (first 32 bits of the fractional parts of the square roots of the first 8 primes 2..19):
    let mut h0: u32 = 0x6a09e667;
    let mut h1: u32 = 0xbb67ae85;
    let mut h2: u32 = 0x3c6ef372;
    let mut h3: u32 = 0xa54ff53a;
    let mut h4: u32 = 0x510e527f;
    let mut h5: u32 = 0x9b05688c;
    let mut h6: u32 = 0x1f83d9ab;
    let mut h7: u32 = 0x5be0cd19;

    // Initialize array of round constants:
    // (first 32 bits of the fractional parts of the cube roots of the first 64 primes 2..311):
    #[rustfmt::skip]
    let k: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];

    // println!("pre data: {:?}", data);
    // Pre-processing (Padding):
    // begin with the original message of length L bits
    // TODO: is there a better way than hardcoding number of bits in u8?
    let l: u64 = data.len() as u64 * 8;
    // println!("l: {:?}", l);
    // append a single '1' bit
    data.push(1_u8 << 7);
    // append K '0' bits, where K is the minimum number >= 0 such that L + 1 + K + 64 is a multiple of 512
    let mut j = 512 - ((l + 64 + 8) % 512); // var 'j' to not shadow k, +8 for u8 above
    if j == 512 {
        j = 0;
    } // case where no other zeros are needed
      // println!("j (K): {:?}", j);
    while j >= 8 {
        data.push(0);
        j = j.saturating_sub(8);
    }
    // append L as a 64-bit big-endian integer, making the total post-processed length a multiple of 512 bits
    data.extend_from_slice(&l.to_be_bytes());
    // println!("post data (len {:?}): {:?}", data.len(), data);
    assert_eq!((data.len() * 8) % 512, 0);
    // such that the bits in the message are L 1 00..<K 0's>..00 <L as 64 bit integer> = k*512 total bits

    // Process the message in successive 512-bit chunks:
    // break message into 512-bit chunks
    let num_of_u8s = 512 / 8;
    // for each chunk
    for chunk in data.chunks(num_of_u8s) {
        // println!("chunk (len {:?}): {:?}", chunk.len(), chunk);
        // each chunk should be 64 u8 = 16 u32
        assert_eq!(chunk.len(), 64);
        // create a 64-entry message schedule array w[0..63] of 32-bit words
        // (The initial values in w[0..63] don't matter, so many implementations zero them here)
        let mut w: [u32; 64] = [0; 64];
        // copy current chunk into first 16 words w[0..15] of the message schedule array
        for (i, arr) in (0..).zip(chunk.chunks(4)) {
            // TODO ensure word is big endian
            let word: u32 =
                ((arr[0] as u32) << 24) + ((arr[1] as u32) << 16) + ((arr[2] as u32) << 8) + (arr[3] as u32);
            // println!("{:#} {:#?} {:#x}", i, arr, word);
            w[i] = word;
        }
        // println!("w (len {:?}): {:?}", w.len(), w);

        // Extend the first 16 words into the remaining 48 words w[16..63] of the message schedule array:
        for i in 16..64 {
            // s0 := (w[i-15] rightrotate  7) xor (w[i-15] rightrotate 18) xor (w[i-15] rightshift  3)
            let s0: u32 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            // s1 := (w[i- 2] rightrotate 17) xor (w[i- 2] rightrotate 19) xor (w[i- 2] rightshift 10)
            let s1: u32 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            // w[i] := w[i-16] + s0 + w[i-7] + s1
            w[i] = w[i - 16]
                .overflowing_add(s0)
                .0
                .overflowing_add(w[i - 7])
                .0
                .overflowing_add(s1)
                .0;
        }

        // Initialize working variables to current hash value:
        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;
        let mut f = h5;
        let mut g = h6;
        let mut h = h7;

        // Compression function main loop:
        // for i from 0 to 63
        for i in 0..64 {
            // S1 := (e rightrotate 6) xor (e rightrotate 11) xor (e rightrotate 25)
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            // ch := (e and f) xor ((not e) and g)
            let ch = (e & f) ^ ((!e) & g);
            // temp1 := h + S1 + ch + k[i] + w[i]
            let temp1 = h
                .overflowing_add(s1)
                .0
                .overflowing_add(ch)
                .0
                .overflowing_add(k[i])
                .0
                .overflowing_add(w[i])
                .0;
            // S0 := (a rightrotate 2) xor (a rightrotate 13) xor (a rightrotate 22)
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            // maj := (a and b) xor (a and c) xor (b and c)
            let maj = (a & b) ^ (a & c) ^ (b & c);
            // temp2 := S0 + maj
            let temp2 = s0.overflowing_add(maj).0;

            h = g;
            g = f;
            f = e;
            e = d.overflowing_add(temp1).0;
            d = c;
            c = b;
            b = a;
            a = temp1.overflowing_add(temp2).0;
        }

        // Add the compressed chunk to the current hash value:
        h0 = h0.overflowing_add(a).0;
        h1 = h1.overflowing_add(b).0;
        h2 = h2.overflowing_add(c).0;
        h3 = h3.overflowing_add(d).0;
        h4 = h4.overflowing_add(e).0;
        h5 = h5.overflowing_add(f).0;
        h6 = h6.overflowing_add(g).0;
        h7 = h7.overflowing_add(h).0;
    }

    // Produce the final hash value (big-endian):
    // digest := hash := h0 append h1 append h2 append h3 append h4 append h5 append h6 append h7
    // :08x for min of 8 chars to printed in hexadecimal (2 chars per byte & 4 bytes in u32 => 2*4)
    let digest = format!(
        "{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}",
        h0, h1, h2, h3, h4, h5, h6, h7
    );
    digest
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
        let data = String::from("1111111111222222222233333333334444444444555555555566666666667777777777").into_bytes();
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

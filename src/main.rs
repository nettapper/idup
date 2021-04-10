extern crate image;

use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "idup", about = "Find duplicate images using avg perceptual hash function")]
struct Opt {
    /// Image file
    #[structopt(parse(from_os_str))]
    img: PathBuf,

    /// Image file 2
    #[structopt(parse(from_os_str))]
    img2: Option<PathBuf>
}

fn main() {
    let opt = Opt::from_args();
    println!("{:?}", opt);

    match opt.img2 {
        // one image as command line arg
        // calcualte it's phash and print it
        None => {
            let hash1 = phash(opt.img);
            println!("img: {}", hash1);
        },
        // two images as command line args
        // calcualte both phashes, and dist
        Some(img2) => {
            let hash1 = phash(opt.img);
            println!("img1: {}", hash1);

            let hash2 = phash(img2);
            println!("img2: {}", hash2);

            let diff = hamming_dist(hash1, hash2);
            println!("diff: {}", diff);
        }
    }
}

fn phash(fpath: PathBuf) -> u64 {
    let img = image::open(fpath).expect("test image could not be opened");

    // println!("original dimensions {:?}", img.dimensions());
    // println!("original color {:?}", img.color());

    let img = img.resize_exact(8, 8, image::imageops::FilterType::Gaussian).into_luma8();
    let (w, h) = img.dimensions();
    // println!("new dimensions {:?}", (w, h));

    let mut total: u32 = 0;
    for p in img.iter() {
        total = total.checked_add(*p as u32).expect("overflow when calculating total");
    }
    // println!("total {:?}", total);

    let avg = total / (w * h);
    // println!("average {:?}", avg);

    assert!(w * h == 64);
    // SAFETY this will overflow if width * height is anything over 64 bits
    let mut average_hash: u64 = 0;
    for p in img.iter() {
        // println!("{} {:#b}", *p, average_hash);
        if *p as u32 > avg {
            average_hash += 1;
        }
        average_hash <<= 1;  // shift one from least to most signifigant
    }
    average_hash
}

// counts the number of bits that are different
fn hamming_dist(mut a: u64, mut b: u64) -> u8 {
    let mut count: u8 = 0;
    while a > 0 && b > 0 {
        if a&1 != b&1 {
            count += 1;
        }
        a >>= 1;  // div by 2
        b >>= 1;  // div by 2
    }
    count
}

mod tests {
    use super::hamming_dist;

    #[test]
    fn hamming_dist_same() {
        let x = 0x8f8f978589f9f1c0;
        assert_eq!(hamming_dist(x, x), 0);
    }

    #[test]
    fn hamming_dist_off_by_one() {
        let x = 0x8f8f978589f9f1c0;  // last 4 bits are 0's
        let y = x + 1;
        assert_eq!(hamming_dist(x, y), 1);
        let z = x + 8;  // any pow of 2 should only change on bit (assming no carry bit)
        assert_eq!(hamming_dist(x, z), 1);
    }
}

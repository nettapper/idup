extern crate image;

use image::GenericImageView;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "example", about = "An example of StructOpt usage.")]
struct Opt {
    /// Input file
    #[structopt(parse(from_os_str))]
    img: PathBuf,
}

fn main() {
    let opt = Opt::from_args();
    println!("{:?}", opt);

    let img = image::open(opt.img).expect("test image could not be opened");

    println!("original dimensions {:?}", img.dimensions());
    println!("original color {:?}", img.color());

    let img = img.resize(8, 8, image::imageops::FilterType::Gaussian).into_luma8();
    let (w, h) = img.dimensions();
    println!("new dimensions {:?}", (w, h));

    let mut total: u32 = 0;
    for p in img.iter() {
        total = total.checked_add(*p as u32).expect("overflow when calculating total");
    }
    println!("total {:?}", total);

    let avg = total / (w * h);
    println!("average {:?}", avg);

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
    println!("{}", average_hash);

    // img.save("out.jpg").expect("could not save new image");
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

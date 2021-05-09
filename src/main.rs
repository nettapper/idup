extern crate image;

use std::path::PathBuf;
use structopt::StructOpt;
use std::fs::read;

mod hash;

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
            let img = image::open(&opt.img).expect("Failed to open the file for the perceptual hash");
            let ph = hash::phash::hash(img);
            println!("phash: {}", ph);
            let data = read(&opt.img).expect("Failed to open the file for the sha512 hash");
            let sh = hash::sha512::hash(data);
            println!("sha512: {}", sh);
        },
        // two images as command line args
        // calcualte both phashes, and dist
        Some(img2) => {
            let img = image::open(&opt.img).expect("Failed to open the first file for the perceptual hash");
            let hash1 = hash::phash::hash(img);
            println!("img1: {}", hash1);

            let img2 = image::open(&img2).expect("Failed to open the second file for the perceptual hash");
            let hash2 = hash::phash::hash(img2);
            println!("img2: {}", hash2);

            let diff = hash::hamming_dist(hash1, hash2);
            println!("diff: {}", diff);
        }
    }
}


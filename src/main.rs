extern crate image;

use std::path::PathBuf;
use structopt::StructOpt;

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
            let ph = hash::phash::hash(opt.img);
            println!("phash: {}", ph);
            let sh = hash::sha512::hash();
            println!("sha512: {}", sh);
        },
        // two images as command line args
        // calcualte both phashes, and dist
        Some(img2) => {
            let hash1 = hash::phash::hash(opt.img);
            println!("img1: {}", hash1);

            let hash2 = hash::phash::hash(img2);
            println!("img2: {}", hash2);

            let diff = hash::hamming_dist(hash1, hash2);
            println!("diff: {}", diff);
        }
    }
}


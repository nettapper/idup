extern crate image;

use std::fs::read;
use std::path::PathBuf;
use structopt::StructOpt;

mod hash;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "idup",
    about = "Find duplicate images using avg perceptual hash function"
)]
enum Opt {
    /// Given a path, calculate & store hashes of files in the db
    Scan {
        /// File or folder
        #[structopt(parse(from_os_str))]
        path: PathBuf,
        #[structopt(short, long)]
        recursive: bool,
    },
    /// Retrieve duplicates or near duplicates from the db
    List,
    /// Clean outdated data in the db
    Clean,
    /// Recompute hashes of files in db
    Update,
    /// Print information about a particular file
    Info {
        #[structopt(parse(from_os_str))]
        file: PathBuf,
    },
    /// Print information about two files
    Compare {
        // TODO should I make this 2..n files?
        // TODO should this & info be merged?
        /// File 1
        #[structopt(parse(from_os_str))]
        img1: PathBuf,
        /// File 2
        #[structopt(parse(from_os_str))]
        img2: PathBuf,
    }
}

fn main() {
    let opt = Opt::from_args();
    println!("{:?}", opt);

    match opt {
        // calculate it's phash and print it
        Opt::Info{ file } => {
            // TODO I need better error handling
            let img =
                image::open(&file).expect("Failed to open the file for the perceptual hash");
            let ph = hash::phash::hash(img);
            println!("phash: {}", ph);
            let data = read(&file).expect("Failed to open the file for the sha256 hash");
            let sh = hash::sha256::hash(data);
            println!("sha256: {}", sh);
        }

        // calculate both phashes, and dist
        Opt::Compare{ img1, img2 } => {
            let img = image::open(&img1)
                .expect("Failed to open the first file for the perceptual hash");
            let hash1 = hash::phash::hash(img);
            println!("img1: {}", hash1);

            let img2 =
                image::open(&img2).expect("Failed to open the second file for the perceptual hash");
            let hash2 = hash::phash::hash(img2);
            println!("img2: {}", hash2);

            let diff = hash::hamming_dist(hash1, hash2);
            println!("diff: {}", diff);
        }

        _ => {
            println!("This functionality is currently being worked on");
        }
    }
}

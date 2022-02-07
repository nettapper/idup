use std::path::PathBuf;
use structopt::StructOpt;

mod hash;
mod scan;
mod db;

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
        // TODO should I add follow symlink opt (it looks to be a nightly feature right now)
    },
    /// Retrieve duplicates or near duplicates from the db
    List {
        /// File or folder
        #[structopt(parse(from_os_str))]
        path: Option<PathBuf>,
    },
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
            let ph = hash::phash::hash_path(&file);
            println!("phash: {}", ph);
            let sh = hash::sha256::hash_path(&file);
            println!("sha256: {}", sh);
        }

        // calculate both phashes, and dist
        Opt::Compare{ img1, img2 } => {
            let hash1 = hash::phash::hash_path(&img1);
            println!("img1: {}", hash1);

            let hash2 = hash::phash::hash_path(&img2);
            println!("img2: {}", hash2);

            let diff = hash::hamming_dist(hash1, hash2);
            println!("diff: {}", diff);
        }

        // Find & store hashes into db
        Opt::Scan{ path, recursive } => {
            scan::process_path(path, recursive);
        }

        // List matches of file
        Opt::List{ path } => {
            // TODO future features
            // - if dir, find all matches that fall under the parent
            // - if file, find all matches for that file
            // - if no path given, find all matches in db
            // - optins to do exact match (sha256) or fuzzy (phash)
            match path {
                None => {
                    let iter = db::exact_matches().unwrap();
                    for data in iter {
                        println!("{:?}", data.path);
                    }
                },
                Some(path) => {
                    let iter = db::exact_match(&path).unwrap();
                    for data in iter {
                        println!("{:?}", data.path);
                    }
                }
            }
        }

        _ => {
            println!("This functionality is currently being worked on");
        }
    }
}

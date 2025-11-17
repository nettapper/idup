use clap::Parser;
use env_logger::{Builder, Target};
use log::{debug, error, info, LevelFilter};
use std::path::PathBuf;

mod db;
mod hash;
mod scan;

#[derive(Debug, Parser)]
#[command(name = "idup", about = "Find duplicate images using avg perceptual hash function")]
enum Opt {
    /// Given a path, calculate & store hashes of files in the db
    Scan {
        /// File or folder
        path: PathBuf,
        #[arg(short, long)]
        recursive: bool,
        // TODO should I add follow symlink opt (it looks to be a nightly feature right now)
    },
    /// Retrieve duplicates or near duplicates from the db
    List {
        /// File or folder
        path: Option<PathBuf>,
    },
    /// Clean outdated data in the db
    Clean,
    /// Recompute hashes of files in db
    Update,
    /// Print information about a particular file
    Info { file: PathBuf },
    /// Print information about two files
    Compare {
        // TODO should I make this 2..n files?
        // TODO should this & info be merged?
        /// File 1
        img1: PathBuf,
        /// File 2
        img2: PathBuf,
    },
}

fn main() {
    Builder::new()
        .target(Target::Stdout)
        .filter_level(LevelFilter::Info)
        .parse_default_env()
        .init();
    let opt = Opt::parse();
    debug!("{:?}", opt);

    match opt {
        // calculate it's phash and print it
        Opt::Info { file } => {
            // TODO I need better error handling
            match hash::phash::hash_path(&file) {
                Ok(ph) => info!("phash: {:?}", ph),
                Err(err) => error!("phash err: {}", err),
            }
            match hash::sha256::hash_path(&file) {
                Ok(sh) => info!("sha256: {:?}", sh),
                Err(err) => error!("sha256 err: {}", err),
            }
        }

        // calculate both phashes, and dist
        Opt::Compare { img1, img2 } => {
            let hash1 = hash::phash::hash_path(&img1).unwrap();
            info!("img1: {:?}", hash1);

            let hash2 = hash::phash::hash_path(&img2).unwrap();
            info!("img2: {:?}", hash2);

            let diff = hash::hamming_dist(hash1, hash2);
            match diff {
                Ok(val) => info!("diff: {}", val),
                Err(_) => error!("failed to calculate dist"),
            }
        }

        // Find & store hashes into db
        Opt::Scan { path, recursive } => {
            scan::process_path(path, recursive);
        }

        // List matches of file
        Opt::List { path } => {
            // TODO future features
            // - if dir, find all matches that fall under the parent
            // - if file, find all matches for that file
            // - if no path given, find all matches in db
            // - optins to do exact match (sha256) or fuzzy (phash)
            match path {
                None => {
                    let iter = db::exact_matches().unwrap();
                    for data in iter {
                        info!("{:?}", data.path);
                    }
                }
                Some(path) => {
                    let iter = db::exact_match(&path).unwrap();
                    for data in iter {
                        info!("{:?}", data.path);
                    }
                }
            }
        }

        _ => {
            info!("This functionality is currently being worked on");
        }
    }
}

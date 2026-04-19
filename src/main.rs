use clap::{Parser, Subcommand};
use sqlx::SqlitePool;
use std::path::PathBuf;

mod db;
mod hash;
mod scan;

#[derive(Debug, Parser)]
#[command(
    name = "idup",
    about = "Find duplicate images using avg perceptual hash function"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Given a path, calculate & store hashes of files in the db
    Scan {
        /// File or folder
        #[arg(value_parser = clap::value_parser!(std::path::PathBuf))]
        path: PathBuf,
        #[arg(short, long)]
        recursive: bool,
        // TODO should I add follow symlink opt (it looks to be a nightly feature right now)
    },

    /// Retrieve duplicates or near duplicates from the db
    List {
        /// File or folder
        #[arg(value_parser = clap::value_parser!(std::path::PathBuf))]
        path: Option<PathBuf>,
    },

    /// Clean outdated data in the db
    Clean,

    /// Recompute hashes of files in db
    Update,

    /// Print information about a particular file
    Info {
        #[arg(value_parser = clap::value_parser!(std::path::PathBuf))]
        file: PathBuf,
    },

    /// Print information about two files
    Compare {
        // TODO should I make this 2..n files?
        // TODO should this & info be merged?
        /// File 1
        #[arg(value_parser = clap::value_parser!(std::path::PathBuf))]
        img1: PathBuf,
        /// File 2
        #[arg(value_parser = clap::value_parser!(std::path::PathBuf))]
        img2: PathBuf,
    },
}

#[tokio::main]
async fn main() -> sqlx::Result<()> {
    let cli = Cli::parse();
    let pool: SqlitePool = db::open_pool().await?;

    match cli.command {
        Command::Info { file } => {
            // calculate it's phash and print it
            // TODO I need better error handling
            match hash::phash::hash_path(&file) {
                Ok(ph) => println!("phash: {:?}", ph),
                Err(err) => println!("phash err: {}", err),
            }
            match hash::sha256::hash_path(&file) {
                Ok(sh) => println!("sha256: {:?}", sh),
                Err(err) => println!("sha256 err: {}", err),
            }
        }

        Command::Compare { img1, img2 } => {
            // calculate both phashes, and dist
            let hash1 = hash::phash::hash_path(&img1).unwrap();
            println!("img1: {:?}", hash1);

            let hash2 = hash::phash::hash_path(&img2).unwrap();
            println!("img2: {:?}", hash2);

            let diff = hash::hamming_dist(hash1, hash2);
            match diff {
                Ok(val) => println!("diff: {}", val),
                Err(_) => println!("failed to calculate dist"),
            }
        }

        Command::Scan { path, recursive } => {
            // Find & store hashes into db
            scan::process_path(path, recursive, &pool).await;
        }

        Command::List { path } => {
            // List matches of file
            // TODO future features
            // - if dir, find all matches that fall under the parent
            // - if file, find all matches for that file
            // - if no path given, find all matches in db
            // - optins to do exact match (sha256) or fuzzy (phash)
            match path {
                None => {
                    let data = db::exact_matches(&pool).await.unwrap();
                    for item in data {
                        println!("{:?}", item.path);
                    }
                }
                Some(path) => {
                    let data = db::exact_match(&pool, &path).await.unwrap();
                    for item in data {
                        println!("{:?}", item.path);
                    }
                }
            }
        }

        _ => {
            println!("This functionality is currently being worked on");
        }
    }

    Ok(())
}

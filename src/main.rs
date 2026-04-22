use clap::{Parser, Subcommand};
use sqlx::SqlitePool;
use std::path::PathBuf;

mod db;
mod hash;
mod scan;
mod update;
mod web;

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
    /// Compute and store image hashes (default: SHA-256 base only)
    #[command(
        long_about = "Compute and store image hashes for all images at the given path.\n\
            \n\
            By default only the base SHA-256 of raw pixel data is stored.\n\
            Use a preset (--exact or --all) or combine individual flags\n\
            (--rotations, --flips, --phash) to include additional variants.\n\
            Presets and individual flags are mutually exclusive."
    )]
    Scan {
        /// File or directory to scan
        #[arg(value_parser = clap::value_parser!(std::path::PathBuf))]
        path: PathBuf,

        /// Recurse into subdirectories
        #[arg(short, long)]
        recursive: bool,

        // ── Presets ────────────────────────────────────────────────────────

        /// [Preset] SHA-256 base + rot90/180/270. Detects rotated exact duplicates.
        /// Mutually exclusive with --all and individual hash flags.
        #[arg(long, help_heading = "Hash Presets", conflicts_with_all = ["all", "phash", "rotations", "flips"])]
        exact: bool,

        /// [Preset] All variants: SHA-256 base, all rotations, all flips, and phash.
        /// Mutually exclusive with --exact and individual hash flags.
        #[arg(long, help_heading = "Hash Presets", conflicts_with_all = ["exact", "phash", "rotations", "flips"])]
        all: bool,

        // ── Individual flags ───────────────────────────────────────────────

        /// SHA-256 of rot90, rot180, rot270 (detects rotated duplicates)
        #[arg(long, help_heading = "Individual Hash Flags", conflicts_with_all = ["exact", "all"])]
        rotations: bool,

        /// SHA-256 of flipv and flipv+rot90/180/270 (detects mirrored duplicates)
        #[arg(long, help_heading = "Individual Hash Flags", conflicts_with_all = ["exact", "all"])]
        flips: bool,

        /// Perceptual hash — detects near-duplicate / visually similar images
        #[arg(long, help_heading = "Individual Hash Flags", conflicts_with_all = ["exact", "all"])]
        phash: bool,
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
    Update {
        /// File or directory to update (if not provided, updates all images)
        #[arg(value_parser = clap::value_parser!(std::path::PathBuf))]
        path: Option<PathBuf>,

        /// Delete images from database if their files no longer exist
        #[arg(long)]
        cleanup: bool,
    },

    /// Print information about a particular file
    Info {
        #[arg(value_parser = clap::value_parser!(std::path::PathBuf))]
        file: PathBuf,
    },

    /// Return N random files from the db
    Random {
        /// Number of files to return
        #[arg(default_value_t = 20)]
        n: u32,

        /// Glob pattern to filter paths (e.g. "*.jpg", "/home/user/Photos/*")
        #[arg(short, long)]
        filter: Option<String>,
    },

    /// Serve a web UI for browsing duplicates
    Web {
        /// Port to listen on
        #[arg(short, long, default_value_t = 3000)]
        port: u16,
        /// Open the browser automatically after starting
        #[arg(long)]
        open: bool,
    },
}

#[tokio::main]
async fn main() -> sqlx::Result<()> {
    let cli = Cli::parse();
    let pool: SqlitePool = db::open_pool().await?;

    match cli.command {
        Command::Info { file } => {
            match hash::phash::hash_path(&file) {
                Ok(ph) => println!("phash: {:?}", ph),
                Err(err) => println!("phash err: {}", err),
            }
            match hash::sha256::hash_path(&file) {
                Ok(sh) => println!("sha256: {:?}", sh),
                Err(err) => println!("sha256 err: {}", err),
            }
        }

        Command::Scan { path, recursive, exact, all, phash, rotations, flips } => {
            let opts = if all {
                scan::ScanOptions::all()
            } else if exact {
                scan::ScanOptions::exact()
            } else if phash || rotations || flips {
                scan::ScanOptions {
                    sha256_rotations: rotations,
                    sha256_flips: flips,
                    phash,
                }
            } else {
                scan::ScanOptions::default()
            };
            scan::process_path(path, recursive, &opts, &pool).await;
        }

        Command::List { path } => {
            match path {
                None => {
                    let data = db::exact_matches_grouped(&pool).await.unwrap();
                    if data.is_empty() {
                        println!("No duplicates found.");
                    } else {
                        let mut current_hash = String::new();
                        let mut group_num = 0usize;
                        for item in &data {
                            if item.group_hash != current_hash {
                                if group_num > 0 {
                                    println!();
                                }
                                group_num += 1;
                                println!("[{}]", group_num);
                                current_hash = item.group_hash.clone();
                            }
                            println!("  {}", item.path);
                        }
                    }
                }
                Some(path) => {
                    let data = db::exact_match(&pool, &path).await.unwrap();
                    if data.is_empty() {
                        println!("No duplicates found.");
                    } else {
                        for item in &data {
                            println!("{}", item.path);
                        }
                    }
                }
            }
        }

        Command::Random { n, filter } => {
            match db::random_images(&pool, n, filter.as_deref()).await {
                Err(e) => eprintln!("Error: {e}"),
                Ok(data) if data.is_empty() => println!("No images in db."),
                Ok(data) => {
                    for item in &data {
                        println!("{}", item.path);
                    }
                }
            }
        }

        Command::Clean => {
            print!("This will wipe the entire database. Are you sure? [y/N] ");
            use std::io::{self, Write};
            io::stdout().flush().ok();
            let mut input = String::new();
            io::stdin().read_line(&mut input).ok();
            if matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
                match db::wipe_db(&pool).await {
                    Ok(()) => println!("Database wiped."),
                    Err(e) => eprintln!("Error wiping database: {e}"),
                }
            } else {
                println!("Aborted.");
            }
        }

        Command::Web { port, open } => {
            web::serve(port, open, pool).await;
        }

        Command::Update { path, cleanup } => {
            update::process_update(path, cleanup, &pool).await;
            // Stats are printed inside process_update()
        }
    }

    Ok(())
}

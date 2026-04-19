# idup — Project Info

## Purpose

`idup` is a Rust CLI tool for finding duplicate and near-duplicate images. It scans directories, computes cryptographic and perceptual hashes of each image, persists results in a local SQLite database, and lets the user query for exact or near duplicates.

The name is short for **image duplicates**.

---

## Tech Stack

| Layer | Technology |
|---|---|
| Language | Rust (2021 edition) |
| CLI parsing | clap 4.6.1 (derive feature) |
| Async runtime | tokio 1.x (multi-thread) |
| Database | sqlx 0.8 with SQLite |
| Image processing | image 0.25 |
| File type detection | infer 0.19.0 (magic-byte MIME detection) |
| Logging | env_logger 0.11.8 (controlled via `RUST_LOG`) |

---

## Project Structure

```
idup/
├── Cargo.toml          # Package manifest and direct dependencies
├── Cargo.lock          # Locked dependency tree
├── README.md           # Brief description and perf notes
├── docs/
│   └── PROJECT_INFO.md # This file
└── src/
    ├── main.rs         # CLI entry point and command dispatch
    ├── db/
    │   └── mod.rs      # Database layer (SQLite via sqlx)
    ├── hash/
    │   ├── mod.rs      # Shared hash types (ImgHash, ImgHashKind) and Hamming distance
    │   ├── phash.rs    # Perceptual hash (average hash) — resize to 8x8, compare to mean
    │   └── sha256.rs   # Hand-rolled SHA-256 implementation
    └── scan/
        └── mod.rs      # Filesystem traversal and hashing orchestration
```

---

## Key Modules

### `src/main.rs`
Defines `Cli` and `Command` via clap derive macros. Opens the SQLite connection pool and dispatches to the appropriate logic per subcommand. `Clean` and `Update` commands are currently stubs.

### `src/scan/mod.rs`
`process_path(path, recursive, pool)` — iterates files using explicit stack-based DFS. Uses `infer` to detect images by magic bytes (not file extension). For each image:
1. Clears existing hashes for that path in the DB.
2. Computes 8 SHA-256 hashes of decoded pixel data (all 4 rotations × 2 flips).
3. Computes 1 perceptual hash (pHash).
4. Saves all hashes to the DB.

### `src/hash/phash.rs`
`hash(img) -> u64` — resizes to 8×8, converts to grayscale, computes mean pixel value, produces a 64-bit integer where each bit is 1 if the corresponding pixel is above-mean. Low Hamming distance between two pHashes indicates perceptual similarity.

### `src/hash/sha256.rs`
Hand-rolled SHA-256 based on the Wikipedia pseudocode. `all_hashes_of_img_data(path)` returns 8 hashes covering all rotation/flip variants of the decoded pixel buffer — used to detect exact pixel-identical images regardless of storage format or orientation.

### `src/hash/mod.rs`
Defines `ImgHashKind` (`Phash` or `Sha256(String)` where the string names the transform, e.g. `"imgdata rot90"`), `ImgHash` struct, and `hamming_dist(a, b)`.

### `src/db/mod.rs`
SQLite schema with 3 tables:
- `images(images_id PK, path TEXT UNIQUE)` — one row per file path
- `hashes(images_id FK, kind TEXT, hash TEXT)` — multiple hashes per image
- `partial_hashes(images_id FK, sequence INT, part_hash TEXT)` — pHash split into 4-byte chunks for future indexed fuzzy lookup

DB location: `$XDG_DATA_HOME/idup/idup.db3` (defaults to `~/.local/share/idup/idup.db3`). Created automatically on first run. Includes an in-band schema migration to fix the `hashes` primary key (recreates as `hashes_v2` if old schema detected).

---

## CLI Commands

```
idup scan <path> [--recursive]   # Scan path, compute and store hashes
idup list [<path>]               # List all exact duplicates (optionally for a specific file)
idup random [N]                  # Return N random files from the db (default: 20)
idup info <file>                 # Print phash + sha256 of a single file
idup compare <img1> <img2>       # Print phash of both images and their Hamming distance
idup clean                       # (stub) Remove outdated DB entries
idup update                      # (stub) Recompute hashes for existing DB entries
```

---

## Architecture Notes

- **Dual hashing**: Each image gets a perceptual hash (for near-duplicate/fuzzy matching) and pixel-data SHA-256 across 8 orientations (for exact duplicate detection including rotations/flips).
- **Hand-rolled SHA-256**: Written from scratch as a learning exercise with full unit tests against known vectors. Not a production crypto dependency.
- **Magic-byte file detection**: `infer` inspects file bytes rather than relying on file extensions.
- **Partial hash table**: `partial_hashes` splits pHash into 4-byte chunks with sequence numbers, laying groundwork for indexed fuzzy lookup (not yet surfaced in the CLI).
- **Stack-based DFS traversal**: Avoids recursion-related stack overflows on deep directory trees.
- **Schema migration inline**: No migration framework; schema setup and the `hashes` PK fix are handled in `setup_db()` at startup.
- **No external services**: Entirely self-contained. No network access, no cloud storage, no external APIs.

---

## Build and Test

```sh
cargo build            # Debug build
cargo build --release  # Optimized build (recommended for perf)
cargo test             # Run unit tests (SHA-256 known-vector tests)
cargo run -- <cmd>     # Run directly via Cargo
```

Performance reference: ~1,400 images processed in ~82 seconds (unoptimized build).

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
| Web server | axum 0.8 |
| Web UI | htmx 2.0 (CDN), hand-written HTML/CSS |
| Browser launching | open 5 |

---

## Project Structure

```
idup/
├── Cargo.toml          # Package manifest and direct dependencies
├── Cargo.lock          # Locked dependency tree
├── README.md           # Brief description and perf notes
├── assets/
│   ├── index.html      # Main web UI (SPA using htmx)
│   └── style.css       # Shared stylesheet for web UI
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
    ├── scan/
    │   └── mod.rs      # Filesystem traversal and hashing orchestration
    └── web/
        ├── mod.rs      # Axum router setup and server startup
        └── handlers.rs # HTTP handlers for all routes
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

Returns `ScanStats` with:
- `processed`: number of images scanned
- `elapsed_secs`: total execution time

### `src/update/mod.rs`
`process_update(path, cleanup, pool)` — validates and refreshes image hashes in the DB. For each image in the DB (optionally filtered by path):
1. Checks if the file still exists on disk. If missing and `--cleanup` flag is set, deletes the image from the DB.
2. Computes the base SHA-256 hash of the image data.
3. Compares with the base SHA-256 stored in the DB:
   - If hashes match: marks file as verified.
   - If hashes don't match: recomputes ALL hash types that exist in the DB for that image (e.g., if DB has `sha256 imgdata`, `sha256 rot90`, `phash`, recomputes all three), and updates them in the DB.
4. Reports progress every 10 seconds and prints summary statistics (verified, updated, missing, cleaned).

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

Key public functions:

| Function | Description |
|---|---|
| `open_pool()` | Opens (or creates) the SQLite pool and runs schema migrations |
| `exact_match(pool, path)` | Returns files sharing a sha256 hash with the given path |
| `exact_matches_grouped(pool)` | Returns all duplicate groups ordered by hash |
| `images_for_group(pool, hash)` | Returns all paths that share a given `sha256 imgdata` hash — used by `/gallery?hash=` |
| `path_exists_in_db(pool, path)` | Returns true if the absolute path is tracked in the DB — used to gate `/api/image` |
| `random_images(pool, n)` | Returns N randomly ordered image paths |
| `save(pool, img)` | Upserts an image and its hash |
| `clear_hashes_for_path(pool, path)` | Deletes all hashes for a given path |

DB location: `$XDG_DATA_HOME/idup/idup.db3` (defaults to `~/.local/share/idup/idup.db3`). Created automatically on first run. Includes an in-band schema migration to fix the `hashes` primary key (recreates as `hashes_v2` if old schema detected).

### `src/web/mod.rs` and `src/web/handlers.rs`
Axum-based web server started by `idup web`. Serves a single-page UI (`assets/index.html`) that uses htmx to call JSON/HTML fragment endpoints. Also serves a standalone gallery page for viewing images.

Routes:

| Route | Handler | Description |
|---|---|---|
| `GET /` | `index` | Main SPA (index.html) |
| `GET /style.css` | `style` | Shared stylesheet |
| `POST /api/scan` | `scan` | Run a scan and return result fragment |
| `GET /api/list` | `list` | List duplicate groups (htmx fragment); group headers link to `/gallery` |
| `GET /api/info` | `info` | pHash + SHA-256 for a single file |
| `POST /api/compare` | `compare` | Hamming distance between two images |
| `GET /api/random` | `random` | N random paths; includes "View All in Browser" button |
| `GET /api/image` | `image_file` | Serve a local image by absolute path (DB-gated) |
| `GET /gallery` | `gallery` | Standalone image grid page; accepts `?hash=` or repeated `?path=` params |

---

## CLI Commands

```
idup scan <path> [--recursive]   # Scan path, compute and store hashes
idup list [<path>]               # List all exact duplicates (optionally for a specific file)
idup random [N]                  # Return N random files from the db (default: 20)
idup info <file>                 # Print phash + sha256 of a single file
idup compare <img1> <img2>       # Print phash of both images and their Hamming distance
idup web [--port N] [--open]     # Start the web UI (default port: 3000)
idup clean                       # (stub) Remove outdated DB entries
idup update [PATH] [--cleanup]   # Validate and refresh image hashes in the DB
```

## Web UI Features

The `idup web` command starts a local HTTP server. The UI has panels for each CLI operation (scan, list, info, compare, random) plus image viewing:

- **List panel**: Each duplicate group header is a clickable link that opens a gallery in a new browser tab showing all images in that group.
- **Random panel**: After fetching results, a "View All in Browser" button appears that opens a gallery of all returned images in a new tab.
- **Gallery page** (`/gallery`): Standalone dark-themed image grid. Accepts either a `?hash=<group_hash>` query param (resolves from DB) or repeated `?path=<abs_path>` params.
- **Image serving** (`/api/image`): Serves local image files by absolute path. Access is gated: only paths tracked in the idup DB are served.

---

## Architecture Notes

- **Dual hashing**: Each image gets a perceptual hash (for near-duplicate/fuzzy matching) and pixel-data SHA-256 across 8 orientations (for exact duplicate detection including rotations/flips).
- **Hand-rolled SHA-256**: Written from scratch as a learning exercise with full unit tests against known vectors. Not a production crypto dependency.
- **Magic-byte file detection**: `infer` inspects file bytes rather than relying on file extensions. Also used by `/api/image` to set the correct `Content-Type` response header.
- **Partial hash table**: `partial_hashes` splits pHash into 4-byte chunks with sequence numbers, laying groundwork for indexed fuzzy lookup (not yet surfaced in the CLI).
- **Stack-based DFS traversal**: Avoids recursion-related stack overflows on deep directory trees.
- **Schema migration inline**: No migration framework; schema setup and the `hashes` PK fix are handled in `setup_db()` at startup.
- **Web image serving is DB-gated**: `/api/image` only serves files whose absolute path is already tracked in the idup database. This prevents arbitrary file system access.
- **Gallery page is server-rendered**: The `/gallery` handler builds the full HTML string in Rust (no template engine). Paths are embedded directly into `<img src="/api/image?path=...">` tags with percent-encoded URLs.
- **No external services**: Entirely self-contained. The web UI loads htmx from a CDN but otherwise requires no network access.

---

## Build and Test

```sh
cargo build            # Debug build
cargo build --release  # Optimized build (recommended for perf)
cargo test             # Run unit tests (SHA-256 known-vector tests)
cargo run -- <cmd>     # Run directly via Cargo
```

Performance reference: ~1,400 images processed in ~82 seconds (unoptimized build).

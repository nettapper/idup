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

Workspace with two crates:

```
idup/                      # Workspace root
├── Cargo.toml             # Workspace manifest (defines members and shared metadata)
├── Cargo.lock             # Locked dependency tree
├── README.md              # Brief description and perf notes
├── docs/
│   └── PROJECT_INFO.md    # This file
├── crates/
│   ├── idup/              # Main CLI binary
│   │   ├── Cargo.toml     # idup crate manifest
│   │   ├── assets/
│   │   │   ├── index.html # Main web UI (SPA using htmx)
│   │   │   └── style.css  # Shared stylesheet for web UI
│   │   └── src/
│   │       ├── main.rs    # CLI entry point and command dispatch
│   │       ├── db/
│   │       │   └── mod.rs # Database layer (SQLite via sqlx)
│   │       ├── hash/
│   │       │   ├── mod.rs # Shared hash types (ImgHash, ImgHashKind) and Hamming distance
│   │       │   ├── phash.rs    # Perceptual hash (average hash) — resize to 8x8, compare to mean
│   │       │   └── sha256.rs   # Hand-rolled SHA-256 implementation
│   │       ├── scan/
│   │       │   └── mod.rs # Filesystem traversal and hashing orchestration
│   │       ├── update/
│   │       │   └── mod.rs # Image hash validation and refresh
│   │       └── web/
│   │           ├── mod.rs # Axum router setup and server startup
│   │           └── handlers.rs # HTTP handlers for all routes
│   │
│   └── igen/              # Performance test image generator
│       ├── Cargo.toml     # igen crate manifest (minimal deps: just `image`)
│       └── src/
│           └── main.rs    # Standalone image generation utility
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
| `images_for_group(pool, hash)` | Returns all paths that share a given `sha256 imgdata` hash — used by `/explore?hash=` |
| `path_exists_in_db(pool, path)` | Returns true if the absolute path is tracked in the DB — used to gate `/api/image` |
| `random_images(pool, n, filter)` | Returns N randomly ordered image paths (SQLite RANDOM(), no seed) |
| `random_images_seeded(pool, n, filter, seed)` | Returns N images in deterministic pseudo-random order; same seed+filter = same result |
| `images_in_dir(pool, dir)` | Returns direct (non-recursive) image children of `dir` |
| `subdirs_in_dir(pool, dir)` | Returns full paths of immediate subdirectories of `dir` that contain images |
| `images_matching_filter_in_dir(pool, dir, filter)` | Returns images matching a glob filter, optionally scoped to `dir` |
| `save(pool, img)` | Upserts an image and its hash |
| `db_stats(pool)` | Returns image count and hash counts grouped by kind |
| `clear_hashes_for_path(pool, path)` | Deletes all hashes for a given path |

DB location: `$XDG_DATA_HOME/idup/idup.db3` (defaults to `~/.local/share/idup/idup.db3`). Created automatically on first run. Includes an in-band schema migration to fix the `hashes` primary key (recreates as `hashes_v2` if old schema detected).

### `src/web/mod.rs` and `src/web/handlers.rs`
Axum-based web server started by `idup web`. Serves a single-page UI (`assets/index.html`) that uses htmx to call JSON/HTML fragment endpoints. Also serves a standalone explore page for browsing and viewing images.

Routes:

| Route | Handler | Description |
|---|---|---|
| `GET /` | `index` | Main SPA (index.html) |
| `GET /style.css` | `style` | Shared stylesheet |
| `POST /api/scan` | `scan` | Run a scan and return result fragment |
| `GET /api/list` | `list` | List duplicate groups (htmx fragment); group headers link to `/explore?hash=` |
| `GET /api/info` | `info` | pHash + SHA-256 for a single file |
| `GET /api/random` | `random` | N seeded-random paths; includes "View All in Browser" button linking to `/explore` |
| `GET /api/image` | `image_file` | Serve a local image by absolute path (DB-gated) |
| `GET /explore` | `explore` | Unified standalone image browser; accepts `?dir=`, `?filter=`, `?hash=`, or `?seed=&n=` |
| `POST /api/clean` | `clean` | Wipe all data from the database (images, hashes, partial hashes) |

---

## CLI Commands

```
idup scan <path> [--recursive]   # Scan path, compute and store hashes
idup list [<path>]               # List all exact duplicates (optionally for a specific file)
idup random [N]                  # Return N random files from the db (default: 20)
idup info <file>                 # Print phash + sha256 of a single file
idup compare <img1> <img2>       # Print phash of both images and their Hamming distance
idup web [--port N] [--open]     # Start the web UI (default port: 3000)
idup stats                        # Print database statistics (image count, hash counts by type)
idup clean                       # (stub) Remove outdated DB entries
idup update [PATH] [--cleanup]   # Validate and refresh image hashes in the DB
```

## Web UI Features

The `idup web` command starts a local HTTP server. The UI has panels for each CLI operation (scan, list, info, random, explore) plus image viewing:

- **List panel**: Each duplicate group header is a clickable link that opens `/explore?hash=<hash>` in a new browser tab.
- **Random panel**: After fetching results, a "View All in Browser" button links to `/explore?seed=<seed>&n=<n>&filter=<filter>`, allowing deterministic replay of the exact same random selection.
- **Clean panel**: Wipes the entire database. A browser `confirm()` dialog (via htmx `hx-confirm`) prompts the user before the request is sent.
- **Explore panel**: Opens `/explore` in a new tab with optional starting directory and glob filter.
- **Explore page** (`/explore`): Unified standalone image browser. Accepts:
  - `?dir=<path>` — directory browser showing subdirectory cards + images in the current dir, with breadcrumb navigation
  - `?filter=<glob>` — shows all images in the DB matching the glob (optionally scoped with `?dir=`)
  - `?hash=<group_hash>` — shows all images in a duplicate group (used by the list panel)
  - `?seed=<u64>&n=<u32>&filter=<glob>` — deterministic random N images (used by the random panel)
  - The page always shows a filter bar (dir + glob inputs) for in-page navigation without returning to the SPA.
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
# Build all workspace members
cargo build            # Debug build
cargo build --release  # Optimized build (recommended for perf)

# Build specific binaries
cargo build --bin idup
cargo build --bin igen

# Run tests (from workspace root)
cargo test --bins      # Run unit tests (SHA-256 known-vector tests, db tests)
cargo test --test '*'  # Run integration tests

# Run commands directly
cargo run --bin idup -- <cmd>      # Run idup CLI
cargo run --bin igen -- --dir <path> --count <n> --dupe-pct <pct>
```

A `Makefile` is provided as a convenience wrapper:

```sh
make build             # cargo build
make release           # cargo build --release
make test              # unit tests only (cargo test --bins)
make integration-test  # integration tests only (cargo test --test '*')
make all               # unit + integration
make lint              # cargo clippy -- -D warnings
make clean             # cargo clean
```

Performance reference: ~1,400 images processed in ~82 seconds (unoptimized build).

---

## Benchmarking

### `make bench`

Generates a 200-image test dataset with ~20% intentional duplicates, varying in size (64x64, 256x256, 1024x1024), then runs `idup scan --all` (all hash variants) and reports throughput:

```
Done. 200 files scanned in 5.43s (36.83 imgs/sec).
```

Uses temporary directories that are auto-cleaned up after the benchmark.

### `make bench-flamegraph`

Profiles the benchmark under CPU flamegraph visualization:

```sh
# First install flamegraph (one-time):
cargo install flamegraph

# Then run:
make bench-flamegraph
```

Outputs `flamegraph.svg` in the project root, showing per-function CPU time as a flame chart. Useful for identifying performance bottlenecks (e.g., image decoding vs. hashing vs. database I/O).

### Image Generator Binary: `igen`

Standalone utility (in `crates/igen/`) for generating test image datasets:

```sh
cargo run --release --bin igen -- \
  --dir /tmp/test_images \
  --count 200 \
  --dupe-pct 20
```

**Arguments:**

| Arg | Default | Purpose |
|---|---|---|
| `--dir <path>` | (required) | Output directory for generated images |
| `--count <n>` | 200 | Total images to generate |
| `--dupe-pct <n>` | 20 | Percentage of images that are exact duplicates (same seed, different filename) |

Outputs the directory path and total count to stdout (one per line), making it suitable for shell scripting.

---

## Integration Tests

Integration tests live in `crates/idup/tests/` and invoke the compiled `idup` binary as a
subprocess via [`assert_cmd`](https://crates.io/crates/assert_cmd). Each test
gets a fully isolated, auto-cleaned environment.

### Running Integration Tests

```sh
# Run integration tests for the idup crate
cargo test -p idup

# Run specific test
cargo test -p idup scan_detects_exact_duplicate

# Run all tests (unit + integration) in the workspace
cargo test --workspace
```

### DB isolation — `IDUP_DB_PATH`

Setting the `IDUP_DB_PATH` environment variable overrides the default database
location (`$XDG_DATA_HOME/idup/idup.db3`). When set, idup prints a notice at
startup:

```
[idup] IDUP_DB_PATH is set — using db at: /path/to/custom.db3
```

Integration tests always set this variable to a path inside a `TempDir` so
they never touch the user's real database.

### Test helpers (`crates/idup/tests/common/`)

| File | Purpose |
|---|---|
| `tests/common/mod.rs` | Re-exports `image_gen` and `test_env` modules |
| `tests/common/image_gen.rs` | `write_png(dir, filename, seed)` — deterministic PNG generation via a tiny inline LCG; same seed always produces identical pixel data and therefore an identical SHA-256 hash |
| `tests/common/test_env.rs` | `TestEnv::new()` — creates `<tmp>/db/idup.db3` + `<tmp>/imgs/`; `TestEnv::cmd()` returns an `assert_cmd::Command` pre-wired with `IDUP_DB_PATH` |

### Adding a new integration test

1. Create `crates/idup/tests/<name>_test.rs`.
2. Declare `mod common;` at the top.
3. Use `TestEnv::new()`, `image_gen::write_png(...)`, and `env.cmd()` to set
   up and exercise the binary.

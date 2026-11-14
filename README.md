# idup

**Find duplicate images, curate video frames, and tame massive local photo libraries—fast.**

`idup` (image duplicates) is a local-first Rust CLI and web toolkit that indexes your images into SQLite using orientation-aware pixel hashing and perceptual hashing. No cloud, no telemetry, no heavy runtime—just fast native tools and a snappy htmx web UI.

## Why `idup`?

- **Catches hidden duplicates**: Hashes decoded pixel data across all **8 rotations and flips** (`xxHash3` / `SHA-256`) so rotated, mirrored, or re-containerized copies never slip through, alongside **perceptual hashing (`pHash`)** for near-duplicates.
- **Scans anything**: Detects images by magic bytes (not file extensions) and optionally unpacks `.zip` archives on the fly.
- **Full-featured Web UI (`iweb`)**: Browse folders, inspect duplicate groups side-by-side, replay seeded random samples, batch move/delete with keyboard-friendly multi-select, and rotate or crop images directly in the browser.
- **Video-to-image curation (`ivid`)**: Extract frames from videos by time or frame interval, review them in a grid, re-extract promising ranges at finer intervals, and save the best shots straight into your indexed library.
- **Instant local queries**: Backed by a single SQLite database (`~/.local/share/idup/idup.db3`) with incremental verification and dead-path cleanup (`idup update --cleanup`).

## Toolkit

| Binary | Role |
|---|---|
| `idup` | Core CLI — scan directories, list duplicate groups, compare images, and maintain the DB |
| `iweb` | Local Axum + htmx web interface for visual browsing, deduplication, cropping, and video review |
| `ivid` | `ffmpeg`-powered video frame extractor supporting time and frame intervals |
| `icrop` | Lossless-workflow image rotation and cropping CLI/library |
| `igen` | Synthetic image dataset generator for benchmarking |

## Quick Start

```sh
make release

# Index a directory (recursively) and list exact duplicates
./target/release/idup scan ~/Pictures --recursive
./target/release/idup dups

# Launch the web UI
./target/release/iweb --open
```

## Performance

Always run benchmarks and large scans with the optimized release build (`make release`).

> 33312 files scanned in 529.36s (62.93 imgs/sec) on my hardware.


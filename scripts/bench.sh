#!/bin/bash
# Performance benchmark script
# Generates 200 test images and runs idup scan with all hash variants
# Outputs timing and throughput (imgs/sec)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "Building release binary..."
cargo build --release -q

TMPDIR=$(mktemp -d)
IMGDIR="$TMPDIR/imgs"
mkdir -p "$IMGDIR"

echo "Generating test images in $IMGDIR ..."
cargo run --release --bin igen -- --dir "$IMGDIR" --count 200 --dupe-pct 20 > /tmp/igen_out.txt 2>&1

IMGCOUNT=$(tail -1 /tmp/igen_out.txt)
echo "Generated $IMGCOUNT images"

echo "Running scan with all hash variants..."
IDUP_DB_PATH="$TMPDIR/idup.db3" "$PROJECT_ROOT/target/release/idup" scan --recursive --all "$IMGDIR"

rm -rf "$TMPDIR"
echo "Benchmark complete."

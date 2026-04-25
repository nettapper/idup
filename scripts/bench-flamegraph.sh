#!/bin/bash
# Performance benchmark with flamegraph profiling
# Generates 200 test images and runs idup scan under flamegraph profiling
# Outputs flamegraph.svg showing CPU hotspots
#
# Requires: cargo install flamegraph + system 'perf' tool
# On Linux: sudo apt-get install linux-tools-generic (or similar for your distro)

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

echo "Running scan under flamegraph (this may take a moment) ..."

if ! IDUP_DB_PATH="$TMPDIR/idup.db3" cargo flamegraph --bin idup -- scan --recursive --all "$IMGDIR"; then
    echo "Error: flamegraph failed."
    echo "Please ensure:"
    echo "  1. 'perf' is installed (sudo apt-get install linux-tools-generic)"
    echo "  2. cargo-flamegraph is installed (cargo install flamegraph)"
    exit 1
fi

rm -rf "$TMPDIR"
echo "Flamegraph written to flamegraph.svg"

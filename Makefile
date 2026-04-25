.PHONY: build release test integration-test lint clean all bench bench-flamegraph

## Build debug binary
build:
	cargo build

## Build optimised release binary
release:
	cargo build --release

## Run unit tests only (tests embedded in src/, fast, no binary invocation)
test:
	cargo test --bins

## Run integration tests (builds the binary first via assert_cmd)
integration-test:
	cargo test --test '*'

## Run all tests (unit + integration)
all: test integration-test

## Lint with clippy
lint:
	cargo clippy -- -D warnings

## Run performance benchmark (generates 200 test images, scans with all hash variants)
## Outputs timing and throughput (imgs/sec)
bench: release
	@set -e; \
	TMPDIR=$$(mktemp -d); \
	IMGDIR=$$TMPDIR/imgs; \
	mkdir -p $$IMGDIR; \
	echo "Generating test images in $$IMGDIR ..."; \
	cargo run --release --example igen -- --dir $$IMGDIR --count 200 --dupe-pct 20 > /tmp/igen_out.txt 2>&1; \
	IMGCOUNT=$$(tail -1 /tmp/igen_out.txt); \
	echo "Generated $$IMGCOUNT images"; \
	IDUP_DB_PATH=$$TMPDIR/idup.db3 ./target/release/idup scan --recursive --all $$IMGDIR; \
	rm -rf $$TMPDIR; \
	echo "Benchmark complete."

## Run performance benchmark with flamegraph profiling
## Generates flamegraph.svg showing CPU hotspots
## Requires: cargo install flamegraph + system 'perf' tool
## On Linux: sudo apt-get install linux-tools-generic (or similar for your distro)
bench-flamegraph: release
	@set -e; \
	TMPDIR=$$(mktemp -d); \
	IMGDIR=$$TMPDIR/imgs; \
	mkdir -p $$IMGDIR; \
	echo "Generating test images in $$IMGDIR ..."; \
	cargo run --release --example igen -- --dir $$IMGDIR --count 200 --dupe-pct 20 > /tmp/igen_out.txt 2>&1; \
	IMGCOUNT=$$(tail -1 /tmp/igen_out.txt); \
	echo "Generated $$IMGCOUNT images"; \
	echo "Running scan under flamegraph (this may take a moment) ..."; \
	IDUP_DB_PATH=$$TMPDIR/idup.db3 cargo flamegraph --bin idup -- scan --recursive --all $$IMGDIR || { echo "Error: flamegraph failed. Ensure 'perf' is installed (sudo apt-get install linux-tools-generic) and cargo-flamegraph is installed (cargo install flamegraph)"; exit 1; }; \
	rm -rf $$TMPDIR; \
	echo "Flamegraph written to flamegraph.svg"

## Remove build artefacts
clean:
	cargo clean

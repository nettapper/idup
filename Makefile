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
	cargo test -p idup --test '*'

## Run all tests (unit + integration)
all:
	cargo test --workspace

## Lint with clippy
lint:
	cargo clippy -- -D warnings

## Run performance benchmark (generates 200 test images, scans with all hash variants)
## Outputs timing and throughput (imgs/sec)
bench:
	@./scripts/bench.sh

## Run performance benchmark with flamegraph profiling
## Generates flamegraph.svg showing CPU hotspots
## Requires: cargo install flamegraph + system 'perf' tool
## On Linux: sudo apt-get install linux-tools-generic (or similar for your distro)
bench-flamegraph:
	@./scripts/bench-flamegraph.sh

## Remove build artefacts
clean:
	cargo clean

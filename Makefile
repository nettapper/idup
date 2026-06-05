.PHONY: build release test test-unit test-integration lint check clean bench bench-flamegraph

## Build debug binary
build:
	cargo build

## Build optimised release binary
release:
	cargo build --release

## Run all tests (unit + integration)
test:
	cargo test --workspace

## Run unit tests only (tests embedded in src/, fast, no binary invocation)
test-unit:
	cargo test --bins

## Run integration tests (builds the binary first via assert_cmd)
test-integration:
	cargo test -p idup --test '*'

## Lint with clippy
lint:
	cargo clippy -- -D warnings

check:
	cargo deny check

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

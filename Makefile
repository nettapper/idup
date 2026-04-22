.PHONY: build release test integration-test lint clean all

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

## Remove build artefacts
clean:
	cargo clean

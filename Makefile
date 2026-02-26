SHELL := /bin/bash

.PHONY: help fmt lint test build check-all check-all-llm

help:
	@echo "Targets:"
	@echo "  make fmt           - Run rustfmt"
	@echo "  make lint          - Run clippy lint checks"
	@echo "  make test          - Run tests"
	@echo "  make build         - Build project"
	@echo "  make check-all     - fmt -> lint -> test -> build"
	@echo "  make check-all-llm - fmt -> lint -> test -> build with local-llm feature"

fmt:
	cargo fmt --all
	cargo fmt --all --check

lint:
	cargo clippy --workspace --all-targets --no-default-features -- \
		-W clippy::all \
		-W clippy::pedantic \
		-W clippy::cargo \
		-W clippy::nursery \
		-W clippy::dbg_macro \
		-W clippy::todo \
		-W clippy::unwrap_used

test:
	cargo test --workspace --no-default-features

build:
	cargo build --workspace --no-default-features

check-all: fmt lint test build

check-all-llm: fmt
	cargo clippy --workspace --all-targets --features local-llm -- \
		-W clippy::all \
		-W clippy::pedantic \
		-W clippy::cargo \
		-W clippy::nursery \
		-W clippy::dbg_macro \
		-W clippy::todo \
		-W clippy::unwrap_used
	cargo test --workspace --features local-llm
	cargo build --workspace --features local-llm

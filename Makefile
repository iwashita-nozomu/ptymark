# Product-only developer entrypoints. Detailed Docker commands live in ptymark.mk.
include ptymark.mk

.PHONY: all build check ci ci-full fmt lint test runtime-dependencies verify-catalog dev clean release-metadata

all: check

build:
	cargo build --locked

fmt:
	cargo fmt --all -- --check

lint:
	cargo clippy --locked --all-targets -- -D warnings

test:
	cargo test --locked --all-targets

check: fmt lint test

ci: check

ci-full: ptymark-check

runtime-dependencies: ptymark-runtime-dependencies

verify-catalog: ptymark-verify-catalog

dev: ptymark-dev

clean: ptymark-clean

release-metadata:
	python3 scripts/check-release-metadata.py

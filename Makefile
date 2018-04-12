all: test

build:
	@cargo build --all

doc:
	@cargo doc

test: cargotest

cargotest:
	@cargo test --all --all-features

format-check:
	@cargo fmt -- --write-mode diff

.PHONY: all doc test cargotest format-check

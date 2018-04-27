all: test

build:
	@cargo build --all-features

doc:
	@cargo doc

test: cargotest

cargotest:
	@cargo test --all-features

format-check:
	@cargo fmt -- --write-mode diff

.PHONY: all doc test cargotest format-check

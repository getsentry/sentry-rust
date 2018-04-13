all: test

build:
	@cargo build --all

doc:
	@cargo doc

checkall:
	@echo 'ALL FEATURES'
	@RUSTFLAGS=-Dwarnings cargo check --all-features
	@echo 'NO DEFAULT FEATURES'
	@RUSTFLAGS=-Dwarnings cargo check --no-default-features
	@echo 'DEFAULT FEATURES'
	@RUSTFLAGS=-Dwarnings cargo check

test: checkall cargotest

cargotest:
	@cargo test --all --all-features

format-check:
	@cargo fmt -- --write-mode diff

.PHONY: all doc test cargotest format-check

all: test
.PHONY: all

check: style lint test
.PHONY: check

build:
	@cargo +stable build --all-features
.PHONY: build

doc:
	@cargo +stable doc
.PHONY: doc

test:
	@cargo +stable test --all-features
.PHONY: test

format:
	@rustup component add rustfmt --toolchain stable 2> /dev/null
	cargo +stable fmt
.PHONY: format

style:
	@rustup component add rustfmt --toolchain stable 2> /dev/null
	cargo +stable fmt -- --check
.PHONY: style

lint:
	@rustup component add clippy --toolchain stable 2> /dev/null
	cargo +stable clippy --all-features --all --tests --examples -- -D clippy::all
.PHONY: lint

all: check
.PHONY: all

clean:
	@cargo clean
.PHONY: clean

build:
	@cargo build
.PHONY: build

doc:
	@cargo doc
.PHONY: doc

check: style lint test
.PHONY: check

# Linting

style:
	@rustup component add rustfmt --toolchain stable 2> /dev/null
	cargo +stable fmt -- --check
.PHONY: style

format:
	@rustup component add rustfmt --toolchain stable 2> /dev/null
	cargo +stable fmt
.PHONY: format

lint:
	@rustup component add clippy --toolchain stable 2> /dev/null
	cargo +stable clippy --all-features --tests --examples -- -D clippy::all
.PHONY: lint

# Tests

test: checkall testall
.PHONY: test

testfast:
	@echo 'TESTSUITE'
	cd sentry && cargo test --features=test
.PHONY: testfast

testall:
	@echo 'TESTSUITE'
	cargo test --all-features
.PHONY: testall

# Checks

checkfast: check-no-default-features check-default-features
.PHONY: checkfast

checkall: check-all-features check-no-default-features check-default-features check-panic check-curl-transport check-actix
.PHONY: checkall

check-all-features:
	@echo 'ALL FEATURES'
	@RUSTFLAGS=-Dwarnings cargo check --all-features
.PHONY: check-all-features

check-default-features:
	@echo 'DEFAULT FEATURES'
	@RUSTFLAGS=-Dwarnings cargo check
.PHONY: check-default-features

check-no-default-features:
	@echo 'NO DEFAULT FEATURES'
	@cd sentry && RUSTFLAGS=-Dwarnings cargo check --no-default-features
.PHONY: check-no-default-features

check-panic:
	@echo 'NO CLIENT + PANIC'
	@cd sentry && RUSTFLAGS=-Dwarnings cargo check --no-default-features --features 'panic'
.PHONY: check-panic

check-curl-transport:
	@echo 'CURL TRANSPORT'
	@cd sentry && RUSTFLAGS=-Dwarnings cargo check --features curl
	@echo 'CURL TRANSPORT ONLY'
	@cd sentry && RUSTFLAGS=-Dwarnings cargo check --no-default-features --features 'curl,panic'
.PHONY: check-curl-transport

check-actix:
	@echo 'ACTIX INTEGRATION'
	@cd sentry-actix && RUSTFLAGS=-Dwarnings cargo check
.PHONY: check-actix

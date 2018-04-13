all: test

build:
	@cargo build --all

doc:
	@cargo doc

check-all-features:
	@echo 'ALL FEATURES'
	@RUSTFLAGS=-Dwarnings cargo check --all-features

check-no-default-features:
	@echo 'NO DEFAULT FEATURES'
	@RUSTFLAGS=-Dwarnings cargo check --no-default-features

check-default-features:
	@echo 'DEFAULT FEATURES'
	@RUSTFLAGS=-Dwarnings cargo check

check-failure:
	@echo 'NO CLIENT + FAILURE'
	@RUSTFLAGS=-Dwarnings cargo check --no-default-features --features 'with_failure'

check-log:
	@echo 'NO CLIENT + LOG'
	@RUSTFLAGS=-Dwarnings cargo check --no-default-features --features 'with_log'

check-panic:
	@echo 'NO CLIENT + PANIC'
	@RUSTFLAGS=-Dwarnings cargo check --no-default-features --features 'with_panic'

check-error-chain:
	@echo 'NO CLIENT + ERROR_CHAIN'
	@RUSTFLAGS=-Dwarnings cargo check --no-default-features --features 'with_error_chain'

check-all-impls:
	@echo 'NO CLIENT + ALL IMPLS'
	@RUSTFLAGS=-Dwarnings cargo check --no-default-features --features 'with_failure,with_log,with_panic,with_error_chain'

checkall: check-all-features check-no-default-features check-default-features check-failure check-log check-panic check-error-chain check-all-impls

test: checkall cargotest

cargotest:
	@cargo test --all --all-features

format-check:
	@cargo fmt -- --write-mode diff

.PHONY: all doc test cargotest format-check

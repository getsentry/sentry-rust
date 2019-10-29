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
	cargo +stable clippy --all-features --tests --all --examples -- -D clippy::all
.PHONY: lint

# Tests

test: checkall testall
.PHONY: test

testfast:
	@echo 'TESTSUITE'
	cargo test --features=with_test_support
.PHONY: testfast

testall:
	@echo 'TESTSUITE'
	cargo test --all-features --all
.PHONY: testall

# Checks

checkfast: check-no-default-features check-default-features
.PHONY: checkfast

checkall: check-all-features check-no-default-features check-default-features check-failure check-log check-panic check-error-chain check-anyhow check-all-impls check-curl-transport check-actix
.PHONY: checkall

check-all-features:
	@echo 'ALL FEATURES'
	@RUSTFLAGS=-Dwarnings cargo check --all-features --all
.PHONY: check-all-features

check-no-default-features:
	@echo 'NO DEFAULT FEATURES'
	@RUSTFLAGS=-Dwarnings cargo check --no-default-features
.PHONY: check-no-default-features

check-default-features:
	@echo 'DEFAULT FEATURES'
	@RUSTFLAGS=-Dwarnings cargo check
.PHONY: check-default-features

check-failure:
	@echo 'NO CLIENT + FAILURE'
	@RUSTFLAGS=-Dwarnings cargo check --no-default-features --features 'with_failure'
.PHONY: check-failure

check-log:
	@echo 'NO CLIENT + LOG'
	@RUSTFLAGS=-Dwarnings cargo check --no-default-features --features 'with_log'
.PHONY: check-log

check-panic:
	@echo 'NO CLIENT + PANIC'
	@RUSTFLAGS=-Dwarnings cargo check --no-default-features --features 'with_panic'
.PHONY: check-panic

check-error-chain:
	@echo 'NO CLIENT + ERROR_CHAIN'
	@RUSTFLAGS=-Dwarnings cargo check --no-default-features --features 'with_error_chain'
.PHONY: check-error-chain

check-anyhow:
	@echo 'NO CLIENT + ANYHOW'
	@RUSTFLAGS=-Dwarnings cargo check --no-default-features --features 'with_anyhow'
.PHONY: check-anyhow

check-all-impls:
	@echo 'NO CLIENT + ALL IMPLS'
	@RUSTFLAGS=-Dwarnings cargo check --no-default-features --features 'with_failure,with_log,with_panic,with_error_chain'
.PHONY: check-all-impls

check-curl-transport:
	@echo 'CURL TRANSPORT'
	@RUSTFLAGS=-Dwarnings cargo check --features with_curl_transport
	@echo 'CURL TRANSPORT ONLY'
	@RUSTFLAGS=-Dwarnings cargo check --no-default-features --features 'with_curl_transport,with_client_implementation,with_panic'
.PHONY: check-curl-transport

check-actix:
	@echo 'ACTIX INTEGRATION'
	@RUSTFLAGS=-Dwarnings cargo check --manifest-path integrations/sentry-actix/Cargo.toml
.PHONY: check-actix

# Other

travis-push-docs:
	@# Intentionally allow command output
	cargo doc --no-deps
	cp misc/docs/index.html target/doc/
	cd target/ && zip -r gh-pages ./doc
	npm install -g @zeus-ci/cli
	zeus upload -t "application/zip+docs" target/gh-pages.zip
.PHONY: travis-push-docs

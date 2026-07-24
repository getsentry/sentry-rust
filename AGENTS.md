# AGENTS.md

## Cursor Cloud specific instructions

This repo is the **Sentry SDK for Rust** — a Cargo workspace of library crates (`sentry`,
`sentry-core`, `sentry-types`, and integration crates). There is no long-running server or
GUI; the "applications" are the examples under `sentry/examples/` and `sentry-actix/examples/`.

### Toolchain / environment (already provisioned by the update script + snapshot)
- MSRV is **1.88** (`Cargo.toml` `rust-version`), so the default toolchain is Rust **stable**
  (>= 1.88). The base image's older 1.83 toolchain is not sufficient — stable is installed and
  set as default in the snapshot, with `rustfmt` and `clippy` components.
- System libs `libssl-dev` and `pkg-config` are required (the `native-tls`/`openssl-sys` and
  `curl` code paths pull in OpenSSL). These are baked into the snapshot. If a build fails with
  "system library `openssl` was not found", reinstall with
  `sudo apt-get install -y libssl-dev pkg-config`.

### Standard commands (mirror CI in `.github/workflows/`)
- Lint: `cargo fmt --all -- --check` and
  `cargo clippy --all-features --workspace --tests --examples --locked -- -D clippy::all`
- Build/check: `cargo check --all-features --locked`
- Test: `cargo test --workspace --all-features --all-targets --locked` plus
  `cargo test --workspace --all-features --doc --locked`
- CI sets `RUSTFLAGS: -Dwarnings`; export it when reproducing CI locally.

### Running an example (the "app")
- `cargo run --example message-demo` runs, but with **no DSN the client is disabled** and no
  event is transmitted (it just logs "initialized disabled sentry client").
- To exercise the full capture → serialize → transport path, provide a DSN via the
  `SENTRY_DSN` env var (read automatically by `sentry::init`), e.g. point it at a local mock
  ingest server: `SENTRY_DSN="http://key@127.0.0.1:9999/42" cargo run --example message-demo`.
  A real event envelope is then POSTed to `/api/<project_id>/envelope/`.

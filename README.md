<p align="center">
  <a href="https://sentry.io" target="_blank" align="center">
    <img src="https://sentry-brand.storage.googleapis.com/sentry-logo-black.png" width="280">
  </a>
  <br />
</p>

# Sentry SDK for Rust

[![Build Status](https://travis-ci.com/getsentry/sentry-rust.svg?branch=master)](https://travis-ci.com/getsentry/sentry-rust)

This workspace contains various crates that provide support for logging events and errors / panics to the
[Sentry](https://sentry.io/) error logging service.

- [sentry](./sentry) [![crates.io](https://img.shields.io/crates/v/sentry.svg)](https://crates.io/crates/sentry)
  [![docs.rs](https://docs.rs/sentry/badge.svg)](https://docs.rs/sentry)

  The main `sentry` crate aimed at application users that want to log events to sentry.

- [sentry-actix](./sentry-actix)
  [![crates.io](https://img.shields.io/crates/v/sentry-actix.svg)](https://crates.io/crates/sentry-actix)
  [![docs.rs](https://docs.rs/sentry-actix/badge.svg)](https://docs.rs/sentry-actix)

  An integration for the `actix-web (3.0+)` framework.

- [sentry-anyhow](./sentry-anyhow)
  [![crates.io](https://img.shields.io/crates/v/sentry-anyhow.svg)](https://crates.io/crates/sentry-anyhow)
  [![docs.rs](https://docs.rs/sentry-anyhow/badge.svg)](https://docs.rs/sentry-anyhow)

  An integration for `anyhow` errors.

- [sentry-backtrace](./sentry-backtrace)
  [![crates.io](https://img.shields.io/crates/v/sentry-backtrace.svg)](https://crates.io/crates/sentry-backtrace)
  [![docs.rs](https://docs.rs/sentry-backtrace/badge.svg)](https://docs.rs/sentry-backtrace)

  A utility crate that creates and processes backtraces.

- [sentry-contexts](./sentry-contexts)
  [![crates.io](https://img.shields.io/crates/v/sentry-contexts.svg)](https://crates.io/crates/sentry-contexts)
  [![docs.rs](https://docs.rs/sentry-contexts/badge.svg)](https://docs.rs/sentry-contexts)

  An integration that provides `os`, `device` and `rust` contexts.

- [sentry-core](./sentry-core)
  [![crates.io](https://img.shields.io/crates/v/sentry-core.svg)](https://crates.io/crates/sentry-core)
  [![docs.rs](https://docs.rs/sentry-core/badge.svg)](https://docs.rs/sentry-core)

  The core of `sentry`, which can be used to instrument code, and to write integrations that generate events or hook
  into event processing.

- [sentry-debug-images](./sentry-debug-images)
  [![crates.io](https://img.shields.io/crates/v/sentry-debug-images.svg)](https://crates.io/crates/sentry-debug-images)
  [![docs.rs](https://docs.rs/sentry-debug-images/badge.svg)](https://docs.rs/sentry-debug-images)

  An integration that adds a list of loaded libraries to events.

- [sentry-error-chain](./sentry-error-chain)
  [![crates.io](https://img.shields.io/crates/v/sentry-error-chain.svg)](https://crates.io/crates/sentry-error-chain)
  [![docs.rs](https://docs.rs/sentry-error-chain/badge.svg)](https://docs.rs/sentry-error-chain)

  An integration for the `error-chain` crate. This is _deprecated_ and will be completely removed in the future.

- [sentry-failure](./sentry-failure)
  [![crates.io](https://img.shields.io/crates/v/sentry-failure.svg)](https://crates.io/crates/sentry-failure)
  [![docs.rs](https://docs.rs/sentry-failure/badge.svg)](https://docs.rs/sentry-failure)

  An integration for the `failure` crate. This is _deprecated_ and will be completely removed in the future.

- [sentry-log](./sentry-log)
  [![crates.io](https://img.shields.io/crates/v/sentry-log.svg)](https://crates.io/crates/sentry-log)
  [![docs.rs](https://docs.rs/sentry-log/badge.svg)](https://docs.rs/sentry-log)

  An integration for the `log` and `env_logger` crate.

- [sentry-panic](./sentry-panic)
  [![crates.io](https://img.shields.io/crates/v/sentry-panic.svg)](https://crates.io/crates/sentry-panic)
  [![docs.rs](https://docs.rs/sentry-panic/badge.svg)](https://docs.rs/sentry-panic)

  An integration for capturing and logging panics.

- [sentry-slog](./sentry-slog)
  [![crates.io](https://img.shields.io/crates/v/sentry-slog.svg)](https://crates.io/crates/sentry-slog)
  [![docs.rs](https://docs.rs/sentry-slog/badge.svg)](https://docs.rs/sentry-slog)

  An integration for the `slog` crate.

- [sentry-types](./sentry-types)
  [![crates.io](https://img.shields.io/crates/v/sentry-types.svg)](https://crates.io/crates/sentry-types)
  [![docs.rs](https://docs.rs/sentry-types/badge.svg)](https://docs.rs/sentry-types)

  Contains types for the Sentry v7 protocol as well as other common types.

**Note**: Until the _1.0_ release, the crates in this repository are considered work in progress and do not follow
semver semantics. Between minor releases, we might occasionally introduce breaking changes while we are exploring the
best API and adding new features.

## Requirements

We currently only verify this crate against a recent version of Sentry hosted on [sentry.io](https://sentry.io/) but it
should work with on-prem Sentry versions 8.20 and later.

Additionally, the lowest Rust version we target is _1.42.0_.

## Resources

- [Discord](https://discord.gg/ez5KZN7) server for project discussions.
- Follow [@getsentry](https://twitter.com/getsentry) on Twitter for updates

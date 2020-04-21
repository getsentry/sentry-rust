<p align="center">
  <a href="https://sentry.io" target="_blank" align="center">
    <img src="https://sentry-brand.storage.googleapis.com/sentry-logo-black.png" width="280">
  </a>
  <br />
</p>

# Sentry-Core

[![Build Status](https://travis-ci.com/getsentry/sentry-rust.svg?branch=master)](https://travis-ci.com/getsentry/sentry-rust)
[![Crates.io](https://img.shields.io/crates/v/sentry-core.svg?style=flat)](https://crates.io/crates/sentry-core)

The core of `sentry`, which can be used to instrument code, and to write
integrations that generate events or hook into event processing.

**Note**: Until the _1.0_ release, the `sentry` crate is considered work in
progress and does not follow semver semantics. Between minor releases, we might
occasionally introduce breaking changes while we are exploring the best API and
adding new features.

## Requirements

We currently only verify this crate against a recent version of Sentry hosted on
[sentry.io](https://sentry.io/) but it should work with on-prem Sentry versions
8.20 and later.

Additionally, the lowest Rust version we target is _1.40.0_.

## Resources

- [crates.io](https://crates.io/crates/sentry-core)
- [Documentation](https://getsentry.github.io/sentry-rust)
- [Bug Tracker](https://github.com/getsentry/sentry-rust/issues)
- [Discord](https://discord.gg/ez5KZN7) server for project discussions.
- Follow [@getsentry](https://twitter.com/getsentry) on Twitter for updates

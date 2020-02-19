<p align="center">
  <a href="https://sentry.io" target="_blank" align="center">
    <img src="https://sentry-brand.storage.googleapis.com/sentry-logo-black.png" width="280">
  </a>
  <br />
</p>

# Sentry Rust

[![Build Status](https://travis-ci.com/getsentry/sentry-rust.svg?branch=master)](https://travis-ci.com/getsentry/sentry-rust)
[![Crates.io](https://img.shields.io/crates/v/sentry.svg?style=flat)](https://crates.io/crates/sentry)

This crate provides support for logging events and errors / panics to the
[Sentry](https://sentry.io/) error logging service. It integrates with the
standard panic system in Rust as well as a few popular error handling setups.

**Note**: Until the _1.0_ release, the `sentry` crate is considered work in
progress and does not follow semver semantics. Between minor releases, we might
occasionally introduce breaking changes while we are exploring the best API and
adding new features.

## Requirements

We currently only verify this crate against a recent version of Sentry hosted on
[sentry.io](https://sentry.io/) but it should work with on-prem Sentry versions
8.20 and later.

Additionally, the lowest Rust version we target is _1.39.0_.

## Resources

- [crates.io](https://crates.io/crates/sentry)
- [Documentation](https://getsentry.github.io/sentry-rust)
- [Bug Tracker](https://github.com/getsentry/sentry-rust/issues)
- [Discord](https://discord.gg/ez5KZN7) server for project discussions.
- Follow [@getsentry](https://twitter.com/getsentry) on Twitter for updates

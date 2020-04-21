# Changelog

## 0.18.0

- Upgrade most dependencies to their current versions (#183):
  - `env_logger 0.7`
  - `reqwest 0.10`
  - `error-chain 0.12`
  - `url 2.1`
  - `sentry-types 0.14`

- Remove the `log` and `env_logger` integration from default features (#183).
- Fix backtraces for newer `failure` and `backtrace` versions (#183).
- Fix compilation of the `with_rust_info` feature (#183).
- Add "panics" sections to functions that may panic (#174).
- Document all feature flags consistently.

## 0.17.0

- Upgrade findshlibs (#153)

## 0.16.0

- Bump the minimum required Rust version to **1.34.0**.
- Fix potentially broken payloads when using the `curl` transport (#152).
- Report the SDK as `sentry.rust` for analytics (#142).

## 0.15.5

- Fix backtraces with inline frames in newer Rust versions (#141).

## 0.15.4

- Added a feature flag to disable the default sentry features in sentry-actix. (#139)

## 0.15.3

- Added `with_rustls` and `with_native_tls` features to control SSL in the default
  reqwest transport. (#137)

## 0.15.2

- Added support for passing custom reqwest clients/curl handles to the transport (#130)

## 0.15.1

- Correct dependency bump for sentry types.

## 0.15.0

- Upgraded to newer version of the internal sentry types crate.

## 0.14.2

- Fixed a potential issue where an event might have been dropped if it was sent
  right after the curl transport was created.

## 0.14.1

- Fixed an issue where turning off the http transports would cause a compile error.

## 0.14.0

- Added support for reading `SENTRY_ENVIRONMENT` and `SENTRY_RELEASE` environment
  variables.
- Added support for panicking with failure errors.
- Added `attach_stacktraces` configuration option to logging integration
- Fixed a bug where `emit_breadcrumbs` was incorrectly handled.
- Restructured the transport system. You can now disable the builtin HTTP
  reqwest based transport or opt for the curl transport.
- Fixed a potential issue where an event might have been dropped if it was sent
  right after the reqwest transport was created.
- Added support for server side symbolication for linux platforms.

## 0.13.0

**Breaking Change**: This release requires Rust 1.31 or newer.

- Upgrade the logger integration to `env_logger:0.6`
- Support debug identifiers of loaded images on Linux (#114)
- Update `sentry-types` to the latest version
- Fix `log::log_enabled!` when log integration is active

## 0.12.1

- Resolve a memory leak in the actix integration.
- Fix an issue where dropping a scope guard for a non active hub resulted in a
  panic.
- Added support for the new failure `Fail::name`
- Improved support for actix failure based error
- Added `RefUnwindSafe` for `ClientOptions`
- Remove the never supported `repos` option.

## 0.12.0

- Upgrade reqwest to 0.9
- Add support for debug logging through the log crate (`with_debug_to_log` feature)
- Added debug log for when events are dropped in the transport.

## 0.11.1

- Fix compilation error in `sentry-actix` (#87)

## 0.11.0

- Added `sentry::with_scope`
- Updated the sentry types to 0.8.x

## 0.10.0

- Streamline types with other SDKs. Most notabe changes:
  - `Event::id` has been renamed to `Event::event_id`
  - `Event::exceptions` has been renamed to `Event::exception`
  - Some collections are now wrapped in `Values`
- Added more debug logs.

## 0.9.0

- Removed `drain_events`. Events are now drained by calling `Client::close` or on the
  transport on `Transport::shutdown`.
- Removed `Hub::add_event_processor`. This was replaced by `Scope::add_event_processor`
  which is easier to use (only returns factory function)/
- Added various new client configuration values.
- Unified option handling

This is likely to be the final API before 1.0

## 0.3.1

- Remove null byte terminator from device model context (#33)
- Fix `uname` breaking builds on Windows (#32)
- Fix the crate documentation link (#31)

# Changelog

## Unreleased

**Breaking Changes**:

- Bump the minimum required Rust version to **1.42.0**.
- The `actix` integration / middleware is now compatible with `actix-web 3`.
- Removed all deprecated exports and deprecated feature flags.
- The `failure` integration / feature is now off-by-default along with its deprecation.
- The `log` and `slog` integrations were re-designed, they now offer types that wrap a `log::Log` or `slog::Drain` and forward log events to the currently active sentry `Hub` based on an optional filter and an optional mapper.
- The new `log` integration will not implicitly call `log::set_max_level_filter` anymore, and users need to do so manually.

**Deprecations**:

- The `error-chain` and `failure` integration was officially deprecated and will be removed soon.

## 0.20.1

**Fixes**:

- Fixed a deadlock when nesting `configure_scope` calls.
- Improved transport shutdown logic and fixed a potential deadlock on shutdown.

## 0.20.0

**Highlights**:

- The Rust SDK now has **experimental** support for [Release Health Sessions](https://docs.sentry.io/product/releases/health/) using the `start_session` and `end_session` API (global and on the `Hub`).

**Breaking Changes**:

- The `Transport` was changed to work on `Envelope`s instead of `Event`s. The `send_event` trait function was removed in favor of `send_envelope`.

**Features**:

- The `Envelope`, `SessionUpdate`, and other related types have been added to the `sentry_types::protocol::v7` module.
- A `clear_breadcrumbs` method was added to `Scope`.
- `sentry_contexts::utils` is now public.

**Fixes**:

- Panic events now have a proper `mechanism`.

**Deprecations**:

- The `Future` and `FutureExt` exports have been renamed to `SentryFuture` and `SentryFutureExt` respectively.

**Thank you**:

Features, fixes and improvements in this release have been contributed by:

- [@Jake-Shadle](https://github.com/Jake-Shadle)
- [@maxcountryman](https://github.com/maxcountryman)
- [@ErichDonGubler](https://github.com/ErichDonGubler)
- [@nCrazed](https://github.com/nCrazed)
- [@jrconlin](https://github.com/jrconlin)

## 0.19.1

**Fixes**:

- Better deal with concurrent Hub access.

## 0.19.0

**Highlights**:

The `sentry` crate has been split up into a `sentry-core`, and many smaller per-integration crates. Application users should continue using the `sentry` crate, but library users and integration/transport authors are encouraged to use the `sentry-core` crate instead.

Additionally, sentry can now be extended via `Integration`s.

**Breaking Changes**:

- The `utils` module has been removed, and most utils have been moved into integrations.
- The `integrations` module was completely rewritten.
- When constructing a `Client` using a `ClientOptions` struct manually, it does not have any default integrations, and it does not resolve default options from environment variables any more. Please use the explicit `apply_defaults` function instead. The `init` function will automatically call `apply_defaults`.
- The `init` function can’t be called with a `Client` anymore.

**Features**:

- Sentry can now capture `std::error::Error` types, using the `capture_error` and `Hub::capture_error` functions, and an additional `event_from_error` utility function.
- Sentry now has built-in support to bind a `Hub` to a `Future`.
- Sentry can now be extended with `Integration`s.
- The `ClientInitGuard`, `Future` and `ScopeGuard` structs and `apply_defaults`, `capture_error`, `event_from_error`, `with_integration` and `parse_type_from_debug` functions have been added to the root exports.
- The `FutureExt`, `Integration`, `IntoBreadcrumbs`, `IntoDsn`, `Transport` and `TransportFactory` traits are now exported.
- The `types` module now re-exports `sentry-types`.

**Deprecations**:

- The `internals` module is deprecated. Please `use` items from the crate root or the `types` module instead.
- All the feature flags have been renamed, the old names are still available but will be removed in the future.

## 0.18.1

- Fix potential segfault with `with_debug_meta` (#211).
- Fix panic when running inside of tokio (#186).

## 0.18.0

- Bump the minimum required Rust version to **1.40.0**.
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

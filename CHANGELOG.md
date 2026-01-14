# Changelog

## Unreleased

### Fixes

- Fixed thread corruption bug where `HubSwitchGuard` could be dropped on wrong thread ([#957](https://github.com/getsentry/sentry-rust/pull/957))
  - **Breaking change**: `sentry_core::HubSwitchGuard` is now `!Send`, preventing it from being moved across threads. Code that previously sent the guard to another thread will no longer compile.

## 0.46.1

### Improvements

- Make it possible to == Transaction/Span/TransactionOrSpan ([#942](https://github.com/getsentry/sentry-rust/pull/942))

### Dependencies

- Update reqwest from 0.12.15 to 0.12.25 ([#951](https://github.com/getsentry/sentry-rust/pull/951))

## 0.46.0

### Breaking changes

- Removed the `ClientOptions` struct's `trim_backtraces` and `extra_border_frames` fields ([#925](https://github.com/getsentry/sentry-rust/pull/925)).
  - These fields configured backtrace trimming, which is being removed in this release.

### Improvements

- Removed backtrace trimming to align the Rust SDK with the general principle that Sentry SDKs should only truncate telemetry data when needed to comply with [documented size limits](https://develop.sentry.dev/sdk/data-model/envelopes/#size-limits) ([#925](https://github.com/getsentry/sentry-rust/pull/925)). This change ensures that as much data as possible remains available for debugging.
  - If you notice any new issues being created for existing errors after this change, please open an issue on [GitHub](https://github.com/getsentry/sentry-rust/issues/new/choose).

### Fixes

- fix: adjust sentry.origin for log integration ([#919](https://github.com/getsentry/sentry-rust/pull/919)) by @lcian

## 0.45.0

### Breaking changes

- Add custom variant to `AttachmentType` that holds an arbitrary String. ([#916](https://github.com/getsentry/sentry-rust/pull/916))

## 0.44.0

### Breaking changes

- feat(log): support combined LogFilters and RecordMappings ([#914](https://github.com/getsentry/sentry-rust/pull/914)) by @lcian
  - Breaking change: `sentry::integrations::log::LogFilter` has been changed to a `bitflags` struct.
  - It's now possible to map a `log` record to multiple items in Sentry by combining multiple log filters in the filter, e.g. `log::Level::ERROR => LogFilter::Event | LogFilter::Log`.
  - If using a custom `mapper` instead, it's possible to return a `Vec<sentry::integrations::log::RecordMapping>` to map a `log` record to multiple items in Sentry. 

### Behavioral changes

- ref(log): send logs by default when logs feature flag is enabled ([#915](https://github.com/getsentry/sentry-rust/pull/915)) by @lcian
  - If the `logs` feature flag is enabled, the default Sentry `log` logger now sends logs for all events at or above INFO.
- ref(logs): enable logs by default if logs feature flag is used ([#910](https://github.com/getsentry/sentry-rust/pull/910)) by @lcian
  - This changes the default value of `sentry::ClientOptions::enable_logs` to `true`.
  - This simplifies the setup of Sentry structured logs by requiring users to just add the `log` feature flag to the `sentry` dependency to opt-in to sending logs.
  - When the `log` feature flag is enabled, the `tracing` and `log` integrations will send structured logs to Sentry for all logs/events at or above INFO level by default.

## 0.43.0

### Breaking changes

- ref(tracing): rework tracing to Sentry span name/op conversion ([#887](https://github.com/getsentry/sentry-rust/pull/887)) by @lcian
  - The `tracing` integration now uses the tracing span name as the Sentry span name by default.
  - Before this change, the span name would be set based on the `tracing` span target (`<module>::<function>` when using the `tracing::instrument` macro).
  - The `tracing` integration now uses `<span target>::<span name>` as the default Sentry span op (i.e. `<module>::<function>` when using `tracing::instrument`).
  - Before this change, the span op would be set based on the `tracing` span name.
  - Read below to learn how to customize the span name and op.
  - When upgrading, please ensure to adapt any queries, metrics or dashboards to use the new span names/ops.
- ref(tracing): use standard code attributes ([#899](https://github.com/getsentry/sentry-rust/pull/899)) by @lcian
  - Logs now carry the attributes `code.module.name`, `code.file.path` and `code.line.number` standardized in OTEL to surface the respective information, in contrast with the previously sent `tracing.module_path`, `tracing.file` and `tracing.line`.
- fix(actix): capture only server errors ([#877](https://github.com/getsentry/sentry-rust/pull/877)) by @lcian
  - The Actix integration now properly honors the `capture_server_errors` option (enabled by default), capturing errors returned by middleware only if they are server errors (HTTP status code 5xx).
  - Previously, if a middleware were to process the request after the Sentry middleware and return an error, our middleware would always capture it and send it to Sentry, regardless if it was a client, server or some other kind of error.
  - With this change, we capture errors returned by middleware only if those errors can be classified as server errors.
  - There is no change in behavior when it comes to errors returned by services, in which case the Sentry middleware only captures server errors exclusively.
- fix: send trace origin correctly ([#906](https://github.com/getsentry/sentry-rust/pull/906)) by @lcian
  - `TraceContext` now has an additional field `origin`, used to report which integration created a transaction.

### Behavioral changes

- feat(tracing): send both breadcrumbs and logs by default ([#878](https://github.com/getsentry/sentry-rust/pull/878)) by @lcian
  - If the `logs` feature flag is enabled, and `enable_logs: true` is set on your client options, the default Sentry `tracing` layer now sends logs for all events at or above INFO.

### Features

- ref(tracing): rework tracing to Sentry span name/op conversion ([#887](https://github.com/getsentry/sentry-rust/pull/887)) by @lcian
  - Additional special fields have been added that allow overriding certain data on the Sentry span:
    - `sentry.op`: override the Sentry span op.
    - `sentry.name`: override the Sentry span name.
    - `sentry.trace`: given a string matching a valid `sentry-trace` header (sent automatically by client SDKs), continues the distributed trace instead of starting a new one. If the value is not a valid `sentry-trace` header or a trace is already started, this value is ignored.
  - `sentry.op` and `sentry.name` can also be applied retroactively by declaring fields with value `tracing::field::Empty` and then recorded using `tracing::Span::record`.
  - Example usage:
    ```rust
    #[tracing::instrument(skip_all, fields(
        sentry.op = "http.server",
        sentry.name = "GET /payments",
        sentry.trace = headers.get("sentry-trace").unwrap_or(&"".to_owned()),
    ))]
    async fn handle_request(headers: std::collections::HashMap<String, String>) {
        // ...
    }
    ```
  - Additional attributes are sent along with each span by default:
    - `sentry.tracing.target`: corresponds to the `tracing` span's `metadata.target()`
    - `code.module.name`, `code.file.path`, `code.line.number`

- feat(core): add Response context ([#874](https://github.com/getsentry/sentry-rust/pull/874)) by @lcian
  - The `Response` context can now be attached to events, to include information about HTTP responses such as headers, cookies and status code.
  - Example:
    ```rust
    let mut event = Event::new();
    let response = ResponseContext {
        cookies: Some(r#""csrftoken": "1234567""#.to_owned()),
        headers: Some(headers_map),
        status_code: Some(500),
        body_size: Some(15),
        data: Some("Invalid request"),
    };
    event
        .contexts
        .insert("response".to_owned(), response.into());
    ```

### Fixes

- build(panic): Fix build without other dependencies ([#883](https://github.com/getsentry/sentry-rust/pull/883)) by @liskin
  - The `sentry-panic` crate now builds successfully when used as a standalone dependency.
- fix(transport): add rate limits for logs ([#894](https://github.com/getsentry/sentry-rust/pull/894)) by @giortzisg

## 0.42.0

### Features

- feat(log): support kv feature of log (#851) by @lcian
  - Attributes added to a `log` record using the `kv` feature are now recorded as attributes on the log sent to Sentry.
- feat(types): add all the missing supported envelope headers ([#867](https://github.com/getsentry/sentry-rust/pull/867)) by @lcian
- feat(types): add setters for envelope headers ([#868](https://github.com/getsentry/sentry-rust/pull/868)) by @lcian
  - It's now possible to set all of the [envelope headers](https://develop.sentry.dev/sdk/data-model/envelopes/#headers) supported by the protocol when constructing envelopes.
- feat(core): add some DSC fields to transaction envelope headers ([#869](https://github.com/getsentry/sentry-rust/pull/869)) by @lcian
  - The SDK now sends additional envelope headers with transactions. This should solve some extrapolation issues for span metrics.

### Behavioral changes

- feat: filter username and password in URLs ([#864](https://github.com/getsentry/sentry-rust/pull/864)) by @lcian
  - Usernames and passwords that could be contained in URLs captured when using the Actix Web or axum integration are now always filtered out.
  - If the `Request` is created manually by the user, then these fields are not filtered out.
  - This information was already filtered by Relay, but should also be filtered by the SDK itself as a first line of defense.

### Fixes

- docs: match description of `debug` option with behavior since PR #820 ([#860](https://github.com/getsentry/sentry-rust/pull/860)) by @AlexTMjugador

## 0.41.0

### Breaking changes

- feat(tracing): support combined EventFilters and EventMappings (#847) by @lcian
  - `EventFilter` has been changed to a `bitflags` struct.
  - It's now possible to map a `tracing` event to multiple items in Sentry by combining multiple event filters in the `event_filter`, e.g. `tracing::Level::ERROR => EventFilter::Event | EventFilter::Log`.
  - It's also possible to use `EventMapping::Combined` to map a `tracing` event to multiple items in Sentry.
  - `ctx` in the signatures of `event_from_event`, `breadcrumb_from_event` and `log_from_event` has been changed to take `impl Into<Option<&'context Context<'context, S>>>` to avoid cloning the `Context` when mapping to multiple items.

### Features

- feat(core): emit debug log when calling capture_log but logs are disabled (#849) by @lcian

### Fixes

- fix(logs): stringify u64 attributes greater than `i64::MAX` (#846) by @lcian

### Dependencies

- chore(deps): bump `anyhow` and disable its `backtrace` feature (#632) by @LunaBorowska

## 0.40.0

### Breaking changes

- refactor(logs): apply user attributes to log regardless of `send_default_pii` (#843) by @lcian
  - User attributes should be applied to logs regardless of `send_default_pii`. Therefore, that parameter was removed from `sentry_core::Scope::apply_to_log`.

### Features

- feat(tracing): add support for logs (#840) by @lcian
  - To capture `tracing` events as Sentry structured logs, enable the `logs` feature of the `sentry` crate.
  - Then, initialize the SDK with `enable_logs: true` in your client options.
  - Finally, set up a custom event filter to map events to logs based on criteria such as severity. For example:
  ```rust
      let sentry_layer = sentry_tracing::layer().event_filter(|md| match *md.level() {
          tracing::Level::ERROR => EventFilter::Event,
          tracing::Level::TRACE => EventFilter::Ignore,
          _ => EventFilter::Log,
      });
  ```
- feat(log): add support for logs (#841) by @lcian
  - To capture `log` records as Sentry structured logs, enable the `logs` feature of the `sentry` crate.
  - Then, initialize the SDK with `enable_logs: true` in your client options.
  - Finally, set up a custom event filter to map records to Sentry logs based on criteria such as severity. For example:
  ```rust
      let logger = sentry::integrations::log::SentryLogger::new().filter(|md| match md.level() {
          log::Level::Error => LogFilter::Event,
          log::Level::Trace => LogFilter::Ignore,
          _ => LogFilter::Log,
      });
  ```
- refactor(logs): cache default attributes and add OS attributes (#842) by @lcian
  - `os.name` and `os.version` are now being attached to logs as default attributes.

### Fixes

- fix(logs): send environment in `sentry.environment` default attribute (#837) by @lcian

### Behavioral changes

- refactor(tracing): refactor internal code and improve docs (#839) by @lcian
  - Errors carried by breadcrumbs will now be stored in the breadcrumb `data` under their original field name.
  - Before, they were all stored under a single key called `errors`.

### Dependencies

- chore(deps): upgrade `ureq` to 3.x (#835) by @algesten

## 0.39.0

### Features

Support for [Sentry structured logs](https://docs.sentry.io/product/explore/logs/) has been added to the SDK.
- To set up logs, enable the `logs` feature of the `sentry` crate and set `enable_logs` to `true` in your client options.
- Then, use the `logger_trace!`, `logger_debug!`, `logger_info!`, `logger_warn!`, `logger_error!` and `logger_fatal!` macros to capture logs.
- To filter or update logs before they are sent, you can use the `before_send_log` client option.
- Please note that breaking changes could occur until the API is finalized. 

- feat(logs): add log protocol types (#821) by @lcian
- feat(logs): add ability to capture and send logs (#823) by @lcian & @Swatinem
- feat(logs): add macro-based API (#827) by @lcian & @szokeasaurusrex
- feat(logs): send logs in batches (#831) by @lcian

### Behavioral changes

- feat(core): implement Tracing without Performance (#811) by @lcian
  - The SDK now implements Tracing without Performance, which makes it so that each `Scope` is associated with an object holding some tracing information.
  - This information is used as a fallback when capturing an event with tracing disabled or otherwise no ongoing span, to still allow related events to be linked by a trace.
  - A new API `Scope::iter_trace_propagation_headers` has been provided that will use the fallback tracing information if there is no current `Span` on the `Scope`.

### Breaking changes

- refactor: remove `debug-logs` feature (#820) by @lcian
  - The deprecated `debug-logs` feature of the `sentry` crate, used for the SDK's own internal logging, has been removed.

## 0.38.1

### Fixes

- build: include `sentry-actix` optionally when `release-health` is enabled (#806) by @lcian
  - `sentry-actix` is now being included as a dependency only when explicitly added, either as a direct dependency or through the `actix` feature flag of the `sentry` crate.
  - Due to a mistake in the `Cargo.toml`, it was previously being included as a dependency by default when using just the `sentry` crate with default features.

## 0.38.0

### OpenTelemetry integration

An OpenTelemetry integration has been released. Please refer to the changelog entry below for the details.

### Breaking changes

- refactor(tracing): remove `EventFilter::exception` and always attach exception (#768) by @lcian
  - The `EventFilter::Exception` enum variant has been removed. Please use `EventFilter::Event` instead to achieve the same behavior.
  - Using `EventFilter::Event` will always attach any error struct used within the `error` field passed to the `tracing` macro, as `EventFilter::Exception` did previously.
  - The `error` field will also be attached to breadcrumbs as an `errors` field resembling the structure of Sentry events created from error structs.
- fix: use `release-health` flag in `sentry-actix` and remove it from subcrates where unneeded (#787) by @lcian
  - As a follow-up from the changes in the previous release, the `ClientOptions` fields `auto_session_tracking` and `session_mode` are now gated behind the `release-health` feature flag of the `sentry` crate.
  - If you depend on `sentry` with `default-features = false`, you need to include the `release-health` feature flag to benefit from the [Release Health](https://docs.sentry.io/product/releases/health/) features of Sentry and have access to the aforementioned client options.
  - The `release-health` feature flag is used correctly in `sentry-actix` to enable compilation of that subcrate when it's disabled.
  - The `release-health` has been removed from the `sentry-tracing` and `sentry-tower` subcrates, where it was unnecessary.
- refactor: remove Surf transport (#766) by @lcian
  - The Surf transport has been removed as the `surf` crate is unmaintained and it was holding back dependency upgrades.
  - If you really want to still use Surf, you can define a custom `TransportFactory` and pass it as the `transport` in your `ClientOptions`

### Behavioral changes

- refactor: honor `send_default_pii` in `sentry-actix` and `sentry-tower` (#771) by @lcian
  - The client option `send_default_pii` (disabled by default) is now honored by `sentry-actix` and `sentry-tower`.
  - This means that potentially sensitive headers such as authorization, cookies, and those that usually contain the user's IP address are filtered and not sent to Sentry.
  - If you want to get back to the previous behavior and capture all headers, please set `send_default_pii` to `true` in your `ClientOptions`.
  - Please refer to our [Data Collected](https://docs.sentry.io/platforms/rust/data-management/data-collected/) page for a comprehensive view of the data collected by the SDK.
- refactor(debug-images): force init `DEBUG_META` on integration init (#773) by @lcian
  - The `DebugImages` integration has been updated to init the `DEBUG_META` `Lazy` immediately.
  - Using this integration is known to cause issues in specific versions of the Linux kernel due to issues in a library it depends on.
  - Previously, on problematic systems the SDK would cause deadlock after capturing the first event. Now the SDK will panic on initialization instead. Please open an issue if you're affected.

### Features

- feat(otel): add OpenTelemetry SpanProcessor, Propagator, Extractor (#779) by @lcian
  - A new integration for the `opentelemetry` crate has been released.
  - It can be used to capture spans created using the `opentelemetry` API and send them to Sentry.
  - Distributed tracing is also supported, provided that the upstream/downstream services support the Sentry or W3C distributed tracing metadata format.
  - Please refer to the subcrate's README or the crate docs to see an example of setup and usage.
- feat: expose `sentry-actix` as a feature of `sentry` (#788) by @lcian
  - `sentry-actix` is now exposed by the `sentry` crate as `sentry::integrations::actix`, gated behind the `actix` feature flag.
  - Please update your dependencies to not depend on the `sentry-actix` subcrate directly.

### Dependencies

- build(deps): bump openssl from 0.10.71 to 0.10.72 (#762) by @dependabot
- build(deps): bump tokio from 1.44.1 to 1.44.2 (#763) by @dependabot
- chore(deps): bump some dependencies and update `Cargo.lock` (#772) by @lcian

### Various fixes & improvements

- Replace `once_cell` with `std::sync::LazyLock` (#776) by @FalkWoldmann
- chore: update GH issue templates for Linear compatibility (#777) by @stephanie-anderson
- chore: update issue templates with blank issue and Discord link (#778) by @lcian
- refactor(core): fail with message if TLS backend not available (#784) by @lcian
- build: add `sentry-opentelemetry` to workspace (#789) by @lcian
- docs: update docs including OTEL and other integrations (#790) by @lcian
- fix(otel): fix doctests (#794) by @lcian
- fix(otel): fix span and trace ids for distributed tracing (#801) by @lcian
- build(otel): exclude version from circular dev-dependencies (#802) by @lcian

## 0.37.0

### Breaking changes

- chore(msrv): `cargo update` and bump MSRV to 1.81 (#754) by @lcian
  - The minimum supported Rust version has been raised to 1.81.
- feat(core): introduce `release-health` feature (#749) by @pepperoni505
  - A new `release-health` feature flag was introduced that gates the [Release Health](https://docs.sentry.io/product/releases/health/) features of Sentry.
  - This allows for compilation of the SDK on certain WASM targets.
  - Release Health features were already present and enabled with no feature flag in previous versions.
  - The new feature flag will be enabled by default when using `sentry`, `sentry-actix`, `sentry-tower` or `sentry-tracing` with the default features.
  - If you're fine-tuning your feature flags, make sure to enable `release-health` to get back the previous behavior.
- ref(metrics): remove features and code related to the old metrics beta (#740) by @lcian
  - The metrics feature and the code related to it has been removed from the crate, as the Sentry backend stopped ingesting metrics a while ago.
- Switch to MIT license (#724) by @cleptric
  - The license for the crates has been changed to MIT.
 
### Features

- feat(actix): capture HTTP request body (#731) by @pacifistes
  - The middleware for `actix-web` now supports capturing and attaching the request body to HTTP request transactions.
  - You need to enable `send_default_pii` in your client options for this to be enabled, and you can fine-tune the behavior using the new option `max_request_body_size`.
- feat(core): `transaction.set_data` sets data on `TraceContext` (#739) by @lcian
  - `transaction.set_data` now sets data on `TraceContext`, as the SDK should not use the `extra` field.
- ref(backtrace): add entries and extra logic for in-app detection (#756) by @lcian
- feat(core): add more frames to be considered not in_app (#760) by @lcian
  - The logic used by the SDK to detect `in-app` stack frames has been improved. Now the SDK will mark more frames as not `in-app`.
  - A similar improvement has been added to the Sentry [backend](https://github.com/getsentry/sentry/commit/cef4d53e05093d6e9c81c1c49585af86cc135f8b) so that old versions of the SDK can benefit from improved `in-app` reporting.

### Fixes

- fix(http): Finish transaction on drop (#727) by @Dav1dde
  - Fixed a bug where the current transaction was not finished (hence not sent to Sentry) when its corresponding future was dropped, e.g. due to a panic.
- follow https://github.com/getsentry/sentry-rust/pull/439 for actix-web. fix https://github.com/getsentry/sentry-rust/issues/680 (#737) by @pavel-rosputko
  - The HTTP request metadata is now being correctly attached to transactions when using `sentry-actix`.
- fix(tracing): wrap error with synthetic mechanism only if attaching stacktrace (#755) by @lcian
  - Fixed a bug that should result in improved grouping and issue titles for events reported by `sentry-tracing` when not capturing stack traces.
- fix(actix): process request in other middleware using correct Hub (#758) by @lcian
  - The subsequent middleware in the chain when processing a request now execute within the correct Hub.
- fix(anyhow): attach stacktrace only if error provides backtrace (#759) by @lcian
  - Fixed a bug where the SDK was providing incorrect stack traces when capturing an `anyhow` when the `backtrace` feature is enabled but `RUST_BACKTRACE` is not set.
  - This should result in correct grouping of the affected issues.

### Various fixes & improvements

- Fix CS (#726) by @cleptric
- fix(doctests): update prost (#750) by @lcian
- chore(msrv): bump MSRV to 1.75 (#751) by @lcian
- refactor(actix): simplify body_from_http (#757) by @robjtede
- chore: prepare changelog for release (#761) by @lcian

### Dependencies

- build(deps): bump openssl from 0.10.66 to 0.10.70 (#732) by @dependabot
- build(deps): bump ring from 0.17.8 to 0.17.13 (#747) by @dependabot

## 0.36.0

### Various fixes & improvements

- feat(sentry-tower) Make SentryLayer and SentryService `Sync` if request isn't (#721) by @syphar
- sentry-tower: Update `axum` dependency to v0.8 (#718) by @Turbo87
- Allow retrieving user of scope (#715) by @thomaseizinger
- Elide lifetimes where possible (#716) by @thomaseizinger
- Replace release bot with GH app (#714) by @Jeffreyhung
- Delay sampling of span to `finish` (#712) by @thomaseizinger

## 0.35.0

**Fixes**:

- Envelopes will be discarded rather than blocking if the transport channel fills up (previously fixed in async-capable transports, now applied to the curl/ureq transports). ([#701](https://github.com/getsentry/sentry-rust/pull/701))

## 0.34.0

**Features**:

- Renamed the `UNSTABLE_metrics` and `UNSTABLE_cadence` feature flags to `metrics` and `metrics-cadence1` respectively.

## 0.33.0

### Various fixes & improvements

- ref(metrics): Add normalization and update set metrics hashing (#658) by @elramen
- feat: add embedded-svc based http transport (#654) by @madmo

## 0.32.3

**Compatiblity**:

- Raised the MSRV to **1.73**.

**Improvements**:

- Slightly improved overhead of the `tracing` layer. ([#642](https://github.com/getsentry/sentry-rust/pull/642))

**Updates**:

- Updated `reqwest` to version `0.12`.
- Updated `tonic` to version `0.11`.

## 0.32.2

### Various fixes & improvements

- feat(crons): Add new fields to `MonitorConfig` type (#638) by @szokeasaurusrex
- build(deps): bump h2 from 0.3.22 to 0.3.24 (#635) by @dependabot
- fix(hub): avoid deadlocks when emitting events (#633) by @Tuetuopay

## 0.32.1

**Features**:

- Add experimental implementations for Sentry metrics and a cadence sink. These
  require to use the `UNSTABLE_metrics` and `UNSTABLE_cadence` feature flags.
  Note that these APIs are still under development and subject to change.

## 0.32.0

**Features**:

- Updated `sentry-tower` dependencies, including `axum` and `http`.

## 0.31.8

### Various fixes & improvements

- MonitorSchedule constructor that validates crontab syntax (#625) by @szokeasaurusrex
- fix(docs): Fix some doc errors that slipped in (#623) by @flub
- docs(tower): Mention how to enable http feature from sentry crate (#622) by @flub
- build(deps): bump rustix from 0.37.23 to 0.37.25 (#619) by @dependabot

## 0.31.7

### Various fixes & improvements

- The minimum supported Rust version was bumped to **1.68.0** due to requirements from dependencies. (#612)

## 0.31.6

### Various fixes & improvements

- Apply clippy fixes and cherry-pick PRs (#610) by @Swatinem
- ref: Apply user field from scope to transaction event (#596) by @kamilogorek
- Remove profiling support (#595) by @viglia
- chore: upgrade webpki-roots 0.22.5 -> 0.23.0 (#593) by @boxdot

## 0.31.5

### Various fixes & improvements

- chore(deps): bump rustls (#592) by @utkarshgupta137

## 0.31.4

### Various fixes & improvements

- Apply scope metadata to transactions (#590) by @loewenheim

## 0.31.3

### Various fixes & improvements

- feat(tracing): Improve structure for tracing errors (#585) by @jan-auer

## 0.31.2

### Various fixes & improvements

- feat(crons): Add monitor check-in types to sentry-types (#577) by @evanpurkhiser

## 0.31.1

**Features**:

- Add a new `(tower-)axum-matched-path` feature to use the `MatchedPath` as transaction name, along with attaching the request metadata to the transaction.

**Fixes**:

- Fix rate-limiting/filtering of raw envelopes.

**Thank you**:

Features, fixes and improvements in this release have been contributed by:

- [@Turbo87](https://github.com/Turbo87)

## 0.31.0

**Breaking Changes**:

- Aligned profiling-related protocol types.

**Features**:

- Added a `ProfilesSampler` to the `ClientOptions`.

**Fixes**:

- Fix building `ureq` transport without the `native-tls` feature.
- Fixed serialization of raw `Envelope`s, and added a new `from_bytes_raw` constructor.

**Thank you**:

Features, fixes and improvements in this release have been contributed by:

- [@bryanlarsen](https://github.com/bryanlarsen)
- [@jose-acevedoflores](https://github.com/jose-acevedoflores)

## 0.30.0

**Breaking Changes**:

- The minimum supported Rust version was bumped to **1.66.0** due to CI workflow misconfiguration.

**Fixes**:

- Switch to checked version of `from_secs_f64` in `timestamp_to_datetime` function to prevent panics (#554) by @olksdr

**Internal**:

- Disable unnecessary default regex features for `sentry-backtrace` (#552) by @xfix
- Use correct Rust toolchain for MSRV jobs (#555) by @kamilogorek

## 0.29.3

**Features**:

- `debug_images` is now a default feature. ([#545](https://github.com/getsentry/sentry-rust/pull/545)
- Added a `from_path_raw` function to `Envelope` that reads an envelope from a file without parsing anything. ([#549](https://github.com/getsentry/sentry-rust/pull/549))
- Added a `data` method to `performance::Span` that gives access to the span's attached data. ([#548](https://github.com/getsentry/sentry-rust/pull/548))

**Fixes**:

- Envelopes will be discarded rather than blocking if the transport channel fills up. ([#546](https://github.com/getsentry/sentry-rust/pull/546))

## 0.29.2

### Various fixes & improvements

- fix: Prefer `match_pattern` over `match_name` in actix (#539) by @wuerges
- feat(profiling): Add profile context to transaction. (#538) by @viglia
- Re-disable scheduled jobs on forks (#537) by @MarijnS95
- fix: Avoid Deadlock popping ScopeGuards out of order (#536) by @Swatinem
- sentry-core: make TraceContext publicly readable (#534) by @tommilligan
- sentry-core: make TransactionContext.trace_id readable (#533) by @tommilligan
- docs: fix outdated `native-tls`/`rustls` info in README (#535) by @seritools
- features: Make `tower-http` enable the `tower` feature (#532) by @Turbo87

## 0.29.1

**Features**:

- Users of `TransactionContext` may now add `custom` context to it. This may be used by `traces_sampler` to decide sampling rates on a per-transaction basis. ([#512](https://github.com/getsentry/sentry-rust/pull/512))

**Fixes**:

- Correctly strip crates hashes for v0 symbol mangling. ([#525](https://github.com/getsentry/sentry-rust/pull/525))

**Internal**:

- Simplify `Hub::run` and `SentryFuture` by using a scope-guard for `Hub` switching. ([#524](https://github.com/getsentry/sentry-rust/pull/524), [#529](https://github.com/getsentry/sentry-rust/pull/529))

**Thank you**:

Features, fixes and improvements in this release have been contributed by:

- [@tommilligan](https://github.com/tommilligan)

## 0.29.0

**Features**:

- Allow `traces_sampler` to inspect well known properties of `TransactionContext` ([#514](https://github.com/getsentry/sentry-rust/pull/514))

## 0.28.0

**Breaking Changes**:

- The minimum supported Rust version was bumped to **1.60.0** due to requirements from dependencies. ([#498](https://github.com/getsentry/sentry-rust/pull/498))
- Added the `traces_sampler` option to `ClientOptions`. This allows the user to customise sampling rates on a per-transaction basis. ([#510](https://github.com/getsentry/sentry-rust/pull/510))

**Features**:

- Add support for Profiling feature. ([#479](https://github.com/getsentry/sentry-rust/pull/479))
- Add `SSL_VERIFY` option to control certificate verification. ([#508](https://github.com/getsentry/sentry-rust/pull/508))
- Add Windows OS version to OS context ([#499](https://github.com/getsentry/sentry-rust/pull/499))
- Add a `tower-http` feature as a shortcut ([#493](https://github.com/getsentry/sentry-rust/pull/493))

**Internal**:

- Take advantage of weak features in Rust 1.60 for TLS enablement ([#454](https://github.com/getsentry/sentry-rust/pull/454))
- Turn off `pprof` default features ([#491](https://github.com/getsentry/sentry-rust/pull/491))
- Change session update logic to follow the spec ([#477](https://github.com/getsentry/sentry-rust/pull/477))
- Extract public `event_from_error` fn in `sentry-anyhow` ([#476](https://github.com/getsentry/sentry-rust/pull/476))

**Thank you**:

Features, fixes and improvements in this release have been contributed by:

- [@MarijnS95](https://github.com/MarijnS95)
- [@lpraneis](https://github.com/lpraneis)
- [@tommilligan](https://github.com/tommilligan)

## 0.27.0

**Breaking Changes**:

- The minimum supported Rust version was bumped to **1.57.0** due to requirements from dependencies. ([#472](https://github.com/getsentry/sentry-rust/pull/472))
- Add the `rust-version` field to the manifest. ([#473](https://github.com/getsentry/sentry-rust/pull/473))
- Update to edition 2021. ([#473](https://github.com/getsentry/sentry-rust/pull/473))

**Features**:

- Implement `Envelope::from_path` and `Envelope::from_slice`. ([#456](https://github.com/getsentry/sentry-rust/pull/456))
- Add basic `attachment` support. ([#466](https://github.com/getsentry/sentry-rust/pull/466))

**Internal**:

- Replace ancient `lazy_static` crate with `once_cell` or `const` slices. ([#471](https://github.com/getsentry/sentry-rust/pull/471))

**Thank you**:

Features, fixes and improvements in this release have been contributed by:

- [@MarijnS95](https://github.com/MarijnS95)
- [@timfish](https://github.com/timfish)

## 0.26.0

**Breaking Changes**:

- Updated the `debugid` and `uuid` dependencies to versions `0.8.0` and `1.0.0` respectively.

**Features**:

- Request data can now be attached to Transactions and Spans via `set_transaction`. ([#439](https://github.com/getsentry/sentry-rust/pull/439))
- macOS versions are now reported instead of the Darwin kernel version. ([#451](https://github.com/getsentry/sentry-rust/pull/451))
- Support capturing the error of functions instrumented with `#[instrument(err)]`. ([#453](https://github.com/getsentry/sentry-rust/pull/453))
- Support capturing span data of instrumented functions. ([#445](https://github.com/getsentry/sentry-rust/pull/445))
- Expose the `debug_images` function from `sentry-debug-images`.

**Fixes**:

- Generate a more correct request URL in the `sentry-tower` integration. ([#460](https://github.com/getsentry/sentry-rust/pull/460))
- Do not `panic` on invalid `HTTP(S)_PROXY` env. ([#450](https://github.com/getsentry/sentry-rust/pull/450))

**Internal**:

- Project Ids in DSN are treated as opaque strings. ([#452](https://github.com/getsentry/sentry-rust/pull/452))

**Thank you**:

Features, fixes and improvements in this release have been contributed by:

- [@jessfraz](https://github.com/jessfraz)
- [@hannes-vernooij](https://github.com/hannes-vernooij)
- [@rajivshah3](https://github.com/rajivshah3)
- [@MarijnS95](https://github.com/MarijnS95)
- [@kvnvelasco](https://github.com/kvnvelasco)
- [@poliorcetics](https://github.com/poliorcetics)
- [@pbzweihander](https://github.com/pbzweihander)

## 0.25.0

**Breaking Changes**:

- The minimum supported Rust version was bumped to **1.54.0** due to requirements from dependencies.
- Updated the `sentry-actix` integration to `actix-web@4`. ([#437](https://github.com/getsentry/sentry-rust/pull/437))

**Features**:

- Calling `Scope::set_transaction` will override the Transaction name of any currently running performance monitoring transaction. ([#433](https://github.com/getsentry/sentry-rust/pull/433))

**Fixes**:

- Make sure Spans/Transactions have a meaningful/non-empty name. ([#434](https://github.com/getsentry/sentry-rust/pull/434))

**Thank you**:

Features, fixes and improvements in this release have been contributed by:

- [@jessfraz](https://github.com/jessfraz)
- [@fourbytes](https://github.com/fourbytes)

## 0.24.3

**Features**:

- Added `ureq` transport support. ([#419](https://github.com/getsentry/sentry-rust/pull/419))
- Added `GpuContext` to the `Context`. ([#428](https://github.com/getsentry/sentry-rust/pull/428))

**Fixes**:

- Remove unused `serde_json` feature from `curl` dependency. ([#420](http://github.com/getsentry/sentry-rust/pull/420))
- `sentry-tracing`: When converting a `tracing` event to a `sentry` event, don't create an exception if the original event doesn't have one ([#423](https://github.com/getsentry/sentry-rust/pull/423))
- `sentry-tracing`: Add line numbers and tags into custom Contexts sections. ([#430](http://github.com/getsentry/sentry-rust/pull/430))

**Thank you**:

Features, fixes and improvements in this release have been contributed by:

- [@MarijnS95](https://github.com/MarijnS95)

## 0.24.2

**Fixes**:

- Make sure `sentry-core` compiler without the `client` feature. ([#416](https://github.com/getsentry/sentry-rust/pull/416))
- Fix incorrect wrapping of Service Futures in `sentry-tower` that could lead to memory leaks combined with the Http Service. ([#417](https://github.com/getsentry/sentry-rust/pull/417))

## 0.24.1

**Breaking Changes**:

- The minimum supported Rust version was bumped to **1.53.0** due to requirements from dependencies.
- The `backtrace` feature of `sentry-anyhow` is enabled by default. ([#362](https://github.com/getsentry/sentry-rust/pull/362))
- The `tracing-subscriber` dependency of `sentry-tracing` has been bumped to version `0.3.x`. ([#377](https://github.com/getsentry/sentry-rust/pull/377))
- `Scope::add_event_processor` now takes a generic parameter instead of a boxed function.([#380](https://github.com/getsentry/sentry-rust/pull/380))
- The new performance monitoring APIs required changes to a few `protocol` types.
- A few more constructors are now decorated with `#[must_use]`.
- Usage of `chrono` in public API types was removed in favor of `SystemTime`. ([#409](https://github.com/getsentry/sentry-rust/pull/409))

**Features**:

- Added manual APIs for performance monitoring and span/transaction collection. ([#395](https://github.com/getsentry/sentry-rust/pull/395))
- Added span/transaction collection to `sentry-tracing`. ([#350](https://github.com/getsentry/sentry-rust/pull/350), [#400](https://github.com/getsentry/sentry-rust/pull/400))
- Added a new crate `sentry-tower` and feature `tower` that enables integration with `tower`. ([#356](https://github.com/getsentry/sentry-rust/pull/356))
- The new `sentry-tower` crate has a `http` feature which can be used to log request details and start new performance monitoring spans based on incoming distributed tracing headers. ([#397](https://github.com/getsentry/sentry-rust/pull/397))
- Similarly, the `sentry-actix` integration also has the ability to start new performance monitoring spans based on incoming distributed tracing headers. ([#411](https://github.com/getsentry/sentry-rust/pull/411))
- Added a new feature `surf-h1` for using `surf` with the h1 client. ([#357](https://github.com/getsentry/sentry-rust/pull/357))
- Added support for `Span::record` to `sentry-tracing`. ([#364](https://github.com/getsentry/sentry-rust/pull/364))
- Errors captured in the `tracing` integration are being reported as sentry Exceptions. ([#412](https://github.com/getsentry/sentry-rust/pull/412))
- Added Windows support for debug images. ([#366](https://github.com/getsentry/sentry-rust/pull/366))

**Fixes**:

- The `tokio` dependency is now only required for the `curl`, `reqwest`, and `surf` features. ([#363](https://github.com/getsentry/sentry-rust/pull/363))
- The rate limiting implementation was updated to follow the expected behavior. ([#410](https://github.com/getsentry/sentry-rust/pull/410))

**Thank you**:

Features, fixes and improvements in this release have been contributed by:

- [@Tuetuopay](https://github.com/Tuetuopay)
- [@zryambus](https://github.com/zryambus)
- [@Jasper-Bekkers](https://github.com/Jasper-Bekkers)
- [@danielnelson](https://github.com/danielnelson)
- [@leops](https://github.com/leops)
- [@Turbo87](https://github.com/Turbo87)
- [@DmitrySamoylov](https://github.com/DmitrySamoylov)
- [@seanpianka](https://github.com/seanpianka)

## 0.23.0

**Breaking Changes**:

- The minimum supported Rust version was bumped to **1.46.0** due to requirements from dependencies.

**Features**:

- Added support for pre-aggregated Sessions using the new `SessionMode::Request` option. This requires **Sentry 21.2**.
- Added a new `Client::flush` method to explicitly flush the transport and use that to make sure events are flushed out when using `panic=abort`.
- Added a new `flush` hook to the `Transport` trait.
- Exposed a new `RateLimiter` utility that transport implementations can use to drop envelopes early when the DSN is being rate limited.
- Optionally allow capturing backtraces from anyhow errors.
- Added new crate `sentry-tracing` and feature `tracing` that enables support to capture Events and Breadcrumbs from tracing logs.

**Fixes**:

- Honor the `attach_stacktrace` option correctly when capturing errors.
- Added the missing `addr_mode` property to `Frame`.
- Fixed extracting the error type from a `anyhow::msg`.

**Thank you**:

Features, fixes and improvements in this release have been contributed by:

- [@XX](https://github.com/XX)
- [@Jake-Shadle](https://github.com/Jake-Shadle)
- [@Tuetuopay](https://github.com/Tuetuopay)
- [@irevoire](https://github.com/irevoire)
- [@pbzweihander](https://github.com/pbzweihander)

## 0.22.0

**Breaking Changes**:

- The minimum supported Rust version was bumped to **1.45.0**.
- The deprecated `error-chain` and `failure` integrations, features and crates were removed.

**Features**:

- The `slog` integration now supports capturing `slog::KV` pairs for both breadcrumbs and events.
- Preliminary support for attachments was added to `sentry-types` and the `Envelope`. However, deeper integration into the SDK is not yet complete.

**Fixes**:

- Fix regression defaulting `ClientOptions::environment` from `SENTRY_ENVIRONMENT`.
- The `debug-images` integration now captures the correct `image_addr`.
- Do not send invalid exception events in the `log` and `slog` integrations. Both integrations no longer attach the location. To receive location information, set `options.attach_stacktrace` to `true`.
- Process all event backtraces the same way.
- Fix a panic in the session flusher.

**Updates**:

- Updated `reqwest` to version `0.11`, which is based on `tokio 1`.
- Removed usage of the abandoned `im` crate, thus solving a transitive RUSTSEC advisory.

**Thank you**:

Features, fixes and improvements in this release have been contributed by:

- [@jrobsonchase](https://github.com/jrobsonchase)
- [@Jake-Shadle](https://github.com/Jake-Shadle)

## 0.21.0

**Breaking Changes**:

- Bump the minimum required Rust version to **1.42.0**.
- The `actix` integration / middleware is now compatible with `actix-web 3`.
- Removed all deprecated exports and deprecated feature flags.
- The `failure` integration / feature is now off-by-default along with its deprecation.
- The `log` and `slog` integrations were re-designed, they now offer types that wrap a `log::Log` or `slog::Drain` and forward log events to the currently active sentry `Hub` based on an optional filter and an optional mapper.
- The new `log` integration will not implicitly call `log::set_max_level_filter` anymore, and users need to do so manually.

**Features**:

- The SDK will now set a default `environment` based on `debug_assertions`.
- Session updates are now sent lazily.
- Add the new `end_session_with_status` global and Hub functions which allow ending a Release Health Session with an explicit `SessionStatus`.

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

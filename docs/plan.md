# Plan for #882: Support and send Client Reports

## Summary
Implement best-effort client reports in `sentry-rust` so dropped data is visible in Sentry outcomes, aligned with develop docs:
- [Client Reports](https://develop.sentry.dev/sdk/telemetry/client-reports/)
- [Dealing with network failures](https://develop.sentry.dev/sdk/expected-features/#dealing-with-network-failures)
- [Rate limiting](https://develop.sentry.dev/sdk/expected-features/rate-limiting/)
- [Logs (client reports for dropped logs)](https://develop.sentry.dev/sdk/telemetry/logs/#client-reports)

This should be implemented incrementally because it touches `sentry-types`, `sentry-core`, and all HTTP transports.

## Relevant requirements from develop docs

1. SDKs should send `client_report` envelope items with aggregated `discarded_events` (`reason`, `category`, `quantity`).
2. Reports are best-effort; exactness is not required.
3. SDKs should not send one report per drop; aggregate and flush periodically / piggyback on normal envelopes.
4. HTTP handling:
   - `2xx`: success
   - `4xx/5xx` (except `429`): discard and record `send_error`
   - `413`: discard and record `send_error`
   - `429`: discard but **do not** record client report (to avoid double counting)
5. Logs:
   - track dropped log count as `log_item`
   - track dropped log size as `log_byte` (approximate size is acceptable)
6. Logs buffering spec also requires a hard cap of 1000 queued logs.
   - **Scope note for #882:** keep implementation scope narrow to client reports; track the 1000-cap as a separate follow-up issue unless we discover a low-risk one-line fix while implementing.
7. Self-hosted minimum Sentry version for client reports: `21.9.0`.
   - Documentation should explicitly recommend setting `send_client_reports = false` on older self-hosted deployments.

## Current gaps in this repo

- No protocol model or envelope item support for `client_report`.
- No `send_client_reports`/`sendClientReports` option in `ClientOptions`.
- No central drop accounting.
- Drops currently happen but are unreported at many points:
  - event processor / `before_send` / sampling drops
  - transaction unsampled drops
  - transport queue overflow
  - rate-limit filtering
  - HTTP/network failures
  - `before_send_log` drops
- No `log_byte` accounting.
- Logs queue does not currently enforce the 1000-item hard cap from develop logs docs (tracked as a follow-up to keep #882 focused).

## Implementation approach

### 1) Protocol + envelope support (foundation)

Add `client_report` item support in `sentry-types`:
- Introduce protocol structs for client reports and discarded entries.
- Extend envelope parsing/serialization:
  - item header `{"type":"client_report"}`
  - payload JSON per spec.
- Add unit tests for roundtrip serialization and parsing.

### 2) Client report aggregation primitive

Add a thread-safe aggregation component (e.g. `ClientReports`) with:
- `record(reason, category, quantity)`
- `take_report(now)` to drain aggregated counters into one report payload.
- keying by `(reason, category)`.

Design target: usable from both `sentry-core` and transport internals.

### 3) Public option and defaults

Add to `ClientOptions`:
- `send_client_reports: bool` (default `true`).

Behavior:
- when `false`, do no bookkeeping and never attach/send client report items.

### 4) Instrument drop points in `sentry-core`

Record reasons where data is dropped before transport send:
- `event_processor`
- `before_send`
- `sample_rate` (events + transactions)
- `before_send_log`

For transactions: include special-case `span` outcome when a transaction is dropped (per client reports spec).

### 5) Transport-level instrumentation (`sentry` crate)

Record drop reasons in transport paths:
- `queue_overflow`: when transport channel is full (`try_send` failure)
- `ratelimit_backoff`: when envelope/items are dropped by active rate limits
- `network_error`: request failed and envelope is discarded
- `send_error`: non-2xx HTTP responses except 429; include 413
- no client report for `429`

Apply consistently across `reqwest`, `curl`, `ureq`, and `embedded_svc_http`.

### 6) Attach client reports to outgoing envelopes

Before sending an envelope, attach pending `client_report` item when available:
- piggyback on normal envelopes to avoid extra requests
- never create recursion (don’t report drops of client report items themselves)
- avoid generating empty report items.

### 7) Log-specific quantity rules

For dropped logs, emit both:
- `log_item`: dropped log count
- `log_byte`: dropped bytes (estimated, e.g. approximate serialized size)

Implement a reusable log-size estimation helper and use it in all log drop paths.

### 8) Tests

Add coverage for:
- protocol/envelope serialization of `client_report`
- aggregation semantics and drain behavior
- reason/category recording at core drop points
- transport queue overflow and rate limit filtering reports
- HTTP status mapping (`2xx`, `413`, `429`, other `4xx/5xx`)
- logs: `log_item` + `log_byte`
- `send_client_reports = false` disables reports

## Proposed sub-issues

Splitting is recommended. This is a cross-cutting change touching core APIs, transport internals, and protocol.

### Sub-issue A: Add protocol and envelope support for `client_report`
**Description**
- Add client report payload structs and `EnvelopeItem` variant support in `sentry-types`.
- Add serializer/parser tests.

**Dependencies**
- None (foundational).

---

### Sub-issue B: Add shared client report aggregator and `send_client_reports` option
**Description**
- Implement thread-safe aggregation primitive.
- Add `ClientOptions::send_client_reports` defaulting to `true`.
- Wire through client initialization.

**Dependencies**
- Depends on A.

---

### Sub-issue C: Record core-side drops (event/log/transaction pipeline)
**Description**
- Add reporting for `event_processor`, `before_send`, `sample_rate`, `before_send_log`.
- Handle transaction/span special-case accounting.

**Dependencies**
- Depends on B.

---

### Sub-issue D: Record transport-side drops + attach client reports to envelopes
**Description**
- Add drop recording in transport queue overflow, rate-limit filtering, and HTTP send outcomes.
- Add piggyback attachment of pending client reports to outgoing envelopes.
- Ensure behavior parity across reqwest/curl/ureq/embedded transports.

**Dependencies**
- Depends on B.
- Can run in parallel with C after B.

---

### Sub-issue E: Log byte/count outcomes and docs update
**Description**
- Ensure every dropped log path records both `log_item` and `log_byte`.
- Update README/docs for `send_client_reports`, including:
  - minimum self-hosted version (`21.9.0`)
  - explicit recommendation: set `send_client_reports = false` on older self-hosted versions
- Add integration tests covering logs client outcomes.

**Dependencies**
- Depends on C and D.

---

### Sub-issue F (follow-up): Logs hard queue cap parity
**Description**
- Enforce the hard cap of 1000 queued logs (develop logs spec).
- Record client outcomes for logs dropped due to this cap.

**Dependencies**
- Can be implemented after E; intentionally out of scope for #882 unless trivial/low-risk.

---

### Sub-issue G (follow-up): Additional category/precision parity
**Description**
- Extend client-report accounting beyond initial #882 scope (e.g. monitor/check-in and attachment-byte precision) as needed for full parity.

**Dependencies**
- Depends on base client-report infrastructure from A-D.

## Scope decisions (resolved)

1. Keep #882 narrow to client-report infrastructure and primary drop paths.
2. Do **not** expand #882 to broader category/precision parity; handle that in follow-ups.
3. In docs, explicitly recommend `send_client_reports = false` for self-hosted versions older than `21.9.0`.

## Suggested delivery order

A → B → (C and D in parallel) → E

Follow-ups: F, then G.

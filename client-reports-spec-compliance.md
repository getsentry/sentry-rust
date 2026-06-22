# Client Reports Missing Drop Coverage

Date: 2026-06-22

Spec: <https://develop.sentry.dev/sdk/telemetry/client-reports/>

Scope: Sentry Rust SDK client reports across `sentry-types`, `sentry-core`, built-in `sentry` transports, and bundled integrations.

Explicit exclusion: loss tracking in the tokio transport `Thread` is intentionally not listed below. This excludes tokio-thread queue overflow, tokio-thread per-item rate-limit filtering, and tokio-thread global rate-limit drops. Other reqwest HTTP-send behavior is still in scope because it is outside the tokio thread itself.

This document focuses on drop paths that are not currently covered by the implementation. It does not list the lack of a `send_client_reports` option as remaining work, and it does not treat unused discard reasons as a problem by itself.

## Drops not currently covered

### 1. Events dropped by `before_send`

`Client::prepare_event` returns `None` when `before_send` drops an event, but no client-report loss is recorded (`sentry-core/src/client/mod.rs`). This is a concrete SDK-side `error` drop path.

### 2. Events dropped by scope event processors

`Scope::apply_to_event` returns `None` when an event processor drops an event, but no client-report loss is recorded (`sentry-core/src/scope/real.rs`). Because this happens before an envelope exists, the current transport-facing recorder never sees the dropped `error`.

### 3. Events dropped by integrations

`Client::prepare_event` lets integrations drop events by returning `None` from `process_event`, but no client-report loss is recorded (`sentry-core/src/client/mod.rs`). These are SDK-side `error` drops and are separate from scope event processors.

### 4. Error events dropped by `sample_rate`

`Client::prepare_event` silently returns `None` when `sample_should_send(self.options.sample_rate)` rejects an event (`sentry-core/src/client/mod.rs`). This drops an `error` before envelope construction and is not covered by the current transport loss mapping.

### 5. Unsampled transactions dropped at finish time

`Transaction::finish_with_timestamp` returns immediately when `inner.sampled` is false (`sentry-core/src/performance.rs`). No `transaction` loss is recorded.

### 6. Span outcomes for unsampled transactions

The same unsampled transaction path also drops the transaction's span outcome. Per spec, a dropped transaction should also produce a `span` loss with child-span count plus one. The current early return in `Transaction::finish_with_timestamp` records neither the transaction nor span quantities (`sentry-core/src/performance.rs`).

### 7. Finished spans dropped by the transaction span cap

`Span::finish_with_timestamp` only pushes the finished span into the transaction while the transaction span vector remains under the cap; once the cap is exceeded, the span is silently omitted (`sentry-core/src/performance.rs`). This is a direct `span` drop before transport.

### 8. `sentry-tracing` spans dropped by `span_filter`

`SentryLayer::on_new_span` returns immediately when the configured `span_filter` rejects a tracing span (`sentry-tracing/src/layer/mod.rs`). That rejected span is not recorded as a client-report `span` loss.

### 9. `sentry-tracing` event mappings ignored by the layer

`SentryLayer::on_event` drops `EventMapping::Ignore` entries and ignores nested `EventMapping::Combined` values without recording a loss (`sentry-tracing/src/layer/mod.rs`). Depending on what the mapper suppressed, these can correspond to dropped Sentry events, logs, or breadcrumbs.

### 10. Breadcrumbs dropped by `before_breadcrumb`

`Hub::add_breadcrumb` calls `before_breadcrumb` and drops the breadcrumb when the callback returns `None`, without recording any outcome (`sentry-core/src/hub.rs`). If breadcrumbs should be represented in client reports for this SDK, this drop path is currently uncovered.

### 11. Breadcrumbs dropped by `max_breadcrumbs`

`Hub::add_breadcrumb` enforces `max_breadcrumbs` by popping old breadcrumbs from the front of the scope buffer with no client-report recording (`sentry-core/src/hub.rs`). This is an internal buffer-overflow style drop.

### 12. Logs dropped by `before_send_log`

`Client::prepare_log` applies `before_send_log` with `func(log)?`; when the callback returns `None`, the log is dropped without recording (`sentry-core/src/client/mod.rs`). A covered drop would need both `log_item` and `log_byte` quantities.

### 13. Logs dropped when `enable_logs` is false

`Client::capture_log` returns early when `options.enable_logs` is false, without recording a dropped log item or dropped log bytes (`sentry-core/src/client/mod.rs`). If the SDK treats an explicit `capture_log` call in this mode as a discard rather than as non-capture, this path is currently uncovered.

### 14. Metrics dropped by `before_send_metric`

`Client::prepare_metric` applies `before_send_metric` with `func(metric)?`; when the callback returns `None`, the metric is dropped without recording (`sentry-core/src/client/mod.rs`). The transport-side envelope loss mapping already knows how to count `trace_metric` and `trace_metric_byte`, but this SDK-side drop path does not use it.

### 15. Metrics dropped when `enable_metrics` is false

`Client::capture_metric` returns early when `options.enable_metrics` is false, without recording a metric loss (`sentry-core/src/client/mod.rs`). As with disabled logs, this only needs coverage if an explicit capture call in disabled mode is considered a discard.

### 16. Queued envelopes abandoned during std transport shutdown

The std `TransportThread` worker exits as soon as its shutdown flag is set, before processing remaining queued tasks (`sentry/src/transports/thread.rs`). `Drop` sets that flag before sending `Task::Shutdown`. Any queued `SendEnvelope` tasks abandoned by this path are not recorded as lost. This is separate from the excluded tokio transport thread.

### 17. Embedded HTTP transport serialization failures

`EmbeddedSVCHttpTransport::send_envelope` serializes the envelope with `envelope.to_writer(&mut body)?`, and `Transport::send_envelope` only logs the returned error (`sentry/src/transports/embedded_svc_http.rs`). The dropped envelope is not recorded as `internal_sdk_error`.

### 18. Embedded HTTP transport network/request failures

The embedded transport uses fallible HTTP construction, write, flush, submit, and response-read operations, but failures only propagate to a debug log (`sentry/src/transports/embedded_svc_http.rs`). The envelope is discarded without `network_error` or another suitable transport loss being recorded.

### 19. Embedded HTTP transport HTTP 4xx/5xx responses

`EmbeddedSVCHttpTransport` logs HTTP 413 but otherwise returns `Ok(())` regardless of HTTP status (`sentry/src/transports/embedded_svc_http.rs`). HTTP 4xx/5xx responses therefore discard the envelope without `send_error` recording, with the normal exception that HTTP 429 should not be double-counted.

### 20. Built-in transports constructed through deprecated constructors lose reporting

`TransportOptions::try_from_client_options` installs a no-op client-report recorder for backward-compatible construction from `ClientOptions` (`sentry-core/src/transport/options.rs`). Deprecated built-in constructors such as `ReqwestHttpTransport::new(&ClientOptions)` use that path (`sentry/src/transports/reqwest.rs`). The transport may still drop envelopes for queue overflow, rate limiting, network errors, or send errors, but those drops are not reported because the recorder is no-op.

## Cross-cutting problems affecting drop coverage

### 1. SDK-side drops cannot currently record into the per-client aggregator

Most uncovered drops above happen before an envelope item exists. Today the live `Recorder` is created in `EnvelopeSender::new` and passed to the transport factory (`sentry-core/src/client/envelope_sender.rs`, `sentry-core/src/client/mod.rs`). `Client`, `Scope`, tracing integrations, log preparation, and metric preparation do not have a recording handle. Covering SDK-side drops requires exposing an internal per-client recording path, not only adding calls at each drop site.

### 2. Pending client reports can remain unsent if no later envelope is sent

`EnvelopeSender::send_envelope_with` only attaches pending reports to a normal outgoing envelope (`sentry-core/src/client/envelope_sender.rs`). `Client::flush` flushes the transport, and `Client::close` shuts down/removes the transport without draining the aggregator into a final report-only envelope (`sentry-core/src/client/mod.rs`). Even correctly recorded drops can therefore be lost if they occur near shutdown or in an otherwise idle process.

### 3. Serialization failures in some transports are recorded but then panic the worker

The reqwest, ureq, and curl transports record `internal_sdk_error` when `Envelope::to_writer` fails, but then call `expect("envelope should serialize successfully")` (`sentry/src/transports/reqwest.rs`, `sentry/src/transports/ureq.rs`, `sentry/src/transports/curl.rs`). That means the original drop is nominally covered, but the panic can prevent the client report from being delivered and can break later transport processing.

## Already covered drop paths

The current implementation already covers several transport-side drops outside the excluded tokio thread:

- Envelope item category/quantity mapping, including transaction-to-span and log/metric byte outcomes (`sentry-types/src/protocol/client_report/envelope_losses.rs`).
- Std transport queue overflow and disconnected-channel drops (`sentry/src/transports/thread.rs`).
- Std transport global rate-limit drops and per-item rate-limit filtering (`sentry/src/transports/thread.rs`, `sentry/src/transports/ratelimit.rs`).
- Reqwest, ureq, and curl HTTP 4xx/5xx send errors excluding HTTP 429 (`sentry/src/transports/reqwest.rs`, `sentry/src/transports/ureq.rs`, `sentry/src/transports/curl.rs`).
- Reqwest, ureq, and curl network errors (`sentry/src/transports/reqwest.rs`, `sentry/src/transports/ureq.rs`, `sentry/src/transports/curl.rs`).
- Reqwest, ureq, and curl envelope serialization failures are at least recorded as `internal_sdk_error`, subject to the panic problem above.


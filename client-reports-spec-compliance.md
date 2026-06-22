# Client Reports Spec Compliance Findings

Date: 2026-06-22

Spec: <https://develop.sentry.dev/sdk/telemetry/client-reports/>

Scope: Sentry Rust SDK client reports across `sentry-types`, `sentry-core`, and the built-in `sentry` transports.

Explicit exclusion: loss tracking in the tokio transport `Thread` is intentionally not listed below. This excludes tokio-thread queue overflow, tokio-thread per-item rate-limit filtering, and tokio-thread global rate-limit drops. Other reqwest HTTP-send behavior is still in scope because it is outside the tokio thread itself.

## Remaining items

### 1. Add a `send_client_reports` configuration option

The spec says SDKs should provide a `send_client_reports` / `sendClientReports` option and should use a no-op implementation when client reports are disabled. `ClientOptions` currently has no such field (`sentry-core/src/clientoptions.rs`), and `EnvelopeSender::new` always creates a `ClientReportAggregator` and live recorder whenever a transport is built (`sentry-core/src/client/envelope_sender.rs`).

### 2. Wire client-report recording into the SDK pipeline, not only transports

The current recorder is explicitly transport-facing: `Recorder` is documented as a handle for transports and exposes only `record_lost_envelope_item` (`sentry-core/src/client/client_reports/recorder.rs`). `Client` owns only an `EnvelopeSender` and has no direct recorder or internal API for recording losses before an envelope exists (`sentry-core/src/client/mod.rs`). Full compliance requires SDK components that drop telemetry before transport to be able to record into the per-client aggregator.

### 3. Model all spec discard reasons that the SDK needs to emit

`sentry-types::protocol::client_report::Reason` currently includes only `send_error`, `internal_sdk_error`, `network_error`, `ratelimit_backoff`, and `queue_overflow` (`sentry-types/src/protocol/client_report/mod.rs`). The spec also defines `cache_overflow`, `sample_rate`, `before_send`, `event_processor`, `insufficient_data`, `backpressure`, `buffer_overflow`, `ignored`, `invalid`, and `no_parent_span`. Those missing reasons prevent SDK-side drops from being reported with spec-defined reason strings.

### 4. Record events dropped by `before_send`

`Client::prepare_event` returns `None` when `before_send` drops an event, but it does not record a client report with reason `before_send` and category `error` (`sentry-core/src/client/mod.rs`). This is a direct spec recording point.

### 5. Record events dropped by event processors and integrations

Scope event processors can drop events in `Scope::apply_to_event` without recording (`sentry-core/src/scope/real.rs`). Integrations can also drop events in `Client::prepare_event` without recording (`sentry-core/src/client/mod.rs`). These should be reported with the spec's `event_processor` reason, or another spec-defined reason if a specific processor/integration has a better mapping.

### 6. Record error events dropped by `sample_rate`

`Client::prepare_event` silently returns `None` when `sample_should_send(self.options.sample_rate)` rejects an event (`sentry-core/src/client/mod.rs`). This should record reason `sample_rate` for category `error`.

### 7. Record unsampled transactions and their span outcomes

`Transaction::finish_with_timestamp` returns immediately when `inner.sampled` is false (`sentry-core/src/performance.rs`). The spec requires a dropped transaction to record both a `transaction` outcome and an additional `span` outcome with child span count plus one. Unsampled transactions should therefore record reason `sample_rate` for both categories.

### 8. Record spans dropped from the transaction span cap

`Span::finish_with_timestamp` only pushes a finished span into the transaction while `transaction.spans.len() <= MAX_SPANS`; once the cap is exceeded, the span is silently omitted (`sentry-core/src/performance.rs`). The spec requires SDKs to report spans that are dropped before a transaction is sent.

### 9. Record breadcrumbs dropped by `before_breadcrumb`

`Hub::add_breadcrumb` calls `before_breadcrumb` and drops the breadcrumb when the callback returns `None`, without recording any outcome (`sentry-core/src/hub.rs`). To fully cover SDK-side discard points, this needs a spec-defined reason/category mapping; implementing it may require adding currently missing reason support such as `ignored` or `before_send`.

### 10. Record breadcrumb buffer overflow

`Hub::add_breadcrumb` enforces `max_breadcrumbs` by popping old breadcrumbs from the front of the scope buffer with no client-report recording (`sentry-core/src/hub.rs`). The spec defines `buffer_overflow` for SDK internal buffers, explicitly including breadcrumb buffers as an example.

### 11. Record logs dropped because log capture is disabled

`Client::capture_log` returns early when `options.enable_logs` is false, without recording a dropped log item or dropped log bytes (`sentry-core/src/client/mod.rs`). If this is considered a drop rather than a non-capturing mode, compliance requires `log_item` and `log_byte` outcomes.

### 12. Record logs dropped by `before_send_log`

`Client::prepare_log` applies `before_send_log` with `func(log)?`, so a returned `None` drops the log without recording (`sentry-core/src/client/mod.rs`). The spec requires dropped logs to produce both `log_item` and `log_byte` outcomes.

### 13. Record metrics dropped because metric capture is disabled

`Client::capture_metric` returns early when `options.enable_metrics` is false, without recording outcomes (`sentry-core/src/client/mod.rs`). The existing envelope-loss mapping already has `trace_metric` and `trace_metric_byte` categories for transport-side metric drops; SDK-side metric drops need equivalent reporting if disabled capture is treated as a drop.

### 14. Record metrics dropped by `before_send_metric`

`Client::prepare_metric` applies `before_send_metric` with `func(metric)?`, so a returned `None` drops the metric without recording (`sentry-core/src/client/mod.rs`). SDK-side metric filtering should record `trace_metric` and `trace_metric_byte` outcomes, using the appropriate spec discard reason.

### 15. Flush or send pending client reports when there is no later envelope

`EnvelopeSender::send_envelope_with` only attaches a pending client report when another envelope is about to be sent (`sentry-core/src/client/envelope_sender.rs`). `Client::flush` only flushes the transport, and `Client::close` removes/shuts down the transport without draining the `ClientReportAggregator` into a final report-only envelope (`sentry-core/src/client/mod.rs`, `sentry-core/src/client/envelope_sender.rs`). Losses recorded near shutdown, or losses in an otherwise idle application, can remain unsent indefinitely.

### 16. Record queued-envelope losses during std transport shutdown

The std `TransportThread` worker exits as soon as its shutdown flag is set, before processing any remaining queued tasks (`sentry/src/transports/thread.rs`). `Drop` sets that flag before sending `Task::Shutdown`. Any queued `SendEnvelope` tasks abandoned by this path are not recorded as lost. This is separate from the excluded tokio transport thread.

### 17. Add client-report recording to `EmbeddedSVCHttpTransport`

`EmbeddedSVCHttpTransport` stores `TransportOptions`, but `send_envelope` destructures only the DSN and user agent and never uses `client_report_recorder` (`sentry/src/transports/embedded_svc_http.rs`). Serialization, network, and HTTP error losses are only returned/logged and are not recorded. It also logs HTTP 413 but otherwise treats HTTP statuses as success, so 4xx/5xx send-error recording is missing for this transport.

### 18. Avoid no-op recorders in deprecated built-in transport constructors

`TransportOptions::try_from_client_options` creates `client_report_recorder: ClientReportRecorder::new_no_op()` for backward-compatible construction from `ClientOptions` (`sentry-core/src/transport/options.rs`). Deprecated constructors such as `ReqwestHttpTransport::new(&ClientOptions)` use that path (`sentry/src/transports/reqwest.rs`). Built-in transports created this way cannot emit client reports even for losses they otherwise know how to record.

### 19. Discard serialization/processing failures without panicking the worker

The reqwest, ureq, and curl transports record `internal_sdk_error` if `Envelope::to_writer` fails, but then call `expect("envelope should serialize successfully")` (`sentry/src/transports/reqwest.rs`, `sentry/src/transports/ureq.rs`, `sentry/src/transports/curl.rs`). The spec says SDK processing failures should discard the envelope and record `internal_sdk_error`; panicking the worker can prevent the recorded report from ever being sent and can break future transport processing.

### 20. Enforce or verify the 4 KiB client-report item size limit

The spec states that a `client_report` envelope item has a 4 KiB size limit. `Report::new` accepts all provided items, `ClientReportAggregatorInner::take_pending_report` drains every nonzero reason/category counter, and `EnvelopeSender` attaches the result without checking serialized size (`sentry-types/src/protocol/client_report/mod.rs`, `sentry-core/src/client/client_reports/inner.rs`, `sentry-core/src/client/envelope_sender.rs`). There is no guard or test proving that generated reports remain under the limit.

### 21. Add client-report aggregation, wire-format, and end-to-end tests

Current tests cover envelope-item loss mapping in `sentry-types/src/protocol/envelope.rs`, but there are no tests covering `ClientReportAggregator`, `take_pending_report`, attachment to outgoing envelopes, the `discarded_events` JSON wire format, shutdown behavior, or SDK-side discard recording. Full compliance should be protected by tests across `sentry-types`, `sentry-core`, and built-in transports.


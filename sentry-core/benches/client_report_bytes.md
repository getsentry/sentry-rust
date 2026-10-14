# Client-report byte computation benchmark

This disposable experiment measures the drop-path cost of calculating the byte
outcomes required for [logs and metrics](https://develop.sentry.dev/sdk/telemetry/client-reports/#log-byte-outcomes).
It benchmarks logs dropped by `before_send_log`; metrics are verified, not timed.
**Do not ship the benchmark switch.**

## Reproduction

Run from the repository root:

```sh
env -u SENTRY_BENCH_DISABLE_CLIENT_REPORT_BYTES cargo bench --locked \
  -p sentry-core --features client,logs --bench client_report_bytes \
  -- --save-baseline runtime-enabled

SENTRY_BENCH_DISABLE_CLIENT_REPORT_BYTES=1 cargo bench --locked \
  -p sentry-core --features client,logs --bench client_report_bytes \
  -- --save-baseline runtime-disabled
```

For the run-order check, repeat disabled first, then enabled, saving baselines as
`runtime-disabled-repeat` and `runtime-enabled-repeat` respectively.

Any present value of `SENTRY_BENCH_DISABLE_CLIENT_REPORT_BYTES` disables sizing,
including `0`. The setting is cached on the first sizing call; changing it
requires a new process, not a rebuild. Both modes execute the same binary.

The switch in [the size helpers](../../sentry-types/src/protocol/client_report/relay_size.rs)
returns a constant size of `1` for every log and metric, including items in dropped
batches. It skips attribute traversal and all actual size calculation, but
preserves byte-report entries, loss-vector allocation, and atomic counter updates.
The resulting byte quantities are intentionally invalid. This is not a comparison
against disabling client reports entirely.

## Method

[The benchmark](client_report_bytes.rs) captures 1,000 logs per iteration using an
active client, an empty scope, no integrations, and a callback returning `None`.
The client adds its usual two SDK attributes; attribute counts below refer only
to application attributes. The transport is local and sends no network requests.

- Input allocation/cloning and client/scope initialization are outside timing.
- Capture, scope/client enrichment, sizing, callback, report aggregation, and log
  destruction are timed. No report serialization or transport delivery is timed.
- One input batch is live at a time, using Criterion `BatchSize::PerIteration`.
- Scalar attributes alternate between 48-byte strings, integers, booleans, and
  floating-point numbers. The nested case adds one array containing 16 objects,
  each with an ID, duration, status, and a four-element labels array.
- Untimed checks verify individual/batched log and metric sizes, exactly 1,000
  `before_send` item losses, and the expected byte quantity after enrichment.
  The timed callback has no verification counters. No log envelope may reach
  the transport.
- Each case uses 100 Criterion samples, a one-second warmup, and a five-second
  target measurement period. Runs were sequential: enabled, disabled, disabled
  repeat, enabled repeat.

## Results

Measured on 2026-10-13 on an Apple M1 Max, macOS 27.0.1, using Rust 1.98.1,
`aarch64-apple-darwin`, and the default optimized Cargo bench profile.

Times are nanoseconds per dropped log, averaging the two per-mode Criterion
point estimates and dividing batch times by 1,000. Criterion uses slope estimates
for linear sampling and mean estimates for flat sampling. Time saved is
`(enabled - disabled) / enabled`; no combined confidence interval is claimed.

| Workload | Enabled ns/log | Disabled ns/log | Saved ns/log | Time saved |
| --- | ---: | ---: | ---: | ---: |
| 80-byte body, 0 application attributes | 179.24 | 171.88 | 7.35 | 4.10% |
| 256-byte body, 8 scalar attributes | 338.27 | 315.61 | 22.66 | 6.70% |
| 1-KiB body, 64 scalar attributes | 1,416.32 | 1,219.46 | 196.87 | 13.90% |
| 256-byte body, 8 scalar attributes plus nested array | 2,617.01 | 2,058.73 | 558.28 | 21.33% |

The measured effect increases with attribute count and nested value complexity.
The existing implementation does not serialize logs to determine size: string
lengths are constant-time reads, while attribute maps and nested values are walked.
The eight-scalar-attribute workload saves about 23 ns per dropped log; the more
complex synthetic workloads save about 0.2–0.6 microseconds.

Both modes include the cached runtime-toggle check, so these absolute timings
are not timings of the unmodified SDK. The differences include effects on cache
behavior during destruction, not just arithmetic in the sizing helper. These
single-threaded synthetic results do not establish production averages, metric
capture performance, or the cost of dropping data in transport queues.

An initial compile-time-switch comparison reproducibly made the eight-attribute
case slower when sizing was disabled. Those different-build results are not used
for conclusions: the runtime switch removed the anomaly and produced reductions
in every workload in both run orders.

The [local raw artifacts](../../target/client-report-bytes/) contain the
`runtime-*.log` outputs, environment metadata, and `runtime-summary.json`.
Criterion samples and individual 95% confidence intervals are under
`criterion/before_send_log_drop/<case>/runtime-*/`. These artifacts are gitignored;
the summary table above persists independently of them.

Verification also passed 103 `sentry-types` unit tests, 55 protocol integration
tests, and eight `sentry-core` client-report integration tests with sizing enabled.

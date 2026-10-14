#![expect(missing_docs, reason = "disposable benchmark")]

use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use criterion::{criterion_group, criterion_main, BatchSize, Criterion, Throughput};
use sentry_core::protocol::client_report::{Category, LossSource};
use sentry_core::protocol::{
    EnvelopeItem, ItemContainer, Log, LogAttribute, LogLevel, Metric, MetricType,
};
use sentry_core::{Client, ClientOptions, Envelope, Scope, Transport};
use serde_json::{json, Value};

const LOGS_PER_ITERATION: usize = 1_000;
fn bytes_disabled() -> bool {
    std::env::var_os("SENTRY_BENCH_DISABLE_CLIENT_REPORT_BYTES").is_some()
}

#[derive(Default)]
struct CheckingTransport(Mutex<Vec<Envelope>>);

impl Transport for CheckingTransport {
    fn send_envelope(&self, envelope: Envelope) {
        assert!(envelope
            .items()
            .all(|item| matches!(item, EnvelopeItem::ClientReport(_))));
        self.0.lock().unwrap().push(envelope);
    }
}

fn options(transport: Arc<CheckingTransport>) -> ClientOptions {
    ClientOptions::new()
        .dsn("https://public@sentry.invalid/1")
        .transport(transport)
        .before_send_log(|log| {
            black_box(log);
            None
        })
}

fn make_log(body_bytes: usize, attribute_count: usize) -> Log {
    Log {
        level: LogLevel::Info,
        body: "x".repeat(body_bytes),
        trace_id: None,
        timestamp: SystemTime::UNIX_EPOCH,
        severity_number: None,
        attributes: (0..attribute_count)
            .map(|i| {
                let value = match i % 4 {
                    0 => Value::String("v".repeat(48)),
                    1 => json!(42),
                    2 => json!(true),
                    _ => json!(12.5),
                };
                (format!("attribute.{i:02}"), LogAttribute(value))
            })
            .collect(),
    }
}

fn byte_quantity<L: LossSource + ?Sized>(source: &L, category: Category) -> u64 {
    source
        .losses()
        .find(|loss| loss.category == category)
        .unwrap()
        .quantity
}

// Verify the runtime switch in individual and batched logs/metrics before timing.
fn verify_size_switch(log: &Log) {
    let log_bytes = byte_quantity(log, Category::LogByte);
    assert_eq!(log_bytes == 1, bytes_disabled());
    assert_eq!(
        byte_quantity(
            &ItemContainer::Logs(vec![log.clone(); 2]),
            Category::LogByte
        ),
        log_bytes * 2,
    );

    let metric = Metric {
        r#type: MetricType::Counter,
        name: "benchmark.counter".into(),
        value: 1.0,
        timestamp: SystemTime::UNIX_EPOCH,
        trace_id: Default::default(),
        span_id: None,
        unit: None,
        attributes: log
            .attributes
            .iter()
            .map(|(key, value)| (key.clone().into(), value.clone()))
            .collect(),
    };
    let metric_bytes = byte_quantity(&metric, Category::TraceMetricByte);
    assert_eq!(metric_bytes == 1, bytes_disabled());
    assert_eq!(
        byte_quantity(
            &ItemContainer::Metrics(vec![metric; 2]),
            Category::TraceMetricByte,
        ),
        metric_bytes * 2,
    );
}

// A separate client verifies the real drop/report path; this callback is never timed.
fn verify_drops(log: &Log, scope: &Scope) {
    let transport = Arc::new(CheckingTransport::default());
    let prepared_bytes = Arc::new(AtomicU64::new(0));
    let observed_bytes = prepared_bytes.clone();
    let client = Client::from(options(transport.clone()).before_send_log(move |log| {
        observed_bytes.store(byte_quantity(&log, Category::LogByte), Ordering::Relaxed);
        None
    }));
    assert!(client.is_enabled());
    for _ in 0..LOGS_PER_ITERATION {
        client.capture_log(log.clone(), scope);
    }
    client.send_envelope(Envelope::new());
    let envelopes = transport.0.lock().unwrap();
    assert_eq!(envelopes.len(), 1);
    let report = envelopes[0]
        .items()
        .find_map(|item| match item {
            EnvelopeItem::ClientReport(report) => Some(report),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        serde_json::to_value(report).unwrap()["discarded_events"],
        json!([
            {"category": "log_item", "reason": "before_send", "quantity": LOGS_PER_ITERATION},
            {"category": "log_byte", "reason": "before_send",
             "quantity": prepared_bytes.load(Ordering::Relaxed) * LOGS_PER_ITERATION as u64},
        ]),
    );
    assert!(client.close(None));
}

fn client_report_bytes(c: &mut Criterion) {
    eprintln!(
        "Client report size computation: {} (runtime setting, same executable)",
        if bytes_disabled() {
            "DISABLED"
        } else {
            "ENABLED"
        },
    );

    let mut nested = make_log(256, 8);
    nested.attributes.insert(
        "requests".into(),
        LogAttribute(Value::Array(
            (0..16)
                .map(|i| {
                    json!({
                        "id": format!("request-{i:04}"),
                        "duration_ms": 12.5,
                        "status": 200,
                        "labels": ["cache", "eu-central-1", true, 42],
                    })
                })
                .collect(),
        )),
    );
    let cases = [
        ("small_80B_0attrs", make_log(80, 0)),
        ("typical_256B_8attrs", make_log(256, 8)),
        ("wide_1KiB_64attrs", make_log(1_024, 64)),
        ("nested_256B_8attrs_16objects", nested),
    ];
    let scope = Scope::default();
    let mut group = c.benchmark_group("before_send_log_drop");
    group.throughput(Throughput::Elements(LOGS_PER_ITERATION as u64));
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    for (name, template) in cases {
        verify_size_switch(&template);
        verify_drops(&template, &scope);
        let transport = Arc::new(CheckingTransport::default());
        let client = Client::from(options(transport.clone()));
        assert!(client.is_enabled());
        group.bench_function(name, |b| {
            b.iter_batched(
                // Clone/allocate inputs outside timing. Capture, enrichment, loss calculation,
                // reporting and destruction are timed. Keep only one batch live at a time.
                || vec![template.clone(); LOGS_PER_ITERATION],
                |logs| {
                    for log in logs {
                        client.capture_log(black_box(log), black_box(&scope));
                    }
                },
                BatchSize::PerIteration,
            );
        });
        assert!(client.close(None));
        assert!(transport.0.lock().unwrap().is_empty());
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().output_directory(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"), "/../target/client-report-bytes/criterion"
    )));
    targets = client_report_bytes
}
criterion_main!(benches);

use std::sync::Arc;

use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use tracing_subscriber::prelude::*;

struct NoopTransport;

impl sentry::Transport for NoopTransport {
    fn send_envelope(&self, envelope: sentry::Envelope) {}
}

fn active_hub() -> Arc<sentry::Hub> {
    let client = Arc::new(sentry::Client::from(sentry::ClientOptions {
        dsn: Some("https://public@sentry.invalid/1".parse().unwrap()),
        transport: Some(Arc::new(Arc::new(NoopTransport))),
        traces_sample_rate: 1.0,
        ..Default::default()
    }));
    let scope = Arc::new(sentry::Scope::default());
    Arc::new(sentry::Hub::new(Some(client), scope))
}

fn bench_sentry_active<F>(b: &mut criterion::Bencher<'_>, mut op: F)
where
    F: FnMut(),
{
    let dispatch =
        tracing::Dispatch::new(tracing_subscriber::registry().with(sentry_tracing::layer()));
    let hub = active_hub();

    sentry::Hub::run(hub, || {
        tracing::dispatcher::with_default(&dispatch, || {
            b.iter(|| op());
        });
    });
}

fn bench_tracing_only_control<F>(b: &mut criterion::Bencher<'_>, mut op: F)
where
    F: FnMut(),
{
    let dispatch = tracing::Dispatch::new(tracing_subscriber::registry());

    tracing::dispatcher::with_default(&dispatch, || {
        b.iter(|| op());
    });
}

fn tracing_layer_perf(c: &mut Criterion) {
    {
        let mut group = c.benchmark_group("enter_exit_existing_span");
        group.bench_function("sentry_active", |b| {
            let dispatch = tracing::Dispatch::new(
                tracing_subscriber::registry().with(sentry_tracing::layer()),
            );
            let hub = active_hub();

            sentry::Hub::run(hub, || {
                tracing::dispatcher::with_default(&dispatch, || {
                    let span = tracing::info_span!("existing");
                    b.iter(|| {
                        let _guard = span.enter();
                        black_box(());
                    });
                });
            });
        });
        group.bench_function("tracing_only_control", |b| {
            let dispatch = tracing::Dispatch::new(tracing_subscriber::registry());
            tracing::dispatcher::with_default(&dispatch, || {
                let span = tracing::info_span!("existing");
                b.iter(|| {
                    let _guard = span.enter();
                    black_box(());
                });
            });
        });
        group.finish();
    }

    {
        let mut group = c.benchmark_group("create_enter_exit_close_span");
        group.bench_function("sentry_active", |b| {
            bench_sentry_active(b, || {
                let span = tracing::info_span!("created");
                let _guard = span.enter();
                black_box(());
            })
        });
        group.bench_function("tracing_only_control", |b| {
            bench_tracing_only_control(b, || {
                let span = tracing::info_span!("created");
                let _guard = span.enter();
                black_box(());
            })
        });
        group.finish();
    }

    {
        let mut group = c.benchmark_group("reenter_same_span_depth2");
        group.bench_function("sentry_active", |b| {
            bench_sentry_active(b, || {
                let span = tracing::info_span!("reenter");
                let _guard1 = span.enter();
                let _guard2 = span.enter();
                black_box(());
            })
        });
        group.bench_function("tracing_only_control", |b| {
            bench_tracing_only_control(b, || {
                let span = tracing::info_span!("reenter");
                let _guard1 = span.enter();
                let _guard2 = span.enter();
                black_box(());
            })
        });
        group.finish();
    }

    {
        let mut group = c.benchmark_group("cross_thread_shared_span");
        group.bench_function("sentry_active", |b| {
            bench_sentry_active(b, || {
                let span = tracing::info_span!("shared");
                let span_a = span.clone();
                let span_b = span;

                std::thread::scope(|scope| {
                    scope.spawn(move || {
                        let _guard = span_a.enter();
                        let _child = tracing::info_span!("child_a").entered();
                        black_box(());
                    });
                    scope.spawn(move || {
                        let _guard = span_b.enter();
                        let _child = tracing::info_span!("child_b").entered();
                        black_box(());
                    });
                });
            })
        });
        group.bench_function("tracing_only_control", |b| {
            bench_tracing_only_control(b, || {
                let span = tracing::info_span!("shared");
                let span_a = span.clone();
                let span_b = span;

                std::thread::scope(|scope| {
                    scope.spawn(move || {
                        let _guard = span_a.enter();
                        let _child = tracing::info_span!("child_a").entered();
                        black_box(());
                    });
                    scope.spawn(move || {
                        let _guard = span_b.enter();
                        let _child = tracing::info_span!("child_b").entered();
                        black_box(());
                    });
                });
            })
        });
        group.finish();
    }
}

criterion_group!(benches, tracing_layer_perf);
criterion_main!(benches);

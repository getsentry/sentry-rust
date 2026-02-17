//! Batching for Sentry [trace metrics](https://develop.sentry.dev/sdk/telemetry/metrics/).

use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::client::TransportArc;
use crate::protocol::EnvelopeItem;
use crate::Envelope;
use sentry_types::protocol::v7::TraceMetric;

// Flush when there's 100 metrics in the buffer
const MAX_METRIC_ITEMS: usize = 100;
// Or when 5 seconds have passed from the last flush
const FLUSH_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug, Default)]
struct MetricQueue {
    metrics: Vec<TraceMetric>,
}

/// Accumulates trace metrics in the queue and submits them through the transport when one of the
/// flushing conditions is met.
pub(crate) struct MetricsBatcher {
    transport: TransportArc,
    queue: Arc<Mutex<MetricQueue>>,
    shutdown: Arc<(Mutex<bool>, Condvar)>,
    worker: Option<JoinHandle<()>>,
}

impl MetricsBatcher {
    /// Creates a new MetricsBatcher that will submit envelopes to the given `transport`.
    pub(crate) fn new(transport: TransportArc) -> Self {
        let queue = Arc::new(Mutex::new(Default::default()));
        #[allow(clippy::mutex_atomic)]
        let shutdown = Arc::new((Mutex::new(false), Condvar::new()));

        let worker_transport = transport.clone();
        let worker_queue = queue.clone();
        let worker_shutdown = shutdown.clone();
        let worker = std::thread::Builder::new()
            .name("sentry-metrics-batcher".into())
            .spawn(move || {
                let (lock, cvar) = worker_shutdown.as_ref();
                let mut shutdown = lock.lock().unwrap();
                // check this immediately, in case the main thread is already shutting down
                if *shutdown {
                    return;
                }
                let mut last_flush = Instant::now();
                loop {
                    let timeout = FLUSH_INTERVAL
                        .checked_sub(last_flush.elapsed())
                        .unwrap_or_else(|| Duration::from_secs(0));
                    shutdown = cvar.wait_timeout(shutdown, timeout).unwrap().0;
                    if *shutdown {
                        return;
                    }
                    if last_flush.elapsed() >= FLUSH_INTERVAL {
                        MetricsBatcher::flush_queue_internal(
                            worker_queue.lock().unwrap(),
                            &worker_transport,
                        );
                        last_flush = Instant::now();
                    }
                }
            })
            .unwrap();

        Self {
            transport,
            queue,
            shutdown,
            worker: Some(worker),
        }
    }

    /// Enqueues a metric for delayed sending.
    ///
    /// This will automatically flush the queue if it reaches a size of `MAX_METRIC_ITEMS`.
    pub(crate) fn enqueue(&self, metric: TraceMetric) {
        let mut queue = self.queue.lock().unwrap();
        queue.metrics.push(metric);
        if queue.metrics.len() >= MAX_METRIC_ITEMS {
            MetricsBatcher::flush_queue_internal(queue, &self.transport);
        }
    }

    /// Flushes the queue to the transport.
    pub(crate) fn flush(&self) {
        let queue = self.queue.lock().unwrap();
        MetricsBatcher::flush_queue_internal(queue, &self.transport);
    }

    /// Flushes the queue to the transport.
    ///
    /// This is a static method as it will be called from both the background
    /// thread and the main thread on drop.
    fn flush_queue_internal(mut queue_lock: MutexGuard<MetricQueue>, transport: &TransportArc) {
        let metrics = std::mem::take(&mut queue_lock.metrics);
        drop(queue_lock);

        if metrics.is_empty() {
            return;
        }

        sentry_debug!("[MetricsBatcher] Flushing {} metrics", metrics.len());

        if let Some(ref transport) = *transport.read().unwrap() {
            let mut envelope = Envelope::new();
            let metrics_item: EnvelopeItem = metrics.into();
            envelope.add_item(metrics_item);
            transport.send_envelope(envelope);
        }
    }
}

impl Drop for MetricsBatcher {
    fn drop(&mut self) {
        let (lock, cvar) = self.shutdown.as_ref();
        *lock.lock().unwrap() = true;
        cvar.notify_one();

        if let Some(worker) = self.worker.take() {
            worker.join().ok();
        }
        MetricsBatcher::flush_queue_internal(self.queue.lock().unwrap(), &self.transport);
    }
}

#[cfg(all(test, feature = "test"))]
mod tests {
    use crate::test;
    use sentry_types::protocol::v7::{TraceId, TraceMetric, TraceMetricType};
    use std::time::SystemTime;

    fn test_metric(name: &str) -> TraceMetric {
        TraceMetric {
            r#type: TraceMetricType::Counter,
            name: name.to_owned(),
            value: 1.0,
            timestamp: SystemTime::now(),
            trace_id: TraceId::default(),
            span_id: None,
            unit: None,
            attributes: Default::default(),
        }
    }

    // Test that metrics are sent in batches
    #[test]
    fn test_metrics_batching() {
        let envelopes = test::with_captured_envelopes_options(
            || {
                for i in 0..150 {
                    crate::Hub::current().capture_metric(test_metric(&format!("metric.{i}")));
                }
            },
            crate::ClientOptions {
                enable_metrics: true,
                ..Default::default()
            },
        );

        assert_eq!(2, envelopes.len());

        let mut total_metrics = 0;
        for envelope in &envelopes {
            for item in envelope.items() {
                if let crate::protocol::EnvelopeItem::ItemContainer(
                    crate::protocol::ItemContainer::TraceMetrics(metrics),
                ) = item
                {
                    total_metrics += metrics.len();
                }
            }
        }

        assert_eq!(150, total_metrics);
    }

    // Test that the batcher is flushed on client close
    #[test]
    fn test_metrics_batcher_flush() {
        let envelopes = test::with_captured_envelopes_options(
            || {
                for i in 0..12 {
                    crate::Hub::current().capture_metric(test_metric(&format!("metric.{i}")));
                }
            },
            crate::ClientOptions {
                enable_metrics: true,
                ..Default::default()
            },
        );

        assert_eq!(1, envelopes.len());

        for envelope in &envelopes {
            for item in envelope.items() {
                if let crate::protocol::EnvelopeItem::ItemContainer(
                    crate::protocol::ItemContainer::TraceMetrics(metrics),
                ) = item
                {
                    assert_eq!(12, metrics.len());
                    break;
                }
            }
        }
    }
}

//! Generic batching for Sentry envelope items (logs, metrics, etc.).

use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::client::TransportArc;
use crate::protocol::EnvelopeItem;
use crate::Envelope;

// Flush when there's 100 items in the buffer
const MAX_ITEMS: usize = 100;
// Or when 5 seconds have passed from the last flush
const FLUSH_INTERVAL: Duration = Duration::from_secs(5);

/// Accumulates items in a queue and submits them through the transport when one of the flushing
/// conditions is met: either the queue reaches [`MAX_ITEMS`] or [`FLUSH_INTERVAL`] has elapsed.
pub(crate) struct Batcher<T>
where
    EnvelopeItem: From<Vec<T>>,
    T: Send + 'static,
{
    transport: TransportArc,
    queue: Arc<Mutex<Vec<T>>>,
    shutdown: Arc<(Mutex<bool>, Condvar)>,
    worker: Option<JoinHandle<()>>,
    name: &'static str,
}

impl<T> Batcher<T>
where
    EnvelopeItem: From<Vec<T>>,
    T: Send + 'static,
{
    /// Creates a new Batcher that will submit envelopes to the given `transport`.
    ///
    /// `name` is used for the background thread name and debug logging.
    /// `into_envelope_item` converts a batch of items into an [`EnvelopeItem`].
    pub(crate) fn new(transport: TransportArc, name: &'static str) -> Self {
        let queue: Arc<Mutex<Vec<T>>> = Arc::new(Mutex::new(Vec::new()));
        #[allow(clippy::mutex_atomic)]
        let shutdown = Arc::new((Mutex::new(false), Condvar::new()));

        let worker_transport = transport.clone();
        let worker_queue = queue.clone();
        let worker_shutdown = shutdown.clone();
        let worker = std::thread::Builder::new()
            .name(format!("sentry-{name}-batcher"))
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
                        Self::flush_queue_internal(
                            worker_queue.lock().unwrap(),
                            &worker_transport,
                            name,
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
            name,
        }
    }

    /// Enqueues an item for delayed sending.
    ///
    /// This will automatically flush the queue if it reaches [`MAX_ITEMS`].
    pub(crate) fn enqueue(&self, item: T) {
        let mut queue = self.queue.lock().unwrap();
        queue.push(item);
        if queue.len() >= MAX_ITEMS {
            Self::flush_queue_internal(queue, &self.transport, self.name);
        }
    }

    /// Flushes the queue to the transport.
    pub(crate) fn flush(&self) {
        let queue = self.queue.lock().unwrap();
        Self::flush_queue_internal(queue, &self.transport, self.name);
    }

    /// Flushes the queue to the transport.
    ///
    /// This is a static method as it will be called from both the background
    /// thread and the main thread on drop.
    fn flush_queue_internal(
        mut queue_lock: MutexGuard<Vec<T>>,
        transport: &TransportArc,
        name: &str,
    ) {
        let items = std::mem::take(&mut *queue_lock);
        drop(queue_lock);

        if items.is_empty() {
            return;
        }

        sentry_debug!("[Batcher({name})] Flushing {} items", items.len());

        if let Some(ref transport) = *transport.read().unwrap() {
            let mut envelope = Envelope::new();
            envelope.add_item(items);
            transport.send_envelope(envelope);
        }
    }
}

impl<T> Drop for Batcher<T>
where
    EnvelopeItem: From<Vec<T>>,
    T: Send + 'static,
{
    fn drop(&mut self) {
        let (lock, cvar) = self.shutdown.as_ref();
        *lock.lock().unwrap() = true;
        cvar.notify_one();

        if let Some(worker) = self.worker.take() {
            worker.join().ok();
        }
        Self::flush_queue_internal(self.queue.lock().unwrap(), &self.transport, self.name);
    }
}

#[cfg(all(test, feature = "test"))]
mod tests {
    use crate::test;

    // ---- Log batching tests ----

    #[cfg(feature = "logs")]
    mod log_tests {
        use super::*;
        use crate::logger_info;

        #[test]
        fn test_logs_batching() {
            let envelopes = test::with_captured_envelopes_options(
                || {
                    for i in 0..150 {
                        logger_info!("test log {}", i);
                    }
                },
                crate::ClientOptions {
                    enable_logs: true,
                    ..Default::default()
                },
            );

            assert_eq!(2, envelopes.len());

            let mut total_logs = 0;
            for envelope in &envelopes {
                for item in envelope.items() {
                    if let crate::protocol::EnvelopeItem::ItemContainer(
                        crate::protocol::ItemContainer::Logs(logs),
                    ) = item
                    {
                        total_logs += logs.len();
                    }
                }
            }

            assert_eq!(150, total_logs);
        }

        #[test]
        fn test_logs_batcher_flush() {
            let envelopes = test::with_captured_envelopes_options(
                || {
                    for i in 0..12 {
                        logger_info!("test log {}", i);
                    }
                },
                crate::ClientOptions {
                    enable_logs: true,
                    ..Default::default()
                },
            );

            assert_eq!(1, envelopes.len());

            for envelope in &envelopes {
                for item in envelope.items() {
                    if let crate::protocol::EnvelopeItem::ItemContainer(
                        crate::protocol::ItemContainer::Logs(logs),
                    ) = item
                    {
                        assert_eq!(12, logs.len());
                        break;
                    }
                }
            }
        }
    }

    // ---- Metric batching tests ----

    #[cfg(feature = "metrics")]
    mod metric_tests {
        use super::*;
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

        #[test]
        fn test_metrics_disabled_explicitly() {
            let envelopes = test::with_captured_envelopes_options(
                || {
                    for i in 0..10 {
                        crate::Hub::current().capture_metric(test_metric(&format!("metric.{i}")));
                    }
                },
                crate::ClientOptions {
                    enable_metrics: false,
                    ..Default::default()
                },
            );

            assert_eq!(0, envelopes.len());
        }

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
}

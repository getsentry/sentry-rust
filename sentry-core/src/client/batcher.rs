#![cfg(any(feature = "logs", feature = "metrics"))]

//! Generic batching for Sentry envelope items.

use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::EnvelopeSender;
use crate::Envelope;
use crate::protocol::EnvelopeItem;
use sentry_types::protocol::v7::Log;
#[cfg(feature = "metrics")]
use sentry_types::protocol::v7::Metric;

// Flush when there's 100 items in the buffer
const MAX_ITEMS: usize = 100;
// Or when 5 seconds have passed from the last flush
const FLUSH_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug)]
struct BatchQueue<T> {
    items: Vec<T>,
}

pub(super) trait IntoBatchEnvelopeItem: Sized {
    fn into_envelope_item(items: Vec<Self>) -> EnvelopeItem;
}

impl<T> IntoBatchEnvelopeItem for T
where
    Vec<T>: Into<EnvelopeItem>,
{
    fn into_envelope_item(items: Vec<Self>) -> EnvelopeItem {
        items.into()
    }
}

pub(super) trait Batch: IntoBatchEnvelopeItem {
    const TYPE_NAME: &str;
}

impl Batch for Log {
    const TYPE_NAME: &str = "logs";
}

#[cfg(feature = "metrics")]
impl Batch for Metric {
    const TYPE_NAME: &str = "metrics";
}

/// Accumulates items in the queue and submits them through the transport when one of the flushing
/// conditions is met.
pub(super) struct Batcher<T: Batch> {
    envelope_sender: EnvelopeSender,
    queue: Arc<Mutex<BatchQueue<T>>>,
    shutdown: Arc<(Mutex<bool>, Condvar)>,
    worker: Option<JoinHandle<()>>,
}

impl<T> Batcher<T>
where
    T: Batch + Send + 'static,
{
    /// Creates a new Batcher that will submit envelopes to the transport.
    pub(super) fn new(envelope_sender: EnvelopeSender) -> Self {
        let queue = Arc::new(Mutex::new(BatchQueue { items: Vec::new() }));
        #[allow(clippy::mutex_atomic)]
        let shutdown = Arc::new((Mutex::new(false), Condvar::new()));

        let worker_envelope_sender = envelope_sender.clone();
        let worker_queue = queue.clone();
        let worker_shutdown = shutdown.clone();
        let worker = std::thread::Builder::new()
            .name(format!("sentry-{}-batcher", T::TYPE_NAME))
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
                        Batcher::flush_queue_internal(
                            worker_queue.lock().unwrap(),
                            &worker_envelope_sender,
                        );
                        last_flush = Instant::now();
                    }
                }
            })
            .unwrap();

        Self {
            envelope_sender,
            queue,
            shutdown,
            worker: Some(worker),
        }
    }
}

impl<T: Batch> Batcher<T> {
    /// Enqueues an item for delayed sending.
    ///
    /// This will automatically flush the queue if it reaches a size of `MAX_ITEMS`.
    pub(super) fn enqueue(&self, item: T) {
        let mut queue = self.queue.lock().unwrap();
        queue.items.push(item);
        if queue.items.len() >= MAX_ITEMS {
            Batcher::flush_queue_internal(queue, &self.envelope_sender);
        }
    }

    /// Flushes the queue to the transport.
    pub(super) fn flush(&self) {
        let queue = self.queue.lock().unwrap();
        Batcher::flush_queue_internal(queue, &self.envelope_sender);
    }

    /// Flushes the queue to the transport.
    ///
    /// This is a static method as it will be called from both the background
    /// thread and the main thread on drop.
    fn flush_queue_internal(
        mut queue_lock: MutexGuard<BatchQueue<T>>,
        envelope_sender: &EnvelopeSender,
    ) {
        let items = std::mem::take(&mut queue_lock.items);
        drop(queue_lock);

        if items.is_empty() {
            return;
        }

        sentry_debug!("[Batcher({})] Flushing {} items", T::TYPE_NAME, items.len());

        let mut envelope = Envelope::new();
        let envelope_item = T::into_envelope_item(items);
        envelope.add_item(envelope_item);
        envelope_sender.send_envelope(envelope);
    }
}

impl<T: Batch> Drop for Batcher<T> {
    fn drop(&mut self) {
        let (lock, cvar) = self.shutdown.as_ref();
        *lock.lock().unwrap() = true;
        cvar.notify_one();

        if let Some(worker) = self.worker.take() {
            worker.join().ok();
        }
        Batcher::flush_queue_internal(self.queue.lock().unwrap(), &self.envelope_sender);
    }
}

#[cfg(all(test, feature = "test", feature = "logs"))]
mod tests {
    use crate::logger_info;
    use crate::test;

    // Test that logs are sent in batches
    #[test]
    fn test_logs_batching() {
        let envelopes = test::with_captured_envelopes_options(
            || {
                for i in 0..150 {
                    logger_info!("test log {}", i);
                }
            },
            crate::ClientOptions::new().enable_logs(true),
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

    // Test that the batcher is flushed on client close
    #[test]
    fn test_logs_batcher_flush() {
        let envelopes = test::with_captured_envelopes_options(
            || {
                for i in 0..12 {
                    logger_info!("test log {}", i);
                }
            },
            crate::ClientOptions::new().enable_logs(true),
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

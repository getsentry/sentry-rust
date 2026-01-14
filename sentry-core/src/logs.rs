//! Batching for Sentry [structured logs](https://docs.sentry.io/product/explore/logs/).

use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::client::TransportArc;
use crate::protocol::EnvelopeItem;
use crate::Envelope;
use sentry_types::protocol::v7::Log;

// Flush when there's 100 logs in the buffer
const MAX_LOG_ITEMS: usize = 100;
// Or when 5 seconds have passed from the last flush
const FLUSH_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug, Default)]
struct LogQueue {
    logs: Vec<Log>,
}

/// Accumulates logs in the queue and submits them through the transport when one of the flushing
/// conditions is met.
pub(crate) struct LogsBatcher {
    transport: TransportArc,
    queue: Arc<Mutex<LogQueue>>,
    shutdown: Arc<(Mutex<bool>, Condvar)>,
    worker: Option<JoinHandle<()>>,
}

impl LogsBatcher {
    /// Creates a new LogsBatcher that will submit envelopes to the given `transport`.
    pub(crate) fn new(transport: TransportArc) -> Self {
        let queue = Arc::new(Mutex::new(Default::default()));
        #[allow(clippy::mutex_atomic)]
        let shutdown = Arc::new((Mutex::new(false), Condvar::new()));

        let worker_transport = transport.clone();
        let worker_queue = queue.clone();
        let worker_shutdown = shutdown.clone();
        let worker = std::thread::Builder::new()
            .name("sentry-logs-batcher".into())
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
                        LogsBatcher::flush_queue_internal(
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

    /// Enqueues a log for delayed sending.
    ///
    /// This will automatically flush the queue if it reaches a size of `BATCH_SIZE`.
    pub(crate) fn enqueue(&self, log: Log) {
        let mut queue = self.queue.lock().unwrap();
        queue.logs.push(log);
        if queue.logs.len() >= MAX_LOG_ITEMS {
            LogsBatcher::flush_queue_internal(queue, &self.transport);
        }
    }

    /// Flushes the queue to the transport.
    pub(crate) fn flush(&self) {
        let queue = self.queue.lock().unwrap();
        LogsBatcher::flush_queue_internal(queue, &self.transport);
    }

    /// Flushes the queue to the transport.
    ///
    /// This is a static method as it will be called from both the background
    /// thread and the main thread on drop.
    fn flush_queue_internal(mut queue_lock: MutexGuard<LogQueue>, transport: &TransportArc) {
        let logs = std::mem::take(&mut queue_lock.logs);
        drop(queue_lock);

        if logs.is_empty() {
            return;
        }

        sentry_debug!("[LogsBatcher] Flushing {} logs", logs.len());
        if let Some(ref transport) = *transport.read().unwrap() {
            let mut envelope = Envelope::new();
            let logs_item: EnvelopeItem = logs.into();
            envelope.add_item(logs_item);
            transport.send_envelope(envelope);
        }
    }
}

impl Drop for LogsBatcher {
    fn drop(&mut self) {
        let (lock, cvar) = self.shutdown.as_ref();
        *lock.lock().unwrap() = true;
        cvar.notify_one();

        if let Some(worker) = self.worker.take() {
            worker.join().ok();
        }
        LogsBatcher::flush_queue_internal(self.queue.lock().unwrap(), &self.transport);
    }
}

#[cfg(all(test, feature = "test"))]
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

    // Test that the batcher is flushed on client close
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

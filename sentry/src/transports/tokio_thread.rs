use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use sentry_core::client_report::{Reason as ClientReportReason, Recorder as ClientReportRecorder};

use super::ratelimit::{RateLimiter, RateLimitingCategory};
#[cfg(doc)]
use super::{TokioTransportThread, TokioTransportThreadOptions}; // so we can use pub re-exports in docs
use crate::{sentry_debug, Envelope};

#[expect(
    clippy::large_enum_variant,
    reason = "In normal usage this is usually SendEnvelope, the other variants are only used when \
    the user manually calls transport.flush() or when the transport is shut down."
)]
enum Task {
    SendEnvelope(Envelope),
    Flush(SyncSender<()>),
    Shutdown,
}

/// A background-thread powered by [`tokio`] dedicated to sending [`Envelope`]s while respecting the rate limits imposed in the responses.
pub struct TransportThread {
    sender: SyncSender<Task>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    client_report_recorder: ClientReportRecorder,
}

/// Options for constructing a [`TokioTransportThread`].
#[must_use]
pub struct TransportThreadOptions<F> {
    send_fn: F,
    client_report_recorder: ClientReportRecorder,
}

impl<F> TransportThreadOptions<F> {
    /// Creates options with the function used to send envelopes.
    pub fn new(send_fn: F) -> Self {
        Self {
            send_fn,
            client_report_recorder: Default::default(),
        }
    }

    /// Set the [`ClientReportRecorder`] on the options.
    pub fn with_client_report_recorder(self, client_report_recorder: ClientReportRecorder) -> Self {
        Self {
            client_report_recorder,
            ..self
        }
    }
}

impl<F, SendFuture> TransportThreadOptions<F>
where
    F: FnMut(Envelope, RateLimiter) -> SendFuture + Send + 'static,
    // NOTE: return RateLimiter to avoid lifetime issues with mutable borrowing across await.
    SendFuture: std::future::Future<Output = RateLimiter>,
{
    /// Spawn a [`TokioTransportThread`], configured per these options.
    pub fn spawn_thread(self) -> TransportThread {
        TransportThread::with_options(self)
    }
}

impl TransportThread {
    /// Backwards-compatible method to spawn a new background thread.
    ///
    /// Please construct this type via [`TokioTransportThreadOptions`] instead.
    #[deprecated(note = "construct via `TransportThreadOptions` instead")]
    pub fn new<SendFn, SendFuture>(send: SendFn) -> Self
    where
        SendFn: FnMut(Envelope, RateLimiter) -> SendFuture + Send + 'static,
        // NOTE: return RateLimiter to avoid lifetime issues with mutable borrowing across await.
        SendFuture: std::future::Future<Output = RateLimiter>,
    {
        Self::with_options(TransportThreadOptions::new(send))
    }

    /// Spawn a new background thread with options.
    fn with_options<SendFn, SendFuture>(options: TransportThreadOptions<SendFn>) -> Self
    where
        SendFn: FnMut(Envelope, RateLimiter) -> SendFuture + Send + 'static,
        // NOTE: return RateLimiter to avoid lifetime issues with mutable borrowing across await.
        SendFuture: std::future::Future<Output = RateLimiter>,
    {
        let TransportThreadOptions {
            send_fn: mut send,
            client_report_recorder,
        } = options;
        let (sender, receiver) = sync_channel(30);
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_worker = shutdown.clone();
        let handle_client_report_recorder = client_report_recorder.clone();
        let handle = thread::Builder::new()
            .name("sentry-transport".into())
            .spawn(move || {
                // create a runtime on the transport thread
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();

                let mut rl = RateLimiter::new();

                // and block on an async fn in this runtime/thread
                rt.block_on(async move {
                    for task in receiver.into_iter() {
                        if shutdown_worker.load(Ordering::SeqCst) {
                            return;
                        }
                        let envelope = match task {
                            Task::SendEnvelope(envelope) => envelope,
                            Task::Flush(sender) => {
                                sender.send(()).ok();
                                continue;
                            }
                            Task::Shutdown => {
                                return;
                            }
                        };

                        if let Some(time_left) = rl.is_disabled(RateLimitingCategory::Any) {
                            sentry_debug!(
                                "Skipping event send because we're disabled due to rate limits for {}s",
                                time_left.as_secs()
                            );
                            handle_client_report_recorder
                                .record_lost_data(&envelope, ClientReportReason::RatelimitBackoff);
                            continue;
                        }
                        match rl.filter(envelope, &handle_client_report_recorder) {
                            Some(envelope) => {
                                rl = send(envelope, rl).await;
                            }
                            None => {
                                sentry_debug!("Envelope was discarded due to per-item rate limits");
                            }
                        };
                    }
                })
            })
            .ok();

        Self {
            sender,
            shutdown,
            handle,
            client_report_recorder,
        }
    }

    /// Send an [`Envelope`].
    ///
    /// In case the background thread cannot keep up, the [`Envelope`] is dropped.
    pub fn send(&self, envelope: Envelope) {
        // Using send here would mean that when the channel fills up for whatever
        // reason, trying to send an envelope would block everything. We'd rather
        // drop the envelope in that case.
        if let Err(e) = self.sender.try_send(Task::SendEnvelope(envelope)) {
            sentry_debug!("envelope dropped: {e}");

            // Get back the envelope from the TrySendError so we can record it as lost.
            let (task, reason) = match e {
                TrySendError::Full(task) => (task, ClientReportReason::QueueOverflow),
                TrySendError::Disconnected(task) => (task, ClientReportReason::InternalError),
            };
            let Task::SendEnvelope(envelope) = task else {
                unreachable!("we sent a `SendEnvelope` task");
            };

            self.client_report_recorder
                .record_lost_data(&envelope, reason);
        }
    }

    /// Flush all pending [`Envelope`]s.
    ///
    /// Returns true if successful within given timeout.
    pub fn flush(&self, timeout: Duration) -> bool {
        let (sender, receiver) = sync_channel(1);
        let _ = self.sender.send(Task::Flush(sender));
        receiver.recv_timeout(timeout).is_ok()
    }
}

impl Drop for TransportThread {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = self.sender.send(Task::Shutdown);
        if let Some(handle) = self.handle.take() {
            handle.join().unwrap();
        }
    }
}

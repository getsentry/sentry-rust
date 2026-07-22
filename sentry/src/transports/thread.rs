use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam_channel::{bounded, select_biased, unbounded, Sender, TrySendError};
use sentry_core::client_report::{Reason as ClientReportReason, Recorder as ClientReportRecorder};

use super::ratelimit::{RateLimiter, RateLimitingCategory};
use super::DEFAULT_CHANNEL_CAPACITY;
#[cfg(doc)]
use super::{StdTransportThread, StdTransportThreadOptions}; // so we can use pub re-exports in docs
use crate::{sentry_debug, Envelope};

enum ControlTask {
    Flush(Sender<()>),
    Shutdown,
}

/// A background-thread dedicated to sending [`Envelope`]s while respecting the rate limits imposed in the responses.
pub struct TransportThread {
    sender: Sender<Envelope>,
    control_sender: Sender<ControlTask>,
    handle: Option<JoinHandle<()>>,
    client_report_recorder: ClientReportRecorder,
}

/// Options for constructing a [`StdTransportThread`].
#[must_use]
pub struct TransportThreadOptions<F> {
    send_fn: F,
    client_report_recorder: ClientReportRecorder,
    channel_capacity: usize,
}

impl<F> TransportThreadOptions<F> {
    /// Creates options with the function used to send envelopes.
    pub fn new(send_fn: F) -> Self {
        Self {
            send_fn,
            client_report_recorder: Default::default(),
            channel_capacity: DEFAULT_CHANNEL_CAPACITY,
        }
    }

    /// Set the [`ClientReportRecorder`] on the options.
    pub fn with_client_report_recorder(self, client_report_recorder: ClientReportRecorder) -> Self {
        Self {
            client_report_recorder,
            ..self
        }
    }

    /// Set the capacity of the channel that queues envelopes for the background
    /// thread.
    ///
    /// The capacity bounds how many envelopes may be queued before `send`
    /// starts dropping them. A capacity of `0` creates a rendezvous channel:
    /// because `send` uses `try_send`, an envelope is accepted only when the
    /// transport thread is currently waiting on the receiver, otherwise it is
    /// dropped. That is a no-buffer back-pressure policy, not a blanket
    /// "drop everything" mode.
    pub(crate) fn with_channel_capacity(self, channel_capacity: usize) -> Self {
        Self {
            channel_capacity,
            ..self
        }
    }
}

impl<F> TransportThreadOptions<F>
where
    F: FnMut(Envelope, &mut RateLimiter) + Send + 'static,
{
    /// Spawn a [`StdTransportThread`], configured per these options.
    pub fn spawn_thread(self) -> TransportThread {
        TransportThread::with_options(self)
    }
}

impl TransportThread {
    /// Backwards-compatible method to spawn a new background thread.
    ///
    /// Please construct this type via [`StdTransportThreadOptions`] instead.
    #[deprecated(note = "construct via `TransportThreadOptions` instead")]
    pub fn new<SendFn>(send: SendFn) -> Self
    where
        SendFn: FnMut(Envelope, &mut RateLimiter) + Send + 'static,
    {
        Self::with_options(TransportThreadOptions::new(send))
    }

    /// Spawn a new background thread with options.
    fn with_options<SendFn>(options: TransportThreadOptions<SendFn>) -> Self
    where
        SendFn: FnMut(Envelope, &mut RateLimiter) + Send + 'static,
    {
        let TransportThreadOptions {
            send_fn: mut send,
            client_report_recorder,
            channel_capacity,
        } = options;
        let (sender, receiver) = bounded(channel_capacity);
        let (control_sender, control_receiver) = unbounded();
        let handle_client_report_recorder = client_report_recorder.clone();
        let handle = thread::Builder::new()
            .name("sentry-transport".into())
            .spawn(move || {
                let mut rl = RateLimiter::new();
                let mut send_envelope = |envelope| {
                    if let Some(time_left) = rl.is_disabled(RateLimitingCategory::Any) {
                        sentry_debug!(
                            "Skipping event send because we're disabled due to rate limits for {}s",
                            time_left.as_secs()
                        );
                        handle_client_report_recorder
                            .record_lost_data(&envelope, ClientReportReason::RatelimitBackoff);
                    } else {
                        match rl.filter(envelope, &handle_client_report_recorder) {
                            Some(envelope) => {
                                send(envelope, &mut rl);
                            }
                            None => {
                                sentry_debug!("Envelope was discarded due to per-item rate limits");
                            }
                        }
                    }
                };

                loop {
                    select_biased! {
                        recv(control_receiver) -> task => match task {
                            Ok(ControlTask::Flush(sender)) => {
                                for envelope in receiver.try_iter() {
                                    send_envelope(envelope);
                                }
                                sender.send(()).ok();
                            }
                            Ok(ControlTask::Shutdown) | Err(_) => return,
                        },
                        recv(receiver) -> envelope => match envelope {
                            Ok(envelope) => send_envelope(envelope),
                            Err(_) => return,
                        },
                    }
                }
            })
            .ok();

        Self {
            sender,
            control_sender,
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
        if let Err(e) = self.sender.try_send(envelope) {
            sentry_debug!("envelope dropped: {e}");

            // Get back the envelope from the TrySendError so we can record it as lost.
            let (envelope, reason) = match e {
                TrySendError::Full(task) => (task, ClientReportReason::QueueOverflow),
                TrySendError::Disconnected(task) => (task, ClientReportReason::InternalError),
            };
            self.client_report_recorder
                .record_lost_data(&envelope, reason);
        }
    }

    /// Flush all pending [`Envelope`]s.
    ///
    /// Returns true if successful within given timeout.
    pub fn flush(&self, timeout: Duration) -> bool {
        let (sender, receiver) = bounded(1);
        if self
            .control_sender
            .send(ControlTask::Flush(sender))
            .is_err()
        {
            return false;
        }
        receiver.recv_timeout(timeout).is_ok()
    }
}

impl Drop for TransportThread {
    fn drop(&mut self) {
        let (sender, receiver) = bounded(1);
        if self.control_sender.send(ControlTask::Flush(sender)).is_ok() {
            let _ = receiver.recv();
        }
        let _ = self.control_sender.send(ControlTask::Shutdown);
        if let Some(handle) = self.handle.take() {
            handle.join().unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::sync_channel;
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    };
    use std::time::Instant;

    fn envelope() -> Envelope {
        let mut envelope = Envelope::new();
        envelope.add_item(crate::protocol::Event::default());
        envelope
    }

    fn send_rendezvous(transport: &TransportThread) {
        let mut envelope = envelope();
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(1))
            .expect("one-second deadline is representable");
        loop {
            match transport.sender.try_send(envelope) {
                Ok(()) => return,
                Err(TrySendError::Full(returned)) if Instant::now() < deadline => {
                    envelope = returned;
                    thread::yield_now();
                }
                Err(TrySendError::Full(_)) => panic!("worker did not receive rendezvous event"),
                Err(TrySendError::Disconnected(_)) => panic!("worker disconnected"),
            }
        }
    }

    #[test]
    fn flush_wakes_an_idle_transport_thread() {
        let transport =
            TransportThreadOptions::new(|_: Envelope, _: &mut RateLimiter| {}).spawn_thread();

        thread::sleep(Duration::from_millis(1));
        assert!(transport.flush(Duration::from_millis(5)));
    }

    #[test]
    fn flush_waits_for_a_busy_rendezvous_channel() {
        let (started_sender, started_receiver) = sync_channel(1);
        let (release_sender, release_receiver) = sync_channel(1);
        let transport = TransportThreadOptions::new(move |_: Envelope, _: &mut RateLimiter| {
            started_sender.send(()).unwrap();
            release_receiver.recv().unwrap();
        })
        .with_channel_capacity(0)
        .spawn_thread();
        let (result_sender, result_receiver) = sync_channel(1);

        send_rendezvous(&transport);
        started_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        let handle = thread::spawn(move || {
            let result = transport.flush(Duration::from_secs(1));
            result_sender.send(result).unwrap();
        });

        assert!(result_receiver
            .recv_timeout(Duration::from_millis(20))
            .is_err());
        release_sender.send(()).unwrap();
        assert_eq!(
            result_receiver.recv_timeout(Duration::from_secs(1)),
            Ok(true)
        );
        handle.join().unwrap();
    }

    #[test]
    fn flush_drains_queued_envelopes() {
        let (started_sender, started_receiver) = sync_channel(1);
        let (release_sender, release_receiver) = sync_channel(1);
        let sent = Arc::new(AtomicUsize::new(0));
        let sent_worker = sent.clone();
        let block_first = Arc::new(AtomicBool::new(true));
        let block_first_worker = block_first.clone();
        let transport = TransportThreadOptions::new(move |_: Envelope, _: &mut RateLimiter| {
            if block_first_worker.swap(false, Ordering::SeqCst) {
                started_sender.send(()).unwrap();
                release_receiver.recv().unwrap();
            }
            sent_worker.fetch_add(1, Ordering::SeqCst);
        })
        .with_channel_capacity(1)
        .spawn_thread();
        let (result_sender, result_receiver) = sync_channel(1);

        transport.send(envelope());
        started_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        transport.send(envelope());
        let handle = thread::spawn(move || {
            result_sender
                .send(transport.flush(Duration::from_secs(1)))
                .unwrap();
        });

        assert!(result_receiver
            .recv_timeout(Duration::from_millis(20))
            .is_err());
        release_sender.send(()).unwrap();
        assert_eq!(
            result_receiver.recv_timeout(Duration::from_secs(1)),
            Ok(true)
        );
        assert_eq!(sent.load(Ordering::SeqCst), 2);
        handle.join().unwrap();
    }

    #[test]
    fn drop_drains_queued_envelopes() {
        let gate = Arc::new(Mutex::new(()));
        let guard = gate.lock().unwrap();
        let sent = Arc::new(AtomicUsize::new(0));
        let sent_worker = sent.clone();
        let gate_worker = gate.clone();
        let transport = TransportThreadOptions::new(move |_: Envelope, _: &mut RateLimiter| {
            let _guard = gate_worker.lock().unwrap();
            sent_worker.fetch_add(1, Ordering::SeqCst);
        })
        .with_channel_capacity(1)
        .spawn_thread();

        send_rendezvous(&transport);
        send_rendezvous(&transport);
        let handle = thread::spawn(move || drop(transport));
        drop(guard);
        handle.join().unwrap();

        assert_eq!(sent.load(Ordering::SeqCst), 2);
    }
}

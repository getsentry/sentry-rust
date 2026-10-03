//! Moves the crashing thread's scope to the crash reporter.
//!
//! Nothing crosses the process boundary until a crash. The crash handler
//! then names the crashing OS thread, the scope of the hub current on that
//! thread is turned into an [`Event`], and the handler sends the JSON to
//! the reporter before it requests the minidump.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use minidumper_child::{CrashContext, MessageSender};
use sentry_core::protocol::Event;
use sentry_core::Hub;

/// Message kinds on the socket between the app and the reporter.
pub(crate) const MSG_SCOPE_CHUNK: u32 = 1;
pub(crate) const MSG_SCOPE_END: u32 = 2;

/// The reporter reads one message in one call and does not reassemble
/// partial reads on every platform, so keep each message small.
const CHUNK_SIZE: usize = 16 * 1024;

/// Hand-off between the crash handler and the code that serializes the
/// scope.
pub(crate) struct ScopeSync {
    done: AtomicBool,
    buffer: Mutex<Vec<u8>>,
    #[cfg(not(target_os = "macos"))]
    helper: Helper,
}

/// The parked thread that does the serializing on Linux and Windows,
/// where the crash handler runs on the crashing thread and may not
/// allocate.
#[cfg(not(target_os = "macos"))]
struct Helper {
    thread_id: std::sync::atomic::AtomicU64,
    requested: AtomicBool,
    thread: std::thread::Thread,
    timeout: Duration,
}

impl ScopeSync {
    /// Prepares the hand-off. On Linux and Windows this starts the helper
    /// thread, which parks until a crash happens.
    pub(crate) fn start(timeout: Duration) -> Arc<Self> {
        #[cfg(target_os = "macos")]
        {
            let _ = timeout;
            Arc::new(ScopeSync {
                done: AtomicBool::new(false),
                buffer: Mutex::new(Vec::new()),
            })
        }

        #[cfg(not(target_os = "macos"))]
        {
            use std::sync::atomic::AtomicU64;
            use std::thread;

            let (tx, rx) = std::sync::mpsc::sync_channel::<Arc<ScopeSync>>(1);

            let handle = thread::Builder::new()
                .name("sentry-minidump-scope".into())
                .spawn(move || {
                    let Ok(sync) = rx.recv() else { return };
                    while !sync.helper.requested.load(Ordering::Acquire) {
                        thread::park();
                    }
                    let bytes = scope_bytes(sync.helper.thread_id.load(Ordering::Acquire));
                    *sync.buffer.lock().unwrap_or_else(PoisonError::into_inner) = bytes;
                    sync.done.store(true, Ordering::Release);
                })
                .expect("spawn scope helper thread");

            let sync = Arc::new(ScopeSync {
                done: AtomicBool::new(false),
                buffer: Mutex::new(Vec::new()),
                helper: Helper {
                    thread_id: AtomicU64::new(0),
                    requested: AtomicBool::new(false),
                    thread: handle.thread().clone(),
                    timeout,
                },
            });
            tx.send(sync.clone()).expect("helper thread is waiting");
            sync
        }
    }

    /// Runs inside the crash handler.
    ///
    /// On Linux and Windows this is the crashing thread, so it must not
    /// allocate or take locks. It only touches atomics, wakes the helper,
    /// sleeps, and writes from a buffer the helper filled. On macOS every
    /// other thread is suspended, so the work runs inline here instead.
    pub(crate) fn on_crash(&self, crash_context: &CrashContext, sender: &MessageSender<'_>) {
        let thread_id = crashing_thread_id(crash_context);

        #[cfg(target_os = "macos")]
        {
            *self.buffer.lock().unwrap_or_else(PoisonError::into_inner) = scope_bytes(thread_id);
            self.done.store(true, Ordering::Release);
        }

        #[cfg(not(target_os = "macos"))]
        {
            self.helper.thread_id.store(thread_id, Ordering::Release);
            self.helper.requested.store(true, Ordering::Release);
            self.helper.thread.unpark();

            let step = Duration::from_millis(1);
            let mut waited = Duration::ZERO;
            while !self.done.load(Ordering::Acquire) {
                if waited >= self.helper.timeout {
                    return;
                }
                std::thread::sleep(step);
                waited = waited.saturating_add(step);
            }
        }

        // The lock was released before `done` was set, so this cannot block.
        if let Ok(buffer) = self.buffer.try_lock() {
            send_chunks(sender, &buffer);
        }
    }
}

fn send_chunks(sender: &MessageSender<'_>, bytes: &[u8]) {
    for chunk in bytes.chunks(CHUNK_SIZE) {
        if sender.send_message(MSG_SCOPE_CHUNK, chunk).is_err() {
            return;
        }
    }
    sender.send_message(MSG_SCOPE_END, &[]).ok();
}

/// The id of the crashing thread as [`sentry_core::current_os_thread_id`]
/// reports it on that thread.
fn crashing_thread_id(crash_context: &CrashContext) -> u64 {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        crash_context.tid as u64
    }
    #[cfg(target_os = "windows")]
    {
        crash_context.thread_id as u64
    }
    #[cfg(target_os = "macos")]
    {
        crash_context.thread as u64
    }
}

/// Serializes the scope of the hub current on `thread_id`.
///
/// Falls back to the main hub for threads that never used Sentry, which
/// is the scope such a thread would have inherited.
fn scope_bytes(thread_id: u64) -> Vec<u8> {
    let hub = Hub::for_os_thread(thread_id).unwrap_or_else(Hub::main);
    let scope = hub.configure_scope(|scope| scope.clone());
    scope
        .apply_to_event(Event::default())
        .and_then(|event| serde_json::to_vec(&event).ok())
        .unwrap_or_default()
}

/// Collects the scope messages in the reporter process.
#[derive(Default)]
pub(crate) struct ScopeReceiver {
    chunks: Mutex<Vec<u8>>,
    event: Mutex<Option<Event<'static>>>,
}

impl ScopeReceiver {
    pub(crate) fn on_message(&self, kind: u32, buffer: &[u8]) {
        match kind {
            MSG_SCOPE_CHUNK => self
                .chunks
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .extend_from_slice(buffer),
            MSG_SCOPE_END => {
                let bytes = std::mem::take(
                    &mut *self.chunks.lock().unwrap_or_else(PoisonError::into_inner),
                );
                if let Ok(event) = serde_json::from_slice::<Event<'static>>(&bytes) {
                    *self.event.lock().unwrap_or_else(PoisonError::into_inner) = Some(event);
                }
            }
            _ => {}
        }
    }

    /// Returns the received scope as an event, or an empty event.
    pub(crate) fn take_event(&self) -> Event<'static> {
        self.event
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receiver_reassembles_chunks() {
        let event = Event {
            message: Some("hello".into()),
            ..Default::default()
        };
        let bytes = serde_json::to_vec(&event).unwrap();
        let receiver = ScopeReceiver::default();
        let (a, b) = bytes.split_at(bytes.len() / 2);
        receiver.on_message(MSG_SCOPE_CHUNK, a);
        receiver.on_message(MSG_SCOPE_CHUNK, b);
        receiver.on_message(MSG_SCOPE_END, &[]);
        assert_eq!(receiver.take_event().message.as_deref(), Some("hello"));
        assert!(receiver.take_event().message.is_none());
    }

    #[test]
    fn receiver_ignores_bad_json_and_other_kinds() {
        let receiver = ScopeReceiver::default();
        receiver.on_message(99, b"ignored");
        receiver.on_message(MSG_SCOPE_CHUNK, b"not json");
        receiver.on_message(MSG_SCOPE_END, &[]);
        assert!(receiver.take_event().message.is_none());
    }

    #[test]
    fn scope_bytes_uses_the_thread_hub() {
        let hub = Arc::new(Hub::new(None, Default::default()));
        hub.configure_scope(|scope| scope.set_tag("thread", "worker"));
        let bytes = Hub::run(hub, || scope_bytes(sentry_core::current_os_thread_id()));
        let event: Event<'static> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(event.tags.get("thread").map(String::as_str), Some("worker"));
    }
}

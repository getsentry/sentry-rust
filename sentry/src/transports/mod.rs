//! The provided transports.
//!
//! This module exposes all transports that are compiled into the sentry
//! library.  The `reqwest`, `curl` and `surf` features turn on these transports.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::{sentry_debug, ClientOptions, Envelope, Transport, TransportFactory};
use ratelimit::{RateLimiter, RateLimitingCategory};

pub(crate) mod ratelimit;

#[cfg(feature = "reqwest")]
mod reqwest;
#[cfg(feature = "reqwest")]
pub use reqwest::ReqwestHttpTransport;

#[cfg(feature = "curl")]
mod curl;
#[cfg(feature = "curl")]
pub use curl::CurlHttpTransport;

#[cfg(feature = "surf")]
mod surf;
#[cfg(feature = "surf")]
pub use surf::SurfHttpTransport;

#[cfg(feature = "reqwest")]
type DefaultTransport = ReqwestHttpTransport;

#[cfg(all(feature = "curl", not(feature = "reqwest"), not(feature = "surf")))]
type DefaultTransport = CurlHttpTransport;

#[cfg(all(feature = "surf", not(feature = "reqwest"), not(feature = "curl")))]
type DefaultTransport = SurfHttpTransport;

/// The default http transport.
#[cfg(any(feature = "reqwest", feature = "curl", feature = "surf"))]
pub type HttpTransport = DefaultTransport;

/// Creates the default HTTP transport.
///
/// This is the default value for `transport` on the client options.  It
/// creates a `HttpTransport`.  If no http transport was compiled into the
/// library it will panic on transport creation.
#[derive(Clone)]
pub struct DefaultTransportFactory;

impl TransportFactory for DefaultTransportFactory {
    fn create_transport(&self, options: &ClientOptions) -> Arc<dyn Transport> {
        #[cfg(any(feature = "reqwest", feature = "curl", feature = "surf"))]
        {
            Arc::new(HttpTransport::new(options))
        }
        #[cfg(not(any(feature = "reqwest", feature = "curl", feature = "surf")))]
        {
            let _ = options;
            panic!("sentry crate was compiled without transport")
        }
    }
}

enum Task {
    SendEnvelope(Envelope),
    Flush(SyncSender<()>),
    Shutdown,
}

struct TransportThread {
    sender: SyncSender<Task>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl TransportThread {
    fn new<SendFn, SendFuture>(mut send: SendFn) -> Self
    where
        SendFn: FnMut(Envelope, RateLimiter) -> SendFuture + Send + 'static,
        // NOTE: returning RateLimiter here, otherwise we are in borrow hell
        SendFuture: std::future::Future<Output = RateLimiter>,
    {
        let (sender, receiver) = sync_channel(30);
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_worker = shutdown.clone();
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

                        if let Some(time_left) =  rl.is_disabled(RateLimitingCategory::Any) {
                                sentry_debug!(
                                    "Skipping event send because we're disabled due to rate limits for {}s",
                                    time_left.as_secs()
                                );
                                continue;
                            }
                            rl = send(envelope, rl).await;
                    }
                })
            })
            .ok();

        Self {
            sender,
            shutdown,
            handle,
        }
    }

    fn send(&self, envelope: Envelope) {
        let _ = self.sender.send(Task::SendEnvelope(envelope));
    }

    fn flush(&self, timeout: Duration) -> bool {
        let (sender, receiver) = sync_channel(1);
        let _ = self.sender.send(Task::Flush(sender));
        receiver.recv_timeout(timeout).is_err()
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

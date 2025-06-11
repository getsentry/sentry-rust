use std::any::TypeId;
use std::borrow::Cow;
use std::fmt;
use std::panic::RefUnwindSafe;
use std::sync::{Arc, RwLock};
use std::time::Duration;

#[cfg(feature = "release-health")]
use crate::protocol::SessionUpdate;
use rand::random;
use sentry_types::random_uuid;

use crate::constants::SDK_INFO;
#[cfg(feature = "logs")]
use crate::logs::LogsBatcher;
use crate::protocol::{ClientSdkInfo, Event};
#[cfg(feature = "release-health")]
use crate::session::SessionFlusher;
use crate::types::{Dsn, Uuid, Scheme};
#[cfg(feature = "release-health")]
use crate::SessionMode;
use crate::{ClientOptions, Envelope, Hub, Integration, Scope, Transport};
#[cfg(feature = "logs")]
use sentry_types::protocol::v7::{Log, LogAttribute};
use crate::sentry_debug;

impl<T: Into<ClientOptions>> From<T> for Client {
    fn from(o: T) -> Client {
        Client::with_options(o.into())
    }
}

pub(crate) type TransportArc = Arc<RwLock<Option<Arc<dyn Transport>>>>;

/// The Sentry Client.
///
/// The Client is responsible for event processing and sending events to the
/// sentry server via the configured [`Transport`]. It can be created from a
/// [`ClientOptions`].
///
/// See the [Unified API] document for more details.
///
/// # Examples
///
/// ```
/// sentry::Client::from(sentry::ClientOptions::default());
/// ```
///
/// [`ClientOptions`]: struct.ClientOptions.html
/// [`Transport`]: trait.Transport.html
/// [Unified API]: https://develop.sentry.dev/sdk/unified-api/
pub struct Client {
    options: ClientOptions,
    transport: TransportArc,
    #[cfg(feature = "release-health")]
    session_flusher: RwLock<Option<SessionFlusher>>,
    #[cfg(feature = "logs")]
    logs_batcher: RwLock<Option<LogsBatcher>>,
    integrations: Vec<(TypeId, Arc<dyn Integration>)>,
    pub(crate) sdk_info: ClientSdkInfo,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("dsn", &self.dsn())
            .field("options", &self.options)
            .finish()
    }
}

impl Clone for Client {
    fn clone(&self) -> Client {
        let transport = Arc::new(RwLock::new(self.transport.read().unwrap().clone()));

        #[cfg(feature = "release-health")]
        let session_flusher = RwLock::new(Some(SessionFlusher::new(
            transport.clone(),
            self.options.session_mode,
        )));

        #[cfg(feature = "logs")]
        let logs_batcher = RwLock::new(if self.options.enable_logs {
            Some(LogsBatcher::new(transport.clone()))
        } else {
            None
        });

        Client {
            options: self.options.clone(),
            transport,
            #[cfg(feature = "release-health")]
            session_flusher,
            #[cfg(feature = "logs")]
            logs_batcher,
            integrations: self.integrations.clone(),
            sdk_info: self.sdk_info.clone(),
        }
    }
}

impl Client {
    /// Creates a new Sentry client from a config.
    ///
    /// # Supported Configs
    ///
    /// The following common values are supported for the client config:
    ///
    /// * `ClientOptions`: configure the client with the given client options.
    /// * `()` or empty string: Disable the client.
    /// * `&str` / `String` / `&OsStr` / `String`: configure the client with the given DSN.
    /// * `Dsn` / `&Dsn`: configure the client with a given DSN.
    /// * `(Dsn, ClientOptions)`: configure the client from the given DSN and optional options.
    ///
    /// The `Default` implementation of `ClientOptions` pulls in the DSN from the
    /// `SENTRY_DSN` environment variable.
    ///
    /// # Panics
    ///
    /// The `Into<ClientOptions>` implementations can panic for the forms where a DSN needs to be
    /// parsed.  If you want to handle invalid DSNs you need to parse them manually by calling
    /// parse on it and handle the error.
    pub fn from_config<O: Into<ClientOptions>>(opts: O) -> Client {
        Client::with_options(opts.into())
    }

    /// Creates a new sentry client for the given options.
    ///
    /// If the DSN on the options is set to `None` the client will be entirely
    /// disabled.
    pub fn with_options(mut options: ClientOptions) -> Client {
        sentry_debug!("[Client] Creating new client with options: debug={}, dsn={:?}", 
                     options.debug, options.dsn.as_ref().map(|dsn| dsn.to_string()));
        
        // Create the main hub eagerly to avoid problems with the background thread
        // See https://github.com/getsentry/sentry-rust/issues/237
        Hub::with(|_| {});

        let create_transport = || {
            if options.dsn.as_ref()?.scheme() == Scheme::Https || options.dsn.as_ref()?.scheme() == Scheme::Http {
                sentry_debug!("[Client] DSN scheme is supported, creating transport");
            } else {
                sentry_debug!("[Client] DSN scheme is not supported");
                return None;
            }
            options.dsn.as_ref()?;
            let factory = options.transport.as_ref()?;
            Some(factory.create_transport(&options))
        };

        let transport = Arc::new(RwLock::new(create_transport()));
        if transport.read().unwrap().is_some() {
            sentry_debug!("[Client] Transport created successfully");
        } else {
            sentry_debug!("[Client] No transport available (client will be disabled)");
        }

        let mut sdk_info = SDK_INFO.clone();

        // NOTE: We do not filter out duplicate integrations based on their
        // TypeId.
        let integrations: Vec<_> = options
            .integrations
            .iter()
            .map(|integration| (integration.as_ref().type_id(), integration.clone()))
            .collect();

        sentry_debug!("[Client] Setting up {} integrations", integrations.len());
        for (_, integration) in integrations.iter() {
            sentry_debug!("[Client] Setting up integration: {}", integration.name());
            integration.setup(&mut options);
            sdk_info.integrations.push(integration.name().to_string());
        }

        #[cfg(feature = "release-health")]
        let session_flusher = {
            sentry_debug!("[Client] Creating session flusher for mode: {:?}", options.session_mode);
            RwLock::new(Some(SessionFlusher::new(
                transport.clone(),
                options.session_mode,
            )))
        };

        #[cfg(feature = "logs")]
        let logs_batcher = RwLock::new(if options.enable_logs {
            sentry_debug!("[Client] Creating logs batcher");
            Some(LogsBatcher::new(transport.clone()))
        } else {
            sentry_debug!("[Client] Logs are disabled, no batcher created");
            None
        });

        sentry_debug!("[Client] Client initialization complete");

        Client {
            options,
            transport,
            #[cfg(feature = "release-health")]
            session_flusher,
            #[cfg(feature = "logs")]
            logs_batcher,
            integrations,
            sdk_info,
        }
    }

    pub(crate) fn get_integration<I>(&self) -> Option<&I>
    where
        I: Integration,
    {
        let id = TypeId::of::<I>();
        let integration = &self.integrations.iter().find(|(iid, _)| *iid == id)?.1;
        integration.as_ref().as_any().downcast_ref()
    }

    /// Prepares an event for transmission to sentry.
    pub fn prepare_event(
        &self,
        mut event: Event<'static>,
        scope: Option<&Scope>,
    ) -> Option<Event<'static>> {
        sentry_debug!("[Client] Preparing event {} for transmission", event.event_id);
        
        // event_id and sdk_info are set before the processors run so that the
        // processors can poke around in that data.
        if event.event_id.is_nil() {
            event.event_id = random_uuid();
            sentry_debug!("[Client] Generated new event ID: {}", event.event_id);
        }

        if event.sdk.is_none() {
            // NOTE: we need to clone here because `Event` must be `'static`
            event.sdk = Some(Cow::Owned(self.sdk_info.clone()));
            sentry_debug!("[Client] Added SDK info to event");
        }

        if let Some(scope) = scope {
            sentry_debug!("[Client] Applying scope to event");
            event = scope.apply_to_event(event)?;
        }

        sentry_debug!("[Client] Processing event through {} integrations", self.integrations.len());
        for (_, integration) in self.integrations.iter() {
            let id = event.event_id;
            sentry_debug!("[Client] Processing event {} through integration: {}", id, integration.name());
            event = match integration.process_event(event, &self.options) {
                Some(event) => event,
                None => {
                    sentry_debug!("[Client] Integration '{}' dropped event {:?}", integration.name(), id);
                    return None;
                }
            }
        }

        let mut changed_fields = Vec::new();
        if event.release.is_none() {
            event.release.clone_from(&self.options.release);
            if self.options.release.is_some() {
                changed_fields.push("release");
            }
        }
        if event.environment.is_none() {
            event.environment.clone_from(&self.options.environment);
            if self.options.environment.is_some() {
                changed_fields.push("environment");
            }
        }
        if event.server_name.is_none() {
            event.server_name.clone_from(&self.options.server_name);
            if self.options.server_name.is_some() {
                changed_fields.push("server_name");
            }
        }
        if &event.platform == "other" {
            event.platform = "native".into();
            changed_fields.push("platform");
        }

        if !changed_fields.is_empty() {
            sentry_debug!("[Client] Applied client defaults to event fields: {}", changed_fields.join(", "));
        }

        if let Some(ref func) = self.options.before_send {
            sentry_debug!("[Client] Invoking before_send callback for event {}", event.event_id);
            let id = event.event_id;
            if let Some(processed_event) = func(event) {
                event = processed_event;
                sentry_debug!("[Client] before_send callback processed event {}", id);
            } else {
                sentry_debug!("[Client] before_send callback dropped event {:?}", id);
                return None;
            }
        }

        if let Some(scope) = scope {
            scope.update_session_from_event(&event);
        }

        if !self.sample_should_send(self.options.sample_rate) {
            sentry_debug!("[Client] Event {} dropped due to sampling (rate: {})", event.event_id, self.options.sample_rate);
            None
        } else {
            sentry_debug!("[Client] Event {} prepared successfully", event.event_id);
            Some(event)
        }
    }

    /// Returns the options of this client.
    pub fn options(&self) -> &ClientOptions {
        &self.options
    }

    /// Returns the DSN that constructed this client.
    pub fn dsn(&self) -> Option<&Dsn> {
        self.options.dsn.as_ref()
    }

    /// Quick check to see if the client is enabled.
    ///
    /// The Client is enabled if it has a valid DSN and Transport configured.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    ///
    /// let client = sentry::Client::from(sentry::ClientOptions::default());
    /// assert!(!client.is_enabled());
    ///
    /// let dsn = "https://public@example.com/1";
    /// let transport = sentry::test::TestTransport::new();
    /// let client = sentry::Client::from((
    ///     dsn,
    ///     sentry::ClientOptions {
    ///         transport: Some(Arc::new(transport)),
    ///         ..Default::default()
    ///     },
    /// ));
    /// assert!(client.is_enabled());
    /// ```
    pub fn is_enabled(&self) -> bool {
        self.options.dsn.is_some() && self.transport.read().unwrap().is_some()
    }

    /// Captures an event and sends it to sentry.
    pub fn capture_event(&self, event: Event<'static>, scope: Option<&Scope>) -> Uuid {
        sentry_debug!("[Client] Capturing event: {}", event.event_id);
        
        if let Some(ref transport) = *self.transport.read().unwrap() {
            if let Some(event) = self.prepare_event(event, scope) {
                let event_id = event.event_id;
                let mut envelope: Envelope = event.into();
                sentry_debug!("[Client] Created envelope for event {}", event_id);
                
                // For request-mode sessions, we aggregate them all instead of
                // flushing them out early.
                #[cfg(feature = "release-health")]
                if self.options.session_mode == SessionMode::Application {
                    let session_item = scope.and_then(|scope| {
                        scope
                            .session
                            .lock()
                            .unwrap()
                            .as_mut()
                            .and_then(|session| session.create_envelope_item())
                    });
                    if let Some(session_item) = session_item {
                        sentry_debug!("[Client] Added session item to envelope");
                        envelope.add_item(session_item);
                    }
                }

                if let Some(scope) = scope {
                    let attachment_count = scope.attachments.len();
                    if attachment_count > 0 {
                        sentry_debug!("[Client] Adding {} attachments to envelope", attachment_count);
                    }
                    for attachment in scope.attachments.iter().cloned() {
                        envelope.add_item(attachment);
                    }
                }

                sentry_debug!("[Client] Sending envelope for event {}", event_id);
                transport.send_envelope(envelope);
                return event_id;
            }
        } else {
            sentry_debug!("[Client] No transport available, cannot capture event");
        }
        Default::default()
    }

    /// Sends the specified [`Envelope`] to sentry.
    pub fn send_envelope(&self, envelope: Envelope) {
        sentry_debug!("[Client] Sending envelope directly");
        if let Some(ref transport) = *self.transport.read().unwrap() {
            transport.send_envelope(envelope);
        } else {
            sentry_debug!("[Client] No transport available, cannot send envelope");
        }
    }

    #[cfg(feature = "release-health")]
    pub(crate) fn enqueue_session(&self, session_update: SessionUpdate<'static>) {
        sentry_debug!("[Client] Enqueueing session update");
        if let Some(ref flusher) = *self.session_flusher.read().unwrap() {
            flusher.enqueue(session_update);
        }
    }

    /// Drains all pending events without shutting down.
    pub fn flush(&self, timeout: Option<Duration>) -> bool {
        let timeout_ms = timeout.unwrap_or(self.options.shutdown_timeout).as_millis();
        sentry_debug!("[Client] Flushing all pending events (timeout: {}ms)", timeout_ms);
        
        #[cfg(feature = "release-health")]
        if let Some(ref flusher) = *self.session_flusher.read().unwrap() {
            sentry_debug!("[Client] Flushing session data");
            flusher.flush();
        }
        #[cfg(feature = "logs")]
        if let Some(ref batcher) = *self.logs_batcher.read().unwrap() {
            sentry_debug!("[Client] Flushing logs");
            batcher.flush();
        }
        if let Some(ref transport) = *self.transport.read().unwrap() {
            let result = transport.flush(timeout.unwrap_or(self.options.shutdown_timeout));
            sentry_debug!("[Client] Transport flush completed: {}", if result { "success" } else { "timeout/failure" });
            result
        } else {
            sentry_debug!("[Client] No transport to flush");
            true
        }
    }

    /// Drains all pending events and shuts down the transport behind the
    /// client.  After shutting down the transport is removed.
    ///
    /// This returns `true` if the queue was successfully drained in the
    /// given time or `false` if not (for instance because of a timeout).
    /// If no timeout is provided the client will wait for as long a
    /// `shutdown_timeout` in the client options.
    pub fn close(&self, timeout: Option<Duration>) -> bool {
        let timeout_ms = timeout.unwrap_or(self.options.shutdown_timeout).as_millis();
        sentry_debug!("[Client] Closing client (timeout: {}ms)", timeout_ms);
        
        #[cfg(feature = "release-health")]
        {
            sentry_debug!("[Client] Closing session flusher");
            drop(self.session_flusher.write().unwrap().take());
        }
        #[cfg(feature = "logs")]
        {
            sentry_debug!("[Client] Closing logs batcher");
            drop(self.logs_batcher.write().unwrap().take());
        }
        let transport_opt = self.transport.write().unwrap().take();
        if let Some(transport) = transport_opt {
            sentry_debug!("[Client] Requesting transport shutdown");
            let result = transport.shutdown(timeout.unwrap_or(self.options.shutdown_timeout));
            sentry_debug!("[Client] Transport shutdown completed: {}", if result { "success" } else { "timeout/failure" });
            result
        } else {
            sentry_debug!("[Client] No transport to shut down");
            true
        }
    }

    /// Returns a random boolean with a probability defined
    /// by rate
    pub fn sample_should_send(&self, rate: f32) -> bool {
        if rate >= 1.0 {
            true
        } else if rate <= 0.0 {
            false
        } else {
            random::<f32>() < rate
        }
    }

    /// Captures a log and sends it to Sentry.
    #[cfg(feature = "logs")]
    pub fn capture_log(&self, log: Log, scope: &Scope) {
        if !self.options().enable_logs {
            sentry_debug!("[Client] Logs are disabled, ignoring log capture");
            return;
        }
        sentry_debug!("[Client] Capturing log with level: {:?}", log.level);
        if let Some(log) = self.prepare_log(log, scope) {
            if let Some(ref batcher) = *self.logs_batcher.read().unwrap() {
                batcher.enqueue(log);
            } else {
                sentry_debug!("[Client] No logs batcher available");
            }
        }
    }

    /// Prepares a log to be sent, setting the `trace_id` and other default attributes, and
    /// processing it through `before_send_log`.
    #[cfg(feature = "logs")]
    fn prepare_log(&self, mut log: Log, scope: &Scope) -> Option<Log> {
        sentry_debug!("[Client] Preparing log for transmission");
        
        scope.apply_to_log(&mut log, self.options.send_default_pii);
        sentry_debug!("[Client] Applied scope to log");

        self.set_log_default_attributes(&mut log);
        sentry_debug!("[Client] Set default log attributes");

        if let Some(ref func) = self.options.before_send_log {
            sentry_debug!("[Client] Invoking before_send_log callback");
            log = func(log)?;
            sentry_debug!("[Client] before_send_log callback processed log");
        }

        Some(log)
    }

    #[cfg(feature = "logs")]
    fn set_log_default_attributes(&self, log: &mut Log) {
        let mut added_attrs = Vec::new();
        
        if !log.attributes.contains_key("sentry.environment") {
            if let Some(environment) = self.options.environment.as_ref() {
                log.attributes.insert(
                    "sentry.environment".to_owned(),
                    LogAttribute(environment.clone().into()),
                );
                added_attrs.push("sentry.environment");
            }
        }

        if !log.attributes.contains_key("sentry.release") {
            if let Some(release) = self.options.release.as_ref() {
                log.attributes.insert(
                    "sentry.release".to_owned(),
                    LogAttribute(release.clone().into()),
                );
                added_attrs.push("sentry.release");
            }
        }

        if !log.attributes.contains_key("sentry.sdk.name") {
            log.attributes.insert(
                "sentry.sdk.name".to_owned(),
                LogAttribute(self.sdk_info.name.to_owned().into()),
            );
            added_attrs.push("sentry.sdk.name");
        }

        if !log.attributes.contains_key("sentry.sdk.version") {
            log.attributes.insert(
                "sentry.sdk.version".to_owned(),
                LogAttribute(self.sdk_info.version.to_owned().into()),
            );
            added_attrs.push("sentry.sdk.version");
        }

        // TODO: set OS (and Rust?) context

        if !log.attributes.contains_key("server.address") {
            if let Some(server) = &self.options.server_name {
                log.attributes.insert(
                    "server.address".to_owned(),
                    LogAttribute(server.clone().into()),
                );
                added_attrs.push("server.address");
            }
        }

        if !added_attrs.is_empty() {
            sentry_debug!("[Client] Added default log attributes: {}", added_attrs.join(", "));
        }
    }
}

// Make this unwind safe. It's not out of the box because of the
// `BeforeCallback`s inside `ClientOptions`, and the contained Integrations
impl RefUnwindSafe for Client {}

use std::any::TypeId;
use std::borrow::Cow;
#[cfg(any(feature = "logs", feature = "metrics"))]
use std::collections::BTreeMap;
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
#[cfg(feature = "metrics")]
use crate::metrics::MetricsBatcher;
use crate::protocol::{ClientSdkInfo, Event};
#[cfg(feature = "release-health")]
use crate::session::SessionFlusher;
use crate::types::{Dsn, Uuid};
#[cfg(feature = "release-health")]
use crate::SessionMode;
use crate::{ClientOptions, Envelope, Hub, Integration, Scope, Transport};
#[cfg(feature = "logs")]
use sentry_types::protocol::v7::Context;
#[cfg(feature = "logs")]
use sentry_types::protocol::v7::{Log, LogAttribute};
#[cfg(feature = "metrics")]
use sentry_types::protocol::v7::TraceMetric;
#[cfg(all(feature = "metrics", not(feature = "logs")))]
use sentry_types::protocol::v7::LogAttribute;

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
    #[cfg(feature = "logs")]
    default_log_attributes: Option<BTreeMap<String, LogAttribute>>,
    #[cfg(feature = "metrics")]
    metrics_batcher: RwLock<Option<MetricsBatcher>>,
    #[cfg(feature = "metrics")]
    default_metric_attributes: Option<BTreeMap<String, LogAttribute>>,
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

        #[cfg(feature = "metrics")]
        let metrics_batcher = RwLock::new(if self.options.enable_metrics {
            Some(MetricsBatcher::new(transport.clone()))
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
            #[cfg(feature = "logs")]
            default_log_attributes: self.default_log_attributes.clone(),
            #[cfg(feature = "metrics")]
            metrics_batcher,
            #[cfg(feature = "metrics")]
            default_metric_attributes: self.default_metric_attributes.clone(),
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
        // Create the main hub eagerly to avoid problems with the background thread
        // See https://github.com/getsentry/sentry-rust/issues/237
        Hub::with(|_| {});

        let create_transport = || {
            options.dsn.as_ref()?;
            let factory = options.transport.as_ref()?;
            Some(factory.create_transport(&options))
        };

        let transport = Arc::new(RwLock::new(create_transport()));

        let mut sdk_info = SDK_INFO.clone();

        // NOTE: We do not filter out duplicate integrations based on their
        // TypeId.
        let integrations: Vec<_> = options
            .integrations
            .iter()
            .map(|integration| (integration.as_ref().type_id(), integration.clone()))
            .collect();

        for (_, integration) in integrations.iter() {
            integration.setup(&mut options);
            sdk_info.integrations.push(integration.name().to_string());
        }

        #[cfg(feature = "release-health")]
        let session_flusher = RwLock::new(Some(SessionFlusher::new(
            transport.clone(),
            options.session_mode,
        )));

        #[cfg(feature = "logs")]
        let logs_batcher = RwLock::new(if options.enable_logs {
            Some(LogsBatcher::new(transport.clone()))
        } else {
            None
        });

        #[cfg(feature = "metrics")]
        let metrics_batcher = RwLock::new(if options.enable_metrics {
            Some(MetricsBatcher::new(transport.clone()))
        } else {
            None
        });

        #[allow(unused_mut)]
        let mut client = Client {
            options,
            transport,
            #[cfg(feature = "release-health")]
            session_flusher,
            #[cfg(feature = "logs")]
            logs_batcher,
            #[cfg(feature = "logs")]
            default_log_attributes: None,
            #[cfg(feature = "metrics")]
            metrics_batcher,
            #[cfg(feature = "metrics")]
            default_metric_attributes: None,
            integrations,
            sdk_info,
        };

        #[cfg(feature = "logs")]
        client.cache_default_log_attributes();

        #[cfg(feature = "metrics")]
        client.cache_default_metric_attributes();

        client
    }

    #[cfg(feature = "logs")]
    fn cache_default_log_attributes(&mut self) {
        let mut attributes = BTreeMap::new();

        if let Some(environment) = self.options.environment.as_ref() {
            attributes.insert("sentry.environment".to_owned(), environment.clone().into());
        }

        if let Some(release) = self.options.release.as_ref() {
            attributes.insert("sentry.release".to_owned(), release.clone().into());
        }

        attributes.insert(
            "sentry.sdk.name".to_owned(),
            self.sdk_info.name.to_owned().into(),
        );

        attributes.insert(
            "sentry.sdk.version".to_owned(),
            self.sdk_info.version.to_owned().into(),
        );

        // Process a fake event through integrations, so that `ContextIntegration` (if available)
        // provides the OS Context.
        // This is needed as that integration adds the OS Context to events using an event
        // processor, which logs don't go through.
        // We cannot get the `ContextIntegration` directly, as its type lives in `sentry-contexts`,
        // which `sentry-core` doesn't depend on.
        let mut fake_event = Event::default();
        for (_, integration) in self.integrations.iter() {
            if let Some(res) = integration.process_event(fake_event.clone(), &self.options) {
                fake_event = res;
            }
        }

        if let Some(Context::Os(os)) = fake_event.contexts.get("os") {
            if let Some(name) = os.name.as_ref() {
                attributes.insert("os.name".to_owned(), name.to_owned().into());
            }
            if let Some(version) = os.version.as_ref() {
                attributes.insert("os.version".to_owned(), version.to_owned().into());
            }
        }

        if let Some(server) = &self.options.server_name {
            attributes.insert("server.address".to_owned(), server.clone().into());
        }

        self.default_log_attributes = Some(attributes);
    }

    #[cfg(feature = "metrics")]
    fn cache_default_metric_attributes(&mut self) {
        let mut attributes = BTreeMap::new();

        if let Some(environment) = self.options.environment.as_ref() {
            attributes.insert("sentry.environment".to_owned(), environment.clone().into());
        }

        if let Some(release) = self.options.release.as_ref() {
            attributes.insert("sentry.release".to_owned(), release.clone().into());
        }

        attributes.insert(
            "sentry.sdk.name".to_owned(),
            self.sdk_info.name.to_owned().into(),
        );

        attributes.insert(
            "sentry.sdk.version".to_owned(),
            self.sdk_info.version.to_owned().into(),
        );

        if let Some(server) = &self.options.server_name {
            attributes.insert("server.address".to_owned(), server.clone().into());
        }

        self.default_metric_attributes = Some(attributes);
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
        // event_id and sdk_info are set before the processors run so that the
        // processors can poke around in that data.
        if event.event_id.is_nil() {
            event.event_id = random_uuid();
        }

        if event.sdk.is_none() {
            // NOTE: we need to clone here because `Event` must be `'static`
            event.sdk = Some(Cow::Owned(self.sdk_info.clone()));
        }

        if let Some(scope) = scope {
            event = scope.apply_to_event(event)?;
        }

        for (_, integration) in self.integrations.iter() {
            let id = event.event_id;
            event = match integration.process_event(event, &self.options) {
                Some(event) => event,
                None => {
                    sentry_debug!("integration dropped event {:?}", id);
                    return None;
                }
            }
        }

        if event.release.is_none() {
            event.release.clone_from(&self.options.release);
        }
        if event.environment.is_none() {
            event.environment.clone_from(&self.options.environment);
        }
        if event.server_name.is_none() {
            event.server_name.clone_from(&self.options.server_name);
        }
        if &event.platform == "other" {
            event.platform = "native".into();
        }

        if let Some(ref func) = self.options.before_send {
            sentry_debug!("invoking before_send callback");
            let id = event.event_id;
            if let Some(processed_event) = func(event) {
                event = processed_event;
            } else {
                sentry_debug!("before_send dropped event {:?}", id);
                return None;
            }
        }

        if let Some(scope) = scope {
            scope.update_session_from_event(&event);
        }

        if !self.sample_should_send(self.options.sample_rate) {
            None
        } else {
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
        if let Some(ref transport) = *self.transport.read().unwrap() {
            if let Some(event) = self.prepare_event(event, scope) {
                let event_id = event.event_id;
                let mut envelope: Envelope = event.into();
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
                        envelope.add_item(session_item);
                    }
                }

                if let Some(scope) = scope {
                    for attachment in scope.attachments.iter().cloned() {
                        envelope.add_item(attachment);
                    }
                }

                transport.send_envelope(envelope);
                return event_id;
            }
        }
        Default::default()
    }

    /// Sends the specified [`Envelope`] to sentry.
    pub fn send_envelope(&self, envelope: Envelope) {
        if let Some(ref transport) = *self.transport.read().unwrap() {
            transport.send_envelope(envelope);
        }
    }

    #[cfg(feature = "release-health")]
    pub(crate) fn enqueue_session(&self, session_update: SessionUpdate<'static>) {
        if let Some(ref flusher) = *self.session_flusher.read().unwrap() {
            flusher.enqueue(session_update);
        }
    }

    /// Drains all pending events without shutting down.
    pub fn flush(&self, timeout: Option<Duration>) -> bool {
        #[cfg(feature = "release-health")]
        if let Some(ref flusher) = *self.session_flusher.read().unwrap() {
            flusher.flush();
        }
        #[cfg(feature = "logs")]
        if let Some(ref batcher) = *self.logs_batcher.read().unwrap() {
            batcher.flush();
        }
        #[cfg(feature = "metrics")]
        if let Some(ref batcher) = *self.metrics_batcher.read().unwrap() {
            batcher.flush();
        }
        if let Some(ref transport) = *self.transport.read().unwrap() {
            transport.flush(timeout.unwrap_or(self.options.shutdown_timeout))
        } else {
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
        #[cfg(feature = "release-health")]
        drop(self.session_flusher.write().unwrap().take());
        #[cfg(feature = "logs")]
        drop(self.logs_batcher.write().unwrap().take());
        #[cfg(feature = "metrics")]
        drop(self.metrics_batcher.write().unwrap().take());
        let transport_opt = self.transport.write().unwrap().take();
        if let Some(transport) = transport_opt {
            sentry_debug!("client close; request transport to shut down");
            transport.shutdown(timeout.unwrap_or(self.options.shutdown_timeout))
        } else {
            sentry_debug!("client close; no transport to shut down");
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
        if !self.options.enable_logs {
            sentry_debug!("[Client] called capture_log, but options.enable_logs is set to false");
            return;
        }
        if let Some(log) = self.prepare_log(log, scope) {
            if let Some(ref batcher) = *self.logs_batcher.read().unwrap() {
                batcher.enqueue(log);
            }
        }
    }

    /// Prepares a log to be sent, setting the `trace_id` and other default attributes, and
    /// processing it through `before_send_log`.
    #[cfg(feature = "logs")]
    fn prepare_log(&self, mut log: Log, scope: &Scope) -> Option<Log> {
        scope.apply_to_log(&mut log);

        if let Some(default_attributes) = self.default_log_attributes.as_ref() {
            for (key, val) in default_attributes.iter() {
                log.attributes.entry(key.to_owned()).or_insert(val.clone());
            }
        }

        if let Some(ref func) = self.options.before_send_log {
            log = func(log)?;
        }

        Some(log)
    }

    /// Captures a trace metric and sends it to Sentry.
    #[cfg(feature = "metrics")]
    pub fn capture_metric(&self, metric: TraceMetric, scope: &Scope) {
        if !self.options.enable_metrics {
            sentry_debug!(
                "[Client] called capture_metric, but options.enable_metrics is set to false"
            );
            return;
        }
        if let Some(metric) = self.prepare_metric(metric, scope) {
            if let Some(ref batcher) = *self.metrics_batcher.read().unwrap() {
                batcher.enqueue(metric);
            }
        }
    }

    /// Prepares a metric to be sent, setting the `trace_id` and other default attributes, and
    /// processing it through `before_send_metric`.
    #[cfg(feature = "metrics")]
    fn prepare_metric(&self, mut metric: TraceMetric, scope: &Scope) -> Option<TraceMetric> {
        scope.apply_to_metric(&mut metric);

        if let Some(default_attributes) = self.default_metric_attributes.as_ref() {
            for (key, val) in default_attributes.iter() {
                metric
                    .attributes
                    .entry(key.to_owned())
                    .or_insert(val.clone());
            }
        }

        if let Some(ref func) = self.options.before_send_metric {
            metric = func(metric)?;
        }

        Some(metric)
    }
}

// Make this unwind safe. It's not out of the box because of the
// `BeforeCallback`s inside `ClientOptions`, and the contained Integrations
impl RefUnwindSafe for Client {}

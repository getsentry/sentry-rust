THIS SHOULD BE A LINTER ERRORuse std::fmt;

#[cfg(feature = "logs")]
use crate::protocol::Log;
use crate::protocol::{Context, Event, Level, User, Value};
use crate::TransactionOrSpan;

/// A minimal API scope guard.
///
/// Doesn't do anything but can be debug formatted.
#[derive(Default)]
pub struct ScopeGuard;

impl fmt::Debug for ScopeGuard {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ScopeGuard")
    }
}

/// The minimal scope.
///
/// In minimal API mode all modification functions are available as normally
/// just that generally calling them is impossible.
#[derive(Debug, Clone)]
pub struct Scope;

impl Scope {
    /// Clear the scope.
    ///
    /// By default a scope will inherit all values from the higher scope.
    /// In some situations this might not be what a user wants.  Calling
    /// this method will wipe all data contained within.
    pub fn clear(&mut self) {
        minimal_unreachable!();
    }

    /// Sets a level override.
    ///
    /// The level determines the severity of events captured within this scope.
    /// When set, this overrides the level of any events sent to Sentry.
    /// Common levels include [`Level::Error`], [`Level::Warning`], [`Level::Info`], and [`Level::Debug`].
    /// 
    /// Setting this to `None` removes any level override, allowing events to use their original level.
    pub fn set_level(&mut self, level: Option<Level>) {
        let _level = level;
        minimal_unreachable!();
    }

    /// Sets the fingerprint.
    ///
    /// Fingerprints control how Sentry groups events together into issues.
    /// By default, Sentry uses automatic grouping based on stack traces and error messages.
    /// Set a custom fingerprint to override this behavior - events with the same fingerprint
    /// will be grouped into the same issue.
    ///
    /// Pass `None` to use Sentry's default grouping algorithm.
    /// Pass a slice of strings to define a custom grouping key.
    ///
    /// # Examples
    /// ```
    /// scope.set_fingerprint(Some(&["payment-error", "stripe"]));
    /// scope.set_fingerprint(Some(&["{{ default }}", "custom-tag"])); // Combine with default
    /// ```
    pub fn set_fingerprint(&mut self, fingerprint: Option<&[&str]>) {
        let _fingerprint = fingerprint;
        minimal_unreachable!();
    }

    /// Sets the transaction name.
    ///
    /// Transactions represent units of work you want to monitor for performance,
    /// such as web requests, database queries, or background jobs. The transaction name
    /// helps identify and group related performance data in Sentry.
    ///
    /// This is primarily used for performance monitoring. If you're using Sentry's
    /// performance monitoring features, set this to a meaningful name that describes
    /// the operation being performed.
    ///
    /// Pass `None` to clear the transaction name.
    ///
    /// # Examples
    /// ```
    /// scope.set_transaction(Some("GET /api/users"));
    /// scope.set_transaction(Some("process_payment"));
    /// ```
    pub fn set_transaction(&mut self, transaction: Option<&str>) {
        let _transaction = transaction;
        minimal_unreachable!();
    }

    /// Sets the user for the current scope.
    ///
    /// User information helps identify which user was affected by an error or event.
    /// This information appears in the Sentry interface and can be used for filtering
    /// and searching events. The user context is particularly useful for understanding
    /// the impact of issues and debugging user-specific problems.
    ///
    /// Pass `None` to clear the user information.
    ///
    /// # Examples
    /// ```
    /// use sentry_core::protocol::User;
    /// 
    /// let user = User {
    ///     id: Some("12345".into()),
    ///     email: Some("user@example.com".into()),
    ///     username: Some("johndoe".into()),
    ///     ..Default::default()
    /// };
    /// scope.set_user(Some(user));
    /// ```
    pub fn set_user(&mut self, user: Option<User>) {
        let _user = user;
        minimal_unreachable!();
    }

    /// Sets a tag to a specific value.
    ///
    /// Tags are key-value pairs that help you search, filter, and categorize events in Sentry.
    /// They should be used for high-cardinality data that you want to search by.
    /// Tags are always stored as strings and are indexed, making them efficient for filtering.
    ///
    /// Good examples of tags: environment, server_name, user_type, browser, os.
    /// Avoid using tags for high-cardinality data like user IDs or timestamps.
    ///
    /// # Examples
    /// ```
    /// scope.set_tag("environment", "production");
    /// scope.set_tag("user_type", "premium");
    /// scope.set_tag("browser", "chrome");
    /// ```
    pub fn set_tag<V: ToString>(&mut self, key: &str, value: V) {
        let _key = key;
        let _value = value;
        minimal_unreachable!();
    }

    /// Removes a tag.
    pub fn remove_tag(&mut self, key: &str) {
        let _key = key;
        minimal_unreachable!();
    }

    /// Sets a context for a key.
    ///
    /// Contexts provide structured information about the environment in which an event occurred.
    /// Unlike tags, contexts can hold rich, structured data and are designed for providing
    /// detailed debugging information. Common contexts include device info, OS info, runtime info,
    /// and custom application contexts.
    ///
    /// Contexts appear in the Sentry interface as expandable sections with detailed information.
    ///
    /// # Examples
    /// ```
    /// use sentry_core::protocol::{Context, RuntimeContext};
    /// 
    /// // Set a custom context with structured data
    /// scope.set_context("custom", Context::Other(
    ///     [("key".to_string(), "value".into())].into()
    /// ));
    /// 
    /// // Set a runtime context
    /// scope.set_context("runtime", RuntimeContext {
    ///     name: Some("node".to_string()),
    ///     version: Some("18.0.0".to_string()),
    ///     ..Default::default()
    /// });
    /// ```
    pub fn set_context<C: Into<Context>>(&mut self, key: &str, value: C) {
        let _key = key;
        let _value = value;
        minimal_unreachable!();
    }

    /// Removes a context for a key.
    pub fn remove_context(&mut self, key: &str) {
        let _key = key;
        minimal_unreachable!();
    }

    /// Sets extra information to a specific value.
    ///
    /// Extra data provides additional arbitrary information that doesn't fit into other
    /// structured fields. Unlike tags (which are strings only), extra data can contain
    /// any JSON-serializable value including objects, arrays, and nested structures.
    ///
    /// Extra data appears in the "Additional Data" section in the Sentry interface.
    /// Use this for debugging information, configuration values, or any other data
    /// that might be helpful when investigating an issue.
    ///
    /// # Examples
    /// ```
    /// use sentry_core::protocol::Value;
    /// 
    /// scope.set_extra("request_id", "abc123".into());
    /// scope.set_extra("user_settings", Value::Object([
    ///     ("theme".to_string(), "dark".into()),
    ///     ("notifications".to_string(), true.into()),
    /// ].into()));
    /// scope.set_extra("response_time_ms", 150.into());
    /// ```
    pub fn set_extra(&mut self, key: &str, value: Value) {
        let _key = key;
        let _value = value;
        minimal_unreachable!();
    }

    /// Removes a extra.
    pub fn remove_extra(&mut self, key: &str) {
        let _key = key;
        minimal_unreachable!();
    }

    /// Add an event processor to the scope.
    pub fn add_event_processor<F>(&mut self, f: F)
    where
        F: Fn(Event<'static>) -> Option<Event<'static>> + Send + Sync + 'static,
    {
        let _f = f;
        minimal_unreachable!();
    }

    /// Applies the contained scoped data to fill an event.
    pub fn apply_to_event(&self, event: Event<'static>) -> Option<Event<'static>> {
        let _event = event;
        minimal_unreachable!();
    }

    /// Applies the contained scoped data to fill a log.
    #[cfg(feature = "logs")]
    pub fn apply_to_log(&self, log: &mut Log) {
        let _log = log;
        minimal_unreachable!();
    }

    /// Set the given [`TransactionOrSpan`] as the active span for this scope.
    pub fn set_span(&mut self, span: Option<TransactionOrSpan>) {
        let _ = span;
        minimal_unreachable!();
    }

    /// Returns the currently active span.
    pub fn get_span(&self) -> Option<TransactionOrSpan> {
        None
    }
}

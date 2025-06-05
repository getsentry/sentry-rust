//! Macros for Sentry [structured logging](https://docs.sentry.io/product/explore/logs/).

// Helper macro to capture a log at the given level. Should not be used directly.
#[doc(hidden)]
#[macro_export]
macro_rules! logger_log {
    // Simple message
    ($level:expr, $msg:literal) => {{
        let log = $crate::protocol::Log {
            level: $level,
            body: $msg.to_owned(),
            trace_id: None,
            timestamp: ::std::time::SystemTime::now(),
            severity_number: None,
            attributes: $crate::protocol::Map::new(),
        };
        $crate::Hub::current().capture_log(log)
    }};

    // Message with format string and args
    ($level:expr, $fmt:literal, $($arg:expr),+) => {{
        let mut attributes = $crate::protocol::Map::new();

        attributes.insert(
            "sentry.message.template".to_owned(),
            $crate::protocol::LogAttribute($crate::protocol::Value::from($fmt))
        );
        let mut i = 0;
        $(
            attributes.insert(
                format!("sentry.message.parameter.{}", i),
                $crate::protocol::LogAttribute($crate::protocol::Value::from($arg))
            );
            #[allow(unused_assignments)]
            i += 1;
        )*

        let log = $crate::protocol::Log {
            level: $level,
            body: format!($fmt, $($arg),*),
            trace_id: None,
            timestamp: ::std::time::SystemTime::now(),
            severity_number: None,
            attributes,
        };
        $crate::Hub::current().capture_log(log)
    }};

    // Attributes entrypoint
    ($level:expr, $($rest:tt)+) => {{
        let mut attributes = $crate::protocol::Map::new();
        $crate::logger_log!(@internal attributes, $level, $($rest)+)
    }};

    // Attributes base case: no more attributes, simple message
    (@internal $attrs:ident, $level:expr, $msg:literal) => {{
        let log = $crate::protocol::Log {
            level: $level,
            body: $msg.to_owned(),
            trace_id: None,
            timestamp: ::std::time::SystemTime::now(),
            severity_number: None,
            #[allow(clippy::redundant_field_names)]
            attributes: $attrs,
        };
        $crate::Hub::current().capture_log(log)
    }};

    // Attributes base case: no more attributes, message with format string and args
    (@internal $attrs:ident, $level:expr, $fmt:literal, $($arg:expr),+) => {{
        $attrs.insert(
            "sentry.message.template".to_owned(),
            $crate::protocol::LogAttribute($crate::protocol::Value::from($fmt))
        );
        let mut i = 0;
        $(
            $attrs.insert(
                format!("sentry.message.parameter.{}", i),
                $crate::protocol::LogAttribute($crate::protocol::Value::from($arg))
            );
            #[allow(unused_assignments)]
            i += 1;
        )*

        let log = $crate::protocol::Log {
            level: $level,
            body: format!($fmt, $($arg),*),
            trace_id: None,
            timestamp: ::std::time::SystemTime::now(),
            severity_number: None,
            #[allow(clippy::redundant_field_names)]
            attributes: $attrs,
        };
        $crate::Hub::current().capture_log(log)
    }};

    // Attributes recursive case
    (@internal $attrs:ident, $level:expr, $($key:ident).+ = $value:expr, $($rest:tt)+) => {{
        $attrs.insert(
            stringify!($($key).+).to_owned(),
            $crate::protocol::LogAttribute($crate::protocol::Value::from($value))
        );
        $crate::logger_log!(@internal $attrs, $level, $($rest)+)
    }};
}

/// Captures a log at the trace level, with the given message and attributes.
///
/// To attach attributes to a log, pass them with the `key = value` syntax before the message.
/// The message can be a simple string or a format string with its arguments.
///
/// The supported attribute keys are all valid Rust identifiers with up to 8 dots.
/// Using dots will nest multiple attributes under their common prefix in the UI.
///
/// The supported attribute values are simple types, such as string, numbers, and boolean.
///
/// # Examples
///
/// ```
/// use sentry::logger_trace;
///
/// // Simple message
/// logger_trace!("Hello world");
///
/// // Message with format args
/// logger_trace!("Value is {}", 42);
///
/// // Message with format args and attributes
/// logger_trace!(
///     error_code = 500,
///     user.id = "12345",
///     user.email = "test@test.com",
///     success = false,
///     "Error occurred: {}",
///     "bad input"
/// );
/// ```
#[macro_export]
macro_rules! logger_trace {
    ($($arg:tt)+) => {
        $crate::logger_log!($crate::protocol::LogLevel::Trace, $($arg)+)
    };
}

/// Captures a log at the debug level, with the given message and attributes.
///
/// To attach attributes to a log, pass them with the `key = value` syntax before the message.
/// The message can be a simple string or a format string with its arguments.
///
/// The supported attribute keys are all valid Rust identifiers with up to 8 dots.
/// Using dots will nest multiple attributes under their common prefix in the UI.
///
/// The supported attribute values are simple types, such as string, numbers, and boolean.
///
/// # Examples
///
/// ```
/// use sentry::logger_debug;
///
/// // Simple message
/// logger_debug!("Hello world");
///
/// // Message with format args
/// logger_debug!("Value is {}", 42);
///
/// // Message with format args and attributes
/// logger_debug!(
///     error_code = 500,
///     user.id = "12345",
///     user.email = "test@test.com",
///     success = false,
///     "Error occurred: {}",
///     "bad input"
/// );
/// ```
#[macro_export]
macro_rules! logger_debug {
    ($($arg:tt)+) => {
        $crate::logger_log!($crate::protocol::LogLevel::Debug, $($arg)+)
    };
}

/// Captures a log at the info level, with the given message and attributes.
///
/// To attach attributes to a log, pass them with the `key = value` syntax before the message.
/// The message can be a simple string or a format string with its arguments.
///
/// The supported attribute keys are all valid Rust identifiers with up to 8 dots.
/// Using dots will nest multiple attributes under their common prefix in the UI.
///
/// The supported attribute values are simple types, such as string, numbers, and boolean.
///
/// # Examples
///
/// ```
/// use sentry::logger_info;
///
/// // Simple message
/// logger_info!("Hello world");
///
/// // Message with format args
/// logger_info!("Value is {}", 42);
///
/// // Message with format args and attributes
/// logger_info!(
///     error_code = 500,
///     user.id = "12345",
///     user.email = "test@test.com",
///     success = false,
///     "Error occurred: {}",
///     "bad input"
/// );
/// ```
#[macro_export]
macro_rules! logger_info {
    ($($arg:tt)+) => {
        $crate::logger_log!($crate::protocol::LogLevel::Info, $($arg)+)
    };
}

/// Captures a log at the warn level, with the given message and attributes.
///
/// To attach attributes to a log, pass them with the `key = value` syntax before the message.
/// The message can be a simple string or a format string with its arguments.
///
/// The supported attribute keys are all valid Rust identifiers with up to 8 dots.
/// Using dots will nest multiple attributes under their common prefix in the UI.
///
/// The supported attribute values are simple types, such as string, numbers, and boolean.
///
/// # Examples
///
/// ```
/// use sentry::logger_warn;
///
/// // Simple message
/// logger_warn!("Hello world");
///
/// // Message with format args
/// logger_warn!("Value is {}", 42);
///
/// // Message with format args and attributes
/// logger_warn!(
///     error_code = 500,
///     user.id = "12345",
///     user.email = "test@test.com",
///     success = false,
///     "Error occurred: {}",
///     "bad input"
/// );
/// ```
#[macro_export]
macro_rules! logger_warn {
    ($($arg:tt)+) => {
        $crate::logger_log!($crate::protocol::LogLevel::Warn, $($arg)+)
    };
}

/// Captures a log at the error level, with the given message and attributes.
///
/// To attach attributes to a log, pass them with the `key = value` syntax before the message.
/// The message can be a simple string or a format string with its arguments.
///
/// The supported attribute keys are all valid Rust identifiers with up to 8 dots.
/// Using dots will nest multiple attributes under their common prefix in the UI.
///
/// The supported attribute values are simple types, such as string, numbers, and boolean.
///
/// # Examples
///
/// ```
/// use sentry::logger_error;
///
/// // Simple message
/// logger_error!("Hello world");
///
/// // Message with format args
/// logger_error!("Value is {}", 42);
///
/// // Message with format args and attributes
/// logger_error!(
///     error_code = 500,
///     user.id = "12345",
///     user.email = "test@test.com",
///     success = false,
///     "Error occurred: {}",
///     "bad input"
/// );
/// ```
#[macro_export]
macro_rules! logger_error {
    ($($arg:tt)+) => {
        $crate::logger_log!($crate::protocol::LogLevel::Error, $($arg)+)
    };
}

/// Captures a log at the fatal level, with the given message and attributes.
///
/// To attach attributes to a log, pass them with the `key = value` syntax before the message.
/// The message can be a simple string or a format string with its arguments.
///
/// The supported attribute keys are all valid Rust identifiers with up to 8 dots.
/// Using dots will nest multiple attributes under their common prefix in the UI.
///
/// The supported attribute values are simple types, such as string, numbers, and boolean.
///
/// # Examples
///
/// ```
/// use sentry::logger_fatal;
///
/// // Simple message
/// logger_fatal!("Hello world");
///
/// // Message with format args
/// logger_fatal!("Value is {}", 42);
///
/// // Message with format args and attributes
/// logger_fatal!(
///     error_code = 500,
///     user.id = "12345",
///     user.email = "test@test.com",
///     success = false,
///     "Error occurred: {}",
///     "bad input"
/// );
/// ```
#[macro_export]
macro_rules! logger_fatal {
    ($($arg:tt)+) => {
        $crate::logger_log!($crate::protocol::LogLevel::Fatal, $($arg)+)
    };
}

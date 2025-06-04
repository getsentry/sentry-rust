/// Captures a log at the given level, with the given message and attributes.
/// The attributes are passed as `key = value` arguments before the message, which can be a simple string or a format string with its arguments.
///
/// The supported attribute keys are all valid Rust identifiers with up to 8 dots.
/// Using dots will nest multiple attributes under their common prefix in the UI.
///
/// The supported attribute values are simple types, such as string, numbers, and boolean.
///
/// See also the [`crate::trace!`], [`crate::debug!`], [`crate::info!`], [`crate::warn!`], [`crate::error!`], and [`crate::fatal!`] macros, which call `log!` with the corresponding level.
///
/// # Examples
///
/// ```
/// use sentry::{log, protocol::LogLevel};
///
/// // Simple message
/// log!(LogLevel::Info, "Hello world");
///
/// // Message with format args
/// log!(LogLevel::Debug, "Value is {}", 42);
///
/// // Message with format args and attributes
/// log!(LogLevel::Warn,
///     error_code = 500,
///     user.id = "12345",
///     user.email = "test@test.com",
///     success = false,
///     "Error occurred: {}",
///     "bad input"
/// );
/// ```
#[macro_export]
macro_rules! log {
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
            i += 1;
        )*

        let log = $crate::protocol::Log {
            level: $level,
            body: format!($fmt, $($arg),*),
            trace_id: None,
            timestamp: ::std::time::SystemTime::now(),
            severity_number: None,
            attributes: attributes,
        };
        $crate::Hub::current().capture_log(log)
    }};

    // Attributes entrypoint
    ($level:expr, $($rest:tt)+) => {{
        let mut attributes = $crate::protocol::Map::new();
        $crate::log!(@internal attributes, $level, $($rest)+)
    }};

    // Attributes base case: no more attributes, simple message
    (@internal $attrs:ident, $level:expr, $msg:literal) => {{
        let log = $crate::protocol::Log {
            level: $level,
            body: $msg.to_owned(),
            trace_id: None,
            timestamp: ::std::time::SystemTime::now(),
            severity_number: None,
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
            i += 1;
        )*

        let log = $crate::protocol::Log {
            level: $level,
            body: format!($fmt, $($arg),*),
            trace_id: None,
            timestamp: ::std::time::SystemTime::now(),
            severity_number: None,
            attributes: $attrs,
        };
        $crate::Hub::current().capture_log(log)
    }};

    // Attributes recursive case: key with no dots
    (@internal $attrs:ident, $level:expr, $key:ident = $value:expr, $($rest:tt)+) => {{
        $attrs.insert(
            stringify!($key).to_owned(),
            $crate::protocol::LogAttribute($crate::protocol::Value::from($value))
        );
        $crate::log!(@internal $attrs, $level, $($rest)+)
    }};

    // Attributes recursive case: key with 1 dot
    (@internal $attrs:ident, $level:expr, $key1:ident . $key2:ident = $value:expr, $($rest:tt)+) => {{
        $attrs.insert(
            concat!(stringify!($key1), ".", stringify!($key2)).to_owned(),
            $crate::protocol::LogAttribute($crate::protocol::Value::from($value))
        );
        $crate::log!(@internal $attrs, $level, $($rest)+)
    }};

    // Attributes recursive case: key with 2 dots
    (@internal $attrs:ident, $level:expr, $key1:ident . $key2:ident . $key3:ident = $value:expr, $($rest:tt)+) => {{
        $attrs.insert(
            concat!(stringify!($key1), ".", stringify!($key2), ".", stringify!($key3)).to_owned(),
            $crate::protocol::LogAttribute($crate::protocol::Value::from($value))
        );
        $crate::log!(@internal $attrs, $level, $($rest)+)
    }};

    // Attributes recursive case: key with 3 dots
    (@internal $attrs:ident, $level:expr, $key1:ident . $key2:ident . $key3:ident . $key4:ident = $value:expr, $($rest:tt)+) => {{
        $attrs.insert(
            concat!(stringify!($key1), ".", stringify!($key2), ".", stringify!($key3), ".", stringify!($key4)).to_owned(),
            $crate::protocol::LogAttribute($crate::protocol::Value::from($value))
        );
        $crate::log!(@internal $attrs, $level, $($rest)+)
    }};

    // Attributes recursive case: key with 4 dots
    (@internal $attrs:ident, $level:expr, $key1:ident . $key2:ident . $key3:ident . $key4:ident . $key5:ident = $value:expr, $($rest:tt)+) => {{
        $attrs.insert(
            concat!(stringify!($key1), ".", stringify!($key2), ".", stringify!($key3), ".", stringify!($key4), ".", stringify!($key5)).to_owned(),
            $crate::protocol::LogAttribute($crate::protocol::Value::from($value))
        );
        $crate::log!(@internal $attrs, $level, $($rest)+)
    }};

    // Attributes recursive case: key with 5 dots
    (@internal $attrs:ident, $level:expr, $key1:ident . $key2:ident . $key3:ident . $key4:ident . $key5:ident . $key6:ident = $value:expr, $($rest:tt)+) => {{
        $attrs.insert(
            concat!(stringify!($key1), ".", stringify!($key2), ".", stringify!($key3), ".", stringify!($key4), ".", stringify!($key5), ".", stringify!($key6)).to_owned(),
            $crate::protocol::LogAttribute($crate::protocol::Value::from($value))
        );
        $crate::log!(@internal $attrs, $level, $($rest)+)
    }};

    // Attributes recursive case: key with 6 dots
    (@internal $attrs:ident, $level:expr, $key1:ident . $key2:ident . $key3:ident . $key4:ident . $key5:ident . $key6:ident . $key7:ident = $value:expr, $($rest:tt)+) => {{
        $attrs.insert(
            concat!(stringify!($key1), ".", stringify!($key2), ".", stringify!($key3), ".", stringify!($key4), ".", stringify!($key5), ".", stringify!($key6), ".", stringify!($key7)).to_owned(),
            $crate::protocol::LogAttribute($crate::protocol::Value::from($value))
        );
        $crate::log!(@internal $attrs, $level, $($rest)+)
    }};

    // Attributes recursive case: key with 7 dots
    (@internal $attrs:ident, $level:expr, $key1:ident . $key2:ident . $key3:ident . $key4:ident . $key5:ident . $key6:ident . $key7:ident . $key8:ident = $value:expr, $($rest:tt)+) => {{
        $attrs.insert(
            concat!(stringify!($key1), ".", stringify!($key2), ".", stringify!($key3), ".", stringify!($key4), ".", stringify!($key5), ".", stringify!($key6), ".", stringify!($key7), ".", stringify!($key8)).to_owned(),
            $crate::protocol::LogAttribute($crate::protocol::Value::from($value))
        );
        $crate::log!(@internal $attrs, $level, $($rest)+)
    }};

    // Attributes recursive case: key with 8 dots
    (@internal $attrs:ident, $level:expr, $key1:ident . $key2:ident . $key3:ident . $key4:ident . $key5:ident . $key6:ident . $key7:ident . $key8:ident . $key9:ident = $value:expr, $($rest:tt)+) => {{
        $attrs.insert(
            concat!(stringify!($key1), ".", stringify!($key2), ".", stringify!($key3), ".", stringify!($key4), ".", stringify!($key5), ".", stringify!($key6), ".", stringify!($key7), ".", stringify!($key8), ".", stringify!($key9)).to_owned(),
            $crate::protocol::LogAttribute($crate::protocol::Value::from($value))
        );
        $crate::log!(@internal $attrs, $level, $($rest)+)
    }};
}

/// Captures a log at the trace level, with the given message and attributes.
///
/// See the [`log!`] macro for more details.
///
/// # Examples
///
/// ```
/// use sentry::trace;
///
/// // Simple message
/// trace!("Hello world");
///
/// // Message with format args
/// trace!("Value is {}", 42);
///
/// // Message with format args and attributes
/// trace!(
///     error_code = 500,
///     user.id = "12345",
///     user.email = "test@test.com",
///     success = false,
///     "Error occurred: {}",
///     "bad input"
/// );
/// ```
#[macro_export]
macro_rules! trace {
    ($($arg:tt)+) => {
        $crate::log!($crate::protocol::LogLevel::Trace, $($arg)+)
    };
}

/// Captures a log at the debug level, with the given message and attributes.
///
/// See the [`log!`] macro for more details.
///
/// # Examples
///
/// ```
/// use sentry::debug;
///
/// // Simple message
/// debug!("Hello world");
///
/// // Message with format args
/// debug!("Value is {}", 42);
///
/// // Message with format args and attributes
/// debug!(
///     error_code = 500,
///     user.id = "12345",
///     user.email = "test@test.com",
///     success = false,
///     "Error occurred: {}",
///     "bad input"
/// );
/// ```
#[macro_export]
macro_rules! debug {
    ($($arg:tt)+) => {
        $crate::log!($crate::protocol::LogLevel::Debug, $($arg)+)
    };
}

/// Captures a log at the info level, with the given message and attributes.
///
/// See the [`log!`] macro for more details.
///
/// # Examples
///
/// ```
/// use sentry::info;
///
/// // Simple message
/// info!("Hello world");
///
/// // Message with format args
/// info!("Value is {}", 42);
///
/// // Message with format args and attributes
/// info!(
///     error_code = 500,
///     user.id = "12345",
///     user.email = "test@test.com",
///     success = false,
///     "Error occurred: {}",
///     "bad input"
/// );
/// ```
#[macro_export]
macro_rules! info {
    ($($arg:tt)+) => {
        $crate::log!($crate::protocol::LogLevel::Info, $($arg)+)
    };
}

/// Captures a log at the warn level, with the given message and attributes.
///
/// It's possible to attach any number of attributes to the log using the syntax:
/// - `identifier = value` for simple attributes
/// - `identifier.with.dots = value` or `identifier-with-hyphens = value` for structured attributes
///
/// After specifying the attributes, the last parameter(s) consist of the message and optionally format args if the message is a format string.
///
/// # Examples
///
/// ```
/// use sentry::warn;
///
/// // Simple message
/// warn!("Hello world");
///
/// // Message with format args
/// warn!("Value is {}", 42);
///
/// // Message with format args and attributes
/// warn!(
///     error_code = 500,
///     user.id = "12345",
///     user.email = "test@test.com",
///     success = false,
///     "Error occurred: {}",
///     "bad input"
/// );
/// ```
#[macro_export]
macro_rules! warn {
    ($($arg:tt)+) => {
        $crate::log!($crate::protocol::LogLevel::Warn, $($arg)+)
    };
}

/// Captures a log at the error level, with the given message and attributes.
///
/// See the [`log!`] macro for more details.
///
/// # Examples
///
/// ```
/// use sentry::error;
///
/// // Simple message
/// error!("Hello world");
///
/// // Message with format args
/// error!("Value is {}", 42);
///
/// // Message with format args and attributes
/// error!(
///     error_code = 500,
///     user.id = "12345",
///     user.email = "test@test.com",
///     success = false,
///     "Error occurred: {}",
///     "bad input"
/// );
/// ```
#[macro_export]
macro_rules! error {
    ($($arg:tt)+) => {
        $crate::log!($crate::protocol::LogLevel::Error, $($arg)+)
    };
}

/// Captures a log at the fatal level, with the given message and attributes.
///
/// See the [`log!`] macro for more details.
///
/// # Examples
///
/// ```
/// use sentry::fatal;
///
/// // Simple message
/// fatal!("Hello world");
///
/// // Message with format args
/// fatal!("Value is {}", 42);
///
/// // Message with format args and attributes
/// fatal!(
///     error_code = 500,
///     user.id = "12345",
///     user.email = "test@test.com",
///     success = false,
///     "Error occurred: {}",
///     "bad input"
/// );
/// ```
#[macro_export]
macro_rules! fatal {
    ($($arg:tt)+) => {
        $crate::log!($crate::protocol::LogLevel::Fatal, $($arg)+)
    };
}

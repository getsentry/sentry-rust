use std::collections::btree_map::BTreeMap;
use std::fmt;
use std::str;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use serde_json;
use thiserror::Error;

use crate::utils::ts_seconds_float;

pub use super::trace::*;

/// A Log Envelope Item
///
/// See: https://develop.sentry.dev/sdk/data-model/envelope-items/#log
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LogEnvelopeItem {
    /// A list of logs
    pub items: Vec<Log>,
}

/// Represents a log that can be sent to Sentry.
///
/// See: https://develop.sentry.dev/sdk/telemetry/logs/
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Log {
    /// The severity level of the log.
    ///
    /// Allowed values are, from highest to lowest:
    /// `fatal`, `error`, `warn`, `info`, `debug`, `trace`.
    ///
    /// The log level changes how logs are filtered and displayed.
    /// Fatal level logs are emphasized more than trace level logs.
    #[serde(default)]
    pub level: LogLevel,

    /// The log body.
    #[serde(default)]
    pub body: String,

    /// Timestamp in seconds (epoch time) indicating when the  log occurred.
    #[serde(default = "SystemTime::now", with = "ts_seconds_float")]
    pub timestamp: SystemTime,

    /// Determines which trace the log belongs to.
    #[serde(default)]
    pub trace_id: TraceId,

    /// The severity number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity_number: Option<LogSeverityNumber>,

    /// Arbitrary structured data that stores information about the log.
    /// [`LogAttributes`]
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: LogAttributes,
}

impl Log {
    /// Sets an attribute on the log with the given key and value.
    /// The value can be any type that can be converted to a [`LogAttributeValue`].
    /// The type of the value will be automatically determined and stored.
    ///
    /// # Examples
    /// ```
    /// use sentry_types::protocol::Log;
    /// use serde_json::json;
    ///
    /// let mut log = Log::default();
    /// log.set_attribute("user_id", 123);
    /// log.set_attribute("message", "test message");
    /// log.set_attribute("is_error", true);
    /// log.set_attribute("metadata", json!({"key": "value"}));
    /// ```
    pub fn set_attribute<K, V>(&mut self, key: K, value: V)
    where
        K: Into<String>,
        V: Into<LogAttributeValue>,
    {
        self.attributes.insert(key.into(), LogAttribute::new(value));
    }
}

/// An error used when parsing `LogLevel`.
#[derive(Debug, Error)]
#[error("invalid log level")]
pub struct ParseLogLevelError;

/// Represents the severity level of a log.
///
/// From highest to lowest:
/// `fatal`, `error`, `warn`, `info`, `debug`, `trace`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub enum LogLevel {
    /// A fine-grained debugging event. Typically disabled in default configurations.
    #[serde(rename = "trace")]
    Trace,
    /// A debugging event.
    #[serde(rename = "debug")]
    Debug,
    /// An informational event. Indicates that an event happened.
    #[default]
    #[serde(rename = "info")]
    Info,
    /// A warning event. Not an error but is likely more important than an informational event.
    #[serde(rename = "warn")]
    Warn,
    /// An error event. Something went wrong.
    #[serde(rename = "error")]
    Error,
    /// A fatal error such as application or system crash.
    #[serde(rename = "fatal")]
    Fatal,
}

impl str::FromStr for LogLevel {
    type Err = ParseLogLevelError;

    fn from_str(string: &str) -> Result<LogLevel, Self::Err> {
        Ok(match string {
            "trace" => LogLevel::Trace,
            "debug" => LogLevel::Debug,
            "info" | "log" => LogLevel::Info,
            "warning" | "warn" => LogLevel::Warn,
            "error" => LogLevel::Error,
            "fatal" => LogLevel::Fatal,
            _ => return Err(ParseLogLevelError),
        })
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            LogLevel::Trace => write!(f, "trace"),
            LogLevel::Debug => write!(f, "debug"),
            LogLevel::Info => write!(f, "info"),
            LogLevel::Warn => write!(f, "warn"),
            LogLevel::Error => write!(f, "error"),
            LogLevel::Fatal => write!(f, "fatal"),
        }
    }
}

/// Represents the severity number of a log according to OpenTelemetry specification.
///
/// The severity number is an integer between 1-24 where:
/// - 1-4: TRACE - Fine-grained debugging events
/// - 5-8: DEBUG - Debugging events
/// - 9-12: INFO - Informational events
/// - 13-16: WARN - Warning events
/// - 17-20: ERROR - Error events
/// - 21-24: FATAL - Fatal errors
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogSeverityNumber {
    /// TRACE severity (1-4)
    Trace(u8),
    /// DEBUG severity (5-8)
    Debug(u8),
    /// INFO severity (9-12)
    Info(u8),
    /// WARN severity (13-16)
    Warn(u8),
    /// ERROR severity (17-20)
    Error(u8),
    /// FATAL severity (21-24)
    Fatal(u8),
}

impl LogSeverityNumber {
    /// Creates a new LogSeverityNumber from [`LogLevel`].
    pub fn default_from_level(value: LogLevel) -> Self {
        match value {
            LogLevel::Trace => Self::Trace(1),
            LogLevel::Debug => Self::Debug(5),
            LogLevel::Info => Self::Info(9),
            LogLevel::Warn => Self::Warn(13),
            LogLevel::Error => Self::Error(17),
            LogLevel::Fatal => Self::Fatal(21),
        }
    }

    /// Creates a new LogSeverityNumber from a raw u64 value.
    /// Returns None if the value is outside the valid range (1-24).
    pub fn from_raw(value: u64) -> Option<Self> {
        if value == 0 || value > 24 {
            return None;
        }

        Some(match value {
            1..=4 => Self::Trace(value as u8),
            5..=8 => Self::Debug(value as u8),
            9..=12 => Self::Info(value as u8),
            13..=16 => Self::Warn(value as u8),
            17..=20 => Self::Error(value as u8),
            21..=24 => Self::Fatal(value as u8),
            _ => unreachable!(),
        })
    }

    /// Returns the raw u64 value of the severity number.
    pub fn to_raw(&self) -> u64 {
        match self {
            Self::Trace(v) => *v as u64,
            Self::Debug(v) => *v as u64,
            Self::Info(v) => *v as u64,
            Self::Warn(v) => *v as u64,
            Self::Error(v) => *v as u64,
            Self::Fatal(v) => *v as u64,
        }
    }

    /// Returns the severity level corresponding to this severity number.
    pub fn to_level(&self) -> LogLevel {
        match self {
            Self::Trace(_) => LogLevel::Trace,
            Self::Debug(_) => LogLevel::Debug,
            Self::Info(_) => LogLevel::Info,
            Self::Warn(_) => LogLevel::Warn,
            Self::Error(_) => LogLevel::Error,
            Self::Fatal(_) => LogLevel::Fatal,
        }
    }
}

impl Serialize for LogSeverityNumber {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u64(self.to_raw())
    }
}

impl<'de> Deserialize<'de> for LogSeverityNumber {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = u64::deserialize(deserializer)?;
        LogSeverityNumber::from_raw(value)
            .ok_or_else(|| serde::de::Error::custom(format!("invalid severity number: {}", value)))
    }
}

/// Type of a log attributes, maps to OTEL AnyValue
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum LogAttributeType {
    /// A string
    #[serde(rename = "string")]
    String,
    /// An i64 number
    #[serde(rename = "integer")]
    Integer,
    #[serde(rename = "double")]
    /// A f64 number
    Double,
    /// A boolean
    #[serde(rename = "boolean")]
    Boolean,
}

/// Arbitrary structured data that stores information about the log.
pub type LogAttributes = BTreeMap<String, LogAttribute>;

/// The Value of [`LogAttributes`]. Extended from [`serde_json::Value`].
#[derive(Debug, Clone, PartialEq)]
pub struct LogAttributeValue(pub serde_json::Value);

impl Serialize for LogAttributeValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("LogAttributeValue", 2)?;

        match &self.0 {
            serde_json::Value::String(s) => {
                state.serialize_field("value", s)?;
                state.serialize_field("type", "string")?;
            }
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    state.serialize_field("value", &i)?;
                    state.serialize_field("type", "integer")?;
                } else if let Some(f) = n.as_f64() {
                    state.serialize_field("value", &f)?;
                    state.serialize_field("type", "double")?;
                } else {
                    // Convert any other number to string
                    state.serialize_field("value", &n.to_string())?;
                    state.serialize_field("type", "string")?;
                }
            }
            serde_json::Value::Bool(b) => {
                state.serialize_field("value", b)?;
                state.serialize_field("type", "boolean")?;
            }
            // For any other type, convert to string
            _ => {
                state.serialize_field("value", &self.0.to_string())?;
                state.serialize_field("type", "string")?;
            }
        }

        state.end()
    }
}

impl<'de> Deserialize<'de> for LogAttributeValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        enum Field {
            Value,
            Type,
        }

        struct LogAttributeValueVisitor;

        impl<'de> serde::de::Visitor<'de> for LogAttributeValueVisitor {
            type Value = LogAttributeValue;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct LogAttributeValue")
            }

            fn visit_map<V>(self, mut map: V) -> Result<LogAttributeValue, V::Error>
            where
                V: serde::de::MapAccess<'de>,
            {
                let mut value: Option<serde_json::Value> = None;
                let mut r#type: Option<String> = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Value => {
                            if value.is_some() {
                                return Err(serde::de::Error::duplicate_field("value"));
                            }
                            value = Some(map.next_value()?);
                        }
                        Field::Type => {
                            if r#type.is_some() {
                                return Err(serde::de::Error::duplicate_field("type"));
                            }
                            r#type = Some(map.next_value::<String>()?);
                        }
                    }
                }

                let value = value.ok_or_else(|| serde::de::Error::missing_field("value"))?;
                let r#type = r#type.ok_or_else(|| serde::de::Error::missing_field("type"))?;

                let value = match r#type.as_str() {
                    "string" => value,
                    "integer" => {
                        if let Some(i) = value.as_i64() {
                            serde_json::Value::Number(i.into())
                        } else {
                            return Err(serde::de::Error::custom("invalid integer value"));
                        }
                    }
                    "double" => {
                        if let Some(f) = value.as_f64() {
                            serde_json::Value::Number(
                                serde_json::Number::from_f64(f).ok_or_else(|| {
                                    serde::de::Error::custom("invalid double value")
                                })?,
                            )
                        } else {
                            return Err(serde::de::Error::custom("invalid double value"));
                        }
                    }
                    "boolean" => {
                        if let Some(b) = value.as_bool() {
                            serde_json::Value::Bool(b)
                        } else {
                            return Err(serde::de::Error::custom("invalid boolean value"));
                        }
                    }
                    _ => return Err(serde::de::Error::custom("invalid type")),
                };

                Ok(LogAttributeValue(value))
            }
        }

        deserializer.deserialize_struct(
            "LogAttributeValue",
            &["value", "type"],
            LogAttributeValueVisitor,
        )
    }
}

impl From<serde_json::Value> for LogAttributeValue {
    fn from(value: serde_json::Value) -> Self {
        LogAttributeValue(value)
    }
}

impl From<LogAttributeValue> for serde_json::Value {
    fn from(value: LogAttributeValue) -> Self {
        value.0
    }
}

impl From<&str> for LogAttributeValue {
    fn from(value: &str) -> Self {
        LogAttributeValue(serde_json::Value::String(value.to_string()))
    }
}

impl From<String> for LogAttributeValue {
    fn from(value: String) -> Self {
        LogAttributeValue(serde_json::Value::String(value))
    }
}

impl From<f64> for LogAttributeValue {
    fn from(value: f64) -> Self {
        LogAttributeValue(serde_json::Value::Number(
            serde_json::Number::from_f64(value).unwrap_or_else(|| serde_json::Number::from(0)),
        ))
    }
}

impl From<bool> for LogAttributeValue {
    fn from(value: bool) -> Self {
        LogAttributeValue(serde_json::Value::Bool(value))
    }
}

macro_rules! impl_from_integer {
    ($($t:ty)*) => {
        $(
            impl From<$t> for LogAttributeValue {
                fn from(value: $t) -> Self {
                    LogAttributeValue(serde_json::Value::Number(value.into()))
                }
            }
        )*
    };
}

impl_from_integer!(i8 i16 i32 i64 u8 u16 u32 u64);

/// Represents a log attribute
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LogAttribute {
    /// [`LogAttributeValue`]
    pub value: LogAttributeValue,
    /// [`LogAttributeType`]
    pub r#type: LogAttributeType,
}

impl LogAttribute {
    /// Creates a new [`LogAttribute`] with the given value.
    /// The type of the attribute will be automatically determined based on the value.
    /// The type will be one of [`LogAttributeType`] variants.
    ///
    /// # Examples
    /// ```
    /// use sentry_types::protocol::v7::{LogAttribute, LogAttributeType};
    ///
    /// let attr = LogAttribute::new(42);
    /// assert_eq!(attr.r#type, LogAttributeType::Integer);
    ///
    /// let attr = LogAttribute::new("test");
    /// assert_eq!(attr.r#type, LogAttributeType::String);
    /// ```
    pub fn new(value: impl Into<LogAttributeValue>) -> Self {
        let value = value.into();
        let r#type = match &value.0 {
            serde_json::Value::String(_) => LogAttributeType::String,
            serde_json::Value::Number(n) => {
                if let Some(_i) = n.as_i64() {
                    LogAttributeType::Integer
                } else if let Some(_f) = n.as_f64() {
                    LogAttributeType::Double
                } else {
                    // For now, convert any other number types to double
                    // This will be updated when u64 support is added
                    LogAttributeType::Double
                }
            }
            serde_json::Value::Bool(_) => LogAttributeType::Boolean,
            serde_json::Value::Null => LogAttributeType::String,
            _ => LogAttributeType::String, // Default to string for other types
        };
        Self { value, r#type }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_log_serialization() {
        let mut log = Log {
            level: LogLevel::Error,
            body: "Test error message".to_string(),
            timestamp: SystemTime::UNIX_EPOCH,
            trace_id: TraceId::default(),
            severity_number: Some(LogSeverityNumber::default_from_level(LogLevel::Error)),
            attributes: BTreeMap::new(),
        };

        log.set_attribute("string_attr", "test string");
        log.set_attribute("int_attr", 42);
        log.set_attribute("float_attr", 3.14);
        log.set_attribute("bool_attr", true);
        log.set_attribute("null_attr", serde_json::Value::Null);

        // Add array attributes
        log.set_attribute("int_array", json!([1, 2, 3, 4, 5]));

        let serialized = serde_json::to_string(&log).unwrap();
        let deserialized: Log = serde_json::from_str(&serialized).unwrap();

        // Verify the deserialized log matches the original
        assert_eq!(deserialized.level, log.level);
        assert_eq!(deserialized.body, log.body);
        assert_eq!(deserialized.severity_number, log.severity_number);
        assert_eq!(deserialized.trace_id, log.trace_id);

        // Verify attributes
        let attrs = &deserialized.attributes;
        assert_eq!(attrs["string_attr"].value.0, "test string");
        assert_eq!(attrs["string_attr"].r#type, LogAttributeType::String);

        assert_eq!(attrs["int_attr"].value.0, 42);
        assert_eq!(attrs["int_attr"].r#type, LogAttributeType::Integer);

        assert_eq!(attrs["float_attr"].value.0, 3.14);
        assert_eq!(attrs["float_attr"].r#type, LogAttributeType::Double);

        assert_eq!(attrs["bool_attr"].value.0, true);
        assert_eq!(attrs["bool_attr"].r#type, LogAttributeType::Boolean);

        assert_eq!(attrs["null_attr"].value.0, "null");
        assert_eq!(attrs["null_attr"].r#type, LogAttributeType::String);

        assert_eq!(attrs["int_array"].value.0, "[1,2,3,4,5]");
        assert_eq!(attrs["int_array"].r#type, LogAttributeType::String);
    }
}

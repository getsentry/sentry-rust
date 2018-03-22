//! The current latest sentry protocol version.
use std::collections::HashMap;
use std::net::IpAddr;

use chrono;
use chrono::{DateTime, Utc};
use url_serde;
use url::Url;
use serde::de::{Deserialize, Deserializer, Error as DeError};
use serde::ser::{Error as SerError, SerializeMap, Serializer};
use serde_json::{from_value, to_value, Value};

/// Represents a log entry message.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct LogEntry {
    pub message: String,
    #[serde(skip_serializing_if = "Vec::is_empty")] pub params: Vec<Value>,
}

/// Represents a frame.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct Frame {
    pub filename: String,
    pub abs_path: Option<String>,
    pub function: String,
    pub lineno: Option<u32>,
    pub context_line: Option<String>,
    pub pre_context: Option<Vec<String>>,
    pub post_context: Option<Vec<String>>,
}

/// Represents a stacktrace.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct Stacktrace {
    pub frames: Vec<Frame>,
}

/// Represents a list of exceptions.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct Exception {
    pub values: Vec<SingleException>,
}

/// Represents a single exception
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct SingleException {
    #[serde(rename = "type")] pub ty: String,
    pub value: String,
    pub stacktrace: Option<Stacktrace>,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

impl Default for Level {
    fn default() -> Level {
        Level::Info
    }
}

impl Level {
    /// A quick way to check if the level is `debug`.
    pub fn is_debug(&self) -> bool {
        *self == Level::Debug
    }

    /// A quick way to check if the level is `info`.
    pub fn is_info(&self) -> bool {
        *self == Level::Info
    }

    /// A quick way to check if the level is `warning`.
    pub fn is_warning(&self) -> bool {
        *self == Level::Warning
    }

    /// A quick way to check if the level is `error`.
    pub fn is_error(&self) -> bool {
        *self == Level::Error
    }

    /// A quick way to check if the level is `critical`.
    pub fn is_critical(&self) -> bool {
        *self == Level::Critical
    }
}

/// Represents a single breadcrumb
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct Breadcrumb {
    #[serde(with = "chrono::serde::ts_seconds")] pub timestamp: DateTime<Utc>,
    #[serde(rename = "type")] pub ty: String,
    #[serde(skip_serializing_if = "Option::is_none")] pub category: Option<String>,
    #[serde(skip_serializing_if = "Level::is_info")] pub level: Level,
    #[serde(skip_serializing_if = "Option::is_none")] pub message: Option<String>,
    #[serde(skip_serializing_if = "HashMap::is_empty")] pub data: HashMap<String, Value>,
}

impl Default for Breadcrumb {
    fn default() -> Breadcrumb {
        Breadcrumb {
            timestamp: Utc::now(),
            ty: "default".into(),
            category: None,
            level: Default::default(),
            message: None,
            data: HashMap::new(),
        }
    }
}

/// Represents user info.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
#[serde(default)]
pub struct User {
    pub id: Option<String>,
    pub email: Option<String>,
    pub ip_address: Option<IpAddr>,
    pub username: Option<String>,
    #[serde(flatten)] pub data: HashMap<String, Value>,
}

/// Represents http request data.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
#[serde(default)]
pub struct Request {
    #[serde(with = "url_serde")] pub url: Option<Url>,
    pub method: Option<String>,
    // XXX: this makes absolutely no sense because of unicode
    pub data: Option<String>,
    pub query_string: Option<String>,
    pub cookies: Option<String>,
    #[serde(skip_serializing_if = "HashMap::is_empty")] pub headers: HashMap<String, String>,
    #[serde(skip_serializing_if = "HashMap::is_empty")] pub env: HashMap<String, String>,
    #[serde(flatten)] pub other: HashMap<String, Value>,
}

/// Represents a full event for Sentry.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
#[serde(default)]
pub struct Event {
    #[serde(skip_serializing_if = "Option::is_none")] pub level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub fingerprint: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")] pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub logentry: Option<LogEntry>,
    #[serde(skip_serializing_if = "Option::is_none")] pub platform: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub timestamp: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")] pub server_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub release: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub dist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub environment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub user: Option<User>,
    #[serde(skip_serializing_if = "Option::is_none")] pub request: Option<Request>,
    #[serde(skip_serializing_if = "HashMap::is_empty", serialize_with = "serialize_context",
            deserialize_with = "deserialize_context")]
    pub contexts: HashMap<String, Context>,
    #[serde(skip_serializing_if = "Vec::is_empty")] pub breadcrumbs: Vec<Breadcrumb>,
    #[serde(skip_serializing_if = "Option::is_none")] pub exception: Option<Exception>,
    #[serde(skip_serializing_if = "HashMap::is_empty")] pub tags: HashMap<String, String>,
    #[serde(skip_serializing_if = "HashMap::is_empty")] pub extra: HashMap<String, Value>,
    #[serde(flatten)] pub other: HashMap<String, Value>,
}

/// Optional device screen orientation
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Orientation {
    Portrait,
    Landscape,
}

/// General context data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Context {
    pub data: ContextType,
    pub extra: HashMap<String, Value>,
}

/// Typed contextual data
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case", untagged)]
pub enum ContextType {
    Default,
    Device(DeviceContext),
    Os(OsContext),
    Runtime(RuntimeContext),
}

/// Holds device information.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct DeviceContext {
    #[serde(skip_serializing_if = "Option::is_none")] pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub arch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub battery_level: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")] pub orientation: Option<Orientation>,
}

/// Holds operating system information.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct OsContext {
    #[serde(skip_serializing_if = "Option::is_none")] pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub build: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub kernel_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub rooted: Option<bool>,
}

/// Holds information about the runtime.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct RuntimeContext {
    #[serde(skip_serializing_if = "Option::is_none")] pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub version: Option<String>,
}

impl From<ContextType> for Context {
    fn from(data: ContextType) -> Context {
        Context {
            data: data,
            extra: HashMap::new(),
        }
    }
}

impl Default for ContextType {
    fn default() -> ContextType {
        ContextType::Default
    }
}

impl ContextType {
    /// Returns the name of the type for sentry.
    pub fn type_name(&self) -> &str {
        match *self {
            ContextType::Default => "default",
            ContextType::Device(..) => "device",
            ContextType::Os(..) => "os",
            ContextType::Runtime(..) => "runtime",
        }
    }
}

fn deserialize_context<'de, D>(deserializer: D) -> Result<HashMap<String, Context>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = <HashMap<String, Value>>::deserialize(deserializer)?;
    let mut rv = HashMap::new();

    #[derive(Deserialize)]
    pub struct Helper<T> {
        #[serde(flatten)] data: T,
        #[serde(flatten)] extra: HashMap<String, Value>,
    }

    for (key, mut raw_context) in raw {
        let (ty, mut data) = match raw_context {
            Value::Object(mut map) => {
                let has_type = if let Some(&Value::String(..)) = map.get("type") {
                    true
                } else {
                    false
                };
                let ty = if has_type {
                    map.remove("type")
                        .and_then(|x| x.as_str().map(|x| x.to_string()))
                        .unwrap()
                } else {
                    key.to_string()
                };
                (ty, Value::Object(map))
            }
            _ => continue,
        };

        macro_rules! convert_context {
            ($enum:path, $ty:ident) => {{
                let helper = from_value::<Helper<$ty>>(data)
                    .map_err(D::Error::custom)?;
                ($enum(helper.data), helper.extra)
            }}
        }

        let (data, extra) = match ty.as_str() {
            "device" => convert_context!(ContextType::Device, DeviceContext),
            "os" => convert_context!(ContextType::Os, OsContext),
            "runtime" => convert_context!(ContextType::Runtime, RuntimeContext),
            _ => (
                ContextType::Default,
                from_value(data).map_err(D::Error::custom)?,
            ),
        };
        rv.insert(key, Context { data, extra });
    }

    Ok(rv)
}

fn serialize_context<S>(value: &HashMap<String, Context>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut map = try!(serializer.serialize_map(Some(value.len())));

    for (key, value) in value {
        let mut c = match to_value(&value.data).map_err(S::Error::custom)? {
            Value::Object(map) => map,
            _ => unreachable!(),
        };
        c.insert("type".into(), value.data.type_name().into());
        c.extend(
            value
                .extra
                .iter()
                .map(|(key, value)| (key.to_string(), value.clone())),
        );
        try!(map.serialize_entry(key, &c));
    }

    map.end()
}

//! The current latest sentry protocol version.
use std::fmt;
use std::collections::HashMap;
use std::net::IpAddr;

use chrono;
use chrono::{DateTime, Utc};
use url_serde;
use url::Url;
use uuid::Uuid;
use serde::de::{Deserialize, Deserializer, Error as DeError};
use serde::ser::{Error as SerError, Serialize, SerializeMap, Serializer};
use serde_json::{from_value, to_value, Value};

/// Represents a log entry message.
///
/// A log message is similar to the `message` attribute on the event itself but
/// can additionally hold optional parameters.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct LogEntry {
    /// The log message with parameters replaced by `%s`
    pub message: String,
    /// Positional parameters to be inserted into the log entry.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<Value>,
}

/// Represents a frame.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct Frame {
    /// The name of the function is known.
    ///
    /// Note that this might include the name of a class as well if that makes
    /// sense for the language.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function: Option<String>,
    /// The potentially mangled name of the symbol as it appears in an executable.
    ///
    /// This is different from a function name by generally being the mangled
    /// name that appears natively in the binary.  This is relevant for languages
    /// like Swift, C++ or Rust.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    /// The name of the module the frame is contained in.
    ///
    /// Note that this might also include a class name if that is something the
    /// language natively considers to be part of the stack (for instance in Java).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    /// The name of the package that contains the frame.
    ///
    /// For instance this can be a dylib for native languages, the name of the jar
    /// or .NET assembly.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    /// Location information about where the error originated.
    #[serde(flatten)]
    pub location: FileLocation,
    /// Embedded sourcecode in the frame.
    #[serde(flatten)]
    pub source: EmbeddedSources,
    /// In-app indicator.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_app: Option<bool>,
    /// Optional local variables.
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub vars: HashMap<String, Value>,
    /// Optional instruction information for native languages.
    #[serde(flatten)]
    pub instruction_info: InstructionInfo,
}

/// Represents location information.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct FileLocation {
    /// The filename (basename only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    /// If known the absolute path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub abs_path: Option<String>,
    /// The line number if known.
    #[serde(rename = "lineno", skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    /// The column number if known.
    #[serde(rename = "colno", skip_serializing_if = "Option::is_none")]
    pub column: Option<u64>,
}

/// Represents instruction information.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct InstructionInfo {
    /// If known the location of the image.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_addr: Option<u64>,
    /// If known the location of the instruction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instruction_addr: Option<u64>,
    /// If known the location of symbol.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_addr: Option<u64>,
}

/// Represents template debug info.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct TemplateInfo {
    /// Location information about where the error originated.
    #[serde(flatten)]
    pub location: FileLocation,
    /// Embedded sourcecode in the frame.
    #[serde(flatten)]
    pub source: EmbeddedSources,
}

/// Represents contextual information in a frame.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct EmbeddedSources {
    /// The sources of the lines leading up to the current line.
    #[serde(rename = "pre_context")]
    pub pre_lines: Option<Vec<String>>,
    /// The current line as source.
    #[serde(rename = "context_line")]
    pub current_line: Option<String>,
    /// The sources of the lines after the current line.
    #[serde(rename = "post_context")]
    pub post_lines: Option<Vec<String>>,
}

/// Represents a stacktrace.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct Stacktrace {
    /// The list of frames in the stacktrace.
    pub frames: Vec<Frame>,
    /// Optionally a segment of frames removed (`start`, `end`)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frames_omitted: Option<(u64, u64)>,
}

/// Represents a thread id.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub enum ThreadId {
    Int(i64),
    String(String),
}

impl Default for ThreadId {
    fn default() -> ThreadId {
        ThreadId::Int(0)
    }
}

impl fmt::Display for ThreadId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ThreadId::Int(i) => write!(f, "{}", i),
            ThreadId::String(ref s) => write!(f, "{}", s),
        }
    }
}

/// Represents a single thread.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
#[serde(default)]
pub struct Thread {
    pub id: Option<ThreadId>,
    pub name: Option<String>,
    pub stacktrace: Option<Stacktrace>,
    pub crashed: bool,
    pub current: bool,
}

/// Represents a single exception
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct Exception {
    /// The type of the exception
    #[serde(rename = "type")]
    pub ty: String,
    /// The optional value of the exception
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Optionally the stacktrace.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stacktrace: Option<Stacktrace>,
}

/// Represents the level of severity of an event or breadcrumb
#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    /// Indicates very spammy debug information
    Debug,
    /// Informational messages
    Info,
    /// A warning.
    Warning,
    /// An error.
    Error,
    /// Similar to error but indicates a critical event that usually causes a shutdown.
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
            level: Level::Info,
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

/// Represents debug meta information.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
#[serde(default)]
pub struct SdkInfo {
    /// The internal name of the SDK
    sdk_name: String,
    /// the major version of the SDK as integer or 0
    version_major: u32,
    /// the minor version of the SDK as integer or 0
    version_minior: u32,
    /// the patch version of the SDK as integer or 0
    version_patchlevel: u32,
}

/// Represents a debug image.
#[derive(Debug, Clone, PartialEq)]
pub enum DebugImage {
    Apple(AppleDebugImage),
    Proguard(ProguardDebugImage),
    Unknown(HashMap<String, Value>),
}

impl DebugImage {
    /// Returns the name of the type on sentry.
    pub fn type_name(&self) -> &str {
        match *self {
            DebugImage::Apple(..) => "apple",
            DebugImage::Proguard(..) => "proguard",
            DebugImage::Unknown(ref map) => map.get("type")
                .and_then(|x| x.as_str())
                .unwrap_or("unknown"),
        }
    }
}

/// Represents an apple debug image in the debug meta.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AppleDebugImage {
    pub name: String,
    pub arch: Option<String>,
    pub cpu_type: u32,
    pub cpu_subtype: u32,
    pub image_addr: u64,
    pub image_size: u64,
    pub image_vmaddr: u64,
    pub uuid: Uuid,
}

/// Represents a proguard mapping file reference.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ProguardDebugImage {
    pub uuid: Uuid,
}

/// Represents debug meta information.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
#[serde(default)]
pub struct DebugMeta {
    /// Optional system SDK information.
    #[serde(skip_serializing_if = "Option::is_none")]
    sdk_info: Option<SdkInfo>,
    /// A list of debug information files.
    #[serde(skip_serializing_if = "Option::is_none")]
    images: Option<DebugImage>,
}

/// Represents a repository reference.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
#[serde(default)]
pub struct RepoReference {
    /// The name of the repository as it is registered in Sentry.
    pub name: String,
    /// The optional prefix path to apply to source code when pairing it
    /// up with files in the repository.
    pub prefix: Option<String>,
    /// The optional current revision of the local repository.
    pub revision: Option<String>,
}

/// Represents a full event for Sentry.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct Event {
    /// The level of the event (defaults to error)
    #[serde(skip_serializing_if = "Level::is_error")]
    pub level: Level,
    /// An optional fingerprint configuration to override the default.
    #[serde(skip_serializing_if = "is_default_fingerprint")]
    pub fingerprint: Vec<String>,
    /// A message to be sent with the event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Optionally a log entry that can be used instead of the message for
    /// more complex cases.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logentry: Option<LogEntry>,
    /// A platform identifier for this event.
    #[serde(skip_serializing_if = "is_other")]
    pub platform: String,
    /// The timestamp of when the event was created.
    ///
    /// This can be set to `None` in which case the server will set a timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<DateTime<Utc>>,
    /// Optionally the server (or device) name of this event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,
    /// A release identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release: Option<String>,
    /// Repository references
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub repos: HashMap<String, RepoReference>,
    /// An optional distribution identifer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dist: Option<String>,
    /// An optional environment identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    /// Optionally user data to be sent along.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
    /// Optionally HTTP request data to be sent along.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<Request>,
    /// Optional contexts.
    #[serde(skip_serializing_if = "HashMap::is_empty", serialize_with = "serialize_context",
            deserialize_with = "deserialize_context")]
    pub contexts: HashMap<String, Context>,
    /// Exceptions to be attached (one or multiple if chained).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub breadcrumbs: Vec<Breadcrumb>,
    #[serde(skip_serializing_if = "Vec::is_empty", serialize_with = "serialize_exceptions",
            deserialize_with = "deserialize_exceptions", rename = "exception")]
    pub exceptions: Vec<Exception>,
    /// A single stacktrace (deprecated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stacktrace: Option<Stacktrace>,
    /// Simplified template error location info
    #[serde(skip_serializing_if = "Option::is_none", rename = "template")]
    pub template_info: Option<TemplateInfo>,
    /// A list of threads.
    #[serde(skip_serializing_if = "Vec::is_empty", serialize_with = "serialize_threads",
            deserialize_with = "deserialize_threads")]
    pub threads: Vec<Thread>,
    /// Optional tags to be attached to the event.
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub tags: HashMap<String, String>,
    /// Optional extra information to be sent with the event.
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, Value>,
    /// Debug meta information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug_meta: Option<DebugMeta>,
    /// Additional arbitrary keys for forwards compatibility.
    #[serde(flatten)]
    pub other: HashMap<String, Value>,
    // TODO: repos, sdk, logger, culprit, modules
}

fn is_other(value: &str) -> bool {
    value == "other"
}

fn is_default_fingerprint(vec: &Vec<String>) -> bool {
    vec.len() == 1 && (vec[0] == "{{ default }}" || vec[0] == "{{default}}")
}

impl Default for Event {
    fn default() -> Event {
        Event {
            level: Level::Error,
            fingerprint: vec!["{{ default }}".into()],
            message: None,
            logentry: None,
            platform: "other".into(),
            timestamp: None,
            server_name: None,
            release: None,
            repos: HashMap::new(),
            dist: None,
            environment: None,
            user: None,
            request: None,
            contexts: HashMap::new(),
            breadcrumbs: Vec::new(),
            exceptions: Vec::new(),
            stacktrace: None,
            template_info: None,
            threads: Vec::new(),
            tags: HashMap::new(),
            extra: HashMap::new(),
            debug_meta: None,
            other: HashMap::new(),
        }
    }
}

impl Event {
    /// Creates a new event without timestamp.
    pub fn new() -> Event {
        Default::default()
    }

    /// Creates a new event with the current timestamp.
    pub fn new_with_current_timestamp() -> Event {
        let mut rv = Event::new();
        rv.timestamp = Some(Utc::now());
        rv
    }
}

/// Optional device screen orientation
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Orientation {
    /// Portrait device orientation.
    Portrait,
    /// Landscaope device orientation.
    Landscape,
}

/// General context data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Context {
    /// Typed context data.
    pub data: ContextType,
    /// Additional keys sent along not known to the context type.
    pub extra: HashMap<String, Value>,
}

/// Typed contextual data
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case", untagged)]
pub enum ContextType {
    /// Arbitrary contextual information
    Default,
    /// Device data.
    Device(DeviceContext),
    /// Operating system data.
    Os(OsContext),
    /// Runtime data.
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
    let mut map = try!(serializer.serialize_map(None));

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

fn deserialize_exceptions<'de, D>(deserializer: D) -> Result<Vec<Exception>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Repr {
        Qualified { values: Vec<Exception> },
        Unqualified(Vec<Exception>),
        Single(Exception),
    }
    Repr::deserialize(deserializer).map(|x| match x {
        Repr::Qualified { values } => values,
        Repr::Unqualified(values) => values,
        Repr::Single(exc) => vec![exc],
    })
}

fn serialize_exceptions<S>(value: &Vec<Exception>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    #[derive(Serialize)]
    struct Helper<'a> {
        values: &'a [Exception],
    }
    Helper { values: &value }.serialize(serializer)
}

fn deserialize_threads<'de, D>(deserializer: D) -> Result<Vec<Thread>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Repr {
        Qualified { values: Vec<Thread> },
        Unqualified(Vec<Thread>),
    }
    Repr::deserialize(deserializer).map(|x| match x {
        Repr::Qualified { values } => values,
        Repr::Unqualified(values) => values,
    })
}

fn serialize_threads<S>(value: &Vec<Thread>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    #[derive(Serialize)]
    struct Helper<'a> {
        values: &'a [Thread],
    }
    Helper { values: &value }.serialize(serializer)
}

impl<'de> Deserialize<'de> for DebugImage {
    fn deserialize<D>(deserializer: D) -> Result<DebugImage, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut map = match Value::deserialize(deserializer)? {
            Value::Object(map) => map,
            _ => return Err(D::Error::custom("expected debug image")),
        };

        Ok(match map.remove("type").as_ref().and_then(|x| x.as_str()) {
            Some("apple") => {
                let img: AppleDebugImage =
                    from_value(Value::Object(map)).map_err(D::Error::custom)?;
                DebugImage::Apple(img)
            }
            Some("proguard") => {
                let img: ProguardDebugImage =
                    from_value(Value::Object(map)).map_err(D::Error::custom)?;
                DebugImage::Proguard(img)
            }
            Some(ty) => {
                let mut img: HashMap<String, Value> = map.into_iter().collect();
                img.insert("type".into(), ty.into());
                DebugImage::Unknown(img)
            }
            None => DebugImage::Unknown(Default::default()),
        })
    }
}

impl Serialize for DebugImage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut c = match to_value(self).map_err(S::Error::custom)? {
            Value::Object(map) => map,
            _ => unreachable!(),
        };
        c.insert("type".into(), self.type_name().into());
        c.serialize(serializer)
    }
}

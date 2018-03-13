use std::collections::HashMap;

use serde_json::Value;

/// Represents a message.
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct Message {
    pub message: String,
    #[serde(skip_serializing_if="Vec::is_empty")]
    pub params: Vec<String>,
}

/// Represents a frame.
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
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
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Stacktrace {
    pub frames: Vec<Frame>,
}

/// Represents a list of exceptions.
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct Exception {
    pub values: Vec<SingleException>,
}

/// Represents a single exception
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct SingleException {
    #[serde(rename="type")]
    pub ty: String,
    pub value: String,
    pub stacktrace: Option<Stacktrace>,
}

/// Represents a single breadcrumb
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Breadcrumb {
    pub timestamp: f64,
    #[serde(rename="type")]
    pub ty: String,
    pub message: String,
    pub category: String,
}

/// Represents a full event for Sentry.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Event {
    pub tags: HashMap<String, String>,
    pub extra: HashMap<String, Value>,
    pub level: String,
    #[serde(skip_serializing_if="Option::is_none")]
    pub fingerprint: Option<Vec<String>>,
    #[serde(skip_serializing_if="Option::is_none", rename="sentry.interfaces.Message")]
    pub message: Option<Message>,
    pub platform: String,
    pub timestamp: f64,
    #[serde(skip_serializing_if="Option::is_none")]
    pub server_name: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub release: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub dist: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub environment: Option<String>,
    #[serde(skip_serializing_if="HashMap::is_empty")]
    pub user: HashMap<String, String>,
    #[serde(skip_serializing_if="HashMap::is_empty")]
    pub contexts: HashMap<String, HashMap<String, String>>,
    #[serde(skip_serializing_if="Vec::is_empty")]
    pub breadcrumbs: Vec<Breadcrumb>,
    pub exception: Option<Exception>,
}

/// Holds a single contextual item.
#[derive(Debug, Default, Clone)]
pub struct Context {
    data: ContextData,
    rest: HashMap<String, Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all="lowercase")]
pub enum Orientation {
    Portrait,
    Landscape,
}

#[derive(Debug, Clone)]
pub enum ContextData {
    Default,
    Device {
        name: Option<String>,
        family: Option<String>,
        model: Option<String>,
        model_id: Option<String>,
        arch: Option<String>,
        battery_level: Option<f32>, 
        orientation: Option<Orientation>,
    },
    Os {
        name: Option<String>,
        version: Option<String>,
        build: Option<String>,
        kernel_version: Option<String>,
        rooted: Option<bool>,
    },
    Runtime {
        name: Option<String>,
        version: Option<String>,
    }
}

impl Default for ContextData {
    fn default() -> ContextData {
        ContextData::Default
    }
}

impl ContextData {
    pub fn get_type(&self) -> &str {
        match *self {
            ContextData::Default => "default",
            ContextData::Device { .. } => "device",
            ContextData::Os { .. } => "os",
            ContextData::Runtime { .. } => "runtime",
        }
    }
}

impl From<ContextData> for Context {
    fn from(data: ContextData) -> Context {
        Context {
            data: data,
            rest: HashMap::new(),
        }
    }
}

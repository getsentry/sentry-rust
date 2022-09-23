use std::{collections::HashMap, time::SystemTime};

use super::v7::{DebugMeta, TraceId};
use crate::utils::ts_rfc3339;

use serde::{Deserialize, Serialize, Serializer};
use uuid::Uuid;

fn serialize_id<S: Serializer>(uuid: &Uuid, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_some(&uuid.as_simple())
}
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
/// Metadata about the transaction associated with the profile
pub struct TransactionMetadata {
    #[serde(serialize_with = "serialize_id")]
    /// Transaction ID
    pub id: Uuid,
    /// Transaction Name
    pub name: String,
    /// Trace ID
    pub trace_id: TraceId,
    /// Transaction start timestamp in nanoseconds relative to the start of the profiler
    pub relative_start_ns: u64,
    /// Transaction end timestamp in nanoseconds relative to the start of the profiler
    pub relative_end_ns: u64,
    /// ID of the thread in which the transaction started
    #[serde(default)]
    pub active_thread_id: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Hash)]
/// Single frame of a Sample
pub struct RustFrame {
    /// Instruction address
    pub instruction_addr: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
/// Single sample of a profile
pub struct Sample {
    /// ID of the relative stack
    pub stack_id: u32,
    /// Thread ID
    pub thread_id: u64,
    /// Timestamp at which this sample was collected relative to the start of the profiler
    pub relative_timestamp_ns: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
/// Thread metadata
pub struct ThreadMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Thread name
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
/// Collected Profile
pub struct Profile {
    /// list of samples in a profile
    pub samples: Vec<Sample>,
    /// List of stacks: each stacks is a vec of indexed frames
    pub stacks: Vec<Vec<u32>>,
    /// List of frames
    pub frames: Vec<RustFrame>,
    /// Thread metadata
    pub thread_metadata: HashMap<String, ThreadMetadata>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
/// Operating System metadata
pub struct OSMetadata {
    /// OS Name
    pub name: String,
    /// OS Version
    pub version: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Build number
    pub build_number: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
/// Runtime metadata
pub struct RuntimeMetadata {
    /// Runtime name (rustc)
    pub name: String,
    /// Runtime version
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
/// Device metadata
pub struct DeviceMetadata {
    /// Architecture
    pub architecture: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
/// Profile format version
pub enum Version {
    #[serde(rename = "1")]
    /// First version
    V1,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
/// Represents a Profile Envelope ItemType
pub struct SampleProfile {
    /// Format version of the SampleProfile
    pub version: Version,

    #[serde(skip_serializing_if = "Option::is_none")]
    /// Debug meta information
    pub debug_meta: Option<DebugMeta>,

    /// Device metadata information
    pub device: DeviceMetadata,
    /// OS metadata information
    pub os: OSMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Runtime metadata information
    pub runtime: Option<RuntimeMetadata>,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    /// Environment
    pub environment: String,
    #[serde(serialize_with = "serialize_id")]
    /// Event ID or Profile ID
    pub event_id: Uuid,
    /// Platform
    pub platform: String,
    /// Collected profile
    pub profile: Profile,
    /// Release
    pub release: String,
    #[serde(with = "ts_rfc3339")]
    /// Timestamp at which the profiler started
    pub timestamp: SystemTime,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// List of transactions associated with this profile
    pub transactions: Vec<TransactionMetadata>,
}

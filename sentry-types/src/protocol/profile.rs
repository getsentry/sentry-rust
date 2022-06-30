use super::v7::{DebugMeta, TraceId};
use serde::{Deserialize, Serialize, Serializer};
use uuid::Uuid;

fn serialize_id<S: Serializer>(uuid: &Uuid, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_some(&uuid.as_simple())
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
/// Represents a Symbol
pub struct RustFrame {
    /// Raw instruction address
    pub instruction_addr: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
/// Represents a Sample
pub struct Sample {
    /// List of symbols
    pub frames: Vec<RustFrame>,
    /// The thread name
    pub thread_name: String,
    /// The thread id
    pub thread_id: u64,
    /// Nanoseconds elapsed between when the profiler started and when this sample was collected
    pub nanos_relative_to_start: u64,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone, PartialEq)]
/// Represents a collected Profile
pub struct SampledProfile {
    /// Collection start time in nanoseconds
    pub start_time_nanos: u64,
    /// Collection start time in seconds
    pub start_time_secs: u64,
    /// Collection duration in nanoseconds
    pub duration_nanos: u64,
    /// List of collected samples
    pub samples: Vec<Sample>,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone, PartialEq)]
/// Represents a Profile Envelope ItemType
pub struct Profile {
    /// Duration in nanoseconds of the Profile
    pub duration_ns: u64,
    /// List of debug images
    pub debug_meta: DebugMeta,
    /// Platform is `rust`
    pub platform: String,
    /// A string describing the architecture of the CPU that is currently in use
    /// <https://doc.rust-lang.org/std/env/consts/constant.ARCH.html>
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub architecture: Option<String>,
    /// The trace ID
    pub trace_id: TraceId,
    /// The name of the transaction this profile belongs to
    pub transaction_name: String,
    #[serde(serialize_with = "serialize_id")]
    /// The ID of the transaction this profile belongs to
    pub transaction_id: Uuid,
    /// The ID of the event
    #[serde(serialize_with = "serialize_id")]
    pub profile_id: Uuid,
    /// Represents the profile collected
    pub sampled_profile: SampledProfile,
    /// OS name
    #[serde(rename = "device_os_name")]
    pub os_name: String,
    #[serde(rename = "device_os_version")]
    /// OS version
    pub os_version: String,
    /// Package version
    pub version_name: String,
    /// Current binary build ID. See <https://docs.rs/build_id/latest/build_id/>
    pub version_code: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct ProfileItemHeader {
    content_type: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    file_name: String,
    #[serde(rename = "type")]
    typez: String,
    length: usize,
}

/*
impl Profile {
    /// Writes the attachment and its headers to the provided `Writer`.
    pub fn to_writer<W>(&self, writer: &mut W) -> std::io::Result<()>
    where
        W: std::io::Write,
    {
        let serialized_profile = serde_json::to_string(self).unwrap();

        writeln!(
            writer,
            "{}",
            serde_json::to_string(&ProfileItemHeader {
                content_type: "application/json".to_string(),
                file_name: format!("{}.trace", self.trace_id),
                typez: "profile".to_string(),
                length: serialized_profile.len(),
            })?
        )?;

        writer.write_all(serialized_profile.as_bytes())?;
        Ok(())
    }
}
*/

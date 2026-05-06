use std::fmt;

use serde::{Deserialize, Serialize};

/// The different types an attachment can have.
#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize, Default)]
pub enum AttachmentType {
    #[serde(rename = "event.attachment")]
    /// (default) A standard attachment without special meaning.
    #[default]
    Attachment,
    /// A minidump file that creates an error event and is symbolicated. The
    /// file should start with the `MDMP` magic bytes.
    #[serde(rename = "event.minidump")]
    Minidump,
    /// An Apple crash report file that creates an error event and is symbolicated.
    #[serde(rename = "event.applecrashreport")]
    AppleCrashReport,
    /// An XML file containing UE4 crash meta data. During event ingestion,
    /// event contexts and extra fields are extracted from this file.
    #[serde(rename = "unreal.context")]
    UnrealContext,
    /// A plain-text log file obtained from UE4 crashes. During event ingestion,
    /// the last logs are extracted into event breadcrumbs.
    #[serde(rename = "unreal.logs")]
    UnrealLogs,
    /// A custom attachment type with an arbitrary string value.
    #[serde(untagged)]
    Custom(String),
}

impl AttachmentType {
    /// Gets the string value Sentry expects for the attachment type.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Attachment => "event.attachment",
            Self::Minidump => "event.minidump",
            Self::AppleCrashReport => "event.applecrashreport",
            Self::UnrealContext => "unreal.context",
            Self::UnrealLogs => "unreal.logs",
            Self::Custom(s) => s,
        }
    }
}

#[derive(Clone, PartialEq, Default)]
/// Represents an attachment item.
pub struct Attachment {
    /// The actual attachment data.
    pub buffer: Vec<u8>,
    /// The filename of the attachment.
    pub filename: String,
    /// The Content Type of the attachment
    pub content_type: Option<String>,
    /// The special type of this attachment.
    pub ty: Option<AttachmentType>,
}

struct AttachmentHeaderType;

impl Serialize for AttachmentHeaderType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        "attachment".serialize(serializer)
    }
}

#[derive(Serialize)]
struct AttachmentHeader<'a> {
    r#type: AttachmentHeaderType,
    length: usize,
    filename: &'a str,
    attachment_type: &'a AttachmentType,
    content_type: &'a str,
}

impl Attachment {
    /// Writes the attachment and its headers to the provided `Writer`.
    pub fn to_writer<W>(&self, writer: &mut W) -> std::io::Result<()>
    where
        W: std::io::Write,
    {
        let attachment_type = match self.ty.as_ref() {
            Some(ty) => ty,
            None => &Default::default(),
        };

        let content_type = self
            .content_type
            .as_deref()
            .unwrap_or("application/octet-stream");
        let header = AttachmentHeader {
            r#type: AttachmentHeaderType,
            length: self.buffer.len(),
            filename: &self.filename,
            attachment_type,
            content_type,
        };

        serde_json::to_writer(&mut *writer, &header)?;
        writeln!(writer)?;

        writer.write_all(&self.buffer)?;
        Ok(())
    }
}

// Implement Debug manually, otherwise users will be sad when they get a dump
// of decimal encoded bytes to their console
impl fmt::Debug for Attachment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Attachment")
            .field("buffer", &self.buffer.len())
            .field("filename", &self.filename)
            .field("content_type", &self.content_type)
            .field("type", &self.ty)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{self, json};

    #[test]
    fn test_attachment_type_deserialize() {
        let result: AttachmentType = serde_json::from_str(r#""event.minidump""#).unwrap();
        assert_eq!(result, AttachmentType::Minidump);

        let result: AttachmentType = serde_json::from_str(r#""my.custom.type""#).unwrap();
        assert_eq!(result, AttachmentType::Custom("my.custom.type".to_string()));
    }

    #[test]
    fn test_attachment_header_escapes_json_strings() {
        let attachment = Attachment {
            buffer: b"payload".to_vec(),
            filename: "file \"name\"\npart.txt".to_string(),
            content_type: Some("text/\"plain\nnext".to_string()),
            ty: Some(AttachmentType::Custom("custom/\"type\nnext".to_string())),
        };

        let mut buf = Vec::new();
        attachment.to_writer(&mut buf).unwrap();

        let mut parts = buf.splitn(2, |&b| b == b'\n');
        let header: serde_json::Value = serde_json::from_slice(parts.next().unwrap()).unwrap();
        let payload = parts.next().unwrap();

        assert_eq!(
            header,
            json!({
                "type": "attachment",
                "length": 7,
                "filename": "file \"name\"\npart.txt",
                "content_type": "text/\"plain\nnext",
                "attachment_type": "custom/\"type\nnext",
            })
        );
        assert_eq!(payload, b"payload");
    }
}

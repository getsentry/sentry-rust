use std::io::Write;

use crate::protocol::Event;
use crate::session::Session;
use crate::types::Uuid;

#[derive(Clone, Debug)]
#[non_exhaustive]
pub(crate) enum EnvelopeItem {
    Event(Event<'static>),
    Session(Session),
    // TODO:
    // * Attachment,
    // etc…
}

impl From<Event<'static>> for EnvelopeItem {
    fn from(event: Event<'static>) -> Self {
        EnvelopeItem::Event(event)
    }
}

impl From<Session> for EnvelopeItem {
    fn from(session: Session) -> Self {
        EnvelopeItem::Session(session)
    }
}

/// A Sentry Envelope.
///
/// An Envelope is the data format that Sentry uses for Ingestion. It can contain
/// multiple Items, some of which are related, such as Events, and Event Attachments.
/// Other Items, such as Sessions are independant.
///
/// See the [documentation on Envelopes](https://develop.sentry.dev/sdk/envelopes/)
/// for more details.
#[derive(Clone, Default, Debug)]
pub struct Envelope {
    event_id: Option<Uuid>,
    items: Vec<EnvelopeItem>,
}

impl Envelope {
    /// Creates a new empty Envelope.
    pub fn new() -> Envelope {
        Default::default()
    }

    pub(crate) fn add(&mut self, item: EnvelopeItem) {
        self.items.push(item);
    }

    /// Returns the Envelopes Uuid, if any.
    pub fn uuid(&self) -> Option<&Uuid> {
        self.event_id.as_ref()
    }

    /// Returns the [`Event`] contained in this Envelope, if any.
    ///
    /// [`Event`]: protocol/struct.Event.html
    pub fn event(&self) -> Option<&Event<'static>> {
        self.items
            .iter()
            .filter_map(|item| match item {
                EnvelopeItem::Event(event) => Some(event),
                _ => None,
            })
            .next()
    }

    /// Serialize the Envelope into the given [`Write`].
    ///
    /// [`Write`]: https://doc.rust-lang.org/std/io/trait.Write.html
    pub fn to_writer<W>(&self, mut writer: W) -> std::io::Result<()>
    where
        W: Write,
    {
        let mut item_buf = Vec::new();

        // write the headers:
        let event_id = self.uuid();
        match event_id {
            Some(uuid) => writeln!(writer, r#"{{"event_id":"{}"}}"#, uuid)?,
            _ => writeln!(writer, "{{}}")?,
        }

        // write each item:
        for item in &self.items {
            // we write them to a temporary buffer first, since we need their length
            match item {
                EnvelopeItem::Event(event) => serde_json::to_writer(&mut item_buf, event)?,
                EnvelopeItem::Session(session) => serde_json::to_writer(&mut item_buf, session)?,
            }
            let item_type = match item {
                EnvelopeItem::Event(_) => "event",
                EnvelopeItem::Session(_) => "session",
            };
            writeln!(
                writer,
                r#"{{"type":"{}","length":{}}}"#,
                item_type,
                item_buf.len()
            )?;
            writer.write_all(&item_buf)?;
            writeln!(writer)?;
            item_buf.clear();
        }

        Ok(())
    }
}

impl From<Event<'static>> for Envelope {
    fn from(event: Event<'static>) -> Self {
        let mut envelope = Self::default();
        envelope.event_id = Some(event.event_id);
        envelope.items.push(event.into());
        envelope
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn to_buf(envelope: Envelope) -> Vec<u8> {
        let mut vec = Vec::new();
        envelope.to_writer(&mut vec).unwrap();
        vec
    }

    #[test]
    fn test_empty() {
        assert_eq!(to_buf(Envelope::new()), b"{}\n");
    }

    #[test]
    fn test_event() {
        let event_id = Uuid::parse_str("22d00b3f-d1b1-4b5d-8d20-49d138cd8a9c").unwrap();
        let timestamp = "2020-07-20T14:51:14.296Z"
            .parse::<crate::types::DateTime<crate::types::Utc>>()
            .unwrap();
        let event = Event {
            event_id,
            timestamp,
            ..Default::default()
        };
        let envelope = event.into();
        assert_eq!(
            to_buf(envelope),
            br#"{"event_id":"22d00b3f-d1b1-4b5d-8d20-49d138cd8a9c"}
{"type":"event","length":74}
{"event_id":"22d00b3fd1b14b5d8d2049d138cd8a9c","timestamp":1595256674.296}
"#
            .as_ref()
        )
    }
}

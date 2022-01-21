use std::io::Write;

use uuid::Uuid;

use super::v7::{Attachment, Event, SessionAggregates, SessionUpdate, Transaction};

/// An Envelope Item.
///
/// See the [documentation on Items](https://develop.sentry.dev/sdk/envelopes/#items)
/// for more details.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[allow(clippy::large_enum_variant)]
pub enum EnvelopeItem {
    /// An Event Item.
    ///
    /// See the [Event Item documentation](https://develop.sentry.dev/sdk/envelopes/#event)
    /// for more details.
    Event(Event<'static>),
    /// A Session Item.
    ///
    /// See the [Session Item documentation](https://develop.sentry.dev/sdk/envelopes/#session)
    /// for more details.
    SessionUpdate(SessionUpdate<'static>),
    /// A Session Aggregates Item.
    ///
    /// See the [Session Aggregates Item documentation](https://develop.sentry.dev/sdk/envelopes/#sessions)
    /// for more details.
    SessionAggregates(SessionAggregates<'static>),
    /// A Transaction Item.
    ///
    /// See the [Transaction Item documentation](https://develop.sentry.dev/sdk/envelopes/#transaction)
    /// for more details.
    Transaction(Transaction<'static>),
    /// An Attachment Item.
    ///
    /// See the [Attachment Item documentation](https://develop.sentry.dev/sdk/envelopes/#attachment)
    /// for more details.
    Attachment(Attachment),
    // TODO:
    // etc…
}

impl From<Event<'static>> for EnvelopeItem {
    fn from(event: Event<'static>) -> Self {
        EnvelopeItem::Event(event)
    }
}

impl From<SessionUpdate<'static>> for EnvelopeItem {
    fn from(session: SessionUpdate<'static>) -> Self {
        EnvelopeItem::SessionUpdate(session)
    }
}

impl From<SessionAggregates<'static>> for EnvelopeItem {
    fn from(aggregates: SessionAggregates<'static>) -> Self {
        EnvelopeItem::SessionAggregates(aggregates)
    }
}

impl From<Transaction<'static>> for EnvelopeItem {
    fn from(transaction: Transaction<'static>) -> Self {
        EnvelopeItem::Transaction(transaction)
    }
}

/// An Iterator over the items of an Envelope.
#[derive(Clone)]
pub struct EnvelopeItemIter<'s> {
    inner: std::slice::Iter<'s, EnvelopeItem>,
}

impl<'s> Iterator for EnvelopeItemIter<'s> {
    type Item = &'s EnvelopeItem;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// A Sentry Envelope.
///
/// An Envelope is the data format that Sentry uses for Ingestion. It can contain
/// multiple Items, some of which are related, such as Events, and Event Attachments.
/// Other Items, such as Sessions are independent.
///
/// See the [documentation on Envelopes](https://develop.sentry.dev/sdk/envelopes/)
/// for more details.
#[derive(Clone, Default, Debug, PartialEq)]
pub struct Envelope {
    event_id: Option<Uuid>,
    items: Vec<EnvelopeItem>,
}

impl Envelope {
    /// Creates a new empty Envelope.
    pub fn new() -> Envelope {
        Default::default()
    }

    /// Add a new Envelope Item.
    pub fn add_item<I>(&mut self, item: I)
    where
        I: Into<EnvelopeItem>,
    {
        let item = item.into();
        if self.event_id.is_none() {
            if let EnvelopeItem::Event(ref event) = item {
                self.event_id = Some(event.event_id);
            } else if let EnvelopeItem::Transaction(ref transaction) = item {
                self.event_id = Some(transaction.event_id);
            }
        }
        self.items.push(item);
    }

    /// Create an [`Iterator`] over all the [`EnvelopeItem`]s.
    pub fn items(&self) -> EnvelopeItemIter {
        EnvelopeItemIter {
            inner: self.items.iter(),
        }
    }

    /// Returns the Envelopes Uuid, if any.
    pub fn uuid(&self) -> Option<&Uuid> {
        self.event_id.as_ref()
    }

    /// Returns the [`Event`] contained in this Envelope, if any.
    ///
    /// [`Event`]: struct.Event.html
    pub fn event(&self) -> Option<&Event<'static>> {
        self.items
            .iter()
            .filter_map(|item| match item {
                EnvelopeItem::Event(event) => Some(event),
                _ => None,
            })
            .next()
    }

    /// Filters the Envelope's [`EnvelopeItem`]s based on a predicate,
    /// and returns a new Envelope containing only the filtered items.
    ///
    /// Retains the [`EnvelopeItem`]s for which the predicate returns `true`.
    /// Additionally, [`EnvelopeItem::Attachment`]s are only kept if the Envelope
    /// contains an [`EnvelopeItem::Event`] or [`EnvelopeItem::Transaction`].
    ///
    /// [`None`] is returned if no items remain in the Envelope after filtering.
    pub fn filter<P>(self, mut predicate: P) -> Option<Self>
    where
        P: FnMut(&EnvelopeItem) -> bool,
    {
        let mut filtered = Envelope::new();
        for item in self.items {
            if predicate(&item) {
                filtered.add_item(item);
            }
        }

        // filter again, removing attachments which do not make any sense without
        // an event/transaction
        if filtered.uuid().is_none() {
            filtered
                .items
                .retain(|item| !matches!(item, EnvelopeItem::Attachment(..)))
        }

        if filtered.items.is_empty() {
            None
        } else {
            Some(filtered)
        }
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
                EnvelopeItem::SessionUpdate(session) => {
                    serde_json::to_writer(&mut item_buf, session)?
                }
                EnvelopeItem::SessionAggregates(aggregates) => {
                    serde_json::to_writer(&mut item_buf, aggregates)?
                }
                EnvelopeItem::Transaction(transaction) => {
                    serde_json::to_writer(&mut item_buf, transaction)?
                }
                EnvelopeItem::Attachment(attachment) => {
                    attachment.to_writer(&mut writer)?;
                    writeln!(writer)?;
                    continue;
                }
            }
            let item_type = match item {
                EnvelopeItem::Event(_) => "event",
                EnvelopeItem::SessionUpdate(_) => "session",
                EnvelopeItem::SessionAggregates(_) => "sessions",
                EnvelopeItem::Transaction(_) => "transaction",
                EnvelopeItem::Attachment(_) => unreachable!(),
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
        envelope.add_item(event);
        envelope
    }
}

impl From<Transaction<'static>> for Envelope {
    fn from(transaction: Transaction<'static>) -> Self {
        let mut envelope = Self::default();
        envelope.add_item(transaction);
        envelope
    }
}

#[cfg(test)]
mod test {
    use std::time::{Duration, SystemTime};

    use time::format_description::well_known::Rfc3339;
    use time::OffsetDateTime;

    use super::*;
    use crate::protocol::v7::{SessionAttributes, SessionStatus, Span};

    fn to_str(envelope: Envelope) -> String {
        let mut vec = Vec::new();
        envelope.to_writer(&mut vec).unwrap();
        String::from_utf8_lossy(&vec).to_string()
    }

    fn timestamp(s: &str) -> SystemTime {
        let dt = OffsetDateTime::parse(s, &Rfc3339).unwrap();
        let secs = dt.unix_timestamp() as u64;
        let nanos = dt.nanosecond();
        let duration = Duration::new(secs, nanos);
        SystemTime::UNIX_EPOCH.checked_add(duration).unwrap()
    }

    #[test]
    fn test_empty() {
        assert_eq!(to_str(Envelope::new()), "{}\n");
    }

    #[test]
    fn test_event() {
        let event_id = Uuid::parse_str("22d00b3f-d1b1-4b5d-8d20-49d138cd8a9c").unwrap();
        let timestamp = timestamp("2020-07-20T14:51:14.296Z");
        let event = Event {
            event_id,
            timestamp,
            ..Default::default()
        };
        let envelope: Envelope = event.into();
        assert_eq!(
            to_str(envelope),
            r#"{"event_id":"22d00b3f-d1b1-4b5d-8d20-49d138cd8a9c"}
{"type":"event","length":74}
{"event_id":"22d00b3fd1b14b5d8d2049d138cd8a9c","timestamp":1595256674.296}
"#
        )
    }

    #[test]
    fn test_session() {
        let session_id = Uuid::parse_str("22d00b3f-d1b1-4b5d-8d20-49d138cd8a9c").unwrap();
        let started = timestamp("2020-07-20T14:51:14.296Z");
        let session = SessionUpdate {
            session_id,
            distinct_id: Some("foo@bar.baz".to_owned()),
            sequence: None,
            timestamp: None,
            started,
            init: true,
            duration: Some(1.234),
            status: SessionStatus::Ok,
            errors: 123,
            attributes: SessionAttributes {
                release: "foo-bar@1.2.3".into(),
                environment: Some("production".into()),
                ip_address: None,
                user_agent: None,
            },
        };
        let mut envelope = Envelope::new();
        envelope.add_item(session);
        assert_eq!(
            to_str(envelope),
            r#"{}
{"type":"session","length":222}
{"sid":"22d00b3f-d1b1-4b5d-8d20-49d138cd8a9c","did":"foo@bar.baz","started":"2020-07-20T14:51:14.296Z","init":true,"duration":1.234,"status":"ok","errors":123,"attrs":{"release":"foo-bar@1.2.3","environment":"production"}}
"#
        )
    }

    #[test]
    fn test_transaction() {
        let event_id = Uuid::parse_str("22d00b3f-d1b1-4b5d-8d20-49d138cd8a9c").unwrap();
        let span_id = "d42cee9fc3e74f5c".parse().unwrap();
        let trace_id = "335e53d614474acc9f89e632b776cc28".parse().unwrap();
        let start_timestamp = timestamp("2020-07-20T14:51:14.296Z");
        let spans = vec![Span {
            span_id,
            trace_id,
            start_timestamp,
            ..Default::default()
        }];
        let transaction = Transaction {
            event_id,
            start_timestamp,
            spans,
            ..Default::default()
        };
        let envelope: Envelope = transaction.into();
        assert_eq!(
            to_str(envelope),
            r#"{"event_id":"22d00b3f-d1b1-4b5d-8d20-49d138cd8a9c"}
{"type":"transaction","length":200}
{"event_id":"22d00b3fd1b14b5d8d2049d138cd8a9c","start_timestamp":1595256674.296,"spans":[{"span_id":"d42cee9fc3e74f5c","trace_id":"335e53d614474acc9f89e632b776cc28","start_timestamp":1595256674.296}]}
"#
        )
    }
}

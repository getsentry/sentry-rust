use std::time::SystemTime;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::random_uuid;

use super::v7::{Context, Event, Level};

/// Represents feedback from a user.
///
/// Convert a `Feedback` into an [`EnvelopeItem`] with [`From`]/[`Into`] to send it to Sentry as a
/// feedback envelope item.
///
/// [`EnvelopeItem`]: super::v7::EnvelopeItem
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Feedback {
    /// The user's contact email, if provided.
    ///
    /// Sentry attempts to populate this from the user context when it is omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact_email: Option<String>,
    /// The user's name, if provided.
    ///
    /// Sentry attempts to populate this from the user context when it is omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The feedback message from the user.
    ///
    /// The Sentry protocol limits this to a maximum of 4096 characters; longer messages are
    /// truncated by Relay.
    pub message: String,
    /// The URL of the webpage the user was on when submitting the feedback, if applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The identifier of a related error event in the same project.
    ///
    /// Links the feedback to that error in the Sentry User Feedback UI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub associated_event_id: Option<String>,
    /// The identifier of a related Session Replay in the same project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_id: Option<String>,
}

impl Feedback {
    /// Creates new feedback from a user's message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            contact_email: None,
            name: None,
            message: message.into(),
            url: None,
            associated_event_id: None,
            replay_id: None,
        }
    }

    /// Associates the feedback with the user's contact email.
    #[must_use]
    pub fn with_contact_email(mut self, contact_email: impl Into<String>) -> Self {
        self.contact_email = Some(contact_email.into());
        self
    }

    /// Associates the feedback with the user's name.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Records the URL of the webpage the user was on when submitting the feedback.
    #[must_use]
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Links the feedback to a related error event in the same project.
    #[must_use]
    pub fn with_associated_event_id(mut self, associated_event_id: impl Into<String>) -> Self {
        self.associated_event_id = Some(associated_event_id.into());
        self
    }

    /// Links the feedback to a related Session Replay in the same project.
    #[must_use]
    pub fn with_replay_id(mut self, replay_id: impl Into<String>) -> Self {
        self.replay_id = Some(replay_id.into());
        self
    }

    pub(crate) fn to_context(&self) -> Context {
        Context::Feedback(Box::new(self.clone()))
    }
}

/// The key under which the feedback [`Context`] is stored on a feedback [`Event`].
const FEEDBACK_CONTEXT_KEY: &str = "feedback";

/// An [`Event`] carrying a `feedback` context, ready to be sent as a feedback envelope item.
///
/// This type guarantees the presence of the `feedback` context, so [`FeedbackEvent::feedback`]
/// always succeeds. Build one from a [`Feedback`] via [`From`]/[`Into`], then convert it into an
/// [`EnvelopeItem`] the same way.
///
/// [`EnvelopeItem`]: super::v7::EnvelopeItem
#[derive(Debug, Clone, PartialEq)]
pub struct FeedbackEvent(Event<'static>);

impl FeedbackEvent {
    /// Returns the [`Feedback`] carried by the event's `feedback` context.
    pub fn feedback(&self) -> &Feedback {
        match self.0.contexts.get(FEEDBACK_CONTEXT_KEY) {
            Some(Context::Feedback(feedback)) => feedback,
            _ => unreachable!("a FeedbackEvent always carries its feedback context"),
        }
    }

    /// Returns the underlying [`Event`].
    pub fn event(&self) -> &Event<'static> {
        &self.0
    }

    /// Returns a mutable reference to the underlying [`Event`], for tests that need a deterministic
    /// event id or timestamp. Gated to test builds so the feedback-context invariant cannot be
    /// broken by production callers.
    #[cfg(test)]
    pub(crate) fn event_mut(&mut self) -> &mut Event<'static> {
        &mut self.0
    }
}

impl From<Feedback> for FeedbackEvent {
    fn from(feedback: Feedback) -> Self {
        // Feedback is identified by the `feedback` envelope item type and the `feedback` context;
        // Sentry derives the event type from the item type, so it is not set on the event itself.
        let mut event = Event {
            event_id: random_uuid(),
            level: Level::Info,
            timestamp: SystemTime::now(),
            ..Default::default()
        };
        event
            .contexts
            .insert(FEEDBACK_CONTEXT_KEY.to_string(), feedback.to_context());
        Self(event)
    }
}

impl Serialize for FeedbackEvent {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for FeedbackEvent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let event = Event::deserialize(deserializer)?;
        // The event and a plain event share the same payload type, so reject a feedback item
        // whose event is missing the feedback context instead of silently accepting it.
        match event.contexts.get(FEEDBACK_CONTEXT_KEY) {
            Some(Context::Feedback(_)) => Ok(Self(event)),
            _ => Err(serde::de::Error::custom(
                "feedback item is missing its feedback context",
            )),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn feedback() -> Feedback {
        Feedback::new("It broke.")
            .with_contact_email("john.doe@example.com")
            .with_name("John Doe")
    }

    #[test]
    fn test_to_context() {
        let Context::Feedback(context) = feedback().to_context() else {
            panic!("invalid context type");
        };
        assert_eq!(context.message, "It broke.");
        assert_eq!(context.name, Some("John Doe".to_string()));
        assert_eq!(
            context.contact_email,
            Some("john.doe@example.com".to_string())
        );
    }

    #[test]
    fn test_feedback_event_from() {
        let feedback_event = FeedbackEvent::from(feedback());
        assert_eq!(feedback_event.event().level, Level::Info);
        assert_eq!(feedback_event.feedback().message, "It broke.");
    }

    #[test]
    fn test_feedback_event_missing_context_is_rejected() {
        let json = serde_json::to_string(&Event::default()).unwrap();
        let err = serde_json::from_str::<FeedbackEvent>(&json).unwrap_err();
        assert!(err.to_string().contains("missing its feedback context"));
    }

    #[test]
    fn test_all_fields_round_trip() {
        let feedback = feedback()
            .with_url("https://example.com/checkout")
            .with_associated_event_id("22d00b3fd1b14b5d8d2049d138cd8a9c")
            .with_replay_id("d1b14b5d8d2049d138cd8a9c22d00b3f");

        let json = serde_json::to_string(&feedback).unwrap();
        let parsed: Feedback = serde_json::from_str(&json).unwrap();

        assert_eq!(feedback, parsed);
        assert_eq!(parsed.url.as_deref(), Some("https://example.com/checkout"));
        assert_eq!(
            parsed.associated_event_id.as_deref(),
            Some("22d00b3fd1b14b5d8d2049d138cd8a9c")
        );
        assert_eq!(
            parsed.replay_id.as_deref(),
            Some("d1b14b5d8d2049d138cd8a9c22d00b3f")
        );
    }
}

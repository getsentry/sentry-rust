//! Release Health Sessions
//!
//! https://develop.sentry.dev/sdk/sessions/

use std::fmt;
use std::{borrow::Cow, sync::Arc, time::Instant};

use crate::protocol::{Event, Level};
use crate::{
    scope::StackLayer,
    types::{DateTime, Utc, Uuid},
};
use sentry_types::protocol::v7::User;

/// Represents the status of a session.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum SessionStatus {
    Ok,
    Crashed,
    #[allow(dead_code)]
    Abnormal,
    Exited,
}

pub enum SessionUpdate {
    NeedsFlushing(Session),
    Unchanged,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    session_id: Uuid,
    status: SessionStatus,
    errors: usize,
    user: Option<Arc<User>>,
    release: Cow<'static, str>,
    environment: Option<Cow<'static, str>>,
    started: Instant,
    started_utc: DateTime<Utc>,
    duration: Option<f64>,
    init: bool,
    dirty: bool,
}

impl Session {
    pub fn from_stack(stack: &StackLayer) -> Option<Self> {
        let options = stack.client.as_ref()?.options();
        Some(Self {
            session_id: Uuid::new_v4(),
            status: SessionStatus::Ok,
            errors: 0,
            user: stack.scope.user.clone(),
            release: options.release.clone()?,
            environment: options.environment.clone(),
            started: Instant::now(),
            started_utc: Utc::now(),
            duration: None,
            init: true,
            dirty: true,
        })
    }

    pub(crate) fn update_from_event(&mut self, event: &Event<'static>) -> SessionUpdate {
        let mut has_error = event.level >= Level::Error;
        let mut is_crash = false;
        for exc in &event.exception.values {
            has_error = true;
            if let Some(mechanism) = &exc.mechanism {
                if matches!(mechanism.handled, Some(false)) {
                    is_crash = true;
                    break;
                }
            }
        }

        if is_crash {
            self.status = SessionStatus::Crashed;
        }
        if has_error {
            self.errors += 1;
            self.dirty = true;
        }

        if self.dirty {
            self.dirty = false;
            let session = self.clone();
            self.init = false;
            SessionUpdate::NeedsFlushing(session)
        } else {
            SessionUpdate::Unchanged
        }
    }

    pub(crate) fn close(&mut self) {
        self.duration = Some(self.started.elapsed().as_secs_f64());
        if self.status == SessionStatus::Ok {
            self.status = SessionStatus::Exited;
        }
    }
}

#[derive(serde::Serialize)]
struct Attrs {
    release: Cow<'static, str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    environment: Option<Cow<'static, str>>,
}

impl serde::Serialize for Session {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut session = serializer.serialize_struct("Session", 8)?;
        session.serialize_field("sid", &self.session_id)?;
        let did = self.user.as_ref().and_then(|user| {
            user.id
                .as_ref()
                .or_else(|| user.email.as_ref())
                .or_else(|| user.username.as_ref())
        });
        if let Some(did) = did {
            session.serialize_field("did", &did)?;
        } else {
            session.skip_field("did")?;
        }

        session.serialize_field(
            "status",
            match self.status {
                SessionStatus::Ok => "ok",
                SessionStatus::Crashed => "crashed",
                SessionStatus::Abnormal => "abnormal",
                SessionStatus::Exited => "exited",
            },
        )?;
        session.serialize_field("errors", &self.errors)?;
        session.serialize_field("started", &self.started_utc)?;

        if let Some(duration) = self.duration {
            session.serialize_field("duration", &duration)?;
        } else {
            session.skip_field("duration")?;
        }
        if self.init {
            session.serialize_field("init", &true)?;
        } else {
            session.skip_field("init")?;
        }

        session.serialize_field(
            "attrs",
            &Attrs {
                release: self.release.clone(),
                environment: self.environment.clone(),
            },
        )?;

        session.end()
    }
}

#[cfg(all(test, feature = "test"))]
mod tests {
    use crate as sentry;
    use crate::test::with_captured_envelopes_options;
    use crate::{ClientOptions, Envelope};

    fn to_buf(envelope: &Envelope) -> Vec<u8> {
        let mut vec = Vec::new();
        envelope.to_writer(&mut vec).unwrap();
        vec
    }
    fn to_str(envelope: &Envelope) -> String {
        String::from_utf8(to_buf(envelope)).unwrap()
    }

    #[test]
    fn test_session_startstop() {
        let envelopes = with_captured_envelopes_options(
            || {
                sentry::start_session();
                std::thread::sleep(std::time::Duration::from_millis(10));
                sentry::end_session();
            },
            ClientOptions {
                release: Some("some-release".into()),
                ..Default::default()
            },
        );
        assert_eq!(envelopes.len(), 1);

        let body = to_str(&envelopes[0]);
        assert!(body.starts_with("{}\n{\"type\":\"session\","));
        assert!(body.contains(r#""attrs":{"release":"some-release"}"#));
        assert!(body.contains(r#""status":"exited","errors":0"#));
        assert!(body.contains(r#""init":true"#));
    }

    #[test]
    fn test_session_error() {
        let envelopes = with_captured_envelopes_options(
            || {
                sentry::start_session();

                let err = "NaN".parse::<usize>().unwrap_err();
                sentry::capture_error(&err);

                sentry::end_session();
            },
            ClientOptions {
                release: Some("some-release".into()),
                ..Default::default()
            },
        );
        assert_eq!(envelopes.len(), 2);

        let body = to_str(&envelopes[0]);
        assert!(body.contains("{\"type\":\"session\","));
        assert!(body.contains(r#""attrs":{"release":"some-release"}"#));
        assert!(body.contains(r#""status":"ok","errors":1"#));
        assert!(body.contains(r#""init":true"#));

        let body = to_str(&envelopes[1]);
        assert!(body.contains("{\"type\":\"session\","));
        assert!(body.contains(r#""status":"exited","errors":1"#));
        assert!(!body.contains(r#""init":true"#));
    }
}

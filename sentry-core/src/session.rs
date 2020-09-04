//! Release Health Sessions
//!
//! https://develop.sentry.dev/sdk/sessions/

use std::borrow::Cow;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::envelope::EnvelopeItem;
use crate::protocol::{Event, Level, User};
use crate::scope::StackLayer;
use crate::types::{DateTime, Utc, Uuid};
use crate::{Client, Envelope};

/// Represents the status of a session.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum SessionStatus {
    Ok,
    Crashed,
    #[allow(dead_code)]
    Abnormal,
    Exited,
}

// TODO: make this a true POD type and move it to `sentry-types`,
// and split out the client, user, and dirty flag into a separate guard struct
// that lives on the scope.
#[derive(Clone, Debug)]
pub struct Session {
    client: Arc<Client>,
    session_id: Uuid,
    status: SessionStatus,
    errors: usize,
    user: Option<Arc<User>>,
    release: Cow<'static, str>,
    environment: Option<Cow<'static, str>>,
    started: Instant,
    started_utc: DateTime<Utc>,
    duration: Option<Duration>,
    init: bool,
    dirty: bool,
}

impl Drop for Session {
    fn drop(&mut self) {
        self.close();
        if let Some(item) = self.create_envelope_item() {
            let mut envelope = Envelope::new();
            envelope.add(item);
            self.client.capture_envelope(envelope);
        }
    }
}

impl Session {
    pub fn from_stack(stack: &StackLayer) -> Option<Self> {
        let client = stack.client.as_ref()?;
        let options = client.options();
        Some(Self {
            client: client.clone(),
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

    pub(crate) fn update_from_event(&mut self, event: &Event<'static>) {
        if self.status != SessionStatus::Ok {
            // a session that has already transitioned to a "terminal" state
            // should not receive any more updates
            return;
        }
        let mut has_error = event.level >= Level::Error;
        let mut is_crash = false;
        for exc in &event.exception.values {
            has_error = true;
            if let Some(mechanism) = &exc.mechanism {
                if let Some(false) = mechanism.handled {
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
    }

    pub(crate) fn close(&mut self) {
        if self.status == SessionStatus::Ok {
            self.duration = Some(self.started.elapsed());
            self.status = SessionStatus::Exited;
            self.dirty = true;
        }
    }

    pub(crate) fn create_envelope_item(&mut self) -> Option<EnvelopeItem> {
        if self.dirty {
            let item = EnvelopeItem::Session(self.clone());
            self.init = false;
            self.dirty = false;
            return Some(item);
        }
        None
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
            session.serialize_field("duration", &duration.as_secs_f64())?;
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
    use crate::Envelope;

    fn to_buf(envelope: &Envelope) -> Vec<u8> {
        let mut vec = Vec::new();
        envelope.to_writer(&mut vec).unwrap();
        vec
    }
    fn to_str(envelope: &Envelope) -> String {
        String::from_utf8(to_buf(envelope)).unwrap()
    }
    fn capture_envelopes<F>(f: F) -> Vec<Envelope>
    where
        F: FnOnce(),
    {
        crate::test::with_captured_envelopes_options(
            f,
            crate::ClientOptions {
                release: Some("some-release".into()),
                ..Default::default()
            },
        )
    }

    #[test]
    fn test_session_startstop() {
        let envelopes = capture_envelopes(|| {
            sentry::start_session();
            std::thread::sleep(std::time::Duration::from_millis(10));
        });
        assert_eq!(envelopes.len(), 1);

        let body = to_str(&envelopes[0]);
        assert!(body.starts_with("{}\n{\"type\":\"session\","));
        assert!(body.contains(r#""attrs":{"release":"some-release"}"#));
        assert!(body.contains(r#""status":"exited","errors":0"#));
        assert!(body.contains(r#""init":true"#));
    }

    #[test]
    fn test_session_error() {
        let envelopes = capture_envelopes(|| {
            sentry::start_session();

            let err = "NaN".parse::<usize>().unwrap_err();
            sentry::capture_error(&err);
        });
        assert_eq!(envelopes.len(), 2);

        let body = to_str(&envelopes[0]);
        assert!(body.contains(r#"{"type":"session","#));
        assert!(body.contains(r#""attrs":{"release":"some-release"}"#));
        assert!(body.contains(r#""status":"ok","errors":1"#));
        assert!(body.contains(r#""init":true"#));

        let body = to_str(&envelopes[1]);
        assert!(body.contains(r#"{"type":"session","#));
        assert!(body.contains(r#""status":"exited","errors":1"#));
        assert!(!body.contains(r#""init":true"#));
    }

    #[test]
    fn test_session_sampled_errors() {
        let mut envelopes = crate::test::with_captured_envelopes_options(
            || {
                sentry::start_session();

                for _ in 0..100 {
                    let err = "NaN".parse::<usize>().unwrap_err();
                    sentry::capture_error(&err);
                }
            },
            crate::ClientOptions {
                release: Some("some-release".into()),
                sample_rate: 0.5,
                ..Default::default()
            },
        );
        assert!(envelopes.len() > 25);
        assert!(envelopes.len() < 75);

        let body = to_str(&envelopes.pop().unwrap());
        assert!(body.contains(r#"{"type":"session","#));
        assert!(body.contains(r#""status":"exited","errors":100"#));
    }

    /// For _user-mode_ sessions, we want to inherit the session for any _new_
    /// Hub that is spawned from the main thread Hub which already has a session
    /// attached
    #[test]
    fn test_inherit_session_from_top() {
        let envelopes = capture_envelopes(|| {
            sentry::start_session();

            let err = "NaN".parse::<usize>().unwrap_err();
            sentry::capture_error(&err);

            // create a new Hub which should have the same session
            let hub = std::sync::Arc::new(sentry::Hub::new_from_top(sentry::Hub::current()));

            sentry::Hub::run(hub, || {
                let err = "NaN".parse::<usize>().unwrap_err();
                sentry::capture_error(&err);

                sentry::with_scope(
                    |_| {},
                    || {
                        let err = "NaN".parse::<usize>().unwrap_err();
                        sentry::capture_error(&err);
                    },
                );
            });
        });

        assert_eq!(envelopes.len(), 4); // 3 errors and one session end

        let body = to_str(&envelopes[3]);
        assert!(body.contains(r#"{"type":"session","#));
        assert!(body.contains(r#""status":"exited","errors":3"#));
        assert!(!body.contains(r#""init":true"#));
    }

    /// We want to forward-inherit sessions as the previous test asserted, but
    /// not *backwards*. So any new session created in a derived Hub and scope
    /// will only get updates from that particular scope.
    #[test]
    fn test_dont_inherit_session_backwards() {
        let envelopes = capture_envelopes(|| {
            let hub = std::sync::Arc::new(sentry::Hub::new_from_top(sentry::Hub::current()));

            sentry::Hub::run(hub, || {
                sentry::with_scope(
                    |_| {},
                    || {
                        sentry::start_session();

                        let err = "NaN".parse::<usize>().unwrap_err();
                        sentry::capture_error(&err);
                    },
                );

                let err = "NaN".parse::<usize>().unwrap_err();
                sentry::capture_error(&err);
            });

            let err = "NaN".parse::<usize>().unwrap_err();
            sentry::capture_error(&err);
        });

        assert_eq!(envelopes.len(), 4); // 3 errors and one session end

        let body = to_str(&envelopes[0]);
        assert!(body.contains(r#"{"type":"session","#));
        assert!(body.contains(r#""attrs":{"release":"some-release"}"#));
        assert!(body.contains(r#""status":"ok","errors":1"#));
        assert!(body.contains(r#""init":true"#));

        let body = to_str(&envelopes[1]);
        assert!(body.starts_with("{}\n{\"type\":\"session\","));
        assert!(body.contains(r#""status":"exited","errors":1"#));
        assert!(!body.contains(r#""init":true"#));

        // the other two events should not have session updates
        let body = to_str(&envelopes[2]);
        assert!(!body.contains(r#"{"type":"session","#));
        let body = to_str(&envelopes[3]);
        assert!(!body.contains(r#"{"type":"session","#));
    }
}

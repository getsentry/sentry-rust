//! Release Health Sessions
//!
//! https://develop.sentry.dev/sdk/sessions/

use std::sync::Arc;
use std::time::Instant;

use crate::protocol::{
    EnvelopeItem, Event, Level, SessionAttributes, SessionStatus, SessionUpdate,
};
use crate::scope::StackLayer;
use crate::types::{Utc, Uuid};
use crate::{Client, Envelope};

#[derive(Clone, Debug)]
pub struct Session {
    client: Arc<Client>,
    session_update: SessionUpdate<'static>,
    started: Instant,
    dirty: bool,
}

impl Drop for Session {
    fn drop(&mut self) {
        self.close();
        if let Some(item) = self.create_envelope_item() {
            let mut envelope = Envelope::new();
            envelope.add_item(item);
            self.client.capture_envelope(envelope);
        }
    }
}

impl Session {
    pub fn from_stack(stack: &StackLayer) -> Option<Self> {
        let client = stack.client.as_ref()?;
        let options = client.options();
        let user = stack.scope.user.as_ref();
        let distinct_id = user
            .and_then(|user| {
                user.id
                    .as_ref()
                    .or_else(|| user.email.as_ref())
                    .or_else(|| user.username.as_ref())
            })
            .cloned();
        Some(Self {
            client: client.clone(),
            session_update: SessionUpdate {
                session_id: Uuid::new_v4(),
                distinct_id,
                sequence: None,
                timestamp: None,
                started: Utc::now(),
                init: true,
                duration: None,
                status: SessionStatus::Ok,
                errors: 0,
                attributes: SessionAttributes {
                    release: options.release.clone()?,
                    environment: options.environment.clone(),
                    ip_address: None,
                    user_agent: None,
                },
            },
            started: Instant::now(),
            dirty: true,
        })
    }

    pub(crate) fn update_from_event(&mut self, event: &Event<'static>) {
        if self.session_update.status != SessionStatus::Ok {
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
            self.session_update.status = SessionStatus::Crashed;
        }
        if has_error {
            self.session_update.errors += 1;
            self.dirty = true;
        }
    }

    pub(crate) fn close(&mut self) {
        if self.session_update.status == SessionStatus::Ok {
            self.session_update.duration = Some(self.started.elapsed().as_secs_f64());
            self.session_update.status = SessionStatus::Exited;
            self.dirty = true;
        }
    }

    pub(crate) fn create_envelope_item(&mut self) -> Option<EnvelopeItem> {
        if self.dirty {
            let item = self.session_update.clone().into();
            self.session_update.init = false;
            self.dirty = false;
            return Some(item);
        }
        None
    }
}

#[cfg(all(test, feature = "test"))]
mod tests {
    use crate as sentry;
    use crate::protocol::{Envelope, EnvelopeItem, SessionStatus};

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

        let mut items = envelopes[0].items();
        if let Some(EnvelopeItem::SessionUpdate(session)) = items.next() {
            assert_eq!(session.status, SessionStatus::Exited);
            assert!(session.duration.unwrap() > 0.01);
            assert_eq!(session.errors, 0);
            assert_eq!(session.attributes.release, "some-release");
            assert_eq!(session.init, true);
        } else {
            panic!("expected session");
        }
        assert_eq!(items.next(), None);
    }

    #[test]
    fn test_session_error() {
        let envelopes = capture_envelopes(|| {
            sentry::start_session();

            let err = "NaN".parse::<usize>().unwrap_err();
            sentry::capture_error(&err);
        });
        assert_eq!(envelopes.len(), 2);

        let mut items = envelopes[0].items();
        assert!(matches!(items.next(), Some(EnvelopeItem::Event(_))));
        if let Some(EnvelopeItem::SessionUpdate(session)) = items.next() {
            assert_eq!(session.status, SessionStatus::Ok);
            assert_eq!(session.errors, 1);
            assert_eq!(session.attributes.release, "some-release");
            assert_eq!(session.init, true);
        } else {
            panic!("expected session");
        }
        assert_eq!(items.next(), None);

        let mut items = envelopes[1].items();
        if let Some(EnvelopeItem::SessionUpdate(session)) = items.next() {
            assert_eq!(session.status, SessionStatus::Exited);
            assert_eq!(session.errors, 1);
            assert_eq!(session.init, false);
        } else {
            panic!("expected session");
        }
        assert_eq!(items.next(), None);
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

        let envelope = envelopes.pop().unwrap();
        let mut items = envelope.items();
        if let Some(EnvelopeItem::SessionUpdate(session)) = items.next() {
            assert_eq!(session.status, SessionStatus::Exited);
            assert_eq!(session.errors, 100);
        } else {
            panic!("expected session");
        }
        assert_eq!(items.next(), None);
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

        let mut items = envelopes[3].items();
        if let Some(EnvelopeItem::SessionUpdate(session)) = items.next() {
            assert_eq!(session.status, SessionStatus::Exited);
            assert_eq!(session.errors, 3);
            assert_eq!(session.init, false);
        } else {
            panic!("expected session");
        }
        assert_eq!(items.next(), None);
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

        let mut items = envelopes[0].items();
        assert!(matches!(items.next(), Some(EnvelopeItem::Event(_))));
        if let Some(EnvelopeItem::SessionUpdate(session)) = items.next() {
            assert_eq!(session.status, SessionStatus::Ok);
            assert_eq!(session.errors, 1);
            assert_eq!(session.init, true);
        } else {
            panic!("expected session");
        }
        assert_eq!(items.next(), None);

        let mut items = envelopes[1].items();
        if let Some(EnvelopeItem::SessionUpdate(session)) = items.next() {
            assert_eq!(session.status, SessionStatus::Exited);
            assert_eq!(session.errors, 1);
            assert_eq!(session.init, false);
        } else {
            panic!("expected session");
        }
        assert_eq!(items.next(), None);

        // the other two events should not have session updates
        let mut items = envelopes[2].items();
        assert!(matches!(items.next(), Some(EnvelopeItem::Event(_))));
        assert_eq!(items.next(), None);

        let mut items = envelopes[3].items();
        assert!(matches!(items.next(), Some(EnvelopeItem::Event(_))));
        assert_eq!(items.next(), None);
    }
}

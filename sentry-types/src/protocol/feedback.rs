use std::{collections::BTreeMap, time::SystemTime};

use serde::{Deserialize, Serialize};

use crate::random_uuid;

use super::v7::{Context, Event, Level};

/// Represents feedback from a user.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct Feedback {
    /// The user's contact email
    pub contact_email: Option<String>,
    /// The user's name
    pub name: Option<String>,
    /// The feedback from the user
    pub message: String,
}

impl Feedback {
    pub(crate) fn to_context(&self) -> Context {
        Context::Feedback(Box::new(self.clone()))
    }

    pub(crate) fn to_new_event(&self) -> Event<'static> {
        let map = {
            let mut map = BTreeMap::new();
            map.insert("feedback".to_string(), self.to_context());
            map
        };
        Event {
            event_id: random_uuid(),
            level: Level::Info,
            timestamp: SystemTime::now(),
            contexts: map,
            ..Default::default()
        }
    }
}

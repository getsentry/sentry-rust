#![cfg(sentry_any_http_transport)]

//! Utilities for working with client reports.

use sentry_core::client_report::{Reason, Recorder};
use sentry_core::protocol::Envelope;

/// Records all of the items in a given envelope as lost for the given reason.
pub(super) fn record_lost_envelope(recorder: &Recorder, envelope: &Envelope, reason: Reason) {
    for item in envelope.items() {
        recorder.record_lost_envelope_item(item, reason);
    }
}

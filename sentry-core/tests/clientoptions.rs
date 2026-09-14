//! Tests for [`ClientOptions`] setters.

use std::num::NonZeroUsize;

use sentry_core::ClientOptions;

#[test]
fn transport_channel_capacity_stores_value() {
    let options = ClientOptions::new().transport_channel_capacity(42);
    assert_eq!(
        options.transport_channel_capacity,
        Some(NonZeroUsize::new(42).unwrap())
    );
}

#[test]
fn transport_channel_capacity_clamps_zero() {
    let options = ClientOptions::new().transport_channel_capacity(0);
    assert_eq!(options.transport_channel_capacity, Some(NonZeroUsize::MIN));
}

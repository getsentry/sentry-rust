//! Crashes with a scope much larger than one scope message.
//!
//! The end to end test runs this to check that the scope arrives whole
//! on every platform.

use sentry_minidump::MinidumpIntegration;

fn main() {
    let minidump = MinidumpIntegration::new()
        .crashes_dir(std::env::temp_dir().join("sentry-minidump-example"));

    let _guard = sentry::init(sentry::ClientOptions::new().add_integration(minidump));

    sentry::configure_scope(|scope| {
        scope.set_extra("large", "x".repeat(32 * 1024).into());
    });

    unsafe { sadness_generator::raise_segfault() };
}

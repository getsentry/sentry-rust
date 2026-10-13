//! Crashes after setting a tag with a deprecated `MinidumpIntegration`
//! method.
//!
//! The end to end test runs this to check that the deprecated methods
//! still reach the crash event.

use sentry_minidump::MinidumpIntegration;

fn main() {
    let minidump = MinidumpIntegration::new()
        .crashes_dir(std::env::temp_dir().join("sentry-minidump-example"));

    let _guard = sentry::init(sentry::ClientOptions::new().add_integration(minidump));

    sentry::with_integration(|minidump: &MinidumpIntegration, _| {
        #[expect(deprecated)]
        minidump.set_tag("deprecated".into(), Some("set_tag".into()));
    });

    unsafe { sadness_generator::raise_segfault() };
}

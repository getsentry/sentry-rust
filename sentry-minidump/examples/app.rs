//! Example of the `sentry-minidump` crate.
//!
//! This is also executed by the end to end test.

use std::time::Duration;

fn main() {
    let minidump = sentry_minidump::MinidumpIntegration::new()
        .crashes_dir(std::env::temp_dir().join("sentry-minidump-example"))
        .process_name("app-crash-reporter")
        .before_capture(|scope, path| {
            eprintln!("minidump captured at {}", path.display());
            scope.set_tag("crash_reporter", "example");
        })
        .flush_timeout(Duration::from_secs(3));

    // Use this to skip logic in the crash reporter process.
    if minidump.is_crash_reporter_process() {
        eprintln!("starting crash reporter process");
    }

    // Everything before `sentry::init` runs in both processes.
    let _guard = sentry::init(
        sentry::ClientOptions::new()
            // Uncomment the line below to set your DSN, or set the SENTRY_DSN env var.
            // .dsn("<your dsn>")
            .add_integration(minidump),
    );
    // Everything after here runs in the app process only.

    sentry::with_integration(|minidump: &sentry_minidump::MinidumpIntegration, _| {
        minidump.set_user(Some(sentry::User {
            username: Some("john_doe".into()),
            email: Some("john@doe.town".into()),
            ..Default::default()
        }));
    });

    unsafe { sadness_generator::raise_segfault() };
}

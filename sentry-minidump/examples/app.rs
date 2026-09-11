use std::time::Duration;

fn main() {
    // Use this to skip logic in the crash reporter process.
    if sentry_minidump::is_crash_reporter_process() {
        eprintln!("starting crash reporter process");
    }

    // Everything before `sentry::init` runs in both processes.
    let _guard = sentry::init((
        "http://abc123@127.0.0.1:8123/12345",
        sentry::ClientOptions::new().add_integration(
            sentry_minidump::MinidumpIntegration::new()
                .crashes_dir(std::env::temp_dir().join("sentry-minidump-example"))
                .process_name("app-crash-reporter")
                .before_capture(|scope, path| {
                    eprintln!("minidump captured at {}", path.display());
                    scope.set_tag("crash_reporter", "example");
                })
                .flush_timeout(Duration::from_secs(10)),
        ),
    ));
    // Everything after here runs in the app process only.

    sentry::with_integration(|minidump: &sentry_minidump::MinidumpIntegration, _| {
        minidump.set_user(Some(sentry::User {
            username: Some("john_doe".into()),
            email: Some("john@doe.town".into()),
            ..Default::default()
        }));
    });

    std::thread::sleep(Duration::from_secs(10));

    unsafe { sadness_generator::raise_segfault() };
}

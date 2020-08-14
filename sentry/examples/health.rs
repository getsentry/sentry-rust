fn main() {
    let _sentry = sentry::init(sentry::ClientOptions {
        // release health requires a session to be set
        release: sentry::release_name!(),
        debug: true,
        ..Default::default()
    });

    let handle = std::thread::spawn(|| {
        // this session will be set to crashed
        sentry::start_session();
        std::thread::sleep(std::time::Duration::from_secs(3));
        panic!("oh no!");
    });

    sentry::start_session();

    sentry::capture_message(
        "anything with a level >= Error will increase the error count",
        sentry::Level::Error,
    );

    // or any error that has an explicit exception attached
    let err = "NaN".parse::<usize>().unwrap_err();
    sentry::capture_error(&err);

    std::thread::sleep(std::time::Duration::from_secs(2));

    // so this session will increase the errors count by 2, but otherwise has
    // a clean exit.
    sentry::end_session();

    handle.join().ok();
}

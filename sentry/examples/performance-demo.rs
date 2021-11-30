use std::thread;
use std::time::Duration;

// cargo run --example performance-demo
fn main() {
    let _sentry = sentry::init(sentry::ClientOptions {
        release: sentry::release_name!(),
        ..Default::default()
    });

    let transaction = sentry::start_transaction();
    let span1 = transaction.start_child();
    thread::sleep(Duration::from_millis(100));

    let span2 = span1.start_child();
    thread::sleep(Duration::from_millis(200));
    span2.finish();

    span1.finish();
    thread::sleep(Duration::from_millis(100));
    transaction.finish();
}

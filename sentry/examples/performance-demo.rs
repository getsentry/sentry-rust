use std::thread;
use std::time::Duration;

// cargo run --example performance-demo
fn main() {
    let _sentry = sentry::init(sentry::ClientOptions {
        release: sentry::release_name!(),
        ..Default::default()
    });

    let transaction = sentry::start_transaction("transaction", "root span");
    let span1 = transaction.start_child("span1");
    thread::sleep(Duration::from_millis(50));

    let header = span1.iter_headers().next().unwrap();
    thread::spawn(move || {
        let headers = [(header.0, header.1.as_str())];
        let transaction = sentry::Transaction::continue_from_headers(
            "background transaction",
            "root span",
            headers,
        );
        thread::sleep(Duration::from_millis(50));

        let span1 = transaction.start_child("span1");
        thread::sleep(Duration::from_millis(200));
        span1.finish();

        transaction.finish();
    });
    thread::sleep(Duration::from_millis(100));

    let span2 = span1.start_child("span2");
    thread::sleep(Duration::from_millis(200));
    span2.finish();

    span1.finish();
    thread::sleep(Duration::from_millis(100));
    transaction.finish();
}

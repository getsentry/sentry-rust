use std::thread;
use std::time::Duration;

// cargo run --example performance-demo
fn main() {
    let _sentry = sentry::init(sentry::ClientOptions {
        release: sentry::release_name!(),
        ..Default::default()
    });

    let transaction =
        sentry::start_transaction(sentry::TransactionContext::new("transaction", "root span"));
    sentry::configure_scope(|scope| scope.set_span(Some(transaction.clone().into())));

    let span1 = transaction.start_child("span1");
    sentry::configure_scope(|scope| scope.set_span(Some(span1.clone().into())));

    thread::sleep(Duration::from_millis(50));

    let headers = match sentry::Hub::current().get_span() {
        Some(span) => vec![span.iter_headers().next().unwrap()],
        None => vec![],
    };
    thread::spawn(move || {
        let transaction =
            sentry::start_transaction(sentry::TransactionContext::continue_from_headers(
                "background transaction",
                "root span",
                headers.iter().map(|(k, v)| (*k, v.as_str())),
            ));
        sentry::configure_scope(|scope| scope.set_span(Some(transaction.clone().into())));

        thread::sleep(Duration::from_millis(50));

        let span1 = transaction.start_child("span1");
        sentry::configure_scope(|scope| scope.set_span(Some(span1.clone().into())));

        thread::sleep(Duration::from_millis(200));

        span1.finish();
        sentry::configure_scope(|scope| scope.set_span(Some(transaction.clone().into())));

        transaction.finish();
        sentry::configure_scope(|scope| scope.set_span(None));
    });
    thread::sleep(Duration::from_millis(100));

    let span2 = span1.start_child("span2");
    sentry::configure_scope(|scope| scope.set_span(Some(span2.clone().into())));

    sentry::capture_message(
        "A message that should have a trace context",
        sentry::Level::Info,
    );
    thread::sleep(Duration::from_millis(200));

    span2.finish();
    sentry::configure_scope(|scope| scope.set_span(Some(span1.clone().into())));

    span1.finish();
    sentry::configure_scope(|scope| scope.set_span(Some(transaction.clone().into())));

    thread::sleep(Duration::from_millis(100));

    transaction.finish();
    sentry::configure_scope(|scope| scope.set_span(None));
}

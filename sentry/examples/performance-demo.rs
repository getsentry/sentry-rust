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

    main_span1();

    thread::sleep(Duration::from_millis(100));

    transaction.finish();
    sentry::configure_scope(|scope| scope.set_span(None));
}

fn main_span1() {
    wrap_in_span("default", "span1", || {
        thread::sleep(Duration::from_millis(50));

        let headers = match sentry::configure_scope(|scope| scope.get_span()) {
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

            thread_span1();

            transaction.finish();
            sentry::configure_scope(|scope| scope.set_span(None));
        });
        thread::sleep(Duration::from_millis(100));

        main_span2()
    });
}

fn thread_span1() {
    wrap_in_span("default", "span1", || {
        thread::sleep(Duration::from_millis(200));
    })
}

fn main_span2() {
    wrap_in_span("default", "span2", || {
        sentry::capture_message(
            "A message that should have a trace context",
            sentry::Level::Info,
        );
        thread::sleep(Duration::from_millis(200));
    })
}

fn wrap_in_span<F, R>(op: &str, description: &str, f: F) -> R
where
    F: FnOnce() -> R,
{
    let parent = sentry::configure_scope(|scope| scope.get_span());
    let span1: sentry::TransactionOrSpan = match &parent {
        Some(parent) => parent.start_child(op, description).into(),
        None => {
            let ctx = sentry::TransactionContext::new(description, op);
            sentry::start_transaction(ctx).into()
        }
    };
    sentry::configure_scope(|scope| scope.set_span(Some(span1.clone())));

    let rv = f();

    span1.finish();
    sentry::configure_scope(|scope| scope.set_span(parent));

    rv
}

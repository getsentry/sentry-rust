use slog::{debug, error, info, warn};

fn main() {
    let drain = slog::Discard;
    // Default options - breadcrumb from info, event from warnings
    let wrapped_drain = sentry::integrations::slog::wrap_drain(drain, Default::default());
    let _sentry = sentry::init((
        "https://a94ae32be2584e0bbd7a4cbb95971fee@sentry.io/1041156",
        sentry::ClientOptions {
            release: sentry::release_name!(),
            ..Default::default()
        },
    ));
    let root = slog::Logger::root(wrapped_drain, slog::o!("test_slog" => 0));

    debug!(root, "This should not appear"; "111" => "222");
    info!(root, "Info breadcrumb"; "222" => 333);
    warn!(root, "Warning event"; "333" => true);
    error!(root, "Error event"; "444" => "555");
}

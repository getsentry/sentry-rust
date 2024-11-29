use sentry::{ClientOptions, Hub};
use sentry_core::test::TestTransport;

use std::sync::Arc;

pub fn init_sentry(traces_sample_rate: f32) -> Arc<TestTransport> {
    use tracing_subscriber::prelude::*;

    let transport = TestTransport::new();
    let options = ClientOptions {
        dsn: Some("https://test@sentry-tracing.com/test".parse().unwrap()),
        transport: Some(Arc::new(transport.clone())),
        sample_rate: 1.0,
        traces_sample_rate,
        ..ClientOptions::default()
    };
    Hub::current().bind_client(Some(Arc::new(options.into())));

    let _ = tracing_subscriber::registry()
        .with(sentry_tracing::layer().enable_span_attributes())
        .try_init();

    transport
}

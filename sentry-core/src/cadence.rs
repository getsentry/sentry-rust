use std::sync::Arc;

use cadence::MetricSink;

use crate::{Client, Hub};

/// A [`cadence`] compatible [`MetricSink`].
///
/// This will ingest all the emitted metrics to Sentry as well as forward them
/// to the inner [`MetricSink`].
#[derive(Debug)]
pub struct SentryMetricSink<S> {
    client: Arc<Client>,
    sink: S,
}

impl<S> SentryMetricSink<S> {
    /// Creates a new [`SentryMetricSink`], wrapping the given [`MetricSink`].
    pub fn try_new(sink: S) -> Result<Self, S> {
        match Hub::current().client() {
            Some(client) => Ok(Self { client, sink }),
            None => Err(sink),
        }
    }
}

impl<S> MetricSink for SentryMetricSink<S>
where
    S: MetricSink,
{
    fn emit(&self, metric: &str) -> std::io::Result<usize> {
        self.client.add_metric(metric);
        self.sink.emit(metric)
    }

    fn flush(&self) -> std::io::Result<()> {
        if self.client.flush(None) {
            self.sink.flush()
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Flushing Client failed",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use cadence::{Counted, Distributed};
    use sentry_types::protocol::latest::EnvelopeItem;

    use crate::test::with_captured_envelopes;

    use super::*;

    #[test]
    fn test_basic_metrics() {
        let envelopes = with_captured_envelopes(|| {
            let sink = SentryMetricSink::try_new(cadence::NopMetricSink).unwrap();

            let client = cadence::StatsdClient::from_sink("sentry.test", sink);
            client.count("some.count", 1).unwrap();
            client.count("some.count", 10).unwrap();
            client
                .count_with_tags("count.with.tags", 1)
                .with_tag("foo", "bar")
                .send();
            client.distribution("some.distr", 1).unwrap();
            client.distribution("some.distr", 2).unwrap();
            client.distribution("some.distr", 3).unwrap();
        });
        assert_eq!(envelopes.len(), 1);

        let mut items = envelopes[0].items();
        let Some(EnvelopeItem::Metrics(metrics)) = items.next() else {
            panic!("expected metrics");
        };
        let metrics = std::str::from_utf8(metrics).unwrap();

        println!("{metrics}");

        assert!(metrics.contains("sentry.test.count.with.tags:1|c|#foo:bar|T"));
        assert!(metrics.contains("sentry.test.some.count:11|c|T"));
        assert!(metrics.contains("sentry.test.some.distr:1:2:3|d|T"));
        assert_eq!(items.next(), None);
    }
}

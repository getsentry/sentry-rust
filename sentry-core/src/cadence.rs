use std::sync::Arc;

use cadence::MetricSink;

use crate::metrics::{Metric, MetricType, MetricValue};
use crate::units::MetricUnit;
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
    fn emit(&self, string: &str) -> std::io::Result<usize> {
        if let Some(metric) = parse_metric(string) {
            self.client.add_metric(metric);
        }

        self.sink.emit(string)
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

fn parse_metric(string: &str) -> Option<Metric> {
    let mut components = string.split('|');

    let (mri_str, value_str) = components.next()?.split_once(':')?;
    let (name, unit) = match mri_str.split_once('@') {
        Some((name, unit_str)) => (name, unit_str.parse().ok()?),
        None => (mri_str, MetricUnit::None),
    };

    let ty = components.next().and_then(|s| s.parse().ok())?;
    let value = match ty {
        MetricType::Counter => MetricValue::Counter(value_str.parse().ok()?),
        MetricType::Distribution => MetricValue::Distribution(value_str.parse().ok()?),
        MetricType::Set => MetricValue::Set(value_str.parse().ok()?),
        MetricType::Gauge => MetricValue::Gauge(value_str.parse().ok()?),
    };

    let mut builder = Metric::build(name.to_owned(), value).with_unit(unit);

    for component in components {
        if let Some('#') = component.chars().next() {
            for pair in component.get(1..)?.split(',') {
                let mut key_value = pair.splitn(2, ':');

                let key = key_value.next()?.to_owned();
                let value = key_value.next().unwrap_or_default().to_owned();

                builder = builder.with_tag(key, value);
            }
        }
    }

    Some(builder.finish())
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

#![cfg(all(feature = "test", feature = "metrics"))]

use std::collections::HashSet;

use anyhow::{Context, Result};

use sentry::protocol::TraceMetricType;
use sentry_core::protocol::{EnvelopeItem, ItemContainer};
use sentry_core::test;
use sentry_core::{ClientOptions, Hub};
use sentry_types::protocol::v7::TraceMetric;

/// Test that metreics are sent when metrics are enabled.
#[test]
fn sent_when_enabled() {
    let options = ClientOptions {
        enable_metrics: true,
        ..Default::default()
    };

    let mut envelopes =
        test::with_captured_envelopes_options(|| capture_test_metric("test"), options);

    assert_eq!(envelopes.len(), 1, "expected exactly one envelope");

    let envelope = envelopes.pop().unwrap();

    let mut items = envelope.into_items();
    let Some(item) = items.next() else {
        panic!("Expected at least one item");
    };

    assert!(items.next().is_none(), "Expected only one item");

    let EnvelopeItem::ItemContainer(ItemContainer::TraceMetrics(mut metrics)) = item else {
        panic!("Envelope item has unexpected structure");
    };

    assert_eq!(metrics.len(), 1, "Expected exactly one metric");

    let metric = metrics.pop().unwrap();
    assert!(matches!(metric, TraceMetric {
        r#type: TraceMetricType::Counter,
        name,
        value: 1.0,
        ..
    } if name == "test"));
}

/// Test that metrics are disabled (not sent) when disabled in the
/// [`ClientOptions`].
#[test]
fn metrics_disabled_by_default() {
    // Metrics are disabled by default.
    let options: ClientOptions = Default::default();

    let envelopes = test::with_captured_envelopes_options(|| capture_test_metric("test"), options);
    assert!(
        envelopes.is_empty(),
        "no envelopes should be captured when metrics disabled"
    )
}

/// Test that no metrics are captured by a no-op call with
/// metrics enabled
#[test]
fn noop_sends_nothing() {
    let options = ClientOptions {
        enable_metrics: true,
        ..Default::default()
    };

    let envelopes = test::with_captured_envelopes_options(|| (), options);

    assert!(envelopes.is_empty(), "no-op should not capture metrics");
}

/// Test that 100 metrics are sent in a single envelope.
#[test]
fn test_metrics_batching_at_limit() {
    let options = ClientOptions {
        enable_metrics: true,
        ..Default::default()
    };

    let envelopes = test::with_captured_envelopes_options(
        || {
            (0..100)
                .map(|i| format!("metric.{i}"))
                .for_each(capture_test_metric);
        },
        options,
    );

    let envelope = envelopes
        .into_only_item()
        .expect("expected exactly one envelope");
    let item = envelope
        .into_items()
        .into_only_item()
        .expect("expected exactly one item");
    let metrics = item
        .into_metrics()
        .expect("the envelope item is not a metrics item");

    assert_eq!(metrics.len(), 100, "expected 100 metrics");

    let metric_names: HashSet<_> = metrics
        .into_iter()
        .inspect(|metric| assert_eq!(metric.value, 1.0, "metric had unexpected value"))
        .inspect(|metric| {
            assert_eq!(
                metric.r#type,
                TraceMetricType::Counter,
                "metric had unexpected type"
            )
        })
        .map(|metric| metric.name)
        .collect();

    (0..100)
        .map(|i| format!("metric.{i}"))
        .for_each(|metric_name| {
            assert!(
                metric_names.contains(&metric_name),
                "expected metric {metric_name} was not captured"
            )
        });
}

/// Test that 101 envelopes are sent in two separate envelopes
#[test]
fn test_metrics_batching_over_limit() {
    let options = ClientOptions {
        enable_metrics: true,
        ..Default::default()
    };

    let mut envelopes = test::with_captured_envelopes_options(
        || {
            (0..101)
                .map(|i| format!("metric.{i}"))
                .for_each(capture_test_metric);
        },
        options,
    )
    .into_iter();
    let envelope1 = envelopes.next().expect("expected a first envelope");
    let envelope2 = envelopes.next().expect("expected a second envelope");
    assert!(envelopes.next().is_none(), "expected exactly two envelopes");

    let item1 = envelope1
        .into_items()
        .into_only_item()
        .expect("expected exactly one item in the first envelope");
    let metrics1 = item1
        .into_metrics()
        .expect("the first envelope item is not a metrics item");

    assert_eq!(metrics1.len(), 100, "expected 100 metrics");

    let first_metric_names: HashSet<_> = metrics1
        .into_iter()
        .inspect(|metric| assert_eq!(metric.value, 1.0, "metric had unexpected value"))
        .inspect(|metric| {
            assert_eq!(
                metric.r#type,
                TraceMetricType::Counter,
                "metric had unexpected type"
            )
        })
        .map(|metric| metric.name)
        .collect();

    (0..100)
        .map(|i| format!("metric.{i}"))
        .for_each(|metric_name| {
            assert!(
                first_metric_names.contains(&metric_name),
                "expected metric {metric_name} was not captured in the first envelope"
            )
        });

    let item2 = envelope2
        .into_items()
        .into_only_item()
        .expect("expected exactly one item in the second envelope");
    let metrics2 = item2
        .into_metrics()
        .expect("the second envelope item is not a metrics item");
    let metric2 = metrics2
        .into_only_item()
        .expect("expected exactly one metric in the second envelope");

    assert!(
        matches!(metric2, TraceMetric {
            r#type: TraceMetricType::Counter,
            name,
            value: 1.0,
            ..
        } if name == "metric.100"),
        "unexpected metric captured"
    )
}

/// Returns a new [`TraceMetric`] with [type `Counter`](TraceMetricType),
/// the provided name, and a value of `1.0`. The other fields are unspecified.
fn test_metric<S>(name: S) -> TraceMetric
where
    S: Into<String>,
{
    TraceMetric::new(TraceMetricType::Counter, name, 1.0, Default::default())
}

/// Helper function to capture a metric, returned by `test_metric` on the current Hub.
fn capture_test_metric<S>(name: S)
where
    S: Into<String>,
{
    Hub::current().capture_metric(test_metric(name))
}

/// Exention trait for iterators allowing conversion to only item.
trait IntoOnlyElementExt<I> {
    type Item;

    /// Convert the iterator to the only item, erroring if the
    /// iterator does not contain exactly one item.
    fn into_only_item(self) -> Result<Self::Item>;
}

impl<I> IntoOnlyElementExt<I> for I
where
    I: IntoIterator,
{
    type Item = I::Item;

    fn into_only_item(self) -> Result<Self::Item> {
        let mut iter = self.into_iter();
        let rv = iter.next().context("iterator was empty")?;

        match iter.next() {
            Some(_) => anyhow::bail!("iterator had more than one item"),
            None => Ok(rv),
        }
    }
}

trait IntoMetricsExt {
    /// Attempt to convert the provided value to a trace metric,
    /// returning None if the conversion is not possible.
    fn into_metrics(self) -> Option<Vec<TraceMetric>>;
}

impl IntoMetricsExt for EnvelopeItem {
    fn into_metrics(self) -> Option<Vec<TraceMetric>> {
        match self {
            EnvelopeItem::ItemContainer(ItemContainer::TraceMetrics(metrics)) => Some(metrics),
            _ => None,
        }
    }
}

#![cfg(all(feature = "test", feature = "metrics"))]

use std::collections::HashSet;
use std::sync::Arc;

use anyhow::{Context, Result};

use sentry::protocol::{LogAttribute, TraceMetricType, User};
use sentry_core::protocol::{EnvelopeItem, ItemContainer, Value};
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

/// Helper to extract the single metric from captured envelopes.
fn extract_single_metric(envelopes: Vec<sentry_core::Envelope>) -> TraceMetric {
    let envelope = envelopes.into_only_item().expect("expected one envelope");
    let item = envelope
        .into_items()
        .into_only_item()
        .expect("expected one item");
    let mut metrics = item.into_metrics().expect("expected metrics item");
    assert_eq!(metrics.len(), 1, "expected exactly one metric");
    metrics.pop().unwrap()
}

/// Test that trace_id is set from the propagation context when no span is active.
#[test]
fn trace_id_from_propagation_context() {
    let options = ClientOptions {
        enable_metrics: true,
        ..Default::default()
    };

    let envelopes = test::with_captured_envelopes_options(|| capture_test_metric("test"), options);
    let metric = extract_single_metric(envelopes);

    // trace_id should be non-zero (set from propagation context)
    assert_ne!(
        metric.trace_id,
        Default::default(),
        "trace_id should be set from propagation context"
    );
}

/// Test that default SDK attributes are attached to metrics.
#[test]
fn default_attributes_attached() {
    let options = ClientOptions {
        enable_metrics: true,
        environment: Some("test-env".into()),
        release: Some("1.0.0".into()),
        server_name: Some("test-server".into()),
        ..Default::default()
    };

    let envelopes = test::with_captured_envelopes_options(|| capture_test_metric("test"), options);
    let metric = extract_single_metric(envelopes);

    assert_eq!(
        metric.attributes.get("sentry.environment"),
        Some(&LogAttribute(Value::from("test-env"))),
    );
    assert_eq!(
        metric.attributes.get("sentry.release"),
        Some(&LogAttribute(Value::from("1.0.0"))),
    );
    assert!(
        metric.attributes.contains_key("sentry.sdk.name"),
        "sentry.sdk.name should be present"
    );
    assert!(
        metric.attributes.contains_key("sentry.sdk.version"),
        "sentry.sdk.version should be present"
    );
    assert_eq!(
        metric.attributes.get("server.address"),
        Some(&LogAttribute(Value::from("test-server"))),
    );
}

/// Test that explicitly set metric attributes are not overwritten by defaults.
#[test]
fn default_attributes_do_not_overwrite_explicit() {
    let options = ClientOptions {
        enable_metrics: true,
        environment: Some("default-env".into()),
        ..Default::default()
    };

    let envelopes = test::with_captured_envelopes_options(
        || {
            let mut metric = test_metric("test");
            metric.attributes.insert(
                "sentry.environment".to_owned(),
                LogAttribute(Value::from("custom-env")),
            );
            Hub::current().capture_metric(metric);
        },
        options,
    );
    let metric = extract_single_metric(envelopes);

    assert_eq!(
        metric.attributes.get("sentry.environment"),
        Some(&LogAttribute(Value::from("custom-env"))),
        "explicitly set attribute should not be overwritten"
    );
}

/// Test that user attributes are NOT attached when `send_default_pii` is false.
#[test]
fn user_attributes_absent_without_send_default_pii() {
    let options = ClientOptions {
        enable_metrics: true,
        send_default_pii: false,
        ..Default::default()
    };

    let envelopes = test::with_captured_envelopes_options(
        || {
            sentry_core::configure_scope(|scope| {
                scope.set_user(Some(User {
                    id: Some("uid-123".into()),
                    username: Some("testuser".into()),
                    email: Some("test@example.com".into()),
                    ..Default::default()
                }));
            });
            capture_test_metric("test");
        },
        options,
    );
    let metric = extract_single_metric(envelopes);

    assert!(
        !metric.attributes.contains_key("user.id"),
        "user.id should not be set when send_default_pii is false"
    );
    assert!(
        !metric.attributes.contains_key("user.name"),
        "user.name should not be set when send_default_pii is false"
    );
    assert!(
        !metric.attributes.contains_key("user.email"),
        "user.email should not be set when send_default_pii is false"
    );
}

/// Test that user attributes ARE attached when `send_default_pii` is true.
#[test]
fn user_attributes_present_with_send_default_pii() {
    let options = ClientOptions {
        enable_metrics: true,
        send_default_pii: true,
        ..Default::default()
    };

    let envelopes = test::with_captured_envelopes_options(
        || {
            sentry_core::configure_scope(|scope| {
                scope.set_user(Some(User {
                    id: Some("uid-123".into()),
                    username: Some("testuser".into()),
                    email: Some("test@example.com".into()),
                    ..Default::default()
                }));
            });
            capture_test_metric("test");
        },
        options,
    );
    let metric = extract_single_metric(envelopes);

    assert_eq!(
        metric.attributes.get("user.id"),
        Some(&LogAttribute(Value::from("uid-123"))),
    );
    assert_eq!(
        metric.attributes.get("user.name"),
        Some(&LogAttribute(Value::from("testuser"))),
    );
    assert_eq!(
        metric.attributes.get("user.email"),
        Some(&LogAttribute(Value::from("test@example.com"))),
    );
}

/// Test that explicitly set user attributes on the metric are not overwritten
/// by scope user data, even when `send_default_pii` is true.
#[test]
fn user_attributes_do_not_overwrite_explicit() {
    let options = ClientOptions {
        enable_metrics: true,
        send_default_pii: true,
        ..Default::default()
    };

    let envelopes = test::with_captured_envelopes_options(
        || {
            sentry_core::configure_scope(|scope| {
                scope.set_user(Some(User {
                    id: Some("scope-uid".into()),
                    username: Some("scope-user".into()),
                    email: Some("scope@example.com".into()),
                    ..Default::default()
                }));
            });
            let mut metric = test_metric("test");
            metric.attributes.insert(
                "user.id".to_owned(),
                LogAttribute(Value::from("explicit-uid")),
            );
            Hub::current().capture_metric(metric);
        },
        options,
    );
    let metric = extract_single_metric(envelopes);

    assert_eq!(
        metric.attributes.get("user.id"),
        Some(&LogAttribute(Value::from("explicit-uid"))),
        "explicitly set user.id should not be overwritten"
    );
    // Non-explicit user attributes should still come from scope
    assert_eq!(
        metric.attributes.get("user.name"),
        Some(&LogAttribute(Value::from("scope-user"))),
    );
    assert_eq!(
        metric.attributes.get("user.email"),
        Some(&LogAttribute(Value::from("scope@example.com"))),
    );
}

/// Test that `before_send_metric` can filter out metrics.
#[test]
fn before_send_metric_can_drop() {
    let options = ClientOptions {
        enable_metrics: true,
        before_send_metric: Some(Arc::new(|_| None)),
        ..Default::default()
    };

    let envelopes = test::with_captured_envelopes_options(|| capture_test_metric("test"), options);
    assert!(
        envelopes.is_empty(),
        "metric should be dropped by before_send_metric"
    );
}

/// Test that `before_send_metric` can modify metrics.
#[test]
fn before_send_metric_can_modify() {
    let options = ClientOptions {
        enable_metrics: true,
        before_send_metric: Some(Arc::new(|mut metric| {
            metric.attributes.insert(
                "added_by_callback".to_owned(),
                LogAttribute(Value::from("yes")),
            );
            Some(metric)
        })),
        ..Default::default()
    };

    let envelopes = test::with_captured_envelopes_options(|| capture_test_metric("test"), options);
    let metric = extract_single_metric(envelopes);

    assert_eq!(
        metric.attributes.get("added_by_callback"),
        Some(&LogAttribute(Value::from("yes"))),
    );
}

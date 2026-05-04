#![cfg(all(feature = "test", feature = "metrics"))]

use std::collections::HashSet;
use std::sync::Arc;

use anyhow::{Context, Result};

use sentry::protocol::{MetricType, Unit, Value};
use sentry_core::protocol::{EnvelopeItem, ItemContainer};
use sentry_core::{metrics, test};
use sentry_core::{ClientOptions, TransactionContext};
use sentry_types::protocol::v7::{Envelope, LogAttribute, Metric, User};

/// Test that metrics are sent when metrics are enabled.
#[test]
fn sent_when_enabled() {
    let options = ClientOptions {
        enable_metrics: true,
        ..Default::default()
    };

    let mut envelopes =
        test::with_captured_envelopes_options(|| metrics::counter("test", 1).capture(), options);

    assert_eq!(envelopes.len(), 1, "expected exactly one envelope");

    let envelope = envelopes.pop().unwrap();

    let mut items = envelope.into_items();
    let Some(item) = items.next() else {
        panic!("Expected at least one item");
    };

    assert!(items.next().is_none(), "Expected only one item");

    let EnvelopeItem::ItemContainer(ItemContainer::Metrics(mut metrics)) = item else {
        panic!("Envelope item has unexpected structure");
    };

    assert_eq!(metrics.len(), 1, "Expected exactly one metric");

    let metric = metrics.pop().unwrap();
    assert!(matches!(metric, Metric {
        r#type: MetricType::Counter,
        name,
        value: 1.0,
        ..
    } if name == "test"));
}

/// Test that metrics are sent by default.
#[test]
fn metrics_enabled_by_default() {
    let options = ClientOptions::default();

    let envelopes =
        test::with_captured_envelopes_options(|| metrics::counter("test", 1).capture(), options);
    assert_eq!(
        envelopes.len(),
        1,
        "expected exactly one envelope when metrics are enabled by default"
    )
}

/// Test that metrics are disabled (not sent) when disabled in the
/// [`ClientOptions`].
#[test]
fn metrics_disabled_when_configured() {
    let options = ClientOptions {
        enable_metrics: false,
        ..Default::default()
    };

    let envelopes =
        test::with_captured_envelopes_options(|| metrics::counter("test", 1).capture(), options);
    assert!(
        envelopes.is_empty(),
        "no envelopes should be captured when metrics disabled"
    )
}

/// Test that no metrics are captured by a no-op call with
/// metrics enabled
#[test]
fn noop_sends_nothing() {
    let options = ClientOptions::default();

    let envelopes = test::with_captured_envelopes_options(|| (), options);

    assert!(envelopes.is_empty(), "no-op should not capture metrics");
}

/// Test that 100 metrics are sent in a single envelope.
#[test]
fn test_metrics_batching_at_limit() {
    let options = ClientOptions::default();

    let envelopes = test::with_captured_envelopes_options(
        || {
            (0..100)
                .map(|i| format!("metric.{i}"))
                .for_each(|name| metrics::counter(name, 1).capture());
        },
        options,
    );

    let envelope = envelopes
        .try_into_only_item()
        .expect("expected exactly one envelope");
    let item = envelope
        .into_items()
        .try_into_only_item()
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
                MetricType::Counter,
                "metric had unexpected type"
            )
        })
        .map(|metric| metric.name)
        .collect();

    (0..100)
        .map(|i| format!("metric.{i}"))
        .for_each(|metric_name| {
            assert!(
                metric_names.contains(metric_name.as_str()),
                "expected metric {metric_name} was not captured"
            )
        });
}

/// Test that 101 envelopes are sent in two separate envelopes
#[test]
fn test_metrics_batching_over_limit() {
    let options = ClientOptions::default();

    let mut envelopes = test::with_captured_envelopes_options(
        || {
            (0..101)
                .map(|i| format!("metric.{i}"))
                .for_each(|name| metrics::counter(name, 1).capture());
        },
        options,
    )
    .into_iter();
    let envelope1 = envelopes.next().expect("expected a first envelope");
    let envelope2 = envelopes.next().expect("expected a second envelope");
    assert!(envelopes.next().is_none(), "expected exactly two envelopes");

    let item1 = envelope1
        .into_items()
        .try_into_only_item()
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
                MetricType::Counter,
                "metric had unexpected type"
            )
        })
        .map(|metric| metric.name)
        .collect();

    (0..100)
        .map(|i| format!("metric.{i}"))
        .for_each(|metric_name| {
            assert!(
                first_metric_names.contains(metric_name.as_str()),
                "expected metric {metric_name} was not captured in the first envelope"
            )
        });

    let item2 = envelope2
        .into_items()
        .try_into_only_item()
        .expect("expected exactly one item in the second envelope");
    let metrics2 = item2
        .into_metrics()
        .expect("the second envelope item is not a metrics item");
    let metric2 = metrics2
        .try_into_only_item()
        .expect("expected exactly one metric in the second envelope");

    assert!(
        matches!(metric2, Metric {
            r#type: MetricType::Counter,
            name,
            value: 1.0,
            ..
        } if name == "metric.100"),
        "unexpected metric captured"
    )
}

#[test]
fn metric_attributes_are_captured() {
    let options = ClientOptions::default();

    let envelopes = test::with_captured_envelopes_options(
        || {
            metrics::counter("test", 1)
                .attribute("http.route", "/health")
                .attribute("http.response.status_code", 200)
                .capture();
        },
        options,
    );

    let envelope = envelopes
        .try_into_only_item()
        .expect("expected exactly one envelope");
    let item = envelope
        .into_items()
        .try_into_only_item()
        .expect("expected exactly one item");
    let metric = item
        .into_metrics()
        .expect("expected metrics item")
        .try_into_only_item()
        .expect("expected exactly one metric in the envelope");

    let Metric {
        r#type,
        name,
        value,
        timestamp: _,
        trace_id: _,
        span_id,
        unit,
        attributes,
    } = metric;

    assert_eq!(r#type, MetricType::Counter);
    assert_eq!(name, "test");
    assert_eq!(value, 1.0);
    assert!(span_id.is_none());
    assert!(unit.is_none());
    assert_eq!(
        attributes.get("http.route").map(|value| &value.0),
        Some(&Value::from("/health")),
    );
    assert_eq!(
        attributes
            .get("http.response.status_code")
            .map(|value| &value.0),
        Some(&Value::from(200)),
    );
}

#[test]
fn metric_unit_is_captured() {
    let options = ClientOptions::default();

    let envelopes = test::with_captured_envelopes_options(
        || metrics::gauge("test", 42).unit(Unit::Millisecond).capture(),
        options,
    );

    let envelope = envelopes
        .try_into_only_item()
        .expect("expected exactly one envelope");
    let item = envelope
        .into_items()
        .try_into_only_item()
        .expect("expected exactly one item");
    let metric = item
        .into_metrics()
        .expect("expected metrics item")
        .try_into_only_item()
        .expect("expected exactly one metric in the envelope");

    let Metric {
        r#type,
        name,
        value,
        timestamp: _,
        trace_id: _,
        span_id,
        unit,
        attributes: _,
    } = metric;

    assert_eq!(r#type, MetricType::Gauge);
    assert_eq!(name, "test");
    assert_eq!(value, 42.0);
    assert!(span_id.is_none());
    assert_eq!(unit, Some(Unit::Millisecond));
}

/// Test that metrics in the same scope share the same trace_id when no span is active.
///
/// This tests that trace ID is set from the propagation context when there is no active span.
#[test]
fn metrics_share_trace_id_without_active_span() {
    let options = ClientOptions::default();

    let envelopes = test::with_captured_envelopes_options(
        || {
            metrics::counter("test-2", 1).capture();
            metrics::counter("test-2", 1).capture();
        },
        options,
    );
    let envelope = envelopes
        .try_into_only_item()
        .expect("expected one envelope");
    let item = envelope
        .into_items()
        .try_into_only_item()
        .expect("expected one item");
    let metrics = item.into_metrics().expect("expected metrics item");

    let [metric1, metric2] = metrics.as_slice() else {
        panic!("expected exactly two metrics");
    };

    assert_eq!(
        metric1.trace_id, metric2.trace_id,
        "metrics in the same scope should share the same trace_id"
    );

    assert!(metric1.span_id.is_none());
    assert!(metric2.span_id.is_none());
}

/// Test that span_id is set from the active span when one is present.
#[test]
fn metrics_span_id_from_active_span() {
    let options = ClientOptions::default();

    let mut expected_span_id = None;
    let envelopes = test::with_captured_envelopes_options(
        || {
            let transaction_ctx = TransactionContext::new("test transaction", "test");
            expected_span_id = Some(transaction_ctx.span_id());
            let transaction = sentry_core::start_transaction(transaction_ctx);
            sentry_core::configure_scope(|scope| scope.set_span(Some(transaction.clone().into())));
            metrics::counter("test", 1).capture();
            transaction.finish();
        },
        options,
    );

    let expected_span_id = expected_span_id.expect("expected_span_id did not get set");

    let envelope = envelopes
        .try_into_only_item()
        .expect("expected one envelope");
    let item = envelope
        .into_items()
        .try_into_only_item()
        .expect("expected one item");
    let mut metrics = item.into_metrics().expect("expected metrics item");
    let metric = metrics.pop().expect("expected one metric");

    assert_eq!(
        metric.span_id,
        Some(expected_span_id),
        "span_id should be set from the active span"
    );
}

/// Test that default SDK attributes are attached to metrics.
#[test]
fn default_attributes_attached() {
    let options = ClientOptions {
        environment: Some("test-env".into()),
        release: Some("1.0.0".into()),
        server_name: Some("test-server".into()),
        ..Default::default()
    };

    let envelopes =
        test::with_captured_envelopes_options(|| metrics::counter("test", 1).capture(), options);
    let metric = extract_single_metric(envelopes).expect("expected a single-metric envelope");

    let expected_attributes = [
        ("sentry.environment", "test-env"),
        ("sentry.release", "1.0.0"),
        ("sentry.sdk.name", "sentry.rust"),
        ("sentry.sdk.version", env!("CARGO_PKG_VERSION")),
        ("server.address", "test-server"),
    ]
    .into_iter()
    .map(|(attribute, value)| (attribute.into(), value.into()))
    .collect();

    assert_eq!(metric.attributes, expected_attributes);
}

/// Test that optional default attributes are omitted when not configured.
#[test]
fn optional_default_attributes_omitted_when_not_configured() {
    let options = ClientOptions::default();

    let envelopes =
        test::with_captured_envelopes_options(|| metrics::counter("test", 1).capture(), options);
    let metric = extract_single_metric(envelopes).expect("expected a single-metric envelope");

    let expected_attributes = [
        // Importantly, no other attributes should be set.
        ("sentry.sdk.name", "sentry.rust"),
        ("sentry.sdk.version", env!("CARGO_PKG_VERSION")),
    ]
    .into_iter()
    .map(|(attribute, value)| (attribute.into(), value.into()))
    .collect();

    assert_eq!(metric.attributes, expected_attributes);
}

/// Test that explicitly set metric attributes are not overwritten by defaults.
#[test]
fn default_attributes_do_not_overwrite_explicit() {
    let options = ClientOptions {
        environment: Some("default-env".into()),
        ..Default::default()
    };

    let envelopes = test::with_captured_envelopes_options(
        || {
            metrics::counter("test", 1)
                .attribute("sentry.environment", "custom-env")
                .capture();
        },
        options,
    );
    let metric = extract_single_metric(envelopes).expect("expected a single-metric envelope");

    let expected_attributes = [
        // Check the environment is the one set directly on the metric
        ("sentry.environment", "custom-env"),
        // The other default attributes also stay
        ("sentry.sdk.name", "sentry.rust"),
        ("sentry.sdk.version", env!("CARGO_PKG_VERSION")),
    ]
    .into_iter()
    .map(|(attribute, value)| (attribute.into(), value.into()))
    .collect();

    assert_eq!(metric.attributes, expected_attributes);
}

/// Test that user attributes are NOT attached when `send_default_pii` is false.
#[test]
fn user_attributes_absent_without_send_default_pii() {
    let options = ClientOptions {
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
            metrics::counter("test", 1).capture();
        },
        options,
    );
    let metric = extract_single_metric(envelopes).expect("expected a single-metric envelope");

    let expected_attributes = [
        // Note the lack of user attributes, despite setting them on the scope.
        ("sentry.sdk.name", "sentry.rust"),
        ("sentry.sdk.version", env!("CARGO_PKG_VERSION")),
    ]
    .into_iter()
    .map(|(attribute, value)| (attribute.into(), value.into()))
    .collect();

    assert_eq!(metric.attributes, expected_attributes);
}

/// Test that scope user attributes are attached to metrics when
/// `send_default_pii` is true.
#[test]
fn metric_user_attributes_from_scope_are_applied_with_send_default_pii() {
    let options = ClientOptions {
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
            metrics::counter("test", 1).capture()
        },
        options,
    );
    let metric = extract_single_metric(envelopes).expect("expected a single-metric envelope");

    let expected_attributes = [
        ("sentry.sdk.name", "sentry.rust"),
        ("sentry.sdk.version", env!("CARGO_PKG_VERSION")),
        ("user.id", "uid-123"),
        ("user.name", "testuser"),
        ("user.email", "test@example.com"),
    ]
    .into_iter()
    .map(|(attribute, value)| (attribute.into(), value.into()))
    .collect();

    assert_eq!(metric.attributes, expected_attributes);
}

/// Test that if a metric already has any user attribute set, scope user
/// attributes are not merged in.
#[test]
fn metric_user_attributes_do_not_overwrite_explicit() {
    let options = ClientOptions {
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
            metrics::counter("test", 1)
                .attribute("user.id", "explicit-uid")
                .attribute("user.name", "explicit-user")
                .capture();
        },
        options,
    );
    let metric = extract_single_metric(envelopes).expect("expected a single-metric envelope");

    let expected_attributes = [
        ("sentry.sdk.name", "sentry.rust"),
        ("sentry.sdk.version", env!("CARGO_PKG_VERSION")),
        ("user.id", "explicit-uid"),
        ("user.name", "explicit-user"),
    ]
    .into_iter()
    .map(|(attribute, value)| (attribute.into(), value.into()))
    .collect();

    assert_eq!(metric.attributes, expected_attributes);
}

/// Test that `before_send_metric` can filter out metrics.
#[test]
fn before_send_metric_can_drop() {
    let options = ClientOptions {
        before_send_metric: Some(Arc::new(|_| None)),
        ..Default::default()
    };

    let envelopes =
        test::with_captured_envelopes_options(|| metrics::counter("test", 1).capture(), options);
    assert!(
        envelopes.is_empty(),
        "metric should be dropped by before_send_metric"
    );
}

/// Test that `before_send_metric` can modify metrics.
#[test]
fn before_send_metric_can_modify() {
    let options = ClientOptions {
        before_send_metric: Some(Arc::new(|mut metric| {
            metric
                .attributes
                .insert("added_by_callback".into(), LogAttribute(Value::from("yes")));
            Some(metric)
        })),
        ..Default::default()
    };

    let envelopes =
        test::with_captured_envelopes_options(|| metrics::counter("test", 1).capture(), options);
    let metric = extract_single_metric(envelopes).expect("expected a single-metric envelope");

    assert_eq!(
        metric.attributes.get("added_by_callback"),
        Some(&LogAttribute(Value::from("yes"))),
    );
}

/// Returns a [`Metric`] with [type `Counter`](MetricType),
/// the provided name, and a value of `1.0`.
/// Helper to extract the single metric from a list of captured envelopes.
///
/// Asserts that the envelope contains only a single item, which contains only
/// a single metrics item, and returns that metrics item, or an error if failed.
fn extract_single_metric<I>(envelopes: I) -> Result<Metric>
where
    I: IntoIterator<Item = Envelope>,
{
    envelopes
        .try_into_only_item()
        .context("expected exactly one envelope")?
        .into_items()
        .try_into_only_item()
        .context("expected exactly one item")?
        .into_metrics()
        .context("expected a metrics item")?
        .try_into_only_item()
        .context("expected exactly one metric")
}

/// Extension trait for iterators allowing conversion to only item.
trait TryIntoOnlyElementExt<I> {
    type Item;

    /// Convert the iterator to the only item, erroring if the
    /// iterator does not contain exactly one item.
    fn try_into_only_item(self) -> Result<Self::Item>;
}

impl<I> TryIntoOnlyElementExt<I> for I
where
    I: IntoIterator,
{
    type Item = I::Item;

    fn try_into_only_item(self) -> Result<Self::Item> {
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
    fn into_metrics(self) -> Option<Vec<Metric>>;
}

impl IntoMetricsExt for EnvelopeItem {
    fn into_metrics(self) -> Option<Vec<Metric>> {
        match self {
            EnvelopeItem::ItemContainer(ItemContainer::Metrics(metrics)) => Some(metrics),
            _ => None,
        }
    }
}

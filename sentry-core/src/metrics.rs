//! Public API for Sentry [trace metrics](https://develop.sentry.dev/sdk/telemetry/metrics/).

use std::time::SystemTime;

use crate::protocol::{LogAttribute, Map, TraceId, TraceMetric, TraceMetricType};
use crate::Hub;

/// Options for recording a trace metric.
#[derive(Default)]
pub struct MetricOptions {
    /// The measurement unit (e.g. "millisecond", "byte").
    pub unit: Option<String>,
    /// Additional key-value attributes.
    pub attributes: Map<String, LogAttribute>,
}

fn capture_metric(
    metric_type: TraceMetricType,
    name: &str,
    value: f64,
    options: Option<MetricOptions>,
) {
    Hub::with_active(|hub| {
        let opts = options.unwrap_or_default();
        let metric = TraceMetric {
            r#type: metric_type,
            name: name.to_owned(),
            value,
            timestamp: SystemTime::now(),
            trace_id: TraceId::default(),
            span_id: None,
            unit: opts.unit,
            attributes: opts.attributes,
        };
        hub.capture_metric(metric);
    })
}

/// Records a counter metric. Counters track event frequency (e.g., requests, errors).
///
/// # Examples
///
/// ```
/// sentry::metrics_count("api.requests", 1.0, None);
/// ```
pub fn metrics_count(name: &str, value: f64, options: Option<MetricOptions>) {
    capture_metric(TraceMetricType::Counter, name, value, options);
}

/// Records a gauge metric. Gauges represent current state (e.g., memory usage, pool size).
///
/// # Examples
///
/// ```
/// sentry::metrics_gauge("memory.usage", 1024.0, None);
/// ```
pub fn metrics_gauge(name: &str, value: f64, options: Option<MetricOptions>) {
    capture_metric(TraceMetricType::Gauge, name, value, options);
}

/// Records a distribution metric. Distributions measure statistical spread (e.g., response times).
///
/// # Examples
///
/// ```
/// sentry::metrics_distribution("http.response_time", 150.0, None);
/// ```
pub fn metrics_distribution(name: &str, value: f64, options: Option<MetricOptions>) {
    capture_metric(TraceMetricType::Distribution, name, value, options);
}

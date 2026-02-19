//! Public API for Sentry [trace metrics](https://develop.sentry.dev/sdk/telemetry/metrics/).
//!
//! # Examples
//!
//! ```
//! sentry::metrics::count("api.requests").emit(1);
//!
//! sentry::metrics::distribution("response.time")
//!     .with_attribute("route", "my_route")
//!     .with_unit("millisecond")
//!     .emit(0.123);
//!
//! sentry::metrics::gauge("memory.usage").emit(1024.0);
//! ```

use std::time::SystemTime;

use crate::protocol::{LogAttribute, Map, TraceId, TraceMetric, TraceMetricType};
use crate::Hub;

/// Creates a counter metric builder.
///
/// # Examples
///
/// ```
/// sentry::metrics::count("api.requests").emit(1);
/// ```
pub fn count(name: impl Into<String>) -> MetricBuilder {
    MetricBuilder::new(name, TraceMetricType::Counter)
}

/// Creates a gauge metric builder.
///
/// # Examples
///
/// ```
/// sentry::metrics::gauge("memory.usage").emit(1024.0);
/// ```
pub fn gauge(name: impl Into<String>) -> MetricBuilder {
    MetricBuilder::new(name, TraceMetricType::Gauge)
}

/// Creates a distribution metric builder.
///
/// # Examples
///
/// ```
/// sentry::metrics::distribution("response.time")
///     .with_attribute("route", "my_route")
///     .with_unit("millisecond")
///     .emit(0.123);
/// ```
pub fn distribution(name: impl Into<String>) -> MetricBuilder {
    MetricBuilder::new(name, TraceMetricType::Distribution)
}

/// A builder for constructing and emitting a trace metric.
///
/// Created via [`count()`], [`gauge()`], or [`distribution()`].
/// Call [`emit()`](MetricBuilder::emit) to send the metric.
pub struct MetricBuilder {
    name: String,
    metric_type: TraceMetricType,
    unit: Option<String>,
    attributes: Map<String, LogAttribute>,
}

impl MetricBuilder {
    fn new(name: impl Into<String>, metric_type: TraceMetricType) -> Self {
        Self {
            name: name.into(),
            metric_type,
            unit: None,
            attributes: Map::new(),
        }
    }

    /// Sets the measurement unit (e.g. "millisecond", "byte").
    pub fn with_unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    /// Adds a key-value attribute to the metric.
    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<LogAttribute>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }

    /// Emits the metric with the given value.
    pub fn emit(self, value: impl Into<f64>) {
        Hub::with_active(|hub| {
            let metric = TraceMetric {
                r#type: self.metric_type,
                name: self.name,
                value: value.into(),
                timestamp: SystemTime::now(),
                trace_id: TraceId::default(),
                span_id: None,
                unit: self.unit,
                attributes: self.attributes,
            };
            hub.capture_metric(metric);
        })
    }
}

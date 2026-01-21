//! Protocol types for runtime metrics.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::SystemTime;

/// A collection of runtime metrics captured at a point in time.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuntimeMetrics {
    /// Timestamp when metrics were collected.
    pub timestamp: SystemTime,

    /// The runtime/platform identifier (e.g., "rust", "tokio").
    pub platform: String,

    /// Collection of metric values.
    pub metrics: Vec<RuntimeMetric>,
}

impl RuntimeMetrics {
    /// Creates a new RuntimeMetrics collection.
    pub fn new(platform: impl Into<String>) -> Self {
        Self {
            timestamp: SystemTime::now(),
            platform: platform.into(),
            metrics: Vec::new(),
        }
    }

    /// Adds a metric to the collection.
    pub fn add_metric(&mut self, metric: RuntimeMetric) {
        self.metrics.push(metric);
    }

    /// Extends the collection with multiple metrics.
    pub fn extend_metrics(&mut self, metrics: impl IntoIterator<Item = RuntimeMetric>) {
        self.metrics.extend(metrics);
    }

    /// Returns true if there are no metrics.
    pub fn is_empty(&self) -> bool {
        self.metrics.is_empty()
    }
}

/// A single runtime metric measurement.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuntimeMetric {
    /// Metric name following the naming convention (e.g., "runtime.memory.rss").
    pub name: String,

    /// The type of metric (gauge, counter, distribution).
    #[serde(rename = "type")]
    pub metric_type: MetricType,

    /// The metric value.
    pub value: MetricValue,

    /// Unit of measurement (e.g., "bytes", "count", "milliseconds").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,

    /// Optional tags for additional context.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tags: BTreeMap<String, String>,
}

impl RuntimeMetric {
    /// Creates a new gauge metric.
    pub fn gauge(name: impl Into<String>, value: impl Into<MetricValue>) -> Self {
        Self {
            name: name.into(),
            metric_type: MetricType::Gauge,
            value: value.into(),
            unit: None,
            tags: BTreeMap::new(),
        }
    }

    /// Creates a new counter metric.
    pub fn counter(name: impl Into<String>, value: impl Into<MetricValue>) -> Self {
        Self {
            name: name.into(),
            metric_type: MetricType::Counter,
            value: value.into(),
            unit: None,
            tags: BTreeMap::new(),
        }
    }

    /// Sets the unit for this metric.
    #[must_use]
    pub fn with_unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    /// Adds a tag to this metric.
    #[must_use]
    pub fn with_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.insert(key.into(), value.into());
        self
    }
}

/// The type of metric being recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricType {
    /// A point-in-time value that can go up or down (e.g., current memory usage).
    Gauge,
    /// A monotonically increasing value (e.g., total requests processed).
    Counter,
    /// A distribution of values for histograms (e.g., latencies).
    Distribution,
}

/// Metric value representation supporting both integers and floats.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MetricValue {
    /// An integer value.
    Int(i64),
    /// A floating-point value.
    Float(f64),
}

impl From<i64> for MetricValue {
    fn from(v: i64) -> Self {
        MetricValue::Int(v)
    }
}

impl From<i32> for MetricValue {
    fn from(v: i32) -> Self {
        MetricValue::Int(v as i64)
    }
}

impl From<u64> for MetricValue {
    fn from(v: u64) -> Self {
        MetricValue::Int(v as i64)
    }
}

impl From<u32> for MetricValue {
    fn from(v: u32) -> Self {
        MetricValue::Int(v as i64)
    }
}

impl From<usize> for MetricValue {
    fn from(v: usize) -> Self {
        MetricValue::Int(v as i64)
    }
}

impl From<f64> for MetricValue {
    fn from(v: f64) -> Self {
        MetricValue::Float(v)
    }
}

impl From<f32> for MetricValue {
    fn from(v: f32) -> Self {
        MetricValue::Float(v as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_serialization() {
        let metric = RuntimeMetric::gauge("runtime.memory.rss", 1024_i64)
            .with_unit("bytes")
            .with_tag("platform", "linux");

        let json = serde_json::to_string(&metric).unwrap();
        assert!(json.contains("runtime.memory.rss"));
        assert!(json.contains("gauge"));
        assert!(json.contains("1024"));
    }

    #[test]
    fn test_runtime_metrics_collection() {
        let mut metrics = RuntimeMetrics::new("rust");
        metrics.add_metric(RuntimeMetric::gauge("runtime.memory.rss", 1024_i64));
        metrics.add_metric(RuntimeMetric::counter("process.cpu.user_time", 500_i64));

        assert_eq!(metrics.metrics.len(), 2);
        assert!(!metrics.is_empty());
    }
}

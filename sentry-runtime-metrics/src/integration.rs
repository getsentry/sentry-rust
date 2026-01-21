//! The runtime metrics integration.

use std::collections::BTreeMap;

use sentry_core::protocol::{Context, Event, Value};
use sentry_core::{ClientOptions, Integration};

use crate::collectors;
use crate::config::RuntimeMetricsConfig;
use crate::protocol::{RuntimeMetric, RuntimeMetrics};
use crate::MetricCollector;

/// Integration for collecting lightweight runtime metrics.
///
/// This integration automatically collects runtime health metrics that
/// do NOT overlap with tracing, providing quick health insights.
///
/// # Example
///
/// ```rust,ignore
/// use sentry::ClientOptions;
/// use sentry_runtime_metrics::{RuntimeMetricsIntegration, RuntimeMetricsConfig};
/// use std::time::Duration;
///
/// let _guard = sentry::init(ClientOptions::new()
///     .add_integration(RuntimeMetricsIntegration::new(RuntimeMetricsConfig {
///         collection_interval: Duration::from_secs(10),
///         ..Default::default()
///     }))
/// );
/// ```
pub struct RuntimeMetricsIntegration {
    config: RuntimeMetricsConfig,
}

impl RuntimeMetricsIntegration {
    /// Creates a new runtime metrics integration with the given configuration.
    pub fn new(config: RuntimeMetricsConfig) -> Self {
        Self { config }
    }

    /// Creates a new runtime metrics integration with default configuration.
    pub fn default_config() -> Self {
        Self::new(RuntimeMetricsConfig::default())
    }

    /// Collects a snapshot of all metrics.
    pub fn collect_snapshot(&self) -> RuntimeMetrics {
        let mut runtime_metrics = RuntimeMetrics::new("rust");

        // Collect from built-in collectors
        let built_in_collectors = self.build_collectors();
        for collector in &built_in_collectors {
            runtime_metrics.extend_metrics(collector.collect());
        }

        // Collect from custom collectors
        for collector in &self.config.custom_collectors {
            runtime_metrics.extend_metrics(collector.collect());
        }

        runtime_metrics
    }

    fn build_collectors(&self) -> Vec<Box<dyn MetricCollector>> {
        let mut collectors: Vec<Box<dyn MetricCollector>> = Vec::new();

        #[cfg(feature = "memory")]
        if self.config.collect_memory {
            collectors.push(Box::new(collectors::MemoryCollector::new()));
        }

        #[cfg(feature = "process")]
        if self.config.collect_process {
            collectors.push(Box::new(collectors::ProcessCollector::new()));
        }

        #[cfg(feature = "tokio-runtime")]
        if self.config.collect_async_runtime {
            if let Some(collector) = collectors::TokioCollector::try_new() {
                collectors.push(Box::new(collector));
            }
        }

        collectors
    }
}

impl std::fmt::Debug for RuntimeMetricsIntegration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeMetricsIntegration")
            .field("config", &self.config)
            .finish()
    }
}

impl Integration for RuntimeMetricsIntegration {
    fn name(&self) -> &'static str {
        "runtime-metrics"
    }

    fn setup(&self, _options: &mut ClientOptions) {
        // Future: Start background collection task if needed
        // For now, metrics are attached to events via process_event
    }

    fn process_event(
        &self,
        mut event: Event<'static>,
        _options: &ClientOptions,
    ) -> Option<Event<'static>> {
        // Attach current metrics snapshot to event context
        let metrics = self.collect_snapshot();

        if !metrics.is_empty() {
            // Convert metrics to a context-friendly format
            let metrics_context = metrics_to_context(&metrics);
            event.contexts.insert("runtime_metrics".into(), metrics_context);
        }

        Some(event)
    }
}

/// Converts RuntimeMetrics to a Sentry Context for event attachment.
fn metrics_to_context(metrics: &RuntimeMetrics) -> Context {
    let mut data: BTreeMap<String, Value> = BTreeMap::new();
    data.insert("platform".into(), Value::String(metrics.platform.clone()));

    let metrics_array: Vec<Value> = metrics
        .metrics
        .iter()
        .map(metric_to_value)
        .collect();

    data.insert("metrics".into(), Value::Array(metrics_array));

    Context::Other(data)
}

fn metric_to_value(metric: &RuntimeMetric) -> Value {
    // Use serde_json to serialize the metric, then convert
    serde_json::to_value(metric).unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integration_creation() {
        let integration = RuntimeMetricsIntegration::default_config();
        assert_eq!(integration.name(), "runtime-metrics");
    }

    #[test]
    fn test_collect_snapshot() {
        let integration = RuntimeMetricsIntegration::default_config();
        let metrics = integration.collect_snapshot();

        assert_eq!(metrics.platform, "rust");
        // Should have some metrics from built-in collectors
    }
}

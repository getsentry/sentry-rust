//! Configuration for runtime metrics collection.

use std::sync::Arc;
use std::time::Duration;

use crate::collector::MetricCollector;

/// Configuration for the runtime metrics integration.
#[derive(Clone)]
pub struct RuntimeMetricsConfig {
    /// How often to collect and send metrics.
    ///
    /// Default: 10 seconds
    pub collection_interval: Duration,

    /// Enable memory metrics collection.
    ///
    /// Collects: `runtime.memory.rss`, `runtime.memory.heap_allocated`
    ///
    /// Default: true
    pub collect_memory: bool,

    /// Enable process metrics collection.
    ///
    /// Collects: `process.threads.count`, `process.cpu.user_time`,
    /// `process.cpu.system_time`, `process.open_fds`
    ///
    /// Default: true
    pub collect_process: bool,

    /// Enable async runtime metrics collection.
    ///
    /// For Tokio: `async.workers.count`, `async.blocking.threads`, `async.polls.total`
    ///
    /// Default: true (when tokio-runtime feature is enabled)
    pub collect_async_runtime: bool,

    /// Custom metric collectors to include.
    ///
    /// Use this to add application-specific metrics.
    pub custom_collectors: Vec<Arc<dyn MetricCollector>>,
}

impl Default for RuntimeMetricsConfig {
    fn default() -> Self {
        Self {
            collection_interval: Duration::from_secs(10),
            collect_memory: true,
            collect_process: true,
            collect_async_runtime: true,
            custom_collectors: Vec::new(),
        }
    }
}

impl RuntimeMetricsConfig {
    /// Creates a new configuration with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the collection interval.
    #[must_use]
    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.collection_interval = interval;
        self
    }

    /// Enables or disables memory metrics.
    #[must_use]
    pub fn with_memory_metrics(mut self, enabled: bool) -> Self {
        self.collect_memory = enabled;
        self
    }

    /// Enables or disables process metrics.
    #[must_use]
    pub fn with_process_metrics(mut self, enabled: bool) -> Self {
        self.collect_process = enabled;
        self
    }

    /// Enables or disables async runtime metrics.
    #[must_use]
    pub fn with_async_runtime_metrics(mut self, enabled: bool) -> Self {
        self.collect_async_runtime = enabled;
        self
    }

    /// Adds a custom metric collector.
    #[must_use]
    pub fn add_collector<C: MetricCollector>(mut self, collector: C) -> Self {
        self.custom_collectors.push(Arc::new(collector));
        self
    }
}

impl std::fmt::Debug for RuntimeMetricsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeMetricsConfig")
            .field("collection_interval", &self.collection_interval)
            .field("collect_memory", &self.collect_memory)
            .field("collect_process", &self.collect_process)
            .field("collect_async_runtime", &self.collect_async_runtime)
            .field("custom_collectors_count", &self.custom_collectors.len())
            .finish()
    }
}

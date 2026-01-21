//! Tokio runtime metrics collector.
//!
//! This collector requires the `tokio-runtime` feature and Tokio's
//! unstable runtime metrics API.

use crate::collector::MetricCollector;
use crate::protocol::RuntimeMetric;

/// Collects Tokio async runtime metrics.
///
/// Metrics collected (requires `tokio_unstable`):
/// - `async.workers.count` - Number of worker threads
/// - `async.blocking.threads` - Number of blocking threads
/// - `async.polls.total` - Total number of task polls
/// - `async.injection_queue.depth` - Tasks waiting to be assigned
/// - `async.local_queue.depth` - Total local queue depth
///
/// Without `tokio_unstable`, only `async.runtime.available` is reported.
///
/// Note: Most Tokio metrics require the `tokio_unstable` cfg flag:
/// `RUSTFLAGS="--cfg tokio_unstable" cargo build`
pub struct TokioCollector {
    // Used when tokio_unstable is enabled
    #[allow(dead_code)]
    handle: tokio::runtime::Handle,
}

impl TokioCollector {
    /// Try to create a new Tokio collector.
    ///
    /// Returns `None` if no Tokio runtime is available.
    pub fn try_new() -> Option<Self> {
        tokio::runtime::Handle::try_current()
            .ok()
            .map(|handle| Self { handle })
    }

    /// Creates a new Tokio collector with the given runtime handle.
    pub fn with_handle(handle: tokio::runtime::Handle) -> Self {
        Self { handle }
    }
}

impl MetricCollector for TokioCollector {
    fn collect(&self) -> Vec<RuntimeMetric> {
        let mut metrics = Vec::new();

        // Basic runtime info that's always available
        // Note: Most detailed metrics require tokio_unstable

        // When tokio_unstable is enabled, we can access RuntimeMetrics
        #[cfg(tokio_unstable)]
        {
            let rt_metrics = self.handle.metrics();

            // Number of worker threads
            metrics.push(
                RuntimeMetric::gauge("async.workers.count", rt_metrics.num_workers() as i64)
                    .with_unit("count")
                    .with_tag("runtime", "tokio"),
            );

            // Number of blocking threads
            metrics.push(
                RuntimeMetric::gauge(
                    "async.blocking.threads",
                    rt_metrics.num_blocking_threads() as i64,
                )
                .with_unit("count")
                .with_tag("runtime", "tokio"),
            );

            // Total polls across all workers
            let total_polls: u64 = (0..rt_metrics.num_workers())
                .map(|i| rt_metrics.worker_poll_count(i))
                .sum();

            metrics.push(
                RuntimeMetric::counter("async.polls.total", total_polls as i64)
                    .with_tag("runtime", "tokio"),
            );

            // Injection queue depth (tasks waiting to be assigned to workers)
            let injection_queue_depth = rt_metrics.injection_queue_depth();
            metrics.push(
                RuntimeMetric::gauge("async.injection_queue.depth", injection_queue_depth as i64)
                    .with_unit("count")
                    .with_tag("runtime", "tokio"),
            );

            // Worker local queue depths
            let total_local_queue: usize = (0..rt_metrics.num_workers())
                .map(|i| rt_metrics.worker_local_queue_depth(i))
                .sum();

            metrics.push(
                RuntimeMetric::gauge("async.local_queue.depth", total_local_queue as i64)
                    .with_unit("count")
                    .with_tag("runtime", "tokio"),
            );
        }

        // If tokio_unstable is not enabled, we can still provide a marker
        #[cfg(not(tokio_unstable))]
        {
            // Just indicate that Tokio is being used
            metrics.push(
                RuntimeMetric::gauge("async.runtime.available", 1_i64)
                    .with_tag("runtime", "tokio")
                    .with_tag("metrics_available", "false"),
            );
        }

        metrics
    }

    fn name(&self) -> &'static str {
        "tokio"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tokio_collector() {
        let collector = TokioCollector::try_new().expect("should have runtime");
        let metrics = collector.collect();

        // Should have at least one metric
        assert!(!metrics.is_empty());
    }
}

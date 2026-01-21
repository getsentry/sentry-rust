//! Metric collector trait and utilities.

use crate::RuntimeMetric;

/// Trait for custom metric collectors.
///
/// Implement this trait to add custom metrics to the runtime metrics collection.
///
/// # Example
///
/// ```rust
/// use sentry_runtime_metrics::{MetricCollector, RuntimeMetric, MetricValue};
///
/// struct ConnectionPoolCollector {
///     // Reference to your connection pool
/// }
///
/// impl MetricCollector for ConnectionPoolCollector {
///     fn collect(&self) -> Vec<RuntimeMetric> {
///         vec![
///             RuntimeMetric::gauge("db.pool.connections.active", 5_i64)
///                 .with_unit("count"),
///             RuntimeMetric::gauge("db.pool.connections.idle", 10_i64)
///                 .with_unit("count"),
///         ]
///     }
///
///     fn name(&self) -> &'static str {
///         "connection-pool"
///     }
/// }
/// ```
pub trait MetricCollector: Send + Sync + 'static {
    /// Collect metrics and return them.
    ///
    /// This method is called periodically by the integration.
    /// It should be fast and non-blocking.
    fn collect(&self) -> Vec<RuntimeMetric>;

    /// Name of this collector for debugging and logging.
    fn name(&self) -> &'static str;
}

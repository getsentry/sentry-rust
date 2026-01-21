//! Example demonstrating runtime metrics collection.
//!
//! This example shows how to use the RuntimeMetricsIntegration to collect
//! and display runtime health metrics.

use sentry_runtime_metrics::{
    MetricCollector, RuntimeMetric, RuntimeMetricsConfig, RuntimeMetricsIntegration,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// Custom collector example - monitors a hypothetical connection pool
struct ConnectionPoolCollector {
    active: AtomicUsize,
    idle: AtomicUsize,
}

impl ConnectionPoolCollector {
    fn new() -> Self {
        Self {
            active: AtomicUsize::new(5),
            idle: AtomicUsize::new(10),
        }
    }
}

impl MetricCollector for ConnectionPoolCollector {
    fn collect(&self) -> Vec<RuntimeMetric> {
        vec![
            RuntimeMetric::gauge(
                "db.pool.connections.active",
                self.active.load(Ordering::Relaxed) as i64,
            )
            .with_unit("count")
            .with_tag("pool", "main"),
            RuntimeMetric::gauge(
                "db.pool.connections.idle",
                self.idle.load(Ordering::Relaxed) as i64,
            )
            .with_unit("count")
            .with_tag("pool", "main"),
        ]
    }

    fn name(&self) -> &'static str {
        "connection-pool"
    }
}

fn main() {
    // Configure runtime metrics with custom collector
    let config = RuntimeMetricsConfig::new()
        .with_interval(Duration::from_secs(5))
        .with_memory_metrics(true)
        .with_process_metrics(true)
        .add_collector(ConnectionPoolCollector::new());

    // Create the integration
    let integration = RuntimeMetricsIntegration::new(config);

    println!("Runtime Metrics Integration Demo");
    println!("================================\n");

    // Collect and display a snapshot
    let snapshot = integration.collect_snapshot();
    println!("Collected {} metrics at {:?}:\n", snapshot.metrics.len(), snapshot.timestamp);

    for metric in &snapshot.metrics {
        println!(
            "  {:<40} {:?} = {:?} {}",
            metric.name,
            metric.metric_type,
            metric.value,
            metric.unit.as_deref().unwrap_or("")
        );
        if !metric.tags.is_empty() {
            println!("    tags: {:?}", metric.tags);
        }
    }

    println!("\n✓ Runtime metrics collection working!");
    println!("\nTo use with Sentry, add this integration to your ClientOptions:");
    println!("  sentry::init(ClientOptions::new().add_integration(integration))");
}

# Sentry Runtime Metrics

Lightweight runtime metrics collection for Sentry that provides health signals 
that do NOT overlap with Tracing.

## Features

This integration collects metrics that give a quick sense of app/runtime health:

- **Memory metrics**: Heap usage, RSS, allocations
- **Process metrics**: CPU usage, thread count, file descriptors
- **Async runtime metrics**: Task counts, poll durations (Tokio support)

## Usage

```rust
use sentry::ClientOptions;
use sentry_runtime_metrics::{RuntimeMetricsIntegration, RuntimeMetricsConfig};
use std::time::Duration;

fn main() {
    let _guard = sentry::init(ClientOptions::new()
        .add_integration(RuntimeMetricsIntegration::new(RuntimeMetricsConfig {
            collection_interval: Duration::from_secs(10),
            ..Default::default()
        }))
    );
    
    // Your application code...
}
```

## Collected Metrics

### Memory Metrics (`feature = "memory"`, enabled by default)

| Metric | Type | Description |
|--------|------|-------------|
| `runtime.memory.rss` | Gauge | Resident Set Size in bytes |
| `runtime.memory.heap_allocated` | Gauge | Heap allocation (with jemalloc) |

### Process Metrics (`feature = "process"`, enabled by default)

| Metric | Type | Description |
|--------|------|-------------|
| `process.threads.count` | Gauge | Number of threads |
| `process.cpu.user_time` | Counter | User CPU time in ms |
| `process.cpu.system_time` | Counter | System CPU time in ms |
| `process.open_fds` | Gauge | Open file descriptors (Unix) |

### Tokio Runtime Metrics (`feature = "tokio-runtime"`)

| Metric | Type | Description |
|--------|------|-------------|
| `async.workers.count` | Gauge | Number of worker threads |
| `async.blocking.threads` | Gauge | Blocking thread pool size |
| `async.polls.total` | Counter | Total task polls |

## Configuration

```rust
RuntimeMetricsConfig {
    /// How often to collect metrics (default: 10 seconds)
    collection_interval: Duration::from_secs(10),
    
    /// Enable memory metrics collection
    collect_memory: true,
    
    /// Enable process metrics collection
    collect_process: true,
    
    /// Enable async runtime metrics (requires tokio-runtime feature)
    collect_async_runtime: true,
    
    /// Custom metric collectors
    custom_collectors: vec![],
}
```

## Custom Collectors

Implement the `MetricCollector` trait to add custom metrics:

```rust
use sentry_runtime_metrics::{MetricCollector, RuntimeMetric, MetricType, MetricValue};

struct MyCustomCollector;

impl MetricCollector for MyCustomCollector {
    fn collect(&self) -> Vec<RuntimeMetric> {
        vec![RuntimeMetric {
            name: "my_app.custom_metric".into(),
            metric_type: MetricType::Gauge,
            value: MetricValue::Int(42),
            unit: Some("count".into()),
            tags: Default::default(),
        }]
    }
    
    fn name(&self) -> &'static str {
        "my-custom-collector"
    }
}
```

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `memory` | ✅ | Memory metrics collection |
| `process` | ✅ | Process/CPU metrics collection |
| `tokio-runtime` | ❌ | Tokio async runtime metrics |
| `jemalloc` | ❌ | Detailed jemalloc memory stats |

## License

Licensed under the MIT license.

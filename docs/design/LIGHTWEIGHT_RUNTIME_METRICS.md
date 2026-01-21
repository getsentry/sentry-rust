# Lightweight Runtime Metrics Design

## Overview

This document proposes a design for **lightweight runtime metrics** that SDKs can automatically collect for framework, language, or platform-specific health signals. These metrics focus specifically on signals that **do NOT overlap with Tracing** and provide a quick sense of app or runtime health.

Deeper investigation should still rely on traces or profiles.

---

## Goals

1. **Non-overlapping with Tracing**: Capture health signals that traces don't naturally provide
2. **Lightweight**: Minimal overhead, no complex aggregation required
3. **Automatic collection**: Integrations collect metrics without user intervention
4. **Platform-specific**: Tailored metrics for different runtimes (Rust async, mobile, etc.)
5. **Health-oriented**: Surface potential issues like memory pressure, event loop delays, or app hangs

---

## Metric Categories

### 1. Runtime Health Metrics (Rust-specific)

| Metric Name | Type | Unit | Description |
|-------------|------|------|-------------|
| `runtime.memory.heap_used` | Gauge | bytes | Current heap memory usage |
| `runtime.memory.heap_allocated` | Gauge | bytes | Total allocated heap memory |
| `runtime.memory.rss` | Gauge | bytes | Resident Set Size |
| `runtime.gc.collection_count` | Counter | count | Number of GC collections (if applicable) |
| `runtime.threads.active` | Gauge | count | Number of active threads |
| `runtime.threads.peak` | Gauge | count | Peak thread count since start |

### 2. Async Runtime Metrics (Tokio, async-std)

| Metric Name | Type | Unit | Description |
|-------------|------|------|-------------|
| `async.tasks.spawned` | Counter | count | Total tasks spawned |
| `async.tasks.active` | Gauge | count | Currently active tasks |
| `async.tasks.queued` | Gauge | count | Tasks waiting in queue |
| `async.poll.mean_duration` | Gauge | microseconds | Average task poll duration |
| `async.poll.slow_count` | Counter | count | Polls exceeding threshold (e.g., >100μs) |
| `async.blocking.active` | Gauge | count | Active blocking tasks |
| `async.io.pending_ops` | Gauge | count | Pending I/O operations |

### 3. Process/System Metrics

| Metric Name | Type | Unit | Description |
|-------------|------|------|-------------|
| `process.cpu.usage` | Gauge | percent | CPU usage percentage |
| `process.cpu.user_time` | Counter | milliseconds | User CPU time |
| `process.cpu.system_time` | Counter | milliseconds | System CPU time |
| `process.open_fds` | Gauge | count | Open file descriptors |
| `process.uptime` | Gauge | seconds | Process uptime |

### 4. Framework-Specific Metrics

#### Web Frameworks (Actix, Axum, Tower)

| Metric Name | Type | Unit | Description |
|-------------|------|------|-------------|
| `http.connections.active` | Gauge | count | Active HTTP connections |
| `http.connections.idle` | Gauge | count | Idle connections in pool |
| `http.requests.queued` | Gauge | count | Requests waiting to be processed |

#### Database/ORM (SQLx, Diesel, etc.)

| Metric Name | Type | Unit | Description |
|-------------|------|------|-------------|
| `db.pool.connections.active` | Gauge | count | Active DB connections |
| `db.pool.connections.idle` | Gauge | count | Idle connections in pool |
| `db.pool.connections.waiting` | Gauge | count | Requests waiting for connection |
| `db.pool.size` | Gauge | count | Total pool size |

---

## Protocol Design

### New Envelope Item Type: `runtime_metrics`

```rust
/// Runtime metrics item for the envelope.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuntimeMetrics {
    /// Timestamp when metrics were collected
    #[serde(with = "ts_rfc3339")]
    pub timestamp: SystemTime,
    
    /// The runtime/platform identifier
    pub platform: String,
    
    /// Collection of metric values
    pub metrics: Vec<RuntimeMetric>,
}

/// A single runtime metric measurement
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuntimeMetric {
    /// Metric name following the naming convention
    pub name: String,
    
    /// Metric type
    pub r#type: MetricType,
    
    /// The metric value
    pub value: MetricValue,
    
    /// Unit of measurement
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    
    /// Optional tags for additional context
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub tags: Map<String, String>,
}

/// The type of metric being recorded
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricType {
    /// A point-in-time value (e.g., current memory usage)
    Gauge,
    /// A monotonically increasing value (e.g., total requests)
    Counter,
    /// A distribution of values (e.g., latencies)
    Distribution,
}

/// Metric value representation
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MetricValue {
    Int(i64),
    Float(f64),
}
```

### Envelope Item Header

```json
{
  "type": "runtime_metrics",
  "content_type": "application/json"
}
```

### Example Payload

```json
{
  "timestamp": "2026-01-21T10:30:00.000Z",
  "platform": "rust",
  "metrics": [
    {
      "name": "runtime.memory.heap_used",
      "type": "gauge",
      "value": 52428800,
      "unit": "bytes"
    },
    {
      "name": "async.tasks.active",
      "type": "gauge", 
      "value": 42,
      "tags": {
        "runtime": "tokio"
      }
    },
    {
      "name": "async.poll.slow_count",
      "type": "counter",
      "value": 15,
      "tags": {
        "runtime": "tokio",
        "threshold_us": "100"
      }
    }
  ]
}
```

---

## Implementation Design

### New Integration: `RuntimeMetricsIntegration`

```rust
/// Configuration for runtime metrics collection
#[derive(Debug, Clone)]
pub struct RuntimeMetricsConfig {
    /// Collection interval (default: 10 seconds)
    pub collection_interval: Duration,
    
    /// Enable memory metrics (default: true)
    pub collect_memory: bool,
    
    /// Enable process metrics (default: true)
    pub collect_process: bool,
    
    /// Enable async runtime metrics (default: true, if tokio feature enabled)
    pub collect_async_runtime: bool,
    
    /// Threshold for slow poll detection in microseconds (default: 100)
    pub slow_poll_threshold_us: u64,
    
    /// Custom metric collectors
    pub custom_collectors: Vec<Arc<dyn MetricCollector>>,
}

/// Trait for custom metric collectors
pub trait MetricCollector: Send + Sync + 'static {
    /// Collect metrics and return them
    fn collect(&self) -> Vec<RuntimeMetric>;
    
    /// Name of this collector for debugging
    fn name(&self) -> &'static str;
}

/// The runtime metrics integration
pub struct RuntimeMetricsIntegration {
    config: RuntimeMetricsConfig,
    // Internal state for collection
    collector_handle: Option<JoinHandle<()>>,
}

impl Integration for RuntimeMetricsIntegration {
    fn name(&self) -> &'static str {
        "runtime-metrics"
    }
    
    fn setup(&self, options: &mut ClientOptions) {
        // Start background collection task
    }
}
```

### Collector Implementations

#### Memory Collector (using `jemalloc` or system allocator stats)

```rust
pub struct MemoryMetricsCollector;

impl MetricCollector for MemoryMetricsCollector {
    fn collect(&self) -> Vec<RuntimeMetric> {
        let mut metrics = Vec::new();
        
        // Using jemalloc stats if available
        #[cfg(feature = "jemalloc")]
        {
            if let Some(stats) = get_jemalloc_stats() {
                metrics.push(RuntimeMetric {
                    name: "runtime.memory.heap_allocated".into(),
                    r#type: MetricType::Gauge,
                    value: MetricValue::Int(stats.allocated as i64),
                    unit: Some("bytes".into()),
                    tags: Default::default(),
                });
            }
        }
        
        // System memory via /proc/self/statm on Linux
        #[cfg(target_os = "linux")]
        {
            if let Ok(rss) = get_rss_bytes() {
                metrics.push(RuntimeMetric {
                    name: "runtime.memory.rss".into(),
                    r#type: MetricType::Gauge,
                    value: MetricValue::Int(rss),
                    unit: Some("bytes".into()),
                    tags: Default::default(),
                });
            }
        }
        
        metrics
    }
    
    fn name(&self) -> &'static str {
        "memory"
    }
}
```

#### Tokio Runtime Collector

```rust
#[cfg(feature = "tokio")]
pub struct TokioMetricsCollector {
    runtime_handle: tokio::runtime::Handle,
}

impl MetricCollector for TokioMetricsCollector {
    fn collect(&self) -> Vec<RuntimeMetric> {
        let mut metrics = Vec::new();
        
        // Requires tokio's "rt" feature with metrics enabled
        #[cfg(tokio_unstable)]
        {
            let metrics_data = self.runtime_handle.metrics();
            
            metrics.push(RuntimeMetric {
                name: "async.workers.count".into(),
                r#type: MetricType::Gauge,
                value: MetricValue::Int(metrics_data.num_workers() as i64),
                unit: Some("count".into()),
                tags: [("runtime".into(), "tokio".into())].into(),
            });
            
            metrics.push(RuntimeMetric {
                name: "async.blocking.threads".into(),
                r#type: MetricType::Gauge,
                value: MetricValue::Int(metrics_data.num_blocking_threads() as i64),
                unit: Some("count".into()),
                tags: [("runtime".into(), "tokio".into())].into(),
            });
            
            // Aggregate across all workers
            let total_polls: u64 = (0..metrics_data.num_workers())
                .map(|i| metrics_data.worker_poll_count(i))
                .sum();
                
            metrics.push(RuntimeMetric {
                name: "async.polls.total".into(),
                r#type: MetricType::Counter,
                value: MetricValue::Int(total_polls as i64),
                tags: [("runtime".into(), "tokio".into())].into(),
                unit: None,
            });
        }
        
        metrics
    }
    
    fn name(&self) -> &'static str {
        "tokio"
    }
}
```

---

## Crate Structure

```
sentry-runtime-metrics/
├── Cargo.toml
├── README.md
└── src/
    ├── lib.rs              # Main integration
    ├── config.rs           # Configuration types
    ├── protocol.rs         # Protocol types (RuntimeMetrics, etc.)
    ├── collector.rs        # MetricCollector trait
    └── collectors/
        ├── mod.rs
        ├── memory.rs       # Memory metrics
        ├── process.rs      # Process/CPU metrics  
        ├── tokio.rs        # Tokio runtime metrics
        └── async_std.rs    # async-std metrics
```

### Cargo.toml Features

```toml
[package]
name = "sentry-runtime-metrics"
version = "0.1.0"
edition = "2021"

[features]
default = ["memory", "process"]
memory = []
process = []
tokio = ["dep:tokio"]
async-std = ["dep:async-std"]
jemalloc = ["dep:tikv-jemalloc-ctl"]

[dependencies]
sentry-core = { version = "0.38", path = "../sentry-core" }
sentry-types = { version = "0.38", path = "../sentry-types" }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Optional dependencies
tokio = { version = "1", optional = true, features = ["rt"] }
async-std = { version = "1", optional = true }
tikv-jemalloc-ctl = { version = "0.5", optional = true }

# Platform-specific
[target.'cfg(unix)'.dependencies]
libc = "0.2"

[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.52", features = ["Win32_System_ProcessStatus"] }
```

---

## Collection Strategy

### Background Task Approach

```rust
impl RuntimeMetricsIntegration {
    fn start_collection(&self, client: Arc<Client>) {
        let config = self.config.clone();
        let collectors = self.build_collectors();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.collection_interval);
            
            loop {
                interval.tick().await;
                
                let metrics: Vec<RuntimeMetric> = collectors
                    .iter()
                    .flat_map(|c| c.collect())
                    .collect();
                
                if !metrics.is_empty() {
                    let runtime_metrics = RuntimeMetrics {
                        timestamp: SystemTime::now(),
                        platform: "rust".into(),
                        metrics,
                    };
                    
                    // Send via envelope
                    client.send_runtime_metrics(runtime_metrics);
                }
            }
        });
    }
}
```

### On-Event Attachment (Alternative)

For lower overhead, metrics can be attached to events/transactions:

```rust
impl Integration for RuntimeMetricsIntegration {
    fn process_event(
        &self,
        mut event: Event<'static>,
        _options: &ClientOptions,
    ) -> Option<Event<'static>> {
        // Attach current metrics snapshot to event context
        let metrics = self.collect_snapshot();
        
        event.contexts.insert(
            "runtime_metrics".into(),
            Context::Other(metrics.to_context_map()),
        );
        
        Some(event)
    }
}
```

---

## Naming Conventions

Following OpenTelemetry semantic conventions where applicable:

| Prefix | Domain |
|--------|--------|
| `runtime.*` | Language runtime (memory, GC, threads) |
| `async.*` | Async runtime (tasks, polls, workers) |
| `process.*` | OS process level (CPU, FDs, uptime) |
| `http.*` | HTTP server/client |
| `db.*` | Database connections |
| `{framework}.*` | Framework-specific (e.g., `actix.*`) |

---

## Comparison: Metrics vs. Tracing Coverage

| Signal | Covered by Tracing? | Covered by Metrics? | Notes |
|--------|---------------------|---------------------|-------|
| Request latency | ✅ Yes | ❌ No | Use trace spans |
| Error rates | ✅ Yes | ❌ No | Use trace status |
| Memory pressure | ❌ No | ✅ Yes | Point-in-time health |
| Task queue depth | ❌ No | ✅ Yes | Runtime health |
| Thread count | ❌ No | ✅ Yes | Resource utilization |
| Connection pool state | ❌ No | ✅ Yes | Resource health |
| Slow poll detection | ❌ No | ✅ Yes | Runtime problems |
| GC pressure | ❌ No | ✅ Yes | Runtime health |
| CPU usage | ❌ No | ✅ Yes | Resource utilization |

---

## Example Usage

```rust
use sentry::{init, ClientOptions};
use sentry_runtime_metrics::{RuntimeMetricsIntegration, RuntimeMetricsConfig};

fn main() {
    let _guard = init(ClientOptions::new()
        .add_integration(RuntimeMetricsIntegration::new(
            RuntimeMetricsConfig {
                collection_interval: Duration::from_secs(10),
                collect_memory: true,
                collect_process: true,
                collect_async_runtime: true,
                slow_poll_threshold_us: 100,
                ..Default::default()
            }
        ))
    );
    
    // Application code...
}
```

---

## Mobile/Cross-Platform Considerations

For mobile SDKs (not Rust-specific but for reference):

### iOS Metrics
- `app.memory.footprint` - App memory footprint
- `app.memory.physical` - Physical memory used
- `app.cpu.usage` - CPU usage percentage
- `app.thermal.state` - Device thermal state
- `app.battery.level` - Battery level (if available)
- `ui.main_thread.hang_duration` - Main thread hang detection

### Android Metrics
- `app.memory.java_heap` - Java heap usage
- `app.memory.native_heap` - Native heap usage
- `app.cpu.usage` - CPU usage
- `app.anr.pending` - Potential ANR detection
- `ui.frame.slow_count` - Slow frame counter
- `ui.frame.frozen_count` - Frozen frame counter

---

## Open Questions

1. **Collection Interval**: Should this be configurable per-collector or global?
2. **Cardinality**: How to handle high-cardinality tags (e.g., per-endpoint metrics)?
3. **Batching**: Should metrics be batched or sent immediately?
4. **Sampling**: Should metrics respect the same sample rate as events?
5. **Storage**: How long should Sentry retain these metrics?

---

## Next Steps

1. [ ] Implement `RuntimeMetric` and `RuntimeMetrics` protocol types in `sentry-types`
2. [ ] Add `runtime_metrics` envelope item type
3. [ ] Create `sentry-runtime-metrics` crate with core infrastructure
4. [ ] Implement memory collector
5. [ ] Implement process metrics collector
6. [ ] Add Tokio runtime collector (behind feature flag)
7. [ ] Integration tests
8. [ ] Documentation and examples

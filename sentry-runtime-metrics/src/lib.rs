//! Lightweight runtime metrics collection for Sentry.
//!
//! This crate provides automatic collection of runtime health metrics that
//! do NOT overlap with tracing, giving a quick sense of app/runtime health.
//!
//! # Overview
//!
//! The metrics collected focus on:
//! - Memory usage (heap, RSS)
//! - Process metrics (CPU, threads, file descriptors)
//! - Async runtime health (Tokio task counts, poll durations)
//!
//! # Usage
//!
//! ```rust,ignore
//! use sentry::ClientOptions;
//! use sentry_runtime_metrics::{RuntimeMetricsIntegration, RuntimeMetricsConfig};
//! use std::time::Duration;
//!
//! let _guard = sentry::init(ClientOptions::new()
//!     .add_integration(RuntimeMetricsIntegration::new(RuntimeMetricsConfig {
//!         collection_interval: Duration::from_secs(10),
//!         ..Default::default()
//!     }))
//! );
//! ```

#![doc(html_favicon_url = "https://sentry-brand.storage.googleapis.com/favicon.ico")]
#![doc(html_logo_url = "https://sentry-brand.storage.googleapis.com/sentry-glyph-black.png")]
#![warn(missing_docs)]

mod collector;
mod config;
mod integration;
mod protocol;

pub mod collectors;

pub use collector::MetricCollector;
pub use config::RuntimeMetricsConfig;
pub use integration::RuntimeMetricsIntegration;
pub use protocol::{MetricType, MetricValue, RuntimeMetric, RuntimeMetrics};

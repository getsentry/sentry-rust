//! APIs for creating and capturing metrics.
//!
//! With [Sentry's Application Metrics], you can record [counters], [gauges], and
//! [distributions] from application code. This module is available when the `metrics` feature is
//! enabled. Captured metrics are sent through the current [`Hub`] and are associated with the
//! current trace and active span when available. The SDK also attaches [default metric attributes].
//!
//! Counters are unitless. Gauges and distributions support units via `unit`.
//!
//! For more information, see the [Rust SDK metrics guide].
//!
//! [Sentry's Application Metrics]: https://docs.sentry.io/product/explore/metrics/
//! [Application Metrics]: https://docs.sentry.io/product/explore/metrics/
//! [counters]: https://docs.sentry.io/product/explore/metrics/#counters
//! [gauges]: https://docs.sentry.io/product/explore/metrics/#gauges
//! [distributions]: https://docs.sentry.io/product/explore/metrics/#distributions
//! [default metric attributes]: https://docs.sentry.io/platforms/rust/metrics/#default-attributes
//! [Rust SDK metrics guide]: https://docs.sentry.io/platforms/rust/metrics/
//!
//! # Examples
//!
//! Capture counters, gauges, and distributions:
//!
//! ```rust
//! use sentry::metrics;
//! use sentry::protocol::Unit;
//!
//! metrics::counter("http.requests", 1).capture();
//! metrics::gauge("queue.depth", 42).capture();
//! metrics::distribution("http.response_time", 187.5)
//!     .unit(Unit::Millisecond)
//!     .capture();
//! ```
//!
//! Add attributes that can be used to filter and group metrics in Sentry:
//!
//! ```rust
//! use sentry::metrics;
//!
//! metrics::counter("http.requests", 1)
//!     .attribute("http.route", "/health")
//!     .attribute("http.response.status_code", 200)
//!     .capture();
//! ```

use std::collections::BTreeMap;
use std::{borrow::Cow, time::SystemTime};

use sentry_types::protocol::v7::{
    LogAttribute, Metric as ProtocolMetric, MetricType, SpanId, TraceId, Unit,
};

#[cfg(any(doc, feature = "client"))]
use crate::Hub;

/// Creates a counter metric, with the given name and value.
///
/// Use counters for occurrences, such as handled requests or processed jobs. You may set
/// attributes via [`CounterMetric::attribute`].
///
/// Unlike [`gauge`] and [`distribution`] metrics, counters are always unitless.
///
/// # Example
///
/// ```rust
/// use sentry::metrics;
///
/// metrics::counter("http.requests", 1).capture();
/// ```
pub fn counter<N, V>(name: N, value: V) -> CounterMetric
where
    N: Into<Cow<'static, str>>,
    V: Into<f64>,
{
    MetricInner::new(name, value).into()
}

/// Creates a gauge metric, with the given name and value.
///
/// Use gauges for current state, such as queue depth or active connections. Set the unit on the
/// metric via [`GaugeMetric::unit`] where applicable. You may also set attributes with
/// [`GaugeMetric::attribute`].
///
/// # Example
///
/// ```rust
/// use sentry::metrics;
///
/// metrics::gauge("queue.depth", 42).capture();
/// ```
pub fn gauge<N, V>(name: N, value: V) -> GaugeMetric
where
    N: Into<Cow<'static, str>>,
    V: Into<f64>,
{
    MetricInner::new(name, value).into()
}

/// Creates a distribution metric, with the given name and value.
///
/// Use distributions for values that need statistical analysis, such as response time or payload
/// size. Set the unit on the metric via [`DistributionMetric::unit`] where applicable. You may also
/// set attributes with [`DistributionMetric::attribute`].
///
/// # Example
///
/// ```rust
/// use sentry::metrics;
/// use sentry::protocol::Unit;
///
/// metrics::distribution("http.response_time", 187.5)
///     .unit(Unit::Millisecond)
///     .capture();
/// ```
pub fn distribution<N, V>(name: N, value: V) -> DistributionMetric
where
    N: Into<Cow<'static, str>>,
    V: Into<f64>,
{
    MetricInner::new(name, value).into()
}

/// A counter metric, created with [`counter`].
#[must_use = "metrics must be captured via `.capture()` to be sent to Sentry"]
pub struct CounterMetric {
    inner: MetricInner,
}

/// A gauge metric, created with [`gauge`].
#[must_use = "metrics must be captured via `.capture()` to be sent to Sentry"]
pub struct GaugeMetric {
    inner: UnitMetricInner,
}

/// A distribution metric, created with [`distribution`].
#[must_use = "metrics must be captured via `.capture()` to be sent to Sentry"]
pub struct DistributionMetric {
    inner: UnitMetricInner,
}

/// Marker trait for types which can be converted to a [protocol metric](ProtocolMetric),
/// allowing [`Hub::capture_metric`] to capture and send them to Sentry.
///
/// This trait is sealed and cannot be implemented outside this crate.
pub trait IntoProtocolMetric: sealed::IntoProtocolMetricImpl {}

/// Implement metric methods common to all metric types.
///
/// This includes the `attribute` and the `capture` functions.
macro_rules! implement_metric_common_methods {
    ($struct:ident, $metric_type:expr) => {
        impl $struct {
            /// Adds an attribute to the metric.
            ///
            /// Attributes are keys that can be used to filter and group metrics in Sentry. Multiple
            /// attributes can be chained. We recommend using [Sentry semantic conventions] for key
            /// values, where applicable.
            ///
            /// [Sentry semantic conventions]: https://getsentry.github.io/sentry-conventions/
            pub fn attribute<K, V>(self, key: K, value: V) -> Self
            where
                K: Into<Cow<'static, str>>,
                V: Into<LogAttribute>,
            {
                let inner = self.inner.attribute(key, value);
                Self { inner }
            }

            /// Captures the metric on the current [`Hub`], sending it to Sentry.
            ///
            /// If the current hub has no client bound, the metric is dropped. To capture on a
            /// different hub, use [`Hub::capture_metric`].
            #[inline]
            pub fn capture(self) {
                with_client_impl! {{
                    Hub::current().capture_metric(self)
                }}
            }
        }

        impl IntoProtocolMetric for $struct {}

        impl sealed::IntoProtocolMetricImpl for $struct {
            #[expect(private_interfaces)] // Not actually a public API
            fn into_protocol_metric(self, trace: MetricTraceInfo) -> ProtocolMetric {
                self.inner.into_protocol_metric($metric_type, trace)
            }
        }
    };
}

/// Implements the `unit` method for a given metric type.
macro_rules! implement_unit {
    ($struct:ident) => {
        impl $struct {
            /// Sets the unit on the metric.
            pub fn unit<U>(self, unit: U) -> Self
            where
                U: Into<Unit>,
            {
                let inner = self.inner.unit(unit);
                Self { inner }
            }
        }
    };
}

implement_metric_common_methods!(CounterMetric, MetricType::Counter);
implement_metric_common_methods!(GaugeMetric, MetricType::Gauge);
implement_metric_common_methods!(DistributionMetric, MetricType::Distribution);

implement_unit!(GaugeMetric);
implement_unit!(DistributionMetric);

/// Information that links a metric to a trace.
pub(crate) struct MetricTraceInfo {
    pub(crate) trace_id: TraceId,
    pub(crate) span_id: Option<SpanId>,
}

/// Common data that all metrics share.
///
/// Includes the metric type, name, value, and attributes.
struct MetricInner {
    name: Cow<'static, str>,
    value: f64,
    attributes: BTreeMap<Cow<'static, str>, LogAttribute>,
}

/// Common data that metrics, which support units, share.
///
/// Includes everything from [`MetricInner`] plus an optional [`Unit`].
struct UnitMetricInner {
    metric_inner: MetricInner,
    unit: Option<Unit>,
}

impl MetricInner {
    fn new<N, V>(name: N, value: V) -> Self
    where
        N: Into<Cow<'static, str>>,
        V: Into<f64>,
    {
        let name = name.into();
        let value = value.into();

        Self {
            name,
            value,
            attributes: Default::default(),
        }
    }

    fn attribute<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<Cow<'static, str>>,
        V: Into<LogAttribute>,
    {
        self.attributes.insert(key.into(), value.into());
        self
    }

    fn into_protocol_metric(self, r#type: MetricType, trace: MetricTraceInfo) -> ProtocolMetric {
        let Self {
            name,
            value,
            attributes,
        } = self;
        let MetricTraceInfo { trace_id, span_id } = trace;

        ProtocolMetric {
            r#type,
            trace_id,
            name,
            value,
            attributes,
            span_id,
            timestamp: SystemTime::now(),
            unit: None,
        }
    }
}

impl UnitMetricInner {
    fn attribute<K, V>(self, key: K, value: V) -> Self
    where
        K: Into<Cow<'static, str>>,
        V: Into<LogAttribute>,
    {
        Self {
            metric_inner: self.metric_inner.attribute(key, value),
            ..self
        }
    }

    fn unit<U>(self, unit: U) -> Self
    where
        U: Into<Unit>,
    {
        let unit = Some(unit.into());
        Self { unit, ..self }
    }

    fn into_protocol_metric(self, r#type: MetricType, trace: MetricTraceInfo) -> ProtocolMetric {
        ProtocolMetric {
            unit: self.unit,
            ..self.metric_inner.into_protocol_metric(r#type, trace)
        }
    }
}

impl From<MetricInner> for CounterMetric {
    #[inline]
    fn from(inner: MetricInner) -> Self {
        Self { inner }
    }
}

impl From<MetricInner> for GaugeMetric {
    #[inline]
    fn from(inner: MetricInner) -> Self {
        let inner = inner.into();
        Self { inner }
    }
}

impl From<MetricInner> for DistributionMetric {
    #[inline]
    fn from(inner: MetricInner) -> Self {
        let inner = inner.into();
        Self { inner }
    }
}

impl From<MetricInner> for UnitMetricInner {
    #[inline]
    fn from(metric_inner: MetricInner) -> Self {
        Self {
            metric_inner,
            unit: None,
        }
    }
}

/// Private module for used to prevent [`IntoProtocolMetric`] from being implemented outside this
/// crate, and for keeping its implementation details private.
mod sealed {
    use sentry_types::protocol::v7::Metric as ProtocolMetric;

    use crate::metrics::MetricTraceInfo;

    #[cfg(doc)]
    use super::IntoProtocolMetric;

    /// Actual implementation of [`IntoProtocolMetric`]
    pub trait IntoProtocolMetricImpl {
        /// Converts this item into a [`ProtocolMetric`], with the given [`MetricTraceInfo`].
        #[expect(private_interfaces)] // This trait is not actually publicly accessible.
        fn into_protocol_metric(self, trace: MetricTraceInfo) -> ProtocolMetric;
    }
}

//! APIs for creating and capturing metrics.

use std::collections::BTreeMap;
use std::{borrow::Cow, time::SystemTime};

use sentry_types::protocol::v7::{
    LogAttribute, Metric as ProtocolMetric, MetricType, SpanId, TraceId, Unit,
};

use crate::Hub;

/// Creates a counter metric, with the given name and value.
///
/// You may set attributes via [`CounterMetric::attribute`].
///
/// Note that unlike [`gauge`] and [`distribution`] metrics, counters are always unitless.
pub fn counter<N, V>(name: N, value: V) -> CounterMetric
where
    N: Into<Cow<'static, str>>,
    V: Into<f64>,
{
    MetricInner::new(name, value).into()
}

/// Creates a gauge metric, with the given name and value.
///
/// It is recommended to set the unit on the metric via [`GaugeMetric::unit`]. You may also set
/// attributes with [`GaugeMetric::attribute`].
pub fn gauge<N, V>(name: N, value: V) -> GaugeMetric
where
    N: Into<Cow<'static, str>>,
    V: Into<f64>,
{
    MetricInner::new(name, value).into()
}

/// Creates a distribution metric, with the given name and value.
///
/// It is recommended to set the unit on the metric via [`DistributionMetric::unit`]. You may also set
/// attributes with [`DistributionMetric::attribute`].
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
            /// If the current hub has no client, the metric is dropped. To capture on a different
            /// hub, use [`Hub::capture_metric`].
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

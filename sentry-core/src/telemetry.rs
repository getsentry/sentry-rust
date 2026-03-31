#![cfg(feature = "metrics")]

//! Telemetry types
//!
//! This module contains types for telemetry data that can be sent to Sentry using the Sentry SDK.
//! Currently, this is limited to metrics-related types, but more may be added in the future.
//!
//! ### Difference versus [`sentry_types::protocol`]
//!
//! The types in [`sentry_types::protocol`] are primarily meant to be used internally by the SDK.
//! We expose the types publicly to allow them to be filtered and modified arbitrarily in
//! `before_send*` callbacks. However, for most users, we recommend sticking with the types
//! defined here, as they are more restrictive, and will help ensure the data you send is valid.

use std::borrow::Cow;
use std::collections::BTreeMap;

use crate::protocol::{LogAttribute, Metric as ProtocolMetric, MetricType};
use crate::Scope;

/// User-facing telemetry types.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Metric {
    metric_type: MetricType,
    name: Cow<'static, str>,
    value: f64,
    unit: Option<Cow<'static, str>>,
    attributes: BTreeMap<Cow<'static, str>, LogAttribute>,
}

impl Metric {
    /// Creates a counter metric.
    ///
    /// The default value is `1.0`; override with [`Self::value`].
    ///
    /// ```rust
    /// use sentry_core::telemetry::Metric;
    /// let metric = Metric::counter("counter.one");
    /// ```
    ///
    /// ```rust
    /// use sentry_core::telemetry::Metric;
    /// let metric = Metric::counter("counter.forty-two").with_value(42.0);
    /// ```
    pub fn counter<S>(name: S) -> Self
    where
        S: Into<Cow<'static, str>>,
    {
        Self::new(MetricType::Counter, name, 1.0)
    }

    /// Creates a gauge metric.
    ///
    /// The default value is `0.0`; override with [`Self::value`].
    ///
    /// ```rust
    /// use sentry_core::telemetry::Metric;
    /// let metric = Metric::gauge("gauge.ten").with_value(10.0);
    /// ```
    ///
    /// ```rust
    /// use sentry_core::telemetry::Metric;
    /// let metric = Metric::gauge("gauge.minus.ten").with_value(-10.0);
    /// ```
    pub fn gauge<S>(name: S) -> Self
    where
        S: Into<Cow<'static, str>>,
    {
        Self::new(MetricType::Gauge, name, 0.0)
    }

    /// Creates a distribution metric.
    ///
    /// The default value is `0.0`; override with [`Self::value`].
    ///
    /// ```rust
    /// use sentry_core::telemetry::Metric;
    /// let metric = Metric::distribution("distribution.ten").with_value(10.0);
    /// ```
    ///
    /// ```rust
    /// use sentry_core::telemetry::Metric;
    /// let metric = Metric::distribution("distribution.minus.ten").with_value(-10.0);
    pub fn distribution<S>(name: S) -> Self
    where
        S: Into<Cow<'static, str>>,
    {
        Self::new(MetricType::Distribution, name, 0.0)
    }

    /// Set the metric's value.
    ///
    /// This method sets the metric's value to the provided value, after validating the value.
    ///
    /// ### What values are valid?
    ///
    /// Values are valid if they are [finite](`f64::is_finite`). [Counter metrics](Self::counter)
    /// must also be [positive](f64::is_sign_positive) (including +0.0), other metrics take any
    /// finite value.
    ///
    /// ### Panics
    ///
    /// Panics when the `value` is invalid.
    ///
    /// ### Examples
    ///
    /// ```rust
    /// use sentry_core::telemetry::Metric;
    /// let metric = Metric::gauge("foo").with_value(100.0);
    /// ```
    ///
    /// ```rust, should_panic
    /// use sentry_core::telemetry::Metric;
    /// let metric = Metric::counter("oops").with_value(-1.0);
    /// ```
    pub fn with_value(self, value: f64) -> Self {
        assert!(
            self.is_valid_value(value),
            "{value} is an invalid value for {} metrics",
            self.metric_type
        );

        Self { value, ..self }
    }

    /// Set the metric's value, without validating the value.
    ///
    /// ### Safety
    ///
    /// Calling code must ensure the value is valid per the rules in [`Self::with_value`].
    ///
    ///
    /// ### Example
    ///
    /// ```rust
    /// use sentry_core::telemetry::Metric;
    /// let metric = Metric::counter("unsafe");
    ///
    /// // Safety: 1.0 is finite and >= 0.0
    /// let metric = unsafe { metric.with_value_unchecked(1.0) };
    /// ```
    pub unsafe fn with_value_unchecked(self, value: f64) -> Self {
        Self { value, ..self }
    }

    /// Sets the metric unit.
    pub fn with_unit<S>(self, unit: S) -> Self
    where
        S: Into<Cow<'static, str>>,
    {
        let unit = Some(unit.into());
        Self { unit, ..self }
    }

    /// Inserts a metric attribute.
    pub fn add_attribute<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<Cow<'static, str>>,
        V: Into<LogAttribute>,
    {
        self.attributes.insert(key.into(), value.into());
        self
    }

    /// Inserts the provided default attributes.
    ///
    /// Any existing attributes take precedence over the attributes passed.
    pub(crate) fn add_default_attributes<'a, A>(mut self, default_attributes: &'a A)
    where
        &'a A: IntoIterator<Item = (&'a Cow<'static, str>, &'a LogAttribute)>,
    {
        for (k, v) in default_attributes {
            self.attributes
                .entry(k.clone())
                .or_insert_with(|| v.clone());
        }
    }

    /// Converts this [`Metric`] into a [`ProtocolMetric`].
    ///
    /// As a [`ProtocolMetric`] also requires the `trace_id`, and optionally the `span_id`, from
    /// the [`Scope`], this method takes a scope and applies the relevant fields to the returned
    /// [`ProtocolMetric`].
    pub(crate) fn into_protocol_metric(self, scope: &Scope) -> ProtocolMetric {
        let Metric {
            metric_type,
            name,
            value,
            unit,
            attributes,
        } = self;

        let trace_id = scope.trace_id();
        let span_id = scope.get_span().map(|ts| ts.span_id());

        let mut protocol_metric = ProtocolMetric::new(metric_type, name, value, trace_id);
        protocol_metric.unit = unit;
        protocol_metric.attributes = attributes;
        protocol_metric.span_id = span_id;

        protocol_metric
    }

    /// Checks if the provided float value is valid for this metric.
    ///
    /// See [`Self::with_value`] for definition on what's valid.
    fn is_valid_value(&self, value: f64) -> bool {
        value.is_finite() && (self.metric_type != MetricType::Counter || value.is_sign_positive())
    }

    /// Create a new metric with the given type, name, and value.
    ///
    /// Other values are set to their respective defaults.
    fn new<S>(metric_type: MetricType, name: S, value: f64) -> Self
    where
        S: Into<Cow<'static, str>>,
    {
        let name = name.into();

        Self {
            metric_type,
            name,
            value,
            unit: None,
            attributes: BTreeMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::f64;

    use super::*;

    #[test]
    fn counter_defaults() {
        let metric = Metric::counter("counter.one");

        let expected = Metric {
            metric_type: MetricType::Counter,
            name: "counter.one".into(),
            value: 1.0,
            unit: None,
            attributes: BTreeMap::new(),
        };
        assert_eq!(metric, expected);
    }

    #[test]
    fn guage_defaults() {
        let metric = Metric::gauge("gauge.zero");

        let expected = Metric {
            metric_type: MetricType::Gauge,
            name: "gauge.zero".into(),
            value: 0.0,
            unit: None,
            attributes: BTreeMap::new(),
        };
        assert_eq!(metric, expected);
    }

    #[test]
    fn distribution_defaults() {
        let metric = Metric::distribution("distribution.zero");

        let expected = Metric {
            metric_type: MetricType::Distribution,
            name: "distribution.zero".into(),
            value: 0.0,
            unit: None,
            attributes: BTreeMap::new(),
        };
        assert_eq!(metric, expected);
    }

    #[test]
    #[should_panic]
    fn neg_counter_panics() {
        Metric::counter("counter.negone").with_value(-1.0);
    }

    #[test]
    #[should_panic]
    fn negzero_counter_panics() {
        Metric::counter("counter.negzero").with_value(-0.0);
    }

    #[test]
    #[should_panic]
    fn nan_counter_panics() {
        Metric::counter("counter.nan").with_value(f64::NAN);
    }

    #[test]
    #[should_panic]
    fn nan_gauge_panics() {
        Metric::gauge("gauge.nan").with_value(f64::NAN);
    }

    #[test]
    #[should_panic]
    fn nan_distribution_panics() {
        Metric::distribution("distribution.nan").with_value(f64::NAN);
    }

    #[test]
    #[should_panic]
    fn inf_counter_panics() {
        Metric::counter("counter.inf").with_value(f64::INFINITY);
    }

    #[test]
    #[should_panic]
    fn inf_gauge_panics() {
        Metric::gauge("gauge.inf").with_value(f64::INFINITY);
    }

    #[test]
    #[should_panic]
    fn inf_distribution_panics() {
        Metric::distribution("distribution.inf").with_value(f64::INFINITY);
    }

    #[test]
    #[should_panic]
    fn neg_inf_counter_panics() {
        Metric::counter("counter.neginf").with_value(f64::NEG_INFINITY);
    }

    #[test]
    #[should_panic]
    fn neg_inf_gauge_panics() {
        Metric::gauge("gauge.neginf").with_value(f64::NEG_INFINITY);
    }

    #[test]
    #[should_panic]
    fn neg_inf_distribution_panics() {
        Metric::distribution("distribution.neginf").with_value(f64::NEG_INFINITY);
    }
}

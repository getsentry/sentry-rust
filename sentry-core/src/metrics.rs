//! Macros for Sentry [trace metrics](https://develop.sentry.dev/sdk/telemetry/metrics/).

// Helper macro to emit a metric at the given type. Should not be used directly.
#[doc(hidden)]
#[macro_export]
macro_rules! metric_emit {
    // Name and value only
    ($type:expr, $name:expr, $value:expr) => {{
        let metric = $crate::protocol::TraceMetric {
            r#type: $type,
            name: $name.to_owned(),
            value: $value as f64,
            timestamp: ::std::time::SystemTime::now(),
            trace_id: $crate::protocol::TraceId::default(),
            span_id: None,
            unit: None,
            attributes: $crate::protocol::Map::new(),
        };
        $crate::Hub::current().capture_metric(metric)
    }};

    // Attributes entrypoint
    ($type:expr, $name:expr, $value:expr, $($rest:tt)+) => {{
        let mut attributes = $crate::protocol::Map::new();
        let mut unit: Option<String> = None;
        $crate::metric_emit!(@internal attributes, unit, $type, $name, $value, $($rest)+)
    }};

    // Recursive case: unit = value, followed by more
    (@internal $attrs:ident, $unit:ident, $type:expr, $name:expr, $value:expr, unit = $uval:expr, $($rest:tt)+) => {{
        $unit = Some($uval.to_owned());
        $crate::metric_emit!(@internal $attrs, $unit, $type, $name, $value, $($rest)+)
    }};

    // Base case: unit = value (last pair)
    (@internal $attrs:ident, $unit:ident, $type:expr, $name:expr, $value:expr, unit = $uval:expr) => {{
        $unit = Some($uval.to_owned());
        let metric = $crate::protocol::TraceMetric {
            r#type: $type,
            name: $name.to_owned(),
            value: $value as f64,
            timestamp: ::std::time::SystemTime::now(),
            trace_id: $crate::protocol::TraceId::default(),
            span_id: None,
            #[allow(clippy::redundant_field_names)]
            unit: $unit,
            #[allow(clippy::redundant_field_names)]
            attributes: $attrs,
        };
        $crate::Hub::current().capture_metric(metric)
    }};

    // Recursive case: quoted key = value, followed by more
    (@internal $attrs:ident, $unit:ident, $type:expr, $name:expr, $value:expr, $key:literal = $aval:expr, $($rest:tt)+) => {{
        $attrs.insert(
            $key.to_owned(),
            $crate::protocol::LogAttribute($crate::protocol::Value::from($aval))
        );
        $crate::metric_emit!(@internal $attrs, $unit, $type, $name, $value, $($rest)+)
    }};

    // Base case: quoted key = value (last pair)
    (@internal $attrs:ident, $unit:ident, $type:expr, $name:expr, $value:expr, $key:literal = $aval:expr) => {{
        $attrs.insert(
            $key.to_owned(),
            $crate::protocol::LogAttribute($crate::protocol::Value::from($aval))
        );
        let metric = $crate::protocol::TraceMetric {
            r#type: $type,
            name: $name.to_owned(),
            value: $value as f64,
            timestamp: ::std::time::SystemTime::now(),
            trace_id: $crate::protocol::TraceId::default(),
            span_id: None,
            #[allow(clippy::redundant_field_names)]
            unit: $unit,
            #[allow(clippy::redundant_field_names)]
            attributes: $attrs,
        };
        $crate::Hub::current().capture_metric(metric)
    }};

    // Recursive case: ident key = value, followed by more
    (@internal $attrs:ident, $unit:ident, $type:expr, $name:expr, $value:expr, $($key:ident).+ = $aval:expr, $($rest:tt)+) => {{
        $attrs.insert(
            stringify!($($key).+).to_owned(),
            $crate::protocol::LogAttribute($crate::protocol::Value::from($aval))
        );
        $crate::metric_emit!(@internal $attrs, $unit, $type, $name, $value, $($rest)+)
    }};

    // Base case: ident key = value (last pair)
    (@internal $attrs:ident, $unit:ident, $type:expr, $name:expr, $value:expr, $($key:ident).+ = $aval:expr) => {{
        $attrs.insert(
            stringify!($($key).+).to_owned(),
            $crate::protocol::LogAttribute($crate::protocol::Value::from($aval))
        );
        let metric = $crate::protocol::TraceMetric {
            r#type: $type,
            name: $name.to_owned(),
            value: $value as f64,
            timestamp: ::std::time::SystemTime::now(),
            trace_id: $crate::protocol::TraceId::default(),
            span_id: None,
            #[allow(clippy::redundant_field_names)]
            unit: $unit,
            #[allow(clippy::redundant_field_names)]
            attributes: $attrs,
        };
        $crate::Hub::current().capture_metric(metric)
    }};
}

/// Emits a counter metric. Counters track event frequency (e.g., requests, errors).
///
/// Attributes can be passed with `key = value` or `"key" = value` syntax.
/// A measurement unit can be set with `unit = "..."`. To set an attribute
/// named "unit", quote the key: `"unit" = "..."`.
///
/// # Examples
///
/// ```
/// use sentry::metric_count;
///
/// // Simple counter
/// metric_count!("api.requests", 1);
///
/// // With attributes
/// metric_count!("api.requests", 1, route = "/users", method = "GET");
///
/// // With unit
/// metric_count!("api.requests", 1, unit = "request");
///
/// // Quoted key to set an attribute named "unit"
/// metric_count!("api.requests", 1, "unit" = "request");
/// ```
#[macro_export]
macro_rules! metric_count {
    ($name:expr, $value:expr $(, $($rest:tt)+)?) => {
        $crate::metric_emit!($crate::protocol::TraceMetricType::Counter, $name, $value $(, $($rest)+)?)
    };
}

/// Emits a gauge metric. Gauges represent current state (e.g., memory usage, pool size).
///
/// Attributes can be passed with `key = value` or `"key" = value` syntax.
/// A measurement unit can be set with `unit = "..."`. To set an attribute
/// named "unit", quote the key: `"unit" = "..."`.
///
/// # Examples
///
/// ```
/// use sentry::metric_gauge;
///
/// metric_gauge!("memory.usage", 1024.0, unit = "byte");
/// ```
#[macro_export]
macro_rules! metric_gauge {
    ($name:expr, $value:expr $(, $($rest:tt)+)?) => {
        $crate::metric_emit!($crate::protocol::TraceMetricType::Gauge, $name, $value $(, $($rest)+)?)
    };
}

/// Emits a distribution metric. Distributions measure statistical spread (e.g., response times).
///
/// Attributes can be passed with `key = value` or `"key" = value` syntax.
/// A measurement unit can be set with `unit = "..."`. To set an attribute
/// named "unit", quote the key: `"unit" = "..."`.
///
/// # Examples
///
/// ```
/// use sentry::metric_distribution;
///
/// metric_distribution!("response.time", 150.0, unit = "millisecond", route = "/users");
/// ```
#[macro_export]
macro_rules! metric_distribution {
    ($name:expr, $value:expr $(, $($rest:tt)+)?) => {
        $crate::metric_emit!($crate::protocol::TraceMetricType::Distribution, $name, $value $(, $($rest)+)?)
    };
}

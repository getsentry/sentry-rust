//! Macros for Sentry [trace metrics](https://develop.sentry.dev/sdk/telemetry/metrics/).

/// Internal macro support for metric emission. Not part of the public API.
#[doc(hidden)]
#[macro_export]
macro_rules! __metric_emit {
    // Name, value, and explicit unit
    ($type:expr, $name:expr, $value:expr, $unit:expr) => {{
        let metric = $crate::protocol::Metric {
            r#type: $type,
            name: $name.to_owned().into(),
            value: $value as f64,
            timestamp: ::std::time::SystemTime::now(),
            trace_id: $crate::protocol::TraceId::default(),
            span_id: None,
            unit: Some(($unit).into()),
            attributes: $crate::protocol::Map::new(),
        };
        $crate::Hub::current().capture_metric(metric)
    }};

    // Name, value, explicit unit, and attributes
    ($type:expr, $name:expr, $value:expr, $unit:expr, $($rest:tt)+) => {{
        let mut attributes = $crate::protocol::Map::new();
        let unit = Some(($unit).into());
        $crate::__metric_emit!(@internal attributes, unit, $type, $name, $value, $($rest)+)
    }};

    // Name and value only
    (@no_unit $type:expr, $name:expr, $value:expr) => {{
        let metric = $crate::protocol::Metric {
            r#type: $type,
            name: $name.to_owned().into(),
            value: $value as f64,
            timestamp: ::std::time::SystemTime::now(),
            trace_id: $crate::protocol::TraceId::default(),
            span_id: None,
            unit: None,
            attributes: $crate::protocol::Map::new(),
        };
        $crate::Hub::current().capture_metric(metric)
    }};

    // Name and value with attributes
    (@no_unit $type:expr, $name:expr, $value:expr, $($rest:tt)+) => {{
        let mut attributes = $crate::protocol::Map::new();
        let unit = None;
        $crate::__metric_emit!(@internal attributes, unit, $type, $name, $value, $($rest)+)
    }};

    // Recursive case: quoted key = value, followed by more
    (@internal $attrs:ident, $unit:ident, $type:expr, $name:expr, $value:expr, $key:literal = $aval:expr, $($rest:tt)+) => {{
        $attrs.insert(
            $key.to_owned().into(),
            $crate::protocol::LogAttribute($crate::protocol::Value::from($aval))
        );
        $crate::__metric_emit!(@internal $attrs, $unit, $type, $name, $value, $($rest)+)
    }};

    // Base case: quoted key = value (last pair)
    (@internal $attrs:ident, $unit:ident, $type:expr, $name:expr, $value:expr, $key:literal = $aval:expr) => {{
        $attrs.insert(
            $key.to_owned().into(),
            $crate::protocol::LogAttribute($crate::protocol::Value::from($aval))
        );
        let metric = $crate::protocol::Metric {
            r#type: $type,
            name: $name.to_owned().into(),
            value: $value as f64,
            timestamp: ::std::time::SystemTime::now(),
            trace_id: $crate::protocol::TraceId::default(),
            span_id: None,
            unit: $unit,
            attributes: $attrs,
        };
        $crate::Hub::current().capture_metric(metric)
    }};

    // Recursive case: ident key = value, followed by more
    (@internal $attrs:ident, $unit:ident, $type:expr, $name:expr, $value:expr, $($key:ident).+ = $aval:expr, $($rest:tt)+) => {{
        $attrs.insert(
            stringify!($($key).+).to_owned().into(),
            $crate::protocol::LogAttribute($crate::protocol::Value::from($aval))
        );
        $crate::__metric_emit!(@internal $attrs, $unit, $type, $name, $value, $($rest)+)
    }};

    // Base case: ident key = value (last pair)
    (@internal $attrs:ident, $unit:ident, $type:expr, $name:expr, $value:expr, $($key:ident).+ = $aval:expr) => {{
        $attrs.insert(
            stringify!($($key).+).to_owned().into(),
            $crate::protocol::LogAttribute($crate::protocol::Value::from($aval))
        );
        let metric = $crate::protocol::Metric {
            r#type: $type,
            name: $name.to_owned().into(),
            value: $value as f64,
            timestamp: ::std::time::SystemTime::now(),
            trace_id: $crate::protocol::TraceId::default(),
            span_id: None,
            unit: $unit,
            attributes: $attrs,
        };
        $crate::Hub::current().capture_metric(metric)
    }};
}

/// Emits a counter metric. Counters track event frequency (e.g., requests, errors).
///
/// Attributes can be passed with `key = value` or `"key" = value` syntax.
///
/// # Examples
///
/// ```
/// use sentry::metric_count;
///
/// metric_count!("api.requests", 1);
/// metric_count!("api.requests", 1, route = "/users", method = "GET");
/// metric_count!("api.requests", 1, "unit" = "request");
/// ```
#[macro_export]
macro_rules! metric_count {
    ($name:expr, $value:expr $(, $($rest:tt)+)?) => {
        $crate::__metric_emit!(@no_unit $crate::protocol::MetricType::Counter, $name, $value $(, $($rest)+)?)
    };
}

/// Emits a gauge metric. Gauges represent current state (e.g., memory usage, pool size).
///
/// Units are optional and, when provided, are passed as a positional argument
/// after the value.
///
/// Attributes can be passed with `key = value` or `"key" = value` syntax.
///
/// # Examples
///
/// ```
/// use sentry::metric_gauge;
///
/// metric_gauge!("memory.usage", 1024.0);
/// metric_gauge!("memory.usage", 1024.0, "byte");
/// metric_gauge!("memory.usage", 1024.0, "byte", host = "web-1");
/// metric_gauge!("memory.usage", 1024.0, unit = "attribute");
/// ```
#[macro_export]
macro_rules! metric_gauge {
    ($name:expr, $value:expr) => {
        $crate::__metric_emit!(@no_unit $crate::protocol::MetricType::Gauge, $name, $value)
    };
    ($name:expr, $value:expr, $key:literal = $aval:expr $(, $($rest:tt)+)?) => {
        $crate::__metric_emit!(@no_unit $crate::protocol::MetricType::Gauge, $name, $value, $key = $aval $(, $($rest)+)?)
    };
    ($name:expr, $value:expr, $($key:ident).+ = $aval:expr $(, $($rest:tt)+)?) => {
        $crate::__metric_emit!(@no_unit $crate::protocol::MetricType::Gauge, $name, $value, $($key).+ = $aval $(, $($rest)+)?)
    };
    ($name:expr, $value:expr, $unit:expr $(, $($rest:tt)+)?) => {
        $crate::__metric_emit!($crate::protocol::MetricType::Gauge, $name, $value, $unit $(, $($rest)+)?)
    };
}

/// Emits a distribution metric. Distributions measure statistical spread (e.g., response times).
///
/// Units are optional and, when provided, are passed as a positional argument
/// after the value.
///
/// Attributes can be passed with `key = value` or `"key" = value` syntax.
///
/// # Examples
///
/// ```
/// use sentry::metric_distribution;
///
/// metric_distribution!("response.time", 150.0);
/// metric_distribution!("response.time", 150.0, "millisecond", route = "/users");
/// metric_distribution!("response.time", 150.0, unit = "attribute");
/// ```
#[macro_export]
macro_rules! metric_distribution {
    ($name:expr, $value:expr) => {
        $crate::__metric_emit!(@no_unit $crate::protocol::MetricType::Distribution, $name, $value)
    };
    ($name:expr, $value:expr, $key:literal = $aval:expr $(, $($rest:tt)+)?) => {
        $crate::__metric_emit!(@no_unit $crate::protocol::MetricType::Distribution, $name, $value, $key = $aval $(, $($rest)+)?)
    };
    ($name:expr, $value:expr, $($key:ident).+ = $aval:expr $(, $($rest:tt)+)?) => {
        $crate::__metric_emit!(@no_unit $crate::protocol::MetricType::Distribution, $name, $value, $($key).+ = $aval $(, $($rest)+)?)
    };
    ($name:expr, $value:expr, $unit:expr $(, $($rest:tt)+)?) => {
        $crate::__metric_emit!($crate::protocol::MetricType::Distribution, $name, $value, $unit $(, $($rest)+)?)
    };
}

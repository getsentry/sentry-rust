//! Macros for Sentry [trace metrics](https://develop.sentry.dev/sdk/telemetry/metrics/).

/// Internal macro support for metric emission. Not part of the public API.
#[doc(hidden)]
#[macro_export]
macro_rules! __metric_emit {
    ($type:expr, $name:expr, $value:expr $(, $($rest:tt)+)?) => {{
        let mut attributes = $crate::protocol::Map::new();
        $crate::__metric_emit!(@parse attributes, $type, $name, $value, None, allow_unit $(, $($rest)+)?)
    }};

    (@attrs_only $type:expr, $name:expr, $value:expr $(, $($rest:tt)+)?) => {{
        let mut attributes = $crate::protocol::Map::new();
        $crate::__metric_emit!(@parse attributes, $type, $name, $value, None, no_unit $(, $($rest)+)?)
    }};

    (@parse $attrs:ident, $type:expr, $name:expr, $value:expr, $unit:expr, $allow_unit:tt) => {{
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

    (@parse $attrs:ident, $type:expr, $name:expr, $value:expr, $unit:expr, $allow_unit:tt, $key:literal = $aval:expr $(, $($rest:tt)+)?) => {{
        $attrs.insert(
            $key.to_owned().into(),
            $crate::protocol::LogAttribute($crate::protocol::Value::from($aval))
        );
        $crate::__metric_emit!(@parse $attrs, $type, $name, $value, $unit, $allow_unit $(, $($rest)+)?)
    }};

    (@parse $attrs:ident, $type:expr, $name:expr, $value:expr, $unit:expr, $allow_unit:tt, $($key:ident).+ = $aval:expr $(, $($rest:tt)+)?) => {{
        $attrs.insert(
            stringify!($($key).+).to_owned().into(),
            $crate::protocol::LogAttribute($crate::protocol::Value::from($aval))
        );
        $crate::__metric_emit!(@parse $attrs, $type, $name, $value, $unit, $allow_unit $(, $($rest)+)?)
    }};

    (@parse $attrs:ident, $type:expr, $name:expr, $value:expr, None, allow_unit, $next:expr $(, $($rest:tt)+)?) => {{
        $crate::__metric_emit!(@parse $attrs, $type, $name, $value, Some(($next).into()), no_unit $(, $($rest)+)?)
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
        $crate::__metric_emit!(@attrs_only $crate::protocol::MetricType::Counter, $name, $value $(, $($rest)+)?)
    };
}

/// Emits a gauge metric. Gauges represent current state (e.g., memory usage, pool size).
///
/// Units are optional and, when provided, are passed as a positional argument
/// after the value and before any attributes.
///
/// Attributes can be passed with `key = value` or `"key" = value` syntax.
///
/// # Examples
///
/// ```
/// use sentry::metric_gauge;
///
/// metric_gauge!("memory.usage", 1024.0);
/// metric_gauge!("memory.usage", 1024.0, host = "web-1", "cache.hit" = true);
/// metric_gauge!("memory.usage", 1024.0, "byte");
/// metric_gauge!("memory.usage", 1024.0, "byte", host = "web-1", region.name = "us-east-1");
/// ```
#[macro_export]
macro_rules! metric_gauge {
    ($name:expr, $value:expr $(, $($rest:tt)+)?) => {
        $crate::__metric_emit!($crate::protocol::MetricType::Gauge, $name, $value $(, $($rest)+)?)
    };
}

/// Emits a distribution metric. Distributions measure statistical spread (e.g., response times).
///
/// Units are optional and, when provided, are passed as a positional argument
/// after the value and before any attributes.
///
/// Attributes can be passed with `key = value` or `"key" = value` syntax.
///
/// # Examples
///
/// ```
/// use sentry::metric_distribution;
///
/// metric_distribution!("response.time", 150.0);
/// metric_distribution!("response.time", 150.0, route = "/users", http.status_code = 200);
/// metric_distribution!("response.time", 150.0, "millisecond");
/// metric_distribution!("response.time", 150.0, "millisecond", route = "/users", "http.status_code" = 200);
/// ```
#[macro_export]
macro_rules! metric_distribution {
    ($name:expr, $value:expr $(, $($rest:tt)+)?) => {
        $crate::__metric_emit!($crate::protocol::MetricType::Distribution, $name, $value $(, $($rest)+)?)
    };
}

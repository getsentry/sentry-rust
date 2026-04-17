//! Macros for Sentry [trace metrics](https://develop.sentry.dev/sdk/telemetry/metrics/).

/// Internal macro support for metric attribute parsing. Not part of the public API.
#[doc(hidden)]
#[macro_export]
macro_rules! __metric_attrs {
    // Finish: no attribute tokens remain.
    ($attrs:ident) => {};

    // Consume one attribute of the form "key" = value, then recurse.
    ($attrs:ident, $key:literal = $aval:expr $(, $($rest:tt)+)?) => {{
        $attrs.insert(
            $key.into(),
            $crate::protocol::LogAttribute($crate::protocol::Value::from($aval))
        );
        $crate::__metric_attrs!($attrs $(, $($rest)+)?);
    }};

    // Consume one attribute of the form foo.bar = value, then recurse.
    ($attrs:ident, $($key:ident).+ = $aval:expr $(, $($rest:tt)+)?) => {{
        $attrs.insert(
            stringify!($($key).+).into(),
            $crate::protocol::LogAttribute($crate::protocol::Value::from($aval))
        );
        $crate::__metric_attrs!($attrs $(, $($rest)+)?);
    }};

    // Anything else is invalid once attribute parsing has started.
    ($attrs:ident, $($rest:tt)+) => {
        compile_error!(concat!(
            "invalid metric attribute syntax near `",
            stringify!($($rest)+),
            "`; expected `\"key\" = value`, `foo = value`, or `foo.bar = value`"
        ));
    };
}

/// Internal macro support for parsing metric unit and attributes.
///
/// This macro sets a variable called `unit` if the second argument is
/// an expression to Some(unit) or to None if the second value is not
/// an expression.
///
/// Not part of the public API.
#[doc(hidden)]
#[macro_export]
macro_rules! __metric_unit_attrs {
    // @no_unit override
    ($attrs:ident, @no_unit $(, $($rest:tt)+)?) => {{
        $crate::__metric_attrs!($attrs $(, $($rest)+)?);
        None
    }};

    // `"key" = value` is not a positional unit. Treat it as an attribute list.
    ($attrs:ident, $key:literal = $value:expr $(, $($rest:tt)+)?) => {{
        $crate::__metric_unit_attrs!($attrs, @no_unit, $key = $value $(, $($rest)+)?)
    }};

    // `key = value` and `foo.bar = value` are valid expression forms, but
    // should be treated as attributes, not as a positional unit.
    ($attrs:ident, $($key:ident).+ = $value:expr $(, $($rest:tt)+)?) => {{
        $crate::__metric_unit_attrs!($attrs, @no_unit, $($key).+ = $value $(, $($rest)+)?)
    }};

    // Unit was passed
    ($attrs:ident, $unit:expr $(, $($rest:tt)+)?) => {{
        $crate::__metric_attrs!($attrs $(, $($rest)+)?);
        Some($unit.into())
    }};

    // No unit passed
    ($attrs:ident $(, $($rest:tt)+)?) => {{
        $crate::__metric_attrs!($attrs $(, $($rest)+)?);
        None
    }};
}

/// Internal macro support for metric emission. Not part of the public API.
#[doc(hidden)]
#[macro_export]
macro_rules! __metric_emit {
    ($type:expr, $name:expr, $value:expr $(, $($rest:tt)+)?) => {{
        let mut attributes = $crate::protocol::Map::new();
        let unit = $crate::__metric_unit_attrs!(attributes $(, $($rest)+)?);
        let metric = $crate::protocol::Metric {
            r#type: $type,
            name: $name.into(),
            value: $value as f64,
            timestamp: ::std::time::SystemTime::now(),
            // trace_id and span_id are added when applying the scope
            trace_id: $crate::protocol::TraceId::default(),
            span_id: None,
            unit,
            attributes,
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
/// # use sentry::metric_count;
/// metric_count!("api.requests", 1);
/// metric_count!("api.requests", 1, route = "/users", method = "GET");
/// ```
///
/// Positional units are not supported for counters:
///
/// ```compile_fail
/// # use sentry::metric_count;
/// metric_count!("api.requests", 1, "count");
/// ```
#[macro_export]
macro_rules! metric_count {
    ($name:expr, $value:expr $(, $($rest:tt)+)?) => {
        $crate::__metric_emit!($crate::protocol::MetricType::Counter, $name, $value, @no_unit $(, $($rest)+)?)
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

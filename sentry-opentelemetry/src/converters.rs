use sentry_core::protocol::{value::Number, SpanId, SpanStatus, TraceId, Value};

pub(crate) fn convert_span_id(span_id: &opentelemetry::SpanId) -> SpanId {
    span_id.to_bytes().into()
}

pub(crate) fn convert_trace_id(trace_id: &opentelemetry::TraceId) -> TraceId {
    trace_id.to_bytes().into()
}

pub(crate) fn convert_span_status(status: &opentelemetry::trace::Status) -> SpanStatus {
    match status {
        opentelemetry::trace::Status::Unset | opentelemetry::trace::Status::Ok => SpanStatus::Ok,
        opentelemetry::trace::Status::Error { description } => {
            description.parse().unwrap_or(SpanStatus::UnknownError)
        }
    }
}

pub(crate) fn convert_span_kind(kind: opentelemetry::trace::SpanKind) -> Value {
    format!("{kind:?}").to_lowercase().into()
}

pub(crate) fn convert_value(value: opentelemetry::Value) -> Value {
    match value {
        opentelemetry::Value::Bool(x) => Value::Bool(x),
        opentelemetry::Value::I64(x) => Value::Number(x.into()),
        opentelemetry::Value::F64(x) => Number::from_f64(x)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        opentelemetry::Value::String(x) => Value::String(x.into()),
        opentelemetry::Value::Array(arr) => match arr {
            opentelemetry::Array::Bool(items) => {
                Value::Array(items.iter().map(|x| Value::Bool(*x)).collect())
            }
            opentelemetry::Array::I64(items) => Value::Array(
                items
                    .iter()
                    .map(|x| Value::Number(Number::from(*x)))
                    .collect(),
            ),
            opentelemetry::Array::F64(items) => Value::Array(
                items
                    .iter()
                    .filter_map(|x| Number::from_f64(*x))
                    .map(Value::Number)
                    .collect(),
            ),
            opentelemetry::Array::String(items) => {
                Value::Array(items.iter().map(|x| x.as_str().into()).collect())
            }
            _ => Value::Null, // non-exhaustive
        },
        _ => Value::Null, // non-exhaustive
    }
}

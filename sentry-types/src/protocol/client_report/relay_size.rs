//! Relay-compatible content size calculations for client report byte outcomes.

use std::collections::BTreeMap;

use crate::protocol::v7::{Log, LogAttribute, Metric, Value};

type AttributeMap<K> = BTreeMap<K, LogAttribute>;

/// Returns the Relay-compatible content size for a log item.
///
/// The size matches [Relay's log size calculation]: log body bytes plus
/// attribute content bytes, clamped to at least `1`.
///
/// [Relay's log size calculation]: https://github.com/getsentry/relay/blob/master/relay-ourlogs/src/size.rs
pub(super) fn log_byte_size(log: &Log) -> u64 {
    usize_to_u64(log.body.len())
        .saturating_add(log_attribute_map_byte_size(&log.attributes))
        .max(1)
}

/// Returns the Relay-compatible content size for a trace metric item.
///
/// The size matches [Relay's trace metric size calculation]: metric name bytes
/// plus numeric value bytes plus attribute content bytes, clamped to at least `1`.
///
/// [Relay's trace metric size calculation]: https://github.com/getsentry/relay/blob/master/relay-server/src/processing/trace_metrics/utils.rs
pub(super) fn metric_byte_size(metric: &Metric) -> u64 {
    usize_to_u64(metric.name.len())
        .saturating_add(8)
        .saturating_add(log_attribute_map_byte_size(&metric.attributes))
        .max(1)
}

/// Converts `usize` byte counts to `u64`, saturating to `u64::MAX` on overflow.
fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

/// Returns an attribute map size as attribute key bytes plus attribute value bytes.
///
/// The size matches [Relay's shared EAP attribute size calculation].
///
/// [Relay's shared EAP attribute size calculation]: https://github.com/getsentry/relay/blob/master/relay-event-normalization/src/eap/size.rs
fn log_attribute_map_byte_size<K>(attributes: &AttributeMap<K>) -> u64
where
    K: AsRef<str>,
{
    attributes.iter().fold(0u64, |size, (key, attribute)| {
        size.saturating_add(usize_to_u64(key.as_ref().len()))
            .saturating_add(log_attribute_byte_size(attribute))
    })
}

/// Returns a log attribute size using its wrapped value only.
///
/// Relay ignores serialized `LogAttribute` wrapper fields such as `value` and `type`.
fn log_attribute_byte_size(attribute: &LogAttribute) -> u64 {
    value_byte_size(&attribute.0)
}

/// Recursively returns a JSON value size using [Relay's EAP value size rules].
///
/// Booleans count as `1`, all numbers count as `8`, strings count as UTF-8 bytes,
/// arrays count as contained value bytes, objects count as key bytes plus contained
/// value bytes, and null counts as `0`.
///
/// [Relay's EAP value size rules]: https://github.com/getsentry/relay/blob/master/relay-event-normalization/src/eap/size.rs
fn value_byte_size(value: &Value) -> u64 {
    match value {
        Value::Null => 0,
        Value::Bool(_) => 1,
        Value::Number(_) => 8,
        Value::String(value) => usize_to_u64(value.len()),
        Value::Array(values) => values.iter().fold(0u64, |size, value| {
            size.saturating_add(value_byte_size(value))
        }),
        Value::Object(values) => values.iter().fold(0u64, |size, (key, value)| {
            size.saturating_add(usize_to_u64(key.len()))
                .saturating_add(value_byte_size(value))
        }),
    }
}

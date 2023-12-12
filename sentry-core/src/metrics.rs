//! Utilities to track metrics in Sentry.
//!
//! Metrics allow you to track the custom values related to the behavior and performance of your
//! application and send them to Sentry. See [`Metric`] for more information on how to build and
//! capture metrics.

use std::borrow::Cow;
use std::collections::hash_map::{DefaultHasher, Entry};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sentry_types::protocol::latest::{Envelope, EnvelopeItem};

use crate::client::TransportArc;
use crate::{ClientOptions, Hub};

pub use crate::units::*;

const BUCKET_INTERVAL: Duration = Duration::from_secs(10);
const FLUSH_INTERVAL: Duration = Duration::from_secs(5);
const MAX_WEIGHT: usize = 100_000;

/// Type alias for strings used in [`Metric`] for names and tags.
pub type MetricStr = Cow<'static, str>;

/// Type used for [`MetricValue::Counter`].
pub type CounterType = f64;

/// Type used for [`MetricValue::Distribution`].
pub type DistributionType = f64;

/// Type used for [`MetricValue::Set`].
pub type SetType = u32;

/// Type used for [`MetricValue::Gauge`].
pub type GaugeType = f64;

/// The value of a [`Metric`], indicating its type.
#[derive(Debug)]
pub enum MetricValue {
    /// Counts instances of an event.
    ///
    /// Counters can be incremented and decremented. The default operation is to increment a counter
    /// by `1`, although increments by larger values and even floating point values are possible.
    ///
    /// # Example
    ///
    /// ```
    /// use sentry::metrics::{Metric, MetricValue};
    ///
    /// Metric::build("my.counter", MetricValue::Counter(1.0)).send();
    /// ```
    Counter(CounterType),

    /// Builds a statistical distribution over values reported.
    ///
    /// Based on individual reported values, distributions allow to query the maximum, minimum, or
    /// average of the reported values, as well as statistical quantiles. With an increasing number
    /// of values in the distribution, its accuracy becomes approximate.
    ///
    /// # Example
    ///
    /// ```
    /// use sentry::metrics::{Metric, MetricValue};
    ///
    /// Metric::build("my.distribution", MetricValue::Distribution(42.0)).send();
    /// ```
    Distribution(DistributionType),

    /// Counts the number of unique reported values.
    ///
    /// Sets allow sending arbitrary discrete values, including strings, and store the deduplicated
    /// count. With an increasing number of unique values in the set, its accuracy becomes
    /// approximate. It is not possible to query individual values from a set.
    ///
    /// # Example
    ///
    /// To create a set value, use [`MetricValue::set_from_str`] or
    /// [`MetricValue::set_from_display`]. These functions convert the provided argument into a
    /// unique hash value, which is then used as the set value.
    ///
    /// ```
    /// use sentry::metrics::{Metric, MetricValue};
    ///
    /// Metric::build("my.set", MetricValue::set_from_str("foo")).send();
    /// ```
    Set(SetType),

    /// Stores absolute snapshots of values.
    ///
    /// In addition to plain [counters](Self::Counter), gauges store a snapshot of the maximum,
    /// minimum and sum of all values, as well as the last reported value. Note that the "last"
    /// component of this aggregation is not commutative. Which value is preserved as last value is
    /// implementation-defined.
    ///
    /// # Example
    ///
    /// ```
    /// use sentry::metrics::{Metric, MetricValue};
    ///
    /// Metric::build("my.gauge", MetricValue::Gauge(42.0)).send();
    /// ```
    Gauge(GaugeType),
}

impl MetricValue {
    /// Returns a set value representing the given string.
    pub fn set_from_str(string: &str) -> Self {
        Self::Set(hash_set_value(string))
    }

    /// Returns a set value representing the given argument.
    pub fn set_from_display(display: impl fmt::Display) -> Self {
        Self::Set(hash_set_value(&display.to_string()))
    }

    /// Returns the type of the metric value.
    fn ty(&self) -> MetricType {
        match self {
            Self::Counter(_) => MetricType::Counter,
            Self::Distribution(_) => MetricType::Distribution,
            Self::Gauge(_) => MetricType::Gauge,
            Self::Set(_) => MetricType::Set,
        }
    }
}

/// Hashes the given set value.
///
/// Sets only guarantee 32-bit accuracy, but arbitrary strings are allowed on the protocol. Upon
/// parsing, they are hashed and only used as hashes subsequently.
fn hash_set_value(string: &str) -> u32 {
    use std::hash::Hasher;
    let mut hasher = DefaultHasher::default();
    hasher.write(string.as_bytes());
    hasher.finish() as u32
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum MetricType {
    Counter,
    Distribution,
    Set,
    Gauge,
}

impl MetricType {
    /// Return the shortcode for this metric type.
    pub fn as_str(&self) -> &'static str {
        match self {
            MetricType::Counter => "c",
            MetricType::Distribution => "d",
            MetricType::Set => "s",
            MetricType::Gauge => "g",
        }
    }
}

impl fmt::Display for MetricType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for MetricType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "c" | "m" => Self::Counter,
            "h" | "d" | "ms" => Self::Distribution,
            "s" => Self::Set,
            "g" => Self::Gauge,
            _ => return Err(()),
        })
    }
}

/// A snapshot of values.
#[derive(Clone, Copy, Debug, PartialEq)]
struct GaugeValue {
    /// The last value reported in the bucket.
    ///
    /// This aggregation is not commutative.
    pub last: GaugeType,
    /// The minimum value reported in the bucket.
    pub min: GaugeType,
    /// The maximum value reported in the bucket.
    pub max: GaugeType,
    /// The sum of all values reported in the bucket.
    pub sum: GaugeType,
    /// The number of times this bucket was updated with a new value.
    pub count: u64,
}

impl GaugeValue {
    /// Creates a gauge snapshot from a single value.
    pub fn single(value: GaugeType) -> Self {
        Self {
            last: value,
            min: value,
            max: value,
            sum: value,
            count: 1,
        }
    }

    /// Inserts a new value into the gauge.
    pub fn insert(&mut self, value: GaugeType) {
        self.last = value;
        self.min = self.min.min(value);
        self.max = self.max.max(value);
        self.sum += value;
        self.count += 1;
    }
}

/// The aggregated value of a [`Metric`] bucket.
enum BucketValue {
    Counter(CounterType),
    Distribution(Vec<DistributionType>),
    Set(BTreeSet<SetType>),
    Gauge(GaugeValue),
}

impl BucketValue {
    /// Inserts a new value into the bucket and returns the added weight.
    pub fn insert(&mut self, value: MetricValue) -> usize {
        match (self, value) {
            (Self::Counter(c1), MetricValue::Counter(c2)) => {
                *c1 += c2;
                0
            }
            (Self::Distribution(d1), MetricValue::Distribution(d2)) => {
                d1.push(d2);
                1
            }
            (Self::Set(s1), MetricValue::Set(s2)) => {
                if s1.insert(s2) {
                    1
                } else {
                    0
                }
            }
            (Self::Gauge(g1), MetricValue::Gauge(g2)) => {
                g1.insert(g2);
                0
            }
            _ => panic!("invalid metric type"),
        }
    }

    /// Returns the number of values stored in this bucket.
    pub fn weight(&self) -> usize {
        match self {
            BucketValue::Counter(_) => 1,
            BucketValue::Distribution(v) => v.len(),
            BucketValue::Set(v) => v.len(),
            BucketValue::Gauge(_) => 5,
        }
    }
}

impl From<MetricValue> for BucketValue {
    fn from(value: MetricValue) -> Self {
        match value {
            MetricValue::Counter(v) => Self::Counter(v),
            MetricValue::Distribution(v) => Self::Distribution(vec![v]),
            MetricValue::Gauge(v) => Self::Gauge(GaugeValue::single(v)),
            MetricValue::Set(v) => Self::Set(std::iter::once(v).collect()),
        }
    }
}

/// UNIX timestamp used for buckets.
type Timestamp = u64;

/// Composite bucket key for [`BucketMap`].
#[derive(PartialEq, Eq, Hash)]
struct BucketKey {
    timestamp: Timestamp,
    ty: MetricType,
    name: MetricStr,
    unit: MetricUnit,
    tags: BTreeMap<MetricStr, MetricStr>,
}

/// A nested map storing metric buckets.
///
/// This map consists of two levels:
///  1. The rounded UNIX timestamp of buckets.
///  2. The metric buckets themselves with a corresponding timestamp.
///
/// This structure allows for efficient dequeueing of buckets that are older than a certain
/// threshold. The buckets are dequeued in order of their timestamp, so the oldest buckets are
/// dequeued first.
type BucketMap = BTreeMap<Timestamp, HashMap<BucketKey, BucketValue>>;

struct AggregatorInner {
    buckets: BucketMap,
    weight: usize,
    running: bool,
    force_flush: bool,
}

impl AggregatorInner {
    pub fn new() -> Self {
        Self {
            buckets: BTreeMap::new(),
            weight: 0,
            running: true,
            force_flush: false,
        }
    }

    /// Adds a new bucket to the aggregator.
    ///
    /// The bucket timestamp is rounded to the nearest bucket interval. Note that this does NOT
    /// automatically flush the aggregator if the weight exceeds the weight threshold.
    pub fn add(&mut self, mut key: BucketKey, value: MetricValue) {
        // Floor timestamp to bucket interval
        key.timestamp /= BUCKET_INTERVAL.as_secs();
        key.timestamp *= BUCKET_INTERVAL.as_secs();

        match self.buckets.entry(key.timestamp).or_default().entry(key) {
            Entry::Occupied(mut e) => self.weight += e.get_mut().insert(value),
            Entry::Vacant(e) => self.weight += e.insert(value.into()).weight(),
        }
    }

    /// Removes and returns all buckets that are ready to flush.
    ///
    /// Buckets are ready to flush as soon as their time window has closed. For example, a bucket
    /// from timestamps `[4600, 4610)` is ready to flush immediately at `4610`.
    pub fn take_buckets(&mut self) -> BucketMap {
        if self.force_flush || !self.running {
            self.weight = 0;
            self.force_flush = false;
            std::mem::take(&mut self.buckets)
        } else {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .saturating_sub(BUCKET_INTERVAL)
                .as_secs();

            // Split all buckets after the cutoff time. `split` contains newer buckets, which should
            // remain, so swap them. After the swap, `split` contains all older buckets.
            let mut split = self.buckets.split_off(&timestamp);
            std::mem::swap(&mut split, &mut self.buckets);

            self.weight -= split
                .values()
                .flat_map(|map| map.values())
                .map(|bucket| bucket.weight())
                .sum::<usize>();

            split
        }
    }

    pub fn weight(&self) -> usize {
        self.weight
    }
}

/// A metric value that contains a numeric value and metadata to be sent to Sentry.
///
/// # Units
///
/// To make the most out of metrics in Sentry, consider assigning a unit during construction. This
/// can be achieved using the [`with_unit`](MetricBuilder::with_unit) builder method.
///
/// ```
/// use sentry::metrics::{Metric, InformationUnit};
///
/// Metric::distribution("request.size", 47.2)
///     .with_unit(InformationUnit::Byte)
///     .send();
/// ```
///
/// # Sending Metrics
///
/// Metrics can be sent to Sentry directly using the [`send`](MetricBuilder::send) method on the
/// constructor. This will send the metric to the [`Client`](crate::Client) on the current [`Hub`].
/// If there is no client on the current hub, the metric is dropped.
///
/// ```
/// use sentry::metrics::Metric;
///
/// Metric::count("requests")
///     .with_tag("method", "GET")
///     .send();
/// ```
///
/// # Sending to a Custom Client
///
/// Metrics can also be sent to a custom client. This is useful if you want to send metrics to a
/// different Sentry project or with different configuration. To do so, finish building the metric
/// and then add it to the client:
///
/// ```
/// use sentry::Hub;
/// use sentry::metrics::Metric;
///
/// let metric = Metric::count("requests")
///    .with_tag("method", "GET")
///    .finish();
///
/// // Obtain a client from somewhere
/// if let Some(client) = Hub::current().client() {
///     client.add_metric(metric);
/// }
/// ```
pub struct Metric {
    /// The name of the metric, identifying it in Sentry.
    ///
    /// The name should consist of
    name: MetricStr,
    unit: MetricUnit,
    value: MetricValue,
    tags: BTreeMap<MetricStr, MetricStr>,
    time: Option<SystemTime>,
}

impl Metric {
    /// Creates a new metric with the stated name and value.
    ///
    /// The provided name identifies the metric in Sentry. It should consist of alphanumeric
    /// characters and `_`, `-`, and `.`. While a single forward slash (`/`) is also allowed in
    /// metric names, it has a special meaning and should not be used in regular metric names. All
    /// characters that do not match this criteria are sanitized.
    ///
    /// The value of the metric determines its type. See the [struct-level](self) docs and
    /// constructor methods for examples on how to build metrics.
    pub fn build(name: impl Into<MetricStr>, value: MetricValue) -> MetricBuilder {
        let metric = Metric {
            name: name.into(),
            unit: MetricUnit::None,
            value,
            tags: BTreeMap::new(),
            time: None,
        };

        MetricBuilder { metric }
    }

    /// Parses a metric from a StatsD string.
    ///
    /// This supports regular StatsD payloads with an extension for tags. In the below example, tags
    /// are optional:
    ///
    /// ```plain
    /// <metricname>:<value>|<type>|#<tag1>:<value1>,<tag2>:<value2>
    /// ```
    ///
    /// Units are encoded into the metric name, separated by an `@`:
    ///
    /// ```plain
    /// <metricname>@<unit>:<value>|<type>|#<tag1>:<value1>,<tag2>:<value2>
    /// ```
    pub fn parse_statsd(string: &str) -> Result<Self, ParseMetricError> {
        parse_metric_opt(string).ok_or(ParseMetricError(()))
    }

    /// Builds a metric that increments a [counter](MetricValue::Counter) by the given value.
    ///
    /// # Example
    ///
    /// ```
    /// use sentry::metrics::{Metric};
    ///
    /// Metric::incr("operation.total_values", 7.0).send();
    /// ```
    pub fn incr(name: impl Into<MetricStr>, value: f64) -> MetricBuilder {
        Self::build(name, MetricValue::Counter(value))
    }

    /// Builds a metric that [counts](MetricValue::Counter) the single occurrence of an event.
    ///
    /// # Example
    ///
    /// ```
    /// use sentry::metrics::{Metric};
    ///
    /// Metric::count("requests").send();
    /// ```
    pub fn count(name: impl Into<MetricStr>) -> MetricBuilder {
        Self::build(name, MetricValue::Counter(1.0))
    }

    /// Builds a metric that tracks the duration of an operation.
    ///
    /// This is a [distribution](MetricValue::Distribution) metric that is tracked in seconds.
    ///
    /// # Example
    ///
    /// ```
    /// use std::time::Duration;
    /// use sentry::metrics::{Metric};
    ///
    /// Metric::timing("operation", Duration::from_secs(1)).send();
    /// ```
    pub fn timing(name: impl Into<MetricStr>, timing: Duration) -> MetricBuilder {
        Self::build(name, MetricValue::Distribution(timing.as_secs_f64()))
            .with_unit(DurationUnit::Second)
    }

    /// Builds a metric that tracks the [distribution](MetricValue::Distribution) of values.
    ///
    /// # Example
    ///
    /// ```
    /// use sentry::metrics::{Metric};
    ///
    /// Metric::distribution("operation.batch_size", 42.0).send();
    /// ```
    pub fn distribution(name: impl Into<MetricStr>, value: f64) -> MetricBuilder {
        Self::build(name, MetricValue::Distribution(value))
    }

    /// Builds a metric that tracks the [unique number](MetricValue::Set) of values provided.
    ///
    /// See [`MetricValue`] for more ways to construct sets.
    ///
    /// # Example
    ///
    /// ```
    /// use sentry::metrics::{Metric};
    ///
    /// Metric::set("users", "user1").send();
    /// ```
    pub fn set(name: impl Into<MetricStr>, string: &str) -> MetricBuilder {
        Self::build(name, MetricValue::set_from_str(string))
    }

    /// Builds a metric that tracks the [snapshot](MetricValue::Gauge) of provided values.
    ///
    /// # Example
    ///
    /// ```
    /// use sentry::metrics::{Metric};
    ///
    /// Metric::gauge("cache.size", 42.0).send();
    /// ```
    pub fn gauge(name: impl Into<MetricStr>, value: f64) -> MetricBuilder {
        Self::build(name, MetricValue::Gauge(value))
    }
}

/// A builder for metrics.
///
/// Use one of the [`Metric`] constructors to create a new builder. See the struct-level docs for
/// examples of how to build metrics.
#[must_use]
pub struct MetricBuilder {
    metric: Metric,
}

impl MetricBuilder {
    /// Sets the unit for the metric.
    ///
    /// The unit augments the metric value by giving it a magnitude and semantics. Some units have
    /// special support when rendering metrics or their values in Sentry, such as for timings. See
    /// [`MetricUnit`] for more information on the supported units. The unit can be set to
    /// [`MetricUnit::None`] to indicate that the metric has no unit, or to [`MetricUnit::Custom`]
    /// to indicate a user-defined unit.
    ///
    /// By default, the unit is set to [`MetricUnit::None`].
    pub fn with_unit(mut self, unit: impl Into<MetricUnit>) -> Self {
        self.metric.unit = unit.into();
        self
    }

    /// Adds a tag to the metric.
    ///
    /// Tags allow you to add dimensions to metrics. They are key-value pairs that can be filtered
    /// or grouped by in Sentry.
    ///
    /// When sent to Sentry via [`MetricBuilder::send`] or when added to a
    /// [`Client`](crate::Client), the client may add default tags to the metrics, such as the
    /// `release` or the `environment` from the Scope.
    pub fn with_tag(mut self, name: impl Into<MetricStr>, value: impl Into<MetricStr>) -> Self {
        self.metric.tags.insert(name.into(), value.into());
        self
    }

    /// Sets the timestamp for the metric.
    ///
    /// By default, the timestamp is set to the current time when the metric is built or sent.
    pub fn with_time(mut self, time: SystemTime) -> Self {
        self.metric.time = Some(time);
        self
    }

    /// Builds the metric.
    pub fn finish(self) -> Metric {
        self.metric
    }

    /// Sends the metric to the current client.
    ///
    /// If there is no client on the current [`Hub`], the metric is dropped.
    pub fn send(self) {
        if let Some(client) = Hub::current().client() {
            client.add_metric(self.finish());
        }
    }
}

/// Error emitted from [`Metric::parse_statsd`] for invalid metric strings.
#[derive(Debug)]
pub struct ParseMetricError(());

impl std::error::Error for ParseMetricError {}

impl fmt::Display for ParseMetricError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid metric string")
    }
}

fn parse_metric_opt(string: &str) -> Option<Metric> {
    let mut components = string.split('|');

    let (mri_str, value_str) = components.next()?.split_once(':')?;
    let (name, unit) = match mri_str.split_once('@') {
        Some((name, unit_str)) => (name, unit_str.parse().ok()?),
        None => (mri_str, MetricUnit::None),
    };

    let ty = components.next().and_then(|s| s.parse().ok())?;
    let value = match ty {
        MetricType::Counter => MetricValue::Counter(value_str.parse().ok()?),
        MetricType::Distribution => MetricValue::Distribution(value_str.parse().ok()?),
        MetricType::Set => MetricValue::Set(value_str.parse().ok()?),
        MetricType::Gauge => MetricValue::Gauge(value_str.parse().ok()?),
    };

    let mut builder = Metric::build(name.to_owned(), value).with_unit(unit);

    for component in components {
        if let Some('#') = component.chars().next() {
            for pair in component.get(1..)?.split(',') {
                let mut key_value = pair.splitn(2, ':');

                let key = key_value.next()?.to_owned();
                let value = key_value.next().unwrap_or_default().to_owned();

                builder = builder.with_tag(key, value);
            }
        }
    }

    Some(builder.finish())
}

pub(crate) type TagMap = BTreeMap<MetricStr, MetricStr>;

fn get_default_tags(options: &ClientOptions) -> TagMap {
    let mut tags = TagMap::new();
    if let Some(ref release) = options.release {
        tags.insert("release".into(), release.clone());
    }
    if let Some(ref environment) = options.environment {
        tags.insert("environment".into(), environment.clone());
    }
    tags
}

pub(crate) struct MetricAggregator {
    inner: Arc<Mutex<AggregatorInner>>,
    handle: Option<JoinHandle<()>>,
}

impl MetricAggregator {
    pub fn new(transport: TransportArc, options: &ClientOptions) -> Self {
        let default_tags = get_default_tags(options);

        let inner = Arc::new(Mutex::new(AggregatorInner::new()));
        let inner_clone = Arc::clone(&inner);

        let handle = thread::Builder::new()
            .name("sentry-metrics".into())
            .spawn(move || Self::worker_thread(inner_clone, transport, default_tags))
            .expect("failed to spawn thread");

        Self {
            inner,
            handle: Some(handle),
        }
    }

    pub fn add(&self, metric: Metric) {
        let Metric {
            name,
            unit,
            value,
            tags,
            time,
        } = metric;

        let timestamp = time
            .unwrap_or_else(SystemTime::now)
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let key = BucketKey {
            timestamp,
            ty: value.ty(),
            name,
            unit,
            tags,
        };

        let mut guard = self.inner.lock().unwrap();
        guard.add(key, value);

        if guard.weight() > MAX_WEIGHT {
            if let Some(ref handle) = self.handle {
                handle.thread().unpark();
            }
        }
    }

    pub fn flush(&self) {
        self.inner.lock().unwrap().force_flush = true;
        if let Some(ref handle) = self.handle {
            handle.thread().unpark();
        }
    }

    fn worker_thread(inner: Arc<Mutex<AggregatorInner>>, transport: TransportArc, tags: TagMap) {
        let mut running = true;

        while running {
            // Park instead of sleep so we can wake the thread up. Do not account for delays during
            // flushing, since we benefit from some drift to spread out metric submissions.
            thread::park_timeout(FLUSH_INTERVAL);

            let buckets = {
                let mut guard = inner.lock().unwrap();
                running = guard.running;
                guard.take_buckets()
            };

            if !buckets.is_empty() {
                Self::flush_buckets(buckets, &transport, &tags);
            }
        }
    }

    fn flush_buckets(buckets: BucketMap, transport: &TransportArc, tags: &TagMap) {
        // The transport is usually available when flush is called. Prefer a short lock and worst
        // case throw away the result rather than blocking the transport for too long.
        if let Ok(output) = Self::format_payload(buckets, tags) {
            let mut envelope = Envelope::new();
            envelope.add_item(EnvelopeItem::Metrics(output));

            if let Some(ref transport) = *transport.read().unwrap() {
                transport.send_envelope(envelope);
            }
        }
    }

    fn format_payload(buckets: BucketMap, tags: &TagMap) -> std::io::Result<Vec<u8>> {
        use std::io::Write;
        let mut out = vec![];

        for (key, value) in buckets.into_iter().flat_map(|(_, v)| v) {
            write!(&mut out, "{}", SafeKey(key.name.as_ref()))?;
            if key.unit != MetricUnit::None {
                write!(&mut out, "@{}", key.unit)?;
            }

            match value {
                BucketValue::Counter(c) => {
                    write!(&mut out, ":{}", c)?;
                }
                BucketValue::Distribution(d) => {
                    for v in d {
                        write!(&mut out, ":{}", v)?;
                    }
                }
                BucketValue::Set(s) => {
                    for v in s {
                        write!(&mut out, ":{}", v)?;
                    }
                }
                BucketValue::Gauge(g) => {
                    write!(
                        &mut out,
                        ":{}:{}:{}:{}:{}",
                        g.last, g.min, g.max, g.sum, g.count
                    )?;
                }
            }

            write!(&mut out, "|{}", key.ty.as_str())?;

            for (i, (k, v)) in key.tags.iter().chain(tags).enumerate() {
                match i {
                    0 => write!(&mut out, "|#")?,
                    _ => write!(&mut out, ",")?,
                }

                write!(&mut out, "{}:{}", SafeKey(k.as_ref()), SaveVal(v.as_ref()))?;
            }

            writeln!(&mut out, "|T{}", key.timestamp)?;
        }

        Ok(out)
    }
}

impl Drop for MetricAggregator {
    fn drop(&mut self) {
        self.inner.lock().unwrap().running = false;
        if let Some(handle) = self.handle.take() {
            handle.thread().unpark();
            handle.join().unwrap();
        }
    }
}

struct SafeKey<'s>(&'s str);

impl<'s> fmt::Display for SafeKey<'s> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for c in self.0.chars() {
            if c.is_ascii_alphanumeric() || ['_', '-', '.', '/'].contains(&c) {
                write!(f, "{}", c)?;
            }
        }
        Ok(())
    }
}

struct SaveVal<'s>(&'s str);

impl<'s> fmt::Display for SaveVal<'s> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for c in self.0.chars() {
            if c.is_alphanumeric()
                || ['_', ':', '/', '@', '.', '{', '}', '[', ']', '$', '-'].contains(&c)
            {
                write!(f, "{}", c)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::{with_captured_envelopes, with_captured_envelopes_options};
    use crate::ClientOptions;

    /// Returns the current system time and rounded bucket timestamp.
    fn current_time() -> (SystemTime, u64) {
        let now = SystemTime::now();
        let timestamp = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let timestamp = timestamp / 10 * 10;

        (now, timestamp)
    }

    fn get_single_metrics(envelopes: &[Envelope]) -> &str {
        assert_eq!(envelopes.len(), 1, "expected exactly one envelope");

        let mut items = envelopes[0].items();
        let Some(EnvelopeItem::Metrics(payload)) = items.next() else {
            panic!("expected metrics item");
        };

        std::str::from_utf8(payload).unwrap().trim()
    }

    #[test]
    fn test_counter() {
        let (time, ts) = current_time();

        let envelopes = with_captured_envelopes(|| {
            Metric::count("my.metric")
                .with_tag("foo", "bar")
                .with_time(time)
                .send();

            Metric::incr("my.metric", 2.0)
                .with_tag("foo", "bar")
                .with_time(time)
                .send();
        });

        let metrics = get_single_metrics(&envelopes);
        assert_eq!(metrics, format!("my.metric:3|c|#foo:bar|T{ts}"));
    }

    #[test]
    fn test_timing() {
        let (time, ts) = current_time();

        let envelopes = with_captured_envelopes(|| {
            Metric::timing("my.metric", Duration::from_millis(200))
                .with_tag("foo", "bar")
                .with_time(time)
                .send();

            Metric::timing("my.metric", Duration::from_millis(100))
                .with_tag("foo", "bar")
                .with_time(time)
                .send();
        });

        let metrics = get_single_metrics(&envelopes);
        assert_eq!(
            metrics,
            format!("my.metric@second:0.2:0.1|d|#foo:bar|T{ts}")
        );
    }

    #[test]
    fn test_unit() {
        let (time, ts) = current_time();

        let envelopes = with_captured_envelopes(|| {
            Metric::count("my.metric")
                .with_tag("foo", "bar")
                .with_time(time)
                .with_unit("custom")
                .send();
        });

        let metrics = get_single_metrics(&envelopes);
        assert_eq!(metrics, format!("my.metric@custom:1|c|#foo:bar|T{ts}"));
    }

    #[test]
    fn test_default_tags() {
        let (time, ts) = current_time();

        let options = ClientOptions {
            release: Some("myapp@1.0.0".into()),
            environment: Some("production".into()),
            ..Default::default()
        };

        let envelopes = with_captured_envelopes_options(
            || {
                Metric::count("requests")
                    .with_tag("foo", "bar")
                    .with_time(time)
                    .send();
            },
            options,
        );

        let metrics = get_single_metrics(&envelopes);
        assert_eq!(
            metrics,
            format!("requests:1|c|#foo:bar,environment:production,release:myapp@1.0.0|T{ts}")
        );
    }
}

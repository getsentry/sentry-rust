use std::borrow::Cow;
use std::collections::hash_map::{DefaultHasher, Entry};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sentry_types::protocol::latest::{Envelope, EnvelopeItem};

use crate::client::TransportArc;
use crate::Hub;

use crate::units::DurationUnit;
pub use crate::units::MetricUnit;

const BUCKET_INTERVAL: Duration = Duration::from_secs(10);
const FLUSH_INTERVAL: Duration = Duration::from_secs(5);
const MAX_WEIGHT: usize = 100_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum MetricType {
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

/// Type used for Counter metric
pub type CounterType = f64;

/// Type of distribution entries
pub type DistributionType = f64;

/// Type used for set elements in Set metric
pub type SetType = u32;

/// Type used for Gauge entries
pub type GaugeType = f64;

/// A snapshot of values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GaugeValue {
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

enum BucketValue {
    Counter(CounterType),
    Distribution(Vec<DistributionType>),
    Set(BTreeSet<SetType>),
    Gauge(GaugeValue),
}

impl BucketValue {
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

pub type MetricStr = Cow<'static, str>;

type Timestamp = u64;

#[derive(PartialEq, Eq, Hash)]
struct BucketKey {
    timestamp: Timestamp,
    ty: MetricType,
    name: MetricStr,
    unit: MetricUnit,
    tags: BTreeMap<MetricStr, MetricStr>,
}

#[derive(Debug)]
pub enum MetricValue {
    Counter(CounterType),
    Distribution(DistributionType),
    Gauge(GaugeType),
    Set(SetType),
}

impl MetricValue {
    /// Returns a bucket value representing a set with a single given string value.
    pub fn set_from_str(string: &str) -> Self {
        Self::Set(hash_set_value(string))
    }

    /// Returns a bucket value representing a set with a single given value.
    pub fn set_from_display(display: impl fmt::Display) -> Self {
        Self::Set(hash_set_value(&display.to_string()))
    }

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

    pub fn add(&mut self, mut key: BucketKey, value: MetricValue) {
        // Floor timestamp to bucket interval
        key.timestamp /= BUCKET_INTERVAL.as_secs();
        key.timestamp *= BUCKET_INTERVAL.as_secs();

        match self.buckets.entry(key.timestamp).or_default().entry(key) {
            Entry::Occupied(mut e) => self.weight += e.get_mut().insert(value),
            Entry::Vacant(e) => self.weight += e.insert(value.into()).weight(),
        }
    }

    pub fn take_buckets(&mut self) -> BucketMap {
        if self.force_flush || !self.running {
            self.weight = 0;
            self.force_flush = false;
            std::mem::take(&mut self.buckets)
        } else {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .saturating_sub(FLUSH_INTERVAL)
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

pub struct Metric {
    name: MetricStr,
    unit: MetricUnit,
    value: MetricValue,
    tags: BTreeMap<MetricStr, MetricStr>,
    time: Option<SystemTime>,
}

impl Metric {
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

    pub fn parse_statsd(string: &str) -> Result<Self, ParseMetricError> {
        parse_metric_opt(string).ok_or(ParseMetricError(()))
    }

    pub fn incr(name: impl Into<MetricStr>) -> MetricBuilder {
        Self::build(name, MetricValue::Counter(1.0))
    }

    pub fn timing(name: impl Into<MetricStr>, timing: Duration) -> MetricBuilder {
        Self::build(name, MetricValue::Distribution(timing.as_secs_f64()))
            .with_unit(MetricUnit::Duration(DurationUnit::Second))
    }

    pub fn distribution(name: impl Into<MetricStr>, value: f64) -> MetricBuilder {
        Self::build(name, MetricValue::Distribution(value))
    }

    pub fn set(name: impl Into<MetricStr>, string: &str) -> MetricBuilder {
        Self::build(name, MetricValue::set_from_str(string))
    }

    pub fn gauge(name: impl Into<MetricStr>, value: f64) -> MetricBuilder {
        Self::build(name, MetricValue::Gauge(value))
    }
}

#[must_use]
pub struct MetricBuilder {
    metric: Metric,
}

impl MetricBuilder {
    pub fn with_unit(mut self, unit: MetricUnit) -> Self {
        self.metric.unit = unit;
        self
    }

    pub fn with_tag(mut self, name: impl Into<MetricStr>, value: impl Into<MetricStr>) -> Self {
        self.metric.tags.insert(name.into(), value.into());
        self
    }

    pub fn with_time(mut self, time: SystemTime) -> Self {
        self.metric.time = Some(time);
        self
    }

    pub fn finish(self) -> Metric {
        self.metric
    }

    pub fn send(self) {
        if let Some(client) = Hub::current().client() {
            client.add_metric(self.finish());
        }
    }
}

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

pub struct MetricAggregator {
    inner: Arc<Mutex<AggregatorInner>>,
    handle: Option<JoinHandle<()>>,
}

impl MetricAggregator {
    pub fn new(transport: TransportArc) -> Self {
        let inner = Arc::new(Mutex::new(AggregatorInner::new()));
        let inner_clone = Arc::clone(&inner);

        let handle = thread::Builder::new()
            .name("sentry-metrics".into())
            .spawn(move || Self::worker_thread(inner_clone, transport))
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

    fn worker_thread(inner: Arc<Mutex<AggregatorInner>>, transport: TransportArc) {
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
                Self::flush_buckets(buckets, &transport);
            }
        }
    }

    fn flush_buckets(buckets: BucketMap, transport: &TransportArc) {
        // The transport is usually available when flush is called. Prefer a short lock and worst
        // case throw away the result rather than blocking the transport for too long.
        if let Ok(output) = Self::format_payload(buckets) {
            let mut envelope = Envelope::new();
            envelope.add_item(EnvelopeItem::Metrics(output));

            if let Some(ref transport) = *transport.read().unwrap() {
                transport.send_envelope(envelope);
            }
        }
    }

    fn format_payload(buckets: BucketMap) -> std::io::Result<Vec<u8>> {
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

            if !key.tags.is_empty() {
                write!(&mut out, "|#")?;
                for (i, (k, v)) in key.tags.into_iter().enumerate() {
                    if i > 0 {
                        write!(&mut out, ",")?;
                    }
                    write!(&mut out, "{}:{}", SafeKey(k.as_ref()), SaveVal(v.as_ref()))?;
                }
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

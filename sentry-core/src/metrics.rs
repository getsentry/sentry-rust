use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hash, Hasher};
use std::sync::Arc;
use std::sync::Mutex;
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{fmt, mem, thread};

use rand::thread_rng;
use rand::Rng;
use sentry_types::protocol::latest::{Envelope, EnvelopeItem};

use crate::client::TransportArc;
use crate::units::MetricUnit;

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum SetItem {
    String(String),
    I64(i64),
}

/// Utility type for holding onto strings internally.
enum MetricStr<'s> {
    Borrowed(&'s str),
    Owned(Arc<str>),
}

impl<'s> MetricStr<'s> {
    /// Returns a regular string reference.
    pub fn as_str(&self) -> &str {
        match self {
            MetricStr::Borrowed(s) => s,
            MetricStr::Owned(ref s) => &s,
        }
    }

    /// Returns a static, owned version.
    pub fn to_owned(&self) -> MetricStr<'static> {
        match self {
            MetricStr::Borrowed(s) => MetricStr::Owned(Arc::from(*s)),
            MetricStr::Owned(ref s) => MetricStr::Owned(s.clone()),
        }
    }
}

impl<'s> PartialEq for MetricStr<'s> {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl<'s> Eq for MetricStr<'s> {}

impl<'s> PartialOrd for MetricStr<'s> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.as_str().cmp(other.as_str()))
    }
}

impl<'s> Ord for MetricStr<'s> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl<'s> Hash for MetricStr<'s> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state)
    }
}

impl<'s> fmt::Debug for MetricStr<'s> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct BucketKey<'s> {
    key: MetricStr<'s>,
    unit: MetricUnit,
    tags: BTreeMap<MetricStr<'s>, MetricStr<'s>>,
}

impl<'s> BucketKey<'s> {
    /// Returns an owned version of the key with static lifetime
    pub fn into_owned(self) -> BucketKey<'static> {
        BucketKey {
            key: self.key.to_owned(),
            unit: self.unit,
            tags: self
                .tags
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect(),
        }
    }
}

#[derive(Debug)]
enum MetricAggregationState {
    Counter(f64),
    Gauge(f64, f64, f64, f64, u64),
    Distribution(Vec<f64>),
    Set(HashSet<SetItem>),
}

impl MetricAggregationState {
    /// Creates an empty aggregation state.
    fn new(value: MetricValue) -> MetricAggregationState {
        match value {
            MetricValue::Counter(v) => MetricAggregationState::Counter(v),
            MetricValue::Gauge(v) => MetricAggregationState::Gauge(v, v, v, v, 1),
            MetricValue::Distribution(v) => MetricAggregationState::Distribution(vec![v]),
            MetricValue::Set(v) => MetricAggregationState::Set(HashSet::from([v])),
        }
    }

    /// Returns the weight of the aggregation state
    fn weight(&self) -> usize {
        match self {
            MetricAggregationState::Counter(_) => 1,
            MetricAggregationState::Gauge(_, _, _, _, _) => 5,
            MetricAggregationState::Distribution(ref d) => d.len(),
            MetricAggregationState::Set(ref s) => s.len(),
        }
    }

    /// Returns the short code of the type
    fn ty(&self) -> char {
        match self {
            MetricAggregationState::Counter(_) => 'c',
            MetricAggregationState::Gauge(_, _, _, _, _) => 'g',
            MetricAggregationState::Distribution(_) => 'd',
            MetricAggregationState::Set(_) => 's',
        }
    }

    /// Adds a data point into the aggregation state.
    fn add(&mut self, value: MetricValue) {
        match (self, value) {
            (MetricAggregationState::Counter(ref mut c), MetricValue::Counter(v)) => {
                *c += v;
            }
            (
                MetricAggregationState::Gauge(
                    ref mut last,
                    ref mut min,
                    ref mut max,
                    ref mut sum,
                    ref mut count,
                ),
                MetricValue::Gauge(v),
            ) => {
                *last = v;
                *min = min.min(v);
                *max = max.max(v);
                *sum += v;
                *count += 1;
            }
            (MetricAggregationState::Distribution(ref mut d), MetricValue::Distribution(v)) => {
                d.push(v);
            }
            (MetricAggregationState::Set(ref mut s), MetricValue::Set(v)) => {
                s.insert(v);
            }
            _ => panic!("mismatched aggregation state to value"),
        }
    }

    /// Returns an iterator over all values in the state.
    pub fn iter_values(&self) -> Box<dyn Iterator<Item = f64> + '_> {
        match *self {
            MetricAggregationState::Counter(c) => Box::new([c].into_iter()),
            MetricAggregationState::Gauge(last, min, max, sum, count) => {
                Box::new([last, min, max, sum, count as f64].into_iter())
            }
            MetricAggregationState::Distribution(ref d) => Box::new(d.iter().copied()),
            MetricAggregationState::Set(ref s) => Box::new(s.iter().map(|item| match *item {
                SetItem::String(ref s) => {
                    let mut hasher = DefaultHasher::new();
                    s.hash(&mut hasher);
                    hasher.finish() as f64
                }
                SetItem::I64(x) => x as f64,
            })),
        }
    }
}

#[derive(Debug)]
pub enum MetricValue {
    Counter(f64),
    Gauge(f64),
    Distribution(f64),
    Set(SetItem),
}

// bucket size in seconds
const ROLLUP: u64 = 10;

struct AggregatorState {
    startup: SystemTime,
    buckets: BTreeMap<i32, HashMap<BucketKey<'static>, MetricAggregationState>>,
    total_bucket_weight: usize,
    running: bool,
    force_flush: bool,
    flush_shift: f64,
    transport: TransportArc,
}

impl AggregatorState {
    /// Given a current timestamp, returns the local bucket key.
    fn get_bucket_ts(&self, ts: SystemTime) -> i32 {
        if let Ok(pos) = ts.duration_since(self.startup) {
            (pos.as_secs() / ROLLUP) as i32
        } else if let Ok(neg) = self.startup.duration_since(ts) {
            -((neg.as_secs() / ROLLUP) as i32)
        } else {
            // this is unreachable
            0
        }
    }

    /// Bucket ts to timestamp
    fn bucket_ts_to_unix(&self, ts: i32) -> u64 {
        match ts {
            x if x >= 0 => self.startup + Duration::from_secs(x as u64 * ROLLUP),
            x => self.startup + Duration::from_secs((-x) as u64 * ROLLUP),
        }
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
    }

    fn flushable_buckets(
        &mut self,
    ) -> BTreeMap<i32, HashMap<BucketKey<'static>, MetricAggregationState>> {
        if self.force_flush {
            self.total_bucket_weight = 0;
            self.force_flush = false;
            mem::take(&mut self.buckets)
        } else {
            let rv = self.buckets.split_off(&self.get_bucket_ts(
                SystemTime::now()
                    - Duration::from_secs(ROLLUP)
                    - Duration::from_secs_f64(self.flush_shift * ROLLUP as f64),
            ));
            self.total_bucket_weight -= rv
                .values()
                .flat_map(|x| x.values())
                .map(|m| m.weight())
                .sum::<usize>();
            rv
        }
    }

    fn flush_buckets(
        &self,
        buckets: BTreeMap<i32, HashMap<BucketKey<'_>, MetricAggregationState>>,
    ) {
        if let Some(ref mut transport) = *self.transport.write().unwrap() {
            use std::fmt::Write;
            let mut buf = String::new();
            for (ts, local_buckets) in buckets.into_iter() {
                let unix = self.bucket_ts_to_unix(ts);
                for (key, state) in local_buckets.iter() {
                    write!(buf, "{}@{}", SanitzedKey(key.key.as_str()), key.unit).ok();
                    for value in state.iter_values() {
                        write!(buf, ":{}", value).ok();
                    }
                    write!(buf, "|{}", state.ty()).ok();
                    if !key.tags.is_empty() {
                        write!(buf, "|#").ok();
                        for (idx, (key, value)) in key.tags.iter().enumerate() {
                            if idx > 0 {
                                write!(buf, ",").ok();
                            }
                            write!(
                                buf,
                                "{}:{}",
                                SanitzedKey(key.as_str()),
                                SanitzedValue(value.as_str())
                            )
                            .ok();
                        }
                    }
                }
                writeln!(buf, "|T{}", unix).ok();
            }

            let mut envelope = Envelope::new();
            envelope.add_item(EnvelopeItem::Statsd(buf.into_bytes()));
            transport.send_envelope(Envelope::new());
        }
    }
}

pub struct MetricsAggregator {
    state: Arc<Mutex<AggregatorState>>,
    join_handle: JoinHandle<()>,
}

impl fmt::Debug for MetricsAggregator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MetricsAggregator").finish()
    }
}

impl Drop for MetricsAggregator {
    fn drop(&mut self) {
        self.stop();
    }
}

impl MetricsAggregator {
    pub fn new(transport: TransportArc) -> MetricsAggregator {
        let state = Arc::new(Mutex::new(AggregatorState {
            startup: SystemTime::now(),
            buckets: BTreeMap::new(),
            total_bucket_weight: 0,
            running: true,
            force_flush: false,
            flush_shift: thread_rng().gen::<f64>(),
            transport,
        }));

        MetricsAggregator {
            state: state.clone(),
            join_handle: thread::spawn(move || loop {
                let mut state = state.lock().unwrap();
                if !state.running {
                    break;
                }

                let buckets = state.flushable_buckets();
                if !buckets.is_empty() {
                    state.flush_buckets(buckets);
                }

                // release lock before sleep
                drop(state);
                thread::park_timeout(Duration::from_secs(5));
            }),
        }
    }

    /// Stops the background thread.
    pub fn stop(&self) {
        self.state.lock().unwrap().running = false;
        self.join_handle.thread().unpark();
    }

    /// Forces a flush
    pub fn flush(&self) {
        let mut state = self.state.lock().unwrap();
        if !state.running {
            return;
        }
        let buckets = state.flushable_buckets();
        if !buckets.is_empty() {
            state.flush_buckets(buckets);
        }
    }

    /// Adds a value to the aggregator
    pub fn add<'s, T>(
        &self,
        key: &'s str,
        value: MetricValue,
        ts: Option<SystemTime>,
        unit: MetricUnit,
        tags: T,
    ) where
        T: IntoIterator<Item = (&'s str, &'s str)>,
    {
        let mut aggregator_state = self.state.lock().unwrap();
        let ts = aggregator_state.get_bucket_ts(ts.unwrap_or_else(SystemTime::now));
        let bucket = if let Some(bucket) = aggregator_state.buckets.get_mut(&ts) {
            bucket
        } else {
            aggregator_state.buckets.insert(ts, HashMap::default());
            aggregator_state.buckets.get_mut(&ts).unwrap()
        };

        let key = BucketKey {
            key: MetricStr::Borrowed(key),
            unit,
            tags: tags
                .into_iter()
                .map(|(k, v)| (MetricStr::Borrowed(k), MetricStr::Borrowed(v)))
                .collect(),
        };

        // SAFETY: the transmute here exists because I'm lazy to make composite keys
        // in a hashmap work otherwise.  The alternative I beleive is raw_entry_mut which
        // is unstable, the same API on hashbrown or a very complex custom key type (I guess).
        if let Some(state) = bucket.get_mut(unsafe { mem::transmute::<_, _>(&key) }) {
            let old_weight = state.weight();
            state.add(value);
            aggregator_state.total_bucket_weight += state.weight() - old_weight;
        } else {
            let state = MetricAggregationState::new(value);
            let weight = state.weight();
            bucket.insert(key.into_owned(), state);
            aggregator_state.total_bucket_weight += weight;
        }
    }
}

struct SanitzedKey<'s>(&'s str);

impl<'s> fmt::Display for SanitzedKey<'s> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for c in self.0.chars() {
            if c.is_ascii_alphanumeric() || ['_', '-', '.', '/'].contains(&c) {
                write!(f, "{}", c)?;
            }
        }
        Ok(())
    }
}

struct SanitzedValue<'s>(&'s str);

impl<'s> fmt::Display for SanitzedValue<'s> {
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

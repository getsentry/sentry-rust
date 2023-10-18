use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use cadence::MetricSink;
use sentry_types::protocol::latest::{Envelope, EnvelopeItem};

use crate::client::TransportArc;
use crate::{Client, Hub};

#[derive(Debug)]
pub struct SentryMetricSink<S> {
    client: Arc<Client>,
    sink: S,
}

impl<S> SentryMetricSink<S> {
    pub fn try_new(sink: S) -> Result<Self, S> {
        let hub = Hub::current();
        let Some(client) = hub.client() else {
            return Err(sink);
        };

        Ok(Self { client, sink })
    }
}

impl<S> MetricSink for SentryMetricSink<S>
where
    S: MetricSink,
{
    fn emit(&self, metric: &str) -> std::io::Result<usize> {
        self.client.send_metric(metric);
        self.sink.emit(metric)
    }

    fn flush(&self) -> std::io::Result<()> {
        if self.client.flush(None) {
            self.sink.flush()
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Flushing Client failed",
            ))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
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

struct GaugeValue {
    last: f64,
    min: f64,
    max: f64,
    sum: f64,
    count: u64,
}
enum BucketValue {
    Counter(f64),
    Distribution(Vec<f64>),
    Set(BTreeSet<u32>),
    Gauge(GaugeValue),
}
impl BucketValue {
    fn distribution(val: f64) -> BucketValue {
        Self::Distribution(vec![val])
    }

    fn gauge(val: f64) -> BucketValue {
        Self::Gauge(GaugeValue {
            last: val,
            min: val,
            max: val,
            sum: val,
            count: 1,
        })
    }

    fn set_from_str(value: &str) -> BucketValue {
        todo!()
    }

    fn merge(&mut self, other: BucketValue) -> Result<(), ()> {
        match (self, other) {
            (BucketValue::Counter(c1), BucketValue::Counter(c2)) => {
                *c1 += c2;
            }
            (BucketValue::Distribution(d1), BucketValue::Distribution(d2)) => {
                d1.extend(d2);
            }
            (BucketValue::Set(s1), BucketValue::Set(s2)) => s1.extend(s2),
            (BucketValue::Gauge(g1), BucketValue::Gauge(g2)) => {
                g1.last = g2.last;
                g1.min = g1.min.min(g2.min);
                g1.max = g1.max.max(g2.max);
                g1.sum += g2.sum;
                g1.count += g2.count;
            }
            _ => return Err(()),
        }
        Ok(())
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct BucketKey {
    timestamp: u64,
    ty: MetricType,
    name: String,
    tags: String,
}

type AggregateMetrics = BTreeMap<BucketKey, BucketValue>;

enum Task {
    SendMetrics((BucketKey, BucketValue)),
    Flush,
    Shutdown,
}

pub struct MetricFlusher {
    sender: SyncSender<Task>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

const FLUSH_INTERVAL: Duration = Duration::from_secs(10);

impl MetricFlusher {
    pub fn new(transport: TransportArc) -> Self {
        let (sender, receiver) = sync_channel(30);
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_worker = shutdown.clone();
        let handle = thread::Builder::new()
            .name("sentry-metrics".into())
            .spawn(move || Self::worker_thread(receiver, shutdown_worker, transport))
            .ok();

        Self {
            sender,
            shutdown,
            handle,
        }
    }

    pub fn send_metric(&self, metric: &str) {
        fn mk_value(ty: MetricType, value: &str) -> Option<BucketValue> {
            Some(match ty {
                MetricType::Counter => BucketValue::Counter(value.parse().ok()?),
                MetricType::Distribution => BucketValue::distribution(value.parse().ok()?),
                MetricType::Set => BucketValue::set_from_str(value),
                MetricType::Gauge => BucketValue::gauge(value.parse().ok()?),
            })
        }

        fn parse(metric: &str) -> Option<(BucketKey, BucketValue)> {
            let mut components = metric.split('|');
            let mut values = components.next()?.split(':');
            let name = values.next()?;

            let ty: MetricType = components.next().and_then(|s| s.parse().ok())?;
            let mut value = mk_value(ty, values.next()?)?;

            for value_s in values {
                value.merge(mk_value(ty, value_s)?).ok()?;
            }

            let mut tags = "";
            let mut timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            for component in components {
                if let Some(component_tags) = component.strip_prefix('#') {
                    tags = component_tags;
                } else if let Some(component_timestamp) = component.strip_prefix('T') {
                    timestamp = component_timestamp.parse().ok()?;
                }
            }

            Some((
                BucketKey {
                    timestamp,
                    ty,
                    name: name.into(),
                    tags: tags.into(),
                },
                value,
            ))
        }

        if let Some(parsed_metric) = parse(metric) {
            let _ = self.sender.send(Task::SendMetrics(parsed_metric));
        }
    }

    pub fn flush(&self) {
        let _ = self.sender.send(Task::Flush);
    }

    fn worker_thread(receiver: Receiver<Task>, shutdown: Arc<AtomicBool>, transport: TransportArc) {
        let mut buckets = AggregateMetrics::new();
        let mut last_flush = Instant::now();

        loop {
            if shutdown.load(Ordering::SeqCst) {
                Self::flush_buckets(buckets, &transport);
                return;
            }

            let timeout = FLUSH_INTERVAL
                .checked_sub(last_flush.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            match receiver.recv_timeout(timeout) {
                Err(RecvTimeoutError::Timeout) | Ok(Task::Flush) => {
                    // flush
                    Self::flush_buckets(std::mem::take(&mut buckets), &transport);
                    last_flush = Instant::now();
                }
                Ok(Task::SendMetrics((mut key, value))) => {
                    // aggregate
                    let rounding_interval = FLUSH_INTERVAL.as_secs();
                    let rounded_timestamp = (key.timestamp / rounding_interval) * rounding_interval;

                    key.timestamp = rounded_timestamp;

                    match buckets.entry(key) {
                        Entry::Occupied(mut entry) => {
                            let _ = entry.get_mut().merge(value);
                        }
                        Entry::Vacant(entry) => {
                            entry.insert(value);
                        }
                    }
                }
                _ => {
                    // shutdown
                    Self::flush_buckets(buckets, &transport);
                    return;
                }
            }
        }
    }

    fn flush_buckets(buckets: AggregateMetrics, transport: &TransportArc) {
        fn format_payload(buckets: AggregateMetrics) -> std::io::Result<Vec<u8>> {
            use std::io::Write;
            let mut out = vec![];
            for (key, value) in buckets {
                write!(&mut out, "{}", key.name)?;

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
                    write!(&mut out, "|#{}", key.tags)?;
                }
                writeln!(&mut out, "|T{}", key.timestamp)?;
            }

            Ok(out)
        }

        let Ok(output) = format_payload(buckets) else {
            return;
        };

        let mut envelope = Envelope::new();
        envelope.add_item(EnvelopeItem::Metrics(output));

        if let Some(ref transport) = *transport.read().unwrap() {
            transport.send_envelope(envelope);
        }
    }
}

impl Drop for MetricFlusher {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = self.sender.send(Task::Shutdown);
        if let Some(handle) = self.handle.take() {
            handle.join().unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::from_utf8;

    use cadence::{Counted, Distributed};

    use crate::test::with_captured_envelopes;

    use super::*;

    #[test]
    fn test_basic_metrics() {
        let envelopes = with_captured_envelopes(|| {
            let sink = SentryMetricSink::try_new(cadence::NopMetricSink).unwrap();

            let client = cadence::StatsdClient::from_sink("sentry.test", sink);
            client.count("some.count", 1).unwrap();
            client.count("some.count", 10).unwrap();
            client
                .count_with_tags("count.with.tags", 1)
                .with_tag("foo", "bar")
                .send();
            client.distribution("some.distr", 1).unwrap();
            client.distribution("some.distr", 2).unwrap();
            client.distribution("some.distr", 3).unwrap();
        });
        assert_eq!(envelopes.len(), 1);

        let mut items = envelopes[0].items();
        if let Some(EnvelopeItem::Metrics(metrics)) = items.next() {
            let metrics = from_utf8(metrics).unwrap();

            println!("{metrics}");

            assert!(metrics.contains("sentry.test.count.with.tags:1|c|#foo:bar|T"));
            assert!(metrics.contains("sentry.test.some.count:11|c|T"));
            assert!(metrics.contains("sentry.test.some.distr:1:2:3|d|T"));
        } else {
            panic!("expected metrics");
        }
        assert_eq!(items.next(), None);
    }
}

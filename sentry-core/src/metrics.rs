use relay_metrics::{Bucket, UnixTimestamp};

use crate::client::TransportArc;
use crate::Client;

pub struct SentryMetricSink {
    client: Client,
}

impl cadence::MetricSink for SentryMetricSink {
    fn emit(&self, metric: &str) -> std::io::Result<usize> {
        self.client.send_metric(metric);
        Ok(metric.len())
    }

    fn flush(&self) -> std::io::Result<()> {
        if self.client.flush(None) {
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Flushing Client failed",
            ))
        }
    }
}

pub struct MetricFlusher {
    transport: TransportArc,
}

impl MetricFlusher {
    pub fn new(transport: TransportArc) -> Self {
        Self { transport }
    }

    pub fn send_metric(&self, metric: &str) {
        let parsed_metric = Bucket::parse(metric.as_bytes(), UnixTimestamp::now());
    }

    pub fn flush(&self) {}
}

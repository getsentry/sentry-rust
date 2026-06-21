//! Module containing types related to [Client Reports].
//!
//! [Client Reports]: https://develop.sentry.dev/sdk/telemetry/client-reports/

use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use self::list::ClientReportList;
use crate::utils;

pub(crate) use self::envelope_losses::envelope_item_losses;
pub use self::envelope_losses::ItemLoss;
pub use self::list::Item;

mod envelope_losses;
mod list;

/// A [client report].
///
/// [client report]: https://develop.sentry.dev/sdk/telemetry/client-reports/
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Report {
    #[serde(default = "SystemTime::now", with = "utils::ts_seconds_float")]
    timestamp: SystemTime,
    discarded_events: ClientReportList,
}

indexed_enum! {
    /// The reason why a telemetry item was discarded.
    ///
    /// Valid discard reasons are listed in the [develop docs]; this enum may only define a subset of
    /// these data categories, but we will add further categories as we begin using them in the SDK.
    ///
    /// [develop docs]: https://develop.sentry.dev/sdk/telemetry/client-reports/#discard-reasons-1
    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone, Copy)]
    #[serde(rename_all = "snake_case")]
    #[non_exhaustive]
    pub enum Reason {}

    /// The category of data which was dropped.
    ///
    /// Valid categories are listed in the [develop docs]; this enum may only define a subset of these
    /// valid data categories, but we will add further categories as we begin using them in the SDK.
    ///
    /// [develop docs]: https://develop.sentry.dev/sdk/foundations/transport/rate-limiting/#definitions
    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone, Copy)]
    #[serde(rename_all = "snake_case")]
    #[non_exhaustive]
    pub enum Category {
        /// An error event.
        Error,
        /// A session update or quantity of session outcomes contained in a session aggregate.
        Session,
        /// A transaction event.
        ///
        /// Dropped transactions should also be counted as dropped [`Span`]s: one for the
        /// transaction root span extracted by Relay, plus one for each child span.
        ///
        /// [`Span`]: Category::Span
        Transaction,
        /// A span.
        ///
        /// When counting spans for a dropped transaction, the quantity includes all child spans
        /// plus one for the transaction root span extracted by Relay.
        Span,
        /// A quantity of attachment bytes.
        Attachment,
        /// A monitor check-in.
        Monitor,
        /// A log item.
        ///
        /// Dropped logs should also be counted as dropped [`LogByte`]s so client reports include
        /// the approximate volume of dropped log data.
        ///
        /// [`LogByte`]: Category::LogByte
        LogItem,
        /// A quantity of log bytes.
        ///
        /// This complements [`LogItem`]: `log_item` counts dropped logs, while `log_byte` counts
        /// their serialized size.
        ///
        /// [`LogItem`]: Category::LogItem
        LogByte,
        /// A trace metric item.
        ///
        /// Dropped trace metrics should also be counted as dropped [`TraceMetricByte`]s so client
        /// reports include the approximate volume of dropped metric data.
        ///
        /// [`TraceMetricByte`]: Category::TraceMetricByte
        TraceMetric,
        /// A quantity of trace metric bytes.
        ///
        /// This complements [`TraceMetric`]: `trace_metric` counts dropped trace metrics, while
        /// `trace_metric_byte` counts their serialized size.
        ///
        /// [`TraceMetric`]: Category::TraceMetric
        TraceMetricByte,
    }
}

impl Report {
    /// Create a new [`Report`] with the current timestamp, containing the provided client
    /// report items.
    ///
    /// No aggregation is performed on the items; therefore, the calling code should aggregate the
    /// counts for each unique data category and discard reason pair.
    pub fn new<I>(reports: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<Item>,
    {
        let timestamp = SystemTime::now();
        let discarded_events = reports.into_iter().map(Into::into).collect();

        Self {
            timestamp,
            discarded_events,
        }
    }
}

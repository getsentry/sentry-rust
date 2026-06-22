//! Computes client report loss categories and quantities for dropped envelope items.

use std::mem;

use crate::protocol::v7::{
    Attachment, ClientReport, Envelope, EnvelopeItem, Event, ItemContainer, Log, Metric,
    MonitorCheckIn, SessionAggregateItem, SessionAggregates, SessionUpdate, Span, Transaction,
};

use super::{relay_size, Category};

/// A trait for protocol types which can be a source of lost Sentry data if discarded.
pub trait LossSource: private::Sealed {
    /// Returns an iterator over the [`ItemLoss`] values to record if this value is discarded.
    fn losses(&self) -> impl Iterator<Item = ItemLoss> + '_;
}

/// Information about a lost item.
///
/// This only includes the data category and the loss quantity, not the reason for the loss, hence
/// this is distinct from a [`Report`].
///
/// [`Report`]: super::Report
#[non_exhaustive]
pub struct ItemLoss {
    /// The client report data category of the lost item.
    pub category: Category,
    /// The number of lost items or bytes, depending on the category.
    pub quantity: u64,
}

impl LossSource for Envelope {
    fn losses(&self) -> impl Iterator<Item = ItemLoss> + '_ {
        self.items().flat_map(EnvelopeItem::losses)
    }
}

/// An iterator over up to two [`ItemLoss`] values for a discarded protocol item.
#[derive(Default)]
enum ItemLossIter {
    #[default]
    Empty,
    One(ItemLoss),
    Two(ItemLoss, ItemLoss),
}

impl Iterator for ItemLossIter {
    type Item = ItemLoss;

    fn next(&mut self) -> Option<Self::Item> {
        let (rv, next) = match mem::take(self) {
            Self::Empty => (None, Self::Empty),
            Self::One(info) => (Some(info), Self::Empty),
            Self::Two(info1, info2) => (Some(info1), Self::One(info2)),
        };

        *self = next;
        rv
    }
}

impl LossSource for EnvelopeItem {
    fn losses(&self) -> impl Iterator<Item = ItemLoss> + '_ {
        envelope_item_losses(self)
    }
}

impl LossSource for Event<'_> {
    fn losses(&self) -> impl Iterator<Item = ItemLoss> + '_ {
        event_losses(self)
    }
}

impl LossSource for SessionUpdate<'_> {
    fn losses(&self) -> impl Iterator<Item = ItemLoss> + '_ {
        session_update_losses(self)
    }
}

impl LossSource for SessionAggregates<'_> {
    fn losses(&self) -> impl Iterator<Item = ItemLoss> + '_ {
        session_aggregate_losses(self)
    }
}

impl LossSource for Transaction<'_> {
    fn losses(&self) -> impl Iterator<Item = ItemLoss> + '_ {
        transaction_losses(self)
    }
}

impl LossSource for Attachment {
    fn losses(&self) -> impl Iterator<Item = ItemLoss> + '_ {
        attachment_losses(self)
    }
}

impl LossSource for MonitorCheckIn {
    fn losses(&self) -> impl Iterator<Item = ItemLoss> + '_ {
        monitor_check_in_losses(self)
    }
}

impl LossSource for ClientReport {
    fn losses(&self) -> impl Iterator<Item = ItemLoss> + '_ {
        client_report_losses(self)
    }
}

impl LossSource for ItemContainer {
    fn losses(&self) -> impl Iterator<Item = ItemLoss> + '_ {
        item_container_losses(self)
    }
}

impl LossSource for Span {
    fn losses(&self) -> impl Iterator<Item = ItemLoss> + '_ {
        span_losses(self)
    }
}

/// Returns an iterator over the lost items in an envelope item, if it is dropped.
fn envelope_item_losses(envelope_item: &EnvelopeItem) -> ItemLossIter {
    match envelope_item {
        EnvelopeItem::Event(event) => event_losses(event),
        EnvelopeItem::SessionUpdate(update) => session_update_losses(update),
        EnvelopeItem::SessionAggregates(session_aggregates) => {
            session_aggregate_losses(session_aggregates)
        }
        EnvelopeItem::Transaction(transaction) => transaction_losses(transaction),
        EnvelopeItem::Attachment(attachment) => attachment_losses(attachment),
        EnvelopeItem::MonitorCheckIn(check_in) => monitor_check_in_losses(check_in),
        EnvelopeItem::ClientReport(client_report) => client_report_losses(client_report),
        EnvelopeItem::ItemContainer(item_container) => item_container_losses(item_container),
        EnvelopeItem::Raw => ItemLossIter::new([]),
    }
}

/// Returns error-event losses for a discarded event.
fn event_losses(_event: &Event<'_>) -> ItemLossIter {
    ItemLossIter::new([ItemLoss::new(Category::Error, 1)])
}

/// Returns session losses for a discarded session update.
fn session_update_losses(_update: &SessionUpdate<'_>) -> ItemLossIter {
    ItemLossIter::new([ItemLoss::new(Category::Session, 1)])
}

/// Returns session losses from aggregate bucket status counts.
/// The quantity is the saturated sum of exited, errored, abnormal, and crashed sessions.
fn session_aggregate_losses(session_aggregates: &SessionAggregates<'_>) -> ItemLossIter {
    let quantity = session_aggregates
        .aggregates
        .iter()
        .flat_map(
            |&SessionAggregateItem {
                 started: _,
                 distinct_id: _,
                 exited,
                 errored,
                 abnormal,
                 crashed,
             }| { [exited, errored, abnormal, crashed] },
        )
        .map(u64::from)
        .reduce(|sum, v| sum.saturating_add(v))
        .unwrap_or_default();

    ItemLossIter::new([ItemLoss::new(Category::Session, quantity)])
}

/// Returns one transaction loss and the span losses for a transaction item.
/// Span quantity includes the transaction root span plus all child spans.
fn transaction_losses(transaction: &Transaction<'_>) -> ItemLossIter {
    ItemLossIter::new([
        ItemLoss::new(Category::Transaction, 1),
        ItemLoss::new(
            Category::Span,
            transaction
                .spans
                .len()
                .try_into()
                .unwrap_or(u64::MAX)
                .saturating_add(1),
        ),
    ])
}

/// Returns attachment losses measured by serialized payload bytes.
/// The quantity is the attachment buffer length, saturated to `u64::MAX`.
fn attachment_losses(attachment: &Attachment) -> ItemLossIter {
    ItemLossIter::new([ItemLoss::new(
        Category::Attachment,
        attachment.buffer.len().try_into().unwrap_or(u64::MAX),
    )])
}

/// Returns monitor losses for a discarded check-in.
fn monitor_check_in_losses(_check_in: &MonitorCheckIn) -> ItemLossIter {
    ItemLossIter::new([ItemLoss::new(Category::Monitor, 1)])
}

/// Returns no losses for a discarded client report.
///
/// Client reports are never themselves recorded as losses.
fn client_report_losses(_client_report: &ClientReport) -> ItemLossIter {
    ItemLossIter::new([])
}

/// Returns losses for the container's item kind.
/// Each container variant maps to the category and quantity used by Relay.
fn item_container_losses(item_container: &ItemContainer) -> ItemLossIter {
    match item_container {
        ItemContainer::Logs(logs) => log_losses(logs),
        ItemContainer::Metrics(metrics) => metric_losses(metrics),
    }
}

/// Returns log losses measured by item count and Relay-compatible content size.
fn log_losses(logs: &[Log]) -> ItemLossIter {
    let item_quantity = logs.len().try_into().unwrap_or(u64::MAX);
    let byte_quantity = logs.iter().fold(0u64, |sum, log| {
        sum.saturating_add(relay_size::log_byte_size(log))
    });

    ItemLossIter::new([
        ItemLoss::new(Category::LogItem, item_quantity),
        ItemLoss::new(Category::LogByte, byte_quantity),
    ])
}

/// Returns trace metric losses measured by item count and Relay-compatible content size.
fn metric_losses(metrics: &[Metric]) -> ItemLossIter {
    let item_quantity = metrics.len().try_into().unwrap_or(u64::MAX);
    let byte_quantity = metrics.iter().fold(0u64, |sum, metric| {
        sum.saturating_add(relay_size::metric_byte_size(metric))
    });

    ItemLossIter::new([
        ItemLoss::new(Category::TraceMetric, item_quantity),
        ItemLoss::new(Category::TraceMetricByte, byte_quantity),
    ])
}

/// A span always results in a loss of a single span.
fn span_losses(_span: &Span) -> ItemLossIter {
    ItemLossIter::new([ItemLoss::new(Category::Span, 1)])
}

impl ItemLossIter {
    /// Creates an iterator from zero, one, or two [`ItemLoss`] values.
    fn new<T>(value: T) -> Self
    where
        T: Into<Self>,
    {
        value.into()
    }
}

impl ItemLoss {
    /// Creates a new item loss with the given category and quantity.
    fn new(category: Category, quantity: u64) -> Self {
        Self { category, quantity }
    }
}

impl From<[ItemLoss; 0]> for ItemLossIter {
    fn from(_: [ItemLoss; 0]) -> Self {
        Self::Empty
    }
}

impl From<[ItemLoss; 1]> for ItemLossIter {
    fn from(value: [ItemLoss; 1]) -> Self {
        let [info] = value;
        Self::One(info)
    }
}

impl From<[ItemLoss; 2]> for ItemLossIter {
    fn from(value: [ItemLoss; 2]) -> Self {
        let [info1, info2] = value;
        Self::Two(info1, info2)
    }
}

mod private {
    use super::{
        Attachment, ClientReport, Envelope, EnvelopeItem, Event, ItemContainer, MonitorCheckIn,
        SessionAggregates, SessionUpdate, Span, Transaction,
    };

    /// Prevents downstream implementations of [`LossSource`](super::LossSource).
    pub trait Sealed {}

    impl Sealed for EnvelopeItem {}
    impl Sealed for Event<'_> {}
    impl Sealed for SessionUpdate<'_> {}
    impl Sealed for Envelope {}
    impl Sealed for SessionAggregates<'_> {}
    impl Sealed for Transaction<'_> {}
    impl Sealed for Attachment {}
    impl Sealed for MonitorCheckIn {}
    impl Sealed for ClientReport {}
    impl Sealed for ItemContainer {}
    impl Sealed for Span {}
}

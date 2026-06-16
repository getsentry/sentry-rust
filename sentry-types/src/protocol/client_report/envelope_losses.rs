//! Computes client report loss categories and quantities for dropped envelope items.

use std::io::{Result as IoResult, Write};
use std::iter::FlatMap;
use std::mem;

use crate::protocol::v7::{
    Attachment, EnvelopeItem, EnvelopeItemIter, ItemContainer, Log, Metric, SessionAggregateItem,
    SessionAggregates, Transaction,
};

use super::Category;

type EnvelopeLossIterInner<'a> =
    FlatMap<EnvelopeItemIter<'a>, ItemLossIter, fn(&'a EnvelopeItem) -> ItemLossIter>;

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

/// An iterator over [`ItemLoss`] values for a dropped envelope.
pub struct EnvelopeLossIter<'a> {
    inner: EnvelopeLossIterInner<'a>,
}

/// An iterator over [`ItemLoss`].
pub(crate) struct ItemLossIter {
    inner: ItemLossIterInner,
}

impl Iterator for EnvelopeLossIter<'_> {
    type Item = ItemLoss;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl Iterator for ItemLossIter {
    type Item = ItemLoss;

    fn next(&mut self) -> Option<Self::Item> {
        let (rv, new_inner) = match mem::take(&mut self.inner) {
            ItemLossIterInner::Empty => (None, ItemLossIterInner::Empty),
            ItemLossIterInner::One(info) => (Some(info), ItemLossIterInner::Empty),
            ItemLossIterInner::Two(info1, info2) => (Some(info1), ItemLossIterInner::One(info2)),
        };

        self.inner = new_inner;
        rv
    }
}

/// Returns an iterator over the lost items in an envelope item, if it is dropped.
pub(crate) fn envelope_item_losses(envelope_item: &EnvelopeItem) -> ItemLossIter {
    match envelope_item {
        EnvelopeItem::Event(_) => ItemLossIter::new([ItemLoss::new(Category::Error, 1)]),
        EnvelopeItem::SessionUpdate(_) => ItemLossIter::new([ItemLoss::new(Category::Session, 1)]),
        EnvelopeItem::SessionAggregates(session_aggregates) => {
            session_aggregate_losses(session_aggregates)
        }
        EnvelopeItem::Transaction(transaction) => transaction_losses(transaction),
        EnvelopeItem::Attachment(attachment) => attachment_losses(attachment),
        EnvelopeItem::MonitorCheckIn(_) => ItemLossIter::new([ItemLoss::new(Category::Monitor, 1)]),
        EnvelopeItem::ClientReport(_) => ItemLossIter::new([]),
        EnvelopeItem::ItemContainer(item_container) => item_container_losses(item_container),
        EnvelopeItem::Raw => ItemLossIter::new([]),
    }
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

/// Returns losses for the container's item kind.
/// Each container variant maps to the category and quantity used by Relay.
fn item_container_losses(item_container: &ItemContainer) -> ItemLossIter {
    match item_container {
        ItemContainer::Logs(logs) => log_losses(logs),
        ItemContainer::Metrics(metrics) => metric_losses(metrics),
    }
}

/// Returns log losses measured by item count and serialized bytes.
/// Logs that fail serialization contribute zero bytes because they could not be sent.
fn log_losses(logs: &[Log]) -> ItemLossIter {
    let item_quantity = u64::try_from(logs.len()).unwrap_or(u64::MAX);
    let byte_quantity = logs
        .iter()
        .map(|log| {
            let mut sink = CountingSink::default();
            serde_json::to_writer(&mut sink, log)
                .map(|()| sink.bytes_written)
                // If serialization fails, then we wouldn't have been able to send the log.
                // So, nothing is lost.
                .unwrap_or_default()
                .try_into()
                .unwrap_or(u64::MAX)
        })
        .reduce(|sum, v| sum.saturating_add(v))
        .unwrap_or_default();

    ItemLossIter::new([
        ItemLoss::new(Category::LogItem, item_quantity),
        ItemLoss::new(Category::LogByte, byte_quantity),
    ])
}

/// Returns trace metric losses measured by metric item count.
/// The quantity is saturated to `u64::MAX`.
fn metric_losses(metrics: &[Metric]) -> ItemLossIter {
    ItemLossIter::new([ItemLoss::new(
        Category::TraceMetric,
        u64::try_from(metrics.len()).unwrap_or(u64::MAX),
    )])
}

impl<'a> EnvelopeLossIter<'a> {
    pub(crate) fn new(inner: EnvelopeLossIterInner<'a>) -> Self {
        Self { inner }
    }
}

#[derive(Default)]
enum ItemLossIterInner {
    #[default]
    Empty,
    One(ItemLoss),
    Two(ItemLoss, ItemLoss),
}

/// A sink which counts bytes written to it, without storing them anywhere.
#[derive(Default)]
struct CountingSink {
    bytes_written: usize,
}

impl ItemLossIter {
    fn new<T>(value: T) -> Self
    where
        T: Into<ItemLossIterInner>,
    {
        let inner = value.into();
        Self { inner }
    }
}

impl ItemLoss {
    fn new(category: Category, quantity: u64) -> Self {
        Self { category, quantity }
    }
}

impl From<[ItemLoss; 0]> for ItemLossIterInner {
    fn from(_: [ItemLoss; 0]) -> Self {
        Self::Empty
    }
}

impl From<[ItemLoss; 1]> for ItemLossIterInner {
    fn from(value: [ItemLoss; 1]) -> Self {
        let [info] = value;
        Self::One(info)
    }
}

impl From<[ItemLoss; 2]> for ItemLossIterInner {
    fn from(value: [ItemLoss; 2]) -> Self {
        let [info1, info2] = value;
        Self::Two(info1, info2)
    }
}

impl Write for CountingSink {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.bytes_written = self.bytes_written.saturating_add(buf.len());
        Ok(buf.len())
    }

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

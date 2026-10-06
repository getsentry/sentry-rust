use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;

use sentry_core::protocol::Value;
use sentry_core::{Hub, HubSwitchGuard, TransactionOrSpan};
use tracing_core::field::Visit;
use tracing_core::{span, Field, Metadata, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

use super::span_guard_stack::SpanGuardStack;
use crate::converters::FieldVisitor;
use crate::{SENTRY_NAME_FIELD, SENTRY_OP_FIELD, SENTRY_TRACE_FIELD, TAGS_PREFIX};

/// Captures tracing spans as Sentry transactions and spans.
///
/// Install this layer to capture spans; event layers do not manage the span lifecycle.
/// Without a filter, this layer captures spans at every level. Configure a filter to limit span
/// volume; see [`span_layer`] for the recommended levels.
#[non_exhaustive]
pub struct SpanLayer {}

/// Creates a layer that captures tracing spans as Sentry transactions and spans.
///
/// # Filtering
///
/// Without a filter, this layer captures spans at every level. Configure a level filter to limit
/// span volume. We recommend capturing spans at the `INFO` level, and more severe.
///
/// ```rust
/// # use tracing_subscriber::prelude::*;
/// use sentry::integrations::tracing as sentry_tracing;
/// use tracing_subscriber::filter::LevelFilter;
///
/// tracing_subscriber::registry()
///     .with(sentry_tracing::span_layer().with_filter(LevelFilter::INFO))
///     .init();
///
/// // This will be captured ...
/// let _info = tracing::info_span!("INFO span").entered();
///
/// // ... but this will not be, due to LevelFilter::INFO being set.
/// let _debug = tracing::debug_span!("DEBUG span").entered();
/// ```
pub fn span_layer() -> SpanLayer {
    SpanLayer {}
}

#[inline(always)]
fn record_fields<'a, K: AsRef<str> + Into<Cow<'a, str>>>(
    span: &TransactionOrSpan,
    data: BTreeMap<K, Value>,
) {
    match span {
        TransactionOrSpan::Span(span) => {
            let mut span = span.data();
            for (key, value) in data {
                if let Some(stripped_key) = key.as_ref().strip_prefix(TAGS_PREFIX) {
                    match value {
                        Value::Bool(value) => {
                            span.set_tag(stripped_key.to_owned(), value.to_string())
                        }
                        Value::Number(value) => {
                            span.set_tag(stripped_key.to_owned(), value.to_string())
                        }
                        Value::String(value) => span.set_tag(stripped_key.to_owned(), value),
                        _ => span.set_data(key.into().into_owned(), value),
                    }
                } else {
                    span.set_data(key.into().into_owned(), value);
                }
            }
        }
        TransactionOrSpan::Transaction(transaction) => {
            let mut transaction = transaction.data();
            for (key, value) in data {
                if let Some(stripped_key) = key.as_ref().strip_prefix(TAGS_PREFIX) {
                    match value {
                        Value::Bool(value) => {
                            transaction.set_tag(stripped_key.into(), value.to_string())
                        }
                        Value::Number(value) => {
                            transaction.set_tag(stripped_key.into(), value.to_string())
                        }
                        Value::String(value) => transaction.set_tag(stripped_key.into(), value),
                        _ => transaction.set_data(key.into(), value),
                    }
                } else {
                    transaction.set_data(key.into(), value);
                }
            }
        }
    }
}

/// Data attached to tracing span extensions, read by the event converters and
/// finished when the corresponding tracing span closes.
pub(crate) struct SentrySpanData {
    pub(crate) sentry_span: TransactionOrSpan,
    hub: Arc<Hub>,
}

impl<S> Layer<S> for SpanLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    /// When a new Span gets created, start a new sentry span, setting it as the *current* sentry
    /// span.
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
        let span = match ctx.span(id) {
            Some(span) => span,
            None => return,
        };

        let (data, sentry_name, sentry_op, sentry_trace) = extract_span_data(attrs);
        let sentry_name = sentry_name.as_deref().unwrap_or_else(|| span.name());
        let sentry_op =
            sentry_op.unwrap_or_else(|| format!("{}::{}", span.metadata().target(), span.name()));

        let hub = Hub::current();
        let parent_sentry_span = hub.configure_scope(|scope| scope.get_span());

        let mut sentry_span: TransactionOrSpan = match &parent_sentry_span {
            Some(parent) => parent.start_child(&sentry_op, sentry_name).into(),
            None => {
                let ctx = if let Some(trace_header) = sentry_trace {
                    sentry_core::TransactionContext::continue_from_headers(
                        sentry_name,
                        &sentry_op,
                        [("sentry-trace", trace_header.as_str())],
                    )
                } else {
                    sentry_core::TransactionContext::new(sentry_name, &sentry_op)
                };

                let tx = sentry_core::start_transaction(ctx);
                tx.set_origin("auto.tracing");
                tx.into()
            }
        };
        record_fields(&sentry_span, data);
        set_default_attributes(&mut sentry_span, span.metadata());

        let mut extensions = span.extensions_mut();
        extensions.insert(SentrySpanData { sentry_span, hub });
    }

    /// Sets the entered span as *current* sentry span.
    ///
    /// A tracing span can be entered and exited multiple times, for example,
    /// when using a `tracing::Instrumented` future.
    ///
    /// Spans must be exited on the same thread that they are entered. The
    /// `sentry-tracing` integration's behavior is undefined if spans are
    /// exited on threads other than the one they are entered from;
    /// specifically, doing so will likely cause data to bleed between
    /// [`Hub`]s in unexpected ways.
    fn on_enter(&self, id: &span::Id, ctx: Context<'_, S>) {
        let span = match ctx.span(id) {
            Some(span) => span,
            None => return,
        };

        let extensions = span.extensions();
        if let Some(data) = extensions.get::<SentrySpanData>() {
            let hub = Arc::new(Hub::new_from_top(&data.hub));
            hub.configure_scope(|scope| {
                scope.set_span(Some(data.sentry_span.clone()));
            });

            let guard = HubSwitchGuard::new(hub);
            SPAN_GUARDS.with(|guards| {
                guards.borrow_mut().push(id.clone(), guard);
            });
        }
    }

    /// Drop the current span's [`HubSwitchGuard`] to restore the parent [`Hub`].
    fn on_exit(&self, id: &span::Id, ctx: Context<'_, S>) {
        let popped = SPAN_GUARDS.with(|guards| guards.borrow_mut().pop(id.clone()));

        sentry_core::debug_assert_or_log!(
            popped.is_some()
                || ctx
                    .span(id)
                    .is_none_or(|span| span.extensions().get::<SentrySpanData>().is_none()),
            "[SentrySpanLayer] missing HubSwitchGuard on exit for span {id:?}. \
            This span has been exited more times on this thread than it has been entered, \
            likely due to dropping an `Entered` guard in a different thread than where it was \
            entered. This mismatch will likely cause the sentry-tracing layer to leak memory."
        );
    }

    /// Finish the underlying sentry span when the tracing span closes.
    fn on_close(&self, id: span::Id, ctx: Context<'_, S>) {
        let span = match ctx.span(&id) {
            Some(span) => span,
            None => return,
        };

        let mut extensions = span.extensions_mut();
        let SentrySpanData { sentry_span, .. } = match extensions.remove::<SentrySpanData>() {
            Some(data) => data,
            None => return,
        };
        sentry_span.finish();
    }

    /// Implement the writing of extra data to span.
    fn on_record(&self, span: &span::Id, values: &span::Record<'_>, ctx: Context<'_, S>) {
        let span = match ctx.span(span) {
            Some(s) => s,
            _ => return,
        };

        let mut extensions = span.extensions_mut();
        let span = match extensions.get_mut::<SentrySpanData>() {
            Some(t) => &t.sentry_span,
            _ => return,
        };

        let mut data = FieldVisitor::default();
        values.record(&mut data);

        let sentry_name = data
            .json_values
            .remove(SENTRY_NAME_FIELD)
            .and_then(|v| match v {
                Value::String(s) => Some(s),
                _ => None,
            });

        let sentry_op = data
            .json_values
            .remove(SENTRY_OP_FIELD)
            .and_then(|v| match v {
                Value::String(s) => Some(s),
                _ => None,
            });

        // `sentry.trace` cannot be applied retroactively
        data.json_values.remove(SENTRY_TRACE_FIELD);

        if let Some(name) = sentry_name {
            span.set_name(&name);
        }
        if let Some(op) = sentry_op {
            span.set_op(&op);
        }

        record_fields(span, data.json_values);
    }
}

fn set_default_attributes(span: &mut TransactionOrSpan, metadata: &Metadata<'_>) {
    span.set_data("sentry.tracing.target", metadata.target().into());

    if let Some(module) = metadata.module_path() {
        span.set_data("code.module.name", module.into());
    }
    if let Some(file) = metadata.file() {
        span.set_data("code.file.path", file.into());
    }
    if let Some(line) = metadata.line() {
        span.set_data("code.line.number", line.into());
    }
}

/// Extracts the attributes from a span, returning Sentry's special fields separately.
fn extract_span_data(
    attrs: &span::Attributes,
) -> (
    BTreeMap<&'static str, Value>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    let mut json_values = VISITOR_BUFFER.with_borrow_mut(|debug_buffer| {
        let mut visitor = SpanFieldVisitor {
            debug_buffer,
            json_values: Default::default(),
        };
        attrs.record(&mut visitor);
        visitor.json_values
    });

    let name = json_values.remove(SENTRY_NAME_FIELD).and_then(|v| match v {
        Value::String(s) => Some(s),
        _ => None,
    });
    let op = json_values.remove(SENTRY_OP_FIELD).and_then(|v| match v {
        Value::String(s) => Some(s),
        _ => None,
    });
    let sentry_trace = json_values
        .remove(SENTRY_TRACE_FIELD)
        .and_then(|v| match v {
            Value::String(s) => Some(s),
            _ => None,
        });

    (json_values, name, op, sentry_trace)
}

thread_local! {
    static VISITOR_BUFFER: RefCell<String> = const { RefCell::new(String::new()) };
    /// Hub switch guards keyed by span ID.
    ///
    /// Guard bookkeeping is thread-local by design. Correctness expects
    /// balanced enter/exit callbacks on the same thread.
    static SPAN_GUARDS: RefCell<SpanGuardStack> = RefCell::new(SpanGuardStack::new());
}

/// Records all span fields into a `BTreeMap`, reusing a mutable `String` as buffer.
struct SpanFieldVisitor<'s> {
    debug_buffer: &'s mut String,
    json_values: BTreeMap<&'static str, Value>,
}

impl SpanFieldVisitor<'_> {
    fn record<T: Into<Value>>(&mut self, field: &Field, value: T) {
        self.json_values.insert(field.name(), value.into());
    }
}

impl Visit for SpanFieldVisitor<'_> {
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.record(field, value);
    }
    fn record_u64(&mut self, field: &Field, value: u64) {
        self.record(field, value);
    }
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.record(field, value);
    }
    fn record_f64(&mut self, field: &Field, value: f64) {
        self.record(field, value);
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        self.record(field, value);
    }
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        use std::fmt::Write;
        self.debug_buffer.reserve(128);
        write!(self.debug_buffer, "{value:?}").unwrap();
        self.json_values
            .insert(field.name(), self.debug_buffer.as_str().into());
        self.debug_buffer.clear();
    }
}

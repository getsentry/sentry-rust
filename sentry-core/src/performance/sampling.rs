//! This module contains some types that represent sampling decisions.

#[cfg(doc)]
use sentry_types::protocol::v7::client_report;

/// Represents the tracing state of a transaction.
///
/// The possible representations depend on whether tracing is enabled or disabled in this SDK.
///
/// ### If tracing is enabled
///
/// We always have a sampling decision. This decision is propagated from an incoming trace when
/// available, otherwise we make the decision according to the configured sample rate.
///
/// ### If tracing is disabled
///
/// For traces started by this SDK, the sampling decision is deferred. No sampling decision is
/// available.
///
/// If this SDK is continuing an incoming trace, we may have a sampling decision if the incoming
/// trace propagated a sampling decision. As tracing is disabled, the SDK will not sample any
/// spans regardless of the sampling decision, but the incoming tracing decision will again get
/// propagated outwards.
#[derive(Debug, Clone, Copy)]
pub(super) enum TracingState {
    /// Tracing is enabled. In this case, there must be a sampling decision.
    Enabled(SamplingDecision),
    /// Tracing is disabled. In this case, we only have a tracing decision when continuing a trace
    /// that has a sampling decision.
    Disabled(Option<SamplingDecision>),
}

impl TracingState {
    /// Create a new [`TracingState::Enabled`] with the given sampling decision made at the given
    /// sample rate.
    pub(super) fn new_enabled(sampled: bool, sample_rate: f32) -> Self {
        Self::Enabled(SamplingDecision {
            sampled,
            sample_rate,
        })
    }

    /// Create a new [`TracingState::Disabled`] given a sampling decision or `None` if the decision
    /// is deferred.
    ///
    /// The `sample_rate` in the [`SamplingDecision`], if available, is a best-effort estimate of
    /// the sample rate because the SDK does not yet read the sample rate propagated in the baggage
    /// headers. Therefore, we just assume the sample_rate was `1.0` for sampled traces, and `0.0`
    /// for unsampled ones, so that the sample rate is at least consistent with the sampling
    /// decision. Once we read the `sample_rate`, this method should be adjusted to use that rate.
    pub(super) fn new_disabled(sampled: Option<bool>) -> Self {
        let decision = sampled.map(|sampled| SamplingDecision {
            sampled,
            sample_rate: sampled.into(),
        });

        Self::Disabled(decision)
    }

    /// Return whether this trace is sampled, or `None` if no decision is available.
    ///
    /// # ⚠️ Caution
    ///
    /// Never use this method to determine whether the SDK should record spans, as this method
    /// may return `Some(true)` when tracing is disabled, namely, when continuing a sampled trace
    /// in TwP mode. Use [`Self::finish_action`] for this purpose.
    pub(super) fn trace_sampled(&self) -> Option<bool> {
        match self {
            Self::Enabled(decision) | Self::Disabled(Some(decision)) => Some(decision.sampled),
            Self::Disabled(None) => None,
        }
    }

    /// Determine the correct action to take when spans/transactions in this trace are finished.
    ///
    /// See [`FinishAction`] for more details.
    pub(super) fn finish_action(&self) -> FinishAction {
        match *self {
            Self::Enabled(SamplingDecision {
                sampled: true,
                sample_rate,
            }) => FinishAction::Send { sample_rate },

            Self::Enabled(SamplingDecision {
                sampled: false,
                sample_rate: _,
            }) => FinishAction::Discard,

            Self::Disabled(_) => FinishAction::Ignore,
        }
    }
}

/// The trace's sampling decision.
#[derive(Debug, Clone, Copy)]
pub(super) struct SamplingDecision {
    /// The sampling decision.
    pub(super) sampled: bool,
    /// The sample rate at which the decision was made.
    pub(super) sample_rate: f32,
}

/// What the SDK should do with spans/transactions when they are finished.
#[derive(Debug, Clone, Copy)]
pub(super) enum FinishAction {
    /// Send spans/transactions to Sentry.
    ///
    /// This action should be taken for sampled traces when tracing is enabled.
    ///
    /// As we may wish to know the sampling rate used to come to the decision to sample when
    /// finishing the transaction/span, this variant includes the `sample_rate`.
    Send { sample_rate: f32 },
    /// Discard spans/transactions and record a client report with a "sampling rate" reason.
    ///
    /// This action should be taken for unsampled tracing when tracing is enabled.
    Discard,
    /// Ignore spans/transactions. Do not send them to Sentry, and do not record a client report.
    ///
    /// This action should always be taken when tracing is disabled.
    Ignore,
}

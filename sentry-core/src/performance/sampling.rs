//! This module contains some types that represent sampling decisions.

#[cfg(doc)]
use sentry_types::protocol::v7::client_report;
use sentry_types::protocol::v7::{SampleRand, TraceId};

#[cfg(feature = "client")]
use crate::client;

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
///
/// When compiled without the `client` crate feature, only disabled traces can be represented,
/// as enabling tracing requires a client.
#[derive(Debug, Clone, Copy)]
pub(super) enum TracingState {
    /// Tracing is enabled. In this case, there must be a sampling decision.
    #[cfg(feature = "client")]
    Enabled(SamplingDecision),
    /// Tracing is disabled. In this case, we only have a tracing decision when continuing a trace
    /// that has a sampling decision.
    Disabled(Option<SamplingDecision>),
}

impl TracingState {
    /// Create a new [`TracingState::Disabled`] given a sampling decision or `None` if the decision
    /// is deferred.
    ///
    /// The `sample_rate` in the [`SamplingDecision`], if available, is a best-effort estimate of
    /// the sample rate because the SDK does not yet read the sample rate propagated in the baggage
    /// headers. Therefore, we just assume the sample_rate was `1.0` for sampled traces, and `0.0`
    /// for unsampled ones, so that the sample rate is at least consistent with the sampling
    /// decision. Once we read the `sample_rate`, this method should be adjusted to use that rate.
    pub(super) fn new_disabled(sampled: Option<bool>) -> Self {
        // let decision = sampled.map(|sampled| SamplingDecision {
        //     sampled,
        //     #[cfg(feature = "client")]
        //     sample_rate: sampled.into(),
        // });

        // Self::Disabled(decision)
        todo!()
    }

    /// Return whether this trace is sampled, or `None` if no decision is available.
    ///
    /// # ⚠️ Caution
    ///
    /// Never use this method to determine whether the SDK should record spans, as this method
    /// may return `Some(true)` when tracing is disabled, namely, when continuing a sampled trace
    /// in TwP mode. Use [`Self::finish_action`] for this purpose.
    pub(super) fn trace_sampled(&self) -> Option<bool> {
        let decision = match *self {
            #[cfg(feature = "client")]
            Self::Enabled(decision) => Some(decision),
            Self::Disabled(decision) => decision,
        };

        decision.map(|d| d.sampled())
    }

    /// Determine the correct action to take when spans/transactions in this trace are finished.
    ///
    /// See [`FinishAction`] for more details.
    #[cfg(feature = "client")]
    pub(super) fn finish_action(&self) -> FinishAction {
        match *self {
            Self::Enabled(decision) => match decision.sampled() {
                true => FinishAction::Send {
                    sample_rate: decision.sample_rate,
                },
                false => FinishAction::Discard,
            },

            Self::Disabled(_) => FinishAction::Ignore,
        }
    }
}

/// The trace's sampling decision.
#[derive(Debug, Clone, Copy)]
pub(super) struct SamplingDecision {
    /// The random value to compare against the `sample_rate` to get the decision.
    pub(super) sample_rand: SampleRand,
    /// The sample rate at which the decision was made.
    ///
    /// Currently, we only use this on the `client` feature, but if needed we can also provide
    /// this on non-`client` builds.
    #[cfg(feature = "client")]
    pub(super) sample_rate: f32,
}

#[cfg(feature = "client")]
impl SamplingDecision {
    pub(super) fn new_sampled_at(sample_rate: f32) -> Self {
        // let sampled = client::sample_should_send(sample_rate);

        // Self {
        //     sampled,
        //     sample_rate,
        // }
        todo!()
    }

    /// Returns the sampling decision as a boolean.
    pub(super) fn sampled(&self) -> bool {
        self.sample_rand.is_sampled_at(self.sample_rate)
    }
}

/// Makes a sampling decision based on available information.
///
/// This function backfills any missing items so that they are consistent with the other
/// information provided.
///
/// It is possible that the provided `sampled`, `sample_rate`, and `sample_rand` could contradict
/// each other. In this case, we will preserve those values in that order of precedence.
#[must_use]
pub(super) struct SamplingDecider {
    trace_id: TraceId,
    sampled: Option<bool>,
    sample_rate: Option<f32>,
    sample_rand: Option<SampleRand>,
}

impl SamplingDecider {
    /// Create a new decider with the given [`TraceId`], which is used as the randomness seed.
    pub(super) fn new(trace_id: TraceId) -> Self {
        Self {
            trace_id,
            sampled: None,
            sample_rate: None,
            sample_rand: None,
        }
    }

    pub(super) fn sampled(self, sampled: Option<bool>) -> Self {
        Self { sampled, ..self }
    }

    pub(super) fn sample_rate(self, sample_rate: Option<f32>) -> Self {
        Self {
            sample_rate,
            ..self
        }
    }

    pub(super) fn sample_rand(self, sample_rand: Option<SampleRand>) -> Self {
        Self {
            sample_rand,
            ..self
        }
    }

    pub(super) fn decide(self) -> SamplingDecision {
        let Self {
            trace_id,
            sampled,
            sample_rate,
            sample_rand,
        } = self;

        let sample_rand = ensure_consistent_sample_rand(sample_rand, sampled, sample_rate);
    }
}

/// What the SDK should do with spans/transactions when they are finished.
#[cfg(feature = "client")]
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

/// Ensure the provided sample_rand is consistent with the sampling decision and sample rate.
///
/// Returns the sample_rand if yes, otherwise `None`.
fn ensure_consistent_sample_rand(
    sample_rand: Option<SampleRand>,
    sampled: Option<Sampled>,
    sample_rate: Option<f32>,
) -> Option<SampleRand> {
    if let (Some(sample_rand), Some(sampled), Some(sample_rate)) =
        (sample_rand, sampled, sample_rate)
    {
        (sample_rand.is_sampled_at(sample_rate) == sampled).then_some(sample_rand)
    } else {
        sample_rand
    }
}

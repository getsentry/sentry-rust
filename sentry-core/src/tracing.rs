use std::fmt::{Debug, Formatter};

/// Data passed to the `traces_sampler` function,
/// which forms the basis for whatever decisions it might make.
///
/// TODO: This is a placeholder.
#[derive(Debug, Default, Clone, Eq, PartialEq)]
#[non_exhaustive]
pub struct SamplingContext {}

/// Function to compute tracing sample rate dynamically and filter unwanted traces.
pub trait TracesSampler: Fn(SamplingContext) -> bool + Send + Sync {}

impl Debug for dyn TracesSampler {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "TraceSampler {{...}}")
    }
}

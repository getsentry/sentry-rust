use std::fmt::Debug;

/// Data passed to the `traces_sampler` function,
/// which forms the basis for whatever decisions it might make.
///
/// TODO: This is a placeholder.
#[derive(Debug, Default, Clone, Eq, PartialEq)]
#[non_exhaustive]
pub struct SamplingContext {}

/// Function to compute tracing sample rate dynamically and filter unwanted traces.
pub type TracesSampler = fn(SamplingContext) -> bool;

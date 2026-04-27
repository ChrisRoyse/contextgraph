//! Scaling analysis infrastructure.
//!
//! This module provides tools for analyzing how retrieval and clustering
//! performance degrades as corpus size increases.

pub mod corpus_builder;
pub mod degradation;
pub mod memory_profiler;

pub use degradation::{
    DegradationAnalysis, DegradationComparison, DegradationPoint, DegradationRates, ScalingLimits,
    TierImprovement,
};
pub use memory_profiler::{MemoryProfile, MemoryProfiler};

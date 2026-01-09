//! UTL computation metrics and status reporting.
//!
//! This module provides:
//! - `StageThresholds` - Thresholds that vary by lifecycle stage
//! - `QuadrantDistribution` - Johari quadrant classification counts
//! - `UtlComputationMetrics` - ACCUMULATED statistics across computations
//! - `UtlStatus` - Complete status for MCP responses
//!
//! NOTE: `UtlComputationMetrics` is DIFFERENT from the per-computation
//! `UtlMetrics` in context-graph-core.

mod computation;
mod quadrant;
mod status;
mod thresholds;

// Re-export all public types for backwards compatibility
pub use self::computation::UtlComputationMetrics;
pub use self::quadrant::QuadrantDistribution;
pub use self::status::{ThresholdsResponse, UtlStatus, UtlStatusResponse};
pub use self::thresholds::StageThresholds;

//! Phase 4b Teleological Constellation Training (TCT).
//!
//! This crate is fail-closed: missing centroids, missing thresholds,
//! under-sampled calibration cells, stale constellations, provenance drift,
//! malformed RocksDB rows, and non-finite floats are errors. No permissive
//! thresholds or synthetic production fallbacks are used.

pub mod builder;
pub mod calibrator;
pub mod constellation;
pub mod error;
pub mod freshness;
pub mod freshness_policy;
pub mod gtau;
pub mod guard_audit;
pub mod hinge;
pub mod hinge_types;
pub mod inspect;
pub mod panel_slots;
pub mod rate;
pub mod rate_aggregator;
pub mod refresh_report;
pub mod shrinkage;
pub mod shrinkage_engine;
pub mod store;
pub mod types;
pub mod verdict;

pub use builder::*;
pub use calibrator::*;
pub use constellation::*;
pub use error::*;
pub use freshness::*;
pub use freshness_policy::*;
pub use gtau::*;
pub use guard_audit::*;
pub use hinge::*;
pub use hinge_types::*;
pub use inspect::*;
pub use panel_slots::*;
pub use rate::*;
pub use rate_aggregator::*;
pub use refresh_report::*;
pub use shrinkage::*;
pub use shrinkage_engine::*;
pub use store::*;
pub use types::*;
pub use verdict::*;

#[cfg(test)]
mod tests;

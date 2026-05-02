//! DynamicJEPA core contracts.
//!
//! Phase 1 defines strict record types, ids, validation, and versioned bincode
//! codecs. RocksDB storage is intentionally not implemented here; Phase 2 owns
//! the database source of truth.

pub mod adapter;
pub mod artifact;
pub mod binding;
pub mod dataset;
pub mod domain_pack;
pub mod error;
pub mod event;
pub mod features;
pub mod guard;
pub mod ids;
pub mod instrument;
pub mod meaning_compression;
pub mod pair_kinds;
pub mod panel;
pub mod planner;
pub mod predictor;
pub mod record_header;
pub mod schema;
pub mod skill;
pub mod state_action_outcome;
pub mod trajectory;
pub mod validation;
pub mod verification;

#[cfg(test)]
mod tests;

pub use adapter::*;
pub use artifact::*;
pub use binding::*;
pub use dataset::*;
pub use domain_pack::*;
pub use error::{DynamicJepaError, DynamicJepaResult};
pub use event::*;
pub use features::*;
pub use guard::*;
pub use ids::*;
pub use instrument::*;
pub use meaning_compression::*;
pub use pair_kinds::*;
pub use panel::*;
pub use planner::*;
pub use predictor::*;
pub use record_header::*;
pub use schema::*;
pub use skill::*;
pub use state_action_outcome::*;
pub use trajectory::*;
pub use validation::*;
pub use verification::*;

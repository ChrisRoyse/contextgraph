//! DynamicJEPA service crate.
//!
//! Phase 7 owns tiny Candle model training, artifact writing, hash-checked
//! artifact loading, and source-of-truth training/artifact persistence.
//! Phase 8 owns hash-checked prediction, planning, guard decisions, and
//! surprise-event persistence.

pub mod artifact;
pub mod config;
pub mod model;
pub mod service;

pub use artifact::*;
pub use config::*;
pub use model::*;
pub use service::*;

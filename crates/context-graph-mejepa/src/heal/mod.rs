//! ME-JEPA Phase 5 self-healing.
//!
//! The module is fail-closed: every persisted state transition has a RocksDB
//! readback or a file readback verifier, and every operator-visible failure has
//! a stable `MEJEPA_*` code.

pub mod calibration;
pub mod cf;
pub mod dissipation;
pub mod distill;
pub mod dormancy;
pub mod drift;
pub mod drift_attribution;
pub mod drift_bayesian;
pub mod drift_per_cell;
pub mod drift_surprise_weighted;
pub mod drill;
pub mod emergency_eviction;
pub mod errors;
pub mod ewc;
pub mod fisher;
pub mod full_retrain;
pub mod integrity;
pub mod lambda_ramp;
pub mod lora_refresh;
pub mod per_cell_promotion;
pub mod pipeline;
pub mod plasticity;
mod plasticity_metrics;
pub mod policy;
pub mod promote;
pub mod promote_approval;
pub mod readback;
pub mod regulate;
pub mod scheduler;
pub mod scheduler_state;
pub mod store;

pub use calibration::*;
pub use cf::*;
pub use dissipation::*;
pub use distill::*;
pub use dormancy::*;
pub use drift::*;
pub use drift_attribution::*;
pub use drift_bayesian::*;
pub use drift_per_cell::*;
pub use drift_surprise_weighted::*;
pub use drill::*;
pub use emergency_eviction::*;
pub use errors::*;
pub use ewc::*;
pub use fisher::*;
pub use full_retrain::*;
pub use integrity::*;
pub use lambda_ramp::*;
pub use lora_refresh::*;
pub use per_cell_promotion::*;
pub use pipeline::*;
pub use plasticity::*;
pub use policy::*;
pub use promote::*;
pub use promote_approval::*;
pub use readback::*;
pub use regulate::*;
pub use scheduler::*;
pub use scheduler_state::*;
pub use store::*;

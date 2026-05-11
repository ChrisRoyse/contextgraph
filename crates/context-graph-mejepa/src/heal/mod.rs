//! ME-JEPA Phase 5 self-healing.
//!
//! The module is fail-closed: every persisted state transition has a RocksDB
//! readback or a file readback verifier, and every operator-visible failure has
//! a stable `MEJEPA_*` code.

pub mod calibration;
pub mod cf;
pub mod distill;
pub mod drift;
pub mod drill;
pub mod errors;
pub mod ewc;
pub mod fsv;
pub mod full_retrain;
pub mod integrity;
pub mod lora_refresh;
pub mod pipeline;
pub mod promote;
pub mod regulate;
pub mod scheduler;
pub mod scheduler_state;
pub mod store;

pub use calibration::*;
pub use cf::*;
pub use distill::*;
pub use drift::*;
pub use drill::*;
pub use errors::*;
pub use ewc::*;
pub use fsv::*;
pub use full_retrain::*;
pub use integrity::*;
pub use lora_refresh::*;
pub use pipeline::*;
pub use promote::*;
pub use regulate::*;
pub use scheduler::*;
pub use scheduler_state::*;
pub use store::*;

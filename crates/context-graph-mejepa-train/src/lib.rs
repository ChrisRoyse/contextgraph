#![deny(deprecated)]

//! ME-JEPA Phase 3 training crate.
//!
//! This crate owns the Phase 3 training surface: UTML learning-signal math,
//! sampler, optimizer wiring, loss assembly, certificate persistence, holdout
//! evaluation, checkpoints, CLI, and FSV examples. All persisted training state
//! is stored in RocksDB column families and every verification path performs a
//! separate read from RocksDB.

pub mod aux;
pub mod cert;
pub mod checkpoint;
pub mod cli;
pub mod config;
pub mod distill;
pub mod error;
pub mod eval;
pub mod learning_signal;
pub mod lora_wiring;
pub mod loss;
pub mod optim;
pub mod sampler;
pub mod trainer;

pub use cert::{TrainingCertificate, CF_MEJEPA_TRAIN_CERTS};
pub use config::{LossCoefficients, TrainingConfig};
pub use error::{TrainerError, TrainerErrorCode};
pub use learning_signal::{
    compute_l_step, DeltaKComponents, DeltaOmegaComponents, DeltaPComponents, DeltaXiComponents,
    LearningSignal, SkipReason, UtmlError, UtmlErrorCode,
};

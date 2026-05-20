#![deny(deprecated)]

//! ME-JEPA Phase 3 training crate.
//!
//! This crate owns the Phase 3 training surface: UTML learning-signal math,
//! sampler, optimizer wiring, loss assembly, certificate persistence, holdout
//! evaluation, checkpoints, CLI, and FSV examples. All persisted training state
//! is stored in RocksDB column families and every verification path performs a
//! separate read from RocksDB.

pub mod ability_resolver;
pub mod active_constellation;
pub mod aux;
pub mod cert;
pub mod checkpoint;
pub mod chunk_skill_membership;
pub mod cli;
pub mod compression_progress;
pub mod config;
pub mod dda;
pub mod distill;
pub mod error;
pub mod eval;
pub mod label_bridge;
pub mod labels;
pub mod learning_signal;
pub mod lora_wiring;
pub mod loss;
pub mod mistake_log;
pub mod online_head_state;
mod online_head_state_keys;
mod online_head_state_support;
pub mod optim;
pub mod replay_buffer;
pub mod sampler;
pub mod skill_corpus_materialization;
pub mod skill_linkage;
pub mod skill_sequence_discovery;
pub mod skill_sequence_types;
mod skill_validation;
pub mod trainer;

pub use ability_resolver::*;
pub use active_constellation::*;
pub use cert::{TrainingCertificate, CF_MEJEPA_TRAIN_CERTS};
pub use chunk_skill_membership::*;
pub use compression_progress::{
    compression_progress_report, compression_progress_report_from_path,
    conditional_description_length_bits_from_probability,
    render_compression_progress_weekly_section, CompressionProgressCertEntry,
    CompressionProgressError, CompressionProgressMonotonicity, CompressionProgressPoint,
    CompressionProgressReport, CompressionProgressState, DEFAULT_COMPRESSION_PROGRESS_EPSILON_BITS,
    DEFAULT_COMPRESSION_PROGRESS_WINDOW,
};
pub use config::{LossCoefficients, TrainingConfig, Q4_LOSS_COEFFICIENT_MAX};
pub use error::{TrainerError, TrainerErrorCode};
pub use learning_signal::{
    compute_l_step, compute_per_head_learning_signal, DeltaKComponents, DeltaOmegaComponents,
    DeltaPComponents, DeltaXiComponents, HeadSignalInput, LearningSignal, PerHeadLearningSignal,
    SkipReason, UtmlError, UtmlErrorCode,
};
pub use loss::inverse::{
    compose_bidirectional_l_full, inverse_map_loss, unit_gaussian_nll_bits,
    InverseMapLossBreakdown, InverseMapOutputs, InverseMapTargets,
};
pub use mistake_log::*;
pub use online_head_state::*;
pub use replay_buffer::*;
pub use skill_corpus_materialization::*;
pub use skill_linkage::*;
pub use skill_sequence_discovery::*;
pub use trainer::InverseMapForward;

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
// #622: `aux`, `distill`, `lora_wiring` modules deleted as dead code; see
// also #685 / #687. `CheckpointPayload.aux_heads` in `checkpoint.rs` is a
// different field and is kept.
pub mod cert;
pub mod checkpoint;
pub mod chunk_skill_membership;
pub mod cli;
pub mod compression_progress;
pub mod config;
pub mod dda;
pub mod error;
pub mod eval;
pub mod failure_mode_hierarchy;
pub mod label_bridge;
pub mod labels;
pub mod learning_signal;
pub mod live_skill_reverse_index;
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
pub use failure_mode_hierarchy::*;
pub use learning_signal::{
    compute_l_step, compute_per_head_learning_signal, DeltaKComponents, DeltaOmegaComponents,
    DeltaPComponents, DeltaXiComponents, HeadSignalInput, LearningSignal, PerHeadLearningSignal,
    SkipReason, UtmlError, UtmlErrorCode,
};
pub use live_skill_reverse_index::*;
pub use loss::entropy::{
    estimate_latent_entropy_nats, latent_entropy_loss, EntropyLambdaDecision,
    EntropyLambdaScheduler, LatentEntropyConfig, LatentEntropyEstimate, LatentEntropyLossReport,
    UTML_LATENT_ENTROPY_DEGENERATE,
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

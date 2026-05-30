//! ME-JEPA Phase 1b embedder ensemble validation and control surface.
//!
//! This crate is intentionally fail-closed. It does not synthesize vectors or
//! silently switch to CPU. Production loading requires:
//! - a schema-versioned `models_config.toml` with active ME-JEPA-Code content
//!   registrations; E5/E11 are retired and E15-E21 are optional learner-state
//!   domain-extension registrations until real assets are materialized,
//! - SHA-256 manifest pins for every pretrained weight file,
//! - runtime CUDA access for the configured GPU budget,
//! - E17 calibration evidence before `agent_state_score` is used.

pub mod cache;
mod cache_entry;
mod cache_io;
pub mod cache_limits;
pub mod calibration;
pub mod config;
pub mod diagnostics;
pub mod digest;
pub mod dynamic_registry;
pub mod embedder_id;
pub mod ensemble;
pub mod error;
pub mod forward;
pub mod routing;
pub mod types;
pub mod vram;

#[cfg(test)]
mod cache_tests;

pub use cache_limits::{EmbedderCacheConfig, EmbedderCacheTelemetry, PruneReport};
pub use calibration::{agent_state_score, verify_calibration_certificate, CalibrationCertificate};
pub use config::{EmbedderRegistration, ModelsConfig};
pub use diagnostics::{
    embedder_coherence, predictor_collapse_score, CoherenceReport, CollapseReport,
};
pub use digest::{
    digest_file_sha256, digest_manifest_for_embedder, verify_registration_digest, FileDigest,
};
pub use dynamic_registry::{
    dda_signal_count_for_chunks, upper_triangle_len, DynamicEmbedderKind,
    DynamicEmbedderProvenanceRecord, DynamicEmbedderRegistryRecord, RuntimeEmbedderId,
    RuntimeRoutingResult, RuntimeRoutingTable,
};
pub use embedder_id::{EmbedderId, EmbedderKind};
pub use ensemble::{
    verify_all_registration_digests, verify_content_registrations,
    verify_declared_registration_digests, verify_required_registration_digests, Ensemble,
    EnsembleStatus, LoadedEmbedder,
};
pub use error::{EmbedError, EmbedResult};
pub use forward::{
    embedder_model_id, AlgorithmicEmbedderForward, EmbedderForward, PretrainedEmbedderForward,
    SUPPORTED_ALGORITHMIC_FORWARD_EMBEDDERS, SUPPORTED_FORWARD_EMBEDDERS,
    SUPPORTED_PRETRAINED_FORWARD_EMBEDDERS,
};
pub use routing::{route_for_entity_type, routing_coverage};
pub use types::{
    EmbedderInput, EmbedderOutput, LoraAdapterRegistration, RoutingResult, VramBudgetReport,
};
pub use vram::{
    nvidia_smi_status, nvidia_smi_total_mb_from_command, parse_nvidia_smi_memory_total_mb,
    query_vram_budget, VramBudget,
};

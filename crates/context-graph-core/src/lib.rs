#![deny(deprecated)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::module_inception)]
#![allow(clippy::needless_range_loop)]

//! Context Graph Core Library
//!
//! Provides core domain types, traits, and stub implementations for the
//! 13-Embedder Context Graph system for semantic memory retrieval.
//!
//! # Architecture
//!
//! This crate defines:
//! - Domain types (`TeleologicalFingerprint`, `SemanticFingerprint`, `TopicProfile`, etc.)
//! - Core traits (`TeleologicalMemoryStore`, `MultiArrayEmbeddingProvider`, etc.)
//! - Error types and result aliases
//! - Configuration structures
//! - Teleological services (retrieval, fusion, comparison)
//!
//! # Example
//!
//! ```
//! use context_graph_core::traits::{TeleologicalMemoryStore, TeleologicalSearchOptions};
//!
//! // Create search options for querying
//! let options = TeleologicalSearchOptions::quick(10)
//!     .with_min_similarity(0.8);
//! assert_eq!(options.top_k, 10);
//! ```

pub mod causal;
pub mod clustering;
pub mod code;
pub mod config;
pub mod constellation;
pub mod contrastive;
pub mod dynamicjepa;
pub mod embeddings;
pub mod entity;
pub mod error;
pub mod fusion;
pub mod graph;
pub mod graph_linking;
pub mod index;
pub mod injection;
pub mod learner;
pub mod learner_training;
pub mod learning;
pub mod llm_edge_validation;
pub mod memory;
pub mod monitoring;
pub mod quantization;
pub mod retrieval;
pub mod similarity;
pub mod stubs;
pub mod teleological;
pub mod training;
pub mod traits;
pub mod typed_edge_export;
pub mod types;
pub mod weights;

// Re-exports for convenience
pub use config::Config;
// Legacy error types (retained for backwards compatibility)
pub use error::{CoreError, CoreResult};
// TASK-CORE-014: Unified error hierarchy re-exports
pub use error::{
    ConfigError, ContextGraphError, EmbeddingError, GpuError, IndexError, McpError, Result,
    StorageError,
};
pub use types::EdgeType;

// Production monitoring types (traits and error types only)
pub use monitoring::{
    HealthMetrics, LayerInfo, LayerStatus, LayerStatusProvider, MonitorResult, SystemMonitor,
    SystemMonitorError,
};

// AP-007: Stub monitors are TEST ONLY - not available in production builds
// Production code MUST provide real SystemMonitor and LayerStatusProvider implementations.
// (M-M1, 2026-05-19: `StubLayerStatusProvider` renamed to
// `HardcodedActiveLayerStatusProvider` so future agents are not deceived by
// the name; the type still hardcodes L1/L3 = Active.)
#[cfg(test)]
pub use monitoring::{HardcodedActiveLayerStatusProvider, StubSystemMonitor};

// Teleological module re-exports (cross-embedding synergy and fusion)
pub use teleological::{
    DomainAlignments, DomainType, Embedder, EmbedderDims, EmbedderGroup, EmbedderMask,
    GroupAlignments, GroupType, MultiResolutionHierarchy, ProfileId, ProfileMetrics, SynergyMatrix,
    TaskType, TeleologicalProfile, TeleologicalVector, TuckerCore,
};

// Purpose module re-exports (goal hierarchy types) - TASK-CORE-010

// Memory capture types (Phase 1) - TASK-P1-001, TASK-P1-002, TASK-P1-003
pub use memory::{
    ChunkMetadata, HookType, Memory, MemorySource, ResponseType, Session, SessionStatus, TextChunk,
    MAX_CONTENT_LENGTH,
};

// Clustering types (Phase 4) - TASK-P4-001, TASK-P4-002, TASK-P4-003, TASK-P4-004, TASK-P4-005
pub use clustering::{
    birch_defaults, hdbscan_defaults, BIRCHParams, Cluster, ClusterError, ClusterMembership,
    ClusterSelectionMethod, ClusteringFeature, HDBSCANClusterer, HDBSCANParams, Topic, TopicPhase,
    TopicProfile, TopicStability,
};

// Injection pipeline types (Phase 5) - TASK-P5-001, TASK-P5-002, TASK-P5-003, TASK-P5-003b
pub use injection::{
    InjectionCandidate, InjectionCategory, InjectionResult, TemporalBadge, TemporalBadgeType,
    TemporalEnrichmentProvider, TokenBudget, BRIEF_BUDGET, DEFAULT_TOKEN_BUDGET,
};

// Code query detection (ARCH-16) - query-type-aware E7 similarity
pub use code::{compute_e7_similarity_with_query_type, detect_code_query_type, CodeQueryType};

// Fusion strategies (ARCH-18) - Weighted RRF for multi-embedder fusion
pub use fusion::{
    fuse_rankings, normalize_minmax, score_weighted_rrf, weighted_rrf, weighted_sum,
    EmbedderRanking, FusedResult, FusionStrategy, RRF_K,
};

// Graph asymmetric similarity (E8) - directional graph embeddings
pub use graph::{
    adjust_batch_graph_similarities, compute_e8_asymmetric_fingerprint_similarity,
    compute_e8_asymmetric_full, compute_graph_asymmetric_similarity,
    compute_graph_asymmetric_similarity_simple, detect_graph_query_intent, ConnectivityContext,
    GraphDirection,
};

// Graph linking types - K-NN graph construction and multi-relation edges
// ARCH-18: E5/E8 asymmetric similarity, AP-77: fail fast on symmetric cosine violation
pub use graph_linking::{
    DirectedRelation, EdgeError, EdgeResult, EdgeStorageKey, EdgeThresholds, EmbedderEdge,
    GraphLinkEdgeType, KnnGraph, TypedEdge, TypedEdgeStorageKey, DEFAULT_THRESHOLDS, KNN_K,
    MIN_KNN_SIMILARITY, NN_DESCENT_ITERATIONS, NN_DESCENT_SAMPLE_RATE,
};

// Weight profiles - for multi-embedder search weight configuration
pub use weights::{
    get_profile_names, get_weight_profile, space_name, validate_weights, WeightProfileError,
    WEIGHT_PROFILES,
};

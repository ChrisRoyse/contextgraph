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

pub mod atc;
pub mod causal;
pub mod clustering;
pub mod code;
pub mod config;
pub mod embeddings;
pub mod entity;
pub mod error;
pub mod fusion;
pub mod graph;
pub mod injection;
pub mod index;
pub mod marblestone;
pub mod memory;
pub mod monitoring;
pub mod quantization;
pub mod retrieval;
pub mod session;
pub mod similarity;
pub mod steering;
pub mod stubs;
pub mod teleological;
pub mod traits;
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
pub use marblestone::{Domain, EdgeType, NeurotransmitterWeights};

// Production monitoring types (traits and error types only)
pub use monitoring::{
    HealthMetrics, LayerInfo, LayerStatus, LayerStatusProvider, MonitorResult, SystemMonitor,
    SystemMonitorError,
};

// AP-007: Stub monitors are TEST ONLY - not available in production builds
// Production code MUST provide real SystemMonitor and LayerStatusProvider implementations
#[cfg(test)]
pub use monitoring::{StubLayerStatusProvider, StubSystemMonitor};

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
    birch_defaults, BIRCHParams, Cluster, ClusterError, ClusteringFeature, ClusterMembership,
    ClusterSelectionMethod, HDBSCANClusterer, HDBSCANParams, Topic, TopicPhase, TopicProfile,
    TopicStability, hdbscan_defaults,
};

// Injection pipeline types (Phase 5) - TASK-P5-001, TASK-P5-002, TASK-P5-003, TASK-P5-003b
pub use injection::{
    InjectionCandidate, InjectionCategory, InjectionResult, TokenBudget,
    DEFAULT_TOKEN_BUDGET, BRIEF_BUDGET,
    TemporalBadge, TemporalBadgeType, TemporalEnrichmentProvider,
};

// Code query detection (ARCH-16) - query-type-aware E7 similarity
pub use code::{
    CodeQueryType, detect_code_query_type, compute_e7_similarity_with_query_type,
    e7_weight_adjustment,
};

// Fusion strategies (ARCH-18) - Weighted RRF for multi-embedder fusion
pub use fusion::{
    FusionStrategy, EmbedderRanking, FusedResult, RRF_K,
    fuse_rankings, weighted_rrf, weighted_sum, normalize_minmax,
};

// Graph asymmetric similarity (E8) - directional graph embeddings
pub use graph::{
    GraphDirection, ConnectivityContext, compute_graph_asymmetric_similarity,
    compute_graph_asymmetric_similarity_simple, compute_e8_asymmetric_fingerprint_similarity,
    compute_e8_asymmetric_full, detect_graph_query_intent, adjust_batch_graph_similarities,
};

// Session intent tracking (Phase 5) - E10 intent drift detection
pub use session::{
    IntentDrift, IntentSnapshot, IntentTracker, IntentTrajectoryStats,
    DEFAULT_DRIFT_THRESHOLD, DEFAULT_WINDOW_SIZE,
};

// Weight profiles - for multi-embedder search weight configuration
pub use weights::{
    get_weight_profile, get_profile_names, validate_weights, space_name,
    WeightProfileError, WEIGHT_PROFILES,
};

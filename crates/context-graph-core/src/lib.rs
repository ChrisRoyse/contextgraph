#![deny(deprecated)]

//! Context Graph Core Library
//!
//! Provides core domain types, traits, and stub implementations for the
//! Ultimate Context Graph system implementing UTL (Unified Theory of Learning).
//!
//! # Architecture
//!
//! This crate defines:
//! - Domain types (`TeleologicalFingerprint`, `SemanticFingerprint`, `PurposeVector`, etc.)
//! - Core traits (`TeleologicalMemoryStore`, `MultiArrayEmbeddingProvider`, `NervousLayer`, etc.)
//! - Error types and result aliases
//! - Configuration structures
//! - Goal alignment computation (`GoalAlignmentCalculator`, `GoalAlignmentScore`, etc.)
//! - Johari transition management (`JohariTransitionManager`, `DefaultJohariManager`, etc.)
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

pub mod alignment;
pub mod atc;
pub mod autonomous;
pub mod causal;
pub mod config;
pub mod dream;
pub mod error;
pub mod gwt;
pub mod index;
pub mod johari;
pub mod layers;
pub mod marblestone;
pub mod monitoring;
pub mod neuromod;
pub mod purpose;
pub mod retrieval;
pub mod similarity;
pub mod steering;
pub mod stubs;
pub mod teleological;
pub mod traits;
pub mod types;

// Re-exports for convenience
pub use config::Config;
pub use error::{CoreError, CoreResult};
pub use marblestone::{Domain, EdgeType, NeurotransmitterWeights};

// Production monitoring types (traits and error types only)
pub use monitoring::{
    HealthMetrics, LayerInfo, LayerStatus, LayerStatusProvider, MonitorResult,
    SystemMonitor, SystemMonitorError,
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
pub use purpose::{GoalLevel, GoalNode};

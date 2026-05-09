//! Graph relationship utilities for ContextGraph.
//!
//! The local LLM graph-discovery backend was removed in the 2026-05-09
//! ME-JEPA pivot. This crate now contains deterministic graph candidate
//! scanning, graph relationship data types, and E8 activation helpers.
//!
//! # E8 Dimension Change
//!
//! E8 has been upgraded from MiniLM (384D) to e5-large-v2 (1024D):
//! - Shares the model with E1, no extra VRAM
//! - Better semantic understanding for graph relationships
//! - Asymmetric source/target embeddings via learned projections
//!
//! # Content Domains
//!
//! The scanner supports 4 content domains:
//! - **Code**: Programming code, APIs, software documentation
//! - **Legal**: Cases, statutes, contracts, regulations
//! - **Academic**: Research papers, studies, citations
//! - **General**: Other content
//!
//! # Relationship Types
//!
//! The type system tracks 20 relationship types (19 + None) across 6 categories:
//!
//! **Containment**: contains, scoped_by
//! **Dependency**: depends_on, imports, requires
//! **Reference**: references, cites, interprets, distinguishes
//! **Implementation**: implements, complies_with, fulfills
//! **Extension**: extends, modifies, supersedes, overrules
//! **Invocation**: calls, applies, used_by
//!
//! # Pipeline Flow
//!
//! ```text
//! Memories -> Scanner -> Candidates -> Activator -> Graph Edges
//!               |            |              |
//!           Heuristics   E1 Similarity   E8 1024D
//! ```

#[cfg(feature = "llm")]
compile_error!(
    "The local LLM graph backend was retired on 2026-05-09. Use ME-JEPA evidence capture instead."
);

pub mod activator;
pub mod error;
pub mod scanner;
pub mod types;

// Re-exports for convenience
pub use activator::{ActivationStats, ActivatorConfig, E8Activator, GraphEdge, GraphStorage};
pub use error::{GraphAgentError, GraphAgentResult};
pub use scanner::{MemoryScanner, ScannerConfig};
// Shared types from core (no LLM required).
pub use context_graph_core::types::{DiscoveryCycleResult, ServiceStatus};
pub use types::{
    ContentDomain, DomainMarkers, GraphAnalysisResult, GraphCandidate, GraphLinkDirection,
    GraphMarkers, MemoryForGraphAnalysis, RelationshipCategory, RelationshipType,
};

/// E8 embedding dimension (upgraded from 384 to 1024).
///
/// Uses e5-large-v2 instead of MiniLM for better graph understanding.
/// Shares the model with E1 for zero additional VRAM.
pub const E8_DIMENSION: usize = 1024;

//! Graph relationship discovery agent using local LLM.
//!
//! This crate provides LLM-based graph relationship discovery for the
//! context-graph system. It mirrors the architecture of `context-graph-causal-agent`
//! but focuses on structural relationships (imports, dependencies, references, etc.)
//! rather than causal relationships.
//!
//! # Architecture
//!
//! The graph agent shares the Hermes-2-Pro-Mistral-7B LLM with the causal agent via `Arc`,
//! avoiding duplicate VRAM usage. It uses the same ChatML prompt format with
//! different system prompts for graph relationship detection.
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
//! The agent supports 4 content domains:
//! - **Code**: Programming code, APIs, software documentation
//! - **Legal**: Cases, statutes, contracts, regulations
//! - **Academic**: Research papers, studies, citations
//! - **General**: Other content
//!
//! # Relationship Types
//!
//! The agent detects 20 relationship types (19 + None) across 6 categories:
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
//! Memories -> Scanner -> Candidates -> LLM Analysis -> Activator -> Graph Edges
//!               |            |              |              |
//!           Heuristics   E1 Similarity  Hermes-2-Pro   E8 1024D
//! ```
//!
//! # Usage
//!
//! ```rust,no_run
//! use std::sync::Arc;
//! use context_graph_graph_agent::{GraphDiscoveryService, GraphDiscoveryConfig};
//! use context_graph_causal_agent::CausalDiscoveryLLM;
//!
//! async fn example() {
//!     // Create shared LLM (from causal-agent)
//!     let llm = Arc::new(CausalDiscoveryLLM::new().unwrap());
//!     llm.load().await.unwrap();
//!
//!     // Create graph discovery service
//!     let service = GraphDiscoveryService::new(llm);
//!
//!     // Run discovery cycle
//!     // let result = service.run_discovery_cycle(&memories).await?;
//! }
//! ```

pub mod activator;
pub mod error;
#[cfg(feature = "llm")]
pub mod llm;
pub mod scanner;
#[cfg(feature = "llm")]
pub mod service;
#[cfg(feature = "test-utils")]
pub mod stubs;
pub mod types;

// Re-exports for convenience
pub use activator::{ActivatorConfig, ActivationStats, E8Activator, GraphEdge, GraphStorage};
pub use error::{GraphAgentError, GraphAgentResult};
#[cfg(feature = "llm")]
pub use llm::{prompt::GraphPromptBuilder, GraphRelationshipLLM};
pub use scanner::{MemoryScanner, ScannerConfig};
// Shared types from core (no LLM required).
pub use context_graph_core::types::{DiscoveryCycleResult, ServiceStatus};
#[cfg(feature = "llm")]
pub use service::{GraphDiscoveryConfig, GraphDiscoveryService};
pub use types::{
    ContentDomain, DomainMarkers, GraphAnalysisResult, GraphCandidate, GraphLinkDirection,
    GraphMarkers, MemoryForGraphAnalysis, RelationshipCategory, RelationshipType,
};

// Test utilities (feature-gated)
#[cfg(feature = "test-utils")]
pub use stubs::create_stub_graph_discovery_service;

/// E8 embedding dimension (upgraded from 384 to 1024).
///
/// Uses e5-large-v2 instead of MiniLM for better graph understanding.
/// Shares the model with E1 for zero additional VRAM.
pub const E8_DIMENSION: usize = 1024;

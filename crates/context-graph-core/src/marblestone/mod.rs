//! Marblestone architecture integration for context-aware neurotransmitter weighting.
//!
//! This module provides domain classification for the Marblestone edge model,
//! enabling context-specific retrieval behavior in the knowledge graph.
//!
//! # Constitution Reference
//! - edge_model.nt_weights.domain: Code|Legal|Medical|Creative|Research|General
//! - Formula: w_eff = base × (1 + excitatory - inhibitory + 0.5×modulatory)
//!
//! # Module Structure
//! - `domain` - Knowledge domain classification
//! - `neurotransmitter_weights` - NT weight modulation for edges
//! - `edge_type` - Graph edge relationship types
//!
//! # Example
//! ```rust
//! use context_graph_core::marblestone::{Domain, NeurotransmitterWeights, EdgeType};
//!
//! // Create domain-specific weights
//! let weights = NeurotransmitterWeights::for_domain(Domain::Code);
//! let effective = weights.compute_effective_weight(0.8);
//!
//! // Work with edge types
//! let edge = EdgeType::Causal;
//! let base_weight = edge.default_weight();
//! ```

mod contradiction_type;
mod domain;
mod edge_type;
mod neurotransmitter_weights;

#[cfg(test)]
mod edge_case_verification;
#[cfg(test)]
mod tests_domain;
#[cfg(test)]
mod tests_edge_type;
#[cfg(test)]
mod tests_neurotransmitter_weights;

// Re-export all public types
pub use self::contradiction_type::ContradictionType;
pub use self::domain::Domain;
pub use self::edge_type::EdgeType;
pub use self::neurotransmitter_weights::NeurotransmitterWeights;

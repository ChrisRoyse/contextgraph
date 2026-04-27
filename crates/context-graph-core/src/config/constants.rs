//! Centralized constants from constitution.yaml.
//!
//! This module extracts magic numbers into named constants with documentation
//! citing their source in the constitution. All thresholds and constants that
//! were previously hardcoded are now centralized here for:
//!
//! 1. Single source of truth
//! 2. Clear constitution traceability
//! 3. Easy configuration updates
//! 4. Test consistency
//!
//! # Constitution Reference
//!
//! All values here come from `/docs2/constitution.yaml` sections:
//! - `embeddings.similarity` - RRF and similarity constants
//! - `forbidden.AP-003` - "Magic numbers -> define constants"

/// Similarity computation constants from constitution.yaml embeddings.similarity.
///
/// ```yaml
/// embeddings:
///   similarity:
///     method: "Reciprocal Rank Fusion (RRF) across per-space results"
///     formula: "RRF(d) = Σᵢ 1/(k + rankᵢ(d)) where k=60"
///     rrf_constant: 60
/// ```
pub mod similarity {
    /// RRF (Reciprocal Rank Fusion) constant k.
    ///
    /// Constitution: `embeddings.similarity.rrf_constant`
    /// Used in formula: RRF(d) = Σᵢ 1/(k + rankᵢ(d))
    ///
    /// The k=60 value is standard in literature and provides good balance
    /// between giving credit to top ranks while not over-penalizing lower ranks.
    pub const RRF_K: f32 = 60.0;
}

/// Pipeline configuration defaults from constitution.yaml embeddings.retrieval_pipeline.
pub mod pipeline {
    use super::similarity;

    /// Default RRF k constant for aggregation.
    ///
    /// Constitution: `embeddings.similarity.rrf_constant`
    pub const DEFAULT_RRF_K: f32 = similarity::RRF_K;
}

#[cfg(test)]
#[allow(clippy::assertions_on_constants)]
mod tests {
    use super::*;

    #[test]
    fn test_rrf_k_matches_constitution() {
        // RRF k=60 per constitution.yaml embeddings.similarity.rrf_constant
        assert!(
            (similarity::RRF_K - 60.0).abs() < f32::EPSILON,
            "RRF_K must be 60.0 per constitution"
        );
    }

    #[test]
    fn test_pipeline_defaults_use_constants() {
        // Pipeline defaults should reference the canonical constants
        assert_eq!(
            pipeline::DEFAULT_RRF_K,
            similarity::RRF_K,
            "Pipeline RRF_K should use similarity::RRF_K"
        );
    }
}

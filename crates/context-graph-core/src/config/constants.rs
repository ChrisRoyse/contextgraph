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
//! - `teleological.thresholds` - Alignment thresholds
//! - `embeddings.similarity` - RRF and similarity constants
//! - `utl.johari` - Johari quadrant boundaries
//! - `forbidden.AP-003` - "Magic numbers -> define constants"

/// Teleological alignment thresholds from constitution.yaml teleological.thresholds.
///
/// These define the quality levels for goal alignment (θ = alignment score).
///
/// ```yaml
/// teleological:
///   thresholds:
///     optimal: "θ ≥ 0.75"
///     acceptable: "θ ∈ [0.70, 0.75)"
///     warning: "θ ∈ [0.55, 0.70)"
///     critical: "θ < 0.55"
/// ```
pub mod alignment {
    /// Optimal alignment threshold: θ ≥ 0.75
    ///
    /// Constitution: `teleological.thresholds.optimal`
    /// Meaning: Excellent alignment with North Star goal
    pub const OPTIMAL: f32 = 0.75;

    /// Acceptable alignment threshold: θ ∈ [0.70, 0.75)
    ///
    /// Constitution: `teleological.thresholds.acceptable`
    /// Meaning: Good alignment, no action needed
    pub const ACCEPTABLE: f32 = 0.70;

    /// Warning alignment threshold: θ ∈ [0.55, 0.70)
    ///
    /// Constitution: `teleological.thresholds.warning`
    /// Meaning: Needs improvement, monitor closely
    pub const WARNING: f32 = 0.55;

    /// Critical alignment threshold: θ < 0.55
    ///
    /// Constitution: `teleological.thresholds.critical`
    /// Meaning: Critical misalignment, requires intervention
    pub const CRITICAL: f32 = 0.55;

    /// Minimum alignment threshold for pipeline stage 4 (teleological filter).
    ///
    /// Constitution: `embeddings.retrieval_pipeline.stage_4_teleological_filter.method`
    /// Memories below this threshold are discarded in the retrieval pipeline.
    pub const MIN_PIPELINE_THRESHOLD: f32 = CRITICAL;

    /// Failure prediction threshold.
    ///
    /// Constitution: `teleological.thresholds.failure_prediction`
    /// When alignment drops by this amount, failure is predicted 30-60s ahead.
    pub const FAILURE_PREDICTION_DELTA: f32 = -0.15;
}

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

/// Johari quadrant thresholds from constitution.yaml utl.johari.
///
/// These define the entropy (ΔS) and coherence (ΔC) boundaries for
/// classifying memories into Johari quadrants.
///
/// ```yaml
/// utl:
///   johari:
///     Open: "ΔSᵢ<0.5, ΔCᵢ>0.5 → aware in this space"
///     Blind: "ΔSᵢ>0.5, ΔCᵢ<0.5 → discovery opportunity in this space"
///     Hidden: "ΔSᵢ<0.5, ΔCᵢ<0.5 → latent in this space"
///     Unknown: "ΔSᵢ>0.5, ΔCᵢ>0.5 → frontier in this space"
/// ```
pub mod johari {
    /// Johari quadrant boundary threshold.
    ///
    /// Constitution: `utl.johari` (all quadrant definitions use 0.5)
    /// This is the midpoint that separates high/low for both ΔS and ΔC.
    pub const BOUNDARY: f32 = 0.5;

    /// Default blind spot detection threshold.
    ///
    /// Constitution: `utl.johari.Blind` - when external signals exceed this,
    /// a memory may be in a blind spot (known by others, not by self).
    pub const BLIND_SPOT_THRESHOLD: f32 = 0.5;
}

/// Pipeline configuration defaults from constitution.yaml embeddings.retrieval_pipeline.
///
/// ```yaml
/// embeddings:
///   retrieval_pipeline:
///     stage_4_teleological_filter:
///       method: "Filter: alignment < 0.55 → discard"
/// ```
pub mod pipeline {
    use super::alignment;
    use super::similarity;

    /// Default RRF k constant for aggregation.
    ///
    /// Constitution: `embeddings.similarity.rrf_constant`
    pub const DEFAULT_RRF_K: f32 = similarity::RRF_K;

    /// Default minimum alignment threshold for pipeline filtering.
    ///
    /// Constitution: `teleological.thresholds.critical`
    /// This is the stage 4 filter threshold.
    pub const DEFAULT_MIN_ALIGNMENT: f32 = alignment::CRITICAL;
}

/// Goal alignment estimation factors.
///
/// These are placeholder values used when actual alignment scores are unavailable.
/// They should be replaced with proper computation in production.
pub mod estimation {
    /// Content similarity to goal alignment estimation factor.
    ///
    /// Used as a placeholder when purpose alignment is unavailable.
    /// Formula: goal_alignment = content_similarity * CONTENT_TO_GOAL_FACTOR
    ///
    /// NOTE: This is a placeholder per `retrieval/pipeline.rs:578`.
    /// In production, this should be replaced with proper teleological computation.
    pub const CONTENT_TO_GOAL_FACTOR: f32 = 0.9;
}

#[cfg(test)]
#[allow(clippy::assertions_on_constants)]
mod tests {
    use super::*;

    #[test]
    fn test_alignment_thresholds_ordered() {
        // Thresholds must be ordered: OPTIMAL > ACCEPTABLE > WARNING >= CRITICAL
        assert!(
            alignment::OPTIMAL > alignment::ACCEPTABLE,
            "OPTIMAL must be > ACCEPTABLE"
        );
        assert!(
            alignment::ACCEPTABLE > alignment::WARNING,
            "ACCEPTABLE must be > WARNING"
        );
        assert!(
            alignment::WARNING >= alignment::CRITICAL,
            "WARNING must be >= CRITICAL"
        );
    }

    #[test]
    fn test_alignment_threshold_values_match_constitution() {
        // These values MUST match constitution.yaml teleological.thresholds
        assert!(
            (alignment::OPTIMAL - 0.75).abs() < f32::EPSILON,
            "OPTIMAL must be 0.75 per constitution"
        );
        assert!(
            (alignment::ACCEPTABLE - 0.70).abs() < f32::EPSILON,
            "ACCEPTABLE must be 0.70 per constitution"
        );
        assert!(
            (alignment::WARNING - 0.55).abs() < f32::EPSILON,
            "WARNING must be 0.55 per constitution"
        );
        assert!(
            (alignment::CRITICAL - 0.55).abs() < f32::EPSILON,
            "CRITICAL must be 0.55 per constitution"
        );
    }

    #[test]
    fn test_rrf_k_matches_constitution() {
        // RRF k=60 per constitution.yaml embeddings.similarity.rrf_constant
        assert!(
            (similarity::RRF_K - 60.0).abs() < f32::EPSILON,
            "RRF_K must be 60.0 per constitution"
        );
    }

    #[test]
    fn test_johari_boundary_matches_constitution() {
        // Johari boundary is 0.5 per constitution.yaml utl.johari
        assert!(
            (johari::BOUNDARY - 0.5).abs() < f32::EPSILON,
            "BOUNDARY must be 0.5 per constitution"
        );
        assert!(
            (johari::BLIND_SPOT_THRESHOLD - 0.5).abs() < f32::EPSILON,
            "BLIND_SPOT_THRESHOLD must be 0.5 per constitution"
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
        assert_eq!(
            pipeline::DEFAULT_MIN_ALIGNMENT,
            alignment::CRITICAL,
            "Pipeline min alignment should use alignment::CRITICAL"
        );
    }

    #[test]
    fn test_estimation_factor_valid() {
        // Estimation factor should be in (0, 1]
        assert!(
            estimation::CONTENT_TO_GOAL_FACTOR > 0.0,
            "CONTENT_TO_GOAL_FACTOR must be > 0"
        );
        assert!(
            estimation::CONTENT_TO_GOAL_FACTOR <= 1.0,
            "CONTENT_TO_GOAL_FACTOR must be <= 1"
        );
    }
}

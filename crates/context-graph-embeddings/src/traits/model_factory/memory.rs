//! Memory estimation constants and functions.
//!
//! Provides conservative overestimates for model memory requirements.

use crate::types::ModelId;

/// Memory estimates for each model (in bytes, FP32).
/// These are conservative overestimates.
pub const MEMORY_ESTIMATES: [(ModelId, usize); 14] = [
    (ModelId::Semantic, 1_400_000_000),        // 1.3 GB + buffer (e5-large-v2, 335M params)
    (ModelId::TemporalRecent, 15_000_000),     // 10 MB + buffer
    (ModelId::TemporalPeriodic, 15_000_000),   // 10 MB + buffer
    (ModelId::TemporalPositional, 15_000_000), // 10 MB + buffer
    (ModelId::Causal, 650_000_000),            // 600 MB + buffer (nomic-embed-v1.5)
    (ModelId::Sparse, 550_000_000),            // 500 MB + buffer
    (ModelId::Code, 550_000_000),              // 500 MB + buffer
    (ModelId::Graph, 1_400_000_000),           // 1.3 GB + buffer (loads e5-large-v2, 335M params)
    (ModelId::Hdc, 60_000_000),                // 50 MB + buffer
    (ModelId::Contextual, 500_000_000),        // ~440 MB + buffer (e5-base-v2, 110M params)
    (ModelId::Entity, 120_000_000),            // 100 MB + buffer (legacy MiniLM 384D)
    (ModelId::LateInteraction, 450_000_000),   // 400 MB + buffer
    (ModelId::Splade, 550_000_000),            // 500 MB + buffer (similar to E6 Sparse)
    (ModelId::Kepler, 350_000_000),            // ~300 MB + buffer (KEPLER RoBERTa-base 768D)
];

/// Get memory estimate for a ModelId.
///
/// Returns a conservative 500MB default if the model is not in the estimates table,
/// with an error log to alert operators. This prevents silent OOM from returning 0.
pub fn get_memory_estimate(model_id: ModelId) -> usize {
    MEMORY_ESTIMATES
        .iter()
        .find(|(id, _)| *id == model_id)
        .map(|(_, mem)| *mem)
        .unwrap_or_else(|| {
            tracing::error!(
                "E_EMB_MEM_001: No memory estimate for {:?} — returning conservative 500MB default. \
                 Add an entry to MEMORY_ESTIMATES for this model.",
                model_id
            );
            500_000_000 // Conservative default instead of silent 0
        })
}

/// Total memory for all 14 models (FP32).
/// ~6.6 GB without quantization.
pub const TOTAL_MEMORY_ESTIMATE: usize = 6_625_000_000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_estimates_array_has_14_entries() {
        assert_eq!(MEMORY_ESTIMATES.len(), 14);
    }

    #[test]
    fn test_memory_estimates_all_nonzero() {
        for (model_id, memory) in MEMORY_ESTIMATES {
            assert!(memory > 0, "Memory for {:?} should be > 0", model_id);
        }
    }

    #[test]
    fn test_get_memory_estimate_finds_all_models() {
        for model_id in ModelId::all() {
            let estimate = get_memory_estimate(*model_id);
            assert!(
                estimate > 0,
                "get_memory_estimate({:?}) should return > 0",
                model_id
            );
        }
    }

    #[test]
    fn test_total_memory_estimate_matches_sum() {
        let sum: usize = MEMORY_ESTIMATES.iter().map(|(_, m)| m).sum();
        assert_eq!(TOTAL_MEMORY_ESTIMATE, sum);
    }

    #[test]
    fn test_memory_estimate_largest_model() {
        // Semantic (e5-large-v2) and Graph (also e5-large-v2) should be largest at 1.4GB
        let semantic = get_memory_estimate(ModelId::Semantic);
        let graph = get_memory_estimate(ModelId::Graph);
        assert_eq!(semantic, graph, "Semantic and Graph both load e5-large-v2, should match");
        for model_id in ModelId::all() {
            let other = get_memory_estimate(*model_id);
            assert!(
                semantic >= other,
                "Semantic ({}) should be >= {:?} ({})",
                semantic,
                model_id,
                other
            );
        }
    }

    #[test]
    fn test_memory_estimate_smallest_models() {
        // Temporal models (15MB each) are the smallest
        let temporal_recent = get_memory_estimate(ModelId::TemporalRecent);
        let temporal_periodic = get_memory_estimate(ModelId::TemporalPeriodic);
        let temporal_positional = get_memory_estimate(ModelId::TemporalPositional);

        // All three temporal models have the same (smallest) size
        assert_eq!(temporal_recent, temporal_periodic);
        assert_eq!(temporal_periodic, temporal_positional);
        assert_eq!(temporal_recent, 15_000_000);

        // Verify they are smaller than or equal to all others
        for model_id in ModelId::all() {
            let other = get_memory_estimate(*model_id);
            assert!(
                temporal_recent <= other,
                "Temporal models ({}) should be <= {:?} ({})",
                temporal_recent,
                model_id,
                other
            );
        }
    }
}

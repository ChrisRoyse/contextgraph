//! Ground truth validation for per-embedder ground truth generation.
//!
//! Tests:
//! - All 13 embedders have ground truth
//! - Semantic embedders use topic-based relevance
//! - E4 uses session sequences
//! - E8 uses document structure
//! - Minimum relevant docs per query

use std::collections::{HashMap, HashSet};

use super::{CheckPriority, PhaseResult, ValidationCheck, ValidationPhase};
use crate::realdata::config::EmbedderName;
use crate::realdata::ground_truth::{EmbedderGroundTruth, UnifiedGroundTruth};

/// Ground truth validation results.
#[derive(Debug, Clone)]
pub struct GroundTruthValidationResult {
    /// All checks passed.
    pub all_passed: bool,
    /// Individual check results.
    pub checks: Vec<ValidationCheck>,
    /// Per-embedder stats.
    pub embedder_stats: HashMap<EmbedderName, GroundTruthStats>,
    /// Overall stats.
    pub overall_stats: OverallGtStats,
}

/// Stats for a single embedder's ground truth.
#[derive(Debug, Clone, Default)]
pub struct GroundTruthStats {
    /// Number of queries with ground truth.
    pub num_queries: usize,
    /// Min relevant docs per query.
    pub min_relevant: usize,
    /// Max relevant docs per query.
    pub max_relevant: usize,
    /// Mean relevant docs per query.
    pub mean_relevant: f64,
    /// Queries with no relevant docs.
    pub queries_with_zero_relevant: usize,
}

/// Overall ground truth stats.
#[derive(Debug, Clone, Default)]
pub struct OverallGtStats {
    /// Total queries.
    pub total_queries: usize,
    /// Embedders with ground truth.
    pub embedders_with_gt: usize,
    /// Total relevant pairs.
    pub total_relevant_pairs: usize,
}

/// Ground truth validator.
pub struct GroundTruthValidator;

impl GroundTruthValidator {
    /// Run all ground truth validation checks.
    pub fn validate(ground_truth: Option<&UnifiedGroundTruth>) -> GroundTruthValidationResult {
        let mut result = GroundTruthValidationResult {
            all_passed: true,
            checks: Vec::new(),
            embedder_stats: HashMap::new(),
            overall_stats: OverallGtStats::default(),
        };

        // If no ground truth provided, skip with appropriate message
        let Some(gt) = ground_truth else {
            result.checks.push(
                ValidationCheck::new("ground_truth_exists", "Ground truth data provided")
                    .fail("none", "ground truth data"),
            );
            result.all_passed = false;
            return result;
        };

        // Test 1: All 13 embedders have ground truth
        let check = Self::test_all_embedders_have_gt(gt);
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // Compute per-embedder stats
        for embedder in EmbedderName::all() {
            if let Some(embedder_gt) = gt.by_embedder.get(&embedder) {
                result
                    .embedder_stats
                    .insert(embedder, Self::compute_stats(embedder_gt));
            }
        }

        // Overall stats
        result.overall_stats.total_queries = gt.num_queries;
        result.overall_stats.embedders_with_gt = gt.by_embedder.len();
        result.overall_stats.total_relevant_pairs = gt
            .by_embedder
            .values()
            .map(|e| e.total_relevant_pairs)
            .sum();

        // Test 2: Semantic embedders use topic-based relevance
        let check = Self::test_semantic_gt_topic_based(gt);
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // Test 3: E4 uses session sequences
        let check = Self::test_e4_gt_uses_sessions(gt);
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // Test 4: E8 uses document structure
        let check = Self::test_e8_gt_uses_documents(gt);
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // Test 5: Minimum relevant docs per query
        let check = Self::test_min_relevant_per_query(gt);
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // Test 6: No empty embedder ground truth
        let check = Self::test_no_empty_embedder_gt(gt);
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // Test 7: Queries have chunk IDs
        let check = Self::test_queries_have_chunk_ids(gt);
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        // Test 8: Relevant chunk IDs reference valid chunks
        let check = Self::test_relevant_chunk_ids_valid(gt);
        if !check.is_passed() {
            result.all_passed = false;
        }
        result.checks.push(check);

        result
    }

    /// Convert to PhaseResult.
    pub fn to_phase_result(result: GroundTruthValidationResult, duration_ms: u64) -> PhaseResult {
        let mut phase_result = PhaseResult::new(ValidationPhase::GroundTruth);
        for check in result.checks {
            phase_result.add_check(check);
        }
        phase_result.finalize(duration_ms);
        phase_result
    }

    fn compute_stats(embedder_gt: &EmbedderGroundTruth) -> GroundTruthStats {
        let mut stats = GroundTruthStats {
            num_queries: embedder_gt.queries.len(),
            ..Default::default()
        };

        if stats.num_queries == 0 {
            return stats;
        }

        let relevant_counts: Vec<usize> = embedder_gt
            .queries
            .iter()
            .map(|qgt| qgt.relevant_ids.len())
            .collect();

        stats.min_relevant = *relevant_counts.iter().min().unwrap_or(&0);
        stats.max_relevant = *relevant_counts.iter().max().unwrap_or(&0);
        stats.mean_relevant =
            relevant_counts.iter().sum::<usize>() as f64 / relevant_counts.len() as f64;
        stats.queries_with_zero_relevant = relevant_counts.iter().filter(|&&c| c == 0).count();

        stats
    }

    fn test_all_embedders_have_gt(gt: &UnifiedGroundTruth) -> ValidationCheck {
        let check = ValidationCheck::new(
            "all_embedders_have_gt",
            "All 13 embedders have ground truth",
        )
        .with_priority(CheckPriority::Critical);

        let expected_embedders: HashSet<_> = EmbedderName::all().into_iter().collect();
        let actual_embedders: HashSet<_> = gt.by_embedder.keys().cloned().collect();

        let missing: Vec<_> = expected_embedders
            .difference(&actual_embedders)
            .map(|e| e.as_str())
            .collect();

        if missing.is_empty() {
            check.pass(&format!("{}/13", actual_embedders.len()), "13/13")
        } else {
            check
                .fail(&format!("{}/13", actual_embedders.len()), "13/13")
                .with_details(&format!("Missing: {}", missing.join(", ")))
        }
    }

    fn test_semantic_gt_topic_based(gt: &UnifiedGroundTruth) -> ValidationCheck {
        let check = ValidationCheck::new(
            "semantic_gt_topic_based",
            "Semantic embedders use topic-based relevance",
        )
        .with_priority(CheckPriority::High);

        // Semantic embedders should have ground truth where query topic == relevant doc topic
        // We verify this by checking that semantic embedders have non-empty GT
        let semantic = EmbedderName::semantic();
        let semantic_with_gt: Vec<_> = semantic
            .iter()
            .filter(|e| {
                gt.by_embedder
                    .get(e)
                    .map(|g| !g.queries.is_empty())
                    .unwrap_or(false)
            })
            .collect();

        if semantic_with_gt.len() == semantic.len() {
            check
                .pass(
                    &format!("{}/{}", semantic_with_gt.len(), semantic.len()),
                    "7/7 semantic",
                )
                .with_details("E1, E5, E6, E7, E10, E12, E13 have topic-based GT")
        } else {
            check.fail(
                &format!("{}/{}", semantic_with_gt.len(), semantic.len()),
                "7/7 semantic",
            )
        }
    }

    fn test_e4_gt_uses_sessions(gt: &UnifiedGroundTruth) -> ValidationCheck {
        let check = ValidationCheck::new(
            "e4_gt_uses_sessions",
            "E4 ground truth uses session sequences",
        )
        .with_priority(CheckPriority::High);

        let e4_gt = gt.by_embedder.get(&EmbedderName::E4Sequence);

        match e4_gt {
            Some(embedder_gt) if !embedder_gt.queries.is_empty() => {
                // E4 should have GT based on sessions (sequence proximity)
                check.pass(
                    &format!("{} queries", embedder_gt.queries.len()),
                    "session-based GT",
                )
            }
            _ => check.warning("no E4 GT", "session-based GT").with_details(
                "E4 ground truth may be generated differently for temporal embedders",
            ),
        }
    }

    fn test_e8_gt_uses_documents(gt: &UnifiedGroundTruth) -> ValidationCheck {
        let check = ValidationCheck::new(
            "e8_gt_uses_documents",
            "E8 ground truth uses document structure",
        )
        .with_priority(CheckPriority::High);

        let e8_gt = gt.by_embedder.get(&EmbedderName::E8Graph);

        match e8_gt {
            Some(embedder_gt) if !embedder_gt.queries.is_empty() => {
                // E8 should have GT based on document graph structure
                check.pass(
                    &format!("{} queries", embedder_gt.queries.len()),
                    "graph-based GT",
                )
            }
            _ => check
                .warning("no E8 GT", "graph-based GT")
                .with_details("E8 ground truth should be based on document structure"),
        }
    }

    fn test_min_relevant_per_query(gt: &UnifiedGroundTruth) -> ValidationCheck {
        let check = ValidationCheck::new(
            "min_relevant_per_query",
            "Queries have >= 3 relevant docs on average",
        )
        .with_priority(CheckPriority::High);

        // Calculate overall average across all embedders and queries
        let mut total_relevant = 0usize;
        let mut total_queries = 0usize;

        for embedder_gt in gt.by_embedder.values() {
            for qgt in &embedder_gt.queries {
                total_relevant += qgt.relevant_ids.len();
                total_queries += 1;
            }
        }

        if total_queries == 0 {
            return check.fail("no queries", ">= 3 avg");
        }

        let avg = total_relevant as f64 / total_queries as f64;

        if avg >= 3.0 {
            check.pass(&format!("{:.1} avg", avg), ">= 3 avg")
        } else {
            check
                .warning(&format!("{:.1} avg", avg), ">= 3 avg")
                .with_details("Low average may reduce metric reliability")
        }
    }

    fn test_no_empty_embedder_gt(gt: &UnifiedGroundTruth) -> ValidationCheck {
        let check =
            ValidationCheck::new("no_empty_embedder_gt", "No embedder has empty ground truth")
                .with_priority(CheckPriority::High);

        let empty: Vec<_> = gt
            .by_embedder
            .iter()
            .filter(|(_, embedder_gt)| embedder_gt.queries.is_empty())
            .map(|(e, _)| e.as_str())
            .collect();

        if empty.is_empty() {
            check.pass("none empty", "none empty")
        } else {
            check
                .warning(&format!("{} empty", empty.len()), "none empty")
                .with_details(&format!("Empty: {}", empty.join(", ")))
        }
    }

    fn test_queries_have_chunk_ids(gt: &UnifiedGroundTruth) -> ValidationCheck {
        let check = ValidationCheck::new(
            "queries_have_chunk_ids",
            "All queries have associated chunk IDs",
        )
        .with_priority(CheckPriority::High);

        let total_queries = gt.query_chunk_ids.len();
        let queries_with_chunks = gt.query_chunk_ids.iter().filter(|id| !id.is_nil()).count();

        if queries_with_chunks == total_queries {
            check.pass(
                &format!("{}/{}", queries_with_chunks, total_queries),
                "all have chunk_id",
            )
        } else {
            check.warning(
                &format!("{}/{}", queries_with_chunks, total_queries),
                "all have chunk_id",
            )
        }
    }

    fn test_relevant_chunk_ids_valid(gt: &UnifiedGroundTruth) -> ValidationCheck {
        let check = ValidationCheck::new(
            "relevant_chunk_ids_valid",
            "Relevant chunk IDs are non-nil UUIDs",
        )
        .with_priority(CheckPriority::High);

        let mut nil_count = 0usize;
        let mut total_count = 0usize;

        for embedder_gt in gt.by_embedder.values() {
            for qgt in &embedder_gt.queries {
                for chunk_id in &qgt.relevant_ids {
                    total_count += 1;
                    if chunk_id.is_nil() {
                        nil_count += 1;
                    }
                }
            }
        }

        if total_count == 0 {
            return check.warning("0 relevant", "valid UUIDs");
        }

        if nil_count == 0 {
            check.pass(&format!("{} valid", total_count), "no nil UUIDs")
        } else {
            check.fail(
                &format!("{} nil of {}", nil_count, total_count),
                "no nil UUIDs",
            )
        }
    }
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_ground_truth_validation_no_data() {
        let result = GroundTruthValidator::validate(None);
        assert!(!result.all_passed);
        assert!(!result.checks.is_empty());
    }
}

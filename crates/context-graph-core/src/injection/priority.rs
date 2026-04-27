//! Priority ranking for injection candidates.
//!
//! Computes priority scores using the formula:
//! ```text
//! priority = relevance_score * recency_factor * diversity_bonus
//! ```
//!
//! # Constitution Compliance
//! - ARCH-09: Topic threshold = weighted_agreement >= 2.5
//! - AP-60: Temporal embedders NEVER count toward topics
//! - AP-10: No NaN/Infinity in similarity scores
//! - AP-12: No magic numbers - use named constants

use chrono::{DateTime, Duration, Utc};

use super::candidate::InjectionCandidate;

// =============================================================================
// RecencyFactor
// =============================================================================

/// Time-based relevance multiplier.
///
/// Recent memories receive a boost; older memories are penalized.
/// Values from constitution.yaml `injection.recency_factors`.
pub struct RecencyFactor;

impl RecencyFactor {
    /// Within 1 hour: strong boost (1.3)
    pub const HOUR_1: f32 = 1.3;
    /// Within 1 day: moderate boost (1.2)
    pub const DAY_1: f32 = 1.2;
    /// Within 1 week: slight boost (1.1)
    pub const WEEK_1: f32 = 1.1;
    /// Within 1 month: neutral (1.0)
    pub const MONTH_1: f32 = 1.0;
    /// 30 days or older: penalty (0.8)
    pub const OLDER: f32 = 0.8;

    /// Compute recency factor based on memory age.
    ///
    /// Uses `Utc::now()` as reference time.
    #[inline]
    pub fn compute(created_at: DateTime<Utc>) -> f32 {
        Self::compute_relative(created_at, Utc::now())
    }

    /// Compute recency factor with explicit reference time (for testing).
    ///
    /// # Arguments
    /// * `created_at` - When the memory was created
    /// * `now` - Reference time (usually current time)
    ///
    /// # Returns
    /// Recency factor in range [0.8, 1.3]
    ///
    /// # Thresholds (from constitution.yaml)
    /// | Age | Factor |
    /// |-----|--------|
    /// | < 1h | 1.3 |
    /// | < 1d | 1.2 |
    /// | < 7d | 1.1 |
    /// | < 30d | 1.0 |
    /// | >= 30d | 0.8 |
    pub fn compute_relative(created_at: DateTime<Utc>, now: DateTime<Utc>) -> f32 {
        let age = now.signed_duration_since(created_at);

        if age < Duration::hours(1) {
            Self::HOUR_1
        } else if age < Duration::days(1) {
            Self::DAY_1
        } else if age < Duration::days(7) {
            Self::WEEK_1
        } else if age < Duration::days(30) {
            Self::MONTH_1
        } else {
            Self::OLDER
        }
    }
}

// =============================================================================
// DiversityBonus
// =============================================================================

/// Multi-space match multiplier based on weighted_agreement.
///
/// Uses weighted agreement from topic clustering, not raw space count.
/// Temporal embedders (E2-E4) have weight 0.0 and do not contribute.
///
/// Values from constitution.yaml `injection.diversity_bonus`.
pub struct DiversityBonus;

impl DiversityBonus {
    /// Strong topic signal: weighted_agreement >= 5.0 -> 1.5x
    pub const STRONG_TOPIC: f32 = 1.5;
    /// Topic threshold met: weighted_agreement in [2.5, 5.0) -> 1.2x
    pub const TOPIC_THRESHOLD: f32 = 1.2;
    /// Related but below threshold: weighted_agreement in [1.0, 2.5) -> 1.0x
    pub const RELATED: f32 = 1.0;
    /// Weak signal: weighted_agreement < 1.0 -> 0.8x
    pub const WEAK: f32 = 0.8;

    /// Threshold for strong topic signal.
    pub const STRONG_TOPIC_THRESHOLD: f32 = 5.0;
    /// Threshold for topic detection (ARCH-09).
    pub const TOPIC_DETECTION_THRESHOLD: f32 = 2.5;
    /// Threshold for related content.
    pub const RELATED_THRESHOLD: f32 = 1.0;

    /// Compute diversity bonus based on weighted_agreement.
    ///
    /// # Arguments
    /// * `weighted_agreement` - Cross-space agreement score (0.0..=8.5)
    ///
    /// # Returns
    /// Diversity bonus in range [0.8, 1.5]
    ///
    /// # Thresholds (from constitution.yaml)
    /// | weighted_agreement | Bonus |
    /// |--------------------|-------|
    /// | >= 5.0 | 1.5 (strong topic) |
    /// | >= 2.5 | 1.2 (topic threshold) |
    /// | >= 1.0 | 1.0 (related) |
    /// | < 1.0 | 0.8 (weak) |
    #[inline]
    pub fn compute(weighted_agreement: f32) -> f32 {
        if weighted_agreement >= Self::STRONG_TOPIC_THRESHOLD {
            Self::STRONG_TOPIC
        } else if weighted_agreement >= Self::TOPIC_DETECTION_THRESHOLD {
            Self::TOPIC_THRESHOLD
        } else if weighted_agreement >= Self::RELATED_THRESHOLD {
            Self::RELATED
        } else {
            Self::WEAK
        }
    }
}

// =============================================================================
// PriorityRanker
// =============================================================================

/// Computes priority scores and ranks injection candidates.
///
/// # Priority Formula
/// ```text
/// priority = relevance_score * recency_factor * diversity_bonus
/// ```
///
/// # Sort Order
/// 1. By category ascending (DivergenceAlert=1, HighRelevanceCluster=2, etc.)
/// 2. Within category, by priority descending (higher score first)
pub struct PriorityRanker;

impl PriorityRanker {
    /// Compute priority for a single candidate.
    ///
    /// Sets `recency_factor`, `diversity_bonus`, and `priority` fields
    /// based on the candidate's `created_at` and `weighted_agreement`.
    #[inline]
    pub fn compute_priority(candidate: &mut InjectionCandidate) {
        let recency = RecencyFactor::compute(candidate.created_at);
        let diversity = DiversityBonus::compute(candidate.weighted_agreement);
        candidate.set_priority_factors(recency, diversity);
    }

    /// Compute priority with explicit reference time (for deterministic testing).
    #[inline]
    pub fn compute_priority_at(candidate: &mut InjectionCandidate, now: DateTime<Utc>) {
        let recency = RecencyFactor::compute_relative(candidate.created_at, now);
        let diversity = DiversityBonus::compute(candidate.weighted_agreement);
        candidate.set_priority_factors(recency, diversity);
    }

    /// Rank candidates by category then priority.
    ///
    /// 1. Computes priority for each candidate
    /// 2. Sorts by category ascending, then priority descending
    ///
    /// Modifies candidates in place.
    pub fn rank_candidates(candidates: &mut [InjectionCandidate]) {
        for candidate in candidates.iter_mut() {
            Self::compute_priority(candidate);
        }
        candidates.sort();
    }

    /// Rank with explicit reference time (for deterministic testing).
    pub fn rank_candidates_at(candidates: &mut [InjectionCandidate], now: DateTime<Utc>) {
        for candidate in candidates.iter_mut() {
            Self::compute_priority_at(candidate, now);
        }
        candidates.sort();
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::injection::candidate::InjectionCategory;
    use crate::teleological::Embedder;
    use uuid::Uuid;

    fn make_candidate_at(
        relevance: f32,
        created_at: DateTime<Utc>,
        weighted_agreement: f32,
        category: InjectionCategory,
    ) -> InjectionCandidate {
        InjectionCandidate::new(
            Uuid::new_v4(),
            "test content".to_string(),
            relevance,
            weighted_agreement,
            vec![Embedder::Semantic],
            category,
            created_at,
        )
    }

    // =========================================================================
    // RecencyFactor Tests
    // =========================================================================

    #[test]
    fn test_recency_factor_within_hour() {
        let now = Utc::now();
        let created = now - Duration::minutes(30);
        let factor = RecencyFactor::compute_relative(created, now);
        assert!(
            (factor - 1.3).abs() < 0.001,
            "Within 1h should be 1.3, got {}",
            factor
        );
        println!("[PASS] RecencyFactor <1h = 1.3");
    }

    #[test]
    fn test_recency_factor_within_day() {
        let now = Utc::now();
        let created = now - Duration::hours(6);
        let factor = RecencyFactor::compute_relative(created, now);
        assert!(
            (factor - 1.2).abs() < 0.001,
            "Within 1d should be 1.2, got {}",
            factor
        );
        println!("[PASS] RecencyFactor <1d = 1.2");
    }

    #[test]
    fn test_recency_factor_within_week() {
        let now = Utc::now();
        let created = now - Duration::days(3);
        let factor = RecencyFactor::compute_relative(created, now);
        assert!(
            (factor - 1.1).abs() < 0.001,
            "Within 7d should be 1.1, got {}",
            factor
        );
        println!("[PASS] RecencyFactor <7d = 1.1");
    }

    #[test]
    fn test_recency_factor_within_month() {
        let now = Utc::now();
        let created = now - Duration::days(15);
        let factor = RecencyFactor::compute_relative(created, now);
        assert!(
            (factor - 1.0).abs() < 0.001,
            "Within 30d should be 1.0, got {}",
            factor
        );
        println!("[PASS] RecencyFactor <30d = 1.0");
    }

    #[test]
    fn test_recency_factor_older() {
        let now = Utc::now();
        let created = now - Duration::days(100);
        let factor = RecencyFactor::compute_relative(created, now);
        assert!(
            (factor - 0.8).abs() < 0.001,
            "Older should be 0.8, got {}",
            factor
        );
        println!("[PASS] RecencyFactor >=30d = 0.8");
    }

    #[test]
    fn test_recency_factor_boundary_exactly_1_hour() {
        let now = Utc::now();
        let created = now - Duration::hours(1);
        let factor = RecencyFactor::compute_relative(created, now);
        // Exactly 1 hour should be in <1d bucket (1.2)
        assert!(
            (factor - 1.2).abs() < 0.001,
            "Exactly 1h should be 1.2, got {}",
            factor
        );
        println!("[PASS] RecencyFactor boundary at 1h = 1.2");
    }

    #[test]
    fn test_recency_factor_boundary_exactly_1_day() {
        let now = Utc::now();
        let created = now - Duration::days(1);
        let factor = RecencyFactor::compute_relative(created, now);
        // Exactly 1 day should be in <7d bucket (1.1)
        assert!(
            (factor - 1.1).abs() < 0.001,
            "Exactly 1d should be 1.1, got {}",
            factor
        );
        println!("[PASS] RecencyFactor boundary at 1d = 1.1");
    }

    #[test]
    fn test_recency_factor_boundary_exactly_7_days() {
        let now = Utc::now();
        let created = now - Duration::days(7);
        let factor = RecencyFactor::compute_relative(created, now);
        // Exactly 7 days should be in <30d bucket (1.0)
        assert!(
            (factor - 1.0).abs() < 0.001,
            "Exactly 7d should be 1.0, got {}",
            factor
        );
        println!("[PASS] RecencyFactor boundary at 7d = 1.0");
    }

    #[test]
    fn test_recency_factor_boundary_exactly_30_days() {
        let now = Utc::now();
        let created = now - Duration::days(30);
        let factor = RecencyFactor::compute_relative(created, now);
        // Exactly 30 days should be in >=30d bucket (0.8)
        assert!(
            (factor - 0.8).abs() < 0.001,
            "Exactly 30d should be 0.8, got {}",
            factor
        );
        println!("[PASS] RecencyFactor boundary at 30d = 0.8");
    }

    #[test]
    fn test_recency_factor_just_under_1_hour() {
        let now = Utc::now();
        let created = now - Duration::minutes(59);
        let factor = RecencyFactor::compute_relative(created, now);
        assert!(
            (factor - 1.3).abs() < 0.001,
            "Just under 1h should be 1.3, got {}",
            factor
        );
        println!("[PASS] RecencyFactor 59 minutes = 1.3");
    }

    // =========================================================================
    // DiversityBonus Tests
    // =========================================================================

    #[test]
    fn test_diversity_bonus_weak() {
        assert!((DiversityBonus::compute(0.0) - 0.8).abs() < 0.001);
        assert!((DiversityBonus::compute(0.5) - 0.8).abs() < 0.001);
        assert!((DiversityBonus::compute(0.99) - 0.8).abs() < 0.001);
        println!("[PASS] DiversityBonus <1.0 = 0.8 (WEAK)");
    }

    #[test]
    fn test_diversity_bonus_related() {
        assert!((DiversityBonus::compute(1.0) - 1.0).abs() < 0.001);
        assert!((DiversityBonus::compute(2.0) - 1.0).abs() < 0.001);
        assert!((DiversityBonus::compute(2.49) - 1.0).abs() < 0.001);
        println!("[PASS] DiversityBonus [1.0, 2.5) = 1.0 (RELATED)");
    }

    #[test]
    fn test_diversity_bonus_topic_threshold() {
        assert!((DiversityBonus::compute(2.5) - 1.2).abs() < 0.001);
        assert!((DiversityBonus::compute(3.5) - 1.2).abs() < 0.001);
        assert!((DiversityBonus::compute(4.99) - 1.2).abs() < 0.001);
        println!("[PASS] DiversityBonus [2.5, 5.0) = 1.2 (TOPIC_THRESHOLD)");
    }

    #[test]
    fn test_diversity_bonus_strong_topic() {
        assert!((DiversityBonus::compute(5.0) - 1.5).abs() < 0.001);
        assert!((DiversityBonus::compute(7.0) - 1.5).abs() < 0.001);
        assert!((DiversityBonus::compute(8.5) - 1.5).abs() < 0.001);
        println!("[PASS] DiversityBonus >=5.0 = 1.5 (STRONG_TOPIC)");
    }

    #[test]
    fn test_diversity_bonus_boundary_exactly_1() {
        let bonus = DiversityBonus::compute(1.0);
        assert!(
            (bonus - 1.0).abs() < 0.001,
            "Exactly 1.0 should be RELATED (1.0), got {}",
            bonus
        );
        println!("[PASS] DiversityBonus boundary at 1.0 = 1.0");
    }

    #[test]
    fn test_diversity_bonus_boundary_exactly_2_5() {
        let bonus = DiversityBonus::compute(2.5);
        assert!(
            (bonus - 1.2).abs() < 0.001,
            "Exactly 2.5 should be TOPIC_THRESHOLD (1.2), got {}",
            bonus
        );
        println!("[PASS] DiversityBonus boundary at 2.5 = 1.2");
    }

    #[test]
    fn test_diversity_bonus_boundary_exactly_5() {
        let bonus = DiversityBonus::compute(5.0);
        assert!(
            (bonus - 1.5).abs() < 0.001,
            "Exactly 5.0 should be STRONG_TOPIC (1.5), got {}",
            bonus
        );
        println!("[PASS] DiversityBonus boundary at 5.0 = 1.5");
    }

    // =========================================================================
    // PriorityRanker Tests
    // =========================================================================

    #[test]
    fn test_priority_computation_formula() {
        let now = Utc::now();
        let created = now - Duration::hours(2); // <1d -> recency=1.2

        let mut candidate = make_candidate_at(
            0.82, // relevance
            created,
            3.5, // weighted_agreement -> diversity=1.2 (topic threshold)
            InjectionCategory::HighRelevanceCluster,
        );

        PriorityRanker::compute_priority_at(&mut candidate, now);

        // Verify factors
        assert!(
            (candidate.recency_factor - 1.2).abs() < 0.001,
            "recency should be 1.2, got {}",
            candidate.recency_factor
        );
        assert!(
            (candidate.diversity_bonus - 1.2).abs() < 0.001,
            "diversity should be 1.2, got {}",
            candidate.diversity_bonus
        );

        // priority = 0.82 * 1.2 * 1.2 = 1.1808
        let expected = 0.82 * 1.2 * 1.2;
        assert!(
            (candidate.priority - expected).abs() < 0.01,
            "priority should be {}, got {}",
            expected,
            candidate.priority
        );
        println!(
            "[PASS] Priority formula: 0.82 * 1.2 * 1.2 = {:.4}",
            candidate.priority
        );
    }

    #[test]
    fn test_ranking_by_category_first() {
        let now = Utc::now();

        let mut candidates = vec![
            make_candidate_at(0.9, now, 5.5, InjectionCategory::SingleSpaceMatch),
            make_candidate_at(0.5, now, 0.5, InjectionCategory::DivergenceAlert),
            make_candidate_at(0.7, now, 3.0, InjectionCategory::HighRelevanceCluster),
        ];

        PriorityRanker::rank_candidates_at(&mut candidates, now);

        assert_eq!(candidates[0].category, InjectionCategory::DivergenceAlert);
        assert_eq!(
            candidates[1].category,
            InjectionCategory::HighRelevanceCluster
        );
        assert_eq!(candidates[2].category, InjectionCategory::SingleSpaceMatch);
        println!("[PASS] Ranking sorts by category first");
    }

    #[test]
    fn test_ranking_by_priority_within_category() {
        let now = Utc::now();

        let mut candidates = vec![
            make_candidate_at(0.7, now, 3.0, InjectionCategory::HighRelevanceCluster),
            make_candidate_at(0.9, now, 3.0, InjectionCategory::HighRelevanceCluster),
            make_candidate_at(0.8, now, 3.0, InjectionCategory::HighRelevanceCluster),
        ];

        PriorityRanker::rank_candidates_at(&mut candidates, now);

        // Same category, sorted by priority descending
        assert!(
            candidates[0].priority > candidates[1].priority,
            "First should have highest priority: {} > {}",
            candidates[0].priority,
            candidates[1].priority
        );
        assert!(
            candidates[1].priority > candidates[2].priority,
            "Second should have higher priority than third: {} > {}",
            candidates[1].priority,
            candidates[2].priority
        );
        println!("[PASS] Within category, sort by priority descending");
    }

    #[test]
    fn test_spec_example_memory_a_vs_b() {
        // From constitution spec example:
        // Memory A: relevance=0.82, 2h ago (recency=1.2), weighted_agreement=3.5 (diversity=1.2)
        //           priority = 0.82 * 1.2 * 1.2 = 1.1808
        // Memory B: relevance=0.90, 3d ago (recency=1.1), weighted_agreement=1.5 (diversity=1.0)
        //           priority = 0.90 * 1.1 * 1.0 = 0.99
        // Memory A should rank higher despite lower relevance

        let now = Utc::now();

        let mut memory_a = make_candidate_at(
            0.82,
            now - Duration::hours(2),
            3.5,
            InjectionCategory::HighRelevanceCluster,
        );

        let mut memory_b = make_candidate_at(
            0.90,
            now - Duration::days(3),
            1.5,
            InjectionCategory::HighRelevanceCluster,
        );

        PriorityRanker::compute_priority_at(&mut memory_a, now);
        PriorityRanker::compute_priority_at(&mut memory_b, now);

        println!(
            "Memory A: relevance={}, recency={}, diversity={}, priority={}",
            memory_a.relevance_score,
            memory_a.recency_factor,
            memory_a.diversity_bonus,
            memory_a.priority
        );
        println!(
            "Memory B: relevance={}, recency={}, diversity={}, priority={}",
            memory_b.relevance_score,
            memory_b.recency_factor,
            memory_b.diversity_bonus,
            memory_b.priority
        );

        // Memory A should have higher priority
        assert!(
            memory_a.priority > memory_b.priority,
            "Memory A ({}) should rank higher than Memory B ({})",
            memory_a.priority,
            memory_b.priority
        );

        // Verify exact values
        assert!(
            (memory_a.priority - 1.1808).abs() < 0.01,
            "Memory A priority should be ~1.1808, got {}",
            memory_a.priority
        );
        assert!(
            (memory_b.priority - 0.99).abs() < 0.01,
            "Memory B priority should be ~0.99, got {}",
            memory_b.priority
        );

        println!("[PASS] Spec example: Memory A (1.18) > Memory B (0.99)");
    }

    #[test]
    fn test_empty_candidates_slice() {
        let now = Utc::now();
        let mut candidates: Vec<InjectionCandidate> = vec![];
        PriorityRanker::rank_candidates_at(&mut candidates, now);
        assert!(candidates.is_empty());
        println!("[PASS] Empty candidates slice handled correctly");
    }

    #[test]
    fn test_single_candidate() {
        let now = Utc::now();
        let mut candidates = vec![make_candidate_at(
            0.75,
            now - Duration::minutes(30),
            4.0,
            InjectionCategory::HighRelevanceCluster,
        )];

        PriorityRanker::rank_candidates_at(&mut candidates, now);

        // Should have recency=1.3 (within hour), diversity=1.2 (topic threshold)
        assert!(
            (candidates[0].recency_factor - 1.3).abs() < 0.001,
            "Single candidate recency should be 1.3"
        );
        assert!(
            (candidates[0].diversity_bonus - 1.2).abs() < 0.001,
            "Single candidate diversity should be 1.2"
        );
        let expected = 0.75 * 1.3 * 1.2;
        assert!(
            (candidates[0].priority - expected).abs() < 0.01,
            "Single candidate priority should be {}, got {}",
            expected,
            candidates[0].priority
        );
        println!("[PASS] Single candidate ranking works correctly");
    }

    // =========================================================================
    // Edge Case Tests (Required by Manual Verification Protocol)
    // =========================================================================

    #[test]
    fn test_edge_case_exactly_1_hour_boundary() {
        // Edge Case 1: Boundary at exactly 1 hour
        // Expected: Should fall into <1d bucket (1.2), not <1h (1.3)

        let now = Utc::now();
        let created = now - Duration::hours(1);

        let age = now.signed_duration_since(created);
        println!("EDGE CASE 1: Boundary at exactly 1 hour");
        println!("  Before: created_at = now - 1 hour");
        println!("  Age duration: {} seconds", age.num_seconds());

        let factor = RecencyFactor::compute_relative(created, now);

        println!("  After: recency_factor = {}", factor);
        println!("  Expected: 1.2 (falls into <1d bucket, not <1h)");

        assert!(
            (factor - 1.2).abs() < 0.001,
            "Exactly 1h should be 1.2, got {}",
            factor
        );
        println!("[PASS] Edge Case 1: Boundary at exactly 1 hour = 1.2");
    }

    #[test]
    fn test_edge_case_weighted_agreement_2_5_boundary() {
        // Edge Case 2: Boundary at weighted_agreement = 2.5 exactly
        // Expected: Should be TOPIC_THRESHOLD (1.2), not RELATED (1.0)

        let weighted_agreement = 2.5;
        println!("EDGE CASE 2: Boundary at weighted_agreement = 2.5");
        println!("  Before: weighted_agreement = {}", weighted_agreement);

        let bonus = DiversityBonus::compute(weighted_agreement);

        println!("  After: diversity_bonus = {}", bonus);
        println!("  Expected: 1.2 (TOPIC_THRESHOLD, not RELATED)");

        assert!(
            (bonus - 1.2).abs() < 0.001,
            "Exactly 2.5 should be 1.2, got {}",
            bonus
        );
        println!("[PASS] Edge Case 2: Boundary at weighted_agreement = 2.5 = 1.2");
    }

    #[test]
    fn test_edge_case_zero_relevance_score() {
        // Edge Case 3: Zero relevance score
        // Expected: priority = 0.0 * 1.3 * 1.5 = 0.0

        let now = Utc::now();
        let created = now - Duration::minutes(30); // <1h -> recency=1.3

        let mut candidate = make_candidate_at(
            0.0, // zero relevance
            created,
            6.0, // >= 5.0 -> diversity=1.5
            InjectionCategory::HighRelevanceCluster,
        );

        println!("EDGE CASE 3: Zero relevance score");
        println!(
            "  Before: relevance_score = {}, created_at = now - 30min, weighted_agreement = {}",
            candidate.relevance_score, candidate.weighted_agreement
        );

        PriorityRanker::compute_priority_at(&mut candidate, now);

        println!(
            "  After: recency_factor = {}, diversity_bonus = {}, priority = {}",
            candidate.recency_factor, candidate.diversity_bonus, candidate.priority
        );
        println!("  Expected: recency=1.3, diversity=1.5, priority=0.0");

        assert!(
            (candidate.recency_factor - 1.3).abs() < 0.001,
            "Recency should be 1.3, got {}",
            candidate.recency_factor
        );
        assert!(
            (candidate.diversity_bonus - 1.5).abs() < 0.001,
            "Diversity should be 1.5, got {}",
            candidate.diversity_bonus
        );
        assert!(
            candidate.priority.abs() < 0.001,
            "Priority should be 0.0, got {}",
            candidate.priority
        );
        println!("[PASS] Edge Case 3: Zero relevance -> priority = 0.0");
    }

    // =========================================================================
    // Additional boundary tests
    // =========================================================================

    #[test]
    fn test_future_created_at() {
        // Memory created in the future (edge case for time handling)
        let now = Utc::now();
        let created = now + Duration::hours(1);

        let factor = RecencyFactor::compute_relative(created, now);

        // Future memory should still get highest boost (negative age)
        assert!(
            (factor - 1.3).abs() < 0.001,
            "Future memory should be 1.3, got {}",
            factor
        );
        println!("[PASS] Future created_at handled correctly (1.3)");
    }

    #[test]
    fn test_very_old_memory() {
        let now = Utc::now();
        let created = now - Duration::days(365 * 10); // 10 years old

        let factor = RecencyFactor::compute_relative(created, now);

        assert!(
            (factor - 0.8).abs() < 0.001,
            "Very old memory should be 0.8, got {}",
            factor
        );
        println!("[PASS] Very old memory (10 years) handled correctly (0.8)");
    }

    #[test]
    fn test_max_weighted_agreement() {
        let bonus = DiversityBonus::compute(8.5);
        assert!(
            (bonus - 1.5).abs() < 0.001,
            "Max weighted_agreement should give 1.5, got {}",
            bonus
        );
        println!("[PASS] Max weighted_agreement (8.5) -> 1.5");
    }

    #[test]
    fn test_mixed_categories_with_priorities() {
        let now = Utc::now();

        let mut candidates = vec![
            // RecentSession with high relevance
            make_candidate_at(0.95, now, 5.0, InjectionCategory::RecentSession),
            // DivergenceAlert with low relevance
            make_candidate_at(
                0.3,
                now - Duration::days(5),
                0.5,
                InjectionCategory::DivergenceAlert,
            ),
            // HighRelevanceCluster with medium relevance
            make_candidate_at(
                0.7,
                now - Duration::hours(2),
                3.0,
                InjectionCategory::HighRelevanceCluster,
            ),
            // SingleSpaceMatch with high relevance
            make_candidate_at(
                0.85,
                now - Duration::minutes(10),
                2.0,
                InjectionCategory::SingleSpaceMatch,
            ),
        ];

        PriorityRanker::rank_candidates_at(&mut candidates, now);

        // Verify category order (regardless of relevance)
        assert_eq!(candidates[0].category, InjectionCategory::DivergenceAlert);
        assert_eq!(
            candidates[1].category,
            InjectionCategory::HighRelevanceCluster
        );
        assert_eq!(candidates[2].category, InjectionCategory::SingleSpaceMatch);
        assert_eq!(candidates[3].category, InjectionCategory::RecentSession);

        println!("[PASS] Mixed categories sorted correctly (category takes precedence)");
    }

    #[test]
    fn test_priority_factors_preserve_relevance_ordering_within_category() {
        let now = Utc::now();

        // Three candidates in same category, same age, same weighted_agreement
        // Only relevance differs
        let mut candidates = vec![
            make_candidate_at(0.6, now, 3.0, InjectionCategory::HighRelevanceCluster),
            make_candidate_at(0.8, now, 3.0, InjectionCategory::HighRelevanceCluster),
            make_candidate_at(0.4, now, 3.0, InjectionCategory::HighRelevanceCluster),
        ];

        PriorityRanker::rank_candidates_at(&mut candidates, now);

        // Should be sorted by priority descending (which equals relevance * same_factors)
        assert!(
            candidates[0].relevance_score > candidates[1].relevance_score,
            "First should have highest relevance"
        );
        assert!(
            candidates[1].relevance_score > candidates[2].relevance_score,
            "Second should have higher relevance than third"
        );

        println!("[PASS] Relevance ordering preserved within category");
    }
}

//! InjectionCandidate and InjectionCategory types.
//!
//! These types form the foundation of the injection pipeline,
//! tracking candidate memories with their computed scores.
//!
//! # Constitution Compliance
//! - ARCH-09: Topic threshold = weighted_agreement >= 2.5
//! - AP-60: Temporal embedders NEVER count toward topics
//! - AP-10: No NaN/Infinity in similarity/relevance scores
//! - AP-14: No .unwrap() in library code

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::clustering::MAX_WEIGHTED_AGREEMENT;
use crate::teleological::Embedder;

// =============================================================================
// InjectionCategory
// =============================================================================

/// Priority category for injection candidates.
///
/// Lower priority number = higher rank (processed first).
/// Each category has an associated token budget from constitution.yaml.
///
/// | Category | Priority | Budget |
/// |----------|----------|--------|
/// | DivergenceAlert | 1 | 200 tokens |
/// | HighRelevanceCluster | 2 | 400 tokens |
/// | SingleSpaceMatch | 3 | 300 tokens |
/// | RecentSession | 4 | 200 tokens |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InjectionCategory {
    /// Divergence alerts - highest priority (200 tokens).
    /// Triggered when current activity differs from recent work
    /// in SEMANTIC embedding spaces only.
    DivergenceAlert,

    /// High relevance cluster - weighted_agreement >= 2.5 (400 tokens).
    /// Strong topic signal across multiple embedding spaces.
    HighRelevanceCluster,

    /// Single/few space match - weighted_agreement in [1.0, 2.5) (300 tokens).
    /// Related content but not strong enough for topic.
    SingleSpaceMatch,

    /// Recent session context (200 tokens).
    /// Last session summary for continuity.
    RecentSession,
}

impl InjectionCategory {
    /// Returns priority rank (1 = highest, 4 = lowest).
    ///
    /// Used for sorting candidates - lower number = processed first.
    #[inline]
    pub const fn priority(&self) -> u8 {
        match self {
            InjectionCategory::DivergenceAlert => 1,
            InjectionCategory::HighRelevanceCluster => 2,
            InjectionCategory::SingleSpaceMatch => 3,
            InjectionCategory::RecentSession => 4,
        }
    }

    /// Returns the token budget for this category.
    ///
    /// From constitution.yaml injection.priorities section.
    #[inline]
    pub const fn token_budget(&self) -> u32 {
        match self {
            InjectionCategory::DivergenceAlert => 200,
            InjectionCategory::HighRelevanceCluster => 400,
            InjectionCategory::SingleSpaceMatch => 300,
            InjectionCategory::RecentSession => 200,
        }
    }

    /// Returns all category variants in priority order.
    #[inline]
    pub const fn all() -> [InjectionCategory; 4] {
        [
            InjectionCategory::DivergenceAlert,
            InjectionCategory::HighRelevanceCluster,
            InjectionCategory::SingleSpaceMatch,
            InjectionCategory::RecentSession,
        ]
    }

    /// Determine category from weighted_agreement score.
    ///
    /// Uses thresholds from constitution.yaml:
    /// - >= 2.5: HighRelevanceCluster
    /// - >= 1.0: SingleSpaceMatch
    /// - < 1.0: Not recommended for injection
    ///
    /// Note: DivergenceAlert and RecentSession are set explicitly,
    /// not derived from weighted_agreement.
    #[inline]
    pub fn from_weighted_agreement(weighted_agreement: f32) -> Option<InjectionCategory> {
        if weighted_agreement >= 2.5 {
            Some(InjectionCategory::HighRelevanceCluster)
        } else if weighted_agreement >= 1.0 {
            Some(InjectionCategory::SingleSpaceMatch)
        } else {
            None // Below threshold for injection
        }
    }
}

impl Ord for InjectionCategory {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Lower priority number = higher rank (comes first)
        self.priority().cmp(&other.priority())
    }
}

impl PartialOrd for InjectionCategory {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::fmt::Display for InjectionCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InjectionCategory::DivergenceAlert => write!(f, "DivergenceAlert"),
            InjectionCategory::HighRelevanceCluster => write!(f, "HighRelevanceCluster"),
            InjectionCategory::SingleSpaceMatch => write!(f, "SingleSpaceMatch"),
            InjectionCategory::RecentSession => write!(f, "RecentSession"),
        }
    }
}

// =============================================================================
// InjectionCandidate
// =============================================================================

/// Token estimation multiplier (words to tokens).
/// Empirically determined: tokens ≈ words × 1.3
pub const TOKEN_MULTIPLIER: f32 = 1.3;

/// Minimum recency factor (for old memories > 90 days).
pub const MIN_RECENCY_FACTOR: f32 = 0.8;

/// Maximum recency factor (for recent memories < 1 hour).
pub const MAX_RECENCY_FACTOR: f32 = 1.3;

/// Minimum diversity bonus (weak agreement < 1.0).
pub const MIN_DIVERSITY_BONUS: f32 = 0.8;

/// Maximum diversity bonus (strong agreement >= 5.0).
pub const MAX_DIVERSITY_BONUS: f32 = 1.5;

/// A candidate memory for context injection with computed scores.
///
/// This is the primary data structure flowing through the injection pipeline.
/// Each candidate carries all information needed for priority ranking and
/// budget selection.
///
/// # Score Computation
///
/// - `relevance_score`: Base similarity score (0.0..=1.0)
/// - `recency_factor`: Time-based multiplier (0.8..=1.3)
/// - `diversity_bonus`: Multi-space agreement bonus (0.8..=1.5)
/// - `priority`: Final score = relevance × recency × diversity
///
/// # Weighted Agreement
///
/// Per constitution.yaml, computed using category weights:
/// - SEMANTIC (E1, E5, E6, E7, E10, E12, E13): 1.0
/// - TEMPORAL (E2, E3, E4): 0.0 (EXCLUDED per AP-60)
/// - RELATIONAL (E8, E11): 0.5
/// - STRUCTURAL (E9): 0.5
/// - Max possible: 8.5
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectionCandidate {
    /// Memory identifier (UUID from Memory.id).
    pub memory_id: Uuid,

    /// Memory content text.
    pub content: String,

    /// Base relevance score from similarity search (0.0..=1.0).
    /// Must be validated to not contain NaN/Infinity (AP-10).
    pub relevance_score: f32,

    /// Time-based multiplier (0.8..=1.3), computed by PriorityRanker.
    /// - < 1h: 1.3
    /// - < 1d: 1.2
    /// - < 7d: 1.1
    /// - < 30d: 1.0
    /// - > 90d: 0.8
    pub recency_factor: f32,

    /// Multi-space agreement bonus (0.8..=1.5), computed by PriorityRanker.
    /// - weighted_agreement >= 5.0: 1.5
    /// - weighted_agreement in [2.5, 5.0): 1.2
    /// - weighted_agreement in [1.0, 2.5): 1.0
    /// - weighted_agreement < 1.0: 0.8
    pub diversity_bonus: f32,

    /// Weighted agreement score (0.0..=8.5) from multi-space clustering.
    /// Uses category weights per constitution.yaml.
    pub weighted_agreement: f32,

    /// Which embedding spaces matched (exceeded similarity threshold).
    pub matching_spaces: Vec<Embedder>,

    /// Final priority = relevance_score × recency_factor × diversity_bonus.
    /// Computed later by PriorityRanker.
    pub priority: f32,

    /// Estimated token count for budget tracking.
    /// Computed as: word_count × 1.3 (empirical multiplier).
    pub token_count: u32,

    /// Category determines budget pool and sort order.
    pub category: InjectionCategory,

    /// When memory was created.
    pub created_at: DateTime<Utc>,
}

impl InjectionCandidate {
    /// Create a new injection candidate with initial scores.
    ///
    /// `recency_factor`, `diversity_bonus`, and `priority` are initialized
    /// to defaults and should be computed later via `set_priority_factors()`.
    ///
    /// # Arguments
    ///
    /// * `memory_id` - UUID of the source memory
    /// * `content` - Memory content text
    /// * `relevance_score` - Base similarity score (0.0..=1.0)
    /// * `weighted_agreement` - Cross-space agreement (0.0..=8.5)
    /// * `matching_spaces` - Embedders that exceeded threshold
    /// * `category` - Injection category
    /// * `created_at` - Memory creation timestamp
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - `relevance_score` not in 0.0..=1.0
    /// - `weighted_agreement` not in 0.0..=8.5
    /// - `relevance_score` is NaN or Infinity (AP-10)
    /// - `weighted_agreement` is NaN or Infinity (AP-10)
    pub fn new(
        memory_id: Uuid,
        content: String,
        relevance_score: f32,
        weighted_agreement: f32,
        matching_spaces: Vec<Embedder>,
        category: InjectionCategory,
        created_at: DateTime<Utc>,
    ) -> Self {
        // Validate relevance_score (AP-10: no NaN/Infinity)
        assert!(
            !relevance_score.is_nan() && !relevance_score.is_infinite(),
            "relevance_score cannot be NaN or Infinity, got {}",
            relevance_score
        );
        assert!(
            (0.0..=1.0).contains(&relevance_score),
            "relevance_score must be 0.0..=1.0, got {}",
            relevance_score
        );

        // Validate weighted_agreement (AP-10: no NaN/Infinity)
        assert!(
            !weighted_agreement.is_nan() && !weighted_agreement.is_infinite(),
            "weighted_agreement cannot be NaN or Infinity, got {}",
            weighted_agreement
        );
        assert!(
            (0.0..=MAX_WEIGHTED_AGREEMENT).contains(&weighted_agreement),
            "weighted_agreement must be 0.0..=8.5, got {}",
            weighted_agreement
        );

        // Estimate tokens: words × 1.3
        let word_count = content.split_whitespace().count();
        let token_count = (word_count as f32 * TOKEN_MULTIPLIER).ceil() as u32;

        Self {
            memory_id,
            content,
            relevance_score,
            recency_factor: 1.0,  // Default, computed later
            diversity_bonus: 1.0, // Default, computed later
            weighted_agreement,
            matching_spaces,
            priority: 0.0, // Computed later
            token_count,
            category,
            created_at,
        }
    }

    /// Set computed priority factors and update final priority.
    ///
    /// # Arguments
    ///
    /// * `recency` - Time-based factor (0.8..=1.3)
    /// * `diversity` - Agreement-based bonus (0.8..=1.5)
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - `recency` not in 0.8..=1.3
    /// - `diversity` not in 0.8..=1.5
    /// - Either value is NaN or Infinity (AP-10)
    pub fn set_priority_factors(&mut self, recency: f32, diversity: f32) {
        // Validate recency (AP-10)
        assert!(
            !recency.is_nan() && !recency.is_infinite(),
            "recency cannot be NaN or Infinity, got {}",
            recency
        );
        assert!(
            (MIN_RECENCY_FACTOR..=MAX_RECENCY_FACTOR).contains(&recency),
            "recency must be {}..={}, got {}",
            MIN_RECENCY_FACTOR,
            MAX_RECENCY_FACTOR,
            recency
        );

        // Validate diversity (AP-10)
        assert!(
            !diversity.is_nan() && !diversity.is_infinite(),
            "diversity cannot be NaN or Infinity, got {}",
            diversity
        );
        assert!(
            (MIN_DIVERSITY_BONUS..=MAX_DIVERSITY_BONUS).contains(&diversity),
            "diversity must be {}..={}, got {}",
            MIN_DIVERSITY_BONUS,
            MAX_DIVERSITY_BONUS,
            diversity
        );

        self.recency_factor = recency;
        self.diversity_bonus = diversity;
        self.priority = self.relevance_score * recency * diversity;
    }

    /// Check if this candidate would fit in remaining budget.
    #[inline]
    pub fn fits_budget(&self, remaining_tokens: u32) -> bool {
        self.token_count <= remaining_tokens
    }

    /// Get the semantic space count (spaces with weight > 0).
    ///
    /// Excludes temporal embedders (E2-E4) per AP-60.
    pub fn semantic_space_count(&self) -> usize {
        use crate::embeddings::is_temporal;

        self.matching_spaces
            .iter()
            .filter(|&&e| !is_temporal(e))
            .count()
    }
}

impl Ord for InjectionCandidate {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // First by category (lower priority number first)
        match self.category.cmp(&other.category) {
            std::cmp::Ordering::Equal => {
                // Within same category, by priority descending (higher score first)
                // Use total_cmp for NaN-safe comparison (AP-10)
                other.priority.total_cmp(&self.priority)
            }
            ordering => ordering,
        }
    }
}

impl PartialOrd for InjectionCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for InjectionCandidate {
    fn eq(&self, other: &Self) -> bool {
        self.memory_id == other.memory_id
    }
}

impl Eq for InjectionCandidate {}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_candidate(
        relevance: f32,
        weighted_agreement: f32,
        category: InjectionCategory,
    ) -> InjectionCandidate {
        InjectionCandidate::new(
            Uuid::new_v4(),
            "test content for testing".to_string(),
            relevance,
            weighted_agreement,
            vec![Embedder::Semantic, Embedder::Code],
            category,
            Utc::now(),
        )
    }

    // =========================================================================
    // InjectionCategory Tests
    // =========================================================================

    #[test]
    fn test_category_priority_order() {
        assert!(
            InjectionCategory::DivergenceAlert.priority()
                < InjectionCategory::HighRelevanceCluster.priority()
        );
        assert!(
            InjectionCategory::HighRelevanceCluster.priority()
                < InjectionCategory::SingleSpaceMatch.priority()
        );
        assert!(
            InjectionCategory::SingleSpaceMatch.priority()
                < InjectionCategory::RecentSession.priority()
        );
        println!("[PASS] Category priority ordering correct");
    }

    #[test]
    fn test_category_token_budgets() {
        assert_eq!(InjectionCategory::DivergenceAlert.token_budget(), 200);
        assert_eq!(InjectionCategory::HighRelevanceCluster.token_budget(), 400);
        assert_eq!(InjectionCategory::SingleSpaceMatch.token_budget(), 300);
        assert_eq!(InjectionCategory::RecentSession.token_budget(), 200);
        println!("[PASS] Category token budgets match constitution");
    }

    #[test]
    fn test_category_from_weighted_agreement() {
        // >= 2.5 -> HighRelevanceCluster
        assert_eq!(
            InjectionCategory::from_weighted_agreement(3.0),
            Some(InjectionCategory::HighRelevanceCluster)
        );
        assert_eq!(
            InjectionCategory::from_weighted_agreement(2.5),
            Some(InjectionCategory::HighRelevanceCluster)
        );

        // >= 1.0 but < 2.5 -> SingleSpaceMatch
        assert_eq!(
            InjectionCategory::from_weighted_agreement(2.0),
            Some(InjectionCategory::SingleSpaceMatch)
        );
        assert_eq!(
            InjectionCategory::from_weighted_agreement(1.0),
            Some(InjectionCategory::SingleSpaceMatch)
        );

        // < 1.0 -> None
        assert_eq!(InjectionCategory::from_weighted_agreement(0.5), None);
        assert_eq!(InjectionCategory::from_weighted_agreement(0.0), None);

        println!("[PASS] from_weighted_agreement thresholds correct");
    }

    #[test]
    fn test_category_ord() {
        let mut categories = vec![
            InjectionCategory::RecentSession,
            InjectionCategory::DivergenceAlert,
            InjectionCategory::SingleSpaceMatch,
            InjectionCategory::HighRelevanceCluster,
        ];
        categories.sort();

        assert_eq!(categories[0], InjectionCategory::DivergenceAlert);
        assert_eq!(categories[1], InjectionCategory::HighRelevanceCluster);
        assert_eq!(categories[2], InjectionCategory::SingleSpaceMatch);
        assert_eq!(categories[3], InjectionCategory::RecentSession);
        println!("[PASS] Category sorting works correctly");
    }

    // =========================================================================
    // InjectionCandidate Tests
    // =========================================================================

    #[test]
    fn test_candidate_creation() {
        let c = make_candidate(0.8, 3.0, InjectionCategory::HighRelevanceCluster);

        assert!((c.relevance_score - 0.8).abs() < f32::EPSILON);
        assert!((c.weighted_agreement - 3.0).abs() < f32::EPSILON);
        assert_eq!(c.category, InjectionCategory::HighRelevanceCluster);
        assert_eq!(c.recency_factor, 1.0); // Default
        assert_eq!(c.diversity_bonus, 1.0); // Default
        assert_eq!(c.priority, 0.0); // Not yet computed
        println!("[PASS] Candidate creation with defaults");
    }

    #[test]
    fn test_token_estimation() {
        let c = InjectionCandidate::new(
            Uuid::new_v4(),
            "one two three four five".to_string(), // 5 words
            0.5,
            2.0,
            vec![],
            InjectionCategory::SingleSpaceMatch,
            Utc::now(),
        );

        // 5 words × 1.3 = 6.5 → ceil = 7
        assert_eq!(c.token_count, 7);
        println!("[PASS] Token estimation (words × 1.3)");
    }

    #[test]
    fn test_set_priority_factors() {
        let mut c = make_candidate(0.8, 3.0, InjectionCategory::HighRelevanceCluster);

        c.set_priority_factors(1.2, 1.3);

        assert!((c.recency_factor - 1.2).abs() < f32::EPSILON);
        assert!((c.diversity_bonus - 1.3).abs() < f32::EPSILON);

        // priority = 0.8 × 1.2 × 1.3 = 1.248
        let expected = 0.8 * 1.2 * 1.3;
        assert!((c.priority - expected).abs() < 0.001);
        println!("[PASS] Priority factors computed correctly");
    }

    #[test]
    fn test_candidate_sorting_by_category() {
        let mut candidates = vec![
            make_candidate(0.9, 2.0, InjectionCategory::SingleSpaceMatch),
            make_candidate(0.8, 0.5, InjectionCategory::DivergenceAlert),
            make_candidate(0.7, 1.0, InjectionCategory::RecentSession),
            make_candidate(0.85, 3.0, InjectionCategory::HighRelevanceCluster),
        ];

        candidates.sort();

        assert_eq!(candidates[0].category, InjectionCategory::DivergenceAlert);
        assert_eq!(candidates[1].category, InjectionCategory::HighRelevanceCluster);
        assert_eq!(candidates[2].category, InjectionCategory::SingleSpaceMatch);
        assert_eq!(candidates[3].category, InjectionCategory::RecentSession);
        println!("[PASS] Candidates sort by category first");
    }

    #[test]
    fn test_candidate_sorting_within_category() {
        let mut candidates = vec![
            make_candidate(0.7, 3.0, InjectionCategory::HighRelevanceCluster),
            make_candidate(0.9, 3.5, InjectionCategory::HighRelevanceCluster),
            make_candidate(0.8, 4.0, InjectionCategory::HighRelevanceCluster),
        ];

        // Set priority factors so priority = relevance (recency=1.0, diversity=1.0)
        for c in &mut candidates {
            c.set_priority_factors(1.0, 1.0);
        }

        candidates.sort();

        // Within same category, sorted by priority descending
        assert!((candidates[0].priority - 0.9).abs() < 0.001);
        assert!((candidates[1].priority - 0.8).abs() < 0.001);
        assert!((candidates[2].priority - 0.7).abs() < 0.001);
        println!("[PASS] Within category, sort by priority descending");
    }

    #[test]
    fn test_fits_budget() {
        let c = InjectionCandidate::new(
            Uuid::new_v4(),
            "word ".repeat(100).trim().to_string(), // 100 words → 130 tokens
            0.5,
            2.0,
            vec![],
            InjectionCategory::SingleSpaceMatch,
            Utc::now(),
        );

        assert!(c.fits_budget(200));
        assert!(c.fits_budget(130));
        assert!(!c.fits_budget(100));
        println!("[PASS] fits_budget check works");
    }

    // =========================================================================
    // Validation Tests (Edge Cases)
    // =========================================================================

    #[test]
    #[should_panic(expected = "relevance_score must be 0.0..=1.0")]
    fn test_invalid_relevance_too_high() {
        make_candidate(1.5, 2.0, InjectionCategory::SingleSpaceMatch);
    }

    #[test]
    #[should_panic(expected = "relevance_score must be 0.0..=1.0")]
    fn test_invalid_relevance_negative() {
        make_candidate(-0.1, 2.0, InjectionCategory::SingleSpaceMatch);
    }

    #[test]
    #[should_panic(expected = "weighted_agreement must be 0.0..=8.5")]
    fn test_invalid_weighted_agreement_too_high() {
        make_candidate(0.5, 9.0, InjectionCategory::SingleSpaceMatch);
    }

    #[test]
    #[should_panic(expected = "relevance_score cannot be NaN")]
    fn test_nan_relevance() {
        make_candidate(f32::NAN, 2.0, InjectionCategory::SingleSpaceMatch);
    }

    #[test]
    #[should_panic(expected = "weighted_agreement cannot be NaN")]
    fn test_nan_weighted_agreement() {
        make_candidate(0.5, f32::NAN, InjectionCategory::SingleSpaceMatch);
    }

    #[test]
    #[should_panic(expected = "recency must be")]
    fn test_invalid_recency_factor() {
        let mut c = make_candidate(0.5, 2.0, InjectionCategory::SingleSpaceMatch);
        c.set_priority_factors(2.0, 1.0); // Invalid: > 1.3
    }

    #[test]
    #[should_panic(expected = "diversity must be")]
    fn test_invalid_diversity_bonus() {
        let mut c = make_candidate(0.5, 2.0, InjectionCategory::SingleSpaceMatch);
        c.set_priority_factors(1.0, 2.0); // Invalid: > 1.5
    }

    // =========================================================================
    // Boundary Tests
    // =========================================================================

    #[test]
    fn test_boundary_values() {
        // Minimum valid values
        let c1 = make_candidate(0.0, 0.0, InjectionCategory::SingleSpaceMatch);
        assert_eq!(c1.relevance_score, 0.0);
        assert_eq!(c1.weighted_agreement, 0.0);

        // Maximum valid values
        let c2 = make_candidate(1.0, 8.5, InjectionCategory::HighRelevanceCluster);
        assert_eq!(c2.relevance_score, 1.0);
        assert!((c2.weighted_agreement - 8.5).abs() < f32::EPSILON);

        // Boundary for recency/diversity
        let mut c3 = make_candidate(0.5, 2.0, InjectionCategory::SingleSpaceMatch);
        c3.set_priority_factors(0.8, 0.8);
        assert!((c3.recency_factor - 0.8).abs() < f32::EPSILON);

        c3.set_priority_factors(1.3, 1.5);
        assert!((c3.recency_factor - 1.3).abs() < f32::EPSILON);
        assert!((c3.diversity_bonus - 1.5).abs() < f32::EPSILON);

        println!("[PASS] All boundary values accepted");
    }

    // =========================================================================
    // Serialization Tests
    // =========================================================================

    #[test]
    fn test_category_serialization() {
        for cat in InjectionCategory::all() {
            let json = serde_json::to_string(&cat).expect("serialize");
            let restored: InjectionCategory = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(cat, restored);
        }
        println!("[PASS] InjectionCategory serialization roundtrip");
    }

    #[test]
    fn test_candidate_serialization() {
        let c = make_candidate(0.8, 3.0, InjectionCategory::HighRelevanceCluster);
        let json = serde_json::to_string(&c).expect("serialize");
        let restored: InjectionCandidate = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(c.memory_id, restored.memory_id);
        assert!((c.relevance_score - restored.relevance_score).abs() < f32::EPSILON);
        assert_eq!(c.category, restored.category);
        println!("[PASS] InjectionCandidate serialization roundtrip");
    }

    // =========================================================================
    // semantic_space_count Tests
    // =========================================================================

    #[test]
    fn test_semantic_space_count_excludes_temporal() {
        let c = InjectionCandidate::new(
            Uuid::new_v4(),
            "test".to_string(),
            0.5,
            3.0,
            vec![
                Embedder::Semantic,       // Semantic - counts
                Embedder::Code,           // Semantic - counts
                Embedder::TemporalRecent, // Temporal - excluded
                Embedder::TemporalPeriodic, // Temporal - excluded
                Embedder::Graph,      // Relational - counts
            ],
            InjectionCategory::HighRelevanceCluster,
            Utc::now(),
        );

        // Only Semantic, Code, Graph should count (3)
        assert_eq!(c.semantic_space_count(), 3);
        println!("[PASS] semantic_space_count excludes temporal embedders");
    }

    #[test]
    fn test_semantic_space_count_all_temporal() {
        let c = InjectionCandidate::new(
            Uuid::new_v4(),
            "test".to_string(),
            0.5,
            0.0,
            vec![
                Embedder::TemporalRecent,
                Embedder::TemporalPeriodic,
                Embedder::TemporalPositional,
            ],
            InjectionCategory::SingleSpaceMatch,
            Utc::now(),
        );

        // All temporal - none count
        assert_eq!(c.semantic_space_count(), 0);
        println!("[PASS] All temporal embedders result in count 0");
    }

    // =========================================================================
    // Empty content test
    // =========================================================================

    #[test]
    fn test_empty_content() {
        let c = InjectionCandidate::new(
            Uuid::new_v4(),
            "".to_string(), // Empty
            0.5,
            2.0,
            vec![],
            InjectionCategory::SingleSpaceMatch,
            Utc::now(),
        );
        // Expected: token_count = 0 (0 words × 1.3 = 0)
        assert_eq!(c.token_count, 0);
        println!("[PASS] Empty content results in token_count = 0");
    }

    // =========================================================================
    // Display tests
    // =========================================================================

    #[test]
    fn test_category_display() {
        assert_eq!(
            format!("{}", InjectionCategory::DivergenceAlert),
            "DivergenceAlert"
        );
        assert_eq!(
            format!("{}", InjectionCategory::HighRelevanceCluster),
            "HighRelevanceCluster"
        );
        assert_eq!(
            format!("{}", InjectionCategory::SingleSpaceMatch),
            "SingleSpaceMatch"
        );
        assert_eq!(
            format!("{}", InjectionCategory::RecentSession),
            "RecentSession"
        );
        println!("[PASS] Display trait works for InjectionCategory");
    }
}

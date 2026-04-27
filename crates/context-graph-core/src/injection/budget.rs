//! Token budget allocation for context injection.
//!
//! Provides the [`TokenBudget`] struct for tracking token allocations
//! across injection categories, and [`TokenBudgetManager`] for selecting
//! candidates within budget constraints.
//!
//! # Constitution Compliance
//! - Total budget: 1200 tokens (per injection.priorities)
//! - Category allocations sum to total
//! - High-priority categories (DivergenceAlert, HighRelevanceCluster) can overflow

use std::collections::HashMap;

use super::candidate::{InjectionCandidate, InjectionCategory};

/// Token budget allocation for context injection.
/// Each category has a dedicated budget pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenBudget {
    /// Total token limit for all injected context
    pub total: u32,
    /// Budget for divergence alerts (highest priority)
    pub divergence_budget: u32,
    /// Budget for high-relevance cluster matches (weighted_agreement >= 2.5)
    pub cluster_budget: u32,
    /// Budget for single-space matches (weighted_agreement in [1.0, 2.5))
    pub single_space_budget: u32,
    /// Budget for last session summary
    pub session_budget: u32,
    /// Reserved for formatting overhead
    pub reserved: u32,
}

/// Default budget allocation for SessionStart/UserPromptSubmit hooks.
/// Total: 1200 tokens per constitution.yaml.
pub const DEFAULT_TOKEN_BUDGET: TokenBudget = TokenBudget {
    total: 1200,
    divergence_budget: 200,
    cluster_budget: 400,
    single_space_budget: 300,
    session_budget: 200,
    reserved: 100,
};

/// Brief budget for PreToolUse hook.
/// Only 200 tokens for quick context per constitution.yaml.
pub const BRIEF_BUDGET: u32 = 200;

/// Minimum total budget required for meaningful allocation.
pub const MIN_BUDGET: u32 = 100;

/// Error returned when budget is too small.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetTooSmall {
    /// The budget that was provided.
    pub provided: u32,
    /// The minimum required.
    pub minimum: u32,
}

impl std::fmt::Display for BudgetTooSmall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "budget too small: {} provided, minimum is {}",
            self.provided, self.minimum
        )
    }
}

impl std::error::Error for BudgetTooSmall {}

impl TokenBudget {
    /// Create custom budget with specified total.
    /// Allocates proportionally based on default ratios.
    ///
    /// # Arguments
    /// * `total` - Total token budget (must be >= MIN_BUDGET)
    ///
    /// # Errors
    /// Returns `BudgetTooSmall` if `total < MIN_BUDGET` (insufficient for meaningful allocation)
    ///
    /// # Constitution Compliance
    /// - AP-14: Returns Result instead of panicking
    pub fn with_total(total: u32) -> Result<Self, BudgetTooSmall> {
        if total < MIN_BUDGET {
            return Err(BudgetTooSmall {
                provided: total,
                minimum: MIN_BUDGET,
            });
        }

        // Ratios from DEFAULT_TOKEN_BUDGET:
        // divergence: 200/1200 = 1/6
        // cluster: 400/1200 = 1/3
        // single_space: 300/1200 = 1/4
        // session: 200/1200 = 1/6
        // reserved: remainder (absorbs integer division rounding)
        let divergence_budget = total / 6;
        let cluster_budget = total / 3;
        let single_space_budget = total / 4;
        let session_budget = total / 6;
        let reserved =
            total - divergence_budget - cluster_budget - single_space_budget - session_budget;

        Ok(Self {
            total,
            divergence_budget,
            cluster_budget,
            single_space_budget,
            session_budget,
            reserved,
        })
    }

    /// Get budget allocation for a specific category.
    #[inline]
    pub fn budget_for_category(&self, category: InjectionCategory) -> u32 {
        match category {
            InjectionCategory::DivergenceAlert => self.divergence_budget,
            InjectionCategory::HighRelevanceCluster => self.cluster_budget,
            InjectionCategory::SingleSpaceMatch => self.single_space_budget,
            InjectionCategory::RecentSession => self.session_budget,
        }
    }

    /// Usable budget (total minus reserved).
    #[inline]
    pub fn usable(&self) -> u32 {
        self.total.saturating_sub(self.reserved)
    }

    /// Validate that budgets sum correctly.
    pub fn is_valid(&self) -> bool {
        let sum = self.divergence_budget
            + self.cluster_budget
            + self.single_space_budget
            + self.session_budget
            + self.reserved;
        sum == self.total
    }
}

impl Default for TokenBudget {
    fn default() -> Self {
        DEFAULT_TOKEN_BUDGET
    }
}

/// Token estimation multiplier (words to tokens).
/// Must match [`super::candidate::TOKEN_MULTIPLIER`].
const TOKEN_MULTIPLIER: f32 = 1.3;

/// Estimate token count for content.
///
/// Uses word_count × 1.3, matching [`InjectionCandidate`]'s token estimation.
/// Exposed as standalone function for pre-candidate estimation.
#[inline]
pub fn estimate_tokens(content: &str) -> u32 {
    let word_count = content.split_whitespace().count();
    (word_count as f32 * TOKEN_MULTIPLIER).ceil() as u32
}

/// Tracks tokens spent per injection category during selection.
#[derive(Debug, Clone, Default)]
struct CategorySpending {
    divergence: u32,
    cluster: u32,
    single_space: u32,
    session: u32,
}

impl CategorySpending {
    /// Get current spending for a category.
    #[inline]
    fn get(&self, category: InjectionCategory) -> u32 {
        match category {
            InjectionCategory::DivergenceAlert => self.divergence,
            InjectionCategory::HighRelevanceCluster => self.cluster,
            InjectionCategory::SingleSpaceMatch => self.single_space,
            InjectionCategory::RecentSession => self.session,
        }
    }

    /// Add tokens to a category's spending.
    #[inline]
    fn add(&mut self, category: InjectionCategory, amount: u32) {
        let field = match category {
            InjectionCategory::DivergenceAlert => &mut self.divergence,
            InjectionCategory::HighRelevanceCluster => &mut self.cluster,
            InjectionCategory::SingleSpaceMatch => &mut self.single_space,
            InjectionCategory::RecentSession => &mut self.session,
        };
        *field += amount;
    }

    /// Total tokens spent across all categories.
    #[inline]
    fn total(&self) -> u32 {
        self.divergence + self.cluster + self.single_space + self.session
    }
}

/// Statistics about the selection process.
#[derive(Debug, Clone)]
pub struct SelectionStats {
    /// Number of candidates selected.
    pub selected_count: usize,
    /// Number of candidates rejected (exceeded budget).
    pub rejected_count: usize,
    /// Total tokens used by selected candidates.
    pub tokens_used: u32,
    /// Remaining tokens available.
    pub tokens_available: u32,
    /// Tokens used per category.
    pub by_category: HashMap<InjectionCategory, u32>,
}

/// Manages token budget allocation during candidate selection.
///
/// Selects candidates that fit within category and total budget constraints.
/// High-priority categories (priority <= 2) can overflow into reserved budget.
///
/// # Usage
///
/// ```ignore
/// // After PriorityRanker::rank_candidates() has sorted candidates:
/// let selected = TokenBudgetManager::select_within_budget(&candidates, &budget);
/// ```
///
/// # Overflow Rules
///
/// | Category | Priority | Can Overflow? |
/// |----------|----------|---------------|
/// | DivergenceAlert | 1 | Yes |
/// | HighRelevanceCluster | 2 | Yes |
/// | SingleSpaceMatch | 3 | No |
/// | RecentSession | 4 | No |
pub struct TokenBudgetManager;

impl TokenBudgetManager {
    /// Select candidates that fit within budget constraints.
    ///
    /// **IMPORTANT**: Assumes candidates are pre-sorted by `PriorityRanker::rank_candidates()`.
    /// Processes in order, selecting candidates until category budget is exhausted.
    /// High-priority categories (DivergenceAlert, HighRelevanceCluster) can overflow
    /// into reserved budget.
    ///
    /// # Arguments
    /// * `candidates` - Sorted slice of injection candidates
    /// * `budget` - Token budget configuration
    ///
    /// # Returns
    /// Vector of selected candidates (cloned) in selection order
    ///
    /// # Invariants
    /// - Total selected tokens will NEVER exceed `budget.total`
    /// - Low-priority categories (priority >= 3) cannot exceed their allocated budget
    pub fn select_within_budget(
        candidates: &[InjectionCandidate],
        budget: &TokenBudget,
    ) -> Vec<InjectionCandidate> {
        let mut selected = Vec::new();
        let mut spending = CategorySpending::default();

        for candidate in candidates {
            let tokens = candidate.token_count;
            let category = candidate.category;

            if Self::can_select(&spending, budget, category, tokens) {
                spending.add(category, tokens);
                selected.push(candidate.clone());
            }
        }

        selected
    }

    /// Select candidates with detailed statistics.
    ///
    /// Same as `select_within_budget` but also returns selection statistics.
    ///
    /// # Returns
    /// Tuple of (selected candidates, selection statistics)
    pub fn select_with_stats(
        candidates: &[InjectionCandidate],
        budget: &TokenBudget,
    ) -> (Vec<InjectionCandidate>, SelectionStats) {
        let mut selected = Vec::new();
        let mut spending = CategorySpending::default();
        let mut rejected_count = 0;

        for candidate in candidates {
            let tokens = candidate.token_count;
            let category = candidate.category;

            if Self::can_select(&spending, budget, category, tokens) {
                spending.add(category, tokens);
                selected.push(candidate.clone());
            } else {
                rejected_count += 1;
            }
        }

        let stats = SelectionStats {
            selected_count: selected.len(),
            rejected_count,
            tokens_used: spending.total(),
            tokens_available: budget.total.saturating_sub(spending.total()),
            by_category: HashMap::from([
                (InjectionCategory::DivergenceAlert, spending.divergence),
                (InjectionCategory::HighRelevanceCluster, spending.cluster),
                (InjectionCategory::SingleSpaceMatch, spending.single_space),
                (InjectionCategory::RecentSession, spending.session),
            ]),
        };

        (selected, stats)
    }

    /// Check if a candidate can be selected given current spending.
    ///
    /// Rules:
    /// 1. Never exceed total budget (hard limit)
    /// 2. Within category budget: always allow
    /// 3. Over category budget + high-priority (<=2): allow overflow into total
    /// 4. Over category budget + low-priority (>=3): reject
    fn can_select(
        spending: &CategorySpending,
        budget: &TokenBudget,
        category: InjectionCategory,
        tokens: u32,
    ) -> bool {
        let total_spent = spending.total();
        let would_exceed_total = total_spent + tokens > budget.total;

        // Rule 1: Total budget is a hard limit
        if would_exceed_total {
            return false;
        }

        let category_spent = spending.get(category);
        let category_budget = budget.budget_for_category(category);
        let within_category = category_spent + tokens <= category_budget;

        // Rule 2: Within category budget is always allowed
        if within_category {
            return true;
        }

        // Rules 3 & 4: Only high-priority (<=2) can overflow
        // DivergenceAlert=1, HighRelevanceCluster=2 can overflow
        // SingleSpaceMatch=3, RecentSession=4 cannot
        category.priority() <= 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_budget_values() {
        let budget = TokenBudget::default();

        assert_eq!(budget.total, 1200, "Total should be 1200 per constitution");
        assert_eq!(
            budget.divergence_budget, 200,
            "Divergence budget should be 200"
        );
        assert_eq!(budget.cluster_budget, 400, "Cluster budget should be 400");
        assert_eq!(
            budget.single_space_budget, 300,
            "Single space budget should be 300"
        );
        assert_eq!(budget.session_budget, 200, "Session budget should be 200");
        assert_eq!(budget.reserved, 100, "Reserved should be 100");
        println!("[PASS] Default budget values match constitution");
    }

    #[test]
    fn test_default_budget_sums_correctly() {
        let budget = TokenBudget::default();

        let sum = budget.divergence_budget
            + budget.cluster_budget
            + budget.single_space_budget
            + budget.session_budget
            + budget.reserved;

        assert_eq!(sum, budget.total, "Budgets must sum to total");
        assert!(budget.is_valid(), "is_valid() must return true");
        println!("[PASS] Budget sums correctly: {} = {}", sum, budget.total);
    }

    #[test]
    fn test_budget_for_category() {
        let budget = TokenBudget::default();

        assert_eq!(
            budget.budget_for_category(InjectionCategory::DivergenceAlert),
            200,
            "DivergenceAlert budget"
        );
        assert_eq!(
            budget.budget_for_category(InjectionCategory::HighRelevanceCluster),
            400,
            "HighRelevanceCluster budget"
        );
        assert_eq!(
            budget.budget_for_category(InjectionCategory::SingleSpaceMatch),
            300,
            "SingleSpaceMatch budget"
        );
        assert_eq!(
            budget.budget_for_category(InjectionCategory::RecentSession),
            200,
            "RecentSession budget"
        );
        println!("[PASS] budget_for_category returns correct values");
    }

    #[test]
    fn test_with_total_proportional() {
        let budget = TokenBudget::with_total(600).expect("600 >= MIN_BUDGET");

        assert_eq!(budget.total, 600);
        assert!(budget.is_valid(), "Custom budget must be valid");

        // Verify proportions roughly maintained
        assert!(
            budget.cluster_budget >= budget.divergence_budget,
            "Cluster should be >= divergence"
        );
        assert!(
            budget.cluster_budget >= budget.single_space_budget,
            "Cluster should be >= single_space"
        );

        println!("[PASS] with_total(600) creates valid budget: {:?}", budget);
    }

    #[test]
    fn test_with_total_minimum() {
        let budget = TokenBudget::with_total(100).expect("100 == MIN_BUDGET");
        assert_eq!(budget.total, 100);
        assert!(budget.is_valid());
        println!("[PASS] Minimum total (100) accepted");
    }

    #[test]
    fn test_with_total_below_minimum() {
        // AP-14: Returns Result instead of panicking
        let result = TokenBudget::with_total(50);
        assert!(result.is_err(), "50 < MIN_BUDGET should error");
        let err = result.unwrap_err();
        assert_eq!(err.provided, 50);
        assert_eq!(err.minimum, MIN_BUDGET);
        println!("[PASS] Below minimum (50) returns BudgetTooSmall error");
    }

    #[test]
    fn test_usable_budget() {
        let budget = TokenBudget::default();
        // 1200 - 100 reserved = 1100 usable
        assert_eq!(budget.usable(), 1100, "Usable should be total - reserved");
        println!("[PASS] usable() returns {} (1200 - 100)", budget.usable());
    }

    #[test]
    fn test_brief_budget_constant() {
        assert_eq!(
            BRIEF_BUDGET, 200,
            "BRIEF_BUDGET should be 200 for PreToolUse"
        );
        println!("[PASS] BRIEF_BUDGET = 200");
    }

    #[test]
    fn test_default_equals_constant() {
        let budget = TokenBudget::default();
        assert_eq!(
            budget, DEFAULT_TOKEN_BUDGET,
            "Default should equal constant"
        );
        println!("[PASS] TokenBudget::default() == DEFAULT_TOKEN_BUDGET");
    }

    #[test]
    fn test_invalid_budget_detection() {
        let invalid = TokenBudget {
            total: 1200,
            divergence_budget: 0,
            cluster_budget: 0,
            single_space_budget: 0,
            session_budget: 0,
            reserved: 0,
        };
        assert!(!invalid.is_valid(), "Zero allocations should be invalid");
        println!("[PASS] Invalid budget (zero allocations) detected correctly");
    }

    #[test]
    fn test_zero_reserved_is_valid() {
        let budget = TokenBudget {
            total: 1100,
            divergence_budget: 200,
            cluster_budget: 400,
            single_space_budget: 300,
            session_budget: 200,
            reserved: 0,
        };
        assert!(
            budget.is_valid(),
            "Zero reserved should be valid if sums match"
        );
        assert_eq!(
            budget.usable(),
            1100,
            "Usable should equal total when reserved is 0"
        );
        println!("[PASS] Zero reserved budget is valid");
    }

    #[test]
    fn test_large_total() {
        let budget = TokenBudget::with_total(1_000_000).expect("large budget");
        assert!(budget.is_valid());
        assert_eq!(budget.total, 1_000_000);
        println!(
            "[PASS] Large total (1_000_000) accepted and valid: {:?}",
            budget
        );
    }

    #[test]
    fn test_minimum_total_has_positive_allocations() {
        let b = TokenBudget::with_total(100).expect("100 == MIN_BUDGET");
        assert!(b.divergence_budget > 0, "divergence_budget should be > 0");
        assert!(b.cluster_budget > 0, "cluster_budget should be > 0");
        assert!(b.is_valid());
        println!(
            "[PASS] Minimum total has positive allocations: div={}, cluster={}, single={}, session={}, reserved={}",
            b.divergence_budget, b.cluster_budget, b.single_space_budget, b.session_budget, b.reserved
        );
    }
}

#[cfg(test)]
mod budget_manager_tests {
    use super::*;
    use crate::teleological::Embedder;
    use chrono::Utc;
    use uuid::Uuid;

    fn make_candidate(
        tokens: u32,
        category: InjectionCategory,
        priority: f32,
    ) -> InjectionCandidate {
        let word_count = ((tokens as f32) / 1.3).ceil() as usize;
        let content = "word ".repeat(word_count);
        let mut c = InjectionCandidate::new(
            Uuid::new_v4(),
            content,
            0.8, // relevance
            match category {
                InjectionCategory::HighRelevanceCluster => 3.0,
                InjectionCategory::SingleSpaceMatch => 1.5,
                _ => 0.5,
            },
            vec![Embedder::Semantic],
            category,
            Utc::now(),
        );
        c.token_count = tokens; // Override to exact value for testing
        c.priority = priority;
        c
    }

    #[test]
    fn test_select_within_category_budget() {
        let budget = TokenBudget::default(); // divergence=200, cluster=400

        let candidates = vec![
            make_candidate(150, InjectionCategory::DivergenceAlert, 1.0),
            make_candidate(100, InjectionCategory::DivergenceAlert, 0.9),
            // 150 + 100 = 250, exceeds divergence budget (200), but overflow allowed
        ];

        let selected = TokenBudgetManager::select_within_budget(&candidates, &budget);

        // Both should be selected (overflow allowed for high priority)
        assert_eq!(
            selected.len(),
            2,
            "Both divergence alerts should be selected (overflow allowed)"
        );
        println!(
            "[PASS] High-priority overflow allowed: selected {} candidates",
            selected.len()
        );
    }

    #[test]
    fn test_select_respects_total_budget() {
        // Use explicit budget where each category has 100 token budget
        let budget = TokenBudget {
            total: 400,
            divergence_budget: 100,
            cluster_budget: 100,
            single_space_budget: 100,
            session_budget: 50,
            reserved: 50,
        };

        let candidates = vec![
            make_candidate(100, InjectionCategory::DivergenceAlert, 1.0),
            make_candidate(100, InjectionCategory::HighRelevanceCluster, 0.9),
            make_candidate(100, InjectionCategory::SingleSpaceMatch, 0.8),
            make_candidate(100, InjectionCategory::RecentSession, 0.7), // Would exceed session_budget (50) and total (400)
        ];

        let selected = TokenBudgetManager::select_within_budget(&candidates, &budget);

        // Should select first 3 (each fits in their category), 4th exceeds session_budget and low priority
        assert_eq!(
            selected.len(),
            3,
            "Should select exactly 3 candidates (4th exceeds session budget)"
        );

        let total_tokens: u32 = selected.iter().map(|c| c.token_count).sum();
        assert!(
            total_tokens <= 400,
            "Total tokens {} should not exceed 400",
            total_tokens
        );
        println!(
            "[PASS] Total budget respected: {} tokens used",
            total_tokens
        );
    }

    #[test]
    fn test_high_priority_overflow_into_reserved() {
        let budget = TokenBudget {
            total: 500,
            divergence_budget: 100,
            cluster_budget: 200,
            single_space_budget: 100,
            session_budget: 50,
            reserved: 50,
        };

        // Large divergence alert that exceeds category budget (100) but fits in total (500)
        let candidates = vec![make_candidate(150, InjectionCategory::DivergenceAlert, 1.0)];

        let selected = TokenBudgetManager::select_within_budget(&candidates, &budget);

        // Should allow overflow for high priority (priority=1)
        assert_eq!(
            selected.len(),
            1,
            "High-priority candidate should overflow into reserved"
        );
        println!("[PASS] High-priority overflow works");
    }

    #[test]
    fn test_low_priority_no_overflow() {
        let budget = TokenBudget {
            total: 500,
            divergence_budget: 100,
            cluster_budget: 100,
            single_space_budget: 100,
            session_budget: 100,
            reserved: 100,
        };

        // RecentSession (priority=4) exceeds category budget
        let candidates = vec![make_candidate(150, InjectionCategory::RecentSession, 1.0)];

        let selected = TokenBudgetManager::select_within_budget(&candidates, &budget);

        // Should NOT overflow for low priority (priority=4)
        assert_eq!(
            selected.len(),
            0,
            "Low-priority candidate should NOT overflow"
        );
        println!("[PASS] Low-priority overflow blocked correctly");
    }

    #[test]
    fn test_select_with_stats() {
        let budget = TokenBudget::default();

        let candidates = vec![
            make_candidate(100, InjectionCategory::DivergenceAlert, 1.0),
            make_candidate(200, InjectionCategory::HighRelevanceCluster, 0.9),
            make_candidate(100, InjectionCategory::SingleSpaceMatch, 0.8),
        ];

        let (selected, stats) = TokenBudgetManager::select_with_stats(&candidates, &budget);

        assert_eq!(selected.len(), 3);
        assert_eq!(stats.selected_count, 3);
        assert_eq!(stats.rejected_count, 0);
        assert_eq!(stats.tokens_used, 400);
        assert_eq!(stats.tokens_available, 800); // 1200 - 400
        assert_eq!(
            *stats
                .by_category
                .get(&InjectionCategory::DivergenceAlert)
                .unwrap(),
            100
        );
        assert_eq!(
            *stats
                .by_category
                .get(&InjectionCategory::HighRelevanceCluster)
                .unwrap(),
            200
        );
        println!(
            "[PASS] Selection stats accurate: {} used, {} available",
            stats.tokens_used, stats.tokens_available
        );
    }

    #[test]
    fn test_estimate_tokens() {
        // 10 words × 1.3 = 13
        assert_eq!(
            estimate_tokens("one two three four five six seven eight nine ten"),
            13
        );

        // Empty string
        assert_eq!(estimate_tokens(""), 0);

        // Single word → 1 × 1.3 = 1.3 → ceil = 2
        assert_eq!(estimate_tokens("hello"), 2);

        // 5 words × 1.3 = 6.5 → ceil = 7
        assert_eq!(estimate_tokens("the quick brown fox jumps"), 7);

        println!("[PASS] estimate_tokens formula verified");
    }

    #[test]
    fn test_mixed_categories_selection() {
        let budget = TokenBudget::default(); // 1200 total

        let candidates = vec![
            make_candidate(50, InjectionCategory::DivergenceAlert, 1.0),
            make_candidate(100, InjectionCategory::HighRelevanceCluster, 0.95),
            make_candidate(100, InjectionCategory::HighRelevanceCluster, 0.9),
            make_candidate(100, InjectionCategory::HighRelevanceCluster, 0.85),
            make_candidate(100, InjectionCategory::SingleSpaceMatch, 0.8),
            make_candidate(100, InjectionCategory::SingleSpaceMatch, 0.75),
            make_candidate(50, InjectionCategory::RecentSession, 0.7),
        ];

        let selected = TokenBudgetManager::select_within_budget(&candidates, &budget);

        // All should fit: 50+100+100+100+100+100+50 = 600 < 1200
        assert_eq!(
            selected.len(),
            7,
            "All 7 candidates should fit within budget"
        );

        let total: u32 = selected.iter().map(|c| c.token_count).sum();
        assert_eq!(total, 600);
        println!(
            "[PASS] Mixed categories: all 7 selected, {} total tokens",
            total
        );
    }

    #[test]
    fn test_empty_candidates() {
        let budget = TokenBudget::default();
        let candidates: Vec<InjectionCandidate> = vec![];

        let selected = TokenBudgetManager::select_within_budget(&candidates, &budget);

        assert!(selected.is_empty());
        println!("[PASS] Empty candidates handled correctly");
    }

    #[test]
    fn test_single_candidate_fits() {
        let budget = TokenBudget::default();
        let candidates = vec![make_candidate(
            100,
            InjectionCategory::HighRelevanceCluster,
            1.0,
        )];

        let selected = TokenBudgetManager::select_within_budget(&candidates, &budget);

        assert_eq!(selected.len(), 1);
        println!("[PASS] Single candidate selected");
    }

    #[test]
    fn test_single_candidate_too_large() {
        let budget = TokenBudget::with_total(100).expect("100 == MIN_BUDGET");
        let candidates = vec![make_candidate(
            150,
            InjectionCategory::HighRelevanceCluster,
            1.0,
        )];

        let selected = TokenBudgetManager::select_within_budget(&candidates, &budget);

        assert!(
            selected.is_empty(),
            "150-token candidate should not fit in 100-token budget"
        );
        println!("[PASS] Oversized candidate rejected");
    }

    #[test]
    fn test_edge_case_exactly_at_category_budget() {
        let budget = TokenBudget::default(); // divergence_budget = 200
        let candidates = vec![make_candidate(200, InjectionCategory::DivergenceAlert, 1.0)];

        let selected = TokenBudgetManager::select_within_budget(&candidates, &budget);

        assert_eq!(
            selected.len(),
            1,
            "Candidate exactly at category budget should be selected"
        );
    }

    #[test]
    fn test_edge_case_one_token_over_category_budget_low_priority() {
        let budget = TokenBudget {
            total: 500,
            divergence_budget: 100,
            cluster_budget: 100,
            single_space_budget: 100,
            session_budget: 100,
            reserved: 100,
        };
        // RecentSession (priority=4) is 1 token over its category budget
        let candidates = vec![make_candidate(101, InjectionCategory::RecentSession, 1.0)];

        let selected = TokenBudgetManager::select_within_budget(&candidates, &budget);

        assert_eq!(
            selected.len(),
            0,
            "Low priority 1 token over should be rejected"
        );
    }

    #[test]
    fn test_edge_case_total_budget_exactly_exhausted() {
        let budget = TokenBudget {
            total: 300,
            divergence_budget: 100,
            cluster_budget: 100,
            single_space_budget: 100,
            session_budget: 0,
            reserved: 0,
        };
        let candidates = vec![
            make_candidate(100, InjectionCategory::DivergenceAlert, 1.0),
            make_candidate(100, InjectionCategory::HighRelevanceCluster, 0.9),
            make_candidate(100, InjectionCategory::SingleSpaceMatch, 0.8),
        ];

        let (selected, stats) = TokenBudgetManager::select_with_stats(&candidates, &budget);

        assert_eq!(selected.len(), 3);
        assert_eq!(stats.tokens_used, 300);
        assert_eq!(stats.tokens_available, 0);
    }

    #[test]
    fn test_total_budget_invariant_never_exceeded() {
        // Generate various budget and candidate combinations
        let budgets = vec![
            TokenBudget::with_total(100).expect("100 == MIN_BUDGET"),
            TokenBudget::with_total(500).expect("500 >= MIN_BUDGET"),
            TokenBudget::default(),
        ];

        for budget in &budgets {
            // Create candidates that could potentially exceed budget
            let candidates = vec![
                make_candidate(budget.total / 2, InjectionCategory::DivergenceAlert, 1.0),
                make_candidate(
                    budget.total / 2,
                    InjectionCategory::HighRelevanceCluster,
                    0.9,
                ),
                make_candidate(budget.total / 2, InjectionCategory::SingleSpaceMatch, 0.8),
                make_candidate(budget.total / 2, InjectionCategory::RecentSession, 0.7),
            ];

            let selected = TokenBudgetManager::select_within_budget(&candidates, budget);
            let total_tokens: u32 = selected.iter().map(|c| c.token_count).sum();

            assert!(
                total_tokens <= budget.total,
                "INVARIANT VIOLATED: selected {} tokens but budget is {}",
                total_tokens,
                budget.total
            );
        }
        println!("[PASS] Total budget invariant verified across multiple budgets");
    }
}

//! InjectionPipeline orchestrates context injection from retrieval to formatted output.
//!
//! This module provides the main pipeline that combines:
//! - SimilarityRetriever for memory retrieval and divergence detection
//! - PriorityRanker for scoring and ranking candidates
//! - TokenBudgetManager for budget-aware selection
//! - ContextFormatter for markdown output
//!
//! # Architecture
//!
//! ```text
//! InjectionPipeline
//! ├── SimilarityRetriever  - Find similar memories, detect divergence
//! ├── MemoryStore         - Lookup memory content by ID
//! ├── PriorityRanker      - Compute recency/diversity factors
//! ├── TokenBudgetManager  - Select within budget
//! └── ContextFormatter    - Format for injection
//! ```
//!
//! # Constitution Compliance
//!
//! - ARCH-09: Topic threshold is weighted_agreement >= 2.5
//! - ARCH-10: Divergence detection uses SEMANTIC embedders only
//! - AP-10: No NaN/Infinity in scores
//! - AP-14: No .unwrap() in library code
//! - AP-60: Temporal embedders NEVER count toward topic detection
//! - AP-62: Divergence alerts MUST only use SEMANTIC embedders

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use thiserror::Error;
use tracing::{debug, info};
use uuid::Uuid;

use super::budget::{estimate_tokens, SelectionStats, TokenBudget, TokenBudgetManager, BRIEF_BUDGET};
use super::candidate::{InjectionCandidate, InjectionCategory};
use super::formatter::ContextFormatter;
use super::priority::PriorityRanker;
use super::result::InjectionResult;
use crate::memory::{MemoryStore, StorageError};
use crate::retrieval::{RetrieverError, SimilarityResult, SimilarityRetriever};
use crate::types::fingerprint::SemanticFingerprint;

// =============================================================================
// Constants
// =============================================================================

/// Default limit for similar memory retrieval.
const DEFAULT_RETRIEVAL_LIMIT: usize = 50;

/// Minimum weighted agreement for HighRelevanceCluster category.
const HIGH_RELEVANCE_THRESHOLD: f32 = 2.5;

/// Minimum weighted agreement for SingleSpaceMatch category.
const SINGLE_SPACE_THRESHOLD: f32 = 1.0;

// =============================================================================
// InjectionError
// =============================================================================

/// Errors that can occur during context injection pipeline.
///
/// Each variant includes sufficient context for debugging and logging.
/// Errors are fail-fast - no retries or fallbacks at this layer.
///
/// # Constitution Compliance
/// - rust_standards.error_handling: thiserror for library errors
/// - AP-14: No .unwrap() - errors propagated via Result
#[derive(Debug, Error)]
pub enum InjectionError {
    /// Retrieval operation failed.
    ///
    /// Occurs when SimilarityRetriever cannot fetch similar memories
    /// or check divergence.
    #[error("Retrieval failed: {0}")]
    RetrievalFailed(#[from] RetrieverError),

    /// Storage lookup failed.
    ///
    /// Occurs when MemoryStore cannot fetch a memory by ID.
    #[error("Storage lookup failed: {0}")]
    StorageFailed(#[from] StorageError),

    /// Memory not found in store.
    ///
    /// Occurs when a similarity result references a memory ID
    /// that doesn't exist in storage. Could indicate stale indexes.
    #[error("Memory not found: {0}")]
    MemoryNotFound(Uuid),

    /// Invalid query fingerprint.
    ///
    /// Occurs when the provided SemanticFingerprint fails validation.
    #[error("Invalid query fingerprint: {0}")]
    InvalidFingerprint(String),
}

// =============================================================================
// InjectionPipeline
// =============================================================================

/// Orchestrates the full context injection pipeline.
///
/// Combines retrieval, ranking, selection, and formatting into a single
/// cohesive pipeline. Thread-safe via Arc references.
///
/// # Usage
///
/// ```ignore
/// use context_graph_core::injection::InjectionPipeline;
/// use context_graph_core::retrieval::SimilarityRetriever;
/// use context_graph_core::memory::MemoryStore;
/// use std::sync::Arc;
///
/// let store = Arc::new(MemoryStore::new(path)?);
/// let retriever = SimilarityRetriever::with_defaults(store.clone());
/// let pipeline = InjectionPipeline::new(retriever, store);
///
/// // Generate context for SessionStart hook
/// let result = pipeline.generate_context(&query_fingerprint, &session_id, None)?;
/// println!("{}", result.formatted_context);
///
/// // Generate brief context for PreToolUse hook
/// let brief = pipeline.generate_brief_context(&query_fingerprint, &session_id)?;
/// println!("{}", brief.formatted_context);
/// ```
///
/// # Constitution Compliance
///
/// - ARCH-09: Uses weighted_agreement >= 2.5 for HighRelevanceCluster
/// - AP-60: Temporal embedders excluded from topic detection (via SimilarityRetriever)
/// - AP-62: Divergence alerts only from SEMANTIC embedders (via SimilarityRetriever)
pub struct InjectionPipeline {
    /// Retriever for similarity search and divergence detection.
    retriever: SimilarityRetriever,

    /// Store for memory content lookup.
    store: Arc<MemoryStore>,

    /// Token budget configuration.
    budget: TokenBudget,
}

impl InjectionPipeline {
    /// Create a new InjectionPipeline with default configuration.
    ///
    /// Uses default thresholds and weights from constitution.yaml.
    ///
    /// # Arguments
    /// * `retriever` - SimilarityRetriever for memory retrieval
    /// * `store` - MemoryStore for content lookup
    pub fn new(retriever: SimilarityRetriever, store: Arc<MemoryStore>) -> Self {
        Self {
            retriever,
            store,
            budget: TokenBudget::default(),
        }
    }

    /// Create pipeline with custom budget.
    ///
    /// Useful for testing or different hook contexts.
    pub fn with_budget(
        retriever: SimilarityRetriever,
        store: Arc<MemoryStore>,
        budget: TokenBudget,
    ) -> Self {
        Self {
            retriever,
            store,
            budget,
        }
    }

    /// Generate full context for SessionStart/UserPromptSubmit hooks.
    ///
    /// Executes the complete pipeline:
    /// 1. Retrieve similar memories
    /// 2. Check for divergence
    /// 3. Convert to candidates with scores
    /// 4. Rank by priority
    /// 5. Select within budget
    /// 6. Format as markdown
    ///
    /// # Arguments
    /// * `query` - Query fingerprint for similarity search
    /// * `session_id` - Current session for retrieval context
    /// * `limit` - Optional retrieval limit (default: 50)
    ///
    /// # Returns
    /// `InjectionResult` with formatted context and metadata.
    /// Empty result is valid (no relevant context found).
    ///
    /// # Errors
    /// - `InjectionError::RetrievalFailed` - retrieval operation failed
    /// - `InjectionError::StorageFailed` - memory lookup failed
    /// - `InjectionError::MemoryNotFound` - referenced memory missing
    pub fn generate_context(
        &self,
        query: &SemanticFingerprint,
        session_id: &str,
        limit: Option<usize>,
    ) -> Result<InjectionResult, InjectionError> {
        let limit = limit.unwrap_or(DEFAULT_RETRIEVAL_LIMIT);
        info!(
            session_id = %session_id,
            limit = limit,
            "Generating context injection"
        );

        // Step 1: Retrieve similar memories
        let similar_results = self.retriever.retrieve_similar(query, session_id, limit)?;
        debug!(
            count = similar_results.len(),
            "Retrieved similar memories"
        );

        // Step 2: Check for divergence
        let divergence_alerts = self.retriever.check_divergence(query, session_id)?.alerts;
        debug!(
            alert_count = divergence_alerts.len(),
            "Checked divergence"
        );

        // Step 3: Convert to candidates with content lookup
        let mut candidates = self.convert_to_candidates(&similar_results)?;
        debug!(
            candidate_count = candidates.len(),
            "Converted to candidates"
        );

        // Step 4: Rank candidates (static method)
        PriorityRanker::rank_candidates(&mut candidates);
        debug!("Ranked candidates by priority");

        // Step 5: Select within budget (static method)
        let (selected, stats) = TokenBudgetManager::select_with_stats(&candidates, &self.budget);
        debug!(
            selected = selected.len(),
            tokens_used = stats.tokens_used,
            "Selected within budget"
        );

        // Step 6: Format output
        let formatted = ContextFormatter::format_full_context(&selected, &divergence_alerts);
        let tokens_used = estimate_tokens(&formatted);

        // Collect metadata
        let included_memories: Vec<Uuid> = selected.iter().map(|c| c.memory_id).collect();
        let categories_included: Vec<InjectionCategory> = selected
            .iter()
            .map(|c| c.category)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        let result = InjectionResult::new(
            formatted,
            included_memories,
            divergence_alerts,
            tokens_used,
            categories_included,
        );

        info!(
            memory_count = result.memory_count(),
            tokens = result.tokens_used,
            has_alerts = result.has_divergence_alerts(),
            "Context injection complete"
        );

        Ok(result)
    }

    /// Generate brief context for PreToolUse hook.
    ///
    /// Produces compact single-line output under brief token budget.
    /// Does NOT include divergence alerts (too verbose for brief context).
    ///
    /// # Arguments
    /// * `query` - Query fingerprint for similarity search
    /// * `session_id` - Current session for retrieval context
    ///
    /// # Returns
    /// `InjectionResult` with brief formatted context.
    ///
    /// # Errors
    /// Same as `generate_context`.
    pub fn generate_brief_context(
        &self,
        query: &SemanticFingerprint,
        session_id: &str,
    ) -> Result<InjectionResult, InjectionError> {
        info!(
            session_id = %session_id,
            "Generating brief context injection"
        );

        // Retrieve fewer candidates for brief context
        let similar_results = self
            .retriever
            .retrieve_similar(query, session_id, 10)?;

        // Convert to candidates
        let mut candidates = self.convert_to_candidates(&similar_results)?;

        // Rank candidates (static method)
        PriorityRanker::rank_candidates(&mut candidates);

        // Use brief budget (select top candidates)
        let brief_budget = TokenBudget::with_total(BRIEF_BUDGET);
        let selected = TokenBudgetManager::select_within_budget(&candidates, &brief_budget);

        // Format as brief output
        let formatted = ContextFormatter::format_brief_context(&selected);
        let tokens_used = estimate_tokens(&formatted);

        let included_memories: Vec<Uuid> = selected.iter().map(|c| c.memory_id).collect();
        let categories_included: Vec<InjectionCategory> = selected
            .iter()
            .map(|c| c.category)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        let result = InjectionResult::new(
            formatted,
            included_memories,
            Vec::new(),
            tokens_used,
            categories_included,
        );

        debug!(
            memory_count = result.memory_count(),
            tokens = result.tokens_used,
            "Brief context injection complete"
        );

        Ok(result)
    }

    /// Convert similarity results to injection candidates.
    ///
    /// Looks up memory content from store and categorizes by weighted_agreement.
    fn convert_to_candidates(
        &self,
        results: &[SimilarityResult],
    ) -> Result<Vec<InjectionCandidate>, InjectionError> {
        let mut candidates = Vec::with_capacity(results.len());

        for result in results {
            // Lookup memory content from store
            let memory = self
                .store
                .get(result.memory_id)?
                .ok_or(InjectionError::MemoryNotFound(result.memory_id))?;

            // Determine category from weighted_similarity
            // weighted_similarity is already computed correctly (temporal excluded)
            let category = categorize_by_agreement(result.weighted_similarity);

            // Skip candidates below minimum threshold
            let Some(category) = category else {
                debug!(
                    memory_id = %result.memory_id,
                    weighted = result.weighted_similarity,
                    "Skipping candidate below threshold"
                );
                continue;
            };

            let candidate = InjectionCandidate::new(
                result.memory_id,
                memory.content.clone(),
                result.relevance_score,
                result.weighted_similarity,
                result.matching_spaces.clone(),
                category,
                memory.created_at,
            );

            candidates.push(candidate);
        }

        Ok(candidates)
    }

    /// Get selection statistics from last operation.
    ///
    /// Returns empty stats - use `generate_context` return value for actual stats.
    /// This method exists for API compatibility only.
    pub fn empty_stats(&self) -> SelectionStats {
        SelectionStats {
            selected_count: 0,
            rejected_count: 0,
            tokens_used: 0,
            tokens_available: self.budget.total,
            by_category: HashMap::new(),
        }
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Categorize a candidate by weighted_agreement score.
///
/// Uses thresholds from constitution.yaml:
/// - >= 2.5: HighRelevanceCluster
/// - >= 1.0: SingleSpaceMatch
/// - < 1.0: None (below injection threshold)
///
/// Note: DivergenceAlert and RecentSession categories are set explicitly
/// by the caller, not derived from weighted_agreement.
#[inline]
fn categorize_by_agreement(weighted_agreement: f32) -> Option<InjectionCategory> {
    if weighted_agreement >= HIGH_RELEVANCE_THRESHOLD {
        Some(InjectionCategory::HighRelevanceCluster)
    } else if weighted_agreement >= SINGLE_SPACE_THRESHOLD {
        Some(InjectionCategory::SingleSpaceMatch)
    } else {
        None
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Helper Functions Tests
    // =========================================================================

    #[test]
    fn test_categorize_by_agreement_high_relevance() {
        assert_eq!(
            categorize_by_agreement(3.0),
            Some(InjectionCategory::HighRelevanceCluster)
        );
        assert_eq!(
            categorize_by_agreement(2.5),
            Some(InjectionCategory::HighRelevanceCluster)
        );
        assert_eq!(
            categorize_by_agreement(8.5),
            Some(InjectionCategory::HighRelevanceCluster)
        );
        println!("[PASS] weighted_agreement >= 2.5 -> HighRelevanceCluster");
    }

    #[test]
    fn test_categorize_by_agreement_single_space() {
        assert_eq!(
            categorize_by_agreement(2.4),
            Some(InjectionCategory::SingleSpaceMatch)
        );
        assert_eq!(
            categorize_by_agreement(1.5),
            Some(InjectionCategory::SingleSpaceMatch)
        );
        assert_eq!(
            categorize_by_agreement(1.0),
            Some(InjectionCategory::SingleSpaceMatch)
        );
        println!("[PASS] weighted_agreement in [1.0, 2.5) -> SingleSpaceMatch");
    }

    #[test]
    fn test_categorize_by_agreement_below_threshold() {
        assert_eq!(categorize_by_agreement(0.9), None);
        assert_eq!(categorize_by_agreement(0.5), None);
        assert_eq!(categorize_by_agreement(0.0), None);
        println!("[PASS] weighted_agreement < 1.0 -> None");
    }

    #[test]
    fn test_estimate_tokens() {
        // 5 words × 1.3 = 6.5 -> 7
        assert_eq!(estimate_tokens("one two three four five"), 7);

        // 10 words × 1.3 = 13
        assert_eq!(
            estimate_tokens("the quick brown fox jumps over the lazy sleeping dog"),
            13
        );

        // 0 words -> 0
        assert_eq!(estimate_tokens(""), 0);

        // Whitespace only -> 0
        assert_eq!(estimate_tokens("   \n\t  "), 0);

        println!("[PASS] estimate_tokens uses word count × 1.3");
    }

    #[test]
    fn test_estimate_tokens_single_word() {
        // 1 word × 1.3 = 1.3 -> 2
        assert_eq!(estimate_tokens("hello"), 2);
        println!("[PASS] Single word rounds up correctly");
    }

    // =========================================================================
    // InjectionError Tests
    // =========================================================================

    #[test]
    fn test_injection_error_display() {
        let retrieval_err = InjectionError::RetrievalFailed(RetrieverError::Storage(
            StorageError::ReadFailed("test error".to_string()),
        ));
        assert!(retrieval_err.to_string().contains("Retrieval failed"));

        let memory_err = InjectionError::MemoryNotFound(Uuid::new_v4());
        assert!(memory_err.to_string().contains("Memory not found"));

        let fingerprint_err =
            InjectionError::InvalidFingerprint("validation failed".to_string());
        assert!(fingerprint_err.to_string().contains("Invalid query fingerprint"));

        println!("[PASS] InjectionError Display implementations work");
    }

    // =========================================================================
    // Threshold Boundary Tests (FSV Edge Cases)
    // =========================================================================

    #[test]
    fn test_fsv_edge_case_exact_high_relevance_threshold() {
        println!("FSV EDGE CASE 1: Exact threshold 2.5");
        println!("  Before: weighted_agreement = 2.5");

        let result = categorize_by_agreement(2.5);

        println!("  After: category = {:?}", result);
        println!("  Expected: Some(HighRelevanceCluster)");

        assert_eq!(
            result,
            Some(InjectionCategory::HighRelevanceCluster),
            "Exactly 2.5 should be HighRelevanceCluster"
        );
        println!("[PASS] FSV Edge Case 1: 2.5 -> HighRelevanceCluster");
    }

    #[test]
    fn test_fsv_edge_case_just_below_high_relevance() {
        println!("FSV EDGE CASE 2: Just below threshold 2.5");
        println!("  Before: weighted_agreement = 2.499");

        let result = categorize_by_agreement(2.499);

        println!("  After: category = {:?}", result);
        println!("  Expected: Some(SingleSpaceMatch)");

        assert_eq!(
            result,
            Some(InjectionCategory::SingleSpaceMatch),
            "2.499 should be SingleSpaceMatch"
        );
        println!("[PASS] FSV Edge Case 2: 2.499 -> SingleSpaceMatch");
    }

    #[test]
    fn test_fsv_edge_case_exact_single_space_threshold() {
        println!("FSV EDGE CASE 3: Exact threshold 1.0");
        println!("  Before: weighted_agreement = 1.0");

        let result = categorize_by_agreement(1.0);

        println!("  After: category = {:?}", result);
        println!("  Expected: Some(SingleSpaceMatch)");

        assert_eq!(
            result,
            Some(InjectionCategory::SingleSpaceMatch),
            "Exactly 1.0 should be SingleSpaceMatch"
        );
        println!("[PASS] FSV Edge Case 3: 1.0 -> SingleSpaceMatch");
    }

    #[test]
    fn test_fsv_edge_case_just_below_single_space() {
        println!("FSV EDGE CASE 4: Just below threshold 1.0");
        println!("  Before: weighted_agreement = 0.999");

        let result = categorize_by_agreement(0.999);

        println!("  After: category = {:?}", result);
        println!("  Expected: None");

        assert_eq!(result, None, "0.999 should be None");
        println!("[PASS] FSV Edge Case 4: 0.999 -> None");
    }

    #[test]
    fn test_fsv_edge_case_max_weighted_agreement() {
        println!("FSV EDGE CASE 5: Maximum weighted_agreement 8.5");
        println!("  Before: weighted_agreement = 8.5");

        let result = categorize_by_agreement(8.5);

        println!("  After: category = {:?}", result);
        println!("  Expected: Some(HighRelevanceCluster)");

        assert_eq!(
            result,
            Some(InjectionCategory::HighRelevanceCluster),
            "8.5 should be HighRelevanceCluster"
        );
        println!("[PASS] FSV Edge Case 5: 8.5 (max) -> HighRelevanceCluster");
    }

    #[test]
    fn test_fsv_edge_case_zero_agreement() {
        println!("FSV EDGE CASE 6: Zero weighted_agreement");
        println!("  Before: weighted_agreement = 0.0");

        let result = categorize_by_agreement(0.0);

        println!("  After: category = {:?}", result);
        println!("  Expected: None");

        assert_eq!(result, None, "0.0 should be None");
        println!("[PASS] FSV Edge Case 6: 0.0 -> None");
    }

    // =========================================================================
    // Integration Test Placeholders
    // =========================================================================
    // Full integration tests require mock implementations of SimilarityRetriever
    // and MemoryStore, which would be in a separate integration test file.

    #[test]
    fn test_constants() {
        assert_eq!(DEFAULT_RETRIEVAL_LIMIT, 50);
        assert!((HIGH_RELEVANCE_THRESHOLD - 2.5).abs() < f32::EPSILON);
        assert!((SINGLE_SPACE_THRESHOLD - 1.0).abs() < f32::EPSILON);
        println!("[PASS] Constants match constitution requirements");
    }
}

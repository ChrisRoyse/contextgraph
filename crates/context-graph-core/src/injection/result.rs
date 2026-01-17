//! InjectionResult type for injection pipeline output.
//!
//! This module provides the [`InjectionResult`] struct that captures
//! the complete output from context injection, including formatted
//! context and metadata about what was included.
//!
//! # Constitution Compliance
//! - AP-10: No NaN/Infinity in token counts
//! - AP-14: No .unwrap() in library code

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::candidate::InjectionCategory;
use crate::retrieval::DivergenceAlert;

/// Result of context injection pipeline.
///
/// Contains formatted context string ready for injection into Claude Code
/// hooks, plus metadata about what was included for debugging and analytics.
///
/// # Usage
///
/// Returned by `InjectionPipeline::generate_context()`. The `formatted_context`
/// field is injected into hook responses. Other fields support logging.
///
/// # Example
///
/// ```
/// use context_graph_core::injection::InjectionResult;
///
/// let result = InjectionResult::empty();
/// assert!(result.is_empty());
/// assert_eq!(result.memory_count(), 0);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectionResult {
    /// Formatted context string ready for injection into hook response.
    /// Empty string is valid (means no relevant context found).
    pub formatted_context: String,

    /// UUIDs of memories included in the context.
    pub included_memories: Vec<Uuid>,

    /// Divergence alerts surfaced to the user.
    pub divergence_alerts: Vec<DivergenceAlert>,

    /// Actual tokens used in formatted output.
    pub tokens_used: u32,

    /// Which categories had content included.
    pub categories_included: Vec<InjectionCategory>,
}

impl InjectionResult {
    /// Create new result with provided values.
    ///
    /// # Arguments
    ///
    /// * `formatted_context` - Formatted string for injection
    /// * `included_memories` - UUIDs of included memories
    /// * `divergence_alerts` - Divergence alerts to surface
    /// * `tokens_used` - Token count of formatted_context
    /// * `categories_included` - Categories with content
    pub fn new(
        formatted_context: String,
        included_memories: Vec<Uuid>,
        divergence_alerts: Vec<DivergenceAlert>,
        tokens_used: u32,
        categories_included: Vec<InjectionCategory>,
    ) -> Self {
        Self {
            formatted_context,
            included_memories,
            divergence_alerts,
            tokens_used,
            categories_included,
        }
    }

    /// Create empty result for when there's no relevant context.
    /// This is a normal state, not an error.
    #[inline]
    pub fn empty() -> Self {
        Self {
            formatted_context: String::new(),
            included_memories: Vec::new(),
            divergence_alerts: Vec::new(),
            tokens_used: 0,
            categories_included: Vec::new(),
        }
    }

    /// Check if result contains no context.
    /// True iff formatted_context is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.formatted_context.is_empty()
    }

    /// Number of memories included.
    #[inline]
    pub fn memory_count(&self) -> usize {
        self.included_memories.len()
    }

    /// Check if divergence alerts were included.
    #[inline]
    pub fn has_divergence_alerts(&self) -> bool {
        !self.divergence_alerts.is_empty()
    }

    /// Number of divergence alerts.
    #[inline]
    pub fn divergence_alert_count(&self) -> usize {
        self.divergence_alerts.len()
    }
}

impl Default for InjectionResult {
    fn default() -> Self {
        Self::empty()
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_result() {
        let result = InjectionResult::empty();

        assert!(result.is_empty(), "empty() should produce empty result");
        assert!(result.formatted_context.is_empty());
        assert!(result.included_memories.is_empty());
        assert!(result.divergence_alerts.is_empty());
        assert_eq!(result.tokens_used, 0);
        assert!(result.categories_included.is_empty());
        println!("[PASS] empty() returns valid empty result");
    }

    #[test]
    fn test_is_empty_false_when_has_content() {
        let result = InjectionResult::new(
            "Some context".to_string(),
            vec![Uuid::new_v4()],
            vec![],
            15,
            vec![InjectionCategory::HighRelevanceCluster],
        );

        assert!(!result.is_empty(), "Result with content should not be empty");
        println!("[PASS] is_empty() returns false when content exists");
    }

    #[test]
    fn test_default_is_empty() {
        let result = InjectionResult::default();
        assert!(result.is_empty(), "default() should be empty");
        println!("[PASS] Default implementation returns empty");
    }

    #[test]
    fn test_memory_count() {
        let result = InjectionResult::new(
            "context".to_string(),
            vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()],
            vec![],
            100,
            vec![],
        );

        assert_eq!(result.memory_count(), 3);
        println!("[PASS] memory_count() returns correct count");
    }

    #[test]
    fn test_has_divergence_alerts_false() {
        let result = InjectionResult::empty();
        assert!(!result.has_divergence_alerts());
        assert_eq!(result.divergence_alert_count(), 0);
        println!("[PASS] Empty result has no divergence alerts");
    }

    #[test]
    fn test_serialization_roundtrip() {
        let result = InjectionResult::new(
            "test context".to_string(),
            vec![Uuid::new_v4()],
            vec![],
            50,
            vec![InjectionCategory::SingleSpaceMatch],
        );

        let json = serde_json::to_string(&result).expect("serialize");
        let restored: InjectionResult = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(result.formatted_context, restored.formatted_context);
        assert_eq!(result.tokens_used, restored.tokens_used);
        assert_eq!(result.categories_included, restored.categories_included);
        println!("[PASS] Serialization roundtrip works");
    }

    // =========================================================================
    // Edge Cases
    // =========================================================================

    #[test]
    fn test_empty_context_with_metadata() {
        // Valid edge case: no formatted context but has metadata
        let result = InjectionResult::new(
            String::new(), // Empty context
            vec![Uuid::new_v4()], // But has memories
            vec![],
            0,
            vec![InjectionCategory::RecentSession],
        );

        assert!(result.is_empty(), "Empty string means empty");
        assert_eq!(result.memory_count(), 1, "But metadata preserved");
        println!("[PASS] Empty context with metadata handled correctly");
    }

    #[test]
    fn test_max_tokens() {
        let result = InjectionResult::new(
            "x".repeat(10000),
            vec![],
            vec![],
            u32::MAX, // Max tokens
            vec![],
        );

        assert_eq!(result.tokens_used, u32::MAX);
        println!("[PASS] Max token count accepted");
    }
}

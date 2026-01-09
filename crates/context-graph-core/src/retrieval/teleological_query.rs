//! Teleological query types for purpose-aware retrieval.
//!
//! This module provides the `TeleologicalQuery` struct that carries
//! query context including text, embeddings, purpose, goals, and filters
//! for the 5-stage teleological retrieval pipeline.
//!
//! # TASK-L008 Implementation
//!
//! Implements the query structure per constitution.yaml retrieval pipeline spec.
//! FAIL FAST: All validation errors are immediate, no silent fallbacks.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
use crate::types::fingerprint::{PurposeVector, SemanticFingerprint};
use crate::types::JohariQuadrant;

use super::PipelineStageConfig;

/// Query for teleological retrieval with purpose and goal context.
///
/// # Required Fields
///
/// Either `text` or `embeddings` MUST be provided. If both are empty,
/// `validate()` will return `CoreError::ValidationError`.
///
/// # Optional Fields
///
/// - `purpose`: Pre-computed purpose vector (computed if not provided)
/// - `target_goals`: Goal IDs for alignment filtering
/// - `johari_filter`: Restrict results to specific quadrants
/// - `pipeline_config`: Override default stage configuration
/// - `include_breakdown`: Enable detailed per-stage breakdown
///
/// # Example
///
/// ```ignore
/// use context_graph_core::retrieval::TeleologicalQuery;
///
/// let query = TeleologicalQuery::from_text("How does authentication work?")
///     .with_breakdown(true);
///
/// query.validate()?; // FAIL FAST on invalid query
/// ```
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TeleologicalQuery {
    /// Query text (REQUIRED if embeddings not provided).
    ///
    /// Will be embedded using MultiArrayEmbeddingProvider if embeddings is None.
    pub text: String,

    /// Pre-computed embeddings (skips embedding generation if provided).
    ///
    /// Use this when embeddings are already available to skip the ~30ms
    /// embedding latency.
    pub embeddings: Option<SemanticFingerprint>,

    /// Query's purpose vector (computed if not provided).
    ///
    /// If None, will be computed from embeddings using PurposeVectorComputer.
    pub purpose: Option<PurposeVector>,

    /// Target goal IDs for alignment scoring.
    ///
    /// If empty, alignment filtering is skipped (all results pass).
    /// If provided, results below `min_alignment_threshold` are filtered.
    pub target_goals: Vec<Uuid>,

    /// Johari quadrant filter (only return results in these quadrants).
    ///
    /// If None, all quadrants are included.
    /// If Some(vec), only results with dominant quadrant in the vec pass.
    pub johari_filter: Option<Vec<JohariQuadrant>>,

    /// Pipeline stage configuration overrides.
    ///
    /// If None, uses `PipelineStageConfig::default()` with:
    /// - splade_candidates: 1000
    /// - matryoshka_128d_limit: 200
    /// - full_search_limit: 100
    /// - teleological_limit: 50
    /// - late_interaction_limit: 20
    /// - rrf_k: 60.0
    /// - min_alignment_threshold: 0.55
    pub pipeline_config: Option<PipelineStageConfig>,

    /// Include per-stage breakdown in results.
    ///
    /// If true, `TeleologicalRetrievalResult.breakdown` will be populated
    /// with detailed per-stage candidate lists and filtering information.
    /// Useful for debugging and performance analysis.
    pub include_breakdown: bool,
}

impl TeleologicalQuery {
    /// Create a query from text.
    ///
    /// # Arguments
    /// * `text` - The query text to search for
    ///
    /// # Example
    /// ```ignore
    /// let query = TeleologicalQuery::from_text("authentication patterns");
    /// ```
    pub fn from_text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            embeddings: None,
            purpose: None,
            target_goals: Vec::new(),
            johari_filter: None,
            pipeline_config: None,
            include_breakdown: false,
        }
    }

    /// Create a query from pre-computed embeddings.
    ///
    /// Use this to skip embedding generation when embeddings are already
    /// available (e.g., from a previous query or cached embeddings).
    ///
    /// # Arguments
    /// * `embeddings` - Pre-computed semantic fingerprint
    ///
    /// # Example
    /// ```ignore
    /// let query = TeleologicalQuery::from_embeddings(cached_fingerprint);
    /// ```
    pub fn from_embeddings(embeddings: SemanticFingerprint) -> Self {
        Self {
            text: String::new(),
            embeddings: Some(embeddings),
            purpose: None,
            target_goals: Vec::new(),
            johari_filter: None,
            pipeline_config: None,
            include_breakdown: false,
        }
    }

    /// Set the purpose vector.
    pub fn with_purpose(mut self, purpose: PurposeVector) -> Self {
        self.purpose = Some(purpose);
        self
    }

    /// Set target goals for alignment filtering.
    pub fn with_goals(mut self, goals: Vec<Uuid>) -> Self {
        self.target_goals = goals;
        self
    }

    /// Add a single target goal.
    pub fn with_goal(mut self, goal: Uuid) -> Self {
        self.target_goals.push(goal);
        self
    }

    /// Set Johari quadrant filter.
    pub fn with_johari_filter(mut self, quadrants: Vec<JohariQuadrant>) -> Self {
        self.johari_filter = Some(quadrants);
        self
    }

    /// Set pipeline configuration.
    pub fn with_pipeline_config(mut self, config: PipelineStageConfig) -> Self {
        self.pipeline_config = Some(config);
        self
    }

    /// Enable breakdown in results.
    pub fn with_breakdown(mut self, include: bool) -> Self {
        self.include_breakdown = include;
        self
    }

    /// Set the minimum alignment threshold.
    ///
    /// Convenience method that creates or updates pipeline_config with
    /// the specified threshold.
    ///
    /// # Arguments
    /// * `threshold` - Minimum alignment score [0.0, 1.0]
    pub fn with_min_alignment(mut self, threshold: f32) -> Self {
        let config = self.pipeline_config.take().unwrap_or_default();
        self.pipeline_config = Some(PipelineStageConfig {
            min_alignment_threshold: threshold.clamp(0.0, 1.0),
            ..config
        });
        self
    }

    /// Validate query. FAILS FAST with CoreError::ValidationError.
    ///
    /// # Validation Rules
    ///
    /// 1. Either `text` or `embeddings` must be provided (not both empty)
    /// 2. If `text` is provided, it must not be only whitespace
    /// 3. If `johari_filter` is provided, it must not be empty
    /// 4. If `pipeline_config` thresholds must be in valid ranges
    ///
    /// # Errors
    ///
    /// Returns `CoreError::ValidationError` with:
    /// - `field`: The field that failed validation
    /// - `message`: Human-readable description of the failure
    ///
    /// # Example
    /// ```ignore
    /// let query = TeleologicalQuery::default();
    /// let result = query.validate(); // Err - no text or embeddings
    /// assert!(matches!(result, Err(CoreError::ValidationError { .. })));
    /// ```
    pub fn validate(&self) -> CoreResult<()> {
        // Rule 1: Either text or embeddings must be provided
        if self.text.is_empty() && self.embeddings.is_none() {
            tracing::error!(
                target: "pipeline",
                "TeleologicalQuery validation failed: no text or embeddings"
            );
            return Err(CoreError::ValidationError {
                field: "text".to_string(),
                message: "Either text or embeddings must be provided".to_string(),
            });
        }

        // Rule 2: Text must not be only whitespace
        if !self.text.is_empty() && self.text.trim().is_empty() {
            tracing::error!(
                target: "pipeline",
                "TeleologicalQuery validation failed: text is only whitespace"
            );
            return Err(CoreError::ValidationError {
                field: "text".to_string(),
                message: "Query text cannot be only whitespace".to_string(),
            });
        }

        // Rule 3: Johari filter must not be empty if provided
        if let Some(ref filter) = self.johari_filter {
            if filter.is_empty() {
                tracing::error!(
                    target: "pipeline",
                    "TeleologicalQuery validation failed: johari_filter is empty"
                );
                return Err(CoreError::ValidationError {
                    field: "johari_filter".to_string(),
                    message: "Johari filter cannot be empty - use None instead".to_string(),
                });
            }
        }

        // Rule 4: Validate pipeline config if provided
        if let Some(ref config) = self.pipeline_config {
            config.validate()?;
        }

        Ok(())
    }

    /// Check if query has pre-computed embeddings.
    #[inline]
    pub fn has_embeddings(&self) -> bool {
        self.embeddings.is_some()
    }

    /// Check if query has pre-computed purpose.
    #[inline]
    pub fn has_purpose(&self) -> bool {
        self.purpose.is_some()
    }

    /// Check if query has goal alignment filtering.
    #[inline]
    pub fn has_goals(&self) -> bool {
        !self.target_goals.is_empty()
    }

    /// Check if query has Johari quadrant filtering.
    #[inline]
    pub fn has_johari_filter(&self) -> bool {
        self.johari_filter.is_some()
    }

    /// Get the effective pipeline config (uses default if not set).
    pub fn effective_config(&self) -> PipelineStageConfig {
        self.pipeline_config.clone().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_text() {
        let query = TeleologicalQuery::from_text("authentication patterns");
        assert_eq!(query.text, "authentication patterns");
        assert!(query.embeddings.is_none());
        assert!(query.validate().is_ok());

        println!("[VERIFIED] TeleologicalQuery::from_text creates valid query");
    }

    #[test]
    fn test_from_embeddings() {
        let fingerprint = SemanticFingerprint::zeroed();
        let query = TeleologicalQuery::from_embeddings(fingerprint.clone());
        assert!(query.text.is_empty());
        assert!(query.embeddings.is_some());
        assert!(query.validate().is_ok());

        println!("[VERIFIED] TeleologicalQuery::from_embeddings creates valid query");
    }

    #[test]
    fn test_validate_empty_query_fails_fast() {
        let query = TeleologicalQuery::default();
        let result = query.validate();

        assert!(result.is_err(), "Empty query must fail validation");

        match result {
            Err(CoreError::ValidationError { field, message }) => {
                assert_eq!(field, "text");
                assert!(message.contains("text or embeddings"));
                println!("BEFORE: empty query");
                println!(
                    "AFTER: ValidationError {{ field: '{}', message: '{}' }}",
                    field, message
                );
            }
            other => panic!("Expected ValidationError, got: {:?}", other),
        }

        println!("[VERIFIED] Empty query fails fast with ValidationError");
    }

    #[test]
    fn test_validate_whitespace_only_fails() {
        let query = TeleologicalQuery::from_text("   ");
        let result = query.validate();

        assert!(result.is_err(), "Whitespace-only query must fail");

        match result {
            Err(CoreError::ValidationError { field, .. }) => {
                assert_eq!(field, "text");
            }
            other => panic!("Expected ValidationError, got: {:?}", other),
        }

        println!("[VERIFIED] Whitespace-only text fails validation");
    }

    #[test]
    fn test_validate_empty_johari_filter_fails() {
        let query = TeleologicalQuery::from_text("test").with_johari_filter(Vec::new());
        let result = query.validate();

        assert!(result.is_err(), "Empty johari_filter must fail");

        match result {
            Err(CoreError::ValidationError { field, .. }) => {
                assert_eq!(field, "johari_filter");
            }
            other => panic!("Expected ValidationError, got: {:?}", other),
        }

        println!("[VERIFIED] Empty johari_filter fails validation");
    }

    #[test]
    fn test_with_johari_filter() {
        let query = TeleologicalQuery::from_text("test")
            .with_johari_filter(vec![JohariQuadrant::Open, JohariQuadrant::Blind]);

        assert!(query.has_johari_filter());
        assert!(query.validate().is_ok());

        let filter = query.johari_filter.unwrap();
        assert_eq!(filter.len(), 2);
        assert!(filter.contains(&JohariQuadrant::Open));
        assert!(filter.contains(&JohariQuadrant::Blind));

        println!("[VERIFIED] with_johari_filter sets filter correctly");
    }

    #[test]
    fn test_with_goals() {
        let goal1 = Uuid::new_v4();
        let goal2 = Uuid::new_v4();

        let query = TeleologicalQuery::from_text("test")
            .with_goal(goal1)
            .with_goal(goal2);

        assert!(query.has_goals());
        assert_eq!(query.target_goals.len(), 2);
        assert!(query.target_goals.contains(&goal1));
        assert!(query.target_goals.contains(&goal2));

        println!("[VERIFIED] with_goal adds goals correctly");
    }

    #[test]
    fn test_with_min_alignment() {
        let query = TeleologicalQuery::from_text("test").with_min_alignment(0.70);

        let config = query.effective_config();
        assert!((config.min_alignment_threshold - 0.70).abs() < f32::EPSILON);

        println!("[VERIFIED] with_min_alignment sets threshold correctly");
    }

    #[test]
    fn test_with_min_alignment_clamped() {
        let query = TeleologicalQuery::from_text("test").with_min_alignment(1.5);

        let config = query.effective_config();
        assert!((config.min_alignment_threshold - 1.0).abs() < f32::EPSILON);

        let query2 = TeleologicalQuery::from_text("test").with_min_alignment(-0.5);

        let config2 = query2.effective_config();
        assert!((config2.min_alignment_threshold - 0.0).abs() < f32::EPSILON);

        println!("[VERIFIED] with_min_alignment clamps to [0.0, 1.0]");
    }

    #[test]
    fn test_effective_config_default() {
        let query = TeleologicalQuery::from_text("test");
        let config = query.effective_config();

        // Verify defaults from constitution.yaml
        assert_eq!(config.splade_candidates, 1000);
        assert_eq!(config.matryoshka_128d_limit, 200);
        assert_eq!(config.full_search_limit, 100);
        assert_eq!(config.teleological_limit, 50);
        assert_eq!(config.late_interaction_limit, 20);
        assert!((config.rrf_k - 60.0).abs() < f32::EPSILON);
        assert!((config.min_alignment_threshold - 0.55).abs() < f32::EPSILON);

        println!("[VERIFIED] Default config matches constitution.yaml");
    }

    #[test]
    fn test_builder_pattern() {
        let goal = Uuid::new_v4();
        let query = TeleologicalQuery::from_text("complex query")
            .with_goal(goal)
            .with_johari_filter(vec![JohariQuadrant::Open])
            .with_min_alignment(0.75)
            .with_breakdown(true);

        assert_eq!(query.text, "complex query");
        assert!(query.has_goals());
        assert!(query.has_johari_filter());
        assert!(query.include_breakdown);
        assert!(query.validate().is_ok());

        println!("[VERIFIED] Builder pattern chains correctly");
    }
}

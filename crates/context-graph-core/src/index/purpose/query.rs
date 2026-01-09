//! Purpose query types for indexed retrieval.
//!
//! # CRITICAL: NO FALLBACKS
//!
//! All validation failures are fatal errors.
//! Invalid queries must not execute - fail immediately.
//!
//! # Overview
//!
//! This module provides query types for the Purpose Pattern Index (Stage 4):
//!
//! - [`PurposeQueryTarget`]: Specifies what to search for (vector, pattern, or memory-based)
//! - [`PurposeQuery`]: Complete query with filters and constraints
//! - [`PurposeSearchResult`]: Results from purpose-based searches
//!
//! # Usage
//!
//! ```ignore
//! use context_graph_core::index::purpose::query::{
//!     PurposeQuery, PurposeQueryTarget, PurposeSearchResult,
//! };
//! use context_graph_core::types::fingerprint::PurposeVector;
//! use context_graph_core::purpose::GoalId;
//! use context_graph_core::types::JohariQuadrant;
//!
//! // Create a query from a purpose vector
//! let query = PurposeQuery::builder()
//!     .target(PurposeQueryTarget::Vector(purpose_vector))
//!     .limit(10)
//!     .min_similarity(0.7)
//!     .build()?;
//!
//! // Add optional filters
//! let filtered_query = query
//!     .with_goal_filter(GoalId::new("master_ml"))
//!     .with_quadrant_filter(JohariQuadrant::Open);
//! ```
//!
//! # Fail-Fast Semantics
//!
//! All validation is performed at query construction time:
//! - `min_similarity` must be in [0.0, 1.0]
//! - `limit` must be > 0
//! - Invalid values result in `PurposeIndexError::InvalidQuery`

use uuid::Uuid;

use crate::types::fingerprint::PurposeVector;
use crate::types::JohariQuadrant;

use super::entry::{GoalId, PurposeMetadata};
use super::error::{PurposeIndexError, PurposeIndexResult};

/// Specifies the target for a purpose-based query.
///
/// # Variants
///
/// - `Vector`: Query with an existing `PurposeVector`
/// - `Pattern`: Query for pattern clusters with constraints
/// - `FromMemory`: Find memories with similar purpose to a given memory
///
/// # Fail-Fast
///
/// The `Pattern` variant validates its parameters at construction:
/// - `coherence_threshold` must be in [0.0, 1.0]
#[derive(Clone, Debug)]
pub enum PurposeQueryTarget {
    /// Query with a purpose vector.
    ///
    /// Searches for memories with similar 13D alignment profiles.
    Vector(PurposeVector),

    /// Query for pattern clusters.
    ///
    /// Finds clusters of memories with similar purpose patterns.
    ///
    /// # Fields
    ///
    /// - `min_cluster_size`: Minimum number of members in a cluster
    /// - `coherence_threshold`: Minimum coherence score [0.0, 1.0]
    Pattern {
        /// Minimum number of memories in a cluster to be returned.
        min_cluster_size: usize,
        /// Minimum coherence threshold [0.0, 1.0].
        coherence_threshold: f32,
    },

    /// Find memories with similar purpose to a given memory.
    ///
    /// The target memory must exist in the index.
    FromMemory(Uuid),
}

impl PurposeQueryTarget {
    /// Create a Vector target from a PurposeVector.
    #[inline]
    pub fn vector(pv: PurposeVector) -> Self {
        Self::Vector(pv)
    }

    /// Create a Pattern target with validation.
    ///
    /// # Errors
    ///
    /// Returns `PurposeIndexError::InvalidQuery` if:
    /// - `min_cluster_size` is 0
    /// - `coherence_threshold` is not in [0.0, 1.0]
    pub fn pattern(min_cluster_size: usize, coherence_threshold: f32) -> PurposeIndexResult<Self> {
        if min_cluster_size == 0 {
            return Err(PurposeIndexError::invalid_query(
                "min_cluster_size must be > 0",
            ));
        }
        if !(0.0..=1.0).contains(&coherence_threshold) {
            return Err(PurposeIndexError::invalid_query(format!(
                "coherence_threshold {} must be in [0.0, 1.0]",
                coherence_threshold
            )));
        }
        Ok(Self::Pattern {
            min_cluster_size,
            coherence_threshold,
        })
    }

    /// Create a FromMemory target.
    #[inline]
    pub fn from_memory(memory_id: Uuid) -> Self {
        Self::FromMemory(memory_id)
    }

    /// Check if this target requires looking up an existing memory.
    #[inline]
    pub fn requires_memory_lookup(&self) -> bool {
        matches!(self, Self::FromMemory(_))
    }
}

/// Query for purpose-based search operations.
///
/// # Structure
///
/// A `PurposeQuery` consists of:
/// - `target`: What to search for (vector, pattern, or memory-based)
/// - `limit`: Maximum number of results to return
/// - `min_similarity`: Minimum similarity threshold [0.0, 1.0]
/// - `goal_filter`: Optional filter by goal ID
/// - `quadrant_filter`: Optional filter by Johari quadrant
///
/// # Construction
///
/// Use the builder pattern via [`PurposeQueryBuilder`] for flexible construction:
///
/// ```ignore
/// let query = PurposeQuery::builder()
///     .target(PurposeQueryTarget::Vector(pv))
///     .limit(10)
///     .min_similarity(0.7)
///     .goal_filter(GoalId::new("master_ml"))
///     .build()?;
/// ```
///
/// Or use the direct constructor:
///
/// ```ignore
/// let query = PurposeQuery::new(
///     PurposeQueryTarget::Vector(pv),
///     10,
///     0.7,
/// )?;
/// ```
///
/// # Fail-Fast Semantics
///
/// Validation is performed at construction time:
/// - `limit` must be > 0
/// - `min_similarity` must be in [0.0, 1.0]
#[derive(Clone, Debug)]
pub struct PurposeQuery {
    /// The query target specifying what to search for.
    pub target: PurposeQueryTarget,

    /// Maximum number of results to return.
    ///
    /// Must be > 0.
    pub limit: usize,

    /// Minimum similarity threshold [0.0, 1.0].
    ///
    /// Results with similarity below this threshold are filtered out.
    pub min_similarity: f32,

    /// Optional filter by goal ID.
    ///
    /// When set, only memories aligned with this goal are returned.
    pub goal_filter: Option<GoalId>,

    /// Optional filter by Johari quadrant.
    ///
    /// When set, only memories in this quadrant are returned.
    pub quadrant_filter: Option<JohariQuadrant>,
}

impl PurposeQuery {
    /// Create a new PurposeQuery with validation.
    ///
    /// # Arguments
    ///
    /// * `target` - The query target
    /// * `limit` - Maximum results to return (must be > 0)
    /// * `min_similarity` - Minimum similarity threshold [0.0, 1.0]
    ///
    /// # Errors
    ///
    /// Returns `PurposeIndexError::InvalidQuery` if:
    /// - `limit` is 0
    /// - `min_similarity` is not in [0.0, 1.0]
    ///
    /// # Example
    ///
    /// ```ignore
    /// let query = PurposeQuery::new(
    ///     PurposeQueryTarget::Vector(purpose_vector),
    ///     10,
    ///     0.7,
    /// )?;
    /// ```
    pub fn new(
        target: PurposeQueryTarget,
        limit: usize,
        min_similarity: f32,
    ) -> PurposeIndexResult<Self> {
        Self::validate_limit(limit)?;
        Self::validate_min_similarity(min_similarity)?;

        Ok(Self {
            target,
            limit,
            min_similarity,
            goal_filter: None,
            quadrant_filter: None,
        })
    }

    /// Create a builder for constructing PurposeQuery.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let query = PurposeQuery::builder()
    ///     .target(PurposeQueryTarget::Vector(pv))
    ///     .limit(10)
    ///     .min_similarity(0.5)
    ///     .build()?;
    /// ```
    #[inline]
    pub fn builder() -> PurposeQueryBuilder {
        PurposeQueryBuilder::new()
    }

    /// Add a goal filter to the query.
    ///
    /// Returns a new query with the filter applied.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let filtered = query.with_goal_filter(GoalId::new("master_ml"));
    /// ```
    #[must_use]
    pub fn with_goal_filter(mut self, goal: GoalId) -> Self {
        self.goal_filter = Some(goal);
        self
    }

    /// Add a quadrant filter to the query.
    ///
    /// Returns a new query with the filter applied.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let filtered = query.with_quadrant_filter(JohariQuadrant::Open);
    /// ```
    #[must_use]
    pub fn with_quadrant_filter(mut self, quadrant: JohariQuadrant) -> Self {
        self.quadrant_filter = Some(quadrant);
        self
    }

    /// Validate that this query is internally consistent.
    ///
    /// This is called automatically during construction but can be
    /// called again if the query is modified.
    ///
    /// # Errors
    ///
    /// Returns `PurposeIndexError::InvalidQuery` if validation fails.
    pub fn validate(&self) -> PurposeIndexResult<()> {
        Self::validate_limit(self.limit)?;
        Self::validate_min_similarity(self.min_similarity)?;
        Ok(())
    }

    /// Validate the limit parameter.
    ///
    /// # Errors
    ///
    /// Returns error if limit is 0.
    #[inline]
    fn validate_limit(limit: usize) -> PurposeIndexResult<()> {
        if limit == 0 {
            return Err(PurposeIndexError::invalid_query("limit must be > 0"));
        }
        Ok(())
    }

    /// Validate the min_similarity parameter.
    ///
    /// # Errors
    ///
    /// Returns error if min_similarity is not in [0.0, 1.0].
    #[inline]
    fn validate_min_similarity(min_similarity: f32) -> PurposeIndexResult<()> {
        if !(0.0..=1.0).contains(&min_similarity) {
            return Err(PurposeIndexError::invalid_query(format!(
                "min_similarity {} must be in [0.0, 1.0]",
                min_similarity
            )));
        }
        if min_similarity.is_nan() {
            return Err(PurposeIndexError::invalid_query(
                "min_similarity cannot be NaN",
            ));
        }
        Ok(())
    }

    /// Check if this query has any filters applied.
    #[inline]
    pub fn has_filters(&self) -> bool {
        self.goal_filter.is_some() || self.quadrant_filter.is_some()
    }

    /// Get the number of filters applied.
    #[inline]
    pub fn filter_count(&self) -> usize {
        let mut count = 0;
        if self.goal_filter.is_some() {
            count += 1;
        }
        if self.quadrant_filter.is_some() {
            count += 1;
        }
        count
    }
}

/// Builder for constructing [`PurposeQuery`] instances.
///
/// Provides a fluent interface for building queries with validation
/// performed at the final `build()` step.
///
/// # Example
///
/// ```ignore
/// let query = PurposeQueryBuilder::new()
///     .target(PurposeQueryTarget::Vector(pv))
///     .limit(10)
///     .min_similarity(0.7)
///     .goal_filter(GoalId::new("learn_pytorch"))
///     .quadrant_filter(JohariQuadrant::Open)
///     .build()?;
/// ```
#[derive(Clone, Debug, Default)]
pub struct PurposeQueryBuilder {
    target: Option<PurposeQueryTarget>,
    limit: Option<usize>,
    min_similarity: Option<f32>,
    goal_filter: Option<GoalId>,
    quadrant_filter: Option<JohariQuadrant>,
}

impl PurposeQueryBuilder {
    /// Create a new builder with default values.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the query target.
    ///
    /// # Required
    ///
    /// This field is required. `build()` will fail if not set.
    #[must_use]
    pub fn target(mut self, target: PurposeQueryTarget) -> Self {
        self.target = Some(target);
        self
    }

    /// Set the maximum number of results.
    ///
    /// # Required
    ///
    /// This field is required. `build()` will fail if not set.
    ///
    /// # Validation
    ///
    /// Must be > 0.
    #[must_use]
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set the minimum similarity threshold.
    ///
    /// # Required
    ///
    /// This field is required. `build()` will fail if not set.
    ///
    /// # Validation
    ///
    /// Must be in [0.0, 1.0].
    #[must_use]
    pub fn min_similarity(mut self, min_similarity: f32) -> Self {
        self.min_similarity = Some(min_similarity);
        self
    }

    /// Set an optional goal filter.
    #[must_use]
    pub fn goal_filter(mut self, goal: GoalId) -> Self {
        self.goal_filter = Some(goal);
        self
    }

    /// Set an optional quadrant filter.
    #[must_use]
    pub fn quadrant_filter(mut self, quadrant: JohariQuadrant) -> Self {
        self.quadrant_filter = Some(quadrant);
        self
    }

    /// Build the query with validation.
    ///
    /// # Errors
    ///
    /// Returns `PurposeIndexError::InvalidQuery` if:
    /// - `target` is not set
    /// - `limit` is not set or is 0
    /// - `min_similarity` is not set or not in [0.0, 1.0]
    pub fn build(self) -> PurposeIndexResult<PurposeQuery> {
        let target = self
            .target
            .ok_or_else(|| PurposeIndexError::invalid_query("target is required"))?;

        let limit = self
            .limit
            .ok_or_else(|| PurposeIndexError::invalid_query("limit is required"))?;

        let min_similarity = self
            .min_similarity
            .ok_or_else(|| PurposeIndexError::invalid_query("min_similarity is required"))?;

        let mut query = PurposeQuery::new(target, limit, min_similarity)?;

        if let Some(goal) = self.goal_filter {
            query = query.with_goal_filter(goal);
        }
        if let Some(quadrant) = self.quadrant_filter {
            query = query.with_quadrant_filter(quadrant);
        }

        Ok(query)
    }
}

/// Result from a purpose-based search operation.
///
/// Contains the matching memory's ID, similarity score, purpose vector,
/// and associated metadata.
///
/// # Fields
///
/// - `memory_id`: UUID of the matching memory
/// - `purpose_similarity`: Similarity score in purpose space [0.0, 1.0]
/// - `purpose_vector`: The full 13D purpose vector
/// - `metadata`: Associated metadata (goal, confidence, quadrant)
///
/// # Ordering
///
/// Results are typically ordered by `purpose_similarity` (descending).
#[derive(Clone, Debug)]
pub struct PurposeSearchResult {
    /// The matching memory ID.
    pub memory_id: Uuid,

    /// Similarity score in purpose space.
    ///
    /// Range: [0.0, 1.0] where 1.0 is identical purpose alignment.
    pub purpose_similarity: f32,

    /// The purpose vector of the matching memory.
    pub purpose_vector: PurposeVector,

    /// Associated metadata about the purpose computation.
    pub metadata: PurposeMetadata,
}

impl PurposeSearchResult {
    /// Create a new search result.
    ///
    /// # Arguments
    ///
    /// * `memory_id` - UUID of the matching memory
    /// * `purpose_similarity` - Similarity score [0.0, 1.0]
    /// * `purpose_vector` - The 13D purpose vector
    /// * `metadata` - Associated metadata
    pub fn new(
        memory_id: Uuid,
        purpose_similarity: f32,
        purpose_vector: PurposeVector,
        metadata: PurposeMetadata,
    ) -> Self {
        Self {
            memory_id,
            purpose_similarity,
            purpose_vector,
            metadata,
        }
    }

    /// Get the aggregate alignment of this result's purpose vector.
    #[inline]
    pub fn aggregate_alignment(&self) -> f32 {
        self.purpose_vector.aggregate_alignment()
    }

    /// Get the dominant embedder index.
    #[inline]
    pub fn dominant_embedder(&self) -> u8 {
        self.purpose_vector.dominant_embedder
    }

    /// Get the coherence score.
    #[inline]
    pub fn coherence(&self) -> f32 {
        self.purpose_vector.coherence
    }

    /// Check if this result passes a given goal filter.
    #[inline]
    pub fn matches_goal(&self, goal: &GoalId) -> bool {
        &self.metadata.primary_goal == goal
    }

    /// Check if this result passes a given quadrant filter.
    #[inline]
    pub fn matches_quadrant(&self, quadrant: JohariQuadrant) -> bool {
        self.metadata.dominant_quadrant == quadrant
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::config::PURPOSE_VECTOR_DIM;

    // ============================================================================
    // Helper functions for creating test data
    // ============================================================================

    /// Create a purpose vector with deterministic values based on a base value.
    fn create_purpose_vector(base: f32) -> PurposeVector {
        let mut alignments = [0.0f32; PURPOSE_VECTOR_DIM];
        for (i, alignment) in alignments.iter_mut().enumerate() {
            *alignment = (base + i as f32 * 0.05).clamp(0.0, 1.0);
        }
        PurposeVector::new(alignments)
    }

    /// Create metadata for testing.
    fn create_metadata(goal: &str, quadrant: JohariQuadrant) -> PurposeMetadata {
        PurposeMetadata::new(GoalId::new(goal), 0.85, quadrant).unwrap()
    }

    // ============================================================================
    // PurposeQueryTarget Tests
    // ============================================================================

    #[test]
    fn test_purpose_query_target_vector() {
        let pv = create_purpose_vector(0.5);
        let target = PurposeQueryTarget::vector(pv.clone());

        match target {
            PurposeQueryTarget::Vector(v) => {
                assert_eq!(v.alignments, pv.alignments);
            }
            _ => panic!("Expected Vector variant"),
        }

        println!("[VERIFIED] PurposeQueryTarget::vector creates Vector variant");
    }

    #[test]
    fn test_purpose_query_target_pattern_valid() {
        let target = PurposeQueryTarget::pattern(5, 0.7).unwrap();

        match target {
            PurposeQueryTarget::Pattern {
                min_cluster_size,
                coherence_threshold,
            } => {
                assert_eq!(min_cluster_size, 5);
                assert!((coherence_threshold - 0.7).abs() < f32::EPSILON);
            }
            _ => panic!("Expected Pattern variant"),
        }

        println!(
            "[VERIFIED] PurposeQueryTarget::pattern creates Pattern variant with valid params"
        );
    }

    #[test]
    fn test_purpose_query_target_pattern_boundary_values() {
        // Test coherence_threshold = 0.0
        let target = PurposeQueryTarget::pattern(1, 0.0).unwrap();
        if let PurposeQueryTarget::Pattern {
            coherence_threshold,
            ..
        } = target
        {
            assert_eq!(coherence_threshold, 0.0);
        }

        // Test coherence_threshold = 1.0
        let target = PurposeQueryTarget::pattern(1, 1.0).unwrap();
        if let PurposeQueryTarget::Pattern {
            coherence_threshold,
            ..
        } = target
        {
            assert_eq!(coherence_threshold, 1.0);
        }

        println!("[VERIFIED] PurposeQueryTarget::pattern accepts boundary values 0.0 and 1.0");
    }

    #[test]
    fn test_purpose_query_target_pattern_invalid_cluster_size() {
        let result = PurposeQueryTarget::pattern(0, 0.5);
        assert!(result.is_err());

        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("min_cluster_size"));

        println!(
            "[VERIFIED] FAIL FAST: PurposeQueryTarget::pattern rejects min_cluster_size=0: {}",
            msg
        );
    }

    #[test]
    fn test_purpose_query_target_pattern_invalid_coherence_over() {
        let result = PurposeQueryTarget::pattern(5, 1.5);
        assert!(result.is_err());

        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("coherence_threshold"));
        assert!(msg.contains("1.5"));

        println!(
            "[VERIFIED] FAIL FAST: PurposeQueryTarget::pattern rejects coherence_threshold=1.5: {}",
            msg
        );
    }

    #[test]
    fn test_purpose_query_target_pattern_invalid_coherence_under() {
        let result = PurposeQueryTarget::pattern(5, -0.1);
        assert!(result.is_err());

        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("coherence_threshold"));

        println!(
            "[VERIFIED] FAIL FAST: PurposeQueryTarget::pattern rejects coherence_threshold=-0.1: {}",
            msg
        );
    }

    #[test]
    fn test_purpose_query_target_from_memory() {
        let id = Uuid::new_v4();
        let target = PurposeQueryTarget::from_memory(id);

        match target {
            PurposeQueryTarget::FromMemory(mem_id) => {
                assert_eq!(mem_id, id);
            }
            _ => panic!("Expected FromMemory variant"),
        }

        println!("[VERIFIED] PurposeQueryTarget::from_memory creates FromMemory variant");
    }

    #[test]
    fn test_purpose_query_target_requires_memory_lookup() {
        let pv = create_purpose_vector(0.5);

        assert!(!PurposeQueryTarget::vector(pv).requires_memory_lookup());
        assert!(!PurposeQueryTarget::pattern(5, 0.7)
            .unwrap()
            .requires_memory_lookup());
        assert!(PurposeQueryTarget::from_memory(Uuid::new_v4()).requires_memory_lookup());

        println!("[VERIFIED] requires_memory_lookup returns true only for FromMemory");
    }

    // ============================================================================
    // PurposeQuery Tests
    // ============================================================================

    #[test]
    fn test_purpose_query_new_valid() {
        let pv = create_purpose_vector(0.5);
        let query = PurposeQuery::new(PurposeQueryTarget::Vector(pv), 10, 0.7).unwrap();

        assert_eq!(query.limit, 10);
        assert!((query.min_similarity - 0.7).abs() < f32::EPSILON);
        assert!(query.goal_filter.is_none());
        assert!(query.quadrant_filter.is_none());

        println!("[VERIFIED] PurposeQuery::new creates valid query");
    }

    #[test]
    fn test_purpose_query_new_boundary_min_similarity() {
        let pv = create_purpose_vector(0.5);

        // Test min_similarity = 0.0
        let query = PurposeQuery::new(PurposeQueryTarget::Vector(pv.clone()), 10, 0.0).unwrap();
        assert_eq!(query.min_similarity, 0.0);

        // Test min_similarity = 1.0
        let query = PurposeQuery::new(PurposeQueryTarget::Vector(pv), 10, 1.0).unwrap();
        assert_eq!(query.min_similarity, 1.0);

        println!("[VERIFIED] PurposeQuery::new accepts min_similarity boundary values 0.0 and 1.0");
    }

    #[test]
    fn test_purpose_query_new_invalid_limit_zero() {
        let pv = create_purpose_vector(0.5);
        let result = PurposeQuery::new(PurposeQueryTarget::Vector(pv), 0, 0.5);

        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("limit"));

        println!(
            "[VERIFIED] FAIL FAST: PurposeQuery::new rejects limit=0: {}",
            msg
        );
    }

    #[test]
    fn test_purpose_query_new_invalid_min_similarity_over() {
        let pv = create_purpose_vector(0.5);
        let result = PurposeQuery::new(PurposeQueryTarget::Vector(pv), 10, 1.5);

        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("min_similarity"));
        assert!(msg.contains("1.5"));

        println!(
            "[VERIFIED] FAIL FAST: PurposeQuery::new rejects min_similarity=1.5: {}",
            msg
        );
    }

    #[test]
    fn test_purpose_query_new_invalid_min_similarity_under() {
        let pv = create_purpose_vector(0.5);
        let result = PurposeQuery::new(PurposeQueryTarget::Vector(pv), 10, -0.1);

        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("min_similarity"));

        println!(
            "[VERIFIED] FAIL FAST: PurposeQuery::new rejects min_similarity=-0.1: {}",
            msg
        );
    }

    #[test]
    fn test_purpose_query_new_invalid_min_similarity_nan() {
        let pv = create_purpose_vector(0.5);
        let result = PurposeQuery::new(PurposeQueryTarget::Vector(pv), 10, f32::NAN);

        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("NaN") || msg.contains("min_similarity"));

        println!(
            "[VERIFIED] FAIL FAST: PurposeQuery::new rejects min_similarity=NaN: {}",
            msg
        );
    }

    #[test]
    fn test_purpose_query_with_goal_filter() {
        let pv = create_purpose_vector(0.5);
        let query = PurposeQuery::new(PurposeQueryTarget::Vector(pv), 10, 0.5)
            .unwrap()
            .with_goal_filter(GoalId::new("master_ml"));

        assert!(query.goal_filter.is_some());
        assert_eq!(query.goal_filter.as_ref().unwrap().as_str(), "master_ml");

        println!("[VERIFIED] PurposeQuery::with_goal_filter sets goal filter correctly");
    }

    #[test]
    fn test_purpose_query_with_quadrant_filter() {
        let pv = create_purpose_vector(0.5);
        let query = PurposeQuery::new(PurposeQueryTarget::Vector(pv), 10, 0.5)
            .unwrap()
            .with_quadrant_filter(JohariQuadrant::Hidden);

        assert!(query.quadrant_filter.is_some());
        assert_eq!(query.quadrant_filter.unwrap(), JohariQuadrant::Hidden);

        println!("[VERIFIED] PurposeQuery::with_quadrant_filter sets quadrant filter correctly");
    }

    #[test]
    fn test_purpose_query_validate() {
        let pv = create_purpose_vector(0.5);
        let query = PurposeQuery::new(PurposeQueryTarget::Vector(pv), 10, 0.5).unwrap();

        assert!(query.validate().is_ok());

        println!("[VERIFIED] PurposeQuery::validate passes for valid query");
    }

    #[test]
    fn test_purpose_query_has_filters() {
        let pv = create_purpose_vector(0.5);

        // No filters
        let query = PurposeQuery::new(PurposeQueryTarget::Vector(pv.clone()), 10, 0.5).unwrap();
        assert!(!query.has_filters());
        assert_eq!(query.filter_count(), 0);

        // Goal filter only
        let query = query.with_goal_filter(GoalId::new("test"));
        assert!(query.has_filters());
        assert_eq!(query.filter_count(), 1);

        // Both filters
        let query = PurposeQuery::new(PurposeQueryTarget::Vector(pv), 10, 0.5)
            .unwrap()
            .with_goal_filter(GoalId::new("test"))
            .with_quadrant_filter(JohariQuadrant::Open);
        assert!(query.has_filters());
        assert_eq!(query.filter_count(), 2);

        println!("[VERIFIED] has_filters and filter_count work correctly");
    }

    // ============================================================================
    // PurposeQueryBuilder Tests
    // ============================================================================

    #[test]
    fn test_purpose_query_builder_full() {
        let pv = create_purpose_vector(0.5);

        let query = PurposeQueryBuilder::new()
            .target(PurposeQueryTarget::Vector(pv))
            .limit(20)
            .min_similarity(0.8)
            .goal_filter(GoalId::new("learn_pytorch"))
            .quadrant_filter(JohariQuadrant::Blind)
            .build()
            .unwrap();

        assert_eq!(query.limit, 20);
        assert!((query.min_similarity - 0.8).abs() < f32::EPSILON);
        assert_eq!(
            query.goal_filter.as_ref().unwrap().as_str(),
            "learn_pytorch"
        );
        assert_eq!(query.quadrant_filter.unwrap(), JohariQuadrant::Blind);

        println!("[VERIFIED] PurposeQueryBuilder builds complete query with all fields");
    }

    #[test]
    fn test_purpose_query_builder_minimal() {
        let pv = create_purpose_vector(0.5);

        let query = PurposeQueryBuilder::new()
            .target(PurposeQueryTarget::Vector(pv))
            .limit(5)
            .min_similarity(0.0)
            .build()
            .unwrap();

        assert_eq!(query.limit, 5);
        assert_eq!(query.min_similarity, 0.0);
        assert!(query.goal_filter.is_none());
        assert!(query.quadrant_filter.is_none());

        println!("[VERIFIED] PurposeQueryBuilder builds minimal query without filters");
    }

    #[test]
    fn test_purpose_query_builder_missing_target() {
        let result = PurposeQueryBuilder::new()
            .limit(10)
            .min_similarity(0.5)
            .build();

        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("target"));

        println!(
            "[VERIFIED] FAIL FAST: PurposeQueryBuilder::build rejects missing target: {}",
            msg
        );
    }

    #[test]
    fn test_purpose_query_builder_missing_limit() {
        let pv = create_purpose_vector(0.5);

        let result = PurposeQueryBuilder::new()
            .target(PurposeQueryTarget::Vector(pv))
            .min_similarity(0.5)
            .build();

        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("limit"));

        println!(
            "[VERIFIED] FAIL FAST: PurposeQueryBuilder::build rejects missing limit: {}",
            msg
        );
    }

    #[test]
    fn test_purpose_query_builder_missing_min_similarity() {
        let pv = create_purpose_vector(0.5);

        let result = PurposeQueryBuilder::new()
            .target(PurposeQueryTarget::Vector(pv))
            .limit(10)
            .build();

        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("min_similarity"));

        println!(
            "[VERIFIED] FAIL FAST: PurposeQueryBuilder::build rejects missing min_similarity: {}",
            msg
        );
    }

    #[test]
    fn test_purpose_query_builder_chained() {
        let pv = create_purpose_vector(0.7);

        // Test that builder methods can be chained in any order
        let query = PurposeQuery::builder()
            .min_similarity(0.6)
            .limit(15)
            .quadrant_filter(JohariQuadrant::Unknown)
            .target(PurposeQueryTarget::Vector(pv))
            .goal_filter(GoalId::new("frontier"))
            .build()
            .unwrap();

        assert_eq!(query.limit, 15);
        assert!((query.min_similarity - 0.6).abs() < f32::EPSILON);

        println!("[VERIFIED] PurposeQueryBuilder allows chaining in any order");
    }

    // ============================================================================
    // PurposeSearchResult Tests
    // ============================================================================

    #[test]
    fn test_purpose_search_result_new() {
        let id = Uuid::new_v4();
        let pv = create_purpose_vector(0.8);
        let metadata = create_metadata("master_ml", JohariQuadrant::Open);

        let result = PurposeSearchResult::new(id, 0.95, pv.clone(), metadata);

        assert_eq!(result.memory_id, id);
        assert!((result.purpose_similarity - 0.95).abs() < f32::EPSILON);
        assert_eq!(result.purpose_vector.alignments, pv.alignments);
        assert_eq!(result.metadata.primary_goal.as_str(), "master_ml");

        println!("[VERIFIED] PurposeSearchResult::new creates result with all fields");
    }

    #[test]
    fn test_purpose_search_result_aggregate_alignment() {
        let pv = PurposeVector::new([0.75; PURPOSE_VECTOR_DIM]);
        let metadata = create_metadata("test", JohariQuadrant::Open);
        let result = PurposeSearchResult::new(Uuid::new_v4(), 0.9, pv, metadata);

        let aggregate = result.aggregate_alignment();
        assert!((aggregate - 0.75).abs() < f32::EPSILON);

        println!("[VERIFIED] PurposeSearchResult::aggregate_alignment returns correct value");
    }

    #[test]
    fn test_purpose_search_result_dominant_embedder() {
        let mut alignments = [0.5; PURPOSE_VECTOR_DIM];
        alignments[7] = 0.95; // E8 is dominant
        let pv = PurposeVector::new(alignments);
        let metadata = create_metadata("test", JohariQuadrant::Open);
        let result = PurposeSearchResult::new(Uuid::new_v4(), 0.9, pv, metadata);

        assert_eq!(result.dominant_embedder(), 7);

        println!("[VERIFIED] PurposeSearchResult::dominant_embedder returns correct index");
    }

    #[test]
    fn test_purpose_search_result_coherence() {
        let pv = PurposeVector::new([0.8; PURPOSE_VECTOR_DIM]); // Uniform = high coherence
        let metadata = create_metadata("test", JohariQuadrant::Open);
        let result = PurposeSearchResult::new(Uuid::new_v4(), 0.9, pv, metadata);

        let coherence = result.coherence();
        assert!((coherence - 1.0).abs() < 1e-6);

        println!("[VERIFIED] PurposeSearchResult::coherence returns correct value");
    }

    #[test]
    fn test_purpose_search_result_matches_goal() {
        let pv = create_purpose_vector(0.8);
        let metadata = create_metadata("master_ml", JohariQuadrant::Open);
        let result = PurposeSearchResult::new(Uuid::new_v4(), 0.9, pv, metadata);

        assert!(result.matches_goal(&GoalId::new("master_ml")));
        assert!(!result.matches_goal(&GoalId::new("other_goal")));

        println!("[VERIFIED] PurposeSearchResult::matches_goal filters correctly");
    }

    #[test]
    fn test_purpose_search_result_matches_quadrant() {
        let pv = create_purpose_vector(0.8);
        let metadata = create_metadata("test", JohariQuadrant::Hidden);
        let result = PurposeSearchResult::new(Uuid::new_v4(), 0.9, pv, metadata);

        assert!(result.matches_quadrant(JohariQuadrant::Hidden));
        assert!(!result.matches_quadrant(JohariQuadrant::Open));
        assert!(!result.matches_quadrant(JohariQuadrant::Blind));
        assert!(!result.matches_quadrant(JohariQuadrant::Unknown));

        println!("[VERIFIED] PurposeSearchResult::matches_quadrant filters correctly");
    }

    #[test]
    fn test_purpose_search_result_clone() {
        let pv = create_purpose_vector(0.8);
        let metadata = create_metadata("test", JohariQuadrant::Open);
        let result = PurposeSearchResult::new(Uuid::new_v4(), 0.9, pv, metadata);

        let cloned = result.clone();

        assert_eq!(cloned.memory_id, result.memory_id);
        assert_eq!(cloned.purpose_similarity, result.purpose_similarity);
        assert_eq!(
            cloned.purpose_vector.alignments,
            result.purpose_vector.alignments
        );

        println!("[VERIFIED] PurposeSearchResult implements Clone correctly");
    }

    #[test]
    fn test_purpose_search_result_debug() {
        let pv = create_purpose_vector(0.8);
        let metadata = create_metadata("test", JohariQuadrant::Open);
        let result = PurposeSearchResult::new(Uuid::nil(), 0.9, pv, metadata);

        let debug_str = format!("{:?}", result);

        assert!(debug_str.contains("PurposeSearchResult"));
        assert!(debug_str.contains("memory_id"));

        println!("[VERIFIED] PurposeSearchResult implements Debug correctly");
    }

    // ============================================================================
    // Edge Case Tests
    // ============================================================================

    #[test]
    fn test_all_quadrant_filters() {
        let pv = create_purpose_vector(0.5);

        for quadrant in JohariQuadrant::all() {
            let query = PurposeQuery::new(PurposeQueryTarget::Vector(pv.clone()), 10, 0.5)
                .unwrap()
                .with_quadrant_filter(quadrant);

            assert_eq!(query.quadrant_filter.unwrap(), quadrant);
        }

        println!("[VERIFIED] PurposeQuery works with all JohariQuadrant variants");
    }

    #[test]
    fn test_query_with_from_memory_target() {
        let id = Uuid::new_v4();
        let query = PurposeQuery::new(PurposeQueryTarget::from_memory(id), 5, 0.3).unwrap();

        assert!(query.target.requires_memory_lookup());
        assert_eq!(query.limit, 5);

        println!("[VERIFIED] PurposeQuery works with FromMemory target");
    }

    #[test]
    fn test_query_with_pattern_target() {
        let target = PurposeQueryTarget::pattern(10, 0.8).unwrap();
        let query = PurposeQuery::new(target, 50, 0.0).unwrap();

        assert!(!query.target.requires_memory_lookup());

        match query.target {
            PurposeQueryTarget::Pattern {
                min_cluster_size,
                coherence_threshold,
            } => {
                assert_eq!(min_cluster_size, 10);
                assert!((coherence_threshold - 0.8).abs() < f32::EPSILON);
            }
            _ => panic!("Expected Pattern target"),
        }

        println!("[VERIFIED] PurposeQuery works with Pattern target");
    }

    #[test]
    fn test_large_limit_value() {
        let pv = create_purpose_vector(0.5);
        let query = PurposeQuery::new(PurposeQueryTarget::Vector(pv), 1_000_000, 0.0).unwrap();

        assert_eq!(query.limit, 1_000_000);

        println!("[VERIFIED] PurposeQuery accepts large limit values");
    }
}

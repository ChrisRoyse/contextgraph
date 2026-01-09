//! Search filters for semantic search operations.
//!
//! Provides builder pattern for constructing search filters that can:
//! - Filter by node type (Domain from Marblestone)
//! - Set minimum similarity threshold
//! - Limit results per query
//! - Apply custom predicates
//!
//! # Constitution Reference
//!
//! - TECH-GRAPH-004: Semantic search specification
//! - AP-001: Never unwrap() in prod - all errors properly typed

use crate::error::{GraphError, GraphResult};

// Import Domain from storage edges module
pub use crate::storage::edges::Domain;

/// Search filter configuration for semantic search.
///
/// Uses builder pattern for ergonomic configuration.
/// All filters are optional - omitted filters are not applied.
///
/// # Example
///
/// ```ignore
/// use context_graph_graph::search::SearchFilters;
/// use context_graph_graph::storage::edges::Domain;
///
/// let filters = SearchFilters::new()
///     .with_domain(Domain::Code)
///     .with_min_similarity(0.7)
///     .with_max_results(100);
/// ```
#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    /// Filter by domain (None = all domains)
    pub domain: Option<Domain>,
    /// Minimum similarity threshold (0.0 to 1.0)
    pub min_similarity: Option<f32>,
    /// Maximum results to return (caps k)
    pub max_results: Option<usize>,
    /// Minimum distance (L2 squared) - for advanced filtering
    pub min_distance: Option<f32>,
    /// Maximum distance (L2 squared) - for advanced filtering
    pub max_distance: Option<f32>,
    /// Node IDs to exclude from results
    pub exclude_ids: Vec<i64>,
}

impl SearchFilters {
    /// Create new empty filters (no filtering applied).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by domain.
    ///
    /// Only nodes in the specified domain will be returned.
    #[must_use]
    pub fn with_domain(mut self, domain: Domain) -> Self {
        self.domain = Some(domain);
        self
    }

    /// Set minimum similarity threshold.
    ///
    /// Results below this similarity score will be filtered out.
    /// Valid range: [0.0, 1.0]
    ///
    /// # Note
    ///
    /// Similarity is computed from L2 distance for normalized vectors:
    /// similarity = 1 - (distance / 2)
    #[must_use]
    pub fn with_min_similarity(mut self, threshold: f32) -> Self {
        self.min_similarity = Some(threshold);
        self
    }

    /// Set maximum number of results.
    ///
    /// Caps the number of results returned. Applied after other filters.
    #[must_use]
    pub fn with_max_results(mut self, max: usize) -> Self {
        self.max_results = Some(max);
        self
    }

    /// Set minimum L2 squared distance.
    ///
    /// Results with distance below this will be filtered (too similar).
    /// Useful for finding diverse results.
    #[must_use]
    pub fn with_min_distance(mut self, min: f32) -> Self {
        self.min_distance = Some(min);
        self
    }

    /// Set maximum L2 squared distance.
    ///
    /// Results with distance above this will be filtered (too different).
    #[must_use]
    pub fn with_max_distance(mut self, max: f32) -> Self {
        self.max_distance = Some(max);
        self
    }

    /// Exclude specific node IDs from results.
    ///
    /// Useful for excluding the query node itself or known irrelevant nodes.
    #[must_use]
    pub fn with_exclude_ids(mut self, ids: Vec<i64>) -> Self {
        self.exclude_ids = ids;
        self
    }

    /// Add a single ID to exclusion list.
    #[must_use]
    pub fn exclude_id(mut self, id: i64) -> Self {
        self.exclude_ids.push(id);
        self
    }

    /// Validate filter parameters.
    ///
    /// Fails fast with clear error messages per AP-001.
    ///
    /// # Errors
    ///
    /// Returns `GraphError::InvalidConfig` if:
    /// - min_similarity not in [0.0, 1.0]
    /// - min_distance > max_distance
    /// - max_results is 0
    pub fn validate(&self) -> GraphResult<()> {
        if let Some(sim) = self.min_similarity {
            if !(0.0..=1.0).contains(&sim) {
                return Err(GraphError::InvalidConfig(format!(
                    "min_similarity must be in [0.0, 1.0], got {}",
                    sim
                )));
            }
        }

        if let (Some(min), Some(max)) = (self.min_distance, self.max_distance) {
            if min > max {
                return Err(GraphError::InvalidConfig(format!(
                    "min_distance ({}) cannot be greater than max_distance ({})",
                    min, max
                )));
            }
        }

        if let Some(max) = self.max_results {
            if max == 0 {
                return Err(GraphError::InvalidConfig(
                    "max_results cannot be 0".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Check if a result passes the distance filters.
    ///
    /// # Arguments
    ///
    /// * `distance` - L2 squared distance from FAISS
    ///
    /// # Returns
    ///
    /// `true` if the result passes all distance filters
    #[inline]
    pub fn passes_distance_filter(&self, distance: f32) -> bool {
        if let Some(min) = self.min_distance {
            if distance < min {
                return false;
            }
        }
        if let Some(max) = self.max_distance {
            if distance > max {
                return false;
            }
        }
        true
    }

    /// Check if a result passes the similarity filter.
    ///
    /// # Arguments
    ///
    /// * `similarity` - Cosine similarity (derived from L2 for normalized vectors)
    ///
    /// # Returns
    ///
    /// `true` if the result passes the similarity filter
    #[inline]
    pub fn passes_similarity_filter(&self, similarity: f32) -> bool {
        if let Some(min_sim) = self.min_similarity {
            if similarity < min_sim {
                return false;
            }
        }
        true
    }

    /// Check if an ID should be excluded.
    ///
    /// # Arguments
    ///
    /// * `id` - Node ID to check
    ///
    /// # Returns
    ///
    /// `true` if the ID is in the exclusion list
    #[inline]
    pub fn is_excluded(&self, id: i64) -> bool {
        self.exclude_ids.contains(&id)
    }

    /// Get the effective k value for FAISS search.
    ///
    /// Returns max_results if set, otherwise the provided default.
    #[inline]
    pub fn effective_k(&self, default_k: usize) -> usize {
        self.max_results.unwrap_or(default_k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_filters_default() {
        let filters = SearchFilters::default();
        assert!(filters.domain.is_none());
        assert!(filters.min_similarity.is_none());
        assert!(filters.max_results.is_none());
        assert!(filters.exclude_ids.is_empty());
    }

    #[test]
    fn test_search_filters_builder() {
        let filters = SearchFilters::new()
            .with_domain(Domain::Code)
            .with_min_similarity(0.75)
            .with_max_results(50)
            .with_min_distance(0.1)
            .with_max_distance(2.0)
            .exclude_id(42);

        assert_eq!(filters.domain, Some(Domain::Code));
        assert_eq!(filters.min_similarity, Some(0.75));
        assert_eq!(filters.max_results, Some(50));
        assert_eq!(filters.min_distance, Some(0.1));
        assert_eq!(filters.max_distance, Some(2.0));
        assert_eq!(filters.exclude_ids, vec![42]);
    }

    #[test]
    fn test_search_filters_validate_success() {
        let filters = SearchFilters::new()
            .with_min_similarity(0.5)
            .with_max_results(100);
        assert!(filters.validate().is_ok());
    }

    #[test]
    fn test_search_filters_validate_similarity_bounds() {
        // Valid boundaries
        assert!(SearchFilters::new()
            .with_min_similarity(0.0)
            .validate()
            .is_ok());
        assert!(SearchFilters::new()
            .with_min_similarity(1.0)
            .validate()
            .is_ok());

        // Invalid: below 0
        let result = SearchFilters::new().with_min_similarity(-0.1).validate();
        assert!(matches!(result, Err(GraphError::InvalidConfig(_))));

        // Invalid: above 1
        let result = SearchFilters::new().with_min_similarity(1.1).validate();
        assert!(matches!(result, Err(GraphError::InvalidConfig(_))));
    }

    #[test]
    fn test_search_filters_validate_distance_order() {
        // Valid: min < max
        let filters = SearchFilters::new()
            .with_min_distance(0.5)
            .with_max_distance(2.0);
        assert!(filters.validate().is_ok());

        // Invalid: min > max
        let filters = SearchFilters::new()
            .with_min_distance(2.0)
            .with_max_distance(0.5);
        let result = filters.validate();
        assert!(matches!(result, Err(GraphError::InvalidConfig(_))));
    }

    #[test]
    fn test_search_filters_validate_max_results_zero() {
        let filters = SearchFilters::new().with_max_results(0);
        let result = filters.validate();
        assert!(matches!(result, Err(GraphError::InvalidConfig(_))));
    }

    #[test]
    fn test_passes_distance_filter() {
        let filters = SearchFilters::new()
            .with_min_distance(0.5)
            .with_max_distance(2.0);

        assert!(!filters.passes_distance_filter(0.3)); // Too close
        assert!(filters.passes_distance_filter(0.5)); // Boundary
        assert!(filters.passes_distance_filter(1.0)); // In range
        assert!(filters.passes_distance_filter(2.0)); // Boundary
        assert!(!filters.passes_distance_filter(2.5)); // Too far
    }

    #[test]
    fn test_passes_similarity_filter() {
        let filters = SearchFilters::new().with_min_similarity(0.7);

        assert!(!filters.passes_similarity_filter(0.5)); // Too low
        assert!(filters.passes_similarity_filter(0.7)); // Boundary
        assert!(filters.passes_similarity_filter(0.9)); // High enough
    }

    #[test]
    fn test_is_excluded() {
        let filters = SearchFilters::new().with_exclude_ids(vec![1, 2, 3]);

        assert!(filters.is_excluded(1));
        assert!(filters.is_excluded(2));
        assert!(filters.is_excluded(3));
        assert!(!filters.is_excluded(4));
        assert!(!filters.is_excluded(0));
    }

    #[test]
    fn test_effective_k() {
        // With max_results set
        let filters = SearchFilters::new().with_max_results(50);
        assert_eq!(filters.effective_k(100), 50);

        // Without max_results, use default
        let filters = SearchFilters::new();
        assert_eq!(filters.effective_k(100), 100);
    }

    #[test]
    fn test_search_filters_clone() {
        let filters = SearchFilters::new()
            .with_domain(Domain::Research)
            .with_min_similarity(0.8);
        let cloned = filters.clone();
        assert_eq!(cloned.domain, filters.domain);
        assert_eq!(cloned.min_similarity, filters.min_similarity);
    }
}

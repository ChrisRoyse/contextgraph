//! High-level entailment query wrapper.
//!
//! Provides a cleaner API for entailment queries (IS-A hierarchy checks)
//! using the underlying entailment cone implementation.
//!
//! # Entailment Queries
//!
//! Entailment queries determine hierarchical relationships:
//! - **Ancestors**: Find concepts that are more general (contain this node in their cone)
//! - **Descendants**: Find concepts that are more specific (contained in this node's cone)
//!
//! # Performance
//!
//! - Single containment check: O(1) - angle computation
//! - Batch check (1000 pairs): <100ms
//! - BFS + filter (depth 3): <10ms
//!
//! # Constitution Reference
//!
//! - perf.latency.entailment_check: <1ms

use crate::config::HyperbolicConfig;
use crate::entailment::{
    entailment_check_batch as low_level_batch, entailment_query as low_level_query,
    entailment_score as low_level_score, is_entailed_by as low_level_is_entailed,
    lowest_common_ancestor as low_level_lca, BatchEntailmentResult, EntailmentCone,
    EntailmentDirection, EntailmentQueryParams, EntailmentResult, LcaResult,
};
use crate::error::GraphResult;
use crate::hyperbolic::PoincarePoint;
use crate::storage::GraphStorage;

/// Query for entailment relationships from a concept node.
///
/// Finds ancestors (more general concepts) or descendants (more specific concepts)
/// based on entailment cone containment in hyperbolic space.
///
/// # Arguments
///
/// * `storage` - Graph storage with hyperbolic coordinates and cones
/// * `concept` - Node ID to query from
/// * `direction` - Whether to find ancestors or descendants
///
/// # Returns
///
/// Vector of `EntailmentResult` with nodes and their membership scores.
///
/// # Example
///
/// ```ignore
/// use context_graph_graph::query::entailment::{query_entailment, EntailmentDirection};
///
/// // Find all ancestors of "Dog" (Animal, Mammal, etc.)
/// let ancestors = query_entailment(&storage, dog_id, EntailmentDirection::Ancestors)?;
///
/// // Find all descendants of "Animal" (Dog, Cat, etc.)
/// let descendants = query_entailment(&storage, animal_id, EntailmentDirection::Descendants)?;
/// ```
pub fn query_entailment(
    storage: &GraphStorage,
    concept: i64,
    direction: EntailmentDirection,
) -> GraphResult<Vec<EntailmentResult>> {
    let params = EntailmentQueryParams::default();
    query_entailment_with_params(storage, concept, direction, params)
}

/// Query for entailment relationships with custom parameters.
///
/// Allows fine-grained control over BFS depth, result limits, and
/// membership score thresholds.
///
/// # Arguments
///
/// * `storage` - Graph storage
/// * `concept` - Node ID to query from
/// * `direction` - Ancestors or descendants
/// * `params` - Custom query parameters
///
/// # Example
///
/// ```ignore
/// let params = EntailmentQueryParams::default()
///     .with_max_depth(5)
///     .with_max_results(50)
///     .with_min_score(0.8);
///
/// let results = query_entailment_with_params(&storage, node_id, direction, params)?;
/// ```
pub fn query_entailment_with_params(
    storage: &GraphStorage,
    concept: i64,
    direction: EntailmentDirection,
    params: EntailmentQueryParams,
) -> GraphResult<Vec<EntailmentResult>> {
    low_level_query(storage, concept, direction, &params)
}

/// Check if one concept is entailed by another.
///
/// A is entailed by B iff B is more general (B's cone contains A).
/// In other words: "Dog IS-A Animal" => `is_entailed(dog, animal)` = true
///
/// # Arguments
///
/// * `storage` - Graph storage
/// * `ancestor_id` - The potentially more general concept
/// * `descendant_id` - The potentially more specific concept
///
/// # Returns
///
/// `true` if descendant is contained in ancestor's cone.
///
/// # Performance
///
/// O(1) - single angle computation.
pub fn is_entailed(storage: &GraphStorage, ancestor_id: i64, descendant_id: i64) -> GraphResult<bool> {
    let config = HyperbolicConfig::default();
    low_level_is_entailed(storage, ancestor_id, descendant_id, &config)
}

/// Check if one concept is entailed by another with custom config.
pub fn is_entailed_with_config(
    storage: &GraphStorage,
    ancestor_id: i64,
    descendant_id: i64,
    config: &HyperbolicConfig,
) -> GraphResult<bool> {
    low_level_is_entailed(storage, ancestor_id, descendant_id, config)
}

/// Get the entailment membership score between two concepts.
///
/// Returns a score in [0, 1] indicating how strongly the descendant
/// is entailed by the ancestor. Higher scores mean stronger IS-A relationship.
///
/// # Arguments
///
/// * `storage` - Graph storage
/// * `ancestor_id` - The potentially more general concept
/// * `descendant_id` - The potentially more specific concept
///
/// # Returns
///
/// Membership score in [0, 1], or 0.0 if not entailed.
pub fn entailment_membership_score(
    storage: &GraphStorage,
    ancestor_id: i64,
    descendant_id: i64,
) -> GraphResult<f32> {
    let config = HyperbolicConfig::default();
    low_level_score(storage, ancestor_id, descendant_id, &config)
}

/// Get the entailment membership score with custom config.
pub fn entailment_membership_score_with_config(
    storage: &GraphStorage,
    ancestor_id: i64,
    descendant_id: i64,
    config: &HyperbolicConfig,
) -> GraphResult<f32> {
    low_level_score(storage, ancestor_id, descendant_id, config)
}

/// Batch check entailment relationships.
///
/// Efficiently checks multiple (ancestor, descendant) pairs in a single operation.
///
/// # Arguments
///
/// * `storage` - Graph storage
/// * `checks` - Vector of (ancestor_id, descendant_id) pairs to check
///
/// # Returns
///
/// Vector of `BatchEntailmentResult` with entailment status and scores.
///
/// # Performance
///
/// More efficient than calling `is_entailed` repeatedly due to batch processing.
pub fn batch_check_entailment(
    storage: &GraphStorage,
    checks: &[(i64, i64)],
) -> GraphResult<Vec<BatchEntailmentResult>> {
    let config = HyperbolicConfig::default();
    low_level_batch(storage, checks, &config)
}

/// Batch check entailment with custom config.
pub fn batch_check_entailment_with_config(
    storage: &GraphStorage,
    checks: &[(i64, i64)],
    config: &HyperbolicConfig,
) -> GraphResult<Vec<BatchEntailmentResult>> {
    low_level_batch(storage, checks, config)
}

/// Find the lowest common ancestor of two concepts.
///
/// The LCA is the most specific concept that is an ancestor of both inputs.
///
/// # Arguments
///
/// * `storage` - Graph storage
/// * `node_a` - First concept
/// * `node_b` - Second concept
///
/// # Returns
///
/// `LcaResult` containing the LCA (if found) and depths from each input.
///
/// # Example
///
/// ```ignore
/// // Find LCA of "Dog" and "Cat"
/// let result = find_lowest_common_ancestor(&storage, dog_id, cat_id)?;
/// // result.lca_id might be Some(mammal_id) or Some(animal_id)
/// ```
pub fn find_lowest_common_ancestor(
    storage: &GraphStorage,
    node_a: i64,
    node_b: i64,
) -> GraphResult<LcaResult> {
    let params = EntailmentQueryParams::default();
    low_level_lca(storage, node_a, node_b, &params)
}

/// Find the lowest common ancestor with custom parameters.
pub fn find_lowest_common_ancestor_with_params(
    storage: &GraphStorage,
    node_a: i64,
    node_b: i64,
    params: &EntailmentQueryParams,
) -> GraphResult<LcaResult> {
    low_level_lca(storage, node_a, node_b, params)
}

/// Get all direct children of a concept (depth 1 descendants).
///
/// Convenience function for getting immediate IS-A children.
pub fn get_direct_children(storage: &GraphStorage, parent_id: i64) -> GraphResult<Vec<i64>> {
    let params = EntailmentQueryParams::default()
        .with_max_depth(1)
        .with_max_results(1000);

    let results = query_entailment_with_params(
        storage,
        parent_id,
        EntailmentDirection::Descendants,
        params,
    )?;

    Ok(results
        .into_iter()
        .filter(|r| r.is_direct)
        .map(|r| r.node_id)
        .collect())
}

/// Get all direct parents of a concept (depth 1 ancestors).
///
/// Convenience function for getting immediate IS-A parents.
pub fn get_direct_parents(storage: &GraphStorage, child_id: i64) -> GraphResult<Vec<i64>> {
    let params = EntailmentQueryParams::default()
        .with_max_depth(1)
        .with_max_results(1000);

    let results =
        query_entailment_with_params(storage, child_id, EntailmentDirection::Ancestors, params)?;

    Ok(results
        .into_iter()
        .filter(|r| r.is_direct)
        .map(|r| r.node_id)
        .collect())
}

/// Get the entailment score using cone and point directly (low-level API).
///
/// Useful when you already have the cone and point loaded.
pub fn cone_membership_score(
    cone: &EntailmentCone,
    point: &PoincarePoint,
    config: &HyperbolicConfig,
) -> f32 {
    use crate::hyperbolic::PoincareBall;
    let ball = PoincareBall::new(config.clone());
    cone.membership_score(point, &ball)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entailment_direction_variants() {
        let _ancestors = EntailmentDirection::Ancestors;
        let _descendants = EntailmentDirection::Descendants;
    }

    #[test]
    fn test_entailment_query_params_builder() {
        let params = EntailmentQueryParams::default()
            .with_max_depth(5)
            .with_max_results(50)
            .with_min_score(0.8);

        assert_eq!(params.max_depth, 5);
        assert_eq!(params.max_results, 50);
        assert_eq!(params.min_membership_score, 0.8);
    }
}

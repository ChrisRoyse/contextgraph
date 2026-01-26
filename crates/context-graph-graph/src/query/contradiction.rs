//! High-level contradiction detection wrapper.
//!
//! Provides a cleaner API for detecting contradicting information
//! in the knowledge graph.
//!
//! # Contradiction Detection
//!
//! Contradictions are identified by combining:
//! - Semantic similarity search (find related nodes)
//! - Explicit CONTRADICTS edges in the graph
//! - Contradiction type classification
//!
//! # Performance
//!
//! - Detection with k=50 candidates: <50ms
//!
//! # Constitution Reference
//!
//! - edge_model.attrs: type:Contradicts
//! - AP-001: Never unwrap() in prod - FAIL FAST

use uuid::Uuid;

use crate::contradiction::{
    check_contradiction as low_level_check, contradiction_detect as low_level_detect,
    get_contradictions as low_level_get, mark_contradiction as low_level_mark,
    ContradictionParams, ContradictionResult, ContradictionType,
};
use crate::error::GraphResult;
use crate::index::FaissGpuIndex;
use crate::search::NoMetadataProvider;
use crate::storage::GraphStorage;

/// Detect contradictions for a node's embedding.
///
/// Searches for nodes that semantically contradict the given embedding,
/// combining vector similarity with explicit contradiction edges.
///
/// # Arguments
///
/// * `index` - FAISS GPU index for semantic search
/// * `storage` - Graph storage
/// * `node_embedding` - Embedding vector of the node to check
/// * `node_id` - UUID of the node to check
/// * `threshold` - Minimum confidence threshold for contradictions
///
/// # Returns
///
/// Vector of `ContradictionResult` with detected contradictions.
///
/// # Example
///
/// ```ignore
/// let contradictions = detect_contradictions(
///     &index,
///     &storage,
///     &embedding,
///     node_id,
///     0.5,  // 50% confidence threshold
/// )?;
///
/// for c in contradictions {
///     println!("Contradiction with {:?}: {:?} (confidence: {})",
///         c.contradicting_node_id, c.contradiction_type, c.confidence);
/// }
/// ```
pub fn detect_contradictions(
    index: &FaissGpuIndex,
    storage: &GraphStorage,
    node_embedding: &[f32],
    node_id: Uuid,
    threshold: f32,
) -> GraphResult<Vec<ContradictionResult>> {
    let params = ContradictionParams::default().threshold(threshold);
    detect_contradictions_with_params(index, storage, node_embedding, node_id, params)
}

/// Detect contradictions with custom parameters.
///
/// Allows fine-grained control over detection sensitivity, search depth,
/// and evidence weighting.
///
/// # Arguments
///
/// * `index` - FAISS GPU index
/// * `storage` - Graph storage
/// * `node_embedding` - Embedding of the node
/// * `node_id` - Node UUID
/// * `params` - Custom detection parameters
pub fn detect_contradictions_with_params(
    index: &FaissGpuIndex,
    storage: &GraphStorage,
    node_embedding: &[f32],
    node_id: Uuid,
    params: ContradictionParams,
) -> GraphResult<Vec<ContradictionResult>> {
    // Validate params - FAIL FAST
    params.validate()?;

    low_level_detect::<NoMetadataProvider>(index, storage, node_id, node_embedding, params, None)
}

/// Get all known contradictions for a node.
///
/// Returns contradictions that have explicit CONTRADICTS edges
/// in the graph (previously detected and stored).
///
/// # Arguments
///
/// * `storage` - Graph storage
/// * `node_id` - Node UUID to get contradictions for
///
/// # Returns
///
/// Vector of `ContradictionResult` for known contradictions.
pub fn get_known_contradictions(
    storage: &GraphStorage,
    node_id: Uuid,
) -> GraphResult<Vec<ContradictionResult>> {
    low_level_get(storage, node_id)
}

/// Check for contradiction between two specific nodes.
///
/// Looks for explicit CONTRADICTS edge between the nodes.
///
/// # Arguments
///
/// * `storage` - Graph storage
/// * `node_a` - First node UUID
/// * `node_b` - Second node UUID
///
/// # Returns
///
/// `Some(ContradictionResult)` if explicit contradiction exists,
/// `None` otherwise.
pub fn check_contradiction_between(
    storage: &GraphStorage,
    node_a: Uuid,
    node_b: Uuid,
) -> GraphResult<Option<ContradictionResult>> {
    low_level_check(storage, node_a, node_b)
}

/// Mark two nodes as contradicting each other.
///
/// Creates a CONTRADICTS edge between two nodes with the given confidence.
///
/// # Arguments
///
/// * `storage` - Graph storage
/// * `node_a` - First node UUID
/// * `node_b` - Second node UUID
/// * `contradiction_type` - Type of contradiction
/// * `confidence` - Confidence score [0, 1]
///
/// # Example
///
/// ```ignore
/// mark_as_contradicting(&storage, node_a, node_b, ContradictionType::DirectOpposition, 0.9)?;
/// ```
pub fn mark_as_contradicting(
    storage: &GraphStorage,
    node_a: Uuid,
    node_b: Uuid,
    contradiction_type: ContradictionType,
    confidence: f32,
) -> GraphResult<()> {
    low_level_mark(storage, node_a, node_b, contradiction_type, confidence)
}

/// High-sensitivity contradiction detection.
///
/// Convenience function that uses lower thresholds to catch more
/// potential contradictions (may include false positives).
pub fn detect_contradictions_sensitive(
    index: &FaissGpuIndex,
    storage: &GraphStorage,
    node_embedding: &[f32],
    node_id: Uuid,
) -> GraphResult<Vec<ContradictionResult>> {
    let params = ContradictionParams::default().high_sensitivity();
    detect_contradictions_with_params(index, storage, node_embedding, node_id, params)
}

/// Low-sensitivity contradiction detection.
///
/// Convenience function that uses higher thresholds to return only
/// high-confidence contradictions (fewer false positives).
pub fn detect_contradictions_strict(
    index: &FaissGpuIndex,
    storage: &GraphStorage,
    node_embedding: &[f32],
    node_id: Uuid,
) -> GraphResult<Vec<ContradictionResult>> {
    let params = ContradictionParams::default().low_sensitivity();
    detect_contradictions_with_params(index, storage, node_embedding, node_id, params)
}

/// Filter contradictions by type.
pub fn filter_by_type(
    contradictions: Vec<ContradictionResult>,
    contradiction_type: ContradictionType,
) -> Vec<ContradictionResult> {
    contradictions
        .into_iter()
        .filter(|c| c.contradiction_type == contradiction_type)
        .collect()
}

/// Get the most severe contradiction from a list.
pub fn most_severe(contradictions: &[ContradictionResult]) -> Option<&ContradictionResult> {
    contradictions
        .iter()
        .max_by(|a, b| a.severity().partial_cmp(&b.severity()).unwrap_or(std::cmp::Ordering::Equal))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contradiction_params_builder() {
        let params = ContradictionParams::default()
            .threshold(0.7)
            .semantic_k(100)
            .min_similarity(0.4)
            .graph_depth(3);

        assert_eq!(params.threshold, 0.7);
        assert_eq!(params.semantic_k, 100);
        assert_eq!(params.min_similarity, 0.4);
        assert_eq!(params.graph_depth, 3);
    }

    #[test]
    fn test_contradiction_params_sensitivity_presets() {
        let high = ContradictionParams::default().high_sensitivity();
        let low = ContradictionParams::default().low_sensitivity();

        assert!(high.threshold < low.threshold);
        assert!(high.semantic_k > low.semantic_k);
    }

    #[test]
    fn test_filter_by_type() {
        let contradictions = vec![
            ContradictionResult {
                contradicting_node_id: Uuid::new_v4(),
                contradiction_type: ContradictionType::DirectOpposition,
                confidence: 0.9,
                semantic_similarity: 0.8,
                edge_weight: None,
                has_explicit_edge: false,
                evidence: vec![],
            },
            ContradictionResult {
                contradicting_node_id: Uuid::new_v4(),
                contradiction_type: ContradictionType::LogicalInconsistency,
                confidence: 0.7,
                semantic_similarity: 0.6,
                edge_weight: None,
                has_explicit_edge: false,
                evidence: vec![],
            },
        ];

        let filtered = filter_by_type(contradictions, ContradictionType::DirectOpposition);
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn test_most_severe() {
        let contradictions = vec![
            ContradictionResult {
                contradicting_node_id: Uuid::new_v4(),
                contradiction_type: ContradictionType::DirectOpposition,
                confidence: 0.5,
                semantic_similarity: 0.8,
                edge_weight: None,
                has_explicit_edge: false,
                evidence: vec![],
            },
            ContradictionResult {
                contradicting_node_id: Uuid::new_v4(),
                contradiction_type: ContradictionType::DirectOpposition,
                confidence: 0.9,
                semantic_similarity: 0.6,
                edge_weight: None,
                has_explicit_edge: false,
                evidence: vec![],
            },
        ];

        let most = most_severe(&contradictions);
        assert!(most.is_some());
        assert_eq!(most.unwrap().confidence, 0.9);
    }
}

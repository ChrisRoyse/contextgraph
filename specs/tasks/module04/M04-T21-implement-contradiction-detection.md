---
id: "M04-T21"
title: "Implement Contradiction Detection"
description: |
  Implement contradiction_detect(node_id, threshold) function.
  Algorithm:
  1. Semantic search for similar nodes (k=50)
  2. Check for CONTRADICTS edges (requires M04-T26)
  3. Compute contradiction confidence based on similarity + edge weight
  4. Return ContradictionResult with node, contradiction_type, confidence.
  ContradictionType: DirectOpposition, LogicalInconsistency, TemporalConflict, CausalConflict.
layer: "surface"
status: "pending"
priority: "high"
estimated_hours: 3
sequence: 29
depends_on:
  - "M04-T18"
  - "M04-T16"
  - "M04-T26"
spec_refs:
  - "TECH-GRAPH-004 Section 8"
  - "REQ-KG-063"
files_to_create:
  - path: "crates/context-graph-graph/src/contradiction/mod.rs"
    description: "Contradiction detection module"
  - path: "crates/context-graph-graph/src/contradiction/detector.rs"
    description: "Contradiction detection algorithm"
files_to_modify:
  - path: "crates/context-graph-graph/src/lib.rs"
    description: "Add contradiction module"
test_file: "crates/context-graph-graph/tests/contradiction_tests.rs"
---

## Context

Contradiction detection identifies conflicting information in the knowledge graph. This is essential for maintaining knowledge consistency and alerting users when new information contradicts existing knowledge. The algorithm combines semantic similarity (to find potentially conflicting statements) with explicit CONTRADICTS edges (marking known contradictions).

The system classifies contradictions into four types:
- **DirectOpposition**: Direct logical negation (A vs not-A)
- **LogicalInconsistency**: Indirect logical conflict
- **TemporalConflict**: Timeline inconsistencies
- **CausalConflict**: Conflicting cause-effect relationships

## Scope

### In Scope
- `contradiction_detect()` function
- ContradictionResult struct
- Semantic similarity + explicit edge combination
- ContradictionType classification
- Confidence scoring

### Out of Scope
- Automatic contradiction resolution
- Natural language entailment for implicit contradictions
- Cross-graph contradiction detection
- Contradiction explanation/justification generation

## Definition of Done

### Signatures

```rust
// In crates/context-graph-graph/src/contradiction/mod.rs

pub mod detector;
pub use detector::*;
```

```rust
// In crates/context-graph-graph/src/contradiction/detector.rs

use std::collections::HashMap;

use crate::error::{GraphError, GraphResult};
use crate::index::gpu_index::FaissGpuIndex;
use crate::search::{semantic_search, SearchFilters};
use crate::storage::rocksdb::{GraphStorage, NodeId};
use crate::storage::edges::{EdgeType, ContradictionType, GraphEdge};
use crate::traversal::bfs::{bfs_traverse, BfsParams};
use crate::Vector1536;

/// Result from contradiction detection
#[derive(Debug, Clone)]
pub struct ContradictionResult {
    /// The node that contradicts the query node
    pub contradicting_node_id: NodeId,

    /// Type of contradiction
    pub contradiction_type: ContradictionType,

    /// Overall confidence score [0, 1]
    pub confidence: f32,

    /// Semantic similarity to query node
    pub semantic_similarity: f32,

    /// Weight of explicit CONTRADICTS edge (if exists)
    pub edge_weight: Option<f32>,

    /// Whether there's an explicit contradiction edge
    pub has_explicit_edge: bool,

    /// Evidence supporting the contradiction
    pub evidence: Vec<String>,
}

impl ContradictionResult {
    /// Check if this is a high-confidence contradiction
    pub fn is_high_confidence(&self, threshold: f32) -> bool {
        self.confidence >= threshold
    }

    /// Get severity based on type and confidence
    pub fn severity(&self) -> f32 {
        self.confidence * self.contradiction_type.severity()
    }
}

/// Parameters for contradiction detection
#[derive(Debug, Clone)]
pub struct ContradictionParams {
    /// Minimum confidence threshold
    pub threshold: f32,

    /// Number of semantic similarity candidates
    pub semantic_k: usize,

    /// Minimum semantic similarity to consider
    pub min_similarity: f32,

    /// BFS depth for graph exploration
    pub graph_depth: u32,

    /// Weight given to explicit edges vs semantic similarity
    /// Higher = more weight to explicit edges
    pub explicit_edge_weight: f32,
}

impl Default for ContradictionParams {
    fn default() -> Self {
        Self {
            threshold: 0.5,
            semantic_k: 50,
            min_similarity: 0.3,
            graph_depth: 2,
            explicit_edge_weight: 0.6,
        }
    }
}

impl ContradictionParams {
    /// Builder: set threshold
    pub fn threshold(mut self, t: f32) -> Self {
        self.threshold = t.max(0.0).min(1.0);
        self
    }

    /// Builder: set semantic k
    pub fn semantic_k(mut self, k: usize) -> Self {
        self.semantic_k = k;
        self
    }

    /// Builder: high sensitivity (lower threshold)
    pub fn high_sensitivity(self) -> Self {
        self.threshold(0.3).semantic_k(100)
    }

    /// Builder: low sensitivity (higher threshold)
    pub fn low_sensitivity(self) -> Self {
        self.threshold(0.7).semantic_k(20)
    }
}

/// Detect contradictions for a given node
///
/// Combines semantic similarity search with explicit CONTRADICTS edges
/// to find potentially conflicting knowledge.
///
/// # Algorithm
/// 1. Semantic search for similar nodes (k candidates)
/// 2. BFS to find nodes with CONTRADICTS edges
/// 3. Combine and score contradictions
/// 4. Classify contradiction types
/// 5. Filter by threshold
///
/// # Arguments
/// * `index` - FAISS GPU index for semantic search
/// * `storage` - Graph storage
/// * `node_id` - Node to check for contradictions
/// * `node_embedding` - Embedding of the query node
/// * `params` - Detection parameters
///
/// # Returns
/// * Vector of contradictions above threshold, sorted by confidence
///
/// # Example
/// ```rust
/// let contradictions = contradiction_detect(
///     &index,
///     &storage,
///     node_id,
///     &node_embedding,
///     ContradictionParams::default().threshold(0.6),
/// )?;
///
/// for c in contradictions {
///     println!("Contradiction with {} ({:?}): {:.2}",
///         c.contradicting_node_id,
///         c.contradiction_type,
///         c.confidence
///     );
/// }
/// ```
pub fn contradiction_detect(
    index: &FaissGpuIndex,
    storage: &GraphStorage,
    node_id: NodeId,
    node_embedding: &Vector1536,
    params: ContradictionParams,
) -> GraphResult<Vec<ContradictionResult>> {
    let mut candidates: HashMap<NodeId, CandidateInfo> = HashMap::new();

    // Step 1: Semantic search for similar nodes
    let semantic_results = semantic_search(
        index,
        storage,
        node_embedding,
        params.semantic_k,
        Some(SearchFilters::new()
            .min_similarity(params.min_similarity)
            .exclude(vec![node_id])),
    )?;

    for result in semantic_results {
        if result.node_id != node_id {
            candidates.insert(result.node_id, CandidateInfo {
                semantic_similarity: result.similarity,
                has_explicit_edge: false,
                edge_weight: None,
                edge_type: None,
            });
        }
    }

    // Step 2: BFS to find CONTRADICTS edges
    let bfs_result = bfs_traverse(
        storage,
        node_id,
        BfsParams::default()
            .max_depth(params.graph_depth)
            .edge_types(vec![EdgeType::Contradicts]),
    )?;

    for edge in bfs_result.edges {
        if edge.edge_type == EdgeType::Contradicts {
            let target = if edge.source == node_id { edge.target } else { edge.source };

            candidates
                .entry(target)
                .and_modify(|info| {
                    info.has_explicit_edge = true;
                    info.edge_weight = Some(edge.weight);
                    info.edge_type = Some(infer_contradiction_type(&edge));
                })
                .or_insert(CandidateInfo {
                    semantic_similarity: 0.0,
                    has_explicit_edge: true,
                    edge_weight: Some(edge.weight),
                    edge_type: Some(infer_contradiction_type(&edge)),
                });
        }
    }

    // Step 3: Score and classify contradictions
    let mut results: Vec<ContradictionResult> = Vec::new();

    for (candidate_id, info) in candidates {
        // Compute combined confidence
        let confidence = compute_confidence(&info, &params);

        if confidence >= params.threshold {
            let contradiction_type = info.edge_type
                .unwrap_or_else(|| infer_type_from_similarity(info.semantic_similarity));

            results.push(ContradictionResult {
                contradicting_node_id: candidate_id,
                contradiction_type,
                confidence,
                semantic_similarity: info.semantic_similarity,
                edge_weight: info.edge_weight,
                has_explicit_edge: info.has_explicit_edge,
                evidence: Vec::new(),  // Populated in future enhancement
            });
        }
    }

    // Sort by confidence descending
    results.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());

    Ok(results)
}

/// Internal candidate info
struct CandidateInfo {
    semantic_similarity: f32,
    has_explicit_edge: bool,
    edge_weight: Option<f32>,
    edge_type: Option<ContradictionType>,
}

/// Compute confidence score from candidate info
fn compute_confidence(info: &CandidateInfo, params: &ContradictionParams) -> f32 {
    let semantic_component = info.semantic_similarity * (1.0 - params.explicit_edge_weight);

    let edge_component = if info.has_explicit_edge {
        info.edge_weight.unwrap_or(0.5) * params.explicit_edge_weight
    } else {
        0.0
    };

    // Boost if both semantic and explicit evidence
    let combined = semantic_component + edge_component;
    let boost = if info.has_explicit_edge && info.semantic_similarity > 0.5 {
        1.2  // 20% boost for corroborating evidence
    } else {
        1.0
    };

    (combined * boost).min(1.0)
}

/// Infer contradiction type from edge metadata
fn infer_contradiction_type(edge: &GraphEdge) -> ContradictionType {
    // Look at edge domain and metadata to classify
    // For now, default to DirectOpposition for explicit edges
    ContradictionType::DirectOpposition
}

/// Infer type from semantic similarity pattern
fn infer_type_from_similarity(similarity: f32) -> ContradictionType {
    if similarity > 0.9 {
        // Very similar content with contradiction = direct opposition
        ContradictionType::DirectOpposition
    } else if similarity > 0.7 {
        ContradictionType::LogicalInconsistency
    } else {
        ContradictionType::CausalConflict
    }
}

/// Check for contradiction between two specific nodes
pub fn check_contradiction(
    storage: &GraphStorage,
    node_a: NodeId,
    node_b: NodeId,
) -> GraphResult<Option<ContradictionResult>> {
    // Check for explicit CONTRADICTS edge
    let edges_a = storage.get_adjacency(node_a)?;

    for edge in edges_a {
        if edge.edge_type == EdgeType::Contradicts && edge.target == node_b {
            return Ok(Some(ContradictionResult {
                contradicting_node_id: node_b,
                contradiction_type: infer_contradiction_type(&edge),
                confidence: edge.confidence,
                semantic_similarity: 0.0,  // Not computed for direct check
                edge_weight: Some(edge.weight),
                has_explicit_edge: true,
                evidence: Vec::new(),
            }));
        }
    }

    Ok(None)
}

/// Mark two nodes as contradicting
pub fn mark_contradiction(
    storage: &mut GraphStorage,
    node_a: NodeId,
    node_b: NodeId,
    contradiction_type: ContradictionType,
    confidence: f32,
) -> GraphResult<()> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Create bidirectional contradiction edges (symmetric)
    let edge_a_to_b = GraphEdge::contradiction(
        generate_edge_id(),
        node_a,
        node_b,
        confidence,
    );

    let edge_b_to_a = GraphEdge::contradiction(
        generate_edge_id(),
        node_b,
        node_a,
        confidence,
    );

    // Add edges
    let mut edges_a = storage.get_adjacency(node_a)?;
    edges_a.push(edge_a_to_b);
    storage.put_adjacency(node_a, &edges_a)?;

    let mut edges_b = storage.get_adjacency(node_b)?;
    edges_b.push(edge_b_to_a);
    storage.put_adjacency(node_b, &edges_b)?;

    Ok(())
}

fn generate_edge_id() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0)
}

/// Get all contradictions for a node (from storage)
pub fn get_contradictions(
    storage: &GraphStorage,
    node_id: NodeId,
) -> GraphResult<Vec<ContradictionResult>> {
    let edges = storage.get_adjacency(node_id)?;

    let results: Vec<ContradictionResult> = edges
        .iter()
        .filter(|e| e.edge_type == EdgeType::Contradicts)
        .map(|e| ContradictionResult {
            contradicting_node_id: e.target,
            contradiction_type: ContradictionType::DirectOpposition,
            confidence: e.confidence,
            semantic_similarity: 0.0,
            edge_weight: Some(e.weight),
            has_explicit_edge: true,
            evidence: Vec::new(),
        })
        .collect();

    Ok(results)
}
```

### Constraints
- Requires M04-T26 EdgeType::CONTRADICTS
- Semantic search k=50 default candidates
- Combines semantic + explicit edge evidence
- Confidence score in [0, 1]
- CONTRADICTS edges are symmetric (bidirectional)

### Acceptance Criteria
- [ ] contradiction_detect() finds semantically similar nodes
- [ ] Checks for explicit CONTRADICTS edge type
- [ ] Computes confidence score in [0,1]
- [ ] Filters by threshold
- [ ] Classifies contradiction type
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm
```
contradiction_detect(index, storage, node_id, embedding, params):
    candidates = {}

    # Step 1: Semantic search
    semantic_results = semantic_search(index, storage, embedding, k=50)
    for result in semantic_results:
        if result.node_id != node_id:
            candidates[result.node_id] = {
                semantic_similarity: result.similarity,
                has_explicit_edge: false,
            }

    # Step 2: BFS for CONTRADICTS edges
    bfs_result = bfs_traverse(storage, node_id,
                              edge_types=[CONTRADICTS], max_depth=2)
    for edge in bfs_result.edges:
        if edge.type == CONTRADICTS:
            target = edge.target if edge.source == node_id else edge.source
            if target in candidates:
                candidates[target].has_explicit_edge = true
                candidates[target].edge_weight = edge.weight
            else:
                candidates[target] = {
                    semantic_similarity: 0,
                    has_explicit_edge: true,
                    edge_weight: edge.weight,
                }

    # Step 3: Score and filter
    results = []
    for candidate_id, info in candidates:
        confidence = compute_confidence(info, params)
        if confidence >= params.threshold:
            type = classify_contradiction(info)
            results.append(ContradictionResult{...})

    results.sort_by(confidence, descending)
    return results
```

### Confidence Calculation
```
confidence = semantic_similarity * (1 - explicit_weight)
           + edge_weight * explicit_weight

If both semantic AND explicit evidence:
    confidence *= 1.2  # 20% boost

Clamp to [0, 1]
```

### Edge Cases
- No CONTRADICTS edges exist: Only semantic results
- High similarity but no edge: Potential implicit contradiction
- Explicit edge but low similarity: Trusted contradiction
- Self-contradiction: Skip node_id in results
- Cyclic contradictions: Handle via visited set in BFS

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph contradiction
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] Semantic search finds similar nodes
- [ ] Explicit edges are detected
- [ ] Confidence scores reasonable
- [ ] Type classification works

### Test Cases

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn setup_test_db_with_contradictions() -> (GraphStorage, NodeId, NodeId) {
        let dir = tempdir().unwrap();
        let mut storage = GraphStorage::open_default(dir.path()).unwrap();

        // Create two contradicting nodes
        let node_a = 1;
        let node_b = 2;

        // Add explicit contradiction edge
        let edge = GraphEdge::contradiction(100, node_a, node_b, 0.9);
        storage.put_adjacency(node_a, &[edge.clone()]).unwrap();

        let reverse_edge = GraphEdge::contradiction(101, node_b, node_a, 0.9);
        storage.put_adjacency(node_b, &[reverse_edge]).unwrap();

        (storage, node_a, node_b)
    }

    #[test]
    fn test_check_contradiction_explicit() {
        let (storage, node_a, node_b) = setup_test_db_with_contradictions();

        let result = check_contradiction(&storage, node_a, node_b).unwrap();

        assert!(result.is_some());
        let contradiction = result.unwrap();
        assert_eq!(contradiction.contradicting_node_id, node_b);
        assert!(contradiction.has_explicit_edge);
        assert!((contradiction.confidence - 0.9).abs() < 1e-6);
    }

    #[test]
    fn test_get_contradictions() {
        let (storage, node_a, _) = setup_test_db_with_contradictions();

        let results = get_contradictions(&storage, node_a).unwrap();

        assert!(!results.is_empty());
        assert!(results[0].has_explicit_edge);
    }

    #[test]
    fn test_contradiction_params_builder() {
        let params = ContradictionParams::default()
            .threshold(0.7)
            .semantic_k(100);

        assert!((params.threshold - 0.7).abs() < 1e-6);
        assert_eq!(params.semantic_k, 100);
    }

    #[test]
    fn test_high_sensitivity() {
        let params = ContradictionParams::default().high_sensitivity();

        assert!(params.threshold < 0.5);
        assert!(params.semantic_k > 50);
    }

    #[test]
    fn test_contradiction_result_severity() {
        let result = ContradictionResult {
            contradicting_node_id: 42,
            contradiction_type: ContradictionType::DirectOpposition,
            confidence: 0.8,
            semantic_similarity: 0.9,
            edge_weight: Some(0.85),
            has_explicit_edge: true,
            evidence: vec![],
        };

        // DirectOpposition has severity 1.0
        // severity = 0.8 * 1.0 = 0.8
        assert!((result.severity() - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_confidence_boost_for_dual_evidence() {
        let info_explicit_only = CandidateInfo {
            semantic_similarity: 0.2,
            has_explicit_edge: true,
            edge_weight: Some(0.8),
            edge_type: None,
        };

        let info_both = CandidateInfo {
            semantic_similarity: 0.8,
            has_explicit_edge: true,
            edge_weight: Some(0.8),
            edge_type: None,
        };

        let params = ContradictionParams::default();

        let conf_single = compute_confidence(&info_explicit_only, &params);
        let conf_both = compute_confidence(&info_both, &params);

        // Dual evidence should have higher confidence
        assert!(conf_both > conf_single);
    }
}
```

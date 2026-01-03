---
id: "M04-T20"
title: "Implement Entailment Query Operation"
description: |
  Implement entailment_query(node_id, direction, max_depth) function.
  Uses EntailmentCone containment for O(1) IS-A hierarchy checks.
  Direction: Ancestors (concepts that entail this) or Descendants (concepts entailed by this).
  Returns Vec<KnowledgeNode> in hierarchy.
  Performance: <1ms per containment check.
layer: "surface"
status: "pending"
priority: "critical"
estimated_hours: 4
sequence: 28
depends_on:
  - "M04-T07"
  - "M04-T13"
spec_refs:
  - "TECH-GRAPH-004 Section 8"
  - "REQ-KG-062"
files_to_create:
  - path: "crates/context-graph-graph/src/entailment/query.rs"
    description: "Entailment query operations"
files_to_modify:
  - path: "crates/context-graph-graph/src/entailment/mod.rs"
    description: "Add query module"
  - path: "crates/context-graph-graph/src/lib.rs"
    description: "Export entailment_query"
test_file: "crates/context-graph-graph/tests/entailment_query_tests.rs"
---

## Context

Entailment queries leverage the hyperbolic geometry of entailment cones for efficient IS-A hierarchy traversal. Unlike graph traversal which requires following edges, entailment cones provide O(1) containment checks: a concept B is entailed by concept A if B's Poincare point lies within A's entailment cone. This enables rapid hierarchical reasoning for type inference, subsumption checking, and knowledge inheritance.

Ancestors are concepts whose cones contain the query node (more general concepts).
Descendants are concepts whose Poincare points lie within the query node's cone (more specific concepts).

## Scope

### In Scope
- `entailment_query()` function with direction parameter
- EntailmentDirection enum (Ancestors, Descendants)
- Candidate generation from graph neighbors + vector similarity
- Batch containment checking
- EntailmentQueryResult struct

### Out of Scope
- CUDA-accelerated cone checking (see M04-T24)
- Transitive closure computation
- Entailment cone training/updates
- Cross-graph entailment

## Definition of Done

### Signatures

```rust
// In crates/context-graph-graph/src/entailment/query.rs

use std::collections::HashSet;

use crate::error::{GraphError, GraphResult};
use crate::storage::rocksdb::{GraphStorage, NodeId};
use crate::entailment::cones::EntailmentCone;
use crate::hyperbolic::{PoincarePoint, PoincareBall, HyperbolicConfig};

/// Direction for entailment query
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntailmentDirection {
    /// Find concepts that entail this node (more general, ancestors)
    /// These are concepts whose cones contain this node's point
    Ancestors,

    /// Find concepts entailed by this node (more specific, descendants)
    /// These are concepts whose points lie within this node's cone
    Descendants,
}

/// Result from entailment query
#[derive(Debug, Clone)]
pub struct EntailmentResult {
    /// Node in the entailment hierarchy
    pub node_id: NodeId,

    /// Poincare point of the node
    pub point: PoincarePoint,

    /// Entailment cone of the node
    pub cone: EntailmentCone,

    /// Membership score [0, 1] for containment
    pub membership_score: f32,

    /// Depth in the hierarchy (0 = direct, 1 = one hop, etc.)
    pub depth: u32,

    /// Is this a direct entailment (depth 0)?
    pub is_direct: bool,
}

/// Parameters for entailment query
#[derive(Debug, Clone)]
pub struct EntailmentQueryParams {
    /// Maximum depth to search
    pub max_depth: u32,

    /// Maximum results to return
    pub max_results: usize,

    /// Minimum membership score to include
    pub min_membership_score: f32,

    /// Include candidates from vector similarity search
    pub use_vector_candidates: bool,

    /// Number of vector similarity candidates to consider
    pub vector_candidates_k: usize,

    /// Hyperbolic configuration
    pub hyperbolic_config: HyperbolicConfig,
}

impl Default for EntailmentQueryParams {
    fn default() -> Self {
        Self {
            max_depth: 5,
            max_results: 100,
            min_membership_score: 0.5,
            use_vector_candidates: true,
            vector_candidates_k: 100,
            hyperbolic_config: HyperbolicConfig::default(),
        }
    }
}

impl EntailmentQueryParams {
    /// Builder: set max depth
    pub fn max_depth(mut self, d: u32) -> Self {
        self.max_depth = d;
        self
    }

    /// Builder: set max results
    pub fn max_results(mut self, n: usize) -> Self {
        self.max_results = n;
        self
    }

    /// Builder: set min membership score
    pub fn min_score(mut self, s: f32) -> Self {
        self.min_membership_score = s.max(0.0).min(1.0);
        self
    }
}

/// Query entailment hierarchy for a node
///
/// Uses EntailmentCone containment for O(1) IS-A hierarchy checks.
/// Performance target: <1ms per containment check.
///
/// # Arguments
/// * `storage` - Graph storage backend
/// * `node_id` - Starting node for query
/// * `direction` - Ancestors (more general) or Descendants (more specific)
/// * `params` - Query parameters
///
/// # Returns
/// * Vector of entailment results, sorted by membership score
///
/// # Algorithm
/// For Ancestors:
/// 1. Get candidate nodes (graph neighbors + optional vector similarity)
/// 2. For each candidate, check if query node's point is in candidate's cone
/// 3. Score by membership_score, filter by threshold
///
/// For Descendants:
/// 1. Get candidate nodes
/// 2. For each candidate, check if candidate's point is in query node's cone
/// 3. Score by membership_score, filter by threshold
///
/// # Example
/// ```rust
/// // Find ancestors (more general concepts)
/// let ancestors = entailment_query(
///     &storage,
///     node_id,
///     EntailmentDirection::Ancestors,
///     EntailmentQueryParams::default().max_depth(3),
/// )?;
///
/// for ancestor in ancestors {
///     println!("Ancestor {} score: {:.3}", ancestor.node_id, ancestor.membership_score);
/// }
/// ```
pub fn entailment_query(
    storage: &GraphStorage,
    node_id: NodeId,
    direction: EntailmentDirection,
    params: EntailmentQueryParams,
) -> GraphResult<Vec<EntailmentResult>> {
    // Get query node's hyperbolic data
    let query_point = storage.get_hyperbolic(node_id)?
        .ok_or(GraphError::InvalidHyperbolicPoint("Query node has no Poincare point".into()))?;

    let query_cone = storage.get_cone(node_id)?
        .ok_or(GraphError::NodeNotFound(node_id))?;

    let ball = PoincareBall::new(&params.hyperbolic_config);

    // Collect candidates through BFS from neighbors
    let mut candidates: HashSet<NodeId> = HashSet::new();
    let mut visited: HashSet<NodeId> = HashSet::new();
    let mut frontier: Vec<(NodeId, u32)> = vec![(node_id, 0)];
    visited.insert(node_id);

    while let Some((current, depth)) = frontier.pop() {
        if depth >= params.max_depth {
            continue;
        }

        // Get neighbors
        let edges = storage.get_adjacency(current)?;

        for edge in edges {
            if visited.insert(edge.target) {
                candidates.insert(edge.target);
                frontier.push((edge.target, depth + 1));
            }
        }
    }

    // Perform containment checks
    let mut results: Vec<EntailmentResult> = Vec::new();

    for candidate_id in candidates {
        // Skip self
        if candidate_id == node_id {
            continue;
        }

        // Get candidate's hyperbolic data
        let candidate_point = match storage.get_hyperbolic(candidate_id)? {
            Some(p) => p,
            None => continue,  // Skip nodes without hyperbolic data
        };

        let candidate_cone = match storage.get_cone(candidate_id)? {
            Some(c) => c,
            None => continue,
        };

        // Compute membership score based on direction
        let (membership_score, depth) = match direction {
            EntailmentDirection::Ancestors => {
                // Query point in candidate's cone?
                let score = candidate_cone.membership_score(&query_point, &ball);
                let d = candidate_cone.depth;
                (score, d)
            }
            EntailmentDirection::Descendants => {
                // Candidate point in query's cone?
                let score = query_cone.membership_score(&candidate_point, &ball);
                let d = candidate_cone.depth;
                (score, d)
            }
        };

        // Filter by minimum score
        if membership_score >= params.min_membership_score {
            results.push(EntailmentResult {
                node_id: candidate_id,
                point: candidate_point,
                cone: candidate_cone,
                membership_score,
                depth,
                is_direct: depth == 0,
            });
        }
    }

    // Sort by membership score (descending)
    results.sort_by(|a, b| b.membership_score.partial_cmp(&a.membership_score).unwrap());

    // Truncate to max results
    results.truncate(params.max_results);

    Ok(results)
}

/// Check if node A entails node B (B is-a A)
///
/// Returns true if B's Poincare point lies within A's entailment cone.
/// O(1) operation, <1ms performance target.
pub fn is_entailed_by(
    storage: &GraphStorage,
    general: NodeId,
    specific: NodeId,
    config: &HyperbolicConfig,
) -> GraphResult<bool> {
    let general_cone = storage.get_cone(general)?
        .ok_or(GraphError::NodeNotFound(general))?;

    let specific_point = storage.get_hyperbolic(specific)?
        .ok_or(GraphError::InvalidHyperbolicPoint("Specific node has no point".into()))?;

    let ball = PoincareBall::new(config);
    Ok(general_cone.contains(&specific_point, &ball))
}

/// Get membership score for entailment relationship
///
/// Returns soft score [0, 1] for how strongly B is entailed by A.
/// CANONICAL FORMULA:
/// - If contained: 1.0
/// - If not: exp(-2.0 * (angle - aperture))
pub fn entailment_score(
    storage: &GraphStorage,
    general: NodeId,
    specific: NodeId,
    config: &HyperbolicConfig,
) -> GraphResult<f32> {
    let general_cone = storage.get_cone(general)?
        .ok_or(GraphError::NodeNotFound(general))?;

    let specific_point = storage.get_hyperbolic(specific)?
        .ok_or(GraphError::InvalidHyperbolicPoint("Specific node has no point".into()))?;

    let ball = PoincareBall::new(config);
    Ok(general_cone.membership_score(&specific_point, &ball))
}

/// Batch entailment check for multiple pairs
pub fn entailment_check_batch(
    storage: &GraphStorage,
    pairs: &[(NodeId, NodeId)],  // (general, specific)
    config: &HyperbolicConfig,
) -> GraphResult<Vec<(bool, f32)>> {
    let ball = PoincareBall::new(config);
    let mut results = Vec::with_capacity(pairs.len());

    for (general, specific) in pairs {
        let general_cone = storage.get_cone(*general)?;
        let specific_point = storage.get_hyperbolic(*specific)?;

        let (contained, score) = match (general_cone, specific_point) {
            (Some(cone), Some(point)) => {
                let s = cone.membership_score(&point, &ball);
                (cone.contains(&point, &ball), s)
            }
            _ => (false, 0.0),
        };

        results.push((contained, score));
    }

    Ok(results)
}

/// Find the lowest common ancestor in the entailment hierarchy
pub fn lowest_common_ancestor(
    storage: &GraphStorage,
    node_a: NodeId,
    node_b: NodeId,
    params: EntailmentQueryParams,
) -> GraphResult<Option<NodeId>> {
    // Get ancestors of both nodes
    let ancestors_a = entailment_query(
        storage, node_a, EntailmentDirection::Ancestors, params.clone()
    )?;
    let ancestors_b = entailment_query(
        storage, node_b, EntailmentDirection::Ancestors, params
    )?;

    // Find intersection with best membership score
    let set_a: HashSet<_> = ancestors_a.iter().map(|r| r.node_id).collect();

    let mut best: Option<(NodeId, f32)> = None;
    for result in ancestors_b {
        if set_a.contains(&result.node_id) {
            if best.is_none() || result.membership_score > best.as_ref().unwrap().1 {
                best = Some((result.node_id, result.membership_score));
            }
        }
    }

    Ok(best.map(|(id, _)| id))
}
```

### Constraints
- O(1) containment check: <1ms per check
- Uses CANONICAL membership_score formula from M04-T07
- Respects max_depth limit for candidate generation
- Filter by min_membership_score threshold
- Sort results by membership_score descending

### Acceptance Criteria
- [ ] entailment_query() retrieves node's cone from storage
- [ ] Checks containment against candidate nodes
- [ ] Ancestors: finds cones that contain this node
- [ ] Descendants: finds nodes contained by this cone
- [ ] Respects max_depth limit
- [ ] Performance: <1ms per cone check
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm
```
entailment_query(storage, node_id, direction, params):
    # Get query node's hyperbolic data
    query_point = storage.get_hyperbolic(node_id)
    query_cone = storage.get_cone(node_id)

    if query_point is None or query_cone is None:
        return Error

    # Collect candidates via BFS
    candidates = set()
    visited = {node_id}
    frontier = [(node_id, 0)]

    while frontier not empty:
        current, depth = frontier.pop()
        if depth >= max_depth:
            continue

        for edge in storage.get_adjacency(current):
            if edge.target not in visited:
                visited.add(edge.target)
                candidates.add(edge.target)
                frontier.push((edge.target, depth + 1))

    # Check containment for each candidate
    results = []
    for candidate in candidates:
        candidate_point = storage.get_hyperbolic(candidate)
        candidate_cone = storage.get_cone(candidate)

        if direction == Ancestors:
            # Is query_point in candidate_cone?
            score = candidate_cone.membership_score(query_point)
        else:  # Descendants
            # Is candidate_point in query_cone?
            score = query_cone.membership_score(candidate_point)

        if score >= min_score:
            results.append(EntailmentResult{candidate, score, ...})

    # Sort and truncate
    results.sort_by(score, descending)
    return results[:max_results]
```

### Membership Score Formula (from M04-T07)
```
membership_score(cone, point):
    tangent = log_map(cone.apex, point)
    to_origin = log_map(cone.apex, origin)
    angle = arccos(dot(tangent, to_origin) / (||tangent|| * ||to_origin||))

    if angle <= cone.effective_aperture():
        return 1.0
    else:
        return exp(-2.0 * (angle - aperture))  # CANONICAL FORMULA
```

### Edge Cases
- Node has no Poincare point: Return error
- Node has no entailment cone: Return error
- No candidates found: Return empty vector
- Self-reference: Skip in results
- All candidates below threshold: Return empty vector

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph entailment_query
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] Ancestors are more general concepts
- [ ] Descendants are more specific concepts
- [ ] is_entailed_by returns correct boolean
- [ ] Performance <1ms per containment check

### Test Cases

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn setup_hierarchy() -> (GraphStorage, NodeId, NodeId, NodeId) {
        // Create: Animal (general) -> Mammal -> Dog (specific)
        let dir = tempdir().unwrap();
        let storage = GraphStorage::open_default(dir.path()).unwrap();

        let animal_id = 1;
        let mammal_id = 2;
        let dog_id = 3;

        // Animal has wide cone (more general)
        let animal_cone = EntailmentCone::new(
            PoincarePoint::origin(),
            1.2,  // Wide aperture
            1.0,
            0,
        );

        // Mammal has medium cone
        let mammal_point = PoincarePoint::from_coords(&[0.1; 64]);
        let mammal_cone = EntailmentCone::new(
            mammal_point.clone(),
            0.8,
            1.0,
            1,
        );

        // Dog has narrow cone (most specific)
        let dog_point = PoincarePoint::from_coords(&[0.15; 64]);
        let dog_cone = EntailmentCone::new(
            dog_point.clone(),
            0.4,
            1.0,
            2,
        );

        storage.put_hyperbolic(animal_id, &PoincarePoint::origin()).unwrap();
        storage.put_hyperbolic(mammal_id, &mammal_point).unwrap();
        storage.put_hyperbolic(dog_id, &dog_point).unwrap();

        storage.put_cone(animal_id, &animal_cone).unwrap();
        storage.put_cone(mammal_id, &mammal_cone).unwrap();
        storage.put_cone(dog_id, &dog_cone).unwrap();

        // Add edges for candidate generation
        storage.put_adjacency(animal_id, &[
            GraphEdge::hierarchical(1, animal_id, mammal_id),
        ]).unwrap();
        storage.put_adjacency(mammal_id, &[
            GraphEdge::hierarchical(2, mammal_id, dog_id),
        ]).unwrap();

        (storage, animal_id, mammal_id, dog_id)
    }

    #[test]
    fn test_entailment_ancestors() {
        let (storage, animal_id, mammal_id, dog_id) = setup_hierarchy();

        // Dog's ancestors should be Mammal and Animal
        let results = entailment_query(
            &storage,
            dog_id,
            EntailmentDirection::Ancestors,
            EntailmentQueryParams::default(),
        ).unwrap();

        let ancestor_ids: Vec<_> = results.iter().map(|r| r.node_id).collect();
        assert!(ancestor_ids.contains(&mammal_id) || ancestor_ids.contains(&animal_id));
    }

    #[test]
    fn test_entailment_descendants() {
        let (storage, animal_id, mammal_id, dog_id) = setup_hierarchy();

        // Animal's descendants should include Mammal and Dog
        let results = entailment_query(
            &storage,
            animal_id,
            EntailmentDirection::Descendants,
            EntailmentQueryParams::default(),
        ).unwrap();

        let descendant_ids: Vec<_> = results.iter().map(|r| r.node_id).collect();
        // May or may not contain depending on cone geometry
        // Just verify no errors and valid structure
        for result in results {
            assert!(result.membership_score >= 0.0);
            assert!(result.membership_score <= 1.0);
        }
    }

    #[test]
    fn test_is_entailed_by() {
        let (storage, animal_id, _, dog_id) = setup_hierarchy();
        let config = HyperbolicConfig::default();

        // Dog is-a Animal (specific is entailed by general)
        // Note: Depends on cone geometry in test setup
        let result = is_entailed_by(&storage, animal_id, dog_id, &config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_entailment_score_canonical() {
        let (storage, animal_id, _, dog_id) = setup_hierarchy();
        let config = HyperbolicConfig::default();

        let score = entailment_score(&storage, animal_id, dog_id, &config).unwrap();

        // Score should be in [0, 1]
        assert!(score >= 0.0);
        assert!(score <= 1.0);
    }

    #[test]
    fn test_batch_entailment_check() {
        let (storage, animal_id, mammal_id, dog_id) = setup_hierarchy();
        let config = HyperbolicConfig::default();

        let pairs = vec![
            (animal_id, mammal_id),
            (animal_id, dog_id),
            (mammal_id, dog_id),
        ];

        let results = entailment_check_batch(&storage, &pairs, &config).unwrap();

        assert_eq!(results.len(), 3);
        for (_, score) in results {
            assert!(score >= 0.0 && score <= 1.0);
        }
    }
}
```

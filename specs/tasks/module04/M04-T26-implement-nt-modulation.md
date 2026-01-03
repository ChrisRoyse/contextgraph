---
id: "M04-T26"
title: "Add EdgeType::CONTRADICTS Variant"
description: |
  Add CONTRADICTS variant to EdgeType enum.
  This is required for contradiction detection in M04-T21.
  EdgeType should now have: Semantic, Temporal, Causal, Hierarchical, Contradicts.
  Update any match statements to handle the new variant.
layer: "logic"
status: "pending"
priority: "high"
estimated_hours: 1
sequence: 25
depends_on:
  - "M04-T14a"
  - "M04-T15"
spec_refs:
  - "TECH-GRAPH-004 Section 4.1"
  - "REQ-KG-063"
files_to_create: []
files_to_modify:
  - path: "crates/context-graph-graph/src/storage/edges.rs"
    description: "Add Contradicts variant to EdgeType enum"
test_file: "crates/context-graph-graph/tests/edge_tests.rs"
---

## Context

The Contradicts edge type represents logical opposition between knowledge nodes. This is essential for contradiction detection (M04-T21) where the system identifies conflicting information in the knowledge graph. By explicitly modeling contradictions as edges, we enable efficient conflict detection during search and retrieval operations.

## Scope

### In Scope
- Add Contradicts variant to EdgeType enum
- Update all() method to include Contradicts
- Update is_directional() and any other methods
- Ensure Serde serialization works
- Add helper methods for contradiction edges

### Out of Scope
- Contradiction detection algorithm (see M04-T21)
- Automatic contradiction inference
- Contradiction resolution

## Definition of Done

### Signatures

```rust
// Updates to crates/context-graph-graph/src/storage/edges.rs

/// Edge type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EdgeType {
    /// Semantic similarity relationship
    Semantic,
    /// Temporal sequence relationship
    Temporal,
    /// Causal relationship (A causes B)
    Causal,
    /// Hierarchical IS-A relationship
    Hierarchical,
    /// Contradiction relationship (A contradicts B)
    Contradicts,
}

impl EdgeType {
    /// Get all edge type variants
    pub fn all() -> &'static [EdgeType] {
        &[
            EdgeType::Semantic,
            EdgeType::Temporal,
            EdgeType::Causal,
            EdgeType::Hierarchical,
            EdgeType::Contradicts,
        ]
    }

    /// Check if this edge type represents a hierarchy
    pub fn is_hierarchical(&self) -> bool {
        matches!(self, EdgeType::Hierarchical)
    }

    /// Check if this edge type is directional
    pub fn is_directional(&self) -> bool {
        matches!(
            self,
            EdgeType::Causal | EdgeType::Temporal | EdgeType::Hierarchical
        )
    }

    /// Check if this edge type represents a contradiction
    pub fn is_contradiction(&self) -> bool {
        matches!(self, EdgeType::Contradicts)
    }

    /// Check if this edge type is symmetric (undirected)
    pub fn is_symmetric(&self) -> bool {
        matches!(self, EdgeType::Semantic | EdgeType::Contradicts)
    }

    /// Get the semantic meaning of this edge type
    pub fn description(&self) -> &'static str {
        match self {
            EdgeType::Semantic => "semantically similar to",
            EdgeType::Temporal => "occurs before/after",
            EdgeType::Causal => "causes",
            EdgeType::Hierarchical => "is a type of",
            EdgeType::Contradicts => "contradicts",
        }
    }

    /// Get the reverse description (for target -> source)
    pub fn reverse_description(&self) -> &'static str {
        match self {
            EdgeType::Semantic => "semantically similar to",
            EdgeType::Temporal => "occurs after/before",
            EdgeType::Causal => "is caused by",
            EdgeType::Hierarchical => "has subtypes including",
            EdgeType::Contradicts => "contradicts",  // symmetric
        }
    }
}

impl std::fmt::Display for EdgeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EdgeType::Semantic => write!(f, "Semantic"),
            EdgeType::Temporal => write!(f, "Temporal"),
            EdgeType::Causal => write!(f, "Causal"),
            EdgeType::Hierarchical => write!(f, "Hierarchical"),
            EdgeType::Contradicts => write!(f, "Contradicts"),
        }
    }
}

// ========== GraphEdge Extensions for Contradictions ==========

impl GraphEdge {
    /// Create a contradiction edge between two nodes
    ///
    /// Contradiction edges are symmetric (A contradicts B implies B contradicts A)
    /// and should be created in both directions for efficient lookup.
    ///
    /// # Arguments
    /// * `id` - Unique edge identifier
    /// * `source` - First contradicting node
    /// * `target` - Second contradicting node
    /// * `confidence` - How confident we are in the contradiction [0, 1]
    pub fn contradiction(id: EdgeId, source: NodeId, target: NodeId, confidence: f32) -> Self {
        let mut edge = Self::new(
            id,
            source,
            target,
            EdgeType::Contradicts,
            confidence,  // Use confidence as weight
            Domain::General,
        );
        edge.confidence = confidence;

        // Contradiction edges use inhibitory-heavy NT weights
        // to reduce retrieval of contradicting information
        edge.neurotransmitter_weights = NeurotransmitterWeights::new(
            0.3,  // Low excitatory
            0.8,  // High inhibitory
            0.1,  // Low modulatory
        );

        edge
    }

    /// Check if this edge represents a contradiction
    pub fn is_contradiction(&self) -> bool {
        self.edge_type.is_contradiction()
    }

    /// Get contradiction confidence (for contradiction edges)
    ///
    /// Returns confidence score for contradiction edges, 0.0 for others
    pub fn contradiction_confidence(&self) -> f32 {
        if self.is_contradiction() {
            self.confidence
        } else {
            0.0
        }
    }
}

// ========== Contradiction Types ==========

/// Classification of contradiction types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContradictionType {
    /// Direct logical opposition (A vs not-A)
    DirectOpposition,
    /// Logical inconsistency (A implies B, but B is false)
    LogicalInconsistency,
    /// Temporal conflict (event order inconsistency)
    TemporalConflict,
    /// Causal conflict (conflicting cause-effect chains)
    CausalConflict,
}

impl ContradictionType {
    /// Get description of this contradiction type
    pub fn description(&self) -> &'static str {
        match self {
            ContradictionType::DirectOpposition => "Direct logical opposition",
            ContradictionType::LogicalInconsistency => "Logical inconsistency",
            ContradictionType::TemporalConflict => "Temporal sequence conflict",
            ContradictionType::CausalConflict => "Causal relationship conflict",
        }
    }

    /// Get severity weight for this type (for prioritization)
    pub fn severity(&self) -> f32 {
        match self {
            ContradictionType::DirectOpposition => 1.0,
            ContradictionType::LogicalInconsistency => 0.8,
            ContradictionType::CausalConflict => 0.6,
            ContradictionType::TemporalConflict => 0.4,
        }
    }
}

impl Default for ContradictionType {
    fn default() -> Self {
        ContradictionType::DirectOpposition
    }
}

impl std::fmt::Display for ContradictionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

/// Extended edge with contradiction metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContradictionEdge {
    /// Base edge data
    pub edge: GraphEdge,
    /// Type of contradiction
    pub contradiction_type: ContradictionType,
    /// Evidence supporting the contradiction
    pub evidence: Vec<String>,
    /// When the contradiction was detected
    pub detected_at: u64,
}

impl ContradictionEdge {
    /// Create a new contradiction edge
    pub fn new(
        id: EdgeId,
        source: NodeId,
        target: NodeId,
        contradiction_type: ContradictionType,
        confidence: f32,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            edge: GraphEdge::contradiction(id, source, target, confidence),
            contradiction_type,
            evidence: Vec::new(),
            detected_at: now,
        }
    }

    /// Add evidence for this contradiction
    pub fn add_evidence(&mut self, evidence: String) {
        self.evidence.push(evidence);
    }

    /// Get combined severity (confidence * type severity)
    pub fn combined_severity(&self) -> f32 {
        self.edge.confidence * self.contradiction_type.severity()
    }
}
```

### Constraints
- EdgeType::Contradicts MUST be added to enum
- all() MUST include Contradicts
- is_contradiction() returns true only for Contradicts
- Contradiction edges use inhibitory-heavy NT weights
- Serde serialization must work for new variant

### Acceptance Criteria
- [ ] EdgeType::Contradicts variant exists
- [ ] All match statements handle Contradicts
- [ ] Serde serialization works for Contradicts
- [ ] Documentation updated
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm
1. Add Contradicts to EdgeType enum
2. Update all() to return 5 variants
3. Add is_contradiction() method
4. Add is_symmetric() method
5. Update Display impl
6. Add GraphEdge::contradiction() factory
7. Define ContradictionType enum
8. Define ContradictionEdge struct

### Edge Cases
- Serializing old data without Contradicts: Should fail fast
- Self-contradiction (node contradicts itself): Allow but unusual
- Symmetric handling: A contradicts B should imply B contradicts A

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph edge_type
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] EdgeType::all() returns 5 variants
- [ ] Contradicts serializes to "Contradicts"
- [ ] is_contradiction() works correctly
- [ ] is_symmetric() returns true for Contradicts

### Test Cases

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_type_all_includes_contradicts() {
        let all = EdgeType::all();
        assert_eq!(all.len(), 5);
        assert!(all.contains(&EdgeType::Contradicts));
    }

    #[test]
    fn test_is_contradiction() {
        assert!(EdgeType::Contradicts.is_contradiction());
        assert!(!EdgeType::Semantic.is_contradiction());
        assert!(!EdgeType::Hierarchical.is_contradiction());
    }

    #[test]
    fn test_is_symmetric() {
        assert!(EdgeType::Semantic.is_symmetric());
        assert!(EdgeType::Contradicts.is_symmetric());
        assert!(!EdgeType::Causal.is_symmetric());
        assert!(!EdgeType::Hierarchical.is_symmetric());
    }

    #[test]
    fn test_contradiction_edge_factory() {
        let edge = GraphEdge::contradiction(1, 100, 200, 0.9);

        assert_eq!(edge.edge_type, EdgeType::Contradicts);
        assert!((edge.confidence - 0.9).abs() < 1e-6);
        assert!(edge.is_contradiction());

        // Should have inhibitory-heavy NT weights
        assert!(edge.neurotransmitter_weights.inhibitory > 0.5);
    }

    #[test]
    fn test_contradiction_edge_nt_weights() {
        let edge = GraphEdge::contradiction(1, 100, 200, 0.9);

        // Contradiction edges should suppress retrieval
        let activation = edge.neurotransmitter_weights.net_activation();
        // 0.3 - 0.8 + (0.1 * 0.5) = -0.45
        assert!(activation < 0.0);  // Negative activation
    }

    #[test]
    fn test_serde_contradicts() {
        let edge = GraphEdge::contradiction(1, 100, 200, 0.9);

        let json = serde_json::to_string(&edge).unwrap();
        assert!(json.contains("Contradicts"));

        let loaded: GraphEdge = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.edge_type, EdgeType::Contradicts);
    }

    #[test]
    fn test_contradiction_type() {
        assert_eq!(ContradictionType::DirectOpposition.severity(), 1.0);
        assert!(ContradictionType::TemporalConflict.severity() < 1.0);
    }

    #[test]
    fn test_contradiction_edge_struct() {
        let mut ce = ContradictionEdge::new(
            1, 100, 200,
            ContradictionType::DirectOpposition,
            0.95,
        );

        ce.add_evidence("Statement A says X".to_string());
        ce.add_evidence("Statement B says not-X".to_string());

        assert_eq!(ce.evidence.len(), 2);
        assert!((ce.combined_severity() - 0.95).abs() < 1e-6);
    }
}
```

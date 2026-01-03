---
id: "M04-T15"
title: "Implement GraphEdge with Marblestone Fields"
description: |
  Implement GraphEdge struct with full Marblestone support.
  Fields: id, source, target, edge_type, weight, confidence, domain,
  neurotransmitter_weights, is_amortized_shortcut, steering_reward,
  traversal_count, created_at, last_traversed_at.
  Include get_modulated_weight(query_domain) method.

  CANONICAL FORMULA for get_modulated_weight():
  w_eff = base_weight * (1.0 + net_activation + domain_bonus) * steering_factor
  Result clamped to [0.0, 1.0]
layer: "logic"
status: "pending"
priority: "critical"
estimated_hours: 3
sequence: 21
depends_on:
  - "M04-T14a"
spec_refs:
  - "TECH-GRAPH-004 Section 4.1"
  - "REQ-KG-040 through REQ-KG-044, REQ-KG-065"
files_to_create: []
files_to_modify:
  - path: "crates/context-graph-graph/src/storage/edges.rs"
    description: "Add GraphEdge struct and EdgeType enum"
test_file: "crates/context-graph-graph/tests/edge_tests.rs"
---

## Context

GraphEdge represents connections in the knowledge graph with full Marblestone neuroscience-inspired modulation support. Beyond basic source/target relationships, edges carry domain information, neurotransmitter weights, and traversal statistics that enable adaptive, domain-aware search. The get_modulated_weight() method is the core of dynamic edge importance calculation.

## Scope

### In Scope
- GraphEdge struct with 13 fields
- EdgeType enum (Semantic, Temporal, Causal, Hierarchical)
- get_modulated_weight() using canonical formula
- record_traversal() for EMA steering reward updates
- Serde serialization for RocksDB storage

### Out of Scope
- EdgeType::Contradicts (see M04-T26)
- Storage operations (see M04-T13)
- Traversal algorithms (see M04-T16, M04-T17)

## Definition of Done

### Signatures

```rust
// Add to crates/context-graph-graph/src/storage/edges.rs

use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use super::NodeId;
use super::{Domain, NeurotransmitterWeights};

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
    // NOTE: Contradicts variant added in M04-T26
}

impl EdgeType {
    /// Get all edge type variants
    pub fn all() -> &'static [EdgeType] {
        &[
            EdgeType::Semantic,
            EdgeType::Temporal,
            EdgeType::Causal,
            EdgeType::Hierarchical,
        ]
    }

    /// Check if this edge type represents a hierarchy
    pub fn is_hierarchical(&self) -> bool {
        matches!(self, EdgeType::Hierarchical)
    }

    /// Check if this edge type is directional
    pub fn is_directional(&self) -> bool {
        matches!(self, EdgeType::Causal | EdgeType::Temporal | EdgeType::Hierarchical)
    }
}

impl Default for EdgeType {
    fn default() -> Self {
        EdgeType::Semantic
    }
}

impl std::fmt::Display for EdgeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EdgeType::Semantic => write!(f, "Semantic"),
            EdgeType::Temporal => write!(f, "Temporal"),
            EdgeType::Causal => write!(f, "Causal"),
            EdgeType::Hierarchical => write!(f, "Hierarchical"),
        }
    }
}

/// Edge ID type
pub type EdgeId = i64;

/// Graph edge with Marblestone neuro-modulation support
///
/// # CANONICAL FORMULA for modulated weight
/// ```text
/// w_eff = base_weight * (1.0 + net_activation + domain_bonus) * steering_factor
/// ```
/// Result is clamped to [0.0, 1.0]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    /// Unique edge identifier
    pub id: EdgeId,

    /// Source node ID
    pub source: NodeId,

    /// Target node ID
    pub target: NodeId,

    /// Edge type classification
    pub edge_type: EdgeType,

    /// Base edge weight [0.0, 1.0]
    /// Higher = stronger relationship
    pub weight: f32,

    /// Confidence score [0.0, 1.0]
    /// How certain we are about this edge
    pub confidence: f32,

    /// Cognitive domain of this edge
    pub domain: Domain,

    /// Neurotransmitter weights for modulation
    pub neurotransmitter_weights: NeurotransmitterWeights,

    /// Whether this is an amortized inference shortcut
    /// (learned pattern that bypasses intermediate steps)
    pub is_amortized_shortcut: bool,

    /// Steering reward from traversal history [0.0, 1.0]
    /// Updated via EMA when edge leads to successful retrieval
    pub steering_reward: f32,

    /// Number of times this edge has been traversed
    pub traversal_count: u64,

    /// Unix timestamp when edge was created
    pub created_at: u64,

    /// Unix timestamp of last traversal (0 if never)
    pub last_traversed_at: u64,
}

impl GraphEdge {
    /// Create a new edge with domain-appropriate NT weights
    ///
    /// # Arguments
    /// * `id` - Unique edge identifier
    /// * `source` - Source node ID
    /// * `target` - Target node ID
    /// * `edge_type` - Type of relationship
    /// * `weight` - Base edge weight [0, 1]
    /// * `domain` - Cognitive domain
    pub fn new(
        id: EdgeId,
        source: NodeId,
        target: NodeId,
        edge_type: EdgeType,
        weight: f32,
        domain: Domain,
    ) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            id,
            source,
            target,
            edge_type,
            weight: weight.clamp(0.0, 1.0),
            confidence: 1.0,
            domain,
            neurotransmitter_weights: NeurotransmitterWeights::for_domain(domain),
            is_amortized_shortcut: false,
            steering_reward: 0.5, // Neutral starting value
            traversal_count: 0,
            created_at: now,
            last_traversed_at: 0,
        }
    }

    /// Create a simple semantic edge
    pub fn semantic(id: EdgeId, source: NodeId, target: NodeId, weight: f32) -> Self {
        Self::new(id, source, target, EdgeType::Semantic, weight, Domain::General)
    }

    /// Create a hierarchical edge
    pub fn hierarchical(id: EdgeId, parent: NodeId, child: NodeId, weight: f32) -> Self {
        Self::new(id, parent, child, EdgeType::Hierarchical, weight, Domain::General)
    }

    /// Get modulated weight for a specific query domain
    ///
    /// # CANONICAL FORMULA
    /// ```text
    /// net_activation = excitatory - inhibitory + (modulatory * 0.5)
    /// domain_bonus = 0.1 if edge_domain == query_domain else 0.0
    /// steering_factor = 0.5 + steering_reward  // Range [0.5, 1.5]
    /// w_eff = base_weight * (1.0 + net_activation + domain_bonus) * steering_factor
    /// ```
    /// Result clamped to [0.0, 1.0]
    ///
    /// # Arguments
    /// * `query_domain` - The domain of the current query/traversal
    ///
    /// # Returns
    /// Effective weight after modulation, clamped to [0.0, 1.0]
    pub fn get_modulated_weight(&self, query_domain: Domain) -> f32 {
        let net_activation = self.neurotransmitter_weights.net_activation();
        let domain_bonus = NeurotransmitterWeights::domain_bonus(self.domain, query_domain);

        // steering_factor: 0.5 + steering_reward puts it in [0.5, 1.5] range
        let steering_factor = 0.5 + self.steering_reward;

        // Apply canonical formula
        let w_eff = self.weight * (1.0 + net_activation + domain_bonus) * steering_factor;

        // Clamp to valid range
        w_eff.clamp(0.0, 1.0)
    }

    /// Get unmodulated base weight
    pub fn base_weight(&self) -> f32 {
        self.weight
    }

    /// Record a traversal of this edge
    ///
    /// Updates traversal count, timestamp, and steering reward via EMA.
    ///
    /// # Arguments
    /// * `success` - Whether the traversal led to successful retrieval
    /// * `alpha` - EMA smoothing factor [0, 1], default 0.1
    pub fn record_traversal(&mut self, success: bool, alpha: f32) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        self.traversal_count += 1;
        self.last_traversed_at = now;

        // Update steering reward via EMA
        let reward = if success { 1.0 } else { 0.0 };
        self.steering_reward = (1.0 - alpha) * self.steering_reward + alpha * reward;
        self.steering_reward = self.steering_reward.clamp(0.0, 1.0);
    }

    /// Record traversal with default EMA alpha (0.1)
    pub fn record_traversal_default(&mut self, success: bool) {
        self.record_traversal(success, 0.1);
    }

    /// Mark this edge as an amortized shortcut
    pub fn mark_as_shortcut(&mut self) {
        self.is_amortized_shortcut = true;
    }

    /// Update confidence score
    pub fn update_confidence(&mut self, new_confidence: f32) {
        self.confidence = new_confidence.clamp(0.0, 1.0);
    }

    /// Update neurotransmitter weights
    pub fn update_nt_weights(&mut self, weights: NeurotransmitterWeights) {
        self.neurotransmitter_weights = weights;
    }

    /// Check if edge has been traversed recently
    ///
    /// # Arguments
    /// * `since` - Unix timestamp to check against
    ///
    /// # Returns
    /// true if last_traversed_at > since
    pub fn traversed_since(&self, since: u64) -> bool {
        self.last_traversed_at > since
    }

    /// Get edge "freshness" - how recent the last traversal was
    ///
    /// Returns seconds since last traversal, or u64::MAX if never traversed.
    pub fn freshness(&self) -> u64 {
        if self.last_traversed_at == 0 {
            return u64::MAX;
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        now.saturating_sub(self.last_traversed_at)
    }

    /// Get composite score combining weight, confidence, and steering
    pub fn composite_score(&self, query_domain: Domain) -> f32 {
        self.get_modulated_weight(query_domain) * self.confidence
    }
}

impl PartialEq for GraphEdge {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for GraphEdge {}

impl std::hash::Hash for GraphEdge {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Default for GraphEdge {
    fn default() -> Self {
        Self::new(0, 0, 0, EdgeType::Semantic, 0.5, Domain::General)
    }
}
```

### Constraints
- GraphEdge struct MUST have all 13 fields
- get_modulated_weight() MUST use CANONICAL formula
- steering_factor = 0.5 + steering_reward (range [0.5, 1.5])
- Result of get_modulated_weight() MUST be clamped to [0.0, 1.0]
- record_traversal() uses EMA for steering_reward updates
- EdgeType does NOT include Contradicts (added in M04-T26)

### Acceptance Criteria
- [ ] GraphEdge struct with all 13 fields
- [ ] new() initializes with domain-appropriate NT weights
- [ ] get_modulated_weight() uses CANONICAL formula
- [ ] record_traversal() increments count and updates steering_reward with EMA
- [ ] EdgeType enum: Semantic, Temporal, Causal, Hierarchical
- [ ] Domain enum: Code, Legal, Medical, Creative, Research, General
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm
1. get_modulated_weight(query_domain):
   ```
   net_activation = NT_weights.net_activation()  // canonical formula
   domain_bonus = 0.1 if self.domain == query_domain else 0.0
   steering_factor = 0.5 + self.steering_reward
   w_eff = self.weight * (1.0 + net_activation + domain_bonus) * steering_factor
   return clamp(w_eff, 0.0, 1.0)
   ```

2. record_traversal(success, alpha):
   ```
   traversal_count += 1
   last_traversed_at = now()
   reward = 1.0 if success else 0.0
   steering_reward = (1 - alpha) * steering_reward + alpha * reward
   steering_reward = clamp(steering_reward, 0.0, 1.0)
   ```

### Edge Cases
- Zero base weight: Modulated weight will also be zero
- Negative net_activation: Can reduce effective weight
- Max steering_reward: steering_factor = 1.5
- Min steering_reward: steering_factor = 0.5

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph edge
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] All 13 fields present in struct
- [ ] get_modulated_weight formula matches canonical
- [ ] EMA updates steering_reward correctly
- [ ] Serde serialization roundtrip works

### Test Cases

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_creation() {
        let edge = GraphEdge::new(1, 10, 20, EdgeType::Semantic, 0.8, Domain::Code);

        assert_eq!(edge.id, 1);
        assert_eq!(edge.source, 10);
        assert_eq!(edge.target, 20);
        assert_eq!(edge.edge_type, EdgeType::Semantic);
        assert!((edge.weight - 0.8).abs() < 1e-6);
        assert_eq!(edge.domain, Domain::Code);

        // Should have Code domain NT weights
        let expected_nt = NeurotransmitterWeights::for_domain(Domain::Code);
        assert_eq!(edge.neurotransmitter_weights, expected_nt);
    }

    #[test]
    fn test_modulated_weight_formula() {
        let mut edge = GraphEdge::new(1, 10, 20, EdgeType::Semantic, 0.5, Domain::Code);
        edge.steering_reward = 0.5;  // steering_factor = 1.0

        // Code domain weights: e=0.7, i=0.3, m=0.2
        // net_activation = 0.7 - 0.3 + (0.2 * 0.5) = 0.5

        // Query same domain: domain_bonus = 0.1
        // w_eff = 0.5 * (1.0 + 0.5 + 0.1) * 1.0 = 0.5 * 1.6 = 0.8
        let w = edge.get_modulated_weight(Domain::Code);
        assert!((w - 0.8).abs() < 0.01);

        // Query different domain: domain_bonus = 0.0
        // w_eff = 0.5 * (1.0 + 0.5 + 0.0) * 1.0 = 0.5 * 1.5 = 0.75
        let w = edge.get_modulated_weight(Domain::Legal);
        assert!((w - 0.75).abs() < 0.01);
    }

    #[test]
    fn test_modulated_weight_clamping() {
        let mut edge = GraphEdge::new(1, 10, 20, EdgeType::Semantic, 1.0, Domain::Creative);
        edge.steering_reward = 1.0;  // steering_factor = 1.5

        // Creative: e=0.8, i=0.2, m=0.5 -> net_activation = 0.8 - 0.2 + 0.25 = 0.85
        // w_eff = 1.0 * (1.0 + 0.85 + 0.1) * 1.5 = 1.0 * 1.95 * 1.5 = 2.925
        // But clamped to 1.0
        let w = edge.get_modulated_weight(Domain::Creative);
        assert!((w - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_record_traversal() {
        let mut edge = GraphEdge::new(1, 10, 20, EdgeType::Semantic, 0.5, Domain::General);
        edge.steering_reward = 0.5;

        // Record successful traversal with alpha=0.1
        edge.record_traversal(true, 0.1);
        // steering_reward = 0.9 * 0.5 + 0.1 * 1.0 = 0.45 + 0.1 = 0.55
        assert!((edge.steering_reward - 0.55).abs() < 1e-6);
        assert_eq!(edge.traversal_count, 1);
        assert!(edge.last_traversed_at > 0);

        // Record failed traversal
        edge.record_traversal(false, 0.1);
        // steering_reward = 0.9 * 0.55 + 0.1 * 0.0 = 0.495
        assert!((edge.steering_reward - 0.495).abs() < 1e-6);
        assert_eq!(edge.traversal_count, 2);
    }

    #[test]
    fn test_serde_roundtrip() {
        let edge = GraphEdge::new(42, 100, 200, EdgeType::Causal, 0.75, Domain::Medical);

        let serialized = bincode::serialize(&edge).unwrap();
        let deserialized: GraphEdge = bincode::deserialize(&serialized).unwrap();

        assert_eq!(edge.id, deserialized.id);
        assert_eq!(edge.source, deserialized.source);
        assert_eq!(edge.target, deserialized.target);
        assert_eq!(edge.edge_type, deserialized.edge_type);
        assert!((edge.weight - deserialized.weight).abs() < 1e-6);
        assert_eq!(edge.domain, deserialized.domain);
    }

    #[test]
    fn test_edge_type_variants() {
        assert_eq!(EdgeType::all().len(), 4);
        assert!(EdgeType::Hierarchical.is_hierarchical());
        assert!(EdgeType::Causal.is_directional());
        assert!(!EdgeType::Semantic.is_directional());
    }
}
```

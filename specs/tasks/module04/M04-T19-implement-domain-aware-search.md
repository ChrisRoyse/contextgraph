---
id: "M04-T19"
title: "Implement Domain-Aware Search (Marblestone)"
description: |
  Implement domain_aware_search(query, domain, k) with neurotransmitter modulation.
  Algorithm:
  1. FAISS k-NN search fetching 3x candidates
  2. Apply NeurotransmitterWeights modulation per domain
  3. Re-rank by modulated score
  4. Return top-k results

  CANONICAL FORMULA for modulation:
  modulated_score = base_similarity * (1.0 + net_activation)

  Performance: <10ms for k=10 on 10M vectors.
layer: "surface"
status: "pending"
priority: "critical"
estimated_hours: 3
sequence: 27
depends_on:
  - "M04-T14a"
  - "M04-T15"
  - "M04-T18"
spec_refs:
  - "TECH-GRAPH-004 Section 8"
  - "REQ-KG-065"
files_to_create:
  - path: "crates/context-graph-graph/src/marblestone/domain_search.rs"
    description: "Domain-aware search with NT modulation"
files_to_modify:
  - path: "crates/context-graph-graph/src/marblestone/mod.rs"
    description: "Add domain_search module"
test_file: "crates/context-graph-graph/tests/domain_search_tests.rs"
---

## Context

Domain-aware search implements the Marblestone brain-inspired modulation system for context-sensitive retrieval. Different knowledge domains (Code, Legal, Medical, Creative, Research, General) have distinct neurotransmitter profiles that modulate edge weights and search relevance. This enables the knowledge graph to surface domain-appropriate content by adjusting the base similarity scores according to excitatory, inhibitory, and modulatory signals.

The CANONICAL FORMULA for modulation ensures consistency across all Marblestone operations:
- `net_activation = excitatory - inhibitory + (modulatory * 0.5)`
- `modulated_score = base_similarity * (1.0 + net_activation)`

## Scope

### In Scope
- `domain_aware_search()` function
- DomainSearchResult struct with base and modulated scores
- Over-fetch 3x candidates for re-ranking
- Apply domain-specific NT profiles
- Re-rank by modulated score

### Out of Scope
- NT profile learning/training
- Cross-domain fusion
- Hierarchical domain matching
- CUDA-accelerated NT modulation

## Definition of Done

### Signatures

```rust
// In crates/context-graph-graph/src/marblestone/domain_search.rs

use crate::error::{GraphError, GraphResult};
use crate::index::gpu_index::FaissGpuIndex;
use crate::search::{semantic_search, SemanticSearchResult, SearchFilters};
use crate::storage::rocksdb::{GraphStorage, NodeId};
use crate::storage::edges::{Domain, NeurotransmitterWeights};
use crate::Vector1536;

/// Result from domain-aware search
#[derive(Debug, Clone)]
pub struct DomainSearchResult {
    /// Node identifier
    pub node_id: NodeId,

    /// Base cosine similarity (before modulation)
    pub base_similarity: f32,

    /// Modulated score after NT adjustment
    pub modulated_score: f32,

    /// L2 distance from query
    pub distance: f32,

    /// Rank in result set (0 = best match)
    pub rank: usize,

    /// Domain of the result node
    pub node_domain: Domain,

    /// Query domain used for modulation
    pub query_domain: Domain,

    /// Whether domain matched (bonus applied)
    pub domain_matched: bool,
}

impl DomainSearchResult {
    /// Create from semantic search result with modulation
    pub fn from_semantic(
        semantic: &SemanticSearchResult,
        modulated_score: f32,
        node_domain: Domain,
        query_domain: Domain,
    ) -> Self {
        Self {
            node_id: semantic.node_id,
            base_similarity: semantic.similarity,
            modulated_score,
            distance: semantic.distance,
            rank: 0,  // Will be set after re-ranking
            node_domain,
            query_domain,
            domain_matched: node_domain == query_domain,
        }
    }

    /// Get the boost/penalty applied (modulated - base)
    pub fn modulation_delta(&self) -> f32 {
        self.modulated_score - self.base_similarity
    }

    /// Get boost ratio (modulated / base)
    pub fn boost_ratio(&self) -> f32 {
        if self.base_similarity > 1e-6 {
            self.modulated_score / self.base_similarity
        } else {
            1.0
        }
    }
}

/// Domain bonus for matching domains
const DOMAIN_MATCH_BONUS: f32 = 0.1;

/// Perform domain-aware search with neurotransmitter modulation
///
/// Uses Marblestone-inspired NT modulation to adjust search relevance
/// based on the query domain. Over-fetches 3x candidates, applies
/// NT modulation, then re-ranks by modulated score.
///
/// CANONICAL FORMULA:
/// ```text
/// net_activation = excitatory - inhibitory + (modulatory * 0.5)
/// modulated_score = base_similarity * (1.0 + net_activation + domain_bonus)
/// ```
///
/// # Arguments
/// * `index` - FAISS GPU index
/// * `storage` - Graph storage for node metadata
/// * `query` - Query embedding
/// * `query_domain` - Domain for NT profile selection
/// * `k` - Number of results to return
/// * `filters` - Optional additional filters
///
/// # Returns
/// * Top-k results ranked by modulated score
///
/// # Performance
/// Target: <10ms for k=10 on 10M vectors
///
/// # Example
/// ```rust
/// let results = domain_aware_search(
///     &index, &storage,
///     &query_embedding,
///     Domain::Code,
///     10,
///     None,
/// )?;
///
/// for result in results {
///     println!("Node {} base: {:.3} modulated: {:.3}",
///         result.node_id,
///         result.base_similarity,
///         result.modulated_score
///     );
/// }
/// ```
pub fn domain_aware_search(
    index: &FaissGpuIndex,
    storage: &GraphStorage,
    query: &Vector1536,
    query_domain: Domain,
    k: usize,
    filters: Option<SearchFilters>,
) -> GraphResult<Vec<DomainSearchResult>> {
    // Over-fetch 3x candidates for re-ranking
    let fetch_k = k * 3;

    // Get base semantic results
    let semantic_results = semantic_search(index, storage, query, fetch_k, filters)?;

    if semantic_results.is_empty() {
        return Ok(Vec::new());
    }

    // Get domain-specific NT profile
    let domain_nt = NeurotransmitterWeights::for_domain(query_domain);
    let base_net_activation = domain_nt.net_activation();

    // Apply modulation to each result
    let mut modulated_results: Vec<DomainSearchResult> = Vec::with_capacity(semantic_results.len());

    for semantic in &semantic_results {
        // Get node's domain
        let node_domain = match storage.get_node_domain(semantic.node_id)? {
            Some(d) => d,
            None => Domain::General,  // Default if unknown
        };

        // Calculate domain bonus
        let domain_bonus = if node_domain == query_domain {
            DOMAIN_MATCH_BONUS
        } else {
            0.0
        };

        // CANONICAL FORMULA: modulated_score = base * (1.0 + net_activation + domain_bonus)
        let modulated_score = semantic.similarity * (1.0 + base_net_activation + domain_bonus);
        let modulated_score = modulated_score.max(0.0).min(1.0);  // Clamp to [0, 1]

        modulated_results.push(DomainSearchResult::from_semantic(
            semantic,
            modulated_score,
            node_domain,
            query_domain,
        ));
    }

    // Re-rank by modulated score (descending)
    modulated_results.sort_by(|a, b| {
        b.modulated_score.partial_cmp(&a.modulated_score).unwrap()
    });

    // Update ranks and truncate to k
    for (i, result) in modulated_results.iter_mut().enumerate() {
        result.rank = i;
    }
    modulated_results.truncate(k);

    Ok(modulated_results)
}

/// Get average modulation boost for a domain
///
/// Returns the expected boost ratio for nodes matching the query domain.
pub fn expected_domain_boost(domain: Domain) -> f32 {
    let nt = NeurotransmitterWeights::for_domain(domain);
    let net_activation = nt.net_activation();
    1.0 + net_activation + DOMAIN_MATCH_BONUS
}

/// Get NT profile summary for a domain
pub fn domain_nt_summary(domain: Domain) -> String {
    let nt = NeurotransmitterWeights::for_domain(domain);
    format!(
        "{}: exc={:.2} inh={:.2} mod={:.2} net={:+.3}",
        domain,
        nt.excitatory,
        nt.inhibitory,
        nt.modulatory,
        nt.net_activation()
    )
}

/// Batch domain-aware search
pub fn domain_aware_search_batch(
    index: &FaissGpuIndex,
    storage: &GraphStorage,
    queries: &[Vector1536],
    query_domain: Domain,
    k: usize,
) -> GraphResult<Vec<Vec<DomainSearchResult>>> {
    // For efficiency, we could batch the FAISS search
    // For now, iterate (optimization in future task)
    queries.iter()
        .map(|q| domain_aware_search(index, storage, q, query_domain, k, None))
        .collect()
}
```

### Constraints
- MUST use CANONICAL formula for net_activation
- MUST over-fetch 3x candidates before modulation
- MUST re-rank by modulated_score, not base_similarity
- Domain match bonus = 0.1
- Modulated score clamped to [0.0, 1.0]
- Performance: <10ms for k=10 on 10M vectors

### Acceptance Criteria
- [ ] domain_aware_search() over-fetches candidates (3x)
- [ ] Applies NT modulation using CANONICAL formula
- [ ] Re-ranks results by modulated score
- [ ] Truncates to requested k
- [ ] DomainSearchResult includes base_distance and modulated_score
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm
```
domain_aware_search(index, storage, query, domain, k, filters):
    # Over-fetch for re-ranking
    fetch_k = k * 3

    # Base semantic search
    base_results = semantic_search(index, storage, query, fetch_k, filters)

    if base_results.empty:
        return []

    # Get domain NT profile
    domain_nt = NeurotransmitterWeights::for_domain(domain)
    net_activation = domain_nt.net_activation()
        # = excitatory - inhibitory + (modulatory * 0.5)

    # Modulate each result
    modulated = []
    for result in base_results:
        node_domain = storage.get_node_domain(result.node_id)

        # Domain bonus for matching
        domain_bonus = 0.1 if node_domain == domain else 0.0

        # CANONICAL FORMULA
        modulated_score = result.similarity * (1.0 + net_activation + domain_bonus)
        modulated_score = clamp(modulated_score, 0.0, 1.0)

        modulated.append(DomainSearchResult{
            node_id: result.node_id,
            base_similarity: result.similarity,
            modulated_score,
            node_domain,
            query_domain: domain,
        })

    # Re-rank by modulated score
    modulated.sort_by(modulated_score, descending)

    # Assign ranks and truncate
    for i, r in modulated:
        r.rank = i

    return modulated[:k]
```

### Domain NT Profiles (from M04-T14)
| Domain | Excitatory | Inhibitory | Modulatory | Net Activation |
|--------|------------|------------|------------|----------------|
| Code | 0.7 | 0.3 | 0.2 | +0.50 |
| Creative | 0.8 | 0.2 | 0.5 | +0.85 |
| Legal | 0.5 | 0.4 | 0.3 | +0.25 |
| Medical | 0.6 | 0.3 | 0.2 | +0.40 |
| Research | 0.6 | 0.2 | 0.4 | +0.60 |
| General | 0.5 | 0.5 | 0.0 | 0.00 |

### Edge Cases
- No base results: Return empty vector
- Unknown node domain: Default to General
- Negative modulated score: Clamp to 0.0
- Score > 1.0: Clamp to 1.0
- All nodes filtered: Return empty vector

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph domain_search
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] Code domain boosts code-domain nodes
- [ ] Creative domain provides highest boost
- [ ] Domain match adds 0.1 bonus
- [ ] Re-ranking changes order from base

### Test Cases

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_formula_net_activation() {
        // Code domain: 0.7 - 0.3 + (0.2 * 0.5) = 0.5
        let code_nt = NeurotransmitterWeights::for_domain(Domain::Code);
        let net = code_nt.net_activation();
        assert!((net - 0.5).abs() < 1e-6);

        // Creative domain: 0.8 - 0.2 + (0.5 * 0.5) = 0.85
        let creative_nt = NeurotransmitterWeights::for_domain(Domain::Creative);
        let net = creative_nt.net_activation();
        assert!((net - 0.85).abs() < 1e-6);
    }

    #[test]
    fn test_modulation_formula() {
        // base_similarity = 0.8
        // net_activation = 0.5 (Code domain)
        // domain_bonus = 0.1 (matching)
        // modulated = 0.8 * (1.0 + 0.5 + 0.1) = 0.8 * 1.6 = 1.28 -> clamped to 1.0

        let base = 0.8f32;
        let net_activation = 0.5f32;
        let domain_bonus = 0.1f32;

        let modulated = base * (1.0 + net_activation + domain_bonus);
        let modulated = modulated.max(0.0).min(1.0);

        assert!((modulated - 1.0).abs() < 1e-6);  // Clamped
    }

    #[test]
    fn test_domain_search_result_from_semantic() {
        let semantic = SemanticSearchResult {
            node_id: 42,
            similarity: 0.7,
            distance: 0.4,
            rank: 0,
        };

        let domain_result = DomainSearchResult::from_semantic(
            &semantic,
            0.9,  // Modulated up
            Domain::Code,
            Domain::Code,
        );

        assert_eq!(domain_result.base_similarity, 0.7);
        assert_eq!(domain_result.modulated_score, 0.9);
        assert!(domain_result.domain_matched);
        assert!((domain_result.modulation_delta() - 0.2).abs() < 1e-6);
    }

    #[test]
    fn test_expected_domain_boost() {
        // Code: 1.0 + 0.5 + 0.1 = 1.6
        let code_boost = expected_domain_boost(Domain::Code);
        assert!((code_boost - 1.6).abs() < 1e-6);

        // General: 1.0 + 0.0 + 0.1 = 1.1
        let general_boost = expected_domain_boost(Domain::General);
        assert!((general_boost - 1.1).abs() < 1e-6);
    }

    #[test]
    #[requires_gpu]
    fn test_domain_aware_search_reranks() {
        // Setup: Create index with mixed domain nodes
        // Verify that domain-matching nodes get boosted above non-matching
        // even if their base similarity is slightly lower

        // This requires full integration test with GPU
        // Placeholder for actual test implementation
    }

    #[test]
    fn test_domain_nt_summary() {
        let summary = domain_nt_summary(Domain::Code);
        assert!(summary.contains("Code"));
        assert!(summary.contains("0.70"));  // excitatory
        assert!(summary.contains("+0.50"));  // net activation
    }
}
```

# TASK-L002: Purpose Vector Computation

```yaml
metadata:
  id: "TASK-L002"
  title: "Purpose Vector Computation"
  layer: "logic"
  priority: "P0"
  estimated_hours: 6
  created: "2026-01-04"
  status: "pending"
  dependencies:
    - "TASK-F001"  # SemanticFingerprint struct
    - "TASK-F002"  # TeleologicalFingerprint struct
  spec_refs:
    - "projectionplan1.md:purpose-vector"
    - "projectionplan2.md:alignment-signature"
```

## Problem Statement

Implement computation of 13-dimensional Purpose Vectors that capture how each embedding space aligns with North Star goals, enabling purpose-aware retrieval and memory organization.

## Context

The Purpose Vector is a 13D signature where each dimension represents the alignment strength of that embedding space to the user's defined goals, including E13 SPLADE for sparse lexical alignment. This enables:
- Purpose-weighted retrieval (prioritize aligned spaces)
- Goal drift detection (track purpose evolution over time)
- Misalignment flagging (identify off-purpose memories)
- Strategic memory organization
- **E13 SPLADE alignment** for keyword/term-based goal matching

## Technical Specification

### Data Structures

```rust
/// A 13-dimensional purpose alignment signature
#[derive(Clone, Debug, PartialEq)]
pub struct PurposeVector {
    /// Alignment score per embedding space [0.0, 1.0]
    /// E1-E12: Dense semantic spaces
    /// E13: SPLADE sparse lexical alignment
    pub alignment: [f32; 13],

    /// North Star goal this vector aligns to
    pub north_star_id: GoalId,

    /// Confidence in the alignment computation
    pub confidence: f32,

    /// When this purpose was computed
    pub computed_at: Timestamp,

    /// Version for cache invalidation
    pub version: u32,

    /// E13 SPLADE keyword alignment details (sparse)
    pub splade_alignment: Option<SpladeAlignment>,
}

/// SPLADE-specific alignment information for E13
#[derive(Clone, Debug, PartialEq)]
pub struct SpladeAlignment {
    /// Top aligned terms with goal vocabulary
    pub aligned_terms: Vec<(String, f32)>,

    /// Lexical coverage of goal keywords
    pub keyword_coverage: f32,

    /// Term overlap with goal description
    pub term_overlap_score: f32,
}

/// Goal identifier in the hierarchy
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GoalId(pub String);

/// Goal hierarchy node
pub struct GoalNode {
    pub id: GoalId,
    pub description: String,
    pub level: GoalLevel,
    pub parent: Option<GoalId>,
    pub embedding: Vec<f32>,  // Goal's semantic embedding
    pub weight: f32,          // Importance weight
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoalLevel {
    NorthStar,  // Top-level aspirational goal
    Strategic,  // Mid-term objectives
    Tactical,   // Short-term tasks
    Immediate,  // Current context
}

/// Configuration for purpose computation
pub struct PurposeComputeConfig {
    /// Goal hierarchy to align against
    pub goal_hierarchy: Vec<GoalNode>,

    /// Minimum alignment threshold for relevance
    pub alignment_threshold: f32,

    /// Whether to propagate alignment up the hierarchy
    pub hierarchical_propagation: bool,

    /// Decay factor for temporal alignment
    pub temporal_decay: f32,
}
```

### Core Trait

```rust
/// Computes purpose vectors for memories
#[async_trait]
pub trait PurposeVectorComputer: Send + Sync {
    /// Compute purpose vector for a semantic fingerprint
    async fn compute_purpose(
        &self,
        fingerprint: &SemanticFingerprint,
        config: &PurposeComputeConfig,
    ) -> Result<PurposeVector, PurposeError>;

    /// Batch compute purpose vectors
    async fn compute_purpose_batch(
        &self,
        fingerprints: &[SemanticFingerprint],
        config: &PurposeComputeConfig,
    ) -> Result<Vec<PurposeVector>, PurposeError>;

    /// Update purpose vector when goals change
    async fn recompute_for_goal_change(
        &self,
        memory_id: MemoryId,
        old_goals: &[GoalNode],
        new_goals: &[GoalNode],
    ) -> Result<PurposeVector, PurposeError>;

    /// Get current goal hierarchy
    fn goal_hierarchy(&self) -> &[GoalNode];

    /// Update goal hierarchy
    async fn update_goals(&mut self, goals: Vec<GoalNode>) -> Result<(), PurposeError>;
}
```

### Computation Algorithm

```rust
impl PurposeVectorComputer for DefaultPurposeComputer {
    async fn compute_purpose(
        &self,
        fingerprint: &SemanticFingerprint,
        config: &PurposeComputeConfig,
    ) -> Result<PurposeVector, PurposeError> {
        let mut alignment = [0.0f32; 13];

        // Get North Star goal
        let north_star = config.goal_hierarchy.iter()
            .find(|g| g.level == GoalLevel::NorthStar)
            .ok_or(PurposeError::NoNorthStar)?;

        // Compute alignment for each dense embedding space (E1-E12)
        for space_idx in 0..12 {
            if let Some(emb) = fingerprint.embeddings.get(space_idx).and_then(|e| e.as_ref()) {
                // Cosine similarity between embedding and goal embedding
                let goal_emb = self.project_goal_to_space(north_star, space_idx)?;
                alignment[space_idx] = cosine_similarity(emb, &goal_emb);

                // Apply hierarchical propagation if enabled
                if config.hierarchical_propagation {
                    alignment[space_idx] = self.propagate_hierarchy(
                        alignment[space_idx],
                        space_idx,
                        config,
                    );
                }
            }
        }

        // Compute E13 SPLADE alignment (sparse lexical)
        let splade_alignment = if let Some(splade_emb) = fingerprint.embeddings.get(12).and_then(|e| e.as_ref()) {
            let splade_result = self.compute_splade_alignment(splade_emb, north_star)?;
            alignment[12] = splade_result.term_overlap_score;
            Some(splade_result)
        } else {
            None
        };

        // Normalize to [0, 1] range
        let max_align = alignment.iter().cloned().fold(0.0f32, f32::max);
        if max_align > 0.0 {
            for a in alignment.iter_mut() {
                *a /= max_align;
            }
        }

        Ok(PurposeVector {
            alignment,
            north_star_id: north_star.id.clone(),
            confidence: self.compute_confidence(&alignment),
            computed_at: Timestamp::now(),
            version: 1,
            splade_alignment,
        })
    }

    /// Compute SPLADE alignment for E13
    fn compute_splade_alignment(
        &self,
        splade_embedding: &[f32],
        goal: &GoalNode,
    ) -> Result<SpladeAlignment, PurposeError> {
        // Extract non-zero terms from SPLADE sparse vector
        let goal_keywords = self.extract_goal_keywords(goal);
        let memory_terms = self.decode_splade_terms(splade_embedding);

        // Compute term overlap
        let mut aligned_terms = Vec::new();
        let mut overlap_score = 0.0;

        for (term, weight) in &memory_terms {
            if goal_keywords.contains(term) {
                aligned_terms.push((term.clone(), *weight));
                overlap_score += weight;
            }
        }

        let keyword_coverage = if !goal_keywords.is_empty() {
            aligned_terms.len() as f32 / goal_keywords.len() as f32
        } else {
            0.0
        };

        Ok(SpladeAlignment {
            aligned_terms,
            keyword_coverage,
            term_overlap_score: overlap_score.min(1.0),
        })
    }
}
```

## Implementation Requirements

### Prerequisites

- [ ] TASK-F001 complete (SemanticFingerprint available)
- [ ] TASK-F002 complete (TeleologicalFingerprint with PurposeVector field)

### Scope

#### In Scope

- PurposeVector struct and computation
- GoalNode hierarchy management
- Per-space alignment calculation
- Hierarchical propagation
- Confidence scoring
- Batch computation

#### Out of Scope

- Goal hierarchy storage (TASK-F004)
- Purpose pattern indexing (TASK-L006)
- Goal alignment UI/API

### Constraints

- Computation < 10ms for single fingerprint
- Support goal hierarchies up to 100 nodes
- Thread-safe goal updates
- Deterministic computation for same inputs

## Pseudo Code

```
FUNCTION compute_purpose_vector(fingerprint, goals):
    north_star = find_north_star(goals)
    alignment = [0.0; 13]

    // Compute dense space alignments (E1-E12)
    FOR space_idx IN 0..12:
        IF fingerprint.embeddings[space_idx] IS NOT NULL:
            // Project goal to this embedding space
            goal_embedding = project_goal(north_star, space_idx)

            // Compute base alignment
            base_align = cosine_similarity(
                fingerprint.embeddings[space_idx],
                goal_embedding
            )

            // Hierarchical propagation: boost if aligned with sub-goals too
            IF hierarchical_propagation:
                sub_goal_boost = 0.0
                FOR sub_goal IN get_children(north_star, goals):
                    sub_emb = project_goal(sub_goal, space_idx)
                    sub_align = cosine_similarity(
                        fingerprint.embeddings[space_idx],
                        sub_emb
                    )
                    sub_goal_boost += sub_align * sub_goal.weight

                alignment[space_idx] = base_align * 0.7 + sub_goal_boost * 0.3
            ELSE:
                alignment[space_idx] = base_align

    // Compute E13 SPLADE alignment (sparse lexical)
    IF fingerprint.embeddings[12] IS NOT NULL:
        splade_emb = fingerprint.embeddings[12]
        goal_keywords = extract_goal_keywords(north_star)
        memory_terms = decode_splade_terms(splade_emb)

        // Compute term overlap with goal vocabulary
        overlap_score = 0.0
        aligned_terms = []
        FOR (term, weight) IN memory_terms:
            IF term IN goal_keywords:
                aligned_terms.push((term, weight))
                overlap_score += weight

        alignment[12] = min(overlap_score, 1.0)
        splade_alignment = SpladeAlignment {
            aligned_terms,
            keyword_coverage: aligned_terms.len() / goal_keywords.len(),
            term_overlap_score: alignment[12]
        }

    // Normalize
    max_val = max(alignment)
    IF max_val > 0:
        alignment = alignment / max_val

    // Compute confidence (higher if alignment is consistent across spaces)
    variance = compute_variance(alignment)
    confidence = 1.0 - min(variance, 1.0)

    RETURN PurposeVector {
        alignment,
        north_star_id: north_star.id,
        confidence,
        computed_at: now(),
        version: 1,
        splade_alignment: splade_alignment  // E13 details
    }
```

## Definition of Done

### Implementation Checklist

- [ ] `PurposeVector` struct with **13D alignment** (E1-E12 dense + E13 SPLADE)
- [ ] `SpladeAlignment` struct for E13 keyword alignment details
- [ ] `GoalNode` hierarchy structure
- [ ] `PurposeVectorComputer` trait
- [ ] Default implementation with cosine similarity (E1-E12)
- [ ] **E13 SPLADE alignment computation** (term overlap with goal keywords)
- [ ] Hierarchical propagation algorithm
- [ ] Confidence scoring (13D)
- [ ] Batch computation support
- [ ] Goal hierarchy update handling

### Testing Requirements

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_purpose_vector_creation() {
        let pv = PurposeVector {
            alignment: [0.5; 13],
            north_star_id: GoalId("test".into()),
            confidence: 0.8,
            computed_at: Timestamp::now(),
            version: 1,
            splade_alignment: None,
        };
        assert_eq!(pv.alignment.len(), 13);
    }

    #[test]
    fn test_splade_alignment() {
        let splade_align = SpladeAlignment {
            aligned_terms: vec![
                ("machine".into(), 0.8),
                ("learning".into(), 0.7),
            ],
            keyword_coverage: 0.5,
            term_overlap_score: 0.75,
        };
        let pv = PurposeVector {
            alignment: {
                let mut a = [0.5; 13];
                a[12] = splade_align.term_overlap_score;
                a
            },
            north_star_id: GoalId("ml_goal".into()),
            confidence: 0.8,
            computed_at: Timestamp::now(),
            version: 1,
            splade_alignment: Some(splade_align),
        };
        assert_eq!(pv.alignment[12], 0.75);
    }

    #[tokio::test]
    async fn test_purpose_computation() {
        let computer = DefaultPurposeComputer::new();
        let fingerprint = create_test_fingerprint();
        let config = create_test_config();

        let purpose = computer.compute_purpose(&fingerprint, &config).await.unwrap();

        // All alignments should be in [0, 1]
        for a in purpose.alignment {
            assert!(a >= 0.0 && a <= 1.0);
        }
        assert!(purpose.confidence >= 0.0 && purpose.confidence <= 1.0);
    }

    #[tokio::test]
    async fn test_hierarchical_propagation() {
        let computer = DefaultPurposeComputer::new();
        let fingerprint = create_test_fingerprint();

        let config_no_prop = PurposeComputeConfig {
            hierarchical_propagation: false,
            ..create_test_config()
        };
        let config_with_prop = PurposeComputeConfig {
            hierarchical_propagation: true,
            ..create_test_config()
        };

        let pv1 = computer.compute_purpose(&fingerprint, &config_no_prop).await.unwrap();
        let pv2 = computer.compute_purpose(&fingerprint, &config_with_prop).await.unwrap();

        // Propagation should affect alignment values
        assert_ne!(pv1.alignment, pv2.alignment);
    }

    #[tokio::test]
    async fn test_batch_computation() {
        let computer = DefaultPurposeComputer::new();
        let fingerprints = vec![create_test_fingerprint(); 10];
        let config = create_test_config();

        let purposes = computer.compute_purpose_batch(&fingerprints, &config).await.unwrap();
        assert_eq!(purposes.len(), 10);
    }
}
```

### Verification Commands

```bash
# Run unit tests
cargo test -p context-graph-core purpose_vector

# Run with coverage
cargo llvm-cov -p context-graph-core --test purpose_vector

# Benchmark computation
cargo bench -p context-graph-core -- purpose_compute
```

## Files to Create

| File | Description |
|------|-------------|
| `crates/context-graph-core/src/purpose/mod.rs` | Purpose module |
| `crates/context-graph-core/src/purpose/vector.rs` | PurposeVector struct |
| `crates/context-graph-core/src/purpose/computer.rs` | PurposeVectorComputer trait and impl |
| `crates/context-graph-core/src/purpose/goals.rs` | GoalNode hierarchy |

## Files to Modify

| File | Change |
|------|--------|
| `crates/context-graph-core/src/lib.rs` | Add `pub mod purpose` |
| `crates/context-graph-core/src/types/teleological_fingerprint.rs` | Use PurposeVector |

## Traceability

| Requirement | Source | Coverage |
|-------------|--------|----------|
| **13D purpose vector** | projectionplan1.md:purpose-vector | Complete |
| Goal alignment | projectionplan2.md:alignment | Complete |
| Hierarchical goals | projectionplan1.md:goal-hierarchy | Complete |
| Purpose evolution | projectionplan2.md:temporal | Partial (L007) |
| **E13 SPLADE alignment** | projectionplan2.md:splade | Complete |

---

*Task created: 2026-01-04*
*Layer: Logic*
*Priority: P0 - Core purpose alignment*

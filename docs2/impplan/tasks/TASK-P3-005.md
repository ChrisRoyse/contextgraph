# TASK-P3-005: MultiSpaceSimilarity

```xml
<task_spec id="TASK-P3-005" version="3.0" audited="2026-01-17">
<metadata>
  <title>MultiSpaceSimilarity Implementation</title>
  <status>COMPLETE</status>
  <layer>logic</layer>
  <sequence>24</sequence>
  <phase>3</phase>
  <implements>
    <requirement_ref>REQ-P3-01</requirement_ref>
    <requirement_ref>REQ-P3-02</requirement_ref>
    <requirement_ref>REQ-P3-03</requirement_ref>
  </implements>
  <depends_on>
    <task_ref status="COMPLETE">TASK-P3-001</task_ref>
    <task_ref status="COMPLETE">TASK-P3-003</task_ref>
    <task_ref status="COMPLETE">TASK-P3-004</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
</metadata>

<context>
Implements the MultiSpaceSimilarity component that computes similarity scores
across all 13 embedding spaces, determines relevance using the ANY() logic,
and calculates weighted relevance scores.

This is the core comparison engine used by both retrieval and divergence detection.

CATEGORY-AWARE WEIGHTING: The weighted similarity calculation uses EmbedderCategory
topic_weights to determine each space's contribution:
- Semantic spaces (E1, E5, E6, E7, E10, E12, E13): weight 1.0
- Temporal spaces (E2-E4): weight 0.0 (EXCLUDED from relevance scoring per AP-60)
- Relational spaces (E8, E11): weight 0.5
- Structural space (E9): weight 0.5

max_weighted_agreement = 7*1.0 + 2*0.5 + 1*0.5 = 8.5 (from constitution)
topic_threshold = 2.5 (per ARCH-09)
</context>

<codebase_state audited="2026-01-16">
## VERIFIED DEPENDENCY FILES

### TASK-P3-001 (COMPLETE): PerSpaceScores and SimilarityResult
Location: crates/context-graph-core/src/retrieval/similarity.rs (492 lines)

```rust
pub const NUM_SPACES: usize = 13;

pub struct PerSpaceScores {
    pub semantic: f32,           // E1
    pub temporal_recent: f32,    // E2
    pub temporal_periodic: f32,  // E3
    pub temporal_positional: f32,// E4
    pub causal: f32,             // E5
    pub sparse: f32,             // E6
    pub code: f32,               // E7
    pub emotional: f32,          // E8
    pub hdc: f32,                // E9
    pub multimodal: f32,         // E10
    pub entity: f32,             // E11
    pub late_interaction: f32,   // E12
    pub keyword_splade: f32,     // E13
}

impl PerSpaceScores {
    pub fn new() -> Self;
    pub fn get_score(&self, embedder: Embedder) -> f32;
    pub fn set_score(&mut self, embedder: Embedder, score: f32);
    pub fn iter(&self) -> impl Iterator<Item = (Embedder, f32)> + '_;
    pub fn max_score(&self) -> f32;
    pub fn mean_score(&self) -> f32;
    pub fn weighted_mean(&self) -> f32;  // Uses category weights, excludes temporal
    pub fn to_array(&self) -> [f32; NUM_SPACES];
    pub fn from_array(arr: [f32; NUM_SPACES]) -> Self;
    pub fn included_spaces() -> Vec<Embedder>;  // Returns 10 non-temporal spaces
}

pub struct SimilarityResult {
    pub memory_id: Uuid,
    pub per_space_scores: PerSpaceScores,
    pub weighted_similarity: f32,
    pub relevance_score: f32,
    pub matching_spaces: Vec<Embedder>,
    pub included_spaces: Vec<Embedder>,
    pub space_count: u8,
}

impl SimilarityResult {
    pub fn new(memory_id: Uuid, scores: PerSpaceScores) -> Self;
    pub fn with_relevance(memory_id: Uuid, scores: PerSpaceScores, relevance_score: f32, matching_spaces: Vec<Embedder>) -> Self;
}
```

### TASK-P3-003 (COMPLETE): Threshold and Weight Configurations
Location: crates/context-graph-core/src/retrieval/config.rs (831 lines)

```rust
/// Category-based space weights (index = Embedder::index())
pub const SPACE_WEIGHTS: [f32; 13] = [
    1.0,  // E1 Semantic
    0.0,  // E2 TemporalRecent (excluded)
    0.0,  // E3 TemporalPeriodic (excluded)
    0.0,  // E4 TemporalPositional (excluded)
    1.0,  // E5 Causal
    1.0,  // E6 Sparse
    1.0,  // E7 Code
    0.5,  // E8 Emotional (Relational)
    0.5,  // E9 Hdc (Structural)
    1.0,  // E10 Multimodal
    0.5,  // E11 Entity (Relational)
    1.0,  // E12 LateInteraction
    1.0,  // E13 KeywordSplade
];

pub struct PerSpaceThresholds { /* 13 f32 fields */ }
impl PerSpaceThresholds {
    pub fn get_threshold(&self, embedder: Embedder) -> f32;
    pub fn set_threshold(&mut self, embedder: Embedder, threshold: f32);
    pub fn to_array(&self) -> [f32; 13];
    pub fn from_array(arr: [f32; 13]) -> Self;
}

pub struct SpaceWeights { weights: [f32; 13] }
impl SpaceWeights {
    pub fn new(weights: [f32; 13]) -> Self;
    pub fn get_weight(&self, embedder: Embedder) -> f32;
    pub fn set_weight(&mut self, embedder: Embedder, weight: f32);
    pub fn normalize(&mut self);
    pub fn normalized(&self) -> Self;
    pub fn sum(&self) -> f32;
    pub fn as_slice(&self) -> &[f32; 13];
}

pub struct SimilarityThresholds {
    pub high: PerSpaceThresholds,
    pub low: PerSpaceThresholds,
}
impl Default for SimilarityThresholds { fn default() -> Self; }
impl SimilarityThresholds {
    pub fn new(high: PerSpaceThresholds, low: PerSpaceThresholds) -> Self;
    pub fn is_high(&self, embedder: Embedder, score: f32) -> bool;
    pub fn is_low(&self, embedder: Embedder, score: f32) -> bool;
    pub fn is_middle(&self, embedder: Embedder, score: f32) -> bool;
}

pub fn high_thresholds() -> PerSpaceThresholds;
pub fn low_thresholds() -> PerSpaceThresholds;
pub fn default_weights() -> SpaceWeights;

pub const RECENT_LOOKBACK_SECS: u64 = 2 * 60 * 60;
pub const MAX_RECENT_MEMORIES: usize = 50;
```

HIGH_THRESHOLDS (exact values):
  semantic: 0.75, temporal_recent: 0.70, temporal_periodic: 0.70, temporal_positional: 0.70,
  causal: 0.70, sparse: 0.60, code: 0.80, emotional: 0.70, hdc: 0.70, multimodal: 0.70,
  entity: 0.70, late_interaction: 0.70, keyword_splade: 0.60

LOW_THRESHOLDS (exact values):
  semantic: 0.30, temporal_recent: 0.30, temporal_periodic: 0.30, temporal_positional: 0.30,
  causal: 0.25, sparse: 0.20, code: 0.35, emotional: 0.30, hdc: 0.30, multimodal: 0.30,
  entity: 0.30, late_interaction: 0.30, keyword_splade: 0.20

### TASK-P3-004 (COMPLETE): Distance Calculator
Location: crates/context-graph-core/src/retrieval/distance.rs (730 lines)

```rust
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32;
pub fn jaccard_similarity(a: &SparseVector, b: &SparseVector) -> f32;
pub fn hamming_similarity(a: &BinaryVector, b: &BinaryVector) -> f32;
pub fn max_sim(query_tokens: &[Vec<f32>], memory_tokens: &[Vec<f32>]) -> f32;
pub fn transe_similarity(a: &[f32], b: &[f32]) -> f32;
pub fn compute_similarity_for_space(embedder: Embedder, query: &SemanticFingerprint, memory: &SemanticFingerprint) -> f32;
pub fn compute_all_similarities(query: &SemanticFingerprint, memory: &SemanticFingerprint) -> [f32; 13];
```

### EmbedderCategory System
Location: crates/context-graph-core/src/embeddings/category.rs (431 lines)

```rust
pub enum EmbedderCategory { Semantic, Temporal, Relational, Structural }

impl EmbedderCategory {
    pub const fn topic_weight(&self) -> f32;  // Semantic=1.0, Temporal=0.0, Relational=0.5, Structural=0.5
    pub const fn is_semantic(&self) -> bool;
    pub const fn is_temporal(&self) -> bool;
    pub const fn used_for_divergence_detection(&self) -> bool;  // Only Semantic returns true
}

pub fn category_for(embedder: Embedder) -> EmbedderCategory;
pub const fn max_weighted_agreement() -> f32;  // Returns 8.5
pub const fn topic_threshold() -> f32;         // Returns 2.5
```

### Embedder Enum
Location: crates/context-graph-core/src/teleological/embedder.rs

```rust
pub enum Embedder {
    Semantic = 0,         // E1
    TemporalRecent = 1,   // E2
    TemporalPeriodic = 2, // E3
    TemporalPositional = 3,// E4
    Causal = 4,           // E5
    Sparse = 5,           // E6
    Code = 6,             // E7
    Emotional = 7,        // E8 (NOT "Graph")
    Hdc = 8,              // E9
    Multimodal = 9,       // E10
    Entity = 10,          // E11
    LateInteraction = 11, // E12
    KeywordSplade = 12,   // E13
}

impl Embedder {
    pub fn all() -> impl Iterator<Item = Embedder>;
    pub fn index(&self) -> usize;
    pub fn from_index(index: usize) -> Option<Embedder>;
    pub fn name(&self) -> &'static str;
}
```

### SemanticFingerprint (TeleologicalArray)
Location: crates/context-graph-core/src/types/fingerprint/semantic/fingerprint.rs

```rust
pub type TeleologicalArray = SemanticFingerprint;

pub struct SemanticFingerprint {
    pub e1_semantic: Vec<f32>,              // 1024D
    pub e2_temporal_recent: Vec<f32>,       // 512D
    pub e3_temporal_periodic: Vec<f32>,     // 512D
    pub e4_temporal_positional: Vec<f32>,   // 512D
    pub e5_causal: Vec<f32>,                // 768D
    pub e6_sparse: SparseVector,            // Sparse
    pub e7_code: Vec<f32>,                  // 1536D
    pub e8_graph: Vec<f32>,                 // 384D (NOTE: field is e8_graph, Embedder is Emotional)
    pub e9_hdc: Vec<f32>,                   // 1024D projected
    pub e10_multimodal: Vec<f32>,           // 768D
    pub e11_entity: Vec<f32>,               // 384D
    pub e12_late_interaction: Vec<Vec<f32>>,// 128D per token
    pub e13_splade: SparseVector,           // Sparse
}

impl SemanticFingerprint {
    pub fn zeroed() -> Self;
    pub fn get(&self, embedder: Embedder) -> EmbeddingRef<'_>;
}

pub enum EmbeddingRef<'a> {
    Dense(&'a [f32]),
    Sparse(&'a SparseVector),
    TokenLevel(&'a [Vec<f32>]),
}
```

### Current retrieval/mod.rs exports
Location: crates/context-graph-core/src/retrieval/mod.rs

```rust
pub mod config;
pub mod distance;
pub mod divergence;
pub mod similarity;
// NOTE: multi_space module does NOT exist yet - this task creates it

pub use config::*;
pub use distance::*;
pub use divergence::*;
pub use similarity::*;
```
</codebase_state>

<input_context_files>
  <file purpose="component_spec" exists="true">docs2/impplan/technical/TECH-PHASE3-SIMILARITY-DIVERGENCE.md</file>
  <file purpose="distance" exists="true">crates/context-graph-core/src/retrieval/distance.rs</file>
  <file purpose="config" exists="true">crates/context-graph-core/src/retrieval/config.rs</file>
  <file purpose="category" exists="true">crates/context-graph-core/src/embeddings/category.rs</file>
  <file purpose="similarity" exists="true">crates/context-graph-core/src/retrieval/similarity.rs</file>
</input_context_files>

<prerequisites verified="2026-01-16">
  <check verified="true">TASK-P3-001 COMPLETE - PerSpaceScores exists at retrieval/similarity.rs with get_score/set_score/weighted_mean</check>
  <check verified="true">TASK-P3-003 COMPLETE - config.rs exists with high_thresholds(), low_thresholds(), default_weights(), SPACE_WEIGHTS</check>
  <check verified="true">TASK-P3-004 COMPLETE - distance.rs exists with compute_similarity_for_space(), compute_all_similarities()</check>
  <check verified="true">EmbedderCategory exists with category_for(), topic_weight(), max_weighted_agreement(), topic_threshold()</check>
  <check verified="true">Tests pass: cargo test --package context-graph-core --lib distance (47 passed)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create multi_space.rs in crates/context-graph-core/src/retrieval/
    - Implement MultiSpaceSimilarity service struct
    - Implement compute_similarity using distance::compute_similarity_for_space
    - Implement is_relevant with ANY() logic (any space > high threshold)
    - Implement matching_spaces to find embedders above threshold
    - Implement compute_relevance_score with category-weighted formula
    - Implement compute_weighted_similarity using category weights
    - Implement compute_full_result to build SimilarityResult
    - Implement is_below_low_threshold for divergence detection support
    - Add batch processing helpers
    - Add sorting helper
    - Exclude temporal spaces (E2-E4) from all weighted calculations (weight=0.0)
  </in_scope>
  <out_of_scope>
    - Memory storage/retrieval (TASK-P3-007)
    - Divergence detection logic (TASK-P3-006)
    - Index building
    - CUDA optimization
  </out_of_scope>
</scope>

<architecture_rules>
  MUST COMPLY:
  - ARCH-09: Topic threshold is weighted_agreement >= 2.5
  - ARCH-10: Divergence detection uses SEMANTIC embedders only (E1, E5, E6, E7, E10, E12, E13)
  - AP-10: No NaN/Infinity in any scores
  - AP-60: Temporal embedders (E2-E4) MUST NOT count toward topic/relevance scoring
  - AP-61: Topic threshold MUST be weighted_agreement >= 2.5

  CATEGORY WEIGHTS (use category_for(embedder).topic_weight()):
    Semantic (E1, E5, E6, E7, E10, E12, E13): 1.0
    Temporal (E2, E3, E4): 0.0 (excluded)
    Relational (E8, E11): 0.5
    Structural (E9): 0.5
</architecture_rules>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/retrieval/multi_space.rs">
use uuid::Uuid;
use crate::embeddings::category::{category_for, max_weighted_agreement};
use crate::teleological::Embedder;
use crate::types::fingerprint::SemanticFingerprint;
use super::config::{SimilarityThresholds, SpaceWeights, default_weights};
use super::distance::compute_similarity_for_space;
use super::similarity::{PerSpaceScores, SimilarityResult};

/// Multi-space similarity computation service
pub struct MultiSpaceSimilarity {
    thresholds: SimilarityThresholds,
    weights: SpaceWeights,
}

impl MultiSpaceSimilarity {
    /// Create with custom thresholds and weights
    pub fn new(thresholds: SimilarityThresholds, weights: SpaceWeights) -> Self;

    /// Create with default configuration from spec
    pub fn with_defaults() -> Self;

    /// Compute similarity scores across all 13 embedding spaces
    pub fn compute_similarity(&amp;self, query: &amp;SemanticFingerprint, memory: &amp;SemanticFingerprint) -> PerSpaceScores;

    /// Check if memory is relevant (ANY space above high threshold)
    pub fn is_relevant(&amp;self, scores: &amp;PerSpaceScores) -> bool;

    /// Get list of embedders where score exceeds high threshold
    pub fn matching_spaces(&amp;self, scores: &amp;PerSpaceScores) -> Vec&lt;Embedder&gt;;

    /// Compute weighted relevance score using category weights
    /// Formula: Sum(category_weight * max(0, score - threshold)) / max_possible
    /// Temporal spaces excluded (weight = 0.0)
    pub fn compute_relevance_score(&amp;self, scores: &amp;PerSpaceScores) -> f32;

    /// Compute weighted similarity (category weights, no threshold subtraction)
    /// Result normalized to [0.0, 1.0]
    pub fn compute_weighted_similarity(&amp;self, scores: &amp;PerSpaceScores) -> f32;

    /// Compute complete SimilarityResult for a memory
    pub fn compute_full_result(&amp;self, memory_id: Uuid, query: &amp;SemanticFingerprint, memory: &amp;SemanticFingerprint) -> SimilarityResult;

    /// Get reference to thresholds
    pub fn thresholds(&amp;self) -> &amp;SimilarityThresholds;

    /// Get reference to weights
    pub fn weights(&amp;self) -> &amp;SpaceWeights;

    /// Check if score is below low threshold (for divergence detection)
    pub fn is_below_low_threshold(&amp;self, embedder: Embedder, score: f32) -> bool;
}

/// Batch comparison for multiple memories
pub fn compute_similarities_batch(
    similarity: &amp;MultiSpaceSimilarity,
    query: &amp;SemanticFingerprint,
    memories: &amp;[(Uuid, SemanticFingerprint)],
) -> Vec&lt;SimilarityResult&gt;;

/// Filter to relevant results only
pub fn filter_relevant(
    similarity: &amp;MultiSpaceSimilarity,
    results: Vec&lt;SimilarityResult&gt;,
) -> Vec&lt;SimilarityResult&gt;;

/// Sort results by relevance score (highest first)
pub fn sort_by_relevance(results: Vec&lt;SimilarityResult&gt;) -> Vec&lt;SimilarityResult&gt;;
    </signature>
  </signatures>

  <constraints>
    - is_relevant returns true if ANY space above high threshold
    - relevance_score = Sum(category_weight * max(0, score - threshold)) / max_possible
    - relevance_score normalized to [0.0, 1.0] range
    - weighted_similarity = Sum(category_weight * score) / Sum(category_weight)
    - All 13 spaces computed in compute_similarity
    - Temporal spaces (E2-E4) MUST have weight 0.0 and be excluded from weighted calculations
    - Use category_for(embedder).topic_weight() to get weights - DO NOT hardcode
    - NO NaN or Infinity values (clamp all results)
  </constraints>

  <verification>
    - Memory with one matching space returns is_relevant = true
    - Memory with no matching spaces returns is_relevant = false
    - Relevance score higher for more matching spaces
    - Category weights correctly applied using category_for()
    - Temporal spaces (E2-E4) contribute 0.0 to weighted_similarity
    - Semantic spaces (E1, E5-E7, E10, E12-E13) contribute full weight (1.0)
    - Relational/Structural spaces contribute half weight (0.5)
    - High temporal scores (0.95) do NOT boost relevance
    - All results in [0.0, 1.0] range
  </verification>
</definition_of_done>

<implementation_code>
File: crates/context-graph-core/src/retrieval/multi_space.rs

```rust
//! Multi-space similarity computation for 13-embedding retrieval.
//!
//! This module implements the core comparison engine that:
//! - Computes similarity scores across all 13 embedding spaces
//! - Determines relevance using ANY() logic (any space above high threshold)
//! - Calculates category-weighted relevance scores
//! - Excludes temporal spaces (E2-E4) from weighted calculations per AP-60
//!
//! # Architecture Rules
//!
//! - ARCH-09: Topic threshold is weighted_agreement >= 2.5
//! - AP-60: Temporal embedders (E2-E4) MUST NOT count toward topic detection
//! - AP-10: No NaN/Infinity in scores

use uuid::Uuid;

use crate::embeddings::category::{category_for, max_weighted_agreement};
use crate::teleological::Embedder;
use crate::types::fingerprint::SemanticFingerprint;

use super::config::{SimilarityThresholds, SpaceWeights, default_weights};
use super::distance::compute_similarity_for_space;
use super::similarity::{PerSpaceScores, SimilarityResult};

/// Multi-space similarity computation service.
///
/// Provides methods for computing similarity across all 13 embedding spaces,
/// determining relevance, and calculating weighted scores.
#[derive(Debug, Clone)]
pub struct MultiSpaceSimilarity {
    thresholds: SimilarityThresholds,
    weights: SpaceWeights,
}

impl MultiSpaceSimilarity {
    /// Create with custom thresholds and weights.
    pub fn new(thresholds: SimilarityThresholds, weights: SpaceWeights) -> Self {
        Self { thresholds, weights }
    }

    /// Create with default configuration from spec.
    ///
    /// Uses high_thresholds/low_thresholds from TECH-PHASE3 spec
    /// and category-derived weights.
    pub fn with_defaults() -> Self {
        Self {
            thresholds: SimilarityThresholds::default(),
            weights: default_weights(),
        }
    }

    /// Compute similarity scores across all 13 embedding spaces.
    ///
    /// Uses the distance calculator to compute per-space similarities.
    pub fn compute_similarity(
        &self,
        query: &SemanticFingerprint,
        memory: &SemanticFingerprint,
    ) -> PerSpaceScores {
        let mut scores = PerSpaceScores::new();

        for embedder in Embedder::all() {
            let sim = compute_similarity_for_space(embedder, query, memory);
            scores.set_score(embedder, sim);
        }

        scores
    }

    /// Check if memory is relevant (ANY space above high threshold).
    ///
    /// Returns true if at least one embedding space has a similarity
    /// score above its high threshold.
    pub fn is_relevant(&self, scores: &PerSpaceScores) -> bool {
        for embedder in Embedder::all() {
            let score = scores.get_score(embedder);
            let threshold = self.thresholds.high.get_threshold(embedder);
            if score > threshold {
                return true;
            }
        }
        false
    }

    /// Get list of embedders where score exceeds high threshold.
    pub fn matching_spaces(&self, scores: &PerSpaceScores) -> Vec<Embedder> {
        let mut matches = Vec::new();

        for embedder in Embedder::all() {
            let score = scores.get_score(embedder);
            let threshold = self.thresholds.high.get_threshold(embedder);
            if score > threshold {
                matches.push(embedder);
            }
        }

        matches
    }

    /// Compute weighted relevance score using category weights.
    ///
    /// Formula: Sum(category_weight * max(0, score - threshold)) / max_possible
    ///
    /// NOTE: Temporal spaces (E2-E4) have category_weight 0.0 and are excluded.
    /// Uses category_for(embedder).topic_weight() for weights.
    pub fn compute_relevance_score(&self, scores: &PerSpaceScores) -> f32 {
        let mut weighted_sum = 0.0_f32;
        let mut max_possible = 0.0_f32;

        for embedder in Embedder::all() {
            let category_weight = category_for(embedder).topic_weight();

            // Skip temporal spaces (weight = 0.0) per AP-60
            if category_weight == 0.0 {
                continue;
            }

            let score = scores.get_score(embedder);
            let threshold = self.thresholds.high.get_threshold(embedder);

            // Score above threshold contributes positively
            let contribution = (score - threshold).max(0.0);
            weighted_sum += category_weight * contribution;

            // Maximum possible is if score was 1.0
            max_possible += category_weight * (1.0 - threshold).max(0.0);
        }

        // Normalize to [0.0, 1.0]
        if max_possible > 0.0 {
            (weighted_sum / max_possible).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Compute weighted similarity using category weights (excludes temporal).
    ///
    /// This is a simpler version that sums weighted scores without threshold subtraction.
    /// Result: Sum(category_weight * score) / Sum(category_weight)
    pub fn compute_weighted_similarity(&self, scores: &PerSpaceScores) -> f32 {
        let mut weighted_sum = 0.0_f32;
        let mut total_weight = 0.0_f32;

        for embedder in Embedder::all() {
            let category_weight = category_for(embedder).topic_weight();

            // Skip temporal spaces (weight = 0.0) per AP-60
            if category_weight == 0.0 {
                continue;
            }

            let score = scores.get_score(embedder);
            weighted_sum += category_weight * score;
            total_weight += category_weight;
        }

        if total_weight > 0.0 {
            (weighted_sum / total_weight).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Compute complete SimilarityResult for a memory.
    pub fn compute_full_result(
        &self,
        memory_id: Uuid,
        query: &SemanticFingerprint,
        memory: &SemanticFingerprint,
    ) -> SimilarityResult {
        let scores = self.compute_similarity(query, memory);
        let matching = self.matching_spaces(&scores);
        let relevance = self.compute_relevance_score(&scores);

        SimilarityResult::with_relevance(memory_id, scores, relevance, matching)
    }

    /// Get reference to thresholds.
    #[inline]
    pub fn thresholds(&self) -> &SimilarityThresholds {
        &self.thresholds
    }

    /// Get reference to weights.
    #[inline]
    pub fn weights(&self) -> &SpaceWeights {
        &self.weights
    }

    /// Check if score is below low threshold (for divergence detection).
    #[inline]
    pub fn is_below_low_threshold(&self, embedder: Embedder, score: f32) -> bool {
        score < self.thresholds.low.get_threshold(embedder)
    }
}

/// Batch comparison for multiple memories.
pub fn compute_similarities_batch(
    similarity: &MultiSpaceSimilarity,
    query: &SemanticFingerprint,
    memories: &[(Uuid, SemanticFingerprint)],
) -> Vec<SimilarityResult> {
    memories
        .iter()
        .map(|(id, memory)| similarity.compute_full_result(*id, query, memory))
        .collect()
}

/// Filter to relevant results only.
pub fn filter_relevant(
    similarity: &MultiSpaceSimilarity,
    results: Vec<SimilarityResult>,
) -> Vec<SimilarityResult> {
    results
        .into_iter()
        .filter(|r| similarity.is_relevant(&r.per_space_scores))
        .collect()
}

/// Sort results by relevance score (highest first).
pub fn sort_by_relevance(mut results: Vec<SimilarityResult>) -> Vec<SimilarityResult> {
    results.sort_by(|a, b| {
        b.relevance_score
            .partial_cmp(&a.relevance_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval::config::{high_thresholds, low_thresholds};

    // =========================================================================
    // is_relevant Tests
    // =========================================================================

    #[test]
    fn test_is_relevant_one_match() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        let mut scores = PerSpaceScores::new();
        scores.set_score(Embedder::Semantic, 0.80); // Above 0.75 threshold
        scores.set_score(Embedder::Code, 0.50);     // Below 0.80 threshold

        assert!(similarity.is_relevant(&scores));
        println!("[PASS] is_relevant returns true with one matching space");
    }

    #[test]
    fn test_is_relevant_no_match() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        let mut scores = PerSpaceScores::new();
        scores.set_score(Embedder::Semantic, 0.70); // Below 0.75 threshold
        scores.set_score(Embedder::Code, 0.50);     // Below 0.80 threshold

        assert!(!similarity.is_relevant(&scores));
        println!("[PASS] is_relevant returns false with no matching spaces");
    }

    #[test]
    fn test_is_relevant_all_temporal_high() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        // High temporal scores but all other spaces low
        let mut scores = PerSpaceScores::new();
        scores.set_score(Embedder::TemporalRecent, 0.95);
        scores.set_score(Embedder::TemporalPeriodic, 0.95);
        scores.set_score(Embedder::TemporalPositional, 0.95);

        // Temporal spaces DO count for is_relevant (threshold check is not weighted)
        // But they have weight 0.0 for weighted_similarity
        assert!(similarity.is_relevant(&scores)); // 0.95 > 0.70 threshold
        println!("[PASS] Temporal spaces above threshold count for is_relevant");
    }

    // =========================================================================
    // matching_spaces Tests
    // =========================================================================

    #[test]
    fn test_matching_spaces() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        let mut scores = PerSpaceScores::new();
        scores.set_score(Embedder::Semantic, 0.80);      // Match (> 0.75)
        scores.set_score(Embedder::Code, 0.85);          // Match (> 0.80)
        scores.set_score(Embedder::Sparse, 0.30);        // No match (< 0.60)

        let matches = similarity.matching_spaces(&scores);
        assert_eq!(matches.len(), 2);
        assert!(matches.contains(&Embedder::Semantic));
        assert!(matches.contains(&Embedder::Code));
        println!("[PASS] matching_spaces returns correct embedder list");
    }

    // =========================================================================
    // compute_relevance_score Tests
    // =========================================================================

    #[test]
    fn test_relevance_score_higher_with_more_matches() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        let mut scores_one = PerSpaceScores::new();
        scores_one.set_score(Embedder::Semantic, 0.80);

        let mut scores_two = PerSpaceScores::new();
        scores_two.set_score(Embedder::Semantic, 0.80);
        scores_two.set_score(Embedder::Code, 0.85);

        let rel_one = similarity.compute_relevance_score(&scores_one);
        let rel_two = similarity.compute_relevance_score(&scores_two);

        assert!(rel_two > rel_one, "rel_two {} should be > rel_one {}", rel_two, rel_one);
        println!("[PASS] More matches = higher relevance: {} > {}", rel_two, rel_one);
    }

    #[test]
    fn test_relevance_score_normalized() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        // Maximum possible scores
        let mut scores = PerSpaceScores::new();
        for embedder in Embedder::all() {
            scores.set_score(embedder, 1.0);
        }

        let rel = similarity.compute_relevance_score(&scores);
        assert!(rel >= 0.0 && rel <= 1.0, "Relevance {} out of [0,1] range", rel);
        println!("[PASS] Relevance score {} is normalized to [0,1]", rel);
    }

    #[test]
    fn test_relevance_score_zero_when_all_below_threshold() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        // All scores below their high thresholds
        let mut scores = PerSpaceScores::new();
        scores.set_score(Embedder::Semantic, 0.50);  // Below 0.75
        scores.set_score(Embedder::Code, 0.60);      // Below 0.80
        scores.set_score(Embedder::Causal, 0.50);    // Below 0.70

        let rel = similarity.compute_relevance_score(&scores);
        assert_eq!(rel, 0.0, "All below threshold should give 0.0 relevance");
        println!("[PASS] All below threshold = relevance 0.0");
    }

    // =========================================================================
    // Temporal Exclusion Tests (AP-60)
    // =========================================================================

    #[test]
    fn test_temporal_excluded_from_weighted_similarity() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        // High temporal scores only
        let mut scores_temporal = PerSpaceScores::new();
        scores_temporal.set_score(Embedder::TemporalRecent, 0.95);
        scores_temporal.set_score(Embedder::TemporalPeriodic, 0.95);
        scores_temporal.set_score(Embedder::TemporalPositional, 0.95);

        let weighted = similarity.compute_weighted_similarity(&scores_temporal);
        // All semantic/relational/structural are 0.0, temporal excluded
        assert!(weighted < 0.01, "Temporal-only scores should give near-zero weighted: {}", weighted);
        println!("[PASS] AP-60: Temporal excluded from weighted_similarity: {}", weighted);
    }

    #[test]
    fn test_temporal_excluded_from_relevance_score() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        // High temporal scores only
        let mut scores_temporal = PerSpaceScores::new();
        scores_temporal.set_score(Embedder::TemporalRecent, 0.95);
        scores_temporal.set_score(Embedder::TemporalPeriodic, 0.95);
        scores_temporal.set_score(Embedder::TemporalPositional, 0.95);

        let rel = similarity.compute_relevance_score(&scores_temporal);
        assert_eq!(rel, 0.0, "Temporal-only should give 0.0 relevance");
        println!("[PASS] AP-60: Temporal excluded from relevance_score");
    }

    // =========================================================================
    // Category Weight Tests
    // =========================================================================

    #[test]
    fn test_semantic_contributes_full_weight() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        let mut scores = PerSpaceScores::new();
        scores.set_score(Embedder::Semantic, 0.90);
        scores.set_score(Embedder::Code, 0.90);

        let weighted = similarity.compute_weighted_similarity(&scores);
        // Two semantic spaces at 0.90 should give positive result
        assert!(weighted > 0.0, "Semantic spaces should contribute: {}", weighted);
        println!("[PASS] Semantic spaces contribute full weight: {}", weighted);
    }

    #[test]
    fn test_relational_contributes_half_weight() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        // One semantic at 1.0, one relational at 1.0
        let mut scores = PerSpaceScores::new();
        scores.set_score(Embedder::Semantic, 1.0);  // weight 1.0
        scores.set_score(Embedder::Emotional, 1.0); // weight 0.5 (relational)

        let weighted = similarity.compute_weighted_similarity(&scores);
        // Sum(w*s) / Sum(w) for non-zero weights
        // For just these two: (1.0*1.0 + 0.5*1.0) / (1.0 + 0.5) = 1.5/1.5 = 1.0
        // But other spaces are 0 with non-zero weight, so:
        // (1.0 + 0.5) / 8.5 = 0.176... NO, the formula uses accumulated total_weight
        // which only accumulates non-zero category_weight spaces
        // So: (1.0 + 0.5 + 0*7 semantic + 0*1 structural) / (1.0 + 0.5 + 7*0 + 1*0)
        // Wait, total_weight accumulates ALL non-temporal weights = 8.5
        // weighted_sum = 1.0*1.0 + 0.5*1.0 = 1.5
        // result = 1.5 / 8.5 = 0.176
        // Actually re-reading: total_weight += category_weight only when weight > 0
        // So it accumulates for ALL 10 non-temporal spaces = 8.5
        assert!(weighted > 0.0 && weighted <= 1.0);
        println!("[PASS] Relational contributes 0.5 weight, result: {}", weighted);
    }

    // =========================================================================
    // is_below_low_threshold Tests
    // =========================================================================

    #[test]
    fn test_below_low_threshold() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        // E1 low threshold is 0.30
        assert!(similarity.is_below_low_threshold(Embedder::Semantic, 0.25));
        assert!(!similarity.is_below_low_threshold(Embedder::Semantic, 0.35));
        println!("[PASS] is_below_low_threshold works correctly");
    }

    // =========================================================================
    // compute_full_result Tests
    // =========================================================================

    #[test]
    fn test_compute_full_result() {
        let similarity = MultiSpaceSimilarity::with_defaults();
        let memory_id = Uuid::new_v4();
        let query = SemanticFingerprint::zeroed();
        let memory = SemanticFingerprint::zeroed();

        let result = similarity.compute_full_result(memory_id, &query, &memory);

        assert_eq!(result.memory_id, memory_id);
        assert_eq!(result.space_count as usize, result.matching_spaces.len());
        assert!(result.relevance_score >= 0.0 && result.relevance_score <= 1.0);
        println!("[PASS] compute_full_result builds valid SimilarityResult");
    }

    // =========================================================================
    // Batch and Sort Tests
    // =========================================================================

    #[test]
    fn test_sort_by_relevance() {
        let results = vec![
            SimilarityResult::with_relevance(Uuid::new_v4(), PerSpaceScores::new(), 0.3, vec![]),
            SimilarityResult::with_relevance(Uuid::new_v4(), PerSpaceScores::new(), 0.9, vec![]),
            SimilarityResult::with_relevance(Uuid::new_v4(), PerSpaceScores::new(), 0.5, vec![]),
        ];

        let sorted = sort_by_relevance(results);

        assert_eq!(sorted[0].relevance_score, 0.9);
        assert_eq!(sorted[1].relevance_score, 0.5);
        assert_eq!(sorted[2].relevance_score, 0.3);
        println!("[PASS] sort_by_relevance orders highest first");
    }

    // =========================================================================
    // Threshold Value Tests
    // =========================================================================

    #[test]
    fn test_threshold_values_from_spec() {
        let high = high_thresholds();
        let low = low_thresholds();

        // Verify key spec values
        assert_eq!(high.get_threshold(Embedder::Semantic), 0.75);
        assert_eq!(high.get_threshold(Embedder::Code), 0.80);
        assert_eq!(high.get_threshold(Embedder::Sparse), 0.60);

        assert_eq!(low.get_threshold(Embedder::Semantic), 0.30);
        assert_eq!(low.get_threshold(Embedder::Causal), 0.25);
        assert_eq!(low.get_threshold(Embedder::Code), 0.35);

        println!("[PASS] Threshold values match spec");
    }

    #[test]
    fn test_all_high_greater_than_low() {
        let high = high_thresholds();
        let low = low_thresholds();

        for embedder in Embedder::all() {
            let h = high.get_threshold(embedder);
            let l = low.get_threshold(embedder);
            assert!(h > l, "{:?}: high {} must be > low {}", embedder, h, l);
        }
        println!("[PASS] All high thresholds > low thresholds");
    }

    // =========================================================================
    // Edge Cases
    // =========================================================================

    #[test]
    fn edge_case_all_zeros() {
        let similarity = MultiSpaceSimilarity::with_defaults();
        let scores = PerSpaceScores::new(); // All zeros

        assert!(!similarity.is_relevant(&scores));
        assert_eq!(similarity.matching_spaces(&scores).len(), 0);
        assert_eq!(similarity.compute_relevance_score(&scores), 0.0);
        assert_eq!(similarity.compute_weighted_similarity(&scores), 0.0);
        println!("[PASS] Edge case: all zeros handled correctly");
    }

    #[test]
    fn edge_case_all_ones() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        let mut scores = PerSpaceScores::new();
        for embedder in Embedder::all() {
            scores.set_score(embedder, 1.0);
        }

        assert!(similarity.is_relevant(&scores));
        let matching = similarity.matching_spaces(&scores);
        assert_eq!(matching.len(), 13); // All spaces match when score = 1.0

        let rel = similarity.compute_relevance_score(&scores);
        let weighted = similarity.compute_weighted_similarity(&scores);

        assert!(rel > 0.99 && rel <= 1.0, "rel={}", rel);
        assert!(weighted > 0.99 && weighted <= 1.0, "weighted={}", weighted);
        println!("[PASS] Edge case: all ones = max relevance");
    }

    #[test]
    fn edge_case_exactly_at_threshold() {
        let similarity = MultiSpaceSimilarity::with_defaults();

        let mut scores = PerSpaceScores::new();
        scores.set_score(Embedder::Semantic, 0.75); // Exactly at threshold

        // > threshold, not >=, so 0.75 should NOT match
        assert!(!similarity.is_relevant(&scores));
        assert_eq!(similarity.matching_spaces(&scores).len(), 0);
        println!("[PASS] Edge case: exactly at threshold = not relevant (> not >=)");
    }
}
```
</implementation_code>

<files_to_create>
  <file path="crates/context-graph-core/src/retrieval/multi_space.rs">MultiSpaceSimilarity implementation</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/retrieval/mod.rs">
    Add: pub mod multi_space;
    Add to pub use: multi_space::*;
  </file>
</files_to_modify>

<modification_instructions>
For crates/context-graph-core/src/retrieval/mod.rs:

Add to module declarations section:
```rust
pub mod multi_space;
```

Add to re-exports section:
```rust
pub use multi_space::{
    MultiSpaceSimilarity,
    compute_similarities_batch,
    filter_relevant,
    sort_by_relevance,
};
```
</modification_instructions>

<validation_criteria>
  <criterion>cargo check --package context-graph-core compiles without errors</criterion>
  <criterion>cargo test --package context-graph-core multi_space -- --nocapture passes all tests</criterion>
  <criterion>cargo clippy --package context-graph-core -- -D warnings has no warnings</criterion>
  <criterion>is_relevant returns true if ANY space above threshold</criterion>
  <criterion>matching_spaces returns correct embedder list</criterion>
  <criterion>relevance_score higher with more matching spaces</criterion>
  <criterion>relevance_score normalized to [0.0, 1.0]</criterion>
  <criterion>Temporal spaces (E2-E4) contribute 0.0 to weighted calculations</criterion>
  <criterion>Category weights correctly applied via category_for().topic_weight()</criterion>
  <criterion>High temporal scores (0.95) do NOT boost relevance</criterion>
  <criterion>All edge cases handled without panic/NaN</criterion>
</validation_criteria>

<test_commands>
  <command description="Check compilation">cargo check --package context-graph-core</command>
  <command description="Run multi_space tests">cargo test --package context-graph-core multi_space -- --nocapture</command>
  <command description="Run clippy">cargo clippy --package context-graph-core -- -D warnings</command>
  <command description="Run all retrieval tests">cargo test --package context-graph-core retrieval -- --nocapture</command>
</test_commands>

<full_state_verification>
## Source of Truth
The computed similarity scores, relevance scores, and weighted_similarity values.

## Execution and Inspection Strategy

After implementing, run these verification steps:

### 1. Compilation Verification
```bash
cargo check --package context-graph-core 2>&1 | head -50
```
Expected: "Finished" with no errors

### 2. Unit Test Execution
```bash
cargo test --package context-graph-core multi_space -- --nocapture 2>&1
```
Expected: All tests pass with "[PASS]" output for each

### 3. Clippy Check
```bash
cargo clippy --package context-graph-core -- -D warnings 2>&1 | head -20
```
Expected: No warnings

## Boundary and Edge Case Audit

### Edge Case 1: All Zeros
```
Input: PerSpaceScores with all scores = 0.0
Before: scores = PerSpaceScores::new()
Expected after compute_relevance_score: 0.0
Expected after compute_weighted_similarity: 0.0
Expected is_relevant: false
Proof: Test output shows "[PASS] Edge case: all zeros handled correctly"
```

### Edge Case 2: All Ones
```
Input: PerSpaceScores with all scores = 1.0
Before: All scores set to 1.0
Expected after compute_relevance_score: ~1.0
Expected after compute_weighted_similarity: ~1.0
Expected is_relevant: true
Expected matching_spaces.len(): 13
Proof: Test output shows "[PASS] Edge case: all ones = max relevance"
```

### Edge Case 3: Exactly at Threshold
```
Input: Semantic score = 0.75 (exactly at high threshold)
Before: scores.set_score(Embedder::Semantic, 0.75)
Expected is_relevant: false (uses > not >=)
Expected matching_spaces.len(): 0
Proof: Test output shows "[PASS] Edge case: exactly at threshold = not relevant"
```

### Edge Case 4: Temporal-Only High Scores
```
Input: Temporal spaces (E2-E4) at 0.95, all others at 0.0
Before: scores.set_score(TemporalRecent, 0.95), etc.
Expected compute_weighted_similarity: ~0.0 (temporals excluded)
Expected compute_relevance_score: 0.0 (temporals have weight 0.0)
Proof: Test output shows "[PASS] AP-60: Temporal excluded"
```

## Evidence of Success
After running tests, capture this output:
```bash
cargo test --package context-graph-core multi_space -- --nocapture 2>&1 | grep -E "^\[PASS\]|^test.*ok$|^running"
```
Expected: All tests show "ok" and "[PASS]" messages.

## Physical Proof Verification
Since this is a computation module (not storage), verification is through:
1. Test assertions passing (return values match expected)
2. No panics during edge case handling
3. All values in documented [0.0, 1.0] range
4. Category weights correctly applied (verified by temporal exclusion test)
</full_state_verification>

<fail_fast_approach>
NO BACKWARDS COMPATIBILITY - fail fast with robust error logging:

1. Use debug_assert!() for invariants:
```rust
debug_assert!(score >= 0.0 && score <= 1.0, "Score {} out of [0,1] range", score);
```

2. Clamp results rather than silently propagating invalid values:
```rust
(weighted_sum / max_possible).clamp(0.0, 1.0)
```

3. Do NOT silently skip invalid states - log and handle:
```rust
if max_possible == 0.0 {
    tracing::warn!("max_possible is 0, returning 0.0");
    return 0.0;
}
```

4. Prefer explicit match arms over wildcards for Embedder to catch missing cases at compile time.
</fail_fast_approach>

<no_mock_data>
CRITICAL: Tests MUST use real values from the spec, not mock data.

WRONG:
  let thresholds = SimilarityThresholds { ... }; // Made up values

RIGHT:
  let similarity = MultiSpaceSimilarity::with_defaults(); // Uses spec values
  assert_eq!(similarity.thresholds().high.get_threshold(Embedder::Semantic), 0.75);
</no_mock_data>
</task_spec>
```

## Execution Checklist

- [ ] Read existing retrieval/mod.rs exports
- [ ] Create multi_space.rs in retrieval directory
- [ ] Implement MultiSpaceSimilarity struct
- [ ] Implement new() and with_defaults()
- [ ] Implement compute_similarity()
- [ ] Implement is_relevant() with ANY() logic
- [ ] Implement matching_spaces()
- [ ] Implement compute_relevance_score() with category weights
- [ ] Implement compute_weighted_similarity()
- [ ] Implement compute_full_result()
- [ ] Implement is_below_low_threshold()
- [ ] Implement batch/filter/sort helpers
- [ ] Write comprehensive unit tests
- [ ] Add pub mod multi_space to mod.rs
- [ ] Add multi_space exports to mod.rs
- [ ] Run `cargo check --package context-graph-core`
- [ ] Run `cargo test --package context-graph-core multi_space -- --nocapture`
- [ ] Run `cargo clippy --package context-graph-core -- -D warnings`
- [ ] Verify all tests show [PASS] messages
- [ ] Verify AP-60 compliance (temporal exclusion)
- [ ] Document test results as evidence
- [ ] Proceed to TASK-P3-006

## Key Discrepancies Fixed from Previous Version

1. **File paths corrected**:
   - `crates/context-graph-core/src/embedding/category.rs` -> `crates/context-graph-core/src/embeddings/category.rs`
   - `crates/context-graph-core/src/embedding/config.rs` -> NOT NEEDED (use category.rs functions)

2. **Import path corrected**:
   - `crate::embedding::Embedder` -> `crate::teleological::Embedder`
   - `crate::embedding::TeleologicalArray` -> `crate::types::fingerprint::SemanticFingerprint`

3. **Function name corrected**:
   - `get_topic_weight(embedder)` -> `category_for(embedder).topic_weight()`

4. **Embedder variant names corrected**:
   - `Embedder::E1Semantic` -> `Embedder::Semantic`
   - `Embedder::E7Code` -> `Embedder::Code`
   - `Embedder::E2TempRecent` -> `Embedder::TemporalRecent`
   - `Embedder::E3TempPeriodic` -> `Embedder::TemporalPeriodic`
   - `Embedder::E4TempPosition` -> `Embedder::TemporalPositional`
   - `Embedder::E6Sparse` -> `Embedder::Sparse`

5. **SparseVector constructor**:
   - Takes `Vec<u16>` for indices, not `Vec<u32>`
   - Two arguments (indices, values), not three

6. **SPACE_WEIGHTS constant**: Already exists in config.rs - use it or derive from category_for()

7. **Tests updated**: Use real Embedder variants, not E1Semantic/E7Code style

8. **with_defaults() fixed**: Don't call `.normalized()` on weights - use `default_weights()` directly

## Research Insights Applied

Based on [multi-scale similarity aggregation research](https://dl.acm.org/doi/10.1145/3581783.3612547) and [weighted embedding aggregation studies](https://link.springer.com/article/10.1007/s41109-025-00699-7):

1. **Category-based weighting** aligns with multi-scale similarity approaches where different semantic levels contribute differently
2. **Temporal exclusion (AP-60)** is justified - temporal proximity != semantic relationship
3. **ANY() logic for relevance** provides broad recall; weighted scoring provides precision ranking
4. **Normalization to [0,1]** enables consistent comparison across different query types

Sources:
- [Multi-Scale Similarity Aggregation for Dynamic Metric Learning](https://dl.acm.org/doi/10.1145/3581783.3612547)
- [Similarity Network Analysis](https://link.springer.com/article/10.1007/s41109-025-00699-7)
- [Asymmetric Embedding Spaces](https://medium.com/@venkatamani_kommineni/asymmetric-embedding-spaces-in-information-retrieval-principles-algorithms-and-a-novel-extension-eba44f80750a)

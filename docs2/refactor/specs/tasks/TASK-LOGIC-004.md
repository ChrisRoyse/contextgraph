# TASK-LOGIC-004: Teleological Comparator

```xml
<task_spec id="TASK-LOGIC-004" version="1.0">
<metadata>
  <title>Implement TeleologicalComparator</title>
  <status>todo</status>
  <layer>logic</layer>
  <sequence>14</sequence>
  <implements>
    <requirement_ref>REQ-COMPARATOR-01</requirement_ref>
    <requirement_ref>REQ-APPLES-TO-APPLES-02</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORE-004</task_ref>
    <task_ref>TASK-LOGIC-001</task_ref>
    <task_ref>TASK-LOGIC-002</task_ref>
    <task_ref>TASK-LOGIC-003</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_days>3</estimated_days>
</metadata>

<context>
The central comparison engine that combines all similarity functions to compare
teleological arrays. Routes to correct similarity function based on embedder type,
applies weights and matrices, and returns detailed comparison results.
</context>

<objective>
Create the TeleologicalComparator struct that implements array-to-array comparison
using all comparison types (single embedder, group, weighted, matrix), returning
per-embedder scores and overall similarity.
</objective>

<rationale>
The comparator is the key to "apples-to-apples" comparison:
1. Both inputs are full 13-embedder arrays
2. Per-embedder similarity computed with appropriate function
3. Results aggregated according to comparison type
4. Detailed breakdown enables interpretability

This replaces the broken projection-based alignment with mathematically sound
comparison.
</rationale>

<input_context_files>
  <file purpose="comparison_types">crates/context-graph-core/src/teleology/comparison.rs</file>
  <file purpose="dense_sim">crates/context-graph-core/src/teleology/similarity/dense.rs</file>
  <file purpose="sparse_sim">crates/context-graph-core/src/teleology/similarity/sparse.rs</file>
  <file purpose="token_sim">crates/context-graph-core/src/teleology/similarity/token_level.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORE-004 complete (ComparisonType, SearchMatrix exist)</check>
  <check>TASK-LOGIC-001 complete (dense similarity)</check>
  <check>TASK-LOGIC-002 complete (sparse similarity)</check>
  <check>TASK-LOGIC-003 complete (token-level similarity)</check>
</prerequisites>

<scope>
  <in_scope>
    <item>Create TeleologicalComparator struct</item>
    <item>Implement compare() method routing to correct similarity</item>
    <item>Support all ComparisonType variants</item>
    <item>Apply search matrices and weights correctly</item>
    <item>Return detailed per-embedder scores</item>
    <item>Implement BatchComparator for parallel comparisons</item>
  </in_scope>
  <out_of_scope>
    <item>Search engine (TASK-LOGIC-005 through 008)</item>
    <item>Goal discovery (TASK-LOGIC-009)</item>
    <item>Drift detection (TASK-LOGIC-010)</item>
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/teleology/comparator.rs">
      use crate::teleology::array::{TeleologicalArray, EmbedderOutput};
      use crate::teleology::comparison::{ComparisonType, ComparisonResult, SearchMatrix};
      use crate::teleology::embedder::Embedder;

      /// Compares teleological arrays using various strategies.
      #[derive(Debug, Clone)]
      pub struct TeleologicalComparator {
          default_comparison: ComparisonType,
      }

      impl TeleologicalComparator {
          pub fn new() -> Self;
          pub fn with_default_comparison(comparison: ComparisonType) -> Self;

          /// Compare two teleological arrays.
          pub fn compare(
              &self,
              a: &TeleologicalArray,
              b: &TeleologicalArray,
              comparison_type: &ComparisonType,
          ) -> ComparisonResult;

          /// Compare using the default comparison type.
          pub fn compare_default(
              &self,
              a: &TeleologicalArray,
              b: &TeleologicalArray,
          ) -> ComparisonResult;

          /// Compare a single embedder pair.
          fn compare_embedder(
              &self,
              a: &EmbedderOutput,
              b: &EmbedderOutput,
              embedder: Embedder,
          ) -> Option<f32>;
      }

      /// Batch comparator for parallel processing.
      pub struct BatchComparator {
          comparator: TeleologicalComparator,
      }

      impl BatchComparator {
          pub fn new() -> Self;

          /// Compare one reference against many targets in parallel.
          pub fn compare_one_to_many(
              &self,
              reference: &TeleologicalArray,
              targets: &[TeleologicalArray],
              comparison_type: &ComparisonType,
          ) -> Vec<ComparisonResult>;
      }
    </signature>
  </signatures>

  <constraints>
    <constraint>Routes to correct similarity function per embedder type</constraint>
    <constraint>Handles Pending/Failed embeddings gracefully (skip)</constraint>
    <constraint>Weights sum to 1.0 in weighted comparison</constraint>
    <constraint>Matrix strategies apply full 13x13 weights</constraint>
    <constraint>BatchComparator uses Rayon for parallelism</constraint>
  </constraints>

  <verification>
    <command>cargo test -p context-graph-core comparator</command>
    <command>cargo bench -p context-graph-core comparator</command>
  </verification>
</definition_of_done>

<pseudo_code>
// crates/context-graph-core/src/teleology/comparator.rs

use crate::teleology::array::{TeleologicalArray, EmbedderOutput};
use crate::teleology::comparison::{ComparisonType, ComparisonResult, SearchMatrix, EmbedderWeights};
use crate::teleology::embedder::{Embedder, EmbedderDims, EmbedderGroup};
use crate::teleology::similarity::{dense, sparse, token_level};
use rayon::prelude::*;

#[derive(Debug, Clone)]
pub struct TeleologicalComparator {
    default_comparison: ComparisonType,
}

impl TeleologicalComparator {
    pub fn new() -> Self {
        Self {
            default_comparison: ComparisonType::WeightedFull(EmbedderWeights::uniform()),
        }
    }

    pub fn with_default_comparison(comparison: ComparisonType) -> Self {
        Self { default_comparison: comparison }
    }

    /// Compare two teleological arrays.
    pub fn compare(
        &self,
        a: &TeleologicalArray,
        b: &TeleologicalArray,
        comparison_type: &ComparisonType,
    ) -> ComparisonResult {
        // Compute per-embedder similarities
        let per_embedder: [Option<f32>; 13] = std::array::from_fn(|idx| {
            let embedder = Embedder::from_index(idx).unwrap();
            self.compare_embedder(a.get(embedder), b.get(embedder), embedder)
        });

        // Aggregate based on comparison type
        let overall = match comparison_type {
            ComparisonType::SingleEmbedder(e) => {
                per_embedder[e.index()].unwrap_or(0.0)
            }
            ComparisonType::EmbedderGroup(group) => {
                self.aggregate_group(&per_embedder, *group)
            }
            ComparisonType::WeightedFull(weights) => {
                self.aggregate_weighted(&per_embedder, weights)
            }
            ComparisonType::MatrixStrategy(matrix) => {
                self.aggregate_matrix(&per_embedder, matrix)
            }
        };

        // Find dominant embedder
        let dominant = per_embedder.iter()
            .enumerate()
            .filter_map(|(i, &s)| s.map(|score| (i, score)))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(i, _)| Embedder::from_index(i).unwrap());

        ComparisonResult {
            overall_similarity: overall,
            per_embedder,
            comparison_type: comparison_type.clone(),
            coherence: self.compute_coherence(&per_embedder),
            dominant_embedder: dominant,
        }
    }

    /// Compare a single embedder pair, routing to correct similarity function.
    fn compare_embedder(
        &self,
        a: &EmbedderOutput,
        b: &EmbedderOutput,
        embedder: Embedder,
    ) -> Option<f32> {
        match (a, b) {
            (EmbedderOutput::Dense(va), EmbedderOutput::Dense(vb)) => {
                dense::cosine_similarity(va, vb).ok()
            }
            (EmbedderOutput::Sparse(sa), EmbedderOutput::Sparse(sb)) => {
                Some(sparse::sparse_cosine_similarity(sa, sb))
            }
            (EmbedderOutput::TokenLevel(ta), EmbedderOutput::TokenLevel(tb)) => {
                Some(token_level::max_sim(ta, tb))
            }
            (EmbedderOutput::Binary(ba), EmbedderOutput::Binary(bb)) => {
                Some(self.binary_similarity(ba, bb))
            }
            (EmbedderOutput::Pending, _) | (_, EmbedderOutput::Pending) => None,
            (EmbedderOutput::Failed(_), _) | (_, EmbedderOutput::Failed(_)) => None,
            _ => None,  // Type mismatch
        }
    }

    fn aggregate_group(&self, scores: &[Option<f32>; 13], group: EmbedderGroup) -> f32 {
        let mask = group.embedders();
        let (sum, count) = mask.iter()
            .filter_map(|e| scores[e.index()])
            .fold((0.0, 0), |(sum, count), s| (sum + s, count + 1));

        if count > 0 { sum / count as f32 } else { 0.0 }
    }

    fn aggregate_weighted(&self, scores: &[Option<f32>; 13], weights: &EmbedderWeights) -> f32 {
        let mut total = 0.0f32;
        let mut weight_sum = 0.0f32;

        for embedder in Embedder::all() {
            let weight = weights.get(embedder);
            if weight > 0.0 {
                if let Some(score) = scores[embedder.index()] {
                    total += score * weight;
                    weight_sum += weight;
                }
            }
        }

        if weight_sum > 0.0 { total / weight_sum } else { 0.0 }
    }

    fn aggregate_matrix(&self, scores: &[Option<f32>; 13], matrix: &SearchMatrix) -> f32 {
        // For diagonal matrices, same as weighted
        // For full matrices, cross-embedder correlation analysis
        let mut total = 0.0f32;
        let mut weight_sum = 0.0f32;

        for i in 0..13 {
            for j in 0..13 {
                let ei = Embedder::from_index(i).unwrap();
                let ej = Embedder::from_index(j).unwrap();
                let weight = matrix.get(ei, ej);

                if weight > 0.0 {
                    if let (Some(si), Some(sj)) = (scores[i], scores[j]) {
                        // For diagonal: i == j, use score directly
                        // For off-diagonal: correlation between embedders
                        if i == j {
                            total += si * weight;
                        } else {
                            // Cross-correlation: geometric mean of scores
                            total += (si * sj).sqrt() * weight;
                        }
                        weight_sum += weight;
                    }
                }
            }
        }

        if weight_sum > 0.0 { total / weight_sum } else { 0.0 }
    }

    fn compute_coherence(&self, scores: &[Option<f32>; 13]) -> Option<f32> {
        let valid: Vec<f32> = scores.iter().filter_map(|&s| s).collect();
        if valid.len() < 2 {
            return None;
        }

        let mean = valid.iter().sum::<f32>() / valid.len() as f32;
        let variance = valid.iter()
            .map(|&s| (s - mean).powi(2))
            .sum::<f32>() / valid.len() as f32;

        // Coherence: inverse of coefficient of variation
        if mean > 0.0 {
            Some(1.0 / (1.0 + variance.sqrt() / mean))
        } else {
            Some(0.0)
        }
    }

    fn binary_similarity(&self, a: &[u8], b: &[u8]) -> f32 {
        // Hamming similarity for HDC binary vectors
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        let matching_bits: u32 = a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x ^ y).count_zeros())
            .sum();

        let total_bits = (a.len() * 8) as f32;
        matching_bits as f32 / total_bits
    }
}

pub struct BatchComparator {
    comparator: TeleologicalComparator,
}

impl BatchComparator {
    pub fn new() -> Self {
        Self {
            comparator: TeleologicalComparator::new(),
        }
    }

    pub fn compare_one_to_many(
        &self,
        reference: &TeleologicalArray,
        targets: &[TeleologicalArray],
        comparison_type: &ComparisonType,
    ) -> Vec<ComparisonResult> {
        targets.par_iter()
            .map(|target| self.comparator.compare(reference, target, comparison_type))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_compare_identical() {
        let comparator = TeleologicalComparator::new();
        let array = TeleologicalArray::new(Uuid::new_v4());

        let result = comparator.compare_default(&array, &array);

        // Pending embeddings should result in None scores
        assert!(result.per_embedder.iter().all(|s| s.is_none()));
    }

    // More tests with actual embeddings...
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/teleology/comparator.rs">
    TeleologicalComparator implementation
  </file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/teleology/mod.rs">
    Add: pub mod comparator;
  </file>
  <file path="crates/context-graph-core/Cargo.toml">
    Add: rayon dependency
  </file>
</files_to_modify>

<validation_criteria>
  <criterion>SingleEmbedder returns that embedder's score</criterion>
  <criterion>WeightedFull correctly weights per-embedder scores</criterion>
  <criterion>BatchComparator processes in parallel</criterion>
  <criterion>Pending/Failed embeddings are skipped gracefully</criterion>
  <criterion>Coherence measure is meaningful</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-core comparator -- --nocapture</command>
  <command>cargo bench -p context-graph-core -- comparator</command>
</test_commands>
</task_spec>
```

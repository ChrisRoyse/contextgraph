# TASK-LOGIC-002: Sparse Similarity Functions

```xml
<task_spec id="TASK-LOGIC-002" version="1.0">
<metadata>
  <title>Implement Sparse Vector Similarity Functions</title>
  <status>todo</status>
  <layer>logic</layer>
  <sequence>12</sequence>
  <implements>
    <requirement_ref>REQ-SIMILARITY-SPARSE-01</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORE-003</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_days>1.5</estimated_days>
</metadata>

<context>
SPLADE embeddings (E6, E13) are sparse vectors with ~30K possible dimensions but
typically only 100-1000 active. Requires specialized similarity functions that
operate efficiently on sparse representations.
</context>

<objective>
Implement sparse vector similarity functions including sparse dot product,
sparse cosine similarity, and Jaccard similarity for SPLADE embeddings.
</objective>

<rationale>
Sparse vectors require different algorithms than dense:
1. Storage: Index-value pairs instead of full arrays
2. Computation: Only process non-zero dimensions
3. Efficiency: O(active) instead of O(total_dims)

SPLADE's term expansion produces interpretable sparse vectors that capture
lexical semantics.
</rationale>

<input_context_files>
  <file purpose="sparse_vector">crates/context-graph-core/src/teleology/array.rs</file>
  <file purpose="dense_similarity">crates/context-graph-core/src/teleology/similarity/dense.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORE-003 complete (SparseVector type exists)</check>
</prerequisites>

<scope>
  <in_scope>
    <item>Implement sparse_dot_product function</item>
    <item>Implement sparse_cosine_similarity function</item>
    <item>Implement jaccard_similarity function</item>
    <item>Implement BM25 scoring for inverted index</item>
    <item>Add unit tests</item>
  </in_scope>
  <out_of_scope>
    <item>Dense similarity (TASK-LOGIC-001)</item>
    <item>Token-level similarity (TASK-LOGIC-003)</item>
    <item>Inverted index implementation (TASK-CORE-007)</item>
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/teleology/similarity/sparse.rs">
      use crate::teleology::array::SparseVector;

      /// Calculate dot product between two sparse vectors.
      /// Only considers indices present in both vectors.
      pub fn sparse_dot_product(a: &SparseVector, b: &SparseVector) -> f32;

      /// Calculate cosine similarity between two sparse vectors.
      pub fn sparse_cosine_similarity(a: &SparseVector, b: &SparseVector) -> f32;

      /// Calculate Jaccard similarity based on active dimensions.
      pub fn jaccard_similarity(a: &SparseVector, b: &SparseVector) -> f32;

      /// Calculate sparse L2 norm.
      pub fn sparse_l2_norm(v: &SparseVector) -> f32;

      /// BM25 scoring parameters.
      #[derive(Debug, Clone)]
      pub struct Bm25Config {
          pub k1: f32,  // Term frequency saturation
          pub b: f32,   // Length normalization
      }

      /// Calculate BM25 score for a sparse query against document.
      pub fn bm25_score(
          query: &SparseVector,
          document: &SparseVector,
          avg_doc_len: f32,
          doc_count: usize,
          term_doc_frequencies: &HashMap<u32, usize>,
          config: &Bm25Config,
      ) -> f32;
    </signature>
  </signatures>

  <constraints>
    <constraint>Efficient for vectors with <1000 active dimensions</constraint>
    <constraint>Handles empty vectors gracefully (return 0.0)</constraint>
    <constraint>Indices must be sorted for efficient intersection</constraint>
    <constraint>Jaccard in [0.0, 1.0] range</constraint>
  </constraints>

  <verification>
    <command>cargo test -p context-graph-core similarity::sparse</command>
  </verification>
</definition_of_done>

<pseudo_code>
// crates/context-graph-core/src/teleology/similarity/sparse.rs

use crate::teleology::array::SparseVector;
use std::collections::HashMap;

/// Calculate dot product between two sparse vectors.
/// Uses merge-join on sorted indices for O(n + m) complexity.
pub fn sparse_dot_product(a: &SparseVector, b: &SparseVector) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let mut sum = 0.0f32;
    let mut i = 0;
    let mut j = 0;

    // Assumes indices are sorted
    while i < a.indices.len() && j < b.indices.len() {
        match a.indices[i].cmp(&b.indices[j]) {
            std::cmp::Ordering::Equal => {
                sum += a.values[i] * b.values[j];
                i += 1;
                j += 1;
            }
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
        }
    }

    sum
}

/// Calculate L2 norm of sparse vector.
pub fn sparse_l2_norm(v: &SparseVector) -> f32 {
    v.values.iter().map(|x| x * x).sum::<f32>().sqrt()
}

/// Calculate cosine similarity between two sparse vectors.
pub fn sparse_cosine_similarity(a: &SparseVector, b: &SparseVector) -> f32 {
    let dot = sparse_dot_product(a, b);
    let norm_a = sparse_l2_norm(a);
    let norm_b = sparse_l2_norm(b);

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

/// Calculate Jaccard similarity based on active dimensions overlap.
pub fn jaccard_similarity(a: &SparseVector, b: &SparseVector) -> f32 {
    if a.is_empty() && b.is_empty() {
        return 1.0;  // Both empty = identical
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let set_a: std::collections::HashSet<_> = a.indices.iter().collect();
    let set_b: std::collections::HashSet<_> = b.indices.iter().collect();

    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();

    intersection as f32 / union as f32
}

#[derive(Debug, Clone)]
pub struct Bm25Config {
    pub k1: f32,
    pub b: f32,
}

impl Default for Bm25Config {
    fn default() -> Self {
        Self { k1: 1.2, b: 0.75 }
    }
}

/// Calculate BM25 score for sparse query against document.
pub fn bm25_score(
    query: &SparseVector,
    document: &SparseVector,
    avg_doc_len: f32,
    doc_count: usize,
    term_doc_frequencies: &HashMap<u32, usize>,
    config: &Bm25Config,
) -> f32 {
    let doc_len = document.values.iter().sum::<f32>();

    let mut score = 0.0f32;

    for (i, &idx) in query.indices.iter().enumerate() {
        // Find term in document
        if let Ok(pos) = document.indices.binary_search(&idx) {
            let tf = document.values[pos];
            let df = *term_doc_frequencies.get(&idx).unwrap_or(&1);

            // IDF component
            let idf = ((doc_count as f32 - df as f32 + 0.5) / (df as f32 + 0.5) + 1.0).ln();

            // TF component with saturation
            let tf_component = (tf * (config.k1 + 1.0))
                / (tf + config.k1 * (1.0 - config.b + config.b * doc_len / avg_doc_len));

            score += idf * tf_component * query.values[i];
        }
    }

    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sparse_dot_identical() {
        let v = SparseVector::new(
            vec![0, 5, 10],
            vec![1.0, 2.0, 3.0],
        );
        let dot = sparse_dot_product(&v, &v);
        assert!((dot - 14.0).abs() < 1e-6);  // 1 + 4 + 9
    }

    #[test]
    fn test_sparse_dot_no_overlap() {
        let a = SparseVector::new(vec![0, 1], vec![1.0, 2.0]);
        let b = SparseVector::new(vec![2, 3], vec![3.0, 4.0]);
        let dot = sparse_dot_product(&a, &b);
        assert_eq!(dot, 0.0);
    }

    #[test]
    fn test_jaccard_identical() {
        let v = SparseVector::new(vec![0, 5, 10], vec![1.0, 2.0, 3.0]);
        let sim = jaccard_similarity(&v, &v);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_jaccard_no_overlap() {
        let a = SparseVector::new(vec![0, 1], vec![1.0, 2.0]);
        let b = SparseVector::new(vec![2, 3], vec![3.0, 4.0]);
        let sim = jaccard_similarity(&a, &b);
        assert_eq!(sim, 0.0);
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/teleology/similarity/sparse.rs">
    Sparse vector similarity functions
  </file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/teleology/similarity/mod.rs">
    Add: pub mod sparse;
  </file>
</files_to_modify>

<validation_criteria>
  <criterion>Dot product of identical sparse vector is sum of squared values</criterion>
  <criterion>Non-overlapping vectors have 0 similarity</criterion>
  <criterion>Jaccard of identical vectors is 1.0</criterion>
  <criterion>BM25 scoring works correctly</criterion>
  <criterion>Empty vectors handled gracefully</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-core similarity::sparse -- --nocapture</command>
</test_commands>
</task_spec>
```

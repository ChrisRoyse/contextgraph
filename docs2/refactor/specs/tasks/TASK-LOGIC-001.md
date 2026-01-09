# TASK-LOGIC-001: Dense Similarity Functions

```xml
<task_spec id="TASK-LOGIC-001" version="1.0">
<metadata>
  <title>Implement Dense Vector Similarity Functions</title>
  <status>todo</status>
  <layer>logic</layer>
  <sequence>11</sequence>
  <implements>
    <requirement_ref>REQ-SIMILARITY-DENSE-01</requirement_ref>
    <requirement_ref>REQ-SIMD-01</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORE-002</task_ref>
    <task_ref>TASK-CORE-003</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_days>2</estimated_days>
</metadata>

<context>
First similarity implementation. Dense vectors (E1, E2, E3, E4, E5, E7, E8, E10, E11)
use cosine similarity, dot product, or Euclidean distance. SIMD acceleration
provides 2-4x speedup on x86_64.
</context>

<objective>
Implement dense vector similarity functions with optional SIMD acceleration for
cosine similarity, dot product, and Euclidean distance.
</objective>

<rationale>
Dense embeddings are the most common type (9 of 13 embedders). Efficient
similarity computation is critical for search performance. SIMD provides
significant speedups for high-dimensional vectors without external dependencies.
</rationale>

<input_context_files>
  <file purpose="array_type">crates/context-graph-core/src/teleology/array.rs</file>
  <file purpose="embedder_dims">crates/context-graph-core/src/teleology/embedder.rs</file>
  <file purpose="search_spec">docs2/refactor/03-SEARCH.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORE-002 complete (Embedder enum exists)</check>
  <check>TASK-CORE-003 complete (EmbedderOutput::Dense exists)</check>
</prerequisites>

<scope>
  <in_scope>
    <item>Implement DenseSimilarity struct</item>
    <item>Implement cosine_similarity function</item>
    <item>Implement dot_product function</item>
    <item>Implement euclidean_distance function</item>
    <item>Implement SIMD-accelerated versions for x86_64</item>
    <item>Add dimension validation</item>
    <item>Add unit tests and benchmarks</item>
  </in_scope>
  <out_of_scope>
    <item>Sparse similarity (TASK-LOGIC-002)</item>
    <item>Token-level similarity (TASK-LOGIC-003)</item>
    <item>Comparator integration (TASK-LOGIC-004)</item>
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/teleology/similarity/dense.rs">
      /// Dense vector similarity functions with optional SIMD acceleration.

      /// Calculate cosine similarity between two dense vectors.
      /// Returns value in [-1.0, 1.0] where 1.0 means identical direction.
      pub fn cosine_similarity(a: &[f32], b: &[f32]) -> Result<f32, SimilarityError>;

      /// Calculate dot product between two dense vectors.
      pub fn dot_product(a: &[f32], b: &[f32]) -> Result<f32, SimilarityError>;

      /// Calculate Euclidean distance between two dense vectors.
      pub fn euclidean_distance(a: &[f32], b: &[f32]) -> Result<f32, SimilarityError>;

      /// Calculate L2 norm (magnitude) of a vector.
      pub fn l2_norm(v: &[f32]) -> f32;

      /// Normalize a vector to unit length.
      pub fn normalize(v: &mut [f32]);

      /// SIMD-accelerated cosine similarity (x86_64 only).
      #[cfg(target_arch = "x86_64")]
      pub fn cosine_similarity_simd(a: &[f32], b: &[f32]) -> Result<f32, SimilarityError>;

      #[derive(Debug, thiserror::Error)]
      pub enum SimilarityError {
          #[error("Dimension mismatch: expected {expected}, got {actual}")]
          DimensionMismatch { expected: usize, actual: usize },
          #[error("Empty vector")]
          EmptyVector,
          #[error("Zero magnitude vector")]
          ZeroMagnitude,
      }
    </signature>
  </signatures>

  <constraints>
    <constraint>Dimension validation before computation</constraint>
    <constraint>SIMD version produces same results as scalar</constraint>
    <constraint>No NaN or Inf in output</constraint>
    <constraint>Cosine similarity in [-1.0, 1.0] range</constraint>
    <constraint>Euclidean distance >= 0</constraint>
  </constraints>

  <verification>
    <command>cargo test -p context-graph-core similarity::dense</command>
    <command>cargo bench -p context-graph-core dense_similarity</command>
  </verification>
</definition_of_done>

<pseudo_code>
// crates/context-graph-core/src/teleology/similarity/dense.rs

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SimilarityError {
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },
    #[error("Empty vector")]
    EmptyVector,
    #[error("Zero magnitude vector")]
    ZeroMagnitude,
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> Result<f32, SimilarityError> {
    if a.is_empty() || b.is_empty() {
        return Err(SimilarityError::EmptyVector);
    }
    if a.len() != b.len() {
        return Err(SimilarityError::DimensionMismatch {
            expected: a.len(),
            actual: b.len(),
        });
    }

    let dot = dot_product_unchecked(a, b);
    let norm_a = l2_norm(a);
    let norm_b = l2_norm(b);

    if norm_a == 0.0 || norm_b == 0.0 {
        return Err(SimilarityError::ZeroMagnitude);
    }

    Ok(dot / (norm_a * norm_b))
}

fn dot_product_unchecked(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

pub fn dot_product(a: &[f32], b: &[f32]) -> Result<f32, SimilarityError> {
    if a.len() != b.len() {
        return Err(SimilarityError::DimensionMismatch {
            expected: a.len(),
            actual: b.len(),
        });
    }
    Ok(dot_product_unchecked(a, b))
}

pub fn l2_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

pub fn euclidean_distance(a: &[f32], b: &[f32]) -> Result<f32, SimilarityError> {
    if a.len() != b.len() {
        return Err(SimilarityError::DimensionMismatch {
            expected: a.len(),
            actual: b.len(),
        });
    }
    let sum: f32 = a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum();
    Ok(sum.sqrt())
}

pub fn normalize(v: &mut [f32]) {
    let norm = l2_norm(v);
    if norm > 0.0 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

#[cfg(target_arch = "x86_64")]
pub fn cosine_similarity_simd(a: &[f32], b: &[f32]) -> Result<f32, SimilarityError> {
    use std::arch::x86_64::*;

    if a.len() != b.len() {
        return Err(SimilarityError::DimensionMismatch {
            expected: a.len(),
            actual: b.len(),
        });
    }

    unsafe {
        let mut dot_sum = _mm256_setzero_ps();
        let mut norm_a_sum = _mm256_setzero_ps();
        let mut norm_b_sum = _mm256_setzero_ps();

        let chunks = a.len() / 8;
        for i in 0..chunks {
            let va = _mm256_loadu_ps(a.as_ptr().add(i * 8));
            let vb = _mm256_loadu_ps(b.as_ptr().add(i * 8));

            dot_sum = _mm256_fmadd_ps(va, vb, dot_sum);
            norm_a_sum = _mm256_fmadd_ps(va, va, norm_a_sum);
            norm_b_sum = _mm256_fmadd_ps(vb, vb, norm_b_sum);
        }

        // Horizontal sum and handle remainder
        let dot = hsum_ps_avx(dot_sum);
        let norm_a = hsum_ps_avx(norm_a_sum).sqrt();
        let norm_b = hsum_ps_avx(norm_b_sum).sqrt();

        // Handle remainder with scalar code
        let remainder_start = chunks * 8;
        let mut dot_rem = 0.0f32;
        let mut norm_a_rem = 0.0f32;
        let mut norm_b_rem = 0.0f32;
        for i in remainder_start..a.len() {
            dot_rem += a[i] * b[i];
            norm_a_rem += a[i] * a[i];
            norm_b_rem += b[i] * b[i];
        }

        let total_dot = dot + dot_rem;
        let total_norm_a = (norm_a * norm_a + norm_a_rem).sqrt();
        let total_norm_b = (norm_b * norm_b + norm_b_rem).sqrt();

        if total_norm_a == 0.0 || total_norm_b == 0.0 {
            return Err(SimilarityError::ZeroMagnitude);
        }

        Ok(total_dot / (total_norm_a * total_norm_b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_identical() {
        let v = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&v, &v).unwrap();
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b).unwrap();
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_dimension_mismatch() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        assert!(matches!(
            cosine_similarity(&a, &b),
            Err(SimilarityError::DimensionMismatch { .. })
        ));
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_simd_matches_scalar() {
        let a: Vec<f32> = (0..1024).map(|i| i as f32 * 0.001).collect();
        let b: Vec<f32> = (0..1024).map(|i| (i as f32 * 0.001).sin()).collect();

        let scalar = cosine_similarity(&a, &b).unwrap();
        let simd = cosine_similarity_simd(&a, &b).unwrap();

        assert!((scalar - simd).abs() < 1e-5);
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/teleology/similarity/dense.rs">
    Dense vector similarity functions
  </file>
  <file path="crates/context-graph-core/src/teleology/similarity/mod.rs">
    Similarity module definition
  </file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/teleology/mod.rs">
    Add: pub mod similarity;
  </file>
</files_to_modify>

<validation_criteria>
  <criterion>Cosine of identical vectors is 1.0</criterion>
  <criterion>Cosine of orthogonal vectors is 0.0</criterion>
  <criterion>SIMD and scalar produce same results</criterion>
  <criterion>Dimension mismatch returns error</criterion>
  <criterion>Benchmark shows SIMD 2-4x faster</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-core similarity::dense -- --nocapture</command>
  <command>cargo bench -p context-graph-core -- dense</command>
</test_commands>
</task_spec>
```

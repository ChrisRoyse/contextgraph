# TASK-LOGIC-003: Token-Level Similarity

```xml
<task_spec id="TASK-LOGIC-003" version="1.0">
<metadata>
  <title>Implement Token-Level Similarity Functions (ColBERT MaxSim)</title>
  <status>todo</status>
  <layer>logic</layer>
  <sequence>13</sequence>
  <implements>
    <requirement_ref>REQ-SIMILARITY-TOKENLEVEL-01</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORE-003</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_days>1.5</estimated_days>
</metadata>

<context>
Late interaction embeddings (E12) use ColBERT-style token-level representations.
Each token has its own 128D embedding, and similarity is computed via MaxSim:
for each query token, find the maximum similarity to any document token.
</context>

<objective>
Implement MaxSim and symmetric MaxSim functions for token-level embeddings,
enabling precision matching for the late interaction embedder.
</objective>

<rationale>
MaxSim provides:
1. Token-level precision: Exact term matching preserved
2. Contextual matching: Embeddings capture token context
3. Efficiency: Can be approximated with inverted indices
4. Interpretability: Know which tokens matched

ColBERT's late interaction is crucial for precision-critical retrieval.
</rationale>

<input_context_files>
  <file purpose="token_embeddings">crates/context-graph-core/src/teleology/array.rs</file>
  <file purpose="dense_similarity">crates/context-graph-core/src/teleology/similarity/dense.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORE-003 complete (TokenEmbeddings type exists)</check>
  <check>TASK-LOGIC-001 complete (dense cosine similarity exists)</check>
</prerequisites>

<scope>
  <in_scope>
    <item>Implement max_sim function</item>
    <item>Implement symmetric_max_sim function</item>
    <item>Implement approximate_max_sim with early termination</item>
    <item>Add token alignment analysis</item>
    <item>Add unit tests</item>
  </in_scope>
  <out_of_scope>
    <item>Dense similarity (TASK-LOGIC-001)</item>
    <item>Sparse similarity (TASK-LOGIC-002)</item>
    <item>Index for approximate search</item>
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/teleology/similarity/token_level.rs">
      use crate::teleology::array::TokenEmbeddings;

      /// Calculate MaxSim score: sum of max similarities per query token.
      /// For each query token, finds the maximum cosine similarity to any doc token.
      pub fn max_sim(query: &TokenEmbeddings, document: &TokenEmbeddings) -> f32;

      /// Calculate symmetric MaxSim: average of both directions.
      pub fn symmetric_max_sim(a: &TokenEmbeddings, b: &TokenEmbeddings) -> f32;

      /// Calculate MaxSim with early termination for approximate matching.
      pub fn approximate_max_sim(
          query: &TokenEmbeddings,
          document: &TokenEmbeddings,
          min_score_threshold: f32,
      ) -> f32;

      /// Result of token alignment analysis.
      #[derive(Debug, Clone)]
      pub struct TokenAlignment {
          pub query_token_idx: usize,
          pub doc_token_idx: usize,
          pub similarity: f32,
      }

      /// Get detailed token alignments for interpretability.
      pub fn token_alignments(
          query: &TokenEmbeddings,
          document: &TokenEmbeddings,
      ) -> Vec<TokenAlignment>;
    </signature>
  </signatures>

  <constraints>
    <constraint>MaxSim score normalized to [0, 1] range</constraint>
    <constraint>Handles empty token sequences (return 0.0)</constraint>
    <constraint>Token embeddings must have same dimensionality</constraint>
    <constraint>Approximate version within 5% of exact</constraint>
  </constraints>

  <verification>
    <command>cargo test -p context-graph-core similarity::token_level</command>
  </verification>
</definition_of_done>

<pseudo_code>
// crates/context-graph-core/src/teleology/similarity/token_level.rs

use crate::teleology::array::TokenEmbeddings;
use crate::teleology::similarity::dense::cosine_similarity;

/// Calculate MaxSim score between query and document token embeddings.
/// Score = (1/|Q|) * sum over q in Q of max over d in D of cos(q, d)
pub fn max_sim(query: &TokenEmbeddings, document: &TokenEmbeddings) -> f32 {
    if query.embeddings.is_empty() || document.embeddings.is_empty() {
        return 0.0;
    }

    let mut total_max_sim = 0.0f32;

    for q_token in &query.embeddings {
        let mut max_sim_for_token = f32::NEG_INFINITY;

        for d_token in &document.embeddings {
            if let Ok(sim) = cosine_similarity(q_token, d_token) {
                if sim > max_sim_for_token {
                    max_sim_for_token = sim;
                }
            }
        }

        if max_sim_for_token > f32::NEG_INFINITY {
            total_max_sim += max_sim_for_token;
        }
    }

    // Normalize by query length
    total_max_sim / query.embeddings.len() as f32
}

/// Symmetric MaxSim: average of MaxSim(A, B) and MaxSim(B, A).
/// Useful when neither is clearly the "query".
pub fn symmetric_max_sim(a: &TokenEmbeddings, b: &TokenEmbeddings) -> f32 {
    let ab = max_sim(a, b);
    let ba = max_sim(b, a);
    (ab + ba) / 2.0
}

/// Approximate MaxSim with early termination.
/// Stops processing a query token once threshold is exceeded.
pub fn approximate_max_sim(
    query: &TokenEmbeddings,
    document: &TokenEmbeddings,
    min_score_threshold: f32,
) -> f32 {
    if query.embeddings.is_empty() || document.embeddings.is_empty() {
        return 0.0;
    }

    let mut total = 0.0f32;

    for q_token in &query.embeddings {
        let mut max_sim = f32::NEG_INFINITY;

        for d_token in &document.embeddings {
            if let Ok(sim) = cosine_similarity(q_token, d_token) {
                if sim > max_sim {
                    max_sim = sim;
                    // Early termination if we've exceeded threshold
                    if max_sim >= min_score_threshold {
                        break;
                    }
                }
            }
        }

        if max_sim > f32::NEG_INFINITY {
            total += max_sim;
        }
    }

    total / query.embeddings.len() as f32
}

#[derive(Debug, Clone)]
pub struct TokenAlignment {
    pub query_token_idx: usize,
    pub doc_token_idx: usize,
    pub similarity: f32,
}

/// Get detailed token alignments for interpretability.
pub fn token_alignments(
    query: &TokenEmbeddings,
    document: &TokenEmbeddings,
) -> Vec<TokenAlignment> {
    let mut alignments = Vec::with_capacity(query.embeddings.len());

    for (q_idx, q_token) in query.embeddings.iter().enumerate() {
        let mut best_d_idx = 0;
        let mut best_sim = f32::NEG_INFINITY;

        for (d_idx, d_token) in document.embeddings.iter().enumerate() {
            if let Ok(sim) = cosine_similarity(q_token, d_token) {
                if sim > best_sim {
                    best_sim = sim;
                    best_d_idx = d_idx;
                }
            }
        }

        alignments.push(TokenAlignment {
            query_token_idx: q_idx,
            doc_token_idx: best_d_idx,
            similarity: best_sim,
        });
    }

    alignments
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_token_embeddings(tokens: Vec<Vec<f32>>) -> TokenEmbeddings {
        let dims = tokens.first().map(|t| t.len()).unwrap_or(0);
        TokenEmbeddings {
            embeddings: tokens,
            token_count: 0,  // Will be set
            dims_per_token: dims,
        }
    }

    #[test]
    fn test_maxsim_identical() {
        let tokens = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
        ];
        let te = create_token_embeddings(tokens);
        let sim = max_sim(&te, &te);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_maxsim_orthogonal() {
        let q = create_token_embeddings(vec![vec![1.0, 0.0]]);
        let d = create_token_embeddings(vec![vec![0.0, 1.0]]);
        let sim = max_sim(&q, &d);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_symmetric() {
        let a = create_token_embeddings(vec![
            vec![1.0, 0.0],
            vec![0.5, 0.5],
        ]);
        let b = create_token_embeddings(vec![
            vec![0.0, 1.0],
            vec![0.7, 0.3],
        ]);

        let sym = symmetric_max_sim(&a, &b);
        let ab = max_sim(&a, &b);
        let ba = max_sim(&b, &a);

        assert!((sym - (ab + ba) / 2.0).abs() < 1e-6);
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/teleology/similarity/token_level.rs">
    Token-level (ColBERT) similarity functions
  </file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/teleology/similarity/mod.rs">
    Add: pub mod token_level;
  </file>
</files_to_modify>

<validation_criteria>
  <criterion>MaxSim of identical sequences is 1.0</criterion>
  <criterion>Symmetric MaxSim is average of both directions</criterion>
  <criterion>Token alignments correctly identify best matches</criterion>
  <criterion>Approximate version within 5% of exact</criterion>
  <criterion>Empty sequences return 0.0</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-core similarity::token_level -- --nocapture</command>
</test_commands>
</task_spec>
```

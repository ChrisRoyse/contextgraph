# TASK-LOGIC-007: Matrix Strategy Search

```xml
<task_spec id="TASK-LOGIC-007" version="1.0">
<metadata>
  <title>Implement Matrix Strategy Search</title>
  <status>todo</status>
  <layer>logic</layer>
  <sequence>17</sequence>
  <implements>
    <requirement_ref>REQ-SEARCH-MATRIX-01</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-LOGIC-004</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_days>2</estimated_days>
</metadata>

<context>
Matrix strategy search applies a full 13x13 weight matrix for search. This enables
cross-embedder correlation analysis where off-diagonal weights capture relationships
between different embedding spaces.
</context>

<objective>
Implement MatrixStrategySearch that applies 13x13 matrices, supports predefined
matrices (semantic_focused, code_heavy, etc.), and enables correlation analysis.
</objective>

<rationale>
Matrix strategies enable:
1. Cross-embedder correlations (off-diagonal)
2. Predefined domain-specific search patterns
3. Custom matrices for specialized use cases
4. Richer search semantics than simple weighting
</rationale>

<input_context_files>
  <file purpose="comparison_types">crates/context-graph-core/src/teleology/comparison.rs</file>
  <file purpose="weighted_search">crates/context-graph-storage/src/teleological/search/weighted.rs</file>
  <file purpose="matrix_definitions">docs2/refactor/08-MCP-TOOLS.md#predefined-search-matrices</file>
</input_context_files>

<prerequisites>
  <check>TASK-LOGIC-004 complete (SearchMatrix type exists)</check>
  <check>TASK-LOGIC-006 complete (WeightedFullSearch exists)</check>
</prerequisites>

<scope>
  <in_scope>
    <item>Create MatrixStrategySearch struct</item>
    <item>Apply 13x13 matrix weights</item>
    <item>Support cross-embedder correlation analysis</item>
    <item>Optimized execution planning based on matrix structure</item>
    <item>Cache hot query patterns</item>
  </in_scope>
  <out_of_scope>
    <item>5-stage pipeline (TASK-LOGIC-008)</item>
    <item>Goal discovery (TASK-LOGIC-009)</item>
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-storage/src/teleological/search/matrix.rs">
      use crate::teleological::store::SearchResult;
      use crate::teleological::index::EmbedderIndex;
      use context_graph_core::teleology::array::TeleologicalArray;
      use context_graph_core::teleology::comparison::SearchMatrix;
      use context_graph_core::teleology::embedder::Embedder;

      /// Matrix strategy search with cross-embedder correlations.
      pub struct MatrixStrategySearch {
          indices: Vec<Box<dyn EmbedderIndex + Send + Sync>>,
          query_cache: QueryCache,
      }

      impl MatrixStrategySearch {
          pub fn new(indices: Vec<Box<dyn EmbedderIndex + Send + Sync>>) -> Self;

          /// Search using a predefined or custom matrix.
          pub async fn search(
              &self,
              query: &TeleologicalArray,
              matrix: &SearchMatrix,
              limit: usize,
              threshold: Option<f32>,
          ) -> Result<Vec<SearchResult>, SearchError>;

          /// Search with correlation analysis enabled.
          pub async fn search_with_correlation(
              &self,
              query: &TeleologicalArray,
              matrix: &SearchMatrix,
              limit: usize,
          ) -> Result<SearchWithCorrelation, SearchError>;

          /// Analyze matrix structure for execution optimization.
          fn analyze_matrix(&self, matrix: &SearchMatrix) -> MatrixAnalysis;
      }

      /// Search results with correlation analysis.
      #[derive(Debug)]
      pub struct SearchWithCorrelation {
          pub results: Vec<SearchResult>,
          pub correlation: CorrelationAnalysis,
      }

      /// Analysis of embedder correlations in results.
      #[derive(Debug)]
      pub struct CorrelationAnalysis {
          /// 13x13 correlation matrix between embedder scores
          pub correlation_matrix: [[f32; 13]; 13],
          /// Detected patterns
          pub patterns: Vec<CorrelationPattern>,
          /// Overall coherence score
          pub coherence: f32,
      }

      #[derive(Debug)]
      pub enum CorrelationPattern {
          ConsensusHigh { embedders: Vec<Embedder>, strength: f32 },
          TemporalSemanticAlign { strength: f32 },
          CodeSemanticDivergence { strength: f32 },
          OutlierEmbedder { embedder: Embedder, deviation: f32 },
      }

      /// Matrix structure analysis for optimization.
      #[derive(Debug)]
      struct MatrixAnalysis {
          is_diagonal: bool,
          non_zero_embedders: Vec<Embedder>,
          has_cross_correlations: bool,
          sparsity: f32,
      }
    </signature>
  </signatures>

  <constraints>
    <constraint>Predefined matrices load correctly</constraint>
    <constraint>Cross-correlation computed when off-diagonal non-zero</constraint>
    <constraint>Sparse matrices optimize to skip zero embedders</constraint>
    <constraint>Query cache reduces repeated work</constraint>
  </constraints>

  <verification>
    <command>cargo test -p context-graph-storage search::matrix</command>
  </verification>
</definition_of_done>

<pseudo_code>
// crates/context-graph-storage/src/teleological/search/matrix.rs

use std::collections::HashMap;
use std::sync::RwLock;

use crate::teleological::store::SearchResult;
use crate::teleological::index::EmbedderIndex;
use crate::teleological::search::single::SearchError;
use context_graph_core::teleology::array::TeleologicalArray;
use context_graph_core::teleology::comparison::SearchMatrix;
use context_graph_core::teleology::embedder::Embedder;

pub struct MatrixStrategySearch {
    indices: Vec<Box<dyn EmbedderIndex + Send + Sync>>,
    query_cache: RwLock<HashMap<u64, Vec<SearchResult>>>,
}

impl MatrixStrategySearch {
    pub fn new(indices: Vec<Box<dyn EmbedderIndex + Send + Sync>>) -> Self {
        Self {
            indices,
            query_cache: RwLock::new(HashMap::new()),
        }
    }

    pub async fn search(
        &self,
        query: &TeleologicalArray,
        matrix: &SearchMatrix,
        limit: usize,
        threshold: Option<f32>,
    ) -> Result<Vec<SearchResult>, SearchError> {
        // Analyze matrix structure
        let analysis = self.analyze_matrix(matrix);

        // If diagonal, delegate to weighted search
        if analysis.is_diagonal && !analysis.has_cross_correlations {
            return self.search_diagonal(query, matrix, limit, threshold).await;
        }

        // Full matrix search with cross-correlations
        self.search_full_matrix(query, matrix, &analysis, limit, threshold).await
    }

    fn analyze_matrix(&self, matrix: &SearchMatrix) -> MatrixAnalysis {
        let mut non_zero_embedders = Vec::new();
        let mut has_cross_correlations = false;
        let mut non_zero_count = 0;

        for i in 0..13 {
            let ei = Embedder::from_index(i).unwrap();
            for j in 0..13 {
                let ej = Embedder::from_index(j).unwrap();
                let weight = matrix.get(ei, ej);

                if weight > 0.0 {
                    non_zero_count += 1;
                    if i == j {
                        non_zero_embedders.push(ei);
                    } else {
                        has_cross_correlations = true;
                    }
                }
            }
        }

        MatrixAnalysis {
            is_diagonal: !has_cross_correlations,
            non_zero_embedders,
            has_cross_correlations,
            sparsity: 1.0 - (non_zero_count as f32 / 169.0),
        }
    }

    async fn search_full_matrix(
        &self,
        query: &TeleologicalArray,
        matrix: &SearchMatrix,
        analysis: &MatrixAnalysis,
        limit: usize,
        threshold: Option<f32>,
    ) -> Result<Vec<SearchResult>, SearchError> {
        // Query all non-zero embedders
        let mut per_embedder_results: Vec<Vec<SearchResult>> = Vec::new();

        for embedder in &analysis.non_zero_embedders {
            let embedding = query.get(*embedder);
            let results = self.indices[embedder.index()]
                .search(embedding, limit * 3, threshold)
                .await
                .map_err(|e| SearchError::Index(e.to_string()))?;

            per_embedder_results.push(
                results.into_iter()
                    .map(|ir| SearchResult {
                        array: TeleologicalArray::new(ir.id),
                        similarity: ir.score,
                        per_embedder_scores: [None; 13],
                    })
                    .collect()
            );
        }

        // Apply matrix weights including cross-correlations
        let fused = self.apply_matrix_weights(per_embedder_results, matrix, analysis);

        Ok(fused.into_iter().take(limit).collect())
    }

    fn apply_matrix_weights(
        &self,
        per_embedder_results: Vec<Vec<SearchResult>>,
        matrix: &SearchMatrix,
        analysis: &MatrixAnalysis,
    ) -> Vec<SearchResult> {
        let mut score_map: HashMap<uuid::Uuid, (f32, [Option<f32>; 13])> = HashMap::new();

        // Collect per-embedder scores
        for (e_idx, results) in per_embedder_results.iter().enumerate() {
            let embedder = analysis.non_zero_embedders[e_idx];
            for result in results {
                let entry = score_map.entry(result.array.id)
                    .or_insert((0.0, [None; 13]));
                entry.1[embedder.index()] = Some(result.similarity);
            }
        }

        // Apply matrix weights
        let mut final_results: Vec<SearchResult> = score_map.into_iter()
            .map(|(id, (_, per_embedder))| {
                let mut total_score = 0.0f32;
                let mut total_weight = 0.0f32;

                for i in 0..13 {
                    for j in 0..13 {
                        let ei = Embedder::from_index(i).unwrap();
                        let ej = Embedder::from_index(j).unwrap();
                        let weight = matrix.get(ei, ej);

                        if weight > 0.0 {
                            if let (Some(si), Some(sj)) = (per_embedder[i], per_embedder[j]) {
                                if i == j {
                                    total_score += si * weight;
                                } else {
                                    // Cross-correlation: geometric mean
                                    total_score += (si * sj).sqrt() * weight;
                                }
                                total_weight += weight;
                            }
                        }
                    }
                }

                let final_score = if total_weight > 0.0 {
                    total_score / total_weight
                } else {
                    0.0
                };

                SearchResult {
                    array: TeleologicalArray::new(id),
                    similarity: final_score,
                    per_embedder_scores: per_embedder,
                }
            })
            .collect();

        final_results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());
        final_results
    }

    pub async fn search_with_correlation(
        &self,
        query: &TeleologicalArray,
        matrix: &SearchMatrix,
        limit: usize,
    ) -> Result<SearchWithCorrelation, SearchError> {
        let results = self.search(query, matrix, limit, None).await?;
        let correlation = self.compute_correlation_analysis(&results);

        Ok(SearchWithCorrelation { results, correlation })
    }

    fn compute_correlation_analysis(&self, results: &[SearchResult]) -> CorrelationAnalysis {
        // Compute correlation matrix from per-embedder scores
        let mut correlation_matrix = [[0.0f32; 13]; 13];
        let mut patterns = Vec::new();

        // Simple correlation computation
        for i in 0..13 {
            for j in 0..13 {
                let scores_i: Vec<f32> = results.iter()
                    .filter_map(|r| r.per_embedder_scores[i])
                    .collect();
                let scores_j: Vec<f32> = results.iter()
                    .filter_map(|r| r.per_embedder_scores[j])
                    .collect();

                if scores_i.len() > 1 && scores_j.len() > 1 {
                    correlation_matrix[i][j] = pearson_correlation(&scores_i, &scores_j);
                }
            }
        }

        // Detect patterns
        // ... pattern detection logic

        let coherence = results.iter()
            .filter_map(|r| {
                let valid: Vec<_> = r.per_embedder_scores.iter()
                    .filter_map(|&s| s)
                    .collect();
                if valid.len() > 1 {
                    let mean = valid.iter().sum::<f32>() / valid.len() as f32;
                    let variance = valid.iter()
                        .map(|&s| (s - mean).powi(2))
                        .sum::<f32>() / valid.len() as f32;
                    Some(1.0 / (1.0 + variance.sqrt()))
                } else {
                    None
                }
            })
            .sum::<f32>() / results.len() as f32;

        CorrelationAnalysis {
            correlation_matrix,
            patterns,
            coherence,
        }
    }
}

fn pearson_correlation(x: &[f32], y: &[f32]) -> f32 {
    if x.len() != y.len() || x.is_empty() {
        return 0.0;
    }

    let n = x.len() as f32;
    let sum_x: f32 = x.iter().sum();
    let sum_y: f32 = y.iter().sum();
    let sum_xy: f32 = x.iter().zip(y).map(|(a, b)| a * b).sum();
    let sum_x2: f32 = x.iter().map(|a| a * a).sum();
    let sum_y2: f32 = y.iter().map(|a| a * a).sum();

    let numerator = n * sum_xy - sum_x * sum_y;
    let denominator = ((n * sum_x2 - sum_x * sum_x) * (n * sum_y2 - sum_y * sum_y)).sqrt();

    if denominator == 0.0 { 0.0 } else { numerator / denominator }
}

#[derive(Debug)]
struct MatrixAnalysis {
    is_diagonal: bool,
    non_zero_embedders: Vec<Embedder>,
    has_cross_correlations: bool,
    sparsity: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagonal_matrix_detection() {
        let matrix = SearchMatrix::identity();
        // Create mock search and test analysis
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-storage/src/teleological/search/matrix.rs">
    Matrix strategy search implementation
  </file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-storage/src/teleological/search/mod.rs">
    Add: pub mod matrix;
  </file>
</files_to_modify>

<validation_criteria>
  <criterion>Diagonal matrices optimized to weighted search</criterion>
  <criterion>Cross-correlations computed when present</criterion>
  <criterion>Predefined matrices work correctly</criterion>
  <criterion>Correlation analysis detects patterns</criterion>
  <criterion>Sparse matrices skip zero embedders</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-storage search::matrix -- --nocapture</command>
</test_commands>
</task_spec>
```

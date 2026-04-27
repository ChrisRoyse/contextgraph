//! Main pipeline execution logic.
//!
//! This module contains the `RetrievalPipeline` struct and the core
//! execution logic for the 5-stage retrieval pipeline (including graph expansion).

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use tracing::debug;

use super::super::super::indexes::{EmbedderIndex, EmbedderIndexRegistry};
use super::super::error::SearchError;
use super::super::single::SingleEmbedderSearch;
use super::stages::StageExecutor;
use super::traits::{InMemorySpladeIndex, InMemoryTokenStorage, SpladeIndex, TokenStorage};
use super::types::{
    PipelineCandidate, PipelineConfig, PipelineError, PipelineResult, PipelineStage, StageResult,
};
use crate::graph_edges::EdgeRepository;

// ============================================================================
// RETRIEVAL PIPELINE
// ============================================================================

/// The 5-stage retrieval pipeline (including graph expansion).
pub struct RetrievalPipeline {
    /// Single embedder search (for Stage 2).
    single_search: SingleEmbedderSearch,
    /// SPLADE inverted index (for Stage 1).
    splade_index: Arc<dyn SpladeIndex>,
    /// Token storage (for Stage 4 MaxSim).
    token_storage: Arc<dyn TokenStorage>,
    /// Edge repository for graph expansion (Stage 3.5).
    edge_repository: Option<Arc<EdgeRepository>>,
    /// Pipeline configuration.
    pub(crate) config: PipelineConfig,
}

impl RetrievalPipeline {
    /// Create a new pipeline with registry.
    ///
    /// # Arguments
    /// * `registry` - Embedder index registry
    /// * `splade_index` - Optional SPLADE index (creates empty in-memory if None)
    /// * `token_storage` - Optional token storage (creates empty in-memory if None)
    pub fn new(
        registry: Arc<EmbedderIndexRegistry>,
        splade_index: Option<Arc<dyn SpladeIndex>>,
        token_storage: Option<Arc<dyn TokenStorage>>,
    ) -> Self {
        Self {
            single_search: SingleEmbedderSearch::new(registry),
            splade_index: splade_index.unwrap_or_else(|| Arc::new(InMemorySpladeIndex::new())),
            token_storage: token_storage.unwrap_or_else(|| Arc::new(InMemoryTokenStorage::new())),
            edge_repository: None,
            config: PipelineConfig::default(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(
        registry: Arc<EmbedderIndexRegistry>,
        config: PipelineConfig,
        splade_index: Option<Arc<dyn SpladeIndex>>,
        token_storage: Option<Arc<dyn TokenStorage>>,
    ) -> Self {
        Self {
            single_search: SingleEmbedderSearch::new(registry),
            splade_index: splade_index.unwrap_or_else(|| Arc::new(InMemorySpladeIndex::new())),
            token_storage: token_storage.unwrap_or_else(|| Arc::new(InMemoryTokenStorage::new())),
            edge_repository: None,
            config,
        }
    }

    /// Get the edge repository.
    pub fn edge_repository(&self) -> Option<&Arc<EdgeRepository>> {
        self.edge_repository.as_ref()
    }

    /// Get the current configuration.
    #[inline]
    pub fn config(&self) -> &PipelineConfig {
        &self.config
    }

    /// Execute full 5-stage pipeline (including graph expansion).
    ///
    /// # Arguments
    /// * `query_splade` - Sparse vector for Stage 1 as (term_id, weight) pairs
    /// * `query_matryoshka` - 128D vector for Stage 2
    /// * `query_semantic` - 1024D vector for Stage 3 RRF
    /// * `query_tokens` - Token embeddings for Stage 4 MaxSim (each 128D)
    ///
    /// # FAIL FAST Errors
    /// - `SearchError::InvalidVector` if query embeddings are invalid
    /// - `SearchError::DimensionMismatch` if query dimensions wrong
    /// - `PipelineError::Timeout` if any stage exceeds max_latency_ms
    pub fn execute(
        &self,
        query_splade: &[(usize, f32)],
        query_matryoshka: &[f32],
        query_semantic: &[f32],
        query_tokens: &[Vec<f32>],
    ) -> Result<PipelineResult, PipelineError> {
        self.execute_stages(
            query_splade,
            query_matryoshka,
            query_semantic,
            query_tokens,
            &PipelineStage::all(),
        )
    }

    /// Execute with stage selection.
    pub fn execute_stages(
        &self,
        query_splade: &[(usize, f32)],
        query_matryoshka: &[f32],
        query_semantic: &[f32],
        query_tokens: &[Vec<f32>],
        stages: &[PipelineStage],
    ) -> Result<PipelineResult, PipelineError> {
        let pipeline_start = Instant::now();
        let mut stage_results = Vec::with_capacity(5);
        let mut stages_executed = Vec::with_capacity(5);
        let mut candidates: Vec<PipelineCandidate> = Vec::new();

        // Validate queries upfront - FAIL FAST
        self.validate_queries(query_matryoshka, query_semantic, query_tokens, stages)?;

        // Create stage set for O(1) lookup
        let stage_set: HashSet<_> = stages.iter().copied().collect();

        // Create stage executor
        let executor = StageExecutor {
            single_search: &self.single_search,
            splade_index: &self.splade_index,
            token_storage: &self.token_storage,
            config: &self.config,
        };

        // Stage 1: SPLADE Filter
        if stage_set.contains(&PipelineStage::SpladeFilter) && self.config.stages[0].enabled {
            let result = executor.stage_splade_filter(query_splade, &self.config.stages[0])?;
            // Extract Copy fields before moving Vec
            let latency_us = result.latency_us;
            let candidates_in = result.candidates_in;
            let candidates_out = result.candidates_out;
            let stage = result.stage;
            candidates = result.candidates;
            let stage_result = StageResult {
                candidates: Vec::new(), // Don't store candidates in stage result
                latency_us,
                candidates_in,
                candidates_out,
                stage,
            };
            stage_results.push(stage_result);
            stages_executed.push(PipelineStage::SpladeFilter);
        }

        // Stage 2: Matryoshka ANN
        if stage_set.contains(&PipelineStage::MatryoshkaAnn) && self.config.stages[1].enabled {
            let result = executor.stage_matryoshka_ann(
                query_matryoshka,
                candidates,
                &self.config.stages[1],
            )?;
            // Extract Copy fields before moving Vec
            let latency_us = result.latency_us;
            let candidates_in = result.candidates_in;
            let candidates_out = result.candidates_out;
            let stage = result.stage;
            candidates = result.candidates;
            let stage_result = StageResult {
                candidates: Vec::new(),
                latency_us,
                candidates_in,
                candidates_out,
                stage,
            };
            stage_results.push(stage_result);
            stages_executed.push(PipelineStage::MatryoshkaAnn);
        }

        // Stage 3: RRF Rerank
        if stage_set.contains(&PipelineStage::RrfRerank) && self.config.stages[2].enabled {
            let result =
                executor.stage_rrf_rerank(query_semantic, candidates, &self.config.stages[2])?;
            // Extract Copy fields before moving Vec
            let latency_us = result.latency_us;
            let candidates_in = result.candidates_in;
            let candidates_out = result.candidates_out;
            let stage = result.stage;
            candidates = result.candidates;
            let stage_result = StageResult {
                candidates: Vec::new(),
                latency_us,
                candidates_in,
                candidates_out,
                stage,
            };
            stage_results.push(stage_result);
            stages_executed.push(PipelineStage::RrfRerank);
        }

        // Stage 3.5: Graph Expansion (optional)
        if stage_set.contains(&PipelineStage::GraphExpansion) && self.config.graph_expansion.enabled
        {
            if let Some(ref edge_repo) = self.edge_repository {
                let result = executor.stage_graph_expansion(
                    candidates,
                    edge_repo.as_ref(),
                    &self.config.graph_expansion,
                )?;
                // Extract Copy fields before moving Vec
                let latency_us = result.latency_us;
                let candidates_in = result.candidates_in;
                let candidates_out = result.candidates_out;
                let stage = result.stage;
                candidates = result.candidates;
                let stage_result = StageResult {
                    candidates: Vec::new(),
                    latency_us,
                    candidates_in,
                    candidates_out,
                    stage,
                };
                stage_results.push(stage_result);
                stages_executed.push(PipelineStage::GraphExpansion);

                debug!(
                    candidates_in,
                    candidates_out, latency_us, "Graph expansion stage completed"
                );
            } else {
                debug!("Graph expansion enabled but no edge_repository configured, skipping");
            }
        }

        // Stage 4: MaxSim Rerank
        if stage_set.contains(&PipelineStage::MaxSimRerank) && self.config.stages[3].enabled {
            let result =
                executor.stage_maxsim_rerank(query_tokens, candidates, &self.config.stages[3])?;
            // Extract Copy fields before moving Vec
            let latency_us = result.latency_us;
            let candidates_in = result.candidates_in;
            let candidates_out = result.candidates_out;
            let stage = result.stage;
            candidates = result.candidates;
            let stage_result = StageResult {
                candidates: Vec::new(),
                latency_us,
                candidates_in,
                candidates_out,
                stage,
            };
            stage_results.push(stage_result);
            stages_executed.push(PipelineStage::MaxSimRerank);
        }

        // Final truncation to k
        candidates.truncate(self.config.k);

        let total_latency_us = pipeline_start.elapsed().as_micros() as u64;

        Ok(PipelineResult {
            results: candidates,
            stage_results,
            total_latency_us,
            stages_executed,
        })
    }

    /// Validate query vectors upfront - FAIL FAST.
    fn validate_queries(
        &self,
        query_matryoshka: &[f32],
        query_semantic: &[f32],
        query_tokens: &[Vec<f32>],
        stages: &[PipelineStage],
    ) -> Result<(), PipelineError> {
        let stage_set: HashSet<_> = stages.iter().copied().collect();

        // Validate Matryoshka dimension (Stage 2)
        if stage_set.contains(&PipelineStage::MatryoshkaAnn) && self.config.stages[1].enabled {
            if query_matryoshka.len() != 128 {
                return Err(SearchError::DimensionMismatch {
                    embedder: EmbedderIndex::E1Matryoshka128,
                    expected: 128,
                    actual: query_matryoshka.len(),
                }
                .into());
            }
            self.validate_vector(query_matryoshka, EmbedderIndex::E1Matryoshka128)?;
        }

        // Validate semantic dimension (Stage 3)
        if stage_set.contains(&PipelineStage::RrfRerank) && self.config.stages[2].enabled {
            if query_semantic.len() != 1024 {
                return Err(SearchError::DimensionMismatch {
                    embedder: EmbedderIndex::E1Semantic,
                    expected: 1024,
                    actual: query_semantic.len(),
                }
                .into());
            }
            self.validate_vector(query_semantic, EmbedderIndex::E1Semantic)?;
        }

        // Validate token dimensions (Stage 4)
        if stage_set.contains(&PipelineStage::MaxSimRerank) && self.config.stages[3].enabled {
            for (i, token) in query_tokens.iter().enumerate() {
                if token.len() != 128 {
                    return Err(SearchError::InvalidVector {
                        embedder: EmbedderIndex::E12LateInteraction,
                        message: format!("Token {} has dimension {}, expected 128", i, token.len()),
                    }
                    .into());
                }
                self.validate_vector(token, EmbedderIndex::E12LateInteraction)?;
            }
        }

        Ok(())
    }

    /// Validate a single vector for NaN/Inf - FAIL FAST.
    fn validate_vector(
        &self,
        vector: &[f32],
        embedder: EmbedderIndex,
    ) -> Result<(), PipelineError> {
        for (i, &v) in vector.iter().enumerate() {
            if v.is_nan() {
                return Err(SearchError::InvalidVector {
                    embedder,
                    message: format!("NaN at index {}", i),
                }
                .into());
            }
            if v.is_infinite() {
                return Err(SearchError::InvalidVector {
                    embedder,
                    message: format!("Inf at index {}", i),
                }
                .into());
            }
        }
        Ok(())
    }
}

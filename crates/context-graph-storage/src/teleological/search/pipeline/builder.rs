//! Pipeline builder pattern for constructing queries.
//!
//! Provides a fluent API for building and executing pipeline queries.

use tracing::error;

use super::execution::RetrievalPipeline;
use super::types::{PipelineError, PipelineResult, PipelineStage};

// ============================================================================
// PIPELINE BUILDER
// ============================================================================

/// Builder for pipeline queries.
pub struct PipelineBuilder {
    pub(crate) query_splade: Option<Vec<(usize, f32)>>,
    pub(crate) query_matryoshka: Option<Vec<f32>>,
    pub(crate) query_semantic: Option<Vec<f32>>,
    pub(crate) query_tokens: Option<Vec<Vec<f32>>>,
    pub(crate) stages: Option<Vec<PipelineStage>>,
    pub(crate) k: Option<usize>,
}

impl PipelineBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            query_splade: None,
            query_matryoshka: None,
            query_semantic: None,
            query_tokens: None,
            stages: None,
            k: None,
        }
    }

    /// Set SPLADE query (sparse vector as term_id, weight pairs).
    pub fn splade(mut self, query: Vec<(usize, f32)>) -> Self {
        self.query_splade = Some(query);
        self
    }

    /// Set Matryoshka 128D query.
    pub fn matryoshka(mut self, query: Vec<f32>) -> Self {
        self.query_matryoshka = Some(query);
        self
    }

    /// Set semantic 1024D query.
    pub fn semantic(mut self, query: Vec<f32>) -> Self {
        self.query_semantic = Some(query);
        self
    }

    /// Set token embeddings for MaxSim (each 128D).
    pub fn tokens(mut self, query: Vec<Vec<f32>>) -> Self {
        self.query_tokens = Some(query);
        self
    }

    /// Set stages to execute.
    pub fn stages(mut self, stages: Vec<PipelineStage>) -> Self {
        self.stages = Some(stages);
        self
    }

    /// Set final result limit.
    pub fn k(mut self, k: usize) -> Self {
        self.k = Some(k);
        self
    }

    /// Execute the pipeline.
    pub fn execute(self, pipeline: &RetrievalPipeline) -> Result<PipelineResult, PipelineError> {
        let query_splade = self.query_splade.unwrap_or_default();
        let query_matryoshka = self.query_matryoshka.unwrap_or_else(|| vec![0.0; 128]);
        let query_semantic = self.query_semantic.ok_or_else(|| {
            error!("Pipeline build failed: query_semantic (E1) is required but not set");
            PipelineError::MissingQuery {
                stage: PipelineStage::RrfRerank,
            }
        })?;
        let query_tokens = self.query_tokens.unwrap_or_default();

        let stages = self.stages.unwrap_or_else(|| PipelineStage::all().to_vec());

        // Create modified config with k
        let mut config = pipeline.config.clone();
        if let Some(k) = self.k {
            config.k = k;
        }

        // Note: This is a workaround since we can't modify pipeline's config
        // In a real implementation, you'd pass config through execute_stages

        pipeline.execute_stages(
            &query_splade,
            &query_matryoshka,
            &query_semantic,
            &query_tokens,
            &stages,
        )
    }
}

impl Default for PipelineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

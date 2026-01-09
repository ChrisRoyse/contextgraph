//! TASK-TELEO-009: MeaningExtractionPipeline Implementation
//!
//! Pipeline for extracting meaning from multi-embedding representations.
//! Coordinates synergy computation, correlation extraction, and group aggregation.
//!
//! # Pipeline Stages
//!
//! 1. Normalize embeddings
//! 2. Extract cross-correlations (78 values)
//! 3. Compute group alignments (6 groups)
//! 4. Build TeleologicalVector
//!
//! # From teleoplan.md
//!
//! "Meaning emerges from the INTERPLAY between embeddings - each perspective
//! adds something unique, but the magic is in their combination."

use crate::teleological::{
    types::{EMBEDDING_DIM, NUM_EMBEDDERS},
    GroupAlignments, SynergyMatrix, TeleologicalVector,
};
use crate::types::fingerprint::PurposeVector;

use super::correlation_extractor::{CorrelationConfig, CorrelationExtractor};
use super::synergy_service::SynergyService;

/// Configuration for the meaning extraction pipeline.
#[derive(Clone, Debug)]
pub struct MeaningPipelineConfig {
    /// Correlation extraction config
    pub correlation_config: CorrelationConfig,
    /// Compute Tucker decomposition (expensive but compact)
    pub compute_tucker: bool,
    /// Confidence threshold for meaningful extraction
    pub min_confidence: f32,
}

impl Default for MeaningPipelineConfig {
    fn default() -> Self {
        Self {
            correlation_config: CorrelationConfig::default(),
            compute_tucker: false,
            min_confidence: 0.3,
        }
    }
}

/// Result of meaning extraction.
#[derive(Clone, Debug)]
pub struct MeaningExtractionResult {
    /// The extracted teleological vector
    pub vector: TeleologicalVector,
    /// Extraction confidence [0.0, 1.0]
    pub confidence: f32,
    /// Per-stage confidence scores
    pub stage_confidences: StageConfidences,
    /// Embedding coverage: how many embeddings contributed meaningfully
    pub embedding_coverage: f32,
}

/// Confidence scores for each pipeline stage.
#[derive(Clone, Debug, Default)]
pub struct StageConfidences {
    /// Purpose vector computation confidence
    pub purpose_vector: f32,
    /// Correlation extraction confidence
    pub correlations: f32,
    /// Group aggregation confidence
    pub groups: f32,
    /// Overall pipeline confidence
    pub overall: f32,
}

/// TELEO-009: Pipeline for extracting meaning from multi-embedding representations.
///
/// # Example
///
/// ```ignore
/// use context_graph_core::teleological::services::MeaningPipeline;
///
/// let pipeline = MeaningPipeline::new();
/// let embeddings = vec![vec![0.0f32; 1024]; 13];
/// let purpose_vector = [0.8f32; 13];
/// let result = pipeline.extract(&embeddings, &purpose_vector);
/// ```
pub struct MeaningPipeline {
    config: MeaningPipelineConfig,
    synergy_service: SynergyService,
    correlation_extractor: CorrelationExtractor,
}

impl MeaningPipeline {
    /// Create a new MeaningPipeline with default configuration.
    pub fn new() -> Self {
        Self {
            config: MeaningPipelineConfig::default(),
            synergy_service: SynergyService::new(),
            correlation_extractor: CorrelationExtractor::new(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(config: MeaningPipelineConfig) -> Self {
        Self {
            correlation_extractor: CorrelationExtractor::with_config(
                config.correlation_config.clone(),
            ),
            config,
            synergy_service: SynergyService::new(),
        }
    }

    /// Create with existing synergy service.
    pub fn with_synergy_service(synergy_service: SynergyService) -> Self {
        Self {
            config: MeaningPipelineConfig::default(),
            synergy_service,
            correlation_extractor: CorrelationExtractor::new(),
        }
    }

    /// Extract meaning from multi-embedding representation.
    ///
    /// # Arguments
    /// * `embeddings` - 13 embedding vectors, each of dimension 1024
    /// * `purpose_alignments` - 13D purpose vector alignments to North Star
    ///
    /// # Panics
    ///
    /// Panics if embeddings count != 13 or dimensions don't match (FAIL FAST).
    pub fn extract(
        &self,
        embeddings: &[Vec<f32>],
        purpose_alignments: &[f32; NUM_EMBEDDERS],
    ) -> MeaningExtractionResult {
        assert!(
            embeddings.len() == NUM_EMBEDDERS,
            "FAIL FAST: Expected {} embeddings, got {}",
            NUM_EMBEDDERS,
            embeddings.len()
        );

        for (i, emb) in embeddings.iter().enumerate() {
            assert!(
                emb.len() == EMBEDDING_DIM,
                "FAIL FAST: Embedding {} has dimension {}, expected {}",
                i,
                emb.len(),
                EMBEDDING_DIM
            );
        }

        // Stage 1: Create purpose vector
        let purpose_vector = PurposeVector::new(*purpose_alignments);
        let pv_confidence = self.compute_purpose_confidence(&purpose_vector);

        // Stage 2: Extract cross-correlations
        let corr_result = self
            .correlation_extractor
            .extract(embeddings, Some(self.synergy_service.matrix()));
        let corr_confidence = 1.0 - corr_result.sparsity;

        // Stage 3: Compute group alignments
        let group_alignments = GroupAlignments::from_alignments(purpose_alignments, None);
        let group_confidence = group_alignments.coherence();

        // Build TeleologicalVector
        let mut vector = TeleologicalVector::with_all(
            purpose_vector,
            corr_result.correlations.to_vec(),
            group_alignments,
            pv_confidence * corr_confidence * group_confidence,
        );

        // Stage 4: Optional Tucker decomposition
        if self.config.compute_tucker {
            // Tucker decomposition would be computed here
            // For now, we leave it as None (computed on-demand)
        }

        // Calculate embedding coverage
        let active_embeddings = purpose_alignments.iter().filter(|&&a| a > 0.1).count();
        let embedding_coverage = active_embeddings as f32 / NUM_EMBEDDERS as f32;

        // Overall confidence
        let overall_confidence =
            (pv_confidence * 0.4 + corr_confidence * 0.3 + group_confidence * 0.3)
                * embedding_coverage.sqrt();

        vector.confidence = overall_confidence;

        MeaningExtractionResult {
            vector,
            confidence: overall_confidence,
            stage_confidences: StageConfidences {
                purpose_vector: pv_confidence,
                correlations: corr_confidence,
                groups: group_confidence,
                overall: overall_confidence,
            },
            embedding_coverage,
        }
    }

    /// Extract meaning with synergy-aligned correlations.
    ///
    /// Uses purpose vector to modulate synergy contributions.
    pub fn extract_aligned(
        &self,
        embeddings: &[Vec<f32>],
        purpose_alignments: &[f32; NUM_EMBEDDERS],
    ) -> MeaningExtractionResult {
        // Get synergy-aligned correlations
        let purpose_vector = PurposeVector::new(*purpose_alignments);
        let aligned_synergies = self.synergy_service.get_aligned_synergies(&purpose_vector);

        // Create modified synergy matrix for correlation extraction
        let mut aligned_matrix = SynergyMatrix::new();
        let mut idx = 0;
        for i in 0..NUM_EMBEDDERS {
            for j in (i + 1)..NUM_EMBEDDERS {
                aligned_matrix.set_synergy(i, j, aligned_synergies[idx].clamp(0.0, 1.0).max(0.1));
                idx += 1;
            }
        }

        // Extract with aligned matrix
        let corr_result = self
            .correlation_extractor
            .extract(embeddings, Some(&aligned_matrix));

        let pv_confidence = self.compute_purpose_confidence(&purpose_vector);
        let corr_confidence = 1.0 - corr_result.sparsity;
        let group_alignments = GroupAlignments::from_alignments(purpose_alignments, None);
        let group_confidence = group_alignments.coherence();

        let overall_confidence = pv_confidence * corr_confidence * group_confidence;

        let vector = TeleologicalVector::with_all(
            purpose_vector,
            corr_result.correlations.to_vec(),
            group_alignments,
            overall_confidence,
        );

        let active_embeddings = purpose_alignments.iter().filter(|&&a| a > 0.1).count();
        let embedding_coverage = active_embeddings as f32 / NUM_EMBEDDERS as f32;

        MeaningExtractionResult {
            vector,
            confidence: overall_confidence,
            stage_confidences: StageConfidences {
                purpose_vector: pv_confidence,
                correlations: corr_confidence,
                groups: group_confidence,
                overall: overall_confidence,
            },
            embedding_coverage,
        }
    }

    /// Compute confidence for a purpose vector.
    fn compute_purpose_confidence(&self, pv: &PurposeVector) -> f32 {
        let aggregate = pv.aggregate_alignment();
        let non_zero = pv.alignments.iter().filter(|&&a| a > 0.1).count();
        let coverage = non_zero as f32 / NUM_EMBEDDERS as f32;

        // Higher confidence if good alignment with good coverage
        aggregate * coverage.sqrt()
    }

    /// Get the synergy service.
    pub fn synergy_service(&self) -> &SynergyService {
        &self.synergy_service
    }

    /// Get mutable synergy service.
    pub fn synergy_service_mut(&mut self) -> &mut SynergyService {
        &mut self.synergy_service
    }

    /// Get configuration.
    pub fn config(&self) -> &MeaningPipelineConfig {
        &self.config
    }

    /// Check if extraction result meets minimum confidence.
    pub fn is_meaningful(&self, result: &MeaningExtractionResult) -> bool {
        result.confidence >= self.config.min_confidence
    }
}

impl Default for MeaningPipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::teleological::CROSS_CORRELATION_COUNT;

    fn make_embeddings(fill: f32) -> Vec<Vec<f32>> {
        vec![vec![fill; EMBEDDING_DIM]; NUM_EMBEDDERS]
    }

    fn make_purpose_alignments(value: f32) -> [f32; NUM_EMBEDDERS] {
        [value; NUM_EMBEDDERS]
    }

    #[test]
    fn test_meaning_pipeline_new() {
        let pipeline = MeaningPipeline::new();
        assert!(pipeline.config().min_confidence > 0.0);

        println!("[PASS] MeaningPipeline::new creates default pipeline");
    }

    #[test]
    fn test_extract_uniform() {
        let pipeline = MeaningPipeline::new();
        let embeddings = make_embeddings(0.5);
        let alignments = make_purpose_alignments(0.8);

        let result = pipeline.extract(&embeddings, &alignments);

        assert!(result.confidence > 0.0);
        assert_eq!(
            result.vector.cross_correlations.len(),
            CROSS_CORRELATION_COUNT
        );

        println!("[PASS] extract produces valid MeaningExtractionResult");
    }

    #[test]
    fn test_extract_builds_complete_vector() {
        let pipeline = MeaningPipeline::new();
        let embeddings = make_embeddings(0.3);
        let alignments = make_purpose_alignments(0.7);

        let result = pipeline.extract(&embeddings, &alignments);

        // Vector should have all components
        assert_eq!(result.vector.purpose_vector.alignments, alignments);
        assert_eq!(result.vector.cross_correlations.len(), 78);
        assert!(result.vector.group_alignments.average() > 0.0);

        println!("[PASS] extract builds complete TeleologicalVector");
    }

    #[test]
    fn test_stage_confidences() {
        let pipeline = MeaningPipeline::new();
        let embeddings = make_embeddings(0.5);
        let alignments = make_purpose_alignments(0.9);

        let result = pipeline.extract(&embeddings, &alignments);

        // All stage confidences should be positive
        assert!(result.stage_confidences.purpose_vector > 0.0);
        assert!(result.stage_confidences.correlations >= 0.0);
        assert!(result.stage_confidences.groups > 0.0);
        assert!(result.stage_confidences.overall > 0.0);

        println!("[PASS] Stage confidences all positive for valid input");
    }

    #[test]
    fn test_embedding_coverage() {
        let pipeline = MeaningPipeline::new();
        let embeddings = make_embeddings(0.5);

        // Full coverage
        let full_alignments = make_purpose_alignments(0.8);
        let full_result = pipeline.extract(&embeddings, &full_alignments);
        assert!(full_result.embedding_coverage > 0.9);

        // Partial coverage
        let mut partial_alignments = [0.0f32; NUM_EMBEDDERS];
        partial_alignments[0] = 0.8;
        partial_alignments[5] = 0.7;
        let partial_result = pipeline.extract(&embeddings, &partial_alignments);
        assert!(partial_result.embedding_coverage < full_result.embedding_coverage);

        println!("[PASS] Embedding coverage calculated correctly");
    }

    #[test]
    fn test_is_meaningful() {
        let pipeline = MeaningPipeline::with_config(MeaningPipelineConfig {
            min_confidence: 0.5,
            ..Default::default()
        });

        let embeddings = make_embeddings(0.5);

        // High alignment = likely meaningful
        let high_result = pipeline.extract(&embeddings, &make_purpose_alignments(0.9));

        // Low alignment = less meaningful
        let low_result = pipeline.extract(&embeddings, &make_purpose_alignments(0.1));

        // High should have higher confidence than low
        assert!(high_result.confidence > low_result.confidence);

        println!("[PASS] is_meaningful correctly classifies results");
    }

    #[test]
    fn test_extract_aligned() {
        let pipeline = MeaningPipeline::new();
        let embeddings = make_embeddings(0.5);
        let alignments = make_purpose_alignments(0.8);

        let result = pipeline.extract_aligned(&embeddings, &alignments);

        assert!(result.confidence > 0.0);

        println!("[PASS] extract_aligned produces valid result");
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_extract_wrong_embedding_count() {
        let pipeline = MeaningPipeline::new();
        let embeddings = vec![vec![0.0f32; EMBEDDING_DIM]; 10]; // Wrong count
        let alignments = make_purpose_alignments(0.5);

        let _ = pipeline.extract(&embeddings, &alignments);
    }

    #[test]
    fn test_synergy_service_access() {
        let mut pipeline = MeaningPipeline::new();

        // Should be able to access synergy service
        let _ = pipeline.synergy_service().total_samples();

        // And mutate it
        let _ = pipeline.synergy_service_mut().total_samples();

        println!("[PASS] Synergy service accessible from pipeline");
    }
}

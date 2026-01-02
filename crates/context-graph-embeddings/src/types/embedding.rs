//! Single model embedding output with validation and normalization.
//!
//! This module provides the `ModelEmbedding` struct which represents
//! the output from a single embedding model in the 12-model pipeline.

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::types::ModelId;

/// Represents the embedding output from a single model.
///
/// # Fields
/// - `model_id`: Which of the 12 models produced this embedding
/// - `vector`: The embedding vector (f32 for GPU compatibility)
/// - `latency_us`: Time taken to generate embedding in microseconds
/// - `attention_weights`: Optional attention scores for interpretability
/// - `is_projected`: Whether vector has been projected to standard dimension
///
/// # Example
/// ```rust
/// use context_graph_embeddings::types::{ModelId, ModelEmbedding};
///
/// let embedding = ModelEmbedding::new(
///     ModelId::Semantic,
///     vec![0.1, 0.2, 0.3],  // simplified - real would be 1024 dims
///     1500,
/// );
/// assert_eq!(embedding.dimension(), 3);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ModelEmbedding {
    /// The model that produced this embedding (E1-E12)
    pub model_id: ModelId,

    /// The embedding vector - f32 for GPU compatibility
    pub vector: Vec<f32>,

    /// Generation latency in microseconds
    pub latency_us: u64,

    /// Optional attention weights for interpretability
    /// Length must match input token count when present
    pub attention_weights: Option<Vec<f32>>,

    /// Whether this vector has been projected to standard dimension
    pub is_projected: bool,
}

impl ModelEmbedding {
    /// Creates a new ModelEmbedding.
    ///
    /// # Arguments
    /// * `model_id` - The model that produced this embedding
    /// * `vector` - The raw embedding vector
    /// * `latency_us` - Generation time in microseconds
    ///
    /// # Note
    /// This does NOT validate the embedding. Call `validate()` after creation
    /// to ensure the embedding meets all requirements.
    #[inline]
    pub fn new(model_id: ModelId, vector: Vec<f32>, latency_us: u64) -> Self {
        Self {
            model_id,
            vector,
            latency_us,
            attention_weights: None,
            is_projected: false,
        }
    }

    /// Creates a new ModelEmbedding with attention weights.
    ///
    /// # Arguments
    /// * `model_id` - The model that produced this embedding
    /// * `vector` - The raw embedding vector
    /// * `latency_us` - Generation time in microseconds
    /// * `attention_weights` - Attention scores from the model
    #[inline]
    pub fn with_attention(
        model_id: ModelId,
        vector: Vec<f32>,
        latency_us: u64,
        attention_weights: Vec<f32>,
    ) -> Self {
        Self {
            model_id,
            vector,
            latency_us,
            attention_weights: Some(attention_weights),
            is_projected: false,
        }
    }

    /// Returns the dimension of the embedding vector.
    #[inline]
    pub fn dimension(&self) -> usize {
        self.vector.len()
    }

    /// Returns true if the embedding vector is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.vector.is_empty()
    }

    /// Validates the embedding against model requirements.
    ///
    /// # Validation Rules
    /// 1. Vector dimension must match `model_id.dimension()` (or `projected_dimension()` if projected)
    /// 2. No NaN values allowed in vector
    /// 3. No Inf values allowed in vector
    /// 4. Vector must not be empty
    ///
    /// # Errors
    /// - `EmbeddingError::EmptyInput` if vector is empty
    /// - `EmbeddingError::InvalidDimension` if dimension doesn't match model
    /// - `EmbeddingError::EmptyInput` if NaN or Inf values found
    ///
    /// # Fail Fast
    /// This method fails immediately on first error - no partial validation.
    pub fn validate(&self) -> EmbeddingResult<()> {
        // Rule 1: Vector must not be empty
        if self.vector.is_empty() {
            return Err(EmbeddingError::EmptyInput);
        }

        // Rule 2: Check dimension matches expected
        let expected_dim = if self.is_projected {
            self.model_id.projected_dimension()
        } else {
            self.model_id.dimension()
        };

        if self.vector.len() != expected_dim {
            return Err(EmbeddingError::InvalidDimension {
                expected: expected_dim,
                actual: self.vector.len(),
            });
        }

        // Rule 3 & 4: Check for NaN and Inf values (fail fast)
        for (idx, &val) in self.vector.iter().enumerate() {
            if val.is_nan() || val.is_infinite() {
                return Err(EmbeddingError::InvalidValue { index: idx, value: val });
            }
        }

        Ok(())
    }

    /// Calculates the L2 (Euclidean) norm of the vector.
    ///
    /// # Returns
    /// The square root of the sum of squared elements.
    /// Returns 0.0 for empty vectors.
    ///
    /// # Performance
    /// Uses SIMD-friendly loop pattern for GPU/CPU optimization.
    #[inline]
    pub fn l2_norm(&self) -> f32 {
        if self.vector.is_empty() {
            return 0.0;
        }

        let sum_squares: f32 = self.vector.iter().map(|x| x * x).sum();

        sum_squares.sqrt()
    }

    /// Normalizes the vector to unit length (L2 norm = 1.0).
    ///
    /// # Behavior
    /// - After normalization, `l2_norm()` returns approximately 1.0
    /// - Zero vectors remain unchanged (avoids division by zero)
    /// - Empty vectors remain unchanged
    ///
    /// # Panics
    /// Does not panic. Zero vectors are handled gracefully.
    pub fn normalize(&mut self) {
        let norm = self.l2_norm();

        // Avoid division by zero - zero vectors stay zero
        if norm > f32::EPSILON {
            for val in &mut self.vector {
                *val /= norm;
            }
        }
    }

    /// Returns a normalized copy of this embedding.
    ///
    /// # Returns
    /// A new ModelEmbedding with the same metadata but normalized vector.
    pub fn normalized(&self) -> Self {
        let mut copy = self.clone();
        copy.normalize();
        copy
    }

    /// Checks if the vector is normalized (L2 norm ≈ 1.0).
    ///
    /// # Arguments
    /// * `epsilon` - Tolerance for floating point comparison (default: 1e-6)
    #[inline]
    pub fn is_normalized(&self, epsilon: f32) -> bool {
        if self.vector.is_empty() {
            return false;
        }
        (self.l2_norm() - 1.0).abs() < epsilon
    }

    /// Marks this embedding as projected to standard dimension.
    ///
    /// # Note
    /// This should be called after projection to update validation expectations.
    #[inline]
    pub fn set_projected(&mut self, projected: bool) {
        self.is_projected = projected;
    }

    /// Validates attention weights if present.
    ///
    /// # Arguments
    /// * `expected_token_count` - The number of input tokens
    ///
    /// # Errors
    /// - `EmbeddingError::InvalidDimension` if attention weights length != token count
    /// - `EmbeddingError::InvalidValue` if NaN/Inf in attention weights
    pub fn validate_attention(&self, expected_token_count: usize) -> EmbeddingResult<()> {
        if let Some(ref weights) = self.attention_weights {
            if weights.len() != expected_token_count {
                return Err(EmbeddingError::InvalidDimension {
                    expected: expected_token_count,
                    actual: weights.len(),
                });
            }

            for (idx, &val) in weights.iter().enumerate() {
                if val.is_nan() || val.is_infinite() {
                    return Err(EmbeddingError::InvalidValue { index: idx, value: val });
                }
            }
        }
        Ok(())
    }

    /// Computes cosine similarity with another embedding.
    ///
    /// # Arguments
    /// * `other` - The embedding to compare against
    ///
    /// # Returns
    /// Cosine similarity in range [-1.0, 1.0]
    ///
    /// # Errors
    /// - `EmbeddingError::InvalidDimension` if dimensions don't match
    /// - `EmbeddingError::EmptyInput` if either vector is empty
    pub fn cosine_similarity(&self, other: &Self) -> EmbeddingResult<f32> {
        if self.vector.is_empty() || other.vector.is_empty() {
            return Err(EmbeddingError::EmptyInput);
        }

        if self.vector.len() != other.vector.len() {
            return Err(EmbeddingError::InvalidDimension {
                expected: self.vector.len(),
                actual: other.vector.len(),
            });
        }

        let dot_product: f32 = self
            .vector
            .iter()
            .zip(other.vector.iter())
            .map(|(a, b)| a * b)
            .sum();

        let norm_a = self.l2_norm();
        let norm_b = other.l2_norm();

        if norm_a < f32::EPSILON || norm_b < f32::EPSILON {
            return Ok(0.0);
        }

        Ok(dot_product / (norm_a * norm_b))
    }

    // ========== GPU Tensor Conversions ==========

    /// Convert embedding to GPU tensor.
    ///
    /// # Arguments
    /// * `device` - The GPU device to create the tensor on
    ///
    /// # Returns
    /// A 1D tensor containing the embedding vector.
    ///
    /// # Example
    /// ```rust,ignore
    /// use context_graph_embeddings::gpu::device;
    ///
    /// let embedding = ModelEmbedding::new(ModelId::Semantic, vec![0.1, 0.2, 0.3], 100);
    /// let tensor = embedding.to_tensor(device())?;
    /// assert_eq!(tensor.dims(), &[3]);
    /// ```
    #[cfg(feature = "candle")]
    pub fn to_tensor(&self, device: &candle_core::Device) -> candle_core::Result<candle_core::Tensor> {
        candle_core::Tensor::from_slice(&self.vector, (self.vector.len(),), device)
    }

    /// Create embedding from GPU tensor.
    ///
    /// # Arguments
    /// * `tensor` - A 1D tensor containing embedding values
    /// * `model_id` - The model ID for the new embedding
    ///
    /// # Returns
    /// A new ModelEmbedding with the tensor values and zero latency.
    ///
    /// # Example
    /// ```rust,ignore
    /// let tensor = Tensor::from_slice(&[0.1, 0.2, 0.3], (3,), device)?;
    /// let embedding = ModelEmbedding::from_tensor(&tensor, ModelId::Semantic)?;
    /// assert_eq!(embedding.dimension(), 3);
    /// ```
    #[cfg(feature = "candle")]
    pub fn from_tensor(tensor: &candle_core::Tensor, model_id: ModelId) -> candle_core::Result<Self> {
        let vector: Vec<f32> = tensor.to_vec1()?;
        Ok(Self::new(model_id, vector, 0))
    }

    /// Convert batch of embeddings to GPU tensor.
    ///
    /// # Arguments
    /// * `embeddings` - Slice of embeddings to batch
    /// * `device` - The GPU device to create the tensor on
    ///
    /// # Returns
    /// A 2D tensor of shape [batch_size, dim].
    #[cfg(feature = "candle")]
    pub fn batch_to_tensor(
        embeddings: &[Self],
        device: &candle_core::Device,
    ) -> candle_core::Result<candle_core::Tensor> {
        if embeddings.is_empty() {
            return candle_core::Tensor::zeros((0, 0), candle_core::DType::F32, device);
        }

        let dim = embeddings[0].dimension();
        let batch_size = embeddings.len();

        // Flatten all vectors into a single slice
        let data: Vec<f32> = embeddings.iter().flat_map(|e| e.vector.iter().copied()).collect();

        candle_core::Tensor::from_slice(&data, (batch_size, dim), device)
    }

    /// Create batch of embeddings from GPU tensor.
    ///
    /// # Arguments
    /// * `tensor` - A 2D tensor of shape [batch_size, dim]
    /// * `model_id` - The model ID for all embeddings in the batch
    ///
    /// # Returns
    /// Vector of ModelEmbedding instances.
    #[cfg(feature = "candle")]
    pub fn batch_from_tensor(
        tensor: &candle_core::Tensor,
        model_id: ModelId,
    ) -> candle_core::Result<Vec<Self>> {
        let vectors: Vec<Vec<f32>> = tensor.to_vec2()?;
        Ok(vectors.into_iter().map(|v| Self::new(model_id, v, 0)).collect())
    }
}

impl Default for ModelEmbedding {
    fn default() -> Self {
        Self {
            model_id: ModelId::Semantic,
            vector: Vec::new(),
            latency_us: 0,
            attention_weights: None,
            is_projected: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ModelId;

    // ========== Construction Tests ==========

    #[test]
    fn test_new_creates_valid_embedding() {
        let vector = vec![0.1, 0.2, 0.3];
        let embedding = ModelEmbedding::new(ModelId::Semantic, vector.clone(), 1500);

        assert_eq!(embedding.model_id, ModelId::Semantic);
        assert_eq!(embedding.vector, vector);
        assert_eq!(embedding.latency_us, 1500);
        assert!(embedding.attention_weights.is_none());
        assert!(!embedding.is_projected);
    }

    #[test]
    fn test_with_attention_creates_embedding_with_weights() {
        let vector = vec![0.1, 0.2, 0.3];
        let attention = vec![0.5, 0.3, 0.2];
        let embedding = ModelEmbedding::with_attention(
            ModelId::TemporalRecent,
            vector.clone(),
            2000,
            attention.clone(),
        );

        assert_eq!(embedding.attention_weights, Some(attention));
    }

    #[test]
    fn test_default_creates_empty_embedding() {
        let embedding = ModelEmbedding::default();

        assert_eq!(embedding.model_id, ModelId::Semantic);
        assert!(embedding.vector.is_empty());
        assert_eq!(embedding.latency_us, 0);
    }

    // ========== Dimension Tests ==========

    #[test]
    fn test_dimension_returns_vector_length() {
        let embedding = ModelEmbedding::new(ModelId::Semantic, vec![1.0; 1024], 100);
        assert_eq!(embedding.dimension(), 1024);
    }

    #[test]
    fn test_is_empty_for_empty_vector() {
        let embedding = ModelEmbedding::default();
        assert!(embedding.is_empty());
    }

    #[test]
    fn test_is_empty_for_non_empty_vector() {
        let embedding = ModelEmbedding::new(ModelId::Semantic, vec![1.0], 100);
        assert!(!embedding.is_empty());
    }

    // ========== Validation Tests ==========

    #[test]
    fn test_validate_correct_dimension_succeeds() {
        // Semantic has dimension 1024
        let embedding = ModelEmbedding::new(ModelId::Semantic, vec![0.1; 1024], 100);
        assert!(embedding.validate().is_ok());
    }

    #[test]
    fn test_validate_wrong_dimension_fails() {
        let embedding = ModelEmbedding::new(
            ModelId::Semantic,
            vec![0.1; 512], // Wrong: should be 1024
            100,
        );

        let err = embedding.validate().unwrap_err();
        match err {
            EmbeddingError::InvalidDimension { expected, actual } => {
                assert_eq!(expected, 1024);
                assert_eq!(actual, 512);
            }
            _ => panic!("Expected InvalidDimension error"),
        }
    }

    #[test]
    fn test_validate_empty_vector_fails() {
        let embedding = ModelEmbedding::default();

        let err = embedding.validate().unwrap_err();
        assert!(
            matches!(err, EmbeddingError::EmptyInput),
            "Expected EmptyInput error for empty vector"
        );
    }

    #[test]
    fn test_validate_nan_value_fails() {
        let mut vector = vec![0.1; 1024];
        vector[500] = f32::NAN;

        let embedding = ModelEmbedding::new(ModelId::Semantic, vector, 100);

        let err = embedding.validate().unwrap_err();
        match err {
            EmbeddingError::InvalidValue { index, value } => {
                assert_eq!(index, 500);
                assert!(value.is_nan());
            }
            _ => panic!("Expected InvalidValue error for NaN"),
        }
    }

    #[test]
    fn test_validate_positive_infinity_fails() {
        let mut vector = vec![0.1; 1024];
        vector[100] = f32::INFINITY;

        let embedding = ModelEmbedding::new(ModelId::Semantic, vector, 100);

        let err = embedding.validate().unwrap_err();
        match err {
            EmbeddingError::InvalidValue { index, value } => {
                assert_eq!(index, 100);
                assert!(value.is_infinite() && value.is_sign_positive());
            }
            _ => panic!("Expected InvalidValue error for Inf"),
        }
    }

    #[test]
    fn test_validate_negative_infinity_fails() {
        let mut vector = vec![0.1; 1024];
        vector[200] = f32::NEG_INFINITY;

        let embedding = ModelEmbedding::new(ModelId::Semantic, vector, 100);

        let err = embedding.validate().unwrap_err();
        match err {
            EmbeddingError::InvalidValue { index, value } => {
                assert_eq!(index, 200);
                assert!(value.is_infinite() && value.is_sign_negative());
            }
            _ => panic!("Expected InvalidValue error for -Inf"),
        }
    }

    #[test]
    fn test_validate_projected_uses_projected_dimension() {
        // Sparse has dimension() = 30522 but projected_dimension() = 1536
        let mut embedding = ModelEmbedding::new(
            ModelId::Sparse,
            vec![0.1; 1536], // projected_dimension() = 1536
            100,
        );
        embedding.set_projected(true);

        assert!(embedding.validate().is_ok());
    }

    // ========== L2 Norm Tests ==========

    #[test]
    fn test_l2_norm_unit_vector() {
        let embedding = ModelEmbedding::new(ModelId::Semantic, vec![1.0, 0.0, 0.0], 100);
        assert!((embedding.l2_norm() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_l2_norm_known_value() {
        // sqrt(3^2 + 4^2) = sqrt(9 + 16) = sqrt(25) = 5
        let embedding = ModelEmbedding::new(ModelId::Semantic, vec![3.0, 4.0], 100);
        assert!((embedding.l2_norm() - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_l2_norm_zero_vector() {
        let embedding = ModelEmbedding::new(ModelId::Semantic, vec![0.0, 0.0, 0.0], 100);
        assert_eq!(embedding.l2_norm(), 0.0);
    }

    #[test]
    fn test_l2_norm_empty_vector() {
        let embedding = ModelEmbedding::default();
        assert_eq!(embedding.l2_norm(), 0.0);
    }

    #[test]
    fn test_l2_norm_single_element() {
        let embedding = ModelEmbedding::new(ModelId::Semantic, vec![5.0], 100);
        assert!((embedding.l2_norm() - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_l2_norm_negative_values() {
        let embedding = ModelEmbedding::new(ModelId::Semantic, vec![-3.0, -4.0], 100);
        assert!((embedding.l2_norm() - 5.0).abs() < 1e-6);
    }

    // ========== Normalization Tests ==========

    #[test]
    fn test_normalize_produces_unit_vector() {
        let mut embedding = ModelEmbedding::new(ModelId::Semantic, vec![3.0, 4.0, 0.0], 100);
        embedding.normalize();

        let norm = embedding.l2_norm();
        assert!(
            (norm - 1.0).abs() < 1e-6,
            "L2 norm should be 1.0, got {}",
            norm
        );
    }

    #[test]
    fn test_normalize_preserves_direction() {
        let original = vec![3.0, 4.0];
        let mut embedding = ModelEmbedding::new(ModelId::Semantic, original.clone(), 100);
        embedding.normalize();

        // Check ratio is preserved
        let ratio = embedding.vector[0] / embedding.vector[1];
        let expected_ratio = 3.0 / 4.0;
        assert!((ratio - expected_ratio).abs() < 1e-6);
    }

    #[test]
    fn test_normalize_zero_vector_unchanged() {
        let mut embedding = ModelEmbedding::new(ModelId::Semantic, vec![0.0, 0.0, 0.0], 100);
        embedding.normalize();

        assert_eq!(embedding.vector, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_normalize_empty_vector_unchanged() {
        let mut embedding = ModelEmbedding::default();
        embedding.normalize();

        assert!(embedding.vector.is_empty());
    }

    #[test]
    fn test_normalized_returns_new_embedding() {
        let embedding = ModelEmbedding::new(ModelId::Semantic, vec![3.0, 4.0], 100);
        let normalized = embedding.normalized();

        // Original unchanged
        assert_eq!(embedding.vector, vec![3.0, 4.0]);
        // New is normalized
        assert!((normalized.l2_norm() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_is_normalized_true_for_unit_vector() {
        let mut embedding = ModelEmbedding::new(ModelId::Semantic, vec![1.0, 2.0, 3.0], 100);
        embedding.normalize();

        assert!(embedding.is_normalized(1e-6));
    }

    #[test]
    fn test_is_normalized_false_for_non_unit_vector() {
        let embedding = ModelEmbedding::new(ModelId::Semantic, vec![1.0, 2.0, 3.0], 100);

        assert!(!embedding.is_normalized(1e-6));
    }

    // ========== Attention Weight Tests ==========

    #[test]
    fn test_validate_attention_correct_length_succeeds() {
        let embedding = ModelEmbedding::with_attention(
            ModelId::Semantic,
            vec![0.1; 1024],
            100,
            vec![0.5, 0.3, 0.2],
        );

        assert!(embedding.validate_attention(3).is_ok());
    }

    #[test]
    fn test_validate_attention_wrong_length_fails() {
        let embedding = ModelEmbedding::with_attention(
            ModelId::Semantic,
            vec![0.1; 1024],
            100,
            vec![0.5, 0.3, 0.2],
        );

        let err = embedding.validate_attention(5).unwrap_err();
        match err {
            EmbeddingError::InvalidDimension { expected, actual } => {
                assert_eq!(expected, 5);
                assert_eq!(actual, 3);
            }
            _ => panic!("Expected InvalidDimension error"),
        }
    }

    #[test]
    fn test_validate_attention_nan_fails() {
        let embedding = ModelEmbedding::with_attention(
            ModelId::Semantic,
            vec![0.1; 1024],
            100,
            vec![0.5, f32::NAN, 0.2],
        );

        let err = embedding.validate_attention(3).unwrap_err();
        match err {
            EmbeddingError::InvalidValue { index, value } => {
                assert_eq!(index, 1);
                assert!(value.is_nan());
            }
            _ => panic!("Expected InvalidValue error"),
        }
    }

    #[test]
    fn test_validate_attention_none_succeeds() {
        let embedding = ModelEmbedding::new(ModelId::Semantic, vec![0.1; 1024], 100);

        // Should succeed when no attention weights present
        assert!(embedding.validate_attention(10).is_ok());
    }

    // ========== Cosine Similarity Tests ==========

    #[test]
    fn test_cosine_similarity_identical_vectors() {
        let embedding1 = ModelEmbedding::new(ModelId::Semantic, vec![1.0, 2.0, 3.0], 100);
        let embedding2 = embedding1.clone();

        let sim = embedding1.cosine_similarity(&embedding2).unwrap();
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite_vectors() {
        let embedding1 = ModelEmbedding::new(ModelId::Semantic, vec![1.0, 0.0, 0.0], 100);
        let embedding2 = ModelEmbedding::new(ModelId::Semantic, vec![-1.0, 0.0, 0.0], 100);

        let sim = embedding1.cosine_similarity(&embedding2).unwrap();
        assert!((sim - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal_vectors() {
        let embedding1 = ModelEmbedding::new(ModelId::Semantic, vec![1.0, 0.0], 100);
        let embedding2 = ModelEmbedding::new(ModelId::Semantic, vec![0.0, 1.0], 100);

        let sim = embedding1.cosine_similarity(&embedding2).unwrap();
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_dimension_mismatch_fails() {
        let embedding1 = ModelEmbedding::new(ModelId::Semantic, vec![1.0, 2.0, 3.0], 100);
        let embedding2 = ModelEmbedding::new(ModelId::Semantic, vec![1.0, 2.0], 100);

        let err = embedding1.cosine_similarity(&embedding2).unwrap_err();
        match err {
            EmbeddingError::InvalidDimension { .. } => {}
            _ => panic!("Expected InvalidDimension error"),
        }
    }

    #[test]
    fn test_cosine_similarity_empty_vector_fails() {
        let embedding1 = ModelEmbedding::default();
        let embedding2 = ModelEmbedding::new(ModelId::Semantic, vec![1.0, 2.0], 100);

        let err = embedding1.cosine_similarity(&embedding2).unwrap_err();
        assert!(
            matches!(err, EmbeddingError::EmptyInput),
            "Expected EmptyInput error"
        );
    }

    // ========== All Model Dimension Tests ==========

    #[test]
    fn test_validate_all_model_dimensions() {
        // All 12 models with their native dimensions from ModelId::dimension()
        let models_and_dims = [
            (ModelId::Semantic, 1024),          // E1: e5-large-v2
            (ModelId::TemporalRecent, 512),     // E2: Custom exponential decay
            (ModelId::TemporalPeriodic, 512),   // E3: Custom Fourier basis
            (ModelId::TemporalPositional, 512), // E4: Custom sinusoidal PE
            (ModelId::Causal, 768),             // E5: Longformer
            (ModelId::Sparse, 30522),           // E6: SPLADE (sparse vocab)
            (ModelId::Code, 256),               // E7: CodeT5p embed_dim
            (ModelId::Graph, 384),              // E8: paraphrase-MiniLM
            (ModelId::Hdc, 10000),              // E9: Hyperdimensional (10K-bit)
            (ModelId::Multimodal, 768),         // E10: CLIP
            (ModelId::Entity, 384),             // E11: all-MiniLM
            (ModelId::LateInteraction, 128),    // E12: ColBERT per-token
        ];

        for (model_id, expected_dim) in models_and_dims {
            let embedding = ModelEmbedding::new(model_id, vec![0.1; expected_dim], 100);

            assert!(
                embedding.validate().is_ok(),
                "Validation failed for {:?} with dimension {}",
                model_id,
                expected_dim
            );
        }
    }

    // ========== Edge Case Tests ==========

    #[test]
    fn test_large_vector_normalization() {
        let large_vec: Vec<f32> = (0..10000).map(|i| i as f32).collect();
        let mut embedding = ModelEmbedding::new(ModelId::Semantic, large_vec, 100);
        embedding.normalize();

        assert!((embedding.l2_norm() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_very_small_values_normalize() {
        // Values with norm < f32::EPSILON should remain unchanged
        let original = vec![1e-20, 1e-20, 1e-20];
        let mut embedding = ModelEmbedding::new(ModelId::Semantic, original.clone(), 100);

        // Verify norm is below EPSILON threshold
        let norm_before = embedding.l2_norm();
        assert!(norm_before < f32::EPSILON, "Test assumes norm < EPSILON");

        embedding.normalize();

        // Vector should remain unchanged (avoid division by near-zero)
        assert_eq!(embedding.vector, original);
        assert_eq!(embedding.l2_norm(), norm_before);
    }

    #[test]
    fn test_clone_preserves_all_fields() {
        let embedding = ModelEmbedding {
            model_id: ModelId::Causal,
            vector: vec![1.0, 2.0, 3.0],
            latency_us: 5000,
            attention_weights: Some(vec![0.5, 0.5]),
            is_projected: true,
        };

        let cloned = embedding.clone();
        assert_eq!(embedding, cloned);
    }
}

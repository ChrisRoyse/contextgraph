// Allow deprecated items within this deprecated module
#![allow(deprecated)]

//! DEPRECATED: Context Projection Weights for Asymmetric Paraphrase/Context Embeddings.
//!
//! # DEPRECATION NOTICE
//!
//! This module is **DEPRECATED** as of v0.2.0. The ContextualModel now uses
//! intfloat/e5-base-v2 with prefix-based asymmetric encoding instead of
//! projection matrices.
//!
//! E5-base-v2 achieves asymmetry through learned prefixes:
//! - "query: " prefix for paraphrase embeddings
//! - "passage: " prefix for context embeddings
//!
//! This creates genuinely learned asymmetric representations, unlike the
//! projection-based approach which only added random perturbations.
//!
//! See `model.rs::embed_dual()` for the new implementation.
//!
//! # Original Documentation (for reference)
//!
//! Following the E5 Causal pattern (ARCH-15) and E8 Graph pattern, this module
//! provided learned projection matrices for creating asymmetric paraphrase/context embeddings.
//!
//! # Architecture
//!
//! The projections transform the base MPNet embedding into paraphrase-role and
//! context-role vectors. They are initialized as perturbed identity matrices
//! (I + N(0, 0.02)) to create immediate asymmetry without training.
//!
//! ```text
//! base_embedding [768D]
//!        |
//!    +---+---+
//!    |       |
//! W_intent  W_context
//!    |       |
//! intent_vec context_vec
//! [768D]     [768D]
//! ```
//!
//! # Semantic Meaning
//!
//! - **Intent**: What the text is trying to accomplish (action-focused)
//! - **Context**: What contextual relationships the text establishes (relation-focused)
//!
//! # Reference
//!
//! - E5 Causal projections: `causal/weights.rs`
//! - E8 Graph projections: `graph/projections.rs`

use candle_core::{DType, Device, Tensor};
use rand::Rng;

use crate::error::{EmbeddingError, EmbeddingResult};

use super::constants::CONTEXTUAL_DIMENSION;

/// Seed for context projection initialization (deterministic).
/// Different from E5's 0xCA05A1 and E8's 0xE8_6FA9 to ensure distinct projections.
#[deprecated(
    since = "0.2.0",
    note = "E5-base-v2 uses prefix-based asymmetry. See model.rs embed_dual()."
)]
pub const CONTEXT_PROJECTION_SEED: u64 = 0xE10_C0E7;

/// Standard deviation for initializing projection weight perturbations.
const PROJECTION_INIT_STD: f64 = 0.02;

/// Learned projection weights for asymmetric paraphrase/context embeddings.
///
/// **DEPRECATED**: This struct is deprecated as of v0.2.0. The ContextualModel
/// now uses E5-base-v2's prefix-based asymmetric encoding instead.
///
/// These projections transform the base MPNet embedding into
/// intent-role and context-role vectors for directional relationships.
///
/// # Intent Role
///
/// - "What is this text trying to accomplish?"
/// - Used for queries like "What does X want?" or "What is the goal?"
/// - Captures action-oriented semantics
///
/// # Context Role
///
/// - "What contextual relationships does this text establish?"
/// - Used for queries like "What relates to X?" or "What is the background?"
/// - Captures relationship-oriented semantics
///
/// # Why Perturbed Identity?
///
/// 1. **Immediate asymmetry**: Different random perturbations create distinct
///    projections from the start
/// 2. **Preserved semantics**: Identity component ensures the base meaning is retained
/// 3. **No training required**: Works out-of-the-box without fine-tuning
/// 4. **Deterministic**: Same seed produces same weights across runs
#[deprecated(
    since = "0.2.0",
    note = "E5-base-v2 uses prefix-based asymmetry. See model.rs embed_dual()."
)]
#[derive(Debug)]
pub struct ContextProjectionWeights {
    /// Intent projection matrix: [768, 768]
    pub intent_projection: Tensor,
    /// Intent projection bias: [768]
    pub intent_bias: Tensor,
    /// Context projection matrix: [768, 768]
    pub context_projection: Tensor,
    /// Context projection bias: [768]
    pub context_bias: Tensor,
    /// Hidden size for validation (768 for MPNet)
    #[allow(dead_code)]
    pub hidden_size: usize,
}

impl ContextProjectionWeights {
    /// Initialize projection weights as perturbed identity matrices.
    ///
    /// Creates W_intent = I + N(0, 0.02) and W_context = I + N(0, 0.02) with
    /// different random perturbations to create asymmetry.
    ///
    /// # Arguments
    ///
    /// * `hidden_size` - Dimension of embeddings (768 for MPNet)
    /// * `device` - Device to create tensors on
    /// * `seed` - Random seed for reproducibility
    ///
    /// # Errors
    ///
    /// Returns `EmbeddingError::InvalidDimension` if hidden_size != 768.
    /// Returns `EmbeddingError::GpuError` if tensor creation fails.
    pub fn initialize(
        hidden_size: usize,
        device: &Device,
        seed: u64,
    ) -> EmbeddingResult<Self> {
        if hidden_size != CONTEXTUAL_DIMENSION {
            return Err(EmbeddingError::InvalidDimension {
                expected: CONTEXTUAL_DIMENSION,
                actual: hidden_size,
            });
        }

        // Use seeded RNG for reproducibility
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);

        // Create perturbed identity for intent projection
        let intent_data = create_perturbed_identity(hidden_size, &mut rng, PROJECTION_INIT_STD);
        let intent_projection =
            Tensor::from_slice(&intent_data, (hidden_size, hidden_size), device)
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("Failed to create context intent projection: {}", e),
                })?
                .to_dtype(DType::F32)
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("Failed to convert context intent projection dtype: {}", e),
                })?;

        // Create perturbed identity for context projection (different perturbation)
        let context_data = create_perturbed_identity(hidden_size, &mut rng, PROJECTION_INIT_STD);
        let context_projection =
            Tensor::from_slice(&context_data, (hidden_size, hidden_size), device)
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("Failed to create context context projection: {}", e),
                })?
                .to_dtype(DType::F32)
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("Failed to convert context context projection dtype: {}", e),
                })?;

        // Initialize biases to small random values (not zero, to add asymmetry)
        let intent_bias_data: Vec<f32> = (0..hidden_size)
            .map(|_| rng.gen_range(-0.01f32..0.01f32))
            .collect();
        let intent_bias =
            Tensor::from_slice(&intent_bias_data, hidden_size, device).map_err(|e| {
                EmbeddingError::GpuError {
                    message: format!("Failed to create context intent bias: {}", e),
                }
            })?;

        let context_bias_data: Vec<f32> = (0..hidden_size)
            .map(|_| rng.gen_range(-0.01f32..0.01f32))
            .collect();
        let context_bias =
            Tensor::from_slice(&context_bias_data, hidden_size, device).map_err(|e| {
                EmbeddingError::GpuError {
                    message: format!("Failed to create context context bias: {}", e),
                }
            })?;

        tracing::debug!(
            "ContextProjectionWeights initialized: {}x{} perturbed identity matrices",
            hidden_size,
            hidden_size
        );

        Ok(Self {
            intent_projection,
            intent_bias,
            context_projection,
            context_bias,
            hidden_size,
        })
    }

    /// Apply intent projection to an embedding.
    ///
    /// Computes: intent_vec = base_embedding @ W_intent^T + b_intent
    ///
    /// Use this when embedding text to capture "what the text wants to accomplish".
    ///
    /// # Arguments
    ///
    /// * `embedding` - Input embedding tensor [1, 768]
    ///
    /// # Returns
    ///
    /// Projected intent embedding [1, 768]
    pub fn project_intent(&self, embedding: &Tensor) -> EmbeddingResult<Tensor> {
        let projected = embedding
            .matmul(
                &self
                    .intent_projection
                    .t()
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("Context intent projection transpose failed: {}", e),
                    })?,
            )
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Context intent projection matmul failed: {}", e),
            })?;

        projected
            .broadcast_add(&self.intent_bias)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Context intent projection bias add failed: {}", e),
            })
    }

    /// Apply context projection to an embedding.
    ///
    /// Computes: context_vec = base_embedding @ W_context^T + b_context
    ///
    /// Use this when embedding text to capture "what contextual relationships it establishes".
    ///
    /// # Arguments
    ///
    /// * `embedding` - Input embedding tensor [1, 768]
    ///
    /// # Returns
    ///
    /// Projected context embedding [1, 768]
    pub fn project_context(&self, embedding: &Tensor) -> EmbeddingResult<Tensor> {
        let projected = embedding
            .matmul(
                &self
                    .context_projection
                    .t()
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("Context context projection transpose failed: {}", e),
                    })?,
            )
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Context context projection matmul failed: {}", e),
            })?;

        projected
            .broadcast_add(&self.context_bias)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Context context projection bias add failed: {}", e),
            })
    }

    /// Project an embedding to both intent and context vectors in one pass.
    ///
    /// Efficient method that applies both projections at once.
    ///
    /// # Arguments
    ///
    /// * `embedding` - Input embedding tensor [1, 768]
    ///
    /// # Returns
    ///
    /// Tuple of (intent_vec, context_vec), each [1, 768]
    pub fn project_dual(&self, embedding: &Tensor) -> EmbeddingResult<(Tensor, Tensor)> {
        let intent_vec = self.project_intent(embedding)?;
        let context_vec = self.project_context(embedding)?;
        Ok((intent_vec, context_vec))
    }
}

/// Create a perturbed identity matrix: I + N(0, std)
///
/// # Arguments
///
/// * `size` - Matrix dimension (e.g., 768 for MPNet)
/// * `rng` - Random number generator
/// * `std` - Standard deviation for perturbation (typically 0.02)
///
/// # Returns
///
/// Flat vector of size*size elements representing the matrix row-major.
fn create_perturbed_identity<R: Rng>(size: usize, rng: &mut R, std: f64) -> Vec<f32> {
    let mut data = vec![0.0f32; size * size];

    for i in 0..size {
        for j in 0..size {
            let idx = i * size + j;
            // Identity component
            let identity: f32 = if i == j { 1.0 } else { 0.0 };
            // Random perturbation from normal distribution
            // Using Box-Muller transform for normal distribution
            let u1: f64 = rng.gen_range(0.0001f64..1.0f64);
            let u2: f64 = rng.gen_range(0.0f64..1.0f64);
            let normal: f64 = (-2.0_f64 * u1.ln()).sqrt() * (2.0_f64 * std::f64::consts::PI * u2).cos();
            let perturbation = (normal * std) as f32;

            data[idx] = identity + perturbation;
        }
    }

    data
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;

    #[test]
    fn test_perturbed_identity_diagonal_dominant() {
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let data = create_perturbed_identity(4, &mut rng, 0.02);

        // Check diagonal elements are close to 1.0
        for i in 0..4 {
            let diag = data[i * 4 + i];
            assert!(
                (diag - 1.0).abs() < 0.1,
                "Diagonal element {} should be close to 1.0, got {}",
                i,
                diag
            );
        }

        // Check off-diagonal elements are close to 0.0
        for i in 0..4 {
            for j in 0..4 {
                if i != j {
                    let off_diag = data[i * 4 + j];
                    assert!(
                        off_diag.abs() < 0.1,
                        "Off-diagonal element ({},{}) should be close to 0.0, got {}",
                        i,
                        j,
                        off_diag
                    );
                }
            }
        }
        println!("[PASS] Perturbed identity is diagonal-dominant");
    }

    #[test]
    fn test_perturbed_identity_different_seeds_different_matrices() {
        use rand::SeedableRng;
        let mut rng1 = rand::rngs::StdRng::seed_from_u64(42);
        let mut rng2 = rand::rngs::StdRng::seed_from_u64(43);

        let data1 = create_perturbed_identity(4, &mut rng1, 0.02);
        let data2 = create_perturbed_identity(4, &mut rng2, 0.02);

        // Matrices should be different
        let mut different = false;
        for (a, b) in data1.iter().zip(data2.iter()) {
            if (a - b).abs() > 1e-6 {
                different = true;
                break;
            }
        }
        assert!(different, "Different seeds should produce different matrices");
        println!("[PASS] Different seeds produce different matrices");
    }

    #[test]
    fn test_perturbed_identity_same_seed_same_matrix() {
        use rand::SeedableRng;
        let mut rng1 = rand::rngs::StdRng::seed_from_u64(42);
        let mut rng2 = rand::rngs::StdRng::seed_from_u64(42);

        let data1 = create_perturbed_identity(4, &mut rng1, 0.02);
        let data2 = create_perturbed_identity(4, &mut rng2, 0.02);

        // Matrices should be identical
        for (a, b) in data1.iter().zip(data2.iter()) {
            assert!(
                (a - b).abs() < 1e-10,
                "Same seed should produce identical matrices"
            );
        }
        println!("[PASS] Same seed produces identical matrices (deterministic)");
    }

    #[test]
    fn test_projection_seed_is_unique() {
        // Ensure E10's seed is different from E5's and E8's
        const E5_CAUSAL_SEED: u64 = 0xCA05A1;
        const E8_GRAPH_SEED: u64 = 0xE8_6FA9;

        assert_ne!(
            CONTEXT_PROJECTION_SEED, E5_CAUSAL_SEED,
            "E10 context projection seed must be different from E5 causal projection seed"
        );
        assert_ne!(
            CONTEXT_PROJECTION_SEED, E8_GRAPH_SEED,
            "E10 context projection seed must be different from E8 graph projection seed"
        );
        println!(
            "[PASS] E10 projection seed is unique: 0x{:X} != 0x{:X} != 0x{:X}",
            CONTEXT_PROJECTION_SEED, E5_CAUSAL_SEED, E8_GRAPH_SEED
        );
    }
}

//! Graph Projection Weights for Asymmetric Embeddings.
//!
//! Following the E5 Causal pattern (ARCH-15), this module provides learned
//! projection matrices for creating asymmetric source/target embeddings.
//!
//! # Architecture
//!
//! The projections transform the base MiniLM embedding into source-role
//! and target-role vectors. They are initialized as perturbed identity
//! matrices (I + N(0, 0.02)) to create immediate asymmetry without training.
//!
//! ```text
//! base_embedding [384D]
//!        |
//!    +---+---+
//!    |       |
//! W_source  W_target
//!    |       |
//! source_vec target_vec
//! [384D]     [384D]
//! ```
//!
//! # Reference
//!
//! - E5 Causal projections: `causal/weights.rs`
//! - E8 upgrade specification: `docs/e8upgrade.md`

use candle_core::{DType, Device, Tensor};
use rand::Rng;

use crate::error::{EmbeddingError, EmbeddingResult};

use super::constants::GRAPH_DIMENSION;

/// Seed for graph projection initialization (deterministic).
/// Different from E5's seed to ensure distinct projections.
pub const GRAPH_PROJECTION_SEED: u64 = 0xE8_6FA9;

/// Standard deviation for initializing projection weight perturbations.
const PROJECTION_INIT_STD: f64 = 0.02;

/// Learned projection weights for asymmetric source/target embeddings.
///
/// These projections transform the base MiniLM embedding into
/// source-role and target-role vectors for directional relationships.
///
/// # Source Role
///
/// - "This memory points TO others"
/// - Used for queries like "What does X import/use/depend on?"
///
/// # Target Role
///
/// - "This memory is pointed TO by others"
/// - Used for queries like "What imports/uses/depends on X?"
///
/// # Why Perturbed Identity?
///
/// 1. **Immediate asymmetry**: Different random perturbations create distinct
///    projections from the start
/// 2. **Preserved semantics**: Identity component ensures the base meaning is retained
/// 3. **No training required**: Works out-of-the-box without fine-tuning
/// 4. **Deterministic**: Same seed produces same weights across runs
#[derive(Debug)]
pub struct GraphProjectionWeights {
    /// Source projection matrix: [384, 384]
    pub source_projection: Tensor,
    /// Source projection bias: [384]
    pub source_bias: Tensor,
    /// Target projection matrix: [384, 384]
    pub target_projection: Tensor,
    /// Target projection bias: [384]
    pub target_bias: Tensor,
    /// Hidden size for validation (384 for MiniLM)
    pub hidden_size: usize,
}

impl GraphProjectionWeights {
    /// Initialize projection weights as perturbed identity matrices.
    ///
    /// Creates W_source = I + N(0, 0.02) and W_target = I + N(0, 0.02) with
    /// different random perturbations to create asymmetry.
    ///
    /// # Arguments
    ///
    /// * `hidden_size` - Dimension of embeddings (384 for MiniLM)
    /// * `device` - Device to create tensors on
    /// * `seed` - Random seed for reproducibility
    ///
    /// # Errors
    ///
    /// Returns `EmbeddingError::GpuError` if tensor creation fails.
    pub fn initialize(
        hidden_size: usize,
        device: &Device,
        seed: u64,
    ) -> EmbeddingResult<Self> {
        if hidden_size != GRAPH_DIMENSION {
            return Err(EmbeddingError::InvalidDimension {
                expected: GRAPH_DIMENSION,
                actual: hidden_size,
            });
        }

        // Use seeded RNG for reproducibility
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);

        // Create perturbed identity for source projection
        let source_data = create_perturbed_identity(hidden_size, &mut rng, PROJECTION_INIT_STD);
        let source_projection =
            Tensor::from_slice(&source_data, (hidden_size, hidden_size), device)
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("Failed to create graph source projection: {}", e),
                })?
                .to_dtype(DType::F32)
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("Failed to convert graph source projection dtype: {}", e),
                })?;

        // Create perturbed identity for target projection (different perturbation)
        let target_data = create_perturbed_identity(hidden_size, &mut rng, PROJECTION_INIT_STD);
        let target_projection =
            Tensor::from_slice(&target_data, (hidden_size, hidden_size), device)
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("Failed to create graph target projection: {}", e),
                })?
                .to_dtype(DType::F32)
                .map_err(|e| EmbeddingError::GpuError {
                    message: format!("Failed to convert graph target projection dtype: {}", e),
                })?;

        // Initialize biases to small random values (not zero, to add asymmetry)
        let source_bias_data: Vec<f32> = (0..hidden_size)
            .map(|_| rng.gen_range(-0.01f32..0.01f32))
            .collect();
        let source_bias =
            Tensor::from_slice(&source_bias_data, hidden_size, device).map_err(|e| {
                EmbeddingError::GpuError {
                    message: format!("Failed to create graph source bias: {}", e),
                }
            })?;

        let target_bias_data: Vec<f32> = (0..hidden_size)
            .map(|_| rng.gen_range(-0.01f32..0.01f32))
            .collect();
        let target_bias =
            Tensor::from_slice(&target_bias_data, hidden_size, device).map_err(|e| {
                EmbeddingError::GpuError {
                    message: format!("Failed to create graph target bias: {}", e),
                }
            })?;

        tracing::debug!(
            "GraphProjectionWeights initialized: {}x{} perturbed identity matrices",
            hidden_size,
            hidden_size
        );

        Ok(Self {
            source_projection,
            source_bias,
            target_projection,
            target_bias,
            hidden_size,
        })
    }

    /// Apply source projection to an embedding.
    ///
    /// Computes: source_vec = base_embedding @ W_source^T + b_source
    ///
    /// Use this when embedding text that "points to" other entities
    /// (e.g., "Module A imports B, C, D").
    ///
    /// # Arguments
    ///
    /// * `embedding` - Input embedding tensor [1, 384]
    ///
    /// # Returns
    ///
    /// Projected source embedding [1, 384]
    pub fn project_source(&self, embedding: &Tensor) -> EmbeddingResult<Tensor> {
        let projected = embedding
            .matmul(
                &self
                    .source_projection
                    .t()
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("Graph source projection transpose failed: {}", e),
                    })?,
            )
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Graph source projection matmul failed: {}", e),
            })?;

        projected
            .broadcast_add(&self.source_bias)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Graph source projection bias add failed: {}", e),
            })
    }

    /// Apply target projection to an embedding.
    ///
    /// Computes: target_vec = base_embedding @ W_target^T + b_target
    ///
    /// Use this when embedding text that "is pointed to" by other entities
    /// (e.g., "Module X is imported by A, B, C").
    ///
    /// # Arguments
    ///
    /// * `embedding` - Input embedding tensor [1, 384]
    ///
    /// # Returns
    ///
    /// Projected target embedding [1, 384]
    pub fn project_target(&self, embedding: &Tensor) -> EmbeddingResult<Tensor> {
        let projected = embedding
            .matmul(
                &self
                    .target_projection
                    .t()
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("Graph target projection transpose failed: {}", e),
                    })?,
            )
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Graph target projection matmul failed: {}", e),
            })?;

        projected
            .broadcast_add(&self.target_bias)
            .map_err(|e| EmbeddingError::GpuError {
                message: format!("Graph target projection bias add failed: {}", e),
            })
    }

    /// Project an embedding to both source and target vectors in one pass.
    ///
    /// Efficient method that applies both projections at once.
    ///
    /// # Arguments
    ///
    /// * `embedding` - Input embedding tensor [1, 384]
    ///
    /// # Returns
    ///
    /// Tuple of (source_vec, target_vec), each [1, 384]
    pub fn project_dual(&self, embedding: &Tensor) -> EmbeddingResult<(Tensor, Tensor)> {
        let source_vec = self.project_source(embedding)?;
        let target_vec = self.project_target(embedding)?;
        Ok((source_vec, target_vec))
    }
}

/// Create a perturbed identity matrix: I + N(0, std)
///
/// # Arguments
///
/// * `size` - Matrix dimension (e.g., 384 for MiniLM)
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
        // Ensure E8's seed is different from E5's
        // E5 CAUSAL_PROJECTION_SEED = 0xCA05A1
        const E5_CAUSAL_SEED: u64 = 0xCA05A1;
        assert_ne!(
            GRAPH_PROJECTION_SEED, E5_CAUSAL_SEED,
            "E8 graph projection seed must be different from E5 causal projection seed"
        );
        println!("[PASS] E8 projection seed is unique from E5: 0x{:X} != 0x{:X}", GRAPH_PROJECTION_SEED, E5_CAUSAL_SEED);
    }
}

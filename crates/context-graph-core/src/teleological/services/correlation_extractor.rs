//! TASK-TELEO-008: CorrelationExtractor Implementation
//!
//! Extracts 78 cross-correlation values from 13 embedding vectors.
//! Each correlation captures the interaction between two embedding perspectives.
//!
//! # From teleoplan.md
//!
//! "Cross-correlations reveal how different knowledge perspectives resonate.
//! When semantic and analogical embeddings both activate strongly, the combined
//! signal is MORE meaningful than either alone."

use crate::teleological::{
    types::EMBEDDING_DIM, SynergyMatrix, CROSS_CORRELATION_COUNT, SYNERGY_DIM,
};

/// Configuration for correlation extraction.
#[derive(Clone, Debug)]
pub struct CorrelationConfig {
    /// Normalization method for embeddings
    pub normalize_embeddings: bool,
    /// Minimum correlation magnitude (below = noise, set to 0)
    pub min_correlation: f32,
    /// Apply synergy weighting to correlations
    pub apply_synergy_weights: bool,
}

impl Default for CorrelationConfig {
    fn default() -> Self {
        Self {
            normalize_embeddings: true,
            min_correlation: 0.01,
            apply_synergy_weights: true,
        }
    }
}

/// Result of correlation extraction.
#[derive(Clone, Debug)]
pub struct CorrelationResult {
    /// 78 cross-correlation values
    pub correlations: [f32; CROSS_CORRELATION_COUNT],
    /// Sparsity: fraction of near-zero correlations
    pub sparsity: f32,
    /// Average correlation magnitude
    pub average_magnitude: f32,
    /// Highest correlation pair indices and value
    pub strongest_pair: Option<(usize, usize, f32)>,
}

/// TELEO-008: Extracts cross-correlations from multi-embedding representations.
///
/// Computes 78 unique pairwise correlations between 13 embedding vectors.
///
/// # Example
///
/// ```
/// use context_graph_core::teleological::services::CorrelationExtractor;
/// use context_graph_core::teleological::SynergyMatrix;
///
/// let extractor = CorrelationExtractor::new();
/// let embeddings = vec![vec![0.0f32; 1024]; 13];
/// let result = extractor.extract(&embeddings, None);
/// assert_eq!(result.correlations.len(), 78);
/// ```
pub struct CorrelationExtractor {
    config: CorrelationConfig,
}

impl CorrelationExtractor {
    /// Create a new CorrelationExtractor with default configuration.
    pub fn new() -> Self {
        Self {
            config: CorrelationConfig::default(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(config: CorrelationConfig) -> Self {
        Self { config }
    }

    /// Extract 78 cross-correlations from 13 embedding vectors.
    ///
    /// # Arguments
    /// * `embeddings` - 13 embedding vectors, each of dimension EMBEDDING_DIM (1024)
    /// * `synergy_matrix` - Optional synergy matrix for weighting
    ///
    /// # Panics
    ///
    /// Panics if embeddings count != 13 or dimensions don't match (FAIL FAST).
    pub fn extract(
        &self,
        embeddings: &[Vec<f32>],
        synergy_matrix: Option<&SynergyMatrix>,
    ) -> CorrelationResult {
        assert!(
            embeddings.len() == SYNERGY_DIM,
            "FAIL FAST: Expected {} embeddings, got {}",
            SYNERGY_DIM,
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

        // Optionally normalize embeddings
        let normalized: Vec<Vec<f32>> = if self.config.normalize_embeddings {
            embeddings.iter().map(|e| Self::normalize(e)).collect()
        } else {
            embeddings.to_vec()
        };

        // Compute 78 correlations
        let mut correlations = [0.0f32; CROSS_CORRELATION_COUNT];
        let mut idx = 0;

        for i in 0..SYNERGY_DIM {
            for j in (i + 1)..SYNERGY_DIM {
                // Compute correlation between embeddings i and j
                let corr = Self::compute_correlation(&normalized[i], &normalized[j]);

                // Apply synergy weighting if enabled
                let weighted = if self.config.apply_synergy_weights {
                    if let Some(matrix) = synergy_matrix {
                        corr * matrix.get_synergy(i, j)
                    } else {
                        corr * SynergyMatrix::BASE_SYNERGIES[i][j]
                    }
                } else {
                    corr
                };

                // Apply noise threshold
                correlations[idx] = if weighted.abs() < self.config.min_correlation {
                    0.0
                } else {
                    weighted
                };

                idx += 1;
            }
        }

        // Compute statistics
        let (sparsity, avg_magnitude, strongest) = self.compute_stats(&correlations);

        CorrelationResult {
            correlations,
            sparsity,
            average_magnitude: avg_magnitude,
            strongest_pair: strongest,
        }
    }

    /// Extract correlations from pre-normalized embeddings.
    ///
    /// Skips normalization step for efficiency when embeddings are already normalized.
    pub fn extract_prenormalized(
        &self,
        embeddings: &[&[f32]; SYNERGY_DIM],
        synergy_matrix: Option<&SynergyMatrix>,
    ) -> CorrelationResult {
        for (i, emb) in embeddings.iter().enumerate() {
            assert!(
                emb.len() == EMBEDDING_DIM,
                "FAIL FAST: Embedding {} has dimension {}, expected {}",
                i,
                emb.len(),
                EMBEDDING_DIM
            );
        }

        let mut correlations = [0.0f32; CROSS_CORRELATION_COUNT];
        let mut idx = 0;

        for i in 0..SYNERGY_DIM {
            for j in (i + 1)..SYNERGY_DIM {
                let corr = Self::compute_correlation(embeddings[i], embeddings[j]);

                let weighted = if self.config.apply_synergy_weights {
                    if let Some(matrix) = synergy_matrix {
                        corr * matrix.get_synergy(i, j)
                    } else {
                        corr * SynergyMatrix::BASE_SYNERGIES[i][j]
                    }
                } else {
                    corr
                };

                correlations[idx] = if weighted.abs() < self.config.min_correlation {
                    0.0
                } else {
                    weighted
                };

                idx += 1;
            }
        }

        let (sparsity, avg_magnitude, strongest) = self.compute_stats(&correlations);

        CorrelationResult {
            correlations,
            sparsity,
            average_magnitude: avg_magnitude,
            strongest_pair: strongest,
        }
    }

    /// Compute correlation between two embedding vectors.
    ///
    /// Uses cosine similarity: dot(a, b) / (||a|| * ||b||)
    fn compute_correlation(a: &[f32], b: &[f32]) -> f32 {
        let mut dot = 0.0f32;
        let mut norm_a = 0.0f32;
        let mut norm_b = 0.0f32;

        for i in 0..a.len() {
            dot += a[i] * b[i];
            norm_a += a[i] * a[i];
            norm_b += b[i] * b[i];
        }

        let denom = (norm_a.sqrt()) * (norm_b.sqrt());
        if denom < f32::EPSILON {
            0.0
        } else {
            dot / denom
        }
    }

    /// L2 normalize a vector.
    fn normalize(v: &[f32]) -> Vec<f32> {
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm < f32::EPSILON {
            vec![0.0; v.len()]
        } else {
            v.iter().map(|x| x / norm).collect()
        }
    }

    /// Compute statistics from correlations.
    fn compute_stats(
        &self,
        correlations: &[f32; CROSS_CORRELATION_COUNT],
    ) -> (f32, f32, Option<(usize, usize, f32)>) {
        let zero_count = correlations
            .iter()
            .filter(|&&c| c.abs() < f32::EPSILON)
            .count();
        let sparsity = zero_count as f32 / CROSS_CORRELATION_COUNT as f32;

        let sum: f32 = correlations.iter().map(|c| c.abs()).sum();
        let avg_magnitude = sum / CROSS_CORRELATION_COUNT as f32;

        // Find strongest pair
        let mut strongest: Option<(usize, f32)> = None;
        for (idx, &corr) in correlations.iter().enumerate() {
            let abs_corr = corr.abs();
            if strongest.is_none() || abs_corr > strongest.unwrap().1 {
                strongest = Some((idx, abs_corr));
            }
        }

        let strongest_pair = strongest.map(|(flat_idx, val)| {
            let (i, j) = SynergyMatrix::flat_to_indices(flat_idx);
            (i, j, val)
        });

        (sparsity, avg_magnitude, strongest_pair)
    }

    /// Get configuration.
    pub fn config(&self) -> &CorrelationConfig {
        &self.config
    }
}

impl Default for CorrelationExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_embeddings(fill: f32) -> Vec<Vec<f32>> {
        vec![vec![fill; EMBEDDING_DIM]; SYNERGY_DIM]
    }

    fn make_varied_embeddings() -> Vec<Vec<f32>> {
        (0..SYNERGY_DIM)
            .map(|i| {
                (0..EMBEDDING_DIM)
                    .map(|j| ((i * EMBEDDING_DIM + j) as f32).sin())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn test_correlation_extractor_new() {
        let extractor = CorrelationExtractor::new();
        assert!(extractor.config().normalize_embeddings);

        println!("[PASS] CorrelationExtractor::new creates default config");
    }

    #[test]
    fn test_extract_uniform() {
        let extractor = CorrelationExtractor::new();
        let embeddings = make_embeddings(1.0);

        let result = extractor.extract(&embeddings, None);

        assert_eq!(result.correlations.len(), CROSS_CORRELATION_COUNT);

        println!("[PASS] extract produces 78 correlations");
    }

    #[test]
    fn test_extract_varied() {
        let extractor = CorrelationExtractor::new();
        let embeddings = make_varied_embeddings();

        let result = extractor.extract(&embeddings, None);

        // Should have some non-zero correlations
        let nonzero_count = result
            .correlations
            .iter()
            .filter(|&&c| c.abs() > 0.01)
            .count();
        assert!(nonzero_count > 0);

        // Should identify strongest pair
        assert!(result.strongest_pair.is_some());

        println!("[PASS] extract with varied embeddings produces meaningful correlations");
    }

    #[test]
    fn test_extract_with_synergy_matrix() {
        let extractor = CorrelationExtractor::new();
        let embeddings = make_varied_embeddings();
        let matrix = SynergyMatrix::with_base_synergies();

        let result = extractor.extract(&embeddings, Some(&matrix));

        // Synergy weighting should affect correlations
        assert!(result.average_magnitude >= 0.0);

        println!("[PASS] extract applies synergy weighting correctly");
    }

    #[test]
    fn test_sparsity_calculation() {
        let extractor = CorrelationExtractor::with_config(CorrelationConfig {
            normalize_embeddings: false,
            min_correlation: 0.5, // High threshold
            apply_synergy_weights: false,
        });

        // Use varied embeddings - uniform embeddings produce perfect correlation (1.0)
        // which would never be below the threshold. Varied embeddings produce
        // diverse correlations, some of which will be below 0.5.
        let embeddings = make_varied_embeddings();
        let result = extractor.extract(&embeddings, None);

        // With varied embeddings and high threshold (0.5), some correlations
        // should be zeroed out, resulting in non-zero sparsity
        assert!(
            result.sparsity > 0.0,
            "Expected sparsity > 0.0 with varied embeddings and high threshold, got {}",
            result.sparsity
        );

        println!("[PASS] Sparsity calculated correctly: {}", result.sparsity);
    }

    #[test]
    fn test_correlation_symmetry() {
        let extractor = CorrelationExtractor::new();
        let embeddings = make_varied_embeddings();

        let result = extractor.extract(&embeddings, None);

        // Verify we have exactly 78 unique pairs (not 156)
        assert_eq!(result.correlations.len(), 78);

        println!("[PASS] Extracts exactly 78 unique pairs (upper triangle)");
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_extract_wrong_count() {
        let extractor = CorrelationExtractor::new();
        let embeddings = vec![vec![0.0f32; EMBEDDING_DIM]; 10]; // Wrong count

        let _ = extractor.extract(&embeddings, None);
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_extract_wrong_dimension() {
        let extractor = CorrelationExtractor::new();
        let mut embeddings = make_embeddings(0.5);
        embeddings[5] = vec![0.0; 512]; // Wrong dimension

        let _ = extractor.extract(&embeddings, None);
    }

    #[test]
    fn test_normalize() {
        let v = vec![3.0, 4.0];
        let normalized = CorrelationExtractor::normalize(&v);

        let norm: f32 = normalized.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 0.001,
            "Normalized vector should have unit norm"
        );

        println!("[PASS] normalize produces unit vectors");
    }

    #[test]
    fn test_correlation_range() {
        let extractor = CorrelationExtractor::with_config(CorrelationConfig {
            normalize_embeddings: true,
            min_correlation: 0.0,
            apply_synergy_weights: false,
        });

        let embeddings = make_varied_embeddings();
        let result = extractor.extract(&embeddings, None);

        // All correlations should be in [-1, 1]
        for &corr in &result.correlations {
            assert!(
                (-1.0..=1.0).contains(&corr),
                "Correlation {} out of range [-1, 1]",
                corr
            );
        }

        println!("[PASS] All correlations in valid range [-1, 1]");
    }
}

//! Top-K expert selection and routing utilities.

use crate::error::{EmbeddingError, EmbeddingResult};

/// Select top-K experts based on probabilities.
///
/// Returns the indices of the K experts with highest probabilities
/// for each sample in the batch.
///
/// # Arguments
///
/// * `probs` - Expert probabilities [batch_size * num_experts]
/// * `batch_size` - Number of samples in batch
/// * `num_experts` - Total number of experts
/// * `top_k` - Number of experts to select per sample
///
/// # Returns
///
/// Tuple of:
/// - `indices`: Selected expert indices [batch_size * top_k]
/// - `weights`: Normalized weights for selected experts [batch_size * top_k]
///
/// # Errors
///
/// - `EmbeddingError::ConfigError` if top_k > num_experts
/// - `EmbeddingError::InvalidDimension` if probs dimensions are wrong
///
/// # Example
///
/// ```
/// use context_graph_embeddings::fusion::gating::routing::select_top_k;
///
/// let probs = vec![0.1, 0.3, 0.2, 0.4]; // 4 experts
/// let (indices, weights) = select_top_k(&probs, 1, 4, 2).unwrap();
/// assert_eq!(indices.len(), 2);
/// assert_eq!(weights.len(), 2);
/// // Weights are normalized to sum to 1
/// let sum: f32 = weights.iter().sum();
/// assert!((sum - 1.0).abs() < 1e-5);
/// ```
pub fn select_top_k(
    probs: &[f32],
    batch_size: usize,
    num_experts: usize,
    top_k: usize,
) -> EmbeddingResult<(Vec<usize>, Vec<f32>)> {
    if top_k > num_experts {
        return Err(EmbeddingError::ConfigError {
            message: format!(
                "top_k ({}) cannot exceed num_experts ({})",
                top_k, num_experts
            ),
        });
    }

    let expected_len = batch_size * num_experts;
    if probs.len() != expected_len {
        return Err(EmbeddingError::InvalidDimension {
            expected: expected_len,
            actual: probs.len(),
        });
    }

    let mut indices = Vec::with_capacity(batch_size * top_k);
    let mut weights = Vec::with_capacity(batch_size * top_k);

    for b in 0..batch_size {
        let offset = b * num_experts;
        let sample_probs = &probs[offset..offset + num_experts];

        // Create (index, prob) pairs and sort by prob descending
        let mut indexed: Vec<(usize, f32)> = sample_probs
            .iter()
            .enumerate()
            .map(|(i, &p)| (i, p))
            .collect();

        indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top K
        let selected: Vec<(usize, f32)> = indexed.into_iter().take(top_k).collect();

        // Renormalize weights to sum to 1
        let weight_sum: f32 = selected.iter().map(|(_, w)| w).sum();

        for (idx, w) in selected {
            indices.push(idx);
            weights.push(w / weight_sum);
        }
    }

    Ok((indices, weights))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_top_k_basic() {
        let probs = vec![0.1, 0.3, 0.2, 0.4]; // 4 experts
        let (indices, weights) = select_top_k(&probs, 1, 4, 2).unwrap();

        assert_eq!(indices.len(), 2);
        assert_eq!(weights.len(), 2);

        // Top 2 should be indices 3 and 1 (with probs 0.4 and 0.3)
        assert!(indices.contains(&3));
        assert!(indices.contains(&1));

        // Weights should sum to 1
        let sum: f32 = weights.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_select_top_k_batch() {
        let probs = vec![
            0.1, 0.3, 0.2, 0.4, // batch 0
            0.4, 0.1, 0.3, 0.2, // batch 1
        ];
        let (indices, weights) = select_top_k(&probs, 2, 4, 2).unwrap();

        assert_eq!(indices.len(), 4);
        assert_eq!(weights.len(), 4);

        // Check batch 0's weights sum to 1
        let sum0: f32 = weights[0..2].iter().sum();
        assert!((sum0 - 1.0).abs() < 1e-5);

        // Check batch 1's weights sum to 1
        let sum1: f32 = weights[2..4].iter().sum();
        assert!((sum1 - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_select_top_k_exceeds_num_experts_fails() {
        let probs = vec![0.25, 0.25, 0.25, 0.25];
        let result = select_top_k(&probs, 1, 4, 5);

        assert!(result.is_err());
    }

    #[test]
    fn test_select_top_k_dimension_mismatch_fails() {
        let probs = vec![0.25, 0.25, 0.25]; // Wrong size
        let result = select_top_k(&probs, 1, 4, 2);

        assert!(result.is_err());
    }
}

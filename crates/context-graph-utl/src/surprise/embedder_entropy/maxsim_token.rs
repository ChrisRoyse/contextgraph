//! MaxSim token-level entropy for E12 (LateInteraction/ColBERT) embeddings.
//!
//! Formula: ΔS = 1 - avg(top-k MaxSim scores)
//! Per constitution.yaml delta_methods.ΔS E12: "Token KNN"
//!
//! # Algorithm
//!
//! Late interaction embeddings (ColBERT) represent queries and documents as
//! variable-length sequences of per-token vectors (128D each). MaxSim finds,
//! for each query token, its maximum cosine similarity with any document token,
//! then averages these scores.
//!
//! MaxSim(Q, D) = (1/|Q|) × Σᵢ max_j cos(qᵢ, dⱼ)
//!
//! 1. **Tokenization:**
//!    - Reshape current embedding into tokens (chunks of 128D)
//!    - Length must be divisible by 128, else return empty (fail fast)
//!
//! 2. **MaxSim Computation:**
//!    - For each history embedding:
//!      a. Reshape into tokens
//!      b. For each current token, find max similarity to any history token
//!      c. Average the per-token max similarities = MaxSim score
//!
//! 3. **Entropy Calculation:**
//!    - Sort MaxSim scores, take top-k
//!    - ΔS = 1 - avg(top-k MaxSim scores), clamped to [0, 1]
//!
//! # Constitution Reference
//!
//! - From constitution.yaml delta_methods.ΔS E12: "Token KNN"
//! - E12 dimension: 128D per token (ColBERT standard)
//! - embeddings.E12_LateInteraction: { dim: "128D/tok", type: dense_per_token }

use super::EmbedderEntropy;
use crate::config::SurpriseConfig;
use crate::error::{UtlError, UtlResult};
use context_graph_core::teleological::Embedder;

/// E12 (LateInteraction) per-token dimension per constitution.yaml.
const E12_TOKEN_DIM: usize = 128;

/// Default minimum tokens required for valid embedding.
const DEFAULT_MIN_TOKENS: usize = 1;

/// Default k neighbors for averaging.
const DEFAULT_K_NEIGHBORS: usize = 5;

/// E12 (LateInteraction) entropy using ColBERT-style MaxSim aggregation.
///
/// Late interaction embeddings are variable-length sequences of per-token
/// vectors (128D each). MaxSim finds, for each query token, its maximum
/// cosine similarity with any document token, then averages these scores.
///
/// # Algorithm
///
/// 1. Reshape current embedding into tokens (chunks of 128D)
/// 2. For each history embedding:
///    a. Reshape into tokens
///    b. For each current token, find max similarity to any history token
///    c. Average the per-token max similarities = MaxSim score
/// 3. ΔS = 1 - avg(top-k MaxSim scores), clamped to [0, 1]
///
/// # Token Representation
///
/// Embeddings are stored as flattened Vec<f32> with length = num_tokens * 128.
/// - 128 elements = 1 token
/// - 256 elements = 2 tokens
/// - Invalid: 257 elements -> not evenly divisible -> return error
///
/// # Constitution Reference
/// E12: "Token KNN" (constitution.yaml delta_methods.ΔS)
#[derive(Debug, Clone)]
pub struct MaxSimTokenEntropy {
    /// Per-token embedding dimension. MUST be 128 (ColBERT standard).
    token_dim: usize,
    /// Minimum token count to consider valid. Default: 1.
    min_tokens: usize,
    /// Running mean for score normalization.
    running_mean: f32,
    /// Running variance for score normalization.
    running_variance: f32,
    /// Sample count for statistics.
    sample_count: usize,
    /// k neighbors for top-k averaging.
    k_neighbors: usize,
}

impl Default for MaxSimTokenEntropy {
    fn default() -> Self {
        Self::new()
    }
}

impl MaxSimTokenEntropy {
    /// Create with constitution defaults (token_dim=128, min_tokens=1, k=5).
    pub fn new() -> Self {
        Self {
            token_dim: E12_TOKEN_DIM,
            min_tokens: DEFAULT_MIN_TOKENS,
            running_mean: 0.5,
            running_variance: 0.1,
            sample_count: 0,
            k_neighbors: DEFAULT_K_NEIGHBORS,
        }
    }

    /// Create with specific token dimension.
    pub fn with_token_dim(token_dim: usize) -> Self {
        Self {
            token_dim: token_dim.max(1),
            ..Self::new()
        }
    }

    /// Create from SurpriseConfig.
    pub fn from_config(config: &SurpriseConfig) -> Self {
        let token_dim = config.late_interaction_token_dim.clamp(64, 256);
        let min_tokens = config.late_interaction_min_tokens.clamp(1, 10);
        let k = config.late_interaction_k_neighbors.clamp(1, 20);

        Self {
            token_dim,
            min_tokens,
            running_mean: 0.5,
            running_variance: 0.1,
            sample_count: 0,
            k_neighbors: k,
        }
    }

    /// Builder: set minimum token count.
    #[must_use]
    pub fn with_min_tokens(mut self, min_tokens: usize) -> Self {
        self.min_tokens = min_tokens.max(1);
        self
    }

    /// Builder: set k neighbors.
    #[must_use]
    pub fn with_k_neighbors(mut self, k: usize) -> Self {
        self.k_neighbors = k.clamp(1, 20);
        self
    }

    /// Reshape flat embedding into token slices.
    /// Returns empty Vec if length not divisible by token_dim.
    #[inline]
    fn tokenize<'a>(&self, embedding: &'a [f32]) -> Vec<&'a [f32]> {
        let token_dim = self.token_dim;
        if embedding.len() % token_dim != 0 {
            return vec![]; // FAIL FAST: invalid length
        }
        embedding.chunks_exact(token_dim).collect()
    }

    /// Compute cosine similarity between two token vectors.
    ///
    /// # Arguments
    /// * `a` - First token vector
    /// * `b` - Second token vector
    ///
    /// # Returns
    /// Cosine similarity in [-1, 1], or 0 if vectors have zero magnitude.
    fn token_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        let min_len = a.len().min(b.len());
        if min_len == 0 {
            return 0.0;
        }

        let mut dot = 0.0f32;
        let mut norm_a = 0.0f32;
        let mut norm_b = 0.0f32;

        for i in 0..min_len {
            dot += a[i] * b[i];
            norm_a += a[i] * a[i];
            norm_b += b[i] * b[i];
        }

        let magnitude = (norm_a * norm_b).sqrt();
        if magnitude < 1e-10 {
            return 0.0; // Avoid division by zero
        }

        (dot / magnitude).clamp(-1.0, 1.0)
    }

    /// Compute MaxSim score between two tokenized embeddings.
    ///
    /// MaxSim(Q, D) = (1/|Q|) × Σᵢ max_j cos(qᵢ, dⱼ)
    ///
    /// For each query token, find maximum similarity to any document token,
    /// then average across all query tokens.
    ///
    /// # Arguments
    /// * `query_tokens` - Query tokens (current embedding tokenized)
    /// * `doc_tokens` - Document tokens (history embedding tokenized)
    ///
    /// # Returns
    /// MaxSim score in [0, 1] (shifted from [-1, 1] to positive range)
    fn compute_maxsim(&self, query_tokens: &[&[f32]], doc_tokens: &[&[f32]]) -> f32 {
        if query_tokens.is_empty() || doc_tokens.is_empty() {
            return 0.0;
        }

        let mut sum_max_sim = 0.0f32;

        for q_token in query_tokens {
            // Find maximum similarity between this query token and any document token
            let max_sim = doc_tokens
                .iter()
                .map(|d_token| self.token_similarity(q_token, d_token))
                .fold(f32::NEG_INFINITY, f32::max);

            // Shift from [-1, 1] to [0, 1] for aggregation
            // MaxSim typically uses raw cosine, but we need [0,1] output
            let normalized_sim = (max_sim + 1.0) / 2.0;
            sum_max_sim += normalized_sim;
        }

        sum_max_sim / query_tokens.len() as f32
    }
}

impl EmbedderEntropy for MaxSimTokenEntropy {
    fn compute_delta_s(
        &self,
        current: &[f32],
        history: &[Vec<f32>],
        k: usize,
    ) -> UtlResult<f32> {
        // Validate input: empty current is an error per spec
        if current.is_empty() {
            return Err(UtlError::EmptyInput);
        }

        // Check for NaN/Infinity in current embedding (AP-10)
        for &v in current {
            if v.is_nan() || v.is_infinite() {
                return Err(UtlError::EntropyError(
                    "Invalid value (NaN/Infinity) in current embedding".to_string(),
                ));
            }
        }

        // Empty history = maximum surprise
        if history.is_empty() {
            return Ok(1.0);
        }

        // Tokenize current embedding
        let query_tokens = self.tokenize(current);

        // Check minimum tokens requirement
        if query_tokens.len() < self.min_tokens {
            // Not enough tokens - return max surprise (novel/unexpected)
            return Ok(1.0);
        }

        // Use provided k or fallback to configured k_neighbors
        let k_to_use = if k > 0 { k } else { self.k_neighbors };

        // Compute MaxSim scores to all valid history embeddings
        let mut maxsim_scores: Vec<f32> = history
            .iter()
            .filter(|h| !h.is_empty())
            .filter(|h| {
                // Validate history embeddings for NaN/Infinity
                h.iter().all(|v| !v.is_nan() && !v.is_infinite())
            })
            .filter_map(|h| {
                let doc_tokens = self.tokenize(h);
                if doc_tokens.is_empty() {
                    return None; // Invalid tokenization
                }
                Some(self.compute_maxsim(&query_tokens, &doc_tokens))
            })
            .filter(|s| !s.is_nan() && !s.is_infinite())
            .collect();

        // If all history was invalid, return max surprise
        if maxsim_scores.is_empty() {
            return Ok(1.0);
        }

        // Sort by highest MaxSim score (descending) and take top-k
        maxsim_scores.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        let k_actual = k_to_use.min(maxsim_scores.len()).max(1);
        let k_best = &maxsim_scores[..k_actual];

        // Compute mean of top-k MaxSim scores
        let mean_maxsim: f32 = k_best.iter().sum::<f32>() / k_actual as f32;

        // ΔS = 1 - mean_maxsim
        // High MaxSim = similar content = low surprise (ΔS near 0)
        // Low MaxSim = novel content = high surprise (ΔS near 1)
        let delta_s = 1.0 - mean_maxsim;

        // Final validation per AP-10: no NaN/Infinity
        let clamped = delta_s.clamp(0.0, 1.0);
        if clamped.is_nan() || clamped.is_infinite() {
            return Err(UtlError::EntropyError(
                "Computed delta_s is NaN or Infinity - violates AP-10".to_string(),
            ));
        }

        Ok(clamped)
    }

    fn embedder_type(&self) -> Embedder {
        Embedder::LateInteraction
    }

    fn reset(&mut self) {
        self.running_mean = 0.5;
        self.running_variance = 0.1;
        self.sample_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Core Functionality Tests ===

    #[test]
    fn test_maxsim_empty_history_returns_one() {
        let calculator = MaxSimTokenEntropy::new();
        let current = vec![0.5f32; E12_TOKEN_DIM]; // 1 token
        let history: Vec<Vec<f32>> = vec![];

        println!("BEFORE: history.len() = 0");
        let result = calculator.compute_delta_s(&current, &history, 5);
        assert!(result.is_ok(), "Should not error on empty history");
        assert_eq!(result.unwrap(), 1.0, "Empty history should return 1.0");
        println!("AFTER: delta_s = 1.0");
        println!("[PASS] test_maxsim_empty_history_returns_one - delta_s = 1.0");
    }

    #[test]
    fn test_maxsim_identical_returns_near_zero() {
        let calculator = MaxSimTokenEntropy::new();

        // Identical embeddings should have MaxSim = 1.0, ΔS = 0.0
        let current = vec![0.5f32; E12_TOKEN_DIM * 2]; // 2 tokens
        let history: Vec<Vec<f32>> = vec![vec![0.5f32; E12_TOKEN_DIM * 2]; 10];

        println!("BEFORE: current == history[i] for all i");
        let result = calculator.compute_delta_s(&current, &history, 5);
        assert!(result.is_ok());
        let delta_s = result.unwrap();

        println!("AFTER: delta_s = {} (expected < 0.5 for identical)", delta_s);
        assert!(
            delta_s < 0.5,
            "Identical tokens should have low surprise, got {}",
            delta_s
        );
        println!(
            "[PASS] test_maxsim_identical_returns_near_zero - delta_s = {} < 0.5",
            delta_s
        );
    }

    #[test]
    fn test_maxsim_orthogonal_returns_moderate() {
        let calculator = MaxSimTokenEntropy::new();

        // Create orthogonal token vectors
        // Orthogonal vectors have cosine = 0, shifted to [0,1] = 0.5
        // MaxSim = 0.5, so ΔS = 1 - 0.5 = 0.5
        let mut current = vec![0.0f32; E12_TOKEN_DIM];
        current[0..64].fill(1.0); // First half non-zero

        let mut hist_item = vec![0.0f32; E12_TOKEN_DIM];
        hist_item[64..128].fill(1.0); // Second half non-zero
        let history = vec![hist_item];

        println!("BEFORE: current orthogonal to history (dot product ≈ 0)");
        let result = calculator.compute_delta_s(&current, &history, 5);
        assert!(result.is_ok());
        let delta_s = result.unwrap();

        println!(
            "AFTER: delta_s = {} (expected ≈ 0.5 for orthogonal)",
            delta_s
        );
        // Orthogonal vectors: cosine=0, shifted=(0+1)/2=0.5, MaxSim=0.5, ΔS=1-0.5=0.5
        assert!(
            (0.45..=0.55).contains(&delta_s),
            "Orthogonal tokens should have moderate surprise ~0.5, got {}",
            delta_s
        );
        println!(
            "[PASS] test_maxsim_orthogonal_returns_moderate - delta_s = {} ≈ 0.5",
            delta_s
        );
    }

    #[test]
    fn test_maxsim_partial_overlap() {
        let calculator = MaxSimTokenEntropy::new();

        // 2 tokens: query token 1 matches history token 1 perfectly
        //           query token 2 is orthogonal to all history tokens
        // Create query: token1 = [1,0,0,...], token2 = [0,1,0,...]
        let mut current = vec![0.0f32; E12_TOKEN_DIM * 2];
        current[0] = 1.0; // token 1: [1,0,0,...]
        current[E12_TOKEN_DIM + 1] = 1.0; // token 2: [0,1,0,...]

        // Create history: token1 = [1,0,0,...], token2 = [0,0,1,...]
        let mut history_item = vec![0.0f32; E12_TOKEN_DIM * 2];
        history_item[0] = 1.0; // token 1: matches query token 1
        history_item[E12_TOKEN_DIM + 2] = 1.0; // token 2: orthogonal to query token 2
        let history = vec![history_item];

        // Expected:
        // - Query token 1 max-matches history token 1: cosine = 1.0, shifted = 1.0
        // - Query token 2 max-matches nothing well: cosine ≈ 0, shifted = 0.5
        // - MaxSim = (1.0 + 0.5) / 2 = 0.75
        // - ΔS = 1 - 0.75 = 0.25

        println!("BEFORE: one token matches perfectly, one is orthogonal");
        let result = calculator.compute_delta_s(&current, &history, 5);
        assert!(result.is_ok());
        let delta_s = result.unwrap();

        println!(
            "AFTER: delta_s = {} (expected ≈ 0.25 for partial match)",
            delta_s
        );
        // Partial match: some tokens match, some don't
        assert!(
            (0.1..0.5).contains(&delta_s),
            "Partial overlap should have low-to-mid surprise (some match), got {}",
            delta_s
        );
        println!(
            "[PASS] test_maxsim_partial_overlap - delta_s = {} ∈ (0.1, 0.5)",
            delta_s
        );
    }

    #[test]
    fn test_maxsim_empty_input_error() {
        let calculator = MaxSimTokenEntropy::new();
        let empty: Vec<f32> = vec![];
        let history = vec![vec![0.5f32; E12_TOKEN_DIM]];

        let result = calculator.compute_delta_s(&empty, &history, 5);
        assert!(
            matches!(result, Err(UtlError::EmptyInput)),
            "Empty input should return EmptyInput error"
        );
        println!("[PASS] test_maxsim_empty_input_error - Err(EmptyInput)");
    }

    #[test]
    fn test_maxsim_embedder_type() {
        let calculator = MaxSimTokenEntropy::new();
        assert_eq!(
            calculator.embedder_type(),
            Embedder::LateInteraction,
            "Should return Embedder::LateInteraction"
        );
        println!("[PASS] test_maxsim_embedder_type - Embedder::LateInteraction");
    }

    #[test]
    fn test_maxsim_valid_range() {
        let calculator = MaxSimTokenEntropy::new();

        // Test various input patterns
        for pattern in 0..5 {
            let current: Vec<f32> = (0..E12_TOKEN_DIM * 2)
                .map(|i| ((i + pattern * 100) as f32) / (E12_TOKEN_DIM * 2) as f32)
                .collect();

            let history: Vec<Vec<f32>> = (0..15)
                .map(|j| {
                    (0..E12_TOKEN_DIM * 2)
                        .map(|i| ((i + j * 50) as f32) / (E12_TOKEN_DIM * 2) as f32)
                        .collect()
                })
                .collect();

            let result = calculator.compute_delta_s(&current, &history, 5);
            assert!(result.is_ok());
            let delta_s = result.unwrap();

            assert!(
                (0.0..=1.0).contains(&delta_s),
                "Pattern {} delta_s {} out of range [0.0, 1.0]",
                pattern,
                delta_s
            );
        }

        println!("[PASS] test_maxsim_valid_range - All outputs in [0.0, 1.0]");
    }

    #[test]
    fn test_maxsim_no_nan_infinity() {
        let calculator = MaxSimTokenEntropy::new();

        // Edge case: very small values
        let small: Vec<f32> = vec![1e-10; E12_TOKEN_DIM];
        let history: Vec<Vec<f32>> = vec![vec![1e-10; E12_TOKEN_DIM]; 10];

        let result = calculator.compute_delta_s(&small, &history, 5);
        assert!(result.is_ok());
        let delta_s = result.unwrap();
        assert!(!delta_s.is_nan(), "delta_s should not be NaN (AP-10)");
        assert!(
            !delta_s.is_infinite(),
            "delta_s should not be Infinite (AP-10)"
        );

        // Edge case: values near 1
        let near_one: Vec<f32> = vec![0.9999; E12_TOKEN_DIM];
        let result2 = calculator.compute_delta_s(&near_one, &history, 5);
        assert!(result2.is_ok());
        let delta_s2 = result2.unwrap();
        assert!(!delta_s2.is_nan(), "delta_s should not be NaN");
        assert!(!delta_s2.is_infinite(), "delta_s should not be Infinite");

        // Edge case: mixed positive and negative
        let mixed: Vec<f32> = (0..E12_TOKEN_DIM)
            .map(|i| if i % 2 == 0 { 0.5 } else { -0.5 })
            .collect();
        let result3 = calculator.compute_delta_s(&mixed, &history, 5);
        assert!(result3.is_ok());
        let delta_s3 = result3.unwrap();
        assert!(
            !delta_s3.is_nan(),
            "delta_s should not be NaN for mixed values"
        );

        println!("[PASS] test_maxsim_no_nan_infinity - AP-10 compliant");
    }

    // === Token Handling Tests ===

    #[test]
    fn test_maxsim_variable_length_query() {
        let calculator = MaxSimTokenEntropy::new();

        // 2 tokens query vs 5 tokens history
        let current = vec![0.5f32; E12_TOKEN_DIM * 2];
        let history = vec![vec![0.5f32; E12_TOKEN_DIM * 5]; 5];

        let result = calculator.compute_delta_s(&current, &history, 5);
        assert!(result.is_ok(), "Should handle variable length query");
        let delta_s = result.unwrap();
        assert!((0.0..=1.0).contains(&delta_s));

        println!(
            "[PASS] test_maxsim_variable_length_query - delta_s = {}",
            delta_s
        );
    }

    #[test]
    fn test_maxsim_variable_length_doc() {
        let calculator = MaxSimTokenEntropy::new();

        // 5 tokens query vs 2 tokens history
        let current = vec![0.5f32; E12_TOKEN_DIM * 5];
        let history = vec![vec![0.5f32; E12_TOKEN_DIM * 2]; 5];

        let result = calculator.compute_delta_s(&current, &history, 5);
        assert!(result.is_ok(), "Should handle variable length doc");
        let delta_s = result.unwrap();
        assert!((0.0..=1.0).contains(&delta_s));

        println!(
            "[PASS] test_maxsim_variable_length_doc - delta_s = {}",
            delta_s
        );
    }

    #[test]
    fn test_maxsim_single_token() {
        let calculator = MaxSimTokenEntropy::new();

        // 1 token each - degenerates to cosine similarity
        let current = vec![0.5f32; E12_TOKEN_DIM];
        let history = vec![vec![0.5f32; E12_TOKEN_DIM]; 10];

        let result = calculator.compute_delta_s(&current, &history, 5);
        assert!(result.is_ok(), "Should handle single token");
        let delta_s = result.unwrap();
        assert!((0.0..=1.0).contains(&delta_s));

        // For identical single tokens, MaxSim = cosine = 1.0, so ΔS should be low
        assert!(
            delta_s < 0.5,
            "Single identical token should give low surprise"
        );

        println!(
            "[PASS] test_maxsim_single_token - delta_s = {} (degenerates to cosine)",
            delta_s
        );
    }

    #[test]
    fn test_maxsim_tokenize_valid() {
        let calculator = MaxSimTokenEntropy::new();
        let embedding = vec![0.5f32; 256]; // 2 tokens

        let tokens = calculator.tokenize(&embedding);
        assert_eq!(tokens.len(), 2, "Should produce 2 tokens from 256 elements");
        assert_eq!(tokens[0].len(), 128, "Each token should be 128D");
        assert_eq!(tokens[1].len(), 128, "Each token should be 128D");

        println!("[PASS] test_maxsim_tokenize_valid - 256 elements → 2 tokens");
    }

    #[test]
    fn test_maxsim_tokenize_invalid() {
        let calculator = MaxSimTokenEntropy::new();
        let embedding = vec![0.5f32; 257]; // NOT divisible by 128

        let tokens = calculator.tokenize(&embedding);
        assert!(
            tokens.is_empty(),
            "Should return empty for invalid length {}",
            embedding.len()
        );

        println!("[PASS] test_maxsim_tokenize_invalid - 257 elements → empty vec");
    }

    // === Configuration Tests ===

    #[test]
    fn test_maxsim_from_config() {
        let mut config = SurpriseConfig::default();
        config.late_interaction_token_dim = 128;
        config.late_interaction_min_tokens = 2;
        config.late_interaction_k_neighbors = 10;

        let calculator = MaxSimTokenEntropy::from_config(&config);

        assert_eq!(
            calculator.token_dim, 128,
            "token_dim should be 128 from config"
        );
        assert_eq!(
            calculator.min_tokens, 2,
            "min_tokens should be 2 from config"
        );
        assert_eq!(
            calculator.k_neighbors, 10,
            "k_neighbors should be 10 from config"
        );

        println!(
            "[PASS] test_maxsim_from_config - token_dim={}, min_tokens={}, k={}",
            calculator.token_dim, calculator.min_tokens, calculator.k_neighbors
        );
    }

    #[test]
    fn test_maxsim_reset() {
        let mut calculator = MaxSimTokenEntropy::new();

        // Modify internal state
        calculator.running_mean = 0.8;
        calculator.running_variance = 0.5;
        calculator.sample_count = 100;

        calculator.reset();

        assert_eq!(
            calculator.running_mean, 0.5,
            "running_mean should reset to 0.5"
        );
        assert_eq!(
            calculator.running_variance, 0.1,
            "running_variance should reset to 0.1"
        );
        assert_eq!(
            calculator.sample_count, 0,
            "sample_count should reset to 0"
        );

        println!("[PASS] test_maxsim_reset - State cleared properly");
    }

    // === Error Handling Tests ===

    #[test]
    fn test_maxsim_nan_input_error() {
        let calculator = MaxSimTokenEntropy::new();
        let mut current = vec![0.5f32; E12_TOKEN_DIM];
        current[0] = f32::NAN;

        let history = vec![vec![0.5f32; E12_TOKEN_DIM]];
        let result = calculator.compute_delta_s(&current, &history, 5);

        assert!(result.is_err(), "Should error on NaN input");
        assert!(
            matches!(result, Err(UtlError::EntropyError(_))),
            "Should return EntropyError for NaN input"
        );

        println!("[PASS] test_maxsim_nan_input_error - Err(EntropyError)");
    }

    #[test]
    fn test_maxsim_infinity_input_error() {
        let calculator = MaxSimTokenEntropy::new();
        let mut current = vec![0.5f32; E12_TOKEN_DIM];
        current[0] = f32::INFINITY;

        let history = vec![vec![0.5f32; E12_TOKEN_DIM]];
        let result = calculator.compute_delta_s(&current, &history, 5);

        assert!(result.is_err(), "Should error on Infinity input");
        assert!(
            matches!(result, Err(UtlError::EntropyError(_))),
            "Should return EntropyError for Infinity input"
        );

        println!("[PASS] test_maxsim_infinity_input_error - Err(EntropyError)");
    }

    // === Edge Case Tests per Task Requirements ===

    #[test]
    fn test_edge_case_empty_history() {
        let calculator = MaxSimTokenEntropy::new();
        let current = vec![0.5f32; E12_TOKEN_DIM]; // 1 token
        let history: Vec<Vec<f32>> = vec![];

        println!("BEFORE: history.len() = {}", history.len());
        let result = calculator.compute_delta_s(&current, &history, 5);
        println!("AFTER: result = {:?}", result);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1.0);

        println!("[PASS] test_edge_case_empty_history");
    }

    #[test]
    fn test_edge_case_max_tokens() {
        let calculator = MaxSimTokenEntropy::new();
        let current = vec![0.5f32; E12_TOKEN_DIM * 100]; // 100 tokens
        let history = vec![vec![0.5f32; E12_TOKEN_DIM * 100]; 10];

        println!("BEFORE: current_tokens=100, history_embeddings=10");
        let result = calculator.compute_delta_s(&current, &history, 5);
        println!("AFTER: delta_s = {:?}", result);

        assert!(result.is_ok());
        let delta_s = result.unwrap();
        assert!((0.0..=1.0).contains(&delta_s));

        println!(
            "[PASS] test_edge_case_max_tokens - delta_s = {} in valid range",
            delta_s
        );
    }

    #[test]
    fn test_edge_case_invalid_token_length() {
        let calculator = MaxSimTokenEntropy::new();
        let current = vec![0.5f32; 257]; // NOT divisible by 128
        let history = vec![vec![0.5f32; E12_TOKEN_DIM]];

        println!("BEFORE: current.len()={} (invalid)", current.len());
        let result = calculator.compute_delta_s(&current, &history, 5);
        println!("AFTER: result = {:?}", result);

        // Should return max surprise (1.0) because tokenization fails
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            1.0,
            "Invalid token length should return max surprise"
        );

        println!("[PASS] test_edge_case_invalid_token_length - returns 1.0 (max surprise)");
    }

    #[test]
    fn test_edge_case_large_values() {
        let calculator = MaxSimTokenEntropy::new();
        let current = vec![f32::MAX / 1000.0; E12_TOKEN_DIM];
        let history = vec![vec![f32::MAX / 1000.0; E12_TOKEN_DIM]; 5];

        println!("BEFORE: large values");
        let result = calculator.compute_delta_s(&current, &history, 5);
        println!("AFTER: delta_s = {:?}, must not be NaN/Inf", result);

        assert!(result.is_ok(), "Should handle large values");
        let delta_s = result.unwrap();
        assert!(!delta_s.is_nan(), "delta_s must not be NaN");
        assert!(!delta_s.is_infinite(), "delta_s must not be Infinite");

        println!("[PASS] test_edge_case_large_values");
    }

    // === Builder Tests ===

    #[test]
    fn test_builder_with_token_dim() {
        let calc = MaxSimTokenEntropy::with_token_dim(64);
        assert_eq!(calc.token_dim, 64, "Token dim should be 64");

        println!("[PASS] test_builder_with_token_dim");
    }

    #[test]
    fn test_builder_with_min_tokens() {
        let calc = MaxSimTokenEntropy::new().with_min_tokens(3);
        assert_eq!(calc.min_tokens, 3, "min_tokens should be 3");

        // Test clamping
        let calc2 = MaxSimTokenEntropy::new().with_min_tokens(0);
        assert_eq!(calc2.min_tokens, 1, "min_tokens should be clamped to 1");

        println!("[PASS] test_builder_with_min_tokens");
    }

    #[test]
    fn test_builder_with_k_neighbors() {
        let calc = MaxSimTokenEntropy::new().with_k_neighbors(7);
        assert_eq!(calc.k_neighbors, 7, "k_neighbors should be 7");

        // Test clamping
        let calc2 = MaxSimTokenEntropy::new().with_k_neighbors(0);
        assert_eq!(calc2.k_neighbors, 1, "k_neighbors should be clamped to 1");

        let calc3 = MaxSimTokenEntropy::new().with_k_neighbors(100);
        assert_eq!(calc3.k_neighbors, 20, "k_neighbors should be clamped to 20");

        println!("[PASS] test_builder_with_k_neighbors");
    }

    // === History Edge Cases ===

    #[test]
    fn test_single_history_item() {
        let calculator = MaxSimTokenEntropy::new();

        let current = vec![0.5f32; E12_TOKEN_DIM];
        let history = vec![vec![0.5f32; E12_TOKEN_DIM]]; // Only 1 item, but k=5

        println!("BEFORE: history.len()={}, k=5", history.len());

        let result = calculator.compute_delta_s(&current, &history, 5);
        assert!(result.is_ok(), "Should handle k > history.len()");
        let delta_s = result.unwrap();

        println!("AFTER: delta_s = {}", delta_s);
        assert!(
            (0.0..=1.0).contains(&delta_s),
            "delta_s should be in valid range"
        );

        println!("[PASS] test_single_history_item - delta_s = {}", delta_s);
    }

    #[test]
    fn test_history_with_empty_embeddings() {
        let calculator = MaxSimTokenEntropy::new();

        let current = vec![0.5f32; E12_TOKEN_DIM];
        let mut history: Vec<Vec<f32>> = vec![vec![0.6f32; E12_TOKEN_DIM]; 5];
        history.push(vec![]); // Add empty embedding
        history.push(vec![]); // Add another empty

        let result = calculator.compute_delta_s(&current, &history, 5);
        assert!(result.is_ok(), "Should filter out empty history embeddings");
        let delta_s = result.unwrap();
        assert!((0.0..=1.0).contains(&delta_s));

        println!(
            "[PASS] test_history_with_empty_embeddings - delta_s = {}",
            delta_s
        );
    }

    #[test]
    fn test_history_with_nan_values() {
        let calculator = MaxSimTokenEntropy::new();

        let current = vec![0.5f32; E12_TOKEN_DIM];
        let mut history: Vec<Vec<f32>> = vec![vec![0.6f32; E12_TOKEN_DIM]; 5];

        // Add embedding with NaN - should be filtered
        let mut nan_embedding = vec![0.5f32; E12_TOKEN_DIM];
        nan_embedding[0] = f32::NAN;
        history.push(nan_embedding);

        let result = calculator.compute_delta_s(&current, &history, 5);
        assert!(result.is_ok(), "Should filter out history with NaN values");
        let delta_s = result.unwrap();
        assert!((0.0..=1.0).contains(&delta_s));

        println!(
            "[PASS] test_history_with_nan_values - delta_s = {}",
            delta_s
        );
    }

    #[test]
    fn test_history_with_invalid_tokenization() {
        let calculator = MaxSimTokenEntropy::new();

        let current = vec![0.5f32; E12_TOKEN_DIM * 2]; // 2 valid tokens
        let mut history: Vec<Vec<f32>> = vec![vec![0.5f32; E12_TOKEN_DIM * 2]; 5];

        // Add embedding that can't be tokenized (not divisible by 128)
        history.push(vec![0.5f32; 257]);

        let result = calculator.compute_delta_s(&current, &history, 5);
        assert!(result.is_ok(), "Should filter out invalid tokenization");
        let delta_s = result.unwrap();
        assert!((0.0..=1.0).contains(&delta_s));

        println!(
            "[PASS] test_history_with_invalid_tokenization - delta_s = {}",
            delta_s
        );
    }

    // === Synthetic Test Data Verification ===

    #[test]
    fn test_synthetic_identical_tokens() {
        let calculator = MaxSimTokenEntropy::new();

        // Identical tokens: current == history
        // Expected: MaxSim = 1.0, ΔS = 0.0
        let current = vec![0.5f32; E12_TOKEN_DIM]; // 1 token, all 0.5
        let history = vec![vec![0.5f32; E12_TOKEN_DIM]; 10]; // 10 identical embeddings

        let result = calculator.compute_delta_s(&current, &history, 5);
        assert!(result.is_ok());
        let delta_s = result.unwrap();

        println!(
            "[VERIFY] Identical tokens: delta_s = {} (expected ≈ 0)",
            delta_s
        );
        assert!(
            delta_s < 0.1,
            "Identical tokens should give delta_s near 0, got {}",
            delta_s
        );
    }

    #[test]
    fn test_synthetic_orthogonal_tokens() {
        let calculator = MaxSimTokenEntropy::new();

        // Orthogonal tokens
        // current: [1.0; 64] ++ [0.0; 64]
        // history: [0.0; 64] ++ [1.0; 64]
        // cosine(orthogonal) = 0, shifted to 0.5, MaxSim = 0.5, ΔS = 0.5
        let mut current = vec![0.0f32; E12_TOKEN_DIM];
        current[0..64].fill(1.0); // First half non-zero

        let mut hist_item = vec![0.0f32; E12_TOKEN_DIM];
        hist_item[64..128].fill(1.0); // Second half non-zero
        let history = vec![hist_item];

        let result = calculator.compute_delta_s(&current, &history, 5);
        assert!(result.is_ok());
        let delta_s = result.unwrap();

        println!(
            "[VERIFY] Orthogonal tokens: delta_s = {} (expected ≈ 0.5)",
            delta_s
        );
        assert!(
            (0.4..0.6).contains(&delta_s),
            "Orthogonal tokens should give delta_s near 0.5, got {}",
            delta_s
        );
    }

    #[test]
    fn test_synthetic_multi_token_partial_match() {
        let calculator = MaxSimTokenEntropy::new();

        // Multi-token with partial match:
        // Query: token1 = [1,0,0,...], token2 = [0,1,0,...]
        // History: token1 = [1,0,0,...] (perfect match), token2 = [0,0,1,...] (orthogonal)
        let mut current = vec![0.0f32; E12_TOKEN_DIM * 2];
        current[0] = 1.0; // token 1: unit vector in dim 0
        current[E12_TOKEN_DIM + 1] = 1.0; // token 2: unit vector in dim 1

        let mut history_item = vec![0.0f32; E12_TOKEN_DIM * 2];
        history_item[0] = 1.0; // token 1: matches query token 1 perfectly
        history_item[E12_TOKEN_DIM + 2] = 1.0; // token 2: orthogonal to query token 2
        let history = vec![history_item];

        // Query token 1 finds perfect match (cosine=1, shifted=1)
        // Query token 2 max match is orthogonal (cosine=0, shifted=0.5)
        // MaxSim = (1.0 + 0.5) / 2 = 0.75
        // ΔS = 1 - 0.75 = 0.25

        let result = calculator.compute_delta_s(&current, &history, 5);
        assert!(result.is_ok());
        let delta_s = result.unwrap();

        println!(
            "[VERIFY] Partial match: delta_s = {} (expected ≈ 0.25)",
            delta_s
        );
        assert!(
            (0.1..0.5).contains(&delta_s),
            "Partial match should give low-mid delta_s, got {}",
            delta_s
        );
    }

    // === Token Similarity Unit Test ===

    #[test]
    fn test_token_similarity_calculation() {
        let calculator = MaxSimTokenEntropy::new();

        // Identical vectors: similarity = 1.0
        let a = vec![0.5f32; 128];
        let b = vec![0.5f32; 128];
        let sim = calculator.token_similarity(&a, &b);
        assert!(
            (sim - 1.0).abs() < 0.001,
            "Identical vectors should have similarity 1.0, got {}",
            sim
        );

        // Opposite vectors: similarity = -1.0
        let c = vec![1.0f32; 128];
        let d = vec![-1.0f32; 128];
        let sim2 = calculator.token_similarity(&c, &d);
        assert!(
            (sim2 - (-1.0)).abs() < 0.001,
            "Opposite vectors should have similarity -1.0, got {}",
            sim2
        );

        // Orthogonal vectors: similarity = 0.0
        let mut e = vec![0.0f32; 128];
        e[0] = 1.0;
        let mut f = vec![0.0f32; 128];
        f[1] = 1.0;
        let sim3 = calculator.token_similarity(&e, &f);
        assert!(
            sim3.abs() < 0.001,
            "Orthogonal vectors should have similarity 0.0, got {}",
            sim3
        );

        println!("[PASS] test_token_similarity_calculation");
    }

    // === MaxSim Computation Unit Test ===

    #[test]
    fn test_maxsim_computation() {
        let calculator = MaxSimTokenEntropy::new();

        // Query: 2 identical tokens
        let q1 = vec![0.5f32; 128];
        let q2 = vec![0.5f32; 128];
        let query_tokens: Vec<&[f32]> = vec![&q1, &q2];

        // Doc: 2 identical tokens (same as query)
        let d1 = vec![0.5f32; 128];
        let d2 = vec![0.5f32; 128];
        let doc_tokens: Vec<&[f32]> = vec![&d1, &d2];

        let maxsim = calculator.compute_maxsim(&query_tokens, &doc_tokens);

        // Each query token has max similarity 1.0 to matching doc token
        // Shifted: (1.0 + 1.0) / 2 = 1.0 per token, avg = 1.0
        println!(
            "[VERIFY] MaxSim computation: {} (expected 1.0 for identical)",
            maxsim
        );
        assert!(
            (maxsim - 1.0).abs() < 0.001,
            "MaxSim should be 1.0 for identical tokens"
        );

        println!("[PASS] test_maxsim_computation");
    }
}

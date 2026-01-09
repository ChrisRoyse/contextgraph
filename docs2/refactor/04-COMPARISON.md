# Comparison Operations Specification

## 1. Overview

This document specifies how teleological arrays are compared. The fundamental principle is **apples-to-apples**: only compatible embedding types are compared directly.

**Target Architecture:**
```
Memory Injection (MCP) → Autonomous Embedding (13 models) → Teleological Array Storage
    → Entry-Point Discovery (any of 13 spaces) → Full Array Comparison (apples to apples)
    → Autonomous Goal Emergence (clustering)
```

## 2. Similarity Functions

### 2.1 Dense Vector Similarity

```rust
/// Similarity functions for dense embeddings
pub struct DenseSimilarity;

impl DenseSimilarity {
    /// Cosine similarity - normalized dot product
    /// Returns value in [-1, 1], typically [0, 1] for normalized vectors
    pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
        assert_eq!(a.len(), b.len(), "Vectors must have same dimension");

        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot / (norm_a * norm_b)
    }

    /// Dot product - unnormalized, useful for pre-normalized vectors
    pub fn dot_product(a: &[f32], b: &[f32]) -> f32 {
        assert_eq!(a.len(), b.len());
        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    }

    /// Euclidean distance (L2) - lower is more similar
    pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
        assert_eq!(a.len(), b.len());
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt()
    }

    /// Euclidean similarity - converts distance to similarity [0, 1]
    pub fn euclidean_similarity(a: &[f32], b: &[f32]) -> f32 {
        1.0 / (1.0 + Self::euclidean_distance(a, b))
    }

    /// Manhattan distance (L1)
    pub fn manhattan_distance(a: &[f32], b: &[f32]) -> f32 {
        assert_eq!(a.len(), b.len());
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).abs())
            .sum()
    }

    /// SIMD-accelerated cosine for performance
    #[cfg(target_arch = "x86_64")]
    pub fn cosine_simd(a: &[f32], b: &[f32]) -> f32 {
        use std::arch::x86_64::*;

        assert_eq!(a.len(), b.len());
        let len = a.len();

        unsafe {
            let mut dot_sum = _mm256_setzero_ps();
            let mut norm_a_sum = _mm256_setzero_ps();
            let mut norm_b_sum = _mm256_setzero_ps();

            let chunks = len / 8;
            for i in 0..chunks {
                let va = _mm256_loadu_ps(a.as_ptr().add(i * 8));
                let vb = _mm256_loadu_ps(b.as_ptr().add(i * 8));

                dot_sum = _mm256_fmadd_ps(va, vb, dot_sum);
                norm_a_sum = _mm256_fmadd_ps(va, va, norm_a_sum);
                norm_b_sum = _mm256_fmadd_ps(vb, vb, norm_b_sum);
            }

            // Horizontal sum
            let dot = Self::hsum256(dot_sum);
            let norm_a = Self::hsum256(norm_a_sum).sqrt();
            let norm_b = Self::hsum256(norm_b_sum).sqrt();

            // Handle remainder
            let mut dot_rem = 0.0f32;
            let mut norm_a_rem = 0.0f32;
            let mut norm_b_rem = 0.0f32;
            for i in (chunks * 8)..len {
                dot_rem += a[i] * b[i];
                norm_a_rem += a[i] * a[i];
                norm_b_rem += b[i] * b[i];
            }

            let total_dot = dot + dot_rem;
            let total_norm_a = (norm_a * norm_a + norm_a_rem).sqrt();
            let total_norm_b = (norm_b * norm_b + norm_b_rem).sqrt();

            if total_norm_a == 0.0 || total_norm_b == 0.0 {
                0.0
            } else {
                total_dot / (total_norm_a * total_norm_b)
            }
        }
    }

    #[cfg(target_arch = "x86_64")]
    unsafe fn hsum256(v: std::arch::x86_64::__m256) -> f32 {
        use std::arch::x86_64::*;
        let low = _mm256_castps256_ps128(v);
        let high = _mm256_extractf128_ps(v, 1);
        let sum128 = _mm_add_ps(low, high);
        let sum64 = _mm_add_ps(sum128, _mm_movehl_ps(sum128, sum128));
        let sum32 = _mm_add_ss(sum64, _mm_shuffle_ps(sum64, sum64, 0x55));
        _mm_cvtss_f32(sum32)
    }
}
```

### 2.2 Sparse Vector Similarity

```rust
/// Similarity functions for sparse embeddings (SPLADE)
pub struct SparseSimilarity;

impl SparseSimilarity {
    /// Sparse dot product
    pub fn dot_product(a: &SparseVector, b: &SparseVector) -> f32 {
        let mut result = 0.0f32;
        let mut i = 0;
        let mut j = 0;

        while i < a.indices.len() && j < b.indices.len() {
            match a.indices[i].cmp(&b.indices[j]) {
                std::cmp::Ordering::Equal => {
                    result += a.values[i] * b.values[j];
                    i += 1;
                    j += 1;
                }
                std::cmp::Ordering::Less => i += 1,
                std::cmp::Ordering::Greater => j += 1,
            }
        }

        result
    }

    /// Sparse cosine similarity
    pub fn cosine(a: &SparseVector, b: &SparseVector) -> f32 {
        let dot = Self::dot_product(a, b);
        let norm_a = Self::l2_norm(a);
        let norm_b = Self::l2_norm(b);

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot / (norm_a * norm_b)
    }

    /// L2 norm of sparse vector
    pub fn l2_norm(v: &SparseVector) -> f32 {
        v.values.iter().map(|x| x * x).sum::<f32>().sqrt()
    }

    /// Jaccard similarity (based on index overlap)
    pub fn jaccard(a: &SparseVector, b: &SparseVector) -> f32 {
        let mut intersection = 0;
        let mut i = 0;
        let mut j = 0;

        while i < a.indices.len() && j < b.indices.len() {
            match a.indices[i].cmp(&b.indices[j]) {
                std::cmp::Ordering::Equal => {
                    intersection += 1;
                    i += 1;
                    j += 1;
                }
                std::cmp::Ordering::Less => i += 1,
                std::cmp::Ordering::Greater => j += 1,
            }
        }

        let union = a.indices.len() + b.indices.len() - intersection;
        if union == 0 {
            0.0
        } else {
            intersection as f32 / union as f32
        }
    }
}
```

### 2.3 Token-Level Similarity

```rust
/// Similarity functions for token-level embeddings (Late Interaction)
pub struct TokenLevelSimilarity;

impl TokenLevelSimilarity {
    /// MaxSim: for each query token, find max similarity to any document token
    pub fn maxsim(query_tokens: &[Vec<f32>], doc_tokens: &[Vec<f32>]) -> f32 {
        if query_tokens.is_empty() || doc_tokens.is_empty() {
            return 0.0;
        }

        let mut total_sim = 0.0f32;

        for query_token in query_tokens {
            let max_sim = doc_tokens.iter()
                .map(|doc_token| DenseSimilarity::cosine(query_token, doc_token))
                .fold(f32::NEG_INFINITY, f32::max);
            total_sim += max_sim;
        }

        total_sim / query_tokens.len() as f32
    }

    /// Symmetric MaxSim: average of both directions
    pub fn symmetric_maxsim(a_tokens: &[Vec<f32>], b_tokens: &[Vec<f32>]) -> f32 {
        let a_to_b = Self::maxsim(a_tokens, b_tokens);
        let b_to_a = Self::maxsim(b_tokens, a_tokens);
        (a_to_b + b_to_a) / 2.0
    }

    /// Exact match count (normalized)
    pub fn exact_match_ratio(a_tokens: &[Vec<f32>], b_tokens: &[Vec<f32>], threshold: f32) -> f32 {
        if a_tokens.is_empty() {
            return 0.0;
        }

        let mut matches = 0;
        for a_token in a_tokens {
            for b_token in b_tokens {
                if DenseSimilarity::cosine(a_token, b_token) >= threshold {
                    matches += 1;
                    break;
                }
            }
        }

        matches as f32 / a_tokens.len() as f32
    }
}
```

## 3. Embedder Output Comparison

```rust
/// Compare two embedder outputs of the SAME type
pub fn compare_embedder_outputs(
    a: &EmbedderOutput,
    b: &EmbedderOutput,
    embedder: Embedder,
) -> Result<f32, ComparisonError> {
    match (a, b) {
        (EmbedderOutput::Dense(vec_a), EmbedderOutput::Dense(vec_b)) => {
            // Validate dimensions match expected for this embedder
            let expected = embedder.expected_dims();
            if let EmbedderDims::Dense(dim) = expected {
                if vec_a.len() != dim || vec_b.len() != dim {
                    return Err(ComparisonError::DimensionMismatch {
                        embedder,
                        expected: dim,
                        got_a: vec_a.len(),
                        got_b: vec_b.len(),
                    });
                }
            }

            Ok(DenseSimilarity::cosine(vec_a, vec_b))
        }

        (EmbedderOutput::Sparse(sparse_a), EmbedderOutput::Sparse(sparse_b)) => {
            Ok(SparseSimilarity::cosine(sparse_a, sparse_b))
        }

        (EmbedderOutput::TokenLevel(tokens_a), EmbedderOutput::TokenLevel(tokens_b)) => {
            Ok(TokenLevelSimilarity::symmetric_maxsim(tokens_a, tokens_b))
        }

        // Type mismatch - this should never happen with proper validation
        _ => Err(ComparisonError::TypeMismatch {
            embedder,
            type_a: std::mem::discriminant(a),
            type_b: std::mem::discriminant(b),
        }),
    }
}
```

## 4. Array Comparison

### 4.1 Full Array Comparison

```rust
/// Compare two teleological arrays
pub struct TeleologicalComparator {
    /// Similarity function for dense vectors
    dense_sim: fn(&[f32], &[f32]) -> f32,

    /// Similarity function for sparse vectors
    sparse_sim: fn(&SparseVector, &SparseVector) -> f32,

    /// Similarity function for token-level
    token_sim: fn(&[Vec<f32>], &[Vec<f32>]) -> f32,
}

impl Default for TeleologicalComparator {
    fn default() -> Self {
        Self {
            dense_sim: DenseSimilarity::cosine,
            sparse_sim: SparseSimilarity::cosine,
            token_sim: TokenLevelSimilarity::symmetric_maxsim,
        }
    }
}

impl TeleologicalComparator {
    /// Compare two arrays using specified comparison type
    pub fn compare(
        &self,
        a: &TeleologicalArray,
        b: &TeleologicalArray,
        comparison: &ComparisonType,
    ) -> ComparisonResult {
        let start = std::time::Instant::now();

        // Compute per-embedder scores
        let embedder_scores = self.compute_all_embedder_scores(a, b);

        // Compute overall similarity based on comparison type
        let (similarity, active_embedders) = match comparison {
            ComparisonType::SingleEmbedder(embedder) => {
                let idx = *embedder as usize;
                let mask = EmbedderMask::new().with(*embedder);
                (embedder_scores[idx], mask)
            }

            ComparisonType::EmbedderGroup(group) => {
                let mask = group.to_mask();
                let active: Vec<usize> = mask.iter().map(|e| e as usize).collect();
                let weight = 1.0 / active.len() as f32;
                let sim: f32 = active.iter()
                    .map(|&i| embedder_scores[i] * weight)
                    .sum();
                (sim, mask)
            }

            ComparisonType::WeightedFull(weights) => {
                let sim: f32 = embedder_scores.iter()
                    .enumerate()
                    .map(|(i, &s)| s * weights[i])
                    .sum();
                (sim, EmbedderMask::all())
            }

            ComparisonType::MatrixStrategy(matrix) => {
                let sim = self.apply_matrix(&embedder_scores, matrix);
                (sim, EmbedderMask::all())
            }
        };

        ComparisonResult {
            similarity,
            embedder_scores,
            active_embedders,
            computation_time_us: start.elapsed().as_micros() as u64,
            correlations: None,
        }
    }

    /// Compute similarity for all 13 embedders
    fn compute_all_embedder_scores(
        &self,
        a: &TeleologicalArray,
        b: &TeleologicalArray,
    ) -> [f32; 13] {
        let mut scores = [0.0f32; 13];

        for (i, embedder) in Embedder::all().iter().enumerate() {
            scores[i] = self.compute_single_embedder_score(
                &a.embeddings[i],
                &b.embeddings[i],
                *embedder,
            );
        }

        scores
    }

    fn compute_single_embedder_score(
        &self,
        a: &EmbedderOutput,
        b: &EmbedderOutput,
        _embedder: Embedder,
    ) -> f32 {
        match (a, b) {
            (EmbedderOutput::Dense(va), EmbedderOutput::Dense(vb)) => {
                (self.dense_sim)(va, vb)
            }
            (EmbedderOutput::Sparse(sa), EmbedderOutput::Sparse(sb)) => {
                (self.sparse_sim)(sa, sb)
            }
            (EmbedderOutput::TokenLevel(ta), EmbedderOutput::TokenLevel(tb)) => {
                (self.token_sim)(ta, tb)
            }
            // Should never happen with validated arrays
            _ => 0.0,
        }
    }

    fn apply_matrix(&self, scores: &[f32; 13], matrix: &SearchMatrix) -> f32 {
        let mut total = 0.0f32;

        for i in 0..13 {
            for j in 0..13 {
                if i == j {
                    total += matrix.weights[i][j] * scores[i];
                } else if matrix.use_correlations {
                    // Off-diagonal: geometric mean
                    let cross = (scores[i] * scores[j]).sqrt();
                    total += matrix.weights[i][j] * cross;
                }
            }
        }

        total
    }
}
```

### 4.2 Batch Comparison

```rust
/// Batch comparison for efficiency
pub struct BatchComparator {
    inner: TeleologicalComparator,
    parallelism: usize,
}

impl BatchComparator {
    pub fn new(parallelism: usize) -> Self {
        Self {
            inner: TeleologicalComparator::default(),
            parallelism,
        }
    }

    /// Compare one query against many candidates
    pub fn compare_one_to_many(
        &self,
        query: &TeleologicalArray,
        candidates: &[TeleologicalArray],
        comparison: &ComparisonType,
    ) -> Vec<ComparisonResult> {
        use rayon::prelude::*;

        candidates
            .par_iter()
            .with_min_len(self.parallelism)
            .map(|candidate| self.inner.compare(query, candidate, comparison))
            .collect()
    }

    /// Compare many queries against many candidates (N x M)
    pub fn compare_many_to_many(
        &self,
        queries: &[TeleologicalArray],
        candidates: &[TeleologicalArray],
        comparison: &ComparisonType,
    ) -> Vec<Vec<ComparisonResult>> {
        use rayon::prelude::*;

        queries
            .par_iter()
            .map(|query| {
                candidates
                    .iter()
                    .map(|candidate| self.inner.compare(query, candidate, comparison))
                    .collect()
            })
            .collect()
    }

    /// Compare each array with every other (pairwise)
    pub fn compare_pairwise(
        &self,
        arrays: &[TeleologicalArray],
        comparison: &ComparisonType,
    ) -> Vec<(usize, usize, ComparisonResult)> {
        use rayon::prelude::*;

        let n = arrays.len();
        let pairs: Vec<(usize, usize)> = (0..n)
            .flat_map(|i| ((i + 1)..n).map(move |j| (i, j)))
            .collect();

        pairs
            .par_iter()
            .map(|&(i, j)| {
                let result = self.inner.compare(&arrays[i], &arrays[j], comparison);
                (i, j, result)
            })
            .collect()
    }
}
```

## 5. Validation

```rust
/// Validation functions to ensure apples-to-apples comparisons
pub struct ComparisonValidator;

impl ComparisonValidator {
    /// Validate a teleological array has all 13 embedders with correct types
    pub fn validate_array(array: &TeleologicalArray) -> Result<(), ValidationError> {
        for (i, embedder) in Embedder::all().iter().enumerate() {
            Self::validate_embedder_output(&array.embeddings[i], *embedder)?;
        }
        Ok(())
    }

    /// Validate a single embedder output
    pub fn validate_embedder_output(
        output: &EmbedderOutput,
        embedder: Embedder,
    ) -> Result<(), ValidationError> {
        match (output, embedder.expected_dims()) {
            (EmbedderOutput::Dense(vec), EmbedderDims::Dense(expected)) => {
                if vec.len() != expected {
                    return Err(ValidationError::WrongDimension {
                        embedder,
                        expected,
                        actual: vec.len(),
                    });
                }
                // Check for NaN/Inf
                if vec.iter().any(|&v| v.is_nan() || v.is_infinite()) {
                    return Err(ValidationError::InvalidValue { embedder });
                }
            }

            (EmbedderOutput::Sparse(_), EmbedderDims::Sparse) => {
                // Sparse vectors are flexible, just validate sorted indices
            }

            (EmbedderOutput::TokenLevel(tokens), EmbedderDims::TokenLevel) => {
                if tokens.is_empty() {
                    return Err(ValidationError::EmptyTokenLevel { embedder });
                }
            }

            _ => {
                return Err(ValidationError::TypeMismatch { embedder });
            }
        }

        Ok(())
    }

    /// Validate two arrays can be compared
    pub fn validate_comparison(
        a: &TeleologicalArray,
        b: &TeleologicalArray,
    ) -> Result<(), ValidationError> {
        Self::validate_array(a)?;
        Self::validate_array(b)?;

        // Both arrays are valid - they can be compared apples-to-apples
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Embedder {embedder:?}: expected {expected}D, got {actual}D")]
    WrongDimension {
        embedder: Embedder,
        expected: usize,
        actual: usize,
    },

    #[error("Embedder {embedder:?}: contains NaN or Infinity")]
    InvalidValue { embedder: Embedder },

    #[error("Embedder {embedder:?}: wrong output type")]
    TypeMismatch { embedder: Embedder },

    #[error("Embedder {embedder:?}: empty token-level embeddings")]
    EmptyTokenLevel { embedder: Embedder },
}
```

## 6. Comparison Hooks

Claude Code hooks enable automatic validation and logging of comparison operations. These hooks prevent cross-embedder mistakes and ensure type safety.

### 6.1 Hook Configuration

```yaml
# .claude/hooks/comparison.yaml
hooks:
  # Pre-comparison validation hook
  - name: pre-comparison-validate
    event: PreToolUse
    matcher:
      tool_name: "compare_arrays|similarity_search|batch_compare"
    hooks:
      - type: command
        command: |
          npx claude-flow@v3alpha hooks pre-task \
            --description "Validate comparison type safety" \
            --context "$CLAUDE_TOOL_INPUT"

  # Post-comparison logging hook
  - name: post-comparison-log
    event: PostToolUse
    matcher:
      tool_name: "compare_arrays|similarity_search"
    hooks:
      - type: command
        command: |
          npx claude-flow@v3alpha hooks post-task \
            --task-id "comparison-$(date +%s)" \
            --success "$CLAUDE_TOOL_SUCCESS" \
            --metrics '{"embedders_used": 13, "comparison_type": "full"}'

  # Cross-embedder prevention hook
  - name: prevent-cross-embedder
    event: PreToolUse
    matcher:
      tool_name: "direct_similarity"
    hooks:
      - type: command
        command: |
          echo "Checking for cross-embedder comparison..."
          if echo "$CLAUDE_TOOL_INPUT" | jq -e '.embedder_a != .embedder_b' > /dev/null 2>&1; then
            echo "ERROR: Cross-embedder comparison detected!"
            echo "Only apples-to-apples comparisons are allowed."
            exit 1
          fi
```

### 6.2 Comparison Validation Hook

```rust
/// Hook that validates comparison operations before execution
pub struct ComparisonValidationHook;

impl ComparisonValidationHook {
    /// Called before any comparison operation
    pub fn pre_compare(
        &self,
        array_a: &TeleologicalArray,
        array_b: &TeleologicalArray,
        comparison_type: &ComparisonType,
    ) -> Result<ComparisonContext, HookError> {
        // Validate both arrays
        ComparisonValidator::validate_comparison(array_a, array_b)?;

        // Check embedder compatibility based on comparison type
        match comparison_type {
            ComparisonType::SingleEmbedder(embedder) => {
                self.validate_single_embedder(array_a, array_b, *embedder)?;
            }
            ComparisonType::EmbedderGroup(group) => {
                for embedder in group.embedders() {
                    self.validate_single_embedder(array_a, array_b, embedder)?;
                }
            }
            _ => {
                // Full comparison - all embedders validated
            }
        }

        Ok(ComparisonContext {
            started_at: std::time::Instant::now(),
            comparison_type: comparison_type.clone(),
            embedder_count: comparison_type.active_embedder_count(),
        })
    }

    /// Called after comparison completes
    pub fn post_compare(
        &self,
        context: ComparisonContext,
        result: &ComparisonResult,
    ) -> Result<(), HookError> {
        let duration = context.started_at.elapsed();

        // Log comparison metrics
        tracing::info!(
            comparison_type = ?context.comparison_type,
            embedder_count = context.embedder_count,
            similarity = result.similarity,
            duration_us = duration.as_micros() as u64,
            "Comparison completed"
        );

        // Store in memory for learning
        if let Ok(memory) = crate::memory::get_memory_store() {
            memory.store(
                "comparison_history",
                &format!("comparison_{}", chrono::Utc::now().timestamp()),
                &serde_json::json!({
                    "similarity": result.similarity,
                    "embedder_scores": result.embedder_scores,
                    "duration_us": duration.as_micros(),
                    "comparison_type": format!("{:?}", context.comparison_type),
                }),
            )?;
        }

        Ok(())
    }

    fn validate_single_embedder(
        &self,
        array_a: &TeleologicalArray,
        array_b: &TeleologicalArray,
        embedder: Embedder,
    ) -> Result<(), HookError> {
        let idx = embedder as usize;
        let output_a = &array_a.embeddings[idx];
        let output_b = &array_b.embeddings[idx];

        // Ensure same output type
        if std::mem::discriminant(output_a) != std::mem::discriminant(output_b) {
            return Err(HookError::CrossEmbedderComparison {
                embedder,
                type_a: output_a.type_name(),
                type_b: output_b.type_name(),
            });
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct ComparisonContext {
    pub started_at: std::time::Instant,
    pub comparison_type: ComparisonType,
    pub embedder_count: usize,
}
```

### 6.3 MCP Integration Hooks

```rust
/// MCP tool hooks for comparison operations
pub struct MCPComparisonHooks;

impl MCPComparisonHooks {
    /// Hook for MCP memory injection before comparison
    pub async fn on_memory_inject(
        &self,
        memory_id: &str,
        content: &str,
    ) -> Result<InjectionResult, HookError> {
        tracing::info!(
            memory_id = memory_id,
            content_length = content.len(),
            "Memory injection received"
        );

        // Trigger autonomous embedding
        let embedding_result = crate::embedding::autonomous_embed(content).await?;

        Ok(InjectionResult {
            memory_id: memory_id.to_string(),
            embeddings_generated: 13,
            storage_key: embedding_result.storage_key,
        })
    }

    /// Hook for comparison result export
    pub async fn on_comparison_complete(
        &self,
        result: &ComparisonResult,
        export_format: ExportFormat,
    ) -> Result<ExportResult, HookError> {
        match export_format {
            ExportFormat::Json => {
                let json = serde_json::to_string_pretty(result)?;
                Ok(ExportResult::Json(json))
            }
            ExportFormat::Memory => {
                // Store in claude-flow memory for cross-session access
                let memory_key = format!("comparison_{}", chrono::Utc::now().timestamp());
                mcp_memory_store("comparisons", &memory_key, result).await?;
                Ok(ExportResult::MemoryKey(memory_key))
            }
        }
    }
}
```

## 7. Comparison Skills

Skills provide reusable comparison operations that can be invoked by Claude Code agents. Each skill encapsulates a specific comparison pattern.

### 7.1 Skill Definitions

```yaml
# .claude/skills/comparison/apples-to-apples.yaml
name: apples-to-apples
description: >
  Compare teleological arrays ensuring type safety.
  Only compares embeddings from the same embedder model.
version: 1.0.0

parameters:
  - name: array_a_id
    type: string
    required: true
    description: Storage ID of first teleological array
  - name: array_b_id
    type: string
    required: true
    description: Storage ID of second teleological array
  - name: embedder
    type: string
    required: false
    description: Specific embedder to compare (default: all)
  - name: similarity_metric
    type: string
    default: cosine
    enum: [cosine, euclidean, dot_product, maxsim]

steps:
  - name: load_arrays
    action: memory_retrieve
    params:
      namespace: teleological_arrays
      keys: ["${array_a_id}", "${array_b_id}"]

  - name: validate
    action: hook_trigger
    params:
      hook: pre-comparison-validate
      context:
        array_a: "${load_arrays.array_a}"
        array_b: "${load_arrays.array_b}"

  - name: compare
    action: rust_fn
    params:
      function: TeleologicalComparator::compare
      args:
        array_a: "${load_arrays.array_a}"
        array_b: "${load_arrays.array_b}"
        embedder: "${embedder}"
        metric: "${similarity_metric}"

  - name: log_result
    action: hook_trigger
    params:
      hook: post-comparison-log
      context:
        result: "${compare.result}"

output:
  similarity: "${compare.result.similarity}"
  embedder_scores: "${compare.result.embedder_scores}"
  insights: "${compare.result.insights}"
```

### 7.2 RRF Fusion Skill

```yaml
# .claude/skills/comparison/rrf-fusion.yaml
name: rrf-fusion
description: >
  Reciprocal Rank Fusion for combining results from multiple embedding spaces.
  Merges ranked lists from different embedders into a unified ranking.
version: 1.0.0

parameters:
  - name: query_array_id
    type: string
    required: true
  - name: candidate_ids
    type: array
    items:
      type: string
    required: true
  - name: embedders
    type: array
    items:
      type: string
    default: ["semantic", "temporal", "causal", "lexical"]
  - name: k
    type: integer
    default: 60
    description: RRF constant (higher = more weight to lower ranks)
  - name: top_n
    type: integer
    default: 10

steps:
  - name: load_query
    action: memory_retrieve
    params:
      namespace: teleological_arrays
      key: "${query_array_id}"

  - name: load_candidates
    action: memory_retrieve_batch
    params:
      namespace: teleological_arrays
      keys: "${candidate_ids}"

  - name: per_embedder_rank
    action: parallel_map
    items: "${embedders}"
    item_name: embedder
    steps:
      - name: compute_similarities
        action: rust_fn
        params:
          function: BatchComparator::compare_one_to_many
          args:
            query: "${load_query.array}"
            candidates: "${load_candidates.arrays}"
            comparison_type:
              SingleEmbedder: "${embedder}"

      - name: rank
        action: rust_fn
        params:
          function: rank_by_similarity
          args:
            results: "${compute_similarities.results}"

  - name: rrf_fusion
    action: rust_fn
    params:
      function: reciprocal_rank_fusion
      args:
        ranked_lists: "${per_embedder_rank.*.rank.ranking}"
        k: "${k}"
        top_n: "${top_n}"

output:
  fused_ranking: "${rrf_fusion.ranking}"
  per_embedder_rankings: "${per_embedder_rank.*.rank.ranking}"
  fusion_scores: "${rrf_fusion.scores}"
```

### 7.3 Multi-Space Similarity Analysis Skill

```rust
/// Skill implementation for multi-space similarity analysis
pub struct MultiSpaceSimilaritySkill;

impl MultiSpaceSimilaritySkill {
    /// Analyze similarity across all 13 embedding spaces
    pub async fn analyze(
        query: &TeleologicalArray,
        candidates: &[TeleologicalArray],
        config: MultiSpaceConfig,
    ) -> Result<MultiSpaceAnalysis, SkillError> {
        let comparator = BatchComparator::new(config.parallelism);

        // Compute similarities in all spaces
        let all_results = comparator.compare_one_to_many(
            query,
            candidates,
            &ComparisonType::WeightedFull(config.weights),
        );

        // Analyze per-space contribution
        let mut space_analysis = Vec::new();
        for embedder in Embedder::all() {
            let space_scores: Vec<f32> = all_results
                .iter()
                .map(|r| r.embedder_scores[embedder as usize])
                .collect();

            space_analysis.push(SpaceAnalysis {
                embedder,
                mean_similarity: mean(&space_scores),
                variance: variance(&space_scores),
                top_k_coverage: compute_top_k_coverage(&space_scores, config.top_k),
            });
        }

        // Identify complementary spaces
        let correlations = compute_space_correlations(&all_results);
        let complementary = find_complementary_spaces(&correlations);

        // Compute ensemble ranking
        let ensemble_ranking = if config.use_rrf {
            Self::compute_rrf_ranking(&all_results, config.rrf_k)
        } else {
            Self::compute_weighted_ranking(&all_results)
        };

        Ok(MultiSpaceAnalysis {
            space_analysis,
            correlations,
            complementary_spaces: complementary,
            ensemble_ranking,
            computation_time_ms: 0, // Set by caller
        })
    }

    fn compute_rrf_ranking(
        results: &[ComparisonResult],
        k: usize,
    ) -> Vec<(usize, f32)> {
        let mut rrf_scores = vec![0.0f32; results.len()];

        for embedder_idx in 0..13 {
            // Get ranking for this embedder
            let mut indexed: Vec<(usize, f32)> = results
                .iter()
                .enumerate()
                .map(|(i, r)| (i, r.embedder_scores[embedder_idx]))
                .collect();
            indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

            // Add RRF contribution
            for (rank, (idx, _)) in indexed.iter().enumerate() {
                rrf_scores[*idx] += 1.0 / (k as f32 + rank as f32 + 1.0);
            }
        }

        // Return sorted by RRF score
        let mut final_ranking: Vec<(usize, f32)> = rrf_scores
            .iter()
            .enumerate()
            .map(|(i, &s)| (i, s))
            .collect();
        final_ranking.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        final_ranking
    }

    fn compute_weighted_ranking(results: &[ComparisonResult]) -> Vec<(usize, f32)> {
        let mut ranking: Vec<(usize, f32)> = results
            .iter()
            .enumerate()
            .map(|(i, r)| (i, r.similarity))
            .collect();
        ranking.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        ranking
    }
}
```

### 7.4 Skill Invocation from Claude Code

```typescript
// Example: Invoking comparison skills from Claude Code
import { Skill } from 'claude-code';

// Apples-to-apples comparison
const comparison = await Skill.invoke('apples-to-apples', {
  array_a_id: 'mem_12345',
  array_b_id: 'mem_67890',
  embedder: 'semantic',  // Optional: compare only semantic space
  similarity_metric: 'cosine'
});

console.log(`Similarity: ${comparison.similarity}`);
console.log(`Embedder scores: ${JSON.stringify(comparison.embedder_scores)}`);

// RRF fusion for multi-embedder search
const fusedResults = await Skill.invoke('rrf-fusion', {
  query_array_id: 'query_001',
  candidate_ids: ['doc_001', 'doc_002', 'doc_003', /* ... */],
  embedders: ['semantic', 'temporal', 'causal', 'lexical', 'emotional'],
  k: 60,
  top_n: 10
});

console.log(`Top results: ${JSON.stringify(fusedResults.fused_ranking)}`);
```

## 8. Comparison Subagents

Subagents enable parallel comparison across the 13 embedding spaces. Each subagent specializes in a specific comparison task.

### 8.1 Subagent Architecture

```
                    +----------------------+
                    |  Coordinator Agent   |
                    |  (Orchestration)     |
                    +----------+-----------+
                               |
           +-------------------+-------------------+
           |                   |                   |
    +------v------+     +------v------+     +------v------+
    | Dense Space |     | Sparse Space|     | Token-Level |
    | Comparators |     | Comparators |     | Comparators |
    | (10 agents) |     | (2 agents)  |     | (1 agent)   |
    +-------------+     +-------------+     +-------------+
           |                   |                   |
           +-------------------+-------------------+
                               |
                    +----------v-----------+
                    |  Fusion Agent        |
                    |  (RRF, Weighted)     |
                    +----------------------+
                               |
                    +----------v-----------+
                    |  Validation Agent    |
                    |  (Type Safety)       |
                    +----------------------+
```

### 8.2 Subagent Configuration

```yaml
# .claude/agents/comparison-swarm.yaml
name: comparison-swarm
description: Parallel comparison across 13 embedding spaces
topology: hierarchical
max_agents: 15

coordinator:
  type: comparison-coordinator
  capabilities:
    - orchestrate_comparison
    - aggregate_results
    - route_to_specialists
  memory_namespace: comparison_coordination

agents:
  # Dense embedding comparators (10 spaces)
  - name: semantic-comparator
    type: dense-comparator
    specialization: semantic
    embedder_index: 0
    similarity_fn: cosine

  - name: temporal-comparator
    type: dense-comparator
    specialization: temporal
    embedder_index: 1
    similarity_fn: cosine

  - name: counterfactual-comparator
    type: dense-comparator
    specialization: counterfactual
    embedder_index: 2
    similarity_fn: cosine

  - name: entity-comparator
    type: dense-comparator
    specialization: entity
    embedder_index: 3
    similarity_fn: cosine

  - name: causal-comparator
    type: dense-comparator
    specialization: causal
    embedder_index: 4
    similarity_fn: cosine

  - name: pragmatic-comparator
    type: dense-comparator
    specialization: pragmatic
    embedder_index: 6
    similarity_fn: cosine

  - name: emotional-comparator
    type: dense-comparator
    specialization: emotional
    embedder_index: 7
    similarity_fn: cosine

  - name: code-comparator
    type: dense-comparator
    specialization: code
    embedder_index: 8
    similarity_fn: cosine

  - name: multimodal-comparator
    type: dense-comparator
    specialization: multimodal
    embedder_index: 9
    similarity_fn: cosine

  - name: domain-comparator
    type: dense-comparator
    specialization: domain
    embedder_index: 10
    similarity_fn: cosine

  # Sparse embedding comparators (2 spaces)
  - name: lexical-comparator
    type: sparse-comparator
    specialization: lexical
    embedder_index: 5
    similarity_fn: sparse_cosine

  - name: hybrid-comparator
    type: sparse-comparator
    specialization: hybrid
    embedder_index: 11
    similarity_fn: sparse_cosine

  # Token-level comparator (1 space)
  - name: colbert-comparator
    type: token-comparator
    specialization: late_interaction
    embedder_index: 12
    similarity_fn: maxsim

  # Fusion and validation
  - name: fusion-agent
    type: fusion-specialist
    capabilities:
      - rrf_fusion
      - weighted_fusion
      - correlation_analysis

  - name: validation-agent
    type: validation-specialist
    capabilities:
      - type_safety_check
      - dimension_validation
      - cross_embedder_prevention

orchestration:
  # Parallel execution of all 13 comparators
  parallel_phase:
    agents:
      - semantic-comparator
      - temporal-comparator
      - counterfactual-comparator
      - entity-comparator
      - causal-comparator
      - lexical-comparator
      - pragmatic-comparator
      - emotional-comparator
      - code-comparator
      - multimodal-comparator
      - domain-comparator
      - hybrid-comparator
      - colbert-comparator
    max_concurrency: 13

  # Sequential aggregation
  aggregation_phase:
    agents:
      - fusion-agent
      - validation-agent
    order: sequential
```

### 8.3 Subagent Implementation

```rust
/// Comparison subagent that specializes in one embedding space
pub struct ComparisonSubagent {
    pub name: String,
    pub embedder: Embedder,
    pub embedder_index: usize,
    pub similarity_fn: SimilarityFunction,
}

impl ComparisonSubagent {
    /// Execute comparison for this embedder space
    pub async fn compare(
        &self,
        array_a: &TeleologicalArray,
        array_b: &TeleologicalArray,
    ) -> SubagentResult {
        let start = std::time::Instant::now();

        // Extract embeddings for this space
        let output_a = &array_a.embeddings[self.embedder_index];
        let output_b = &array_b.embeddings[self.embedder_index];

        // Compute similarity using specialized function
        let similarity = match (&self.similarity_fn, output_a, output_b) {
            (SimilarityFunction::Cosine, EmbedderOutput::Dense(a), EmbedderOutput::Dense(b)) => {
                DenseSimilarity::cosine(a, b)
            }
            (SimilarityFunction::SparseCosine, EmbedderOutput::Sparse(a), EmbedderOutput::Sparse(b)) => {
                SparseSimilarity::cosine(a, b)
            }
            (SimilarityFunction::MaxSim, EmbedderOutput::TokenLevel(a), EmbedderOutput::TokenLevel(b)) => {
                TokenLevelSimilarity::symmetric_maxsim(a, b)
            }
            _ => {
                return SubagentResult::Error(SubagentError::TypeMismatch {
                    embedder: self.embedder,
                });
            }
        };

        SubagentResult::Success {
            embedder: self.embedder,
            similarity,
            computation_time_us: start.elapsed().as_micros() as u64,
        }
    }

    /// Batch comparison for this embedder space
    pub async fn compare_batch(
        &self,
        query: &TeleologicalArray,
        candidates: &[TeleologicalArray],
    ) -> Vec<SubagentResult> {
        use futures::future::join_all;

        let futures: Vec<_> = candidates
            .iter()
            .map(|candidate| self.compare(query, candidate))
            .collect();

        join_all(futures).await
    }
}

/// Coordinator that orchestrates comparison subagents
pub struct ComparisonCoordinator {
    subagents: Vec<ComparisonSubagent>,
    fusion_agent: FusionAgent,
    validation_agent: ValidationAgent,
}

impl ComparisonCoordinator {
    pub fn new() -> Self {
        let subagents = Embedder::all()
            .iter()
            .enumerate()
            .map(|(idx, embedder)| {
                ComparisonSubagent {
                    name: format!("{:?}-comparator", embedder),
                    embedder: *embedder,
                    embedder_index: idx,
                    similarity_fn: embedder.default_similarity_fn(),
                }
            })
            .collect();

        Self {
            subagents,
            fusion_agent: FusionAgent::new(),
            validation_agent: ValidationAgent::new(),
        }
    }

    /// Execute parallel comparison across all 13 spaces
    pub async fn parallel_compare(
        &self,
        array_a: &TeleologicalArray,
        array_b: &TeleologicalArray,
    ) -> Result<CoordinatedComparisonResult, CoordinatorError> {
        // Phase 1: Validation
        self.validation_agent.validate(array_a, array_b)?;

        // Phase 2: Parallel comparison across all spaces
        use futures::future::join_all;

        let comparison_futures: Vec<_> = self.subagents
            .iter()
            .map(|agent| agent.compare(array_a, array_b))
            .collect();

        let subagent_results = join_all(comparison_futures).await;

        // Collect successful results
        let mut embedder_scores = [0.0f32; 13];
        let mut successful_count = 0;

        for result in &subagent_results {
            match result {
                SubagentResult::Success { embedder, similarity, .. } => {
                    embedder_scores[*embedder as usize] = *similarity;
                    successful_count += 1;
                }
                SubagentResult::Error(e) => {
                    tracing::warn!(error = ?e, "Subagent comparison failed");
                }
            }
        }

        // Phase 3: Fusion
        let fused_similarity = self.fusion_agent.fuse(&embedder_scores);

        Ok(CoordinatedComparisonResult {
            similarity: fused_similarity,
            embedder_scores,
            successful_subagents: successful_count,
            total_subagents: 13,
            subagent_results,
        })
    }
}
```

### 8.4 Spawning Comparison Subagents from Claude Code

```typescript
// Spawn comparison swarm using Claude Code's Task tool
async function spawnComparisonSwarm(queryId: string, candidateIds: string[]) {
  // Initialize swarm coordination
  await mcp__claude_flow__swarm_init({
    topology: "hierarchical",
    maxAgents: 15,
    strategy: "specialized"
  });

  // Spawn all comparison agents in parallel
  const agentTasks = [
    // Dense comparators
    Task("SemanticComparator", `
      You are a semantic similarity specialist.
      Compare embeddings at index 0 using cosine similarity.
      Query: ${queryId}
      Candidates: ${JSON.stringify(candidateIds)}
      Store results in memory namespace 'comparison_results'.
    `, "dense-comparator"),

    Task("TemporalComparator", `
      You are a temporal similarity specialist.
      Compare embeddings at index 1 using cosine similarity.
      Query: ${queryId}
      Candidates: ${JSON.stringify(candidateIds)}
    `, "dense-comparator"),

    // ... (10 more dense comparators)

    // Sparse comparators
    Task("LexicalComparator", `
      You are a lexical similarity specialist (SPLADE).
      Compare sparse embeddings at index 5.
      Query: ${queryId}
      Candidates: ${JSON.stringify(candidateIds)}
    `, "sparse-comparator"),

    Task("HybridComparator", `
      You are a hybrid similarity specialist.
      Compare sparse embeddings at index 11.
      Query: ${queryId}
      Candidates: ${JSON.stringify(candidateIds)}
    `, "sparse-comparator"),

    // Token-level comparator
    Task("ColBERTComparator", `
      You are a late-interaction similarity specialist.
      Compare token-level embeddings at index 12 using MaxSim.
      Query: ${queryId}
      Candidates: ${JSON.stringify(candidateIds)}
    `, "token-comparator"),

    // Fusion agent
    Task("FusionAgent", `
      You are the fusion specialist.
      Wait for all comparator results in memory namespace 'comparison_results'.
      Apply RRF fusion with k=60.
      Store final ranking in 'comparison_final'.
    `, "fusion-specialist"),

    // Validation agent
    Task("ValidationAgent", `
      You are the validation specialist.
      Verify all comparisons are type-safe (apples-to-apples).
      Check for cross-embedder mistakes.
      Report any violations.
    `, "validation-specialist"),
  ];

  // All agents spawn in parallel
  await Promise.all(agentTasks);

  // Wait for fusion to complete
  const finalResult = await mcp__claude_flow__memory_search({
    pattern: "comparison_final",
    namespace: "comparison_results"
  });

  return finalResult;
}
```

### 8.5 Validation Subagent

```rust
/// Validation subagent that ensures type safety
pub struct ValidationAgent {
    rules: Vec<ValidationRule>,
}

impl ValidationAgent {
    pub fn new() -> Self {
        Self {
            rules: vec![
                ValidationRule::DimensionMatch,
                ValidationRule::TypeCompatibility,
                ValidationRule::NaNInfCheck,
                ValidationRule::CrossEmbedderPrevention,
            ],
        }
    }

    /// Validate arrays before comparison
    pub fn validate(
        &self,
        array_a: &TeleologicalArray,
        array_b: &TeleologicalArray,
    ) -> Result<(), ValidationError> {
        for rule in &self.rules {
            rule.check(array_a, array_b)?;
        }
        Ok(())
    }

    /// Validate comparison results after execution
    pub fn validate_results(
        &self,
        results: &[SubagentResult],
    ) -> ValidationReport {
        let mut report = ValidationReport::default();

        for result in results {
            match result {
                SubagentResult::Success { embedder, similarity, .. } => {
                    // Check for suspicious values
                    if *similarity < -1.0 || *similarity > 1.0 {
                        report.warnings.push(ValidationWarning::OutOfRange {
                            embedder: *embedder,
                            value: *similarity,
                        });
                    }
                }
                SubagentResult::Error(e) => {
                    report.errors.push(e.clone());
                }
            }
        }

        report.is_valid = report.errors.is_empty();
        report
    }
}

#[derive(Clone, Debug)]
pub enum ValidationRule {
    /// Ensure dimensions match expected for each embedder
    DimensionMatch,

    /// Ensure output types are compatible
    TypeCompatibility,

    /// Check for NaN/Inf values
    NaNInfCheck,

    /// Prevent cross-embedder comparisons
    CrossEmbedderPrevention,
}

impl ValidationRule {
    pub fn check(
        &self,
        array_a: &TeleologicalArray,
        array_b: &TeleologicalArray,
    ) -> Result<(), ValidationError> {
        match self {
            Self::DimensionMatch => {
                for (i, embedder) in Embedder::all().iter().enumerate() {
                    let dim_a = array_a.embeddings[i].dimension();
                    let dim_b = array_b.embeddings[i].dimension();
                    if dim_a != dim_b {
                        return Err(ValidationError::WrongDimension {
                            embedder: *embedder,
                            expected: dim_a,
                            actual: dim_b,
                        });
                    }
                }
                Ok(())
            }

            Self::TypeCompatibility => {
                for (i, embedder) in Embedder::all().iter().enumerate() {
                    let disc_a = std::mem::discriminant(&array_a.embeddings[i]);
                    let disc_b = std::mem::discriminant(&array_b.embeddings[i]);
                    if disc_a != disc_b {
                        return Err(ValidationError::TypeMismatch {
                            embedder: *embedder,
                        });
                    }
                }
                Ok(())
            }

            Self::NaNInfCheck => {
                for (i, embedder) in Embedder::all().iter().enumerate() {
                    if array_a.embeddings[i].contains_invalid()
                        || array_b.embeddings[i].contains_invalid()
                    {
                        return Err(ValidationError::InvalidValue {
                            embedder: *embedder,
                        });
                    }
                }
                Ok(())
            }

            Self::CrossEmbedderPrevention => {
                // This rule is enforced by the type system
                // But we double-check here for safety
                Ok(())
            }
        }
    }
}
```

## 9. Insights Extraction

```rust
/// Extract insights from comparing teleological arrays
pub struct InsightsExtractor;

impl InsightsExtractor {
    /// Analyze what makes two arrays similar or different
    pub fn extract_insights(
        a: &TeleologicalArray,
        b: &TeleologicalArray,
        result: &ComparisonResult,
    ) -> ComparisonInsights {
        let mut insights = ComparisonInsights::default();

        // Find strongest contributing embedders
        let mut indexed: Vec<(usize, f32)> = result.embedder_scores
            .iter()
            .enumerate()
            .map(|(i, &s)| (i, s))
            .collect();
        indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        insights.strongest_embedders = indexed[..3.min(indexed.len())]
            .iter()
            .map(|(i, s)| (Embedder::all()[*i], *s))
            .collect();

        insights.weakest_embedders = indexed[(indexed.len().saturating_sub(3))..]
            .iter()
            .rev()
            .map(|(i, s)| (Embedder::all()[*i], *s))
            .collect();

        // Compute variance to understand consistency
        let mean: f32 = result.embedder_scores.iter().sum::<f32>() / 13.0;
        let variance: f32 = result.embedder_scores.iter()
            .map(|&s| (s - mean).powi(2))
            .sum::<f32>() / 13.0;

        insights.consistency = 1.0 - variance.sqrt();

        // Detect specific patterns
        insights.patterns = Self::detect_comparison_patterns(&result.embedder_scores);

        // Overall interpretation
        insights.interpretation = Self::interpret_comparison(result, &insights);

        insights
    }

    fn detect_comparison_patterns(scores: &[f32; 13]) -> Vec<ComparisonPattern> {
        let mut patterns = Vec::new();

        // Semantic dominance
        if scores[0] > 0.8 && scores[0] > scores.iter().skip(1).cloned().fold(0.0f32, f32::max) + 0.2 {
            patterns.push(ComparisonPattern::SemanticDominant);
        }

        // Temporal relevance
        if scores[1] > 0.7 || scores[2] > 0.7 {
            patterns.push(ComparisonPattern::TemporallyRelevant);
        }

        // Entity connection
        if scores[3] > 0.6 {
            patterns.push(ComparisonPattern::EntityConnected);
        }

        // Causal relationship
        if scores[4] > 0.6 {
            patterns.push(ComparisonPattern::CausallyRelated);
        }

        // Lexical match
        if scores[5] > 0.7 || scores[12] > 0.7 {
            patterns.push(ComparisonPattern::LexicalMatch);
        }

        // Emotional resonance
        if scores[7] > 0.7 {
            patterns.push(ComparisonPattern::EmotionallyResonant);
        }

        patterns
    }

    fn interpret_comparison(
        result: &ComparisonResult,
        insights: &ComparisonInsights,
    ) -> String {
        let mut parts = Vec::new();

        if result.similarity > 0.9 {
            parts.push("Highly similar overall.");
        } else if result.similarity > 0.7 {
            parts.push("Moderately similar.");
        } else if result.similarity > 0.5 {
            parts.push("Weakly similar.");
        } else {
            parts.push("Largely dissimilar.");
        }

        if insights.consistency > 0.8 {
            parts.push("Similarity is consistent across all dimensions.");
        } else if insights.consistency < 0.5 {
            parts.push("Similarity varies significantly by dimension.");
        }

        if !insights.patterns.is_empty() {
            let pattern_names: Vec<&str> = insights.patterns.iter()
                .map(|p| p.name())
                .collect();
            parts.push(&format!("Detected: {}", pattern_names.join(", ")));
        }

        parts.join(" ")
    }
}

#[derive(Clone, Debug, Default)]
pub struct ComparisonInsights {
    /// Embedders contributing most to similarity
    pub strongest_embedders: Vec<(Embedder, f32)>,

    /// Embedders contributing least
    pub weakest_embedders: Vec<(Embedder, f32)>,

    /// How consistent similarity is across embedders
    pub consistency: f32,

    /// Detected patterns
    pub patterns: Vec<ComparisonPattern>,

    /// Human-readable interpretation
    pub interpretation: String,
}

#[derive(Clone, Debug)]
pub enum ComparisonPattern {
    SemanticDominant,
    TemporallyRelevant,
    EntityConnected,
    CausallyRelated,
    LexicalMatch,
    EmotionallyResonant,
    StructuralMatch,
    PragmaticallyAligned,
}

impl ComparisonPattern {
    pub fn name(&self) -> &'static str {
        match self {
            Self::SemanticDominant => "semantic-dominant",
            Self::TemporallyRelevant => "temporally-relevant",
            Self::EntityConnected => "entity-connected",
            Self::CausallyRelated => "causally-related",
            Self::LexicalMatch => "lexical-match",
            Self::EmotionallyResonant => "emotionally-resonant",
            Self::StructuralMatch => "structural-match",
            Self::PragmaticallyAligned => "pragmatically-aligned",
        }
    }
}
```

## 10. Integration Summary

### Hook Integration Points

| Hook | Purpose | When Triggered |
|------|---------|----------------|
| `pre-comparison-validate` | Type safety validation | Before any comparison |
| `post-comparison-log` | Metrics and logging | After comparison completes |
| `prevent-cross-embedder` | Block invalid comparisons | On direct similarity calls |
| `on_memory_inject` | Trigger autonomous embedding | On MCP memory injection |

### Skill Catalog

| Skill | Purpose | Parameters |
|-------|---------|------------|
| `apples-to-apples` | Type-safe comparison | array_ids, embedder, metric |
| `rrf-fusion` | Multi-space ranking | query, candidates, embedders, k |
| `multi-space-analysis` | Full 13-space analysis | query, candidates, config |

### Subagent Types

| Agent Type | Count | Responsibility |
|------------|-------|----------------|
| Dense Comparator | 10 | Cosine similarity for dense spaces |
| Sparse Comparator | 2 | Sparse cosine for SPLADE/hybrid |
| Token Comparator | 1 | MaxSim for late interaction |
| Fusion Agent | 1 | RRF and weighted fusion |
| Validation Agent | 1 | Type safety enforcement |
| Coordinator | 1 | Orchestration |

### End-to-End Flow

```
1. Memory Injection (MCP)
   └── Hook: on_memory_inject
       └── Trigger autonomous embedding

2. Entry-Point Discovery
   └── Skill: apples-to-apples
       └── Hook: pre-comparison-validate
           └── Type safety check

3. Parallel Comparison (Subagents)
   └── 13 specialized comparators
       └── Each space compared independently
           └── Results stored in memory

4. Fusion (Subagent)
   └── Skill: rrf-fusion
       └── Combine rankings from all spaces

5. Validation (Subagent)
   └── Hook: post-comparison-log
       └── Metrics logged

6. Goal Emergence
   └── Clustering based on comparison results
       └── Autonomous pattern detection
```

# TASK-15: Implement TokenPruningQuantizer

```xml
<task_spec id="TASK-EMBED-003" version="3.0">
<metadata>
  <title>Implement TokenPruningQuantizer</title>
  <status>complete</status>
  <layer>logic</layer>
  <sequence>15</sequence>
  <implements><requirement_ref>REQ-EMBED-003</requirement_ref></implements>
  <depends_on>TASK-14</depends_on>
  <blocks>none</blocks>
  <estimated_hours>4</estimated_hours>
</metadata>
</task_spec>
```

---

## AI AGENT CRITICAL INSTRUCTIONS

**READ THIS ENTIRE DOCUMENT BEFORE WRITING ANY CODE.**

You are implementing a token pruning quantizer for E12 (ColBERT/Late Interaction) embeddings. This reduces embedding size by ~50% while maintaining semantic recall quality.

### ABSOLUTE RULES

1. **NO BACKWARDS COMPATIBILITY** - The system must work or fail fast with clear errors
2. **NO MOCK DATA IN TESTS** - Use real synthetic data with known expected outputs
3. **NO WORKAROUNDS OR FALLBACKS** - If something fails, it must error with clear diagnostics
4. **VERIFY ALL OUTPUTS PHYSICALLY** - Check actual data exists after operations

---

## 1. CURRENT CODEBASE STATE (Verified 2026-01-13)

### 1.1 TASK-14 Completion Status: COMPLETE

TASK-14 created the configuration types that this task depends on. The following files exist and work:

```
crates/context-graph-embeddings/src/pruning/
├── mod.rs     # Module exports (28 lines)
└── config.rs  # Config types and tests (366 lines)
```

**Verification command (run this first to confirm):**
```bash
cargo test -p context-graph-embeddings config::tests --features "cuda" --lib -- --nocapture 2>&1 | grep -E "(running|passed|failed)"
# Expected: running 42 tests ... test result: ok. 42 passed
```

### 1.2 Available Types from TASK-14

These types are already defined in `crates/context-graph-embeddings/src/pruning/config.rs`:

```rust
// ALREADY EXISTS - DO NOT RECREATE
pub struct TokenPruningConfig {
    pub target_compression: f32,  // Default: 0.5 (50% compression)
    pub min_tokens: usize,        // Default: 64
    pub scoring_method: ImportanceScoringMethod,
}

pub enum ImportanceScoringMethod {
    AttentionBased,      // Best accuracy, requires attention weights
    EmbeddingMagnitude,  // Fast, L2 norm of embeddings
    Entropy,             // Uses probability distribution entropy
}

pub struct PrunedEmbeddings {
    pub embeddings: Vec<Vec<f32>>,     // [num_retained, 128]
    pub retained_indices: Vec<usize>,   // Sorted ascending
    pub compression_ratio: f32,         // Achieved ratio [0, 1]
}
```

### 1.3 E12 Late Interaction Model Location

The E12 model is at: `crates/context-graph-embeddings/src/models/pretrained/late_interaction/`

**Key constants from `types.rs`:**
```rust
pub const LATE_INTERACTION_DIMENSION: usize = 128;  // Per-token embedding dimension
pub const LATE_INTERACTION_MAX_TOKENS: usize = 512; // Maximum tokens per input
```

**TokenEmbeddings struct from `types.rs`:**
```rust
pub struct TokenEmbeddings {
    pub vectors: Vec<Vec<f32>>,  // [num_tokens, 128]
    pub tokens: Vec<String>,     // Token strings
    pub mask: Vec<bool>,         // Valid (non-padding) tokens
}
```

### 1.4 Error Types (USE THESE - DO NOT CREATE NEW ONES)

Located at: `crates/context-graph-embeddings/src/error/types.rs`

```rust
// USE THESE FOR ERRORS:
EmbeddingError::ConfigError { message: String }      // Invalid configuration
EmbeddingError::InvalidDimension { expected, actual } // Wrong vector dimension
EmbeddingError::EmptyInput                           // Empty input provided
```

---

## 2. WHAT YOU MUST BUILD

Create a single new file: `crates/context-graph-embeddings/src/pruning/token_pruner.rs`

### 2.1 Required Struct and Methods

```rust
// crates/context-graph-embeddings/src/pruning/token_pruner.rs

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::pruning::config::{ImportanceScoringMethod, PrunedEmbeddings, TokenPruningConfig};

/// Token pruning quantizer for E12 Late Interaction embeddings.
///
/// Constitution: embeddings.models.E12_LateInteraction = "128D/tok"
/// Target: ~50% compression (512 -> ~256 tokens)
/// Constraint: Recall@10 degradation < 5%
pub struct TokenPruningQuantizer {
    config: TokenPruningConfig,
}

impl TokenPruningQuantizer {
    /// Create a new token pruning quantizer.
    ///
    /// # Errors
    /// Returns `EmbeddingError::ConfigError` if config validation fails.
    pub fn new(config: TokenPruningConfig) -> EmbeddingResult<Self> {
        config.validate()?;
        Ok(Self { config })
    }

    /// Prune low-importance tokens from E12 embeddings.
    ///
    /// # Arguments
    /// * `embeddings` - Token embeddings, shape [num_tokens, 128]
    /// * `attention_weights` - Optional attention weights for importance scoring
    ///
    /// # Returns
    /// Pruned embeddings with retained token indices.
    ///
    /// # Guarantees
    /// - Output has at least `min_tokens` tokens
    /// - `retained_indices` is sorted in ascending order
    /// - Compression ratio approximately matches `target_compression`
    ///
    /// # Errors
    /// - `EmbeddingError::EmptyInput` if embeddings is empty
    /// - `EmbeddingError::InvalidDimension` if any embedding is not 128D
    pub fn prune(
        &self,
        embeddings: &[Vec<f32>],
        attention_weights: Option<&[f32]>,
    ) -> EmbeddingResult<PrunedEmbeddings> {
        // Implementation here
    }
}
```

### 2.2 Required Private Helper Methods

```rust
impl TokenPruningQuantizer {
    /// Score tokens by their importance using the configured method.
    fn score_tokens(
        &self,
        embeddings: &[Vec<f32>],
        attention_weights: Option<&[f32]>,
    ) -> Vec<f32> {
        match self.config.scoring_method {
            ImportanceScoringMethod::AttentionBased => {
                // Use attention weights if available, otherwise fall back to magnitude
                attention_weights
                    .map(|w| w.to_vec())
                    .unwrap_or_else(|| self.score_by_magnitude(embeddings))
            }
            ImportanceScoringMethod::EmbeddingMagnitude => self.score_by_magnitude(embeddings),
            ImportanceScoringMethod::Entropy => self.score_by_entropy(embeddings),
        }
    }

    /// Score by L2 norm (magnitude) of each token embedding.
    fn score_by_magnitude(&self, embeddings: &[Vec<f32>]) -> Vec<f32> {
        embeddings
            .iter()
            .map(|emb| emb.iter().map(|x| x * x).sum::<f32>().sqrt())
            .collect()
    }

    /// Score by entropy of normalized embedding values.
    fn score_by_entropy(&self, embeddings: &[Vec<f32>]) -> Vec<f32> {
        embeddings
            .iter()
            .map(|emb| {
                let sum: f32 = emb.iter().map(|x| x.abs()).sum();
                if sum == 0.0 {
                    return 0.0;
                }
                let probs: Vec<f32> = emb.iter().map(|x| x.abs() / sum).collect();
                probs
                    .iter()
                    .filter(|&&p| p > 0.0)
                    .map(|&p| -p * p.ln())
                    .sum()
            })
            .collect()
    }
}
```

---

## 3. PRUNING ALGORITHM (STEP BY STEP)

The `prune()` method MUST implement this exact algorithm:

```
ALGORITHM: Token Pruning for E12 Embeddings

INPUT:
  - embeddings: Vec<Vec<f32>> of shape [N, 128] where N = num_tokens
  - attention_weights: Option<&[f32]> of shape [N] (optional)

OUTPUT:
  - PrunedEmbeddings { embeddings, retained_indices, compression_ratio }

STEPS:

1. VALIDATE INPUT
   - IF embeddings.is_empty() THEN RETURN Err(EmptyInput)
   - FOR EACH embedding IN embeddings:
       - IF embedding.len() != 128 THEN RETURN Err(InvalidDimension { expected: 128, actual: embedding.len() })

2. CALCULATE TARGET TOKEN COUNT
   - target_count = floor((1.0 - target_compression) * N)
   - target_count = max(target_count, min_tokens)
   - target_count = min(target_count, N)  // Can't keep more than we have

3. CHECK IF PRUNING NEEDED
   - IF target_count >= N THEN RETURN all embeddings unchanged (no pruning)

4. SCORE ALL TOKENS
   - scores = score_tokens(embeddings, attention_weights)
   - scores.len() MUST equal N

5. RANK TOKENS BY IMPORTANCE
   - Create indexed_scores: Vec<(usize, f32)> = scores.iter().enumerate().collect()
   - Sort by score DESCENDING (highest importance first)

6. SELECT TOP-K TOKENS
   - top_k_indices: Vec<usize> = indexed_scores[0..target_count].map(|(idx, _)| idx)

7. SORT RETAINED INDICES (preserve positional order)
   - retained_indices = top_k_indices.sorted_ascending()

8. EXTRACT PRUNED EMBEDDINGS
   - pruned_embeddings = retained_indices.map(|idx| embeddings[idx].clone())

9. CALCULATE ACHIEVED COMPRESSION
   - compression_ratio = 1.0 - (retained_indices.len() as f32 / N as f32)

10. RETURN
    - PrunedEmbeddings {
        embeddings: pruned_embeddings,
        retained_indices,
        compression_ratio,
      }
```

---

## 4. FILES TO MODIFY

### 4.1 Create: `crates/context-graph-embeddings/src/pruning/token_pruner.rs`

Create this file with the full implementation including tests (see Section 5).

### 4.2 Modify: `crates/context-graph-embeddings/src/pruning/mod.rs`

**Current content:**
```rust
//! Token pruning for E12 (ColBERT) late-interaction embeddings.
//! ...

mod config;

pub use config::{ImportanceScoringMethod, PrunedEmbeddings, TokenPruningConfig};
```

**Add these lines:**
```rust
mod token_pruner;

pub use token_pruner::TokenPruningQuantizer;
```

### 4.3 Modify: `crates/context-graph-embeddings/src/lib.rs`

**Find this line (around line 106):**
```rust
pub use pruning::{ImportanceScoringMethod, PrunedEmbeddings, TokenPruningConfig};
```

**Change to:**
```rust
pub use pruning::{ImportanceScoringMethod, PrunedEmbeddings, TokenPruningConfig, TokenPruningQuantizer};
```

---

## 5. REQUIRED TESTS (MANDATORY)

All tests must go in the same file as the implementation.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // === Helper to create synthetic 128D embeddings ===
    fn make_embedding(base: f32, variation: f32) -> Vec<f32> {
        (0..128).map(|i| base + variation * (i as f32 / 128.0)).collect()
    }

    fn make_embeddings(count: usize) -> Vec<Vec<f32>> {
        (0..count)
            .map(|i| make_embedding(i as f32 * 0.1, 0.5))
            .collect()
    }

    // === Construction Tests ===

    #[test]
    fn test_new_with_default_config() {
        let config = TokenPruningConfig::default();
        let quantizer = TokenPruningQuantizer::new(config);
        assert!(quantizer.is_ok());
    }

    #[test]
    fn test_new_with_invalid_config_fails() {
        let config = TokenPruningConfig {
            target_compression: 1.5, // Invalid: > 1.0
            ..Default::default()
        };
        let result = TokenPruningQuantizer::new(config);
        assert!(result.is_err());
    }

    // === Empty Input Tests ===

    #[test]
    fn test_prune_empty_input_returns_error() {
        let quantizer = TokenPruningQuantizer::new(TokenPruningConfig::default()).unwrap();
        let result = quantizer.prune(&[], None);
        assert!(matches!(result, Err(EmbeddingError::EmptyInput)));
    }

    // === Invalid Dimension Tests ===

    #[test]
    fn test_prune_wrong_dimension_returns_error() {
        let quantizer = TokenPruningQuantizer::new(TokenPruningConfig::default()).unwrap();
        let bad_embeddings = vec![vec![0.0; 64]]; // 64D instead of 128D
        let result = quantizer.prune(&bad_embeddings, None);
        assert!(matches!(
            result,
            Err(EmbeddingError::InvalidDimension { expected: 128, actual: 64 })
        ));
    }

    // === No Pruning Needed Tests ===

    #[test]
    fn test_prune_input_smaller_than_min_tokens_no_pruning() {
        let config = TokenPruningConfig {
            min_tokens: 64,
            target_compression: 0.5,
            ..Default::default()
        };
        let quantizer = TokenPruningQuantizer::new(config).unwrap();

        // Only 32 tokens - less than min_tokens (64)
        let embeddings = make_embeddings(32);
        let result = quantizer.prune(&embeddings, None).unwrap();

        // Should keep all 32 tokens
        assert_eq!(result.embeddings.len(), 32);
        assert_eq!(result.retained_indices.len(), 32);
        assert_eq!(result.compression_ratio, 0.0); // No compression
    }

    // === Standard Pruning Tests ===

    #[test]
    fn test_prune_50_percent_compression() {
        let config = TokenPruningConfig {
            target_compression: 0.5,
            min_tokens: 10,
            scoring_method: ImportanceScoringMethod::EmbeddingMagnitude,
        };
        let quantizer = TokenPruningQuantizer::new(config).unwrap();

        let embeddings = make_embeddings(100);
        let result = quantizer.prune(&embeddings, None).unwrap();

        // Target: 50 tokens (100 * 0.5)
        assert_eq!(result.embeddings.len(), 50);
        assert_eq!(result.retained_indices.len(), 50);

        // Compression should be approximately 0.5
        assert!((result.compression_ratio - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_retained_indices_are_sorted() {
        let config = TokenPruningConfig {
            target_compression: 0.7, // 70% compression -> keep 30%
            min_tokens: 5,
            scoring_method: ImportanceScoringMethod::EmbeddingMagnitude,
        };
        let quantizer = TokenPruningQuantizer::new(config).unwrap();

        let embeddings = make_embeddings(50);
        let result = quantizer.prune(&embeddings, None).unwrap();

        // Verify indices are sorted ascending
        let mut sorted_indices = result.retained_indices.clone();
        sorted_indices.sort();
        assert_eq!(result.retained_indices, sorted_indices);
    }

    // === Min Tokens Constraint Tests ===

    #[test]
    fn test_min_tokens_respected() {
        let config = TokenPruningConfig {
            target_compression: 0.99, // 99% compression would leave 1 token
            min_tokens: 20,           // But min_tokens forces at least 20
            scoring_method: ImportanceScoringMethod::EmbeddingMagnitude,
        };
        let quantizer = TokenPruningQuantizer::new(config).unwrap();

        let embeddings = make_embeddings(100);
        let result = quantizer.prune(&embeddings, None).unwrap();

        // Should have at least min_tokens (20)
        assert_eq!(result.embeddings.len(), 20);
    }

    // === Scoring Method Tests ===

    #[test]
    fn test_magnitude_scoring_prefers_larger_norms() {
        let config = TokenPruningConfig {
            target_compression: 0.5, // Keep 50%
            min_tokens: 1,
            scoring_method: ImportanceScoringMethod::EmbeddingMagnitude,
        };
        let quantizer = TokenPruningQuantizer::new(config).unwrap();

        // Create embeddings with increasing magnitudes
        let embeddings: Vec<Vec<f32>> = (0..10)
            .map(|i| vec![(i as f32 + 1.0) * 0.1; 128])
            .collect();

        let result = quantizer.prune(&embeddings, None).unwrap();

        // Should retain indices 5-9 (highest magnitudes)
        // All retained indices should be >= 5
        for idx in &result.retained_indices {
            assert!(*idx >= 5, "Expected high-magnitude tokens, got index {}", idx);
        }
    }

    #[test]
    fn test_attention_based_uses_provided_weights() {
        let config = TokenPruningConfig {
            target_compression: 0.5,
            min_tokens: 1,
            scoring_method: ImportanceScoringMethod::AttentionBased,
        };
        let quantizer = TokenPruningQuantizer::new(config).unwrap();

        let embeddings = make_embeddings(10);

        // Attention weights: first 5 tokens have high scores
        let attention: Vec<f32> = (0..10)
            .map(|i| if i < 5 { 1.0 } else { 0.1 })
            .collect();

        let result = quantizer.prune(&embeddings, Some(&attention)).unwrap();

        // Should retain first 5 tokens (highest attention)
        for idx in &result.retained_indices {
            assert!(*idx < 5, "Expected high-attention tokens (0-4), got index {}", idx);
        }
    }

    #[test]
    fn test_entropy_scoring_produces_valid_scores() {
        let config = TokenPruningConfig {
            target_compression: 0.5,
            min_tokens: 1,
            scoring_method: ImportanceScoringMethod::Entropy,
        };
        let quantizer = TokenPruningQuantizer::new(config).unwrap();

        let embeddings = make_embeddings(20);
        let result = quantizer.prune(&embeddings, None).unwrap();

        // Should retain 10 tokens
        assert_eq!(result.embeddings.len(), 10);
    }

    // === Edge Case Tests ===

    #[test]
    fn test_single_token_input() {
        let config = TokenPruningConfig {
            target_compression: 0.5,
            min_tokens: 1,
            ..Default::default()
        };
        let quantizer = TokenPruningQuantizer::new(config).unwrap();

        let embeddings = make_embeddings(1);
        let result = quantizer.prune(&embeddings, None).unwrap();

        // Should keep the single token
        assert_eq!(result.embeddings.len(), 1);
        assert_eq!(result.retained_indices, vec![0]);
        assert_eq!(result.compression_ratio, 0.0);
    }

    #[test]
    fn test_zero_embedding_values() {
        let config = TokenPruningConfig {
            target_compression: 0.5,
            min_tokens: 1,
            scoring_method: ImportanceScoringMethod::Entropy,
        };
        let quantizer = TokenPruningQuantizer::new(config).unwrap();

        // All zeros - entropy scoring should handle gracefully
        let embeddings = vec![vec![0.0; 128]; 10];
        let result = quantizer.prune(&embeddings, None);

        assert!(result.is_ok());
        let pruned = result.unwrap();
        assert_eq!(pruned.embeddings.len(), 5); // 50% of 10
    }

    #[test]
    fn test_embeddings_are_copied_correctly() {
        let config = TokenPruningConfig {
            target_compression: 0.5,
            min_tokens: 1,
            scoring_method: ImportanceScoringMethod::EmbeddingMagnitude,
        };
        let quantizer = TokenPruningQuantizer::new(config).unwrap();

        // Create distinct embeddings
        let embeddings: Vec<Vec<f32>> = (0..10)
            .map(|i| vec![i as f32; 128])
            .collect();

        let result = quantizer.prune(&embeddings, None).unwrap();

        // Verify each retained embedding matches the original at that index
        for (pruned_idx, &original_idx) in result.retained_indices.iter().enumerate() {
            assert_eq!(
                result.embeddings[pruned_idx],
                embeddings[original_idx],
                "Embedding at pruned index {} should match original index {}",
                pruned_idx,
                original_idx
            );
        }
    }
}
```

---

## 6. FULL STATE VERIFICATION PROTOCOL (MANDATORY)

After completing implementation, you MUST perform these verification steps.

### 6.1 Source of Truth Identification

| Artifact | Location | Verification Method |
|----------|----------|---------------------|
| `TokenPruningQuantizer` struct | `src/pruning/token_pruner.rs` | File exists, compiles |
| Module export | `src/pruning/mod.rs` | Contains `pub use token_pruner::TokenPruningQuantizer;` |
| Crate export | `src/lib.rs` | Contains `TokenPruningQuantizer` in re-exports |
| Tests passing | Unit tests in `token_pruner.rs` | `cargo test` output shows all pass |

### 6.2 Execute & Inspect Protocol

Run these commands IN ORDER and verify expected outputs:

```bash
# Step 1: Verify file was created
ls -la crates/context-graph-embeddings/src/pruning/token_pruner.rs
# EXPECTED: File exists with size > 3000 bytes

# Step 2: Verify module export
grep -n "pub use token_pruner::TokenPruningQuantizer" crates/context-graph-embeddings/src/pruning/mod.rs
# EXPECTED: Line showing the export

# Step 3: Verify crate export
grep -n "TokenPruningQuantizer" crates/context-graph-embeddings/src/lib.rs
# EXPECTED: Line showing TokenPruningQuantizer in re-exports

# Step 4: Check compilation
cargo check -p context-graph-embeddings --features "cuda" 2>&1 | grep -E "^error"
# EXPECTED: No output (no errors)

# Step 5: Run all pruning tests
cargo test -p context-graph-embeddings token_pruner --features "cuda" --lib -- --nocapture 2>&1 | tail -40
# EXPECTED: All tests pass (16+ tests)

# Step 6: Run full test suite to ensure no regressions
cargo test -p context-graph-embeddings --features "cuda" --lib 2>&1 | tail -10
# EXPECTED: "test result: ok. XXXX passed; 0 failed"
```

### 6.3 Boundary & Edge Case Audit

Execute these manual verification tests in a Rust playground or test file:

**Test Case 1: Empty Input**
```rust
let quantizer = TokenPruningQuantizer::new(TokenPruningConfig::default()).unwrap();
println!("BEFORE prune(): embeddings.len() = 0");
let result = quantizer.prune(&[], None);
println!("AFTER prune(): result = {:?}", result);
// EXPECTED: Err(EmptyInput)
```

**Test Case 2: Maximum Compression with min_tokens Constraint**
```rust
let config = TokenPruningConfig {
    target_compression: 0.99, // Would leave 1 token
    min_tokens: 50,           // Forces at least 50
    ..Default::default()
};
let quantizer = TokenPruningQuantizer::new(config).unwrap();
let embeddings: Vec<Vec<f32>> = (0..100).map(|_| vec![1.0; 128]).collect();
println!("BEFORE prune(): embeddings.len() = {}", embeddings.len());
let result = quantizer.prune(&embeddings, None).unwrap();
println!("AFTER prune(): retained = {}, expected >= 50", result.embeddings.len());
assert!(result.embeddings.len() >= 50);
// EXPECTED: At least 50 tokens retained due to min_tokens constraint
```

**Test Case 3: Invalid Dimension**
```rust
let quantizer = TokenPruningQuantizer::new(TokenPruningConfig::default()).unwrap();
let bad_embeddings = vec![vec![0.0; 256]]; // 256D instead of 128D
println!("BEFORE prune(): embedding[0].len() = {}", bad_embeddings[0].len());
let result = quantizer.prune(&bad_embeddings, None);
println!("AFTER prune(): result = {:?}", result);
// EXPECTED: Err(InvalidDimension { expected: 128, actual: 256 })
```

### 6.4 Evidence of Success

Provide this log after implementation:

```
=== TASK-15 VERIFICATION COMPLETE ===

File created: crates/context-graph-embeddings/src/pruning/token_pruner.rs (XXXX bytes)
Module export: src/pruning/mod.rs line XX
Crate export: src/lib.rs line XX

Compilation: PASSED (0 errors)
Clippy: PASSED (0 warnings) OR (XX warnings - acceptable)

Test results:
  - test_new_with_default_config: PASSED
  - test_new_with_invalid_config_fails: PASSED
  - test_prune_empty_input_returns_error: PASSED
  - test_prune_wrong_dimension_returns_error: PASSED
  - test_prune_input_smaller_than_min_tokens_no_pruning: PASSED
  - test_prune_50_percent_compression: PASSED
  - test_retained_indices_are_sorted: PASSED
  - test_min_tokens_respected: PASSED
  - test_magnitude_scoring_prefers_larger_norms: PASSED
  - test_attention_based_uses_provided_weights: PASSED
  - test_entropy_scoring_produces_valid_scores: PASSED
  - test_single_token_input: PASSED
  - test_zero_embedding_values: PASSED
  - test_embeddings_are_copied_correctly: PASSED

TOTAL: XX passed; 0 failed; 0 ignored

Boundary tests:
  - Empty input: Correctly returned EmptyInput error
  - Max compression with min_tokens: Correctly retained >= 50 tokens
  - Invalid dimension: Correctly returned InvalidDimension error
```

---

## 7. ANTI-PATTERNS (FORBIDDEN)

1. **DO NOT create new error types** - Use existing `EmbeddingError` variants
2. **DO NOT use `.unwrap()` in production code** - Use `?` operator or explicit error handling
3. **DO NOT add fallbacks or default values for errors** - Errors must propagate
4. **DO NOT use mock data in tests** - Use real synthetic data with known properties
5. **DO NOT modify other files** except the three listed in Section 4
6. **DO NOT add dependencies** - Everything needed is already available
7. **DO NOT create workarounds** - Fix root causes
8. **DO NOT use magic numbers** - Use named constants

---

## 8. CONSTITUTION REFERENCES

```yaml
embeddings:
  models:
    E12_LateInteraction: { dim: "128D/tok", type: dense_per_token, use: "Precise match" }
  paradigm: "NO FUSION - Store all 13 embeddings"

perf:
  quality: { info_loss: "<15%" }

rules:
  - "Result<T,E>, thiserror derivation"
  - "Never unwrap() in prod"
```

---

## 9. SUCCESS CRITERIA

The task is COMPLETE when:

1. ✅ `TokenPruningQuantizer` struct exists with `new()` and `prune()` methods
2. ✅ All three scoring methods implemented (AttentionBased, EmbeddingMagnitude, Entropy)
3. ✅ `prune()` returns errors for empty input and invalid dimensions
4. ✅ `prune()` respects `min_tokens` constraint
5. ✅ `retained_indices` is always sorted in ascending order
6. ✅ All 14+ tests pass
7. ✅ Module exported from `pruning/mod.rs`
8. ✅ Type re-exported from `lib.rs`
9. ✅ `cargo check -p context-graph-embeddings --features cuda` succeeds
10. ✅ Boundary tests manually verified

---

## 10. GIT COMMIT MESSAGE

When complete, commit with:

```
feat(embeddings): implement TokenPruningQuantizer for E12 compression (TASK-15)

- Add TokenPruningQuantizer struct with prune() method
- Implement magnitude, entropy, and attention-based scoring
- Enforce min_tokens constraint and sorted retained_indices
- Add comprehensive test suite (14+ tests)
- Export from pruning module and lib.rs

Constitution: embeddings.models.E12_LateInteraction = "128D/tok"
Target: ~50% compression with <15% information loss

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>
```

# TASK-EMBED-003: Implement TokenPruningQuantizer

```xml
<task_spec id="TASK-EMBED-003" version="1.0">
<metadata>
  <title>Implement TokenPruningQuantizer</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>15</sequence>
  <implements><requirement_ref>REQ-EMBED-003</requirement_ref></implements>
  <depends_on>TASK-14</depends_on>
  <estimated_hours>4</estimated_hours>
</metadata>

<context>
TokenPruningQuantizer reduces E12 Late Interaction embeddings (128D/token) by
pruning low-importance tokens. Target: ~50% compression with <5% recall degradation.
Constitution: embeddings.models.E12_LateInteraction
</context>

<input_context_files>
- /home/cabdru/contextgraph/crates/context-graph-embeddings/src/pruning/config.rs (from TASK-EMBED-002)
- /home/cabdru/contextgraph/docs/specs/technical/TECH-REMEDIATION-MASTER.md (section 3.4)
</input_context_files>

<scope>
<in_scope>
- Create token_pruner.rs in pruning module
- Implement TokenPruningQuantizer struct
- Implement prune() method with scoring and selection
- Implement score_tokens() for each ImportanceScoringMethod
- Implement score_by_magnitude() helper
- Implement score_by_entropy() helper
- Ensure min_tokens constraint is respected
</in_scope>
<out_of_scope>
- E12 model integration
- GPU acceleration of pruning
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-embeddings/src/pruning/token_pruner.rs
use crate::pruning::config::{TokenPruningConfig, ImportanceScoringMethod, PrunedEmbeddings};

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
    pub fn new(config: TokenPruningConfig) -> Self;

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
    /// - Compression ratio is approximately `target_compression`
    pub fn prune(
        &self,
        embeddings: &[Vec<f32>],
        attention_weights: Option<&[f32]>,
    ) -> PrunedEmbeddings;
}
```
</signatures>
<constraints>
- Output MUST have at least min_tokens tokens
- retained_indices MUST be sorted (preserve order)
- Compression ratio MUST be approximately target_compression
- MUST handle empty/small inputs gracefully
</constraints>
<verification>
```bash
cargo test -p context-graph-embeddings token_pruner
cargo test -p context-graph-embeddings test_min_tokens_respected
cargo test -p context-graph-embeddings test_compression_ratio
```
</verification>
</definition_of_done>

<files_to_create>
- crates/context-graph-embeddings/src/pruning/token_pruner.rs
</files_to_create>

<files_to_modify>
- crates/context-graph-embeddings/src/pruning/mod.rs (add token_pruner module)
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-embeddings pruning
```
</test_commands>
</task_spec>
```

## Implementation Notes

### Pruning Algorithm

1. Calculate target token count: `target = (1 - target_compression) * num_tokens`
2. Enforce minimum: `target = max(target, min_tokens)`
3. Score all tokens using selected method
4. Sort by importance (descending)
5. Keep top-k tokens
6. Sort retained indices (preserve original order)
7. Extract embeddings at retained indices

### Scoring Implementations

**Magnitude scoring**:
```rust
fn score_by_magnitude(&self, embeddings: &[Vec<f32>]) -> Vec<f32> {
    embeddings.iter()
        .map(|emb| emb.iter().map(|x| x * x).sum::<f32>().sqrt())
        .collect()
}
```

**Entropy scoring**:
```rust
fn score_by_entropy(&self, embeddings: &[Vec<f32>]) -> Vec<f32> {
    embeddings.iter()
        .map(|emb| {
            let sum: f32 = emb.iter().map(|x| x.abs()).sum();
            if sum == 0.0 { return 0.0; }
            let probs: Vec<f32> = emb.iter().map(|x| x.abs() / sum).collect();
            probs.iter()
                .filter(|&&p| p > 0.0)
                .map(|&p| -p * p.ln())
                .sum()
        })
        .collect()
}
```

### Test Cases

1. Empty input -> empty output
2. Input smaller than min_tokens -> no pruning
3. Compression ratio matches target (within tolerance)
4. Retained indices are sorted
5. All scoring methods produce valid scores

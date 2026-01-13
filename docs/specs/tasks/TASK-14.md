# TASK-EMBED-002: Implement TokenPruningConfig types

```xml
<task_spec id="TASK-EMBED-002" version="1.0">
<metadata>
  <title>Implement TokenPruningConfig types</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>14</sequence>
  <implements><requirement_ref>REQ-EMBED-002</requirement_ref></implements>
  <depends_on></depends_on>
  <estimated_hours>2</estimated_hours>
</metadata>

<context>
Token pruning reduces E12 Late Interaction embedding size by ~50% while maintaining
recall quality. This task defines the configuration types and scoring methods
before implementing the actual quantizer.
</context>

<input_context_files>
- /home/cabdru/contextgraph/docs/specs/technical/TECH-REMEDIATION-MASTER.md (section 3.4)
</input_context_files>

<scope>
<in_scope>
- Create pruning module in context-graph-embeddings
- Define TokenPruningConfig struct
- Define ImportanceScoringMethod enum
- Define PrunedEmbeddings result struct
- Add documentation explaining each field
</in_scope>
<out_of_scope>
- Actual pruning implementation (TASK-EMBED-003)
- Integration with E12 model
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-embeddings/src/pruning/config.rs

/// Configuration for token pruning.
#[derive(Debug, Clone)]
pub struct TokenPruningConfig {
    /// Target compression ratio (default: 0.5 = 50% compression)
    /// Range: (0.0, 1.0)
    pub target_compression: f32,

    /// Minimum tokens to retain (default: 64)
    /// Prevents over-pruning short sequences
    pub min_tokens: usize,

    /// Importance scoring method
    pub scoring_method: ImportanceScoringMethod,
}

impl Default for TokenPruningConfig {
    fn default() -> Self {
        Self {
            target_compression: 0.5,
            min_tokens: 64,
            scoring_method: ImportanceScoringMethod::AttentionBased,
        }
    }
}

impl TokenPruningConfig {
    /// Validate configuration.
    ///
    /// # Errors
    /// Returns error if target_compression not in (0.0, 1.0)
    pub fn validate(&self) -> Result<(), ConfigError>;
}

/// Method for scoring token importance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportanceScoringMethod {
    /// Use attention weights from transformer layers
    AttentionBased,

    /// Use L2 norm of token embeddings
    EmbeddingMagnitude,

    /// Use entropy of token probability distribution
    Entropy,
}

/// Result of token pruning operation.
#[derive(Debug, Clone)]
pub struct PrunedEmbeddings {
    /// Pruned token embeddings
    pub embeddings: Vec<Vec<f32>>,

    /// Indices of retained tokens in original sequence
    pub retained_indices: Vec<usize>,

    /// Achieved compression ratio [0, 1]
    pub compression_ratio: f32,
}
```
</signatures>
<constraints>
- target_compression MUST be in (0.0, 1.0) exclusive
- min_tokens MUST be at least 1
- Default compression MUST be 0.5 (50%)
- Default min_tokens MUST be 64
</constraints>
<verification>
```bash
cargo check -p context-graph-embeddings
cargo test -p context-graph-embeddings pruning_config
```
</verification>
</definition_of_done>

<files_to_create>
- crates/context-graph-embeddings/src/pruning/mod.rs
- crates/context-graph-embeddings/src/pruning/config.rs
</files_to_create>

<files_to_modify>
- crates/context-graph-embeddings/src/lib.rs (add pruning module)
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-embeddings pruning
```
</test_commands>
</task_spec>
```

## Implementation Notes

### Scoring Method Rationale

**AttentionBased** (default):
- Uses transformer attention weights
- Most accurate for identifying important tokens
- Requires access to attention during embedding

**EmbeddingMagnitude**:
- Uses L2 norm of embedding vector
- Fast, no additional data needed
- Less accurate but good fallback

**Entropy**:
- Measures information content
- Good for diverse token selection
- Moderate computational cost

### Validation Logic

```rust
impl TokenPruningConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.target_compression <= 0.0 || self.target_compression >= 1.0 {
            return Err(ConfigError::InvalidCompression(self.target_compression));
        }
        if self.min_tokens < 1 {
            return Err(ConfigError::InvalidMinTokens(self.min_tokens));
        }
        Ok(())
    }
}
```

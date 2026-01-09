# TASK-CORE-004: Define Comparison Types

```xml
<task_spec id="TASK-CORE-004" version="1.0">
<metadata>
  <title>Define Comparison Types and Search Matrices</title>
  <status>todo</status>
  <layer>foundation</layer>
  <sequence>4</sequence>
  <implements>
    <requirement_ref>REQ-COMPARISON-01</requirement_ref>
    <requirement_ref>REQ-SEARCH-MATRIX-01</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORE-003</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_days>1.5</estimated_days>
</metadata>

<context>
Defines the different strategies for comparing teleological arrays. This includes
single-embedder comparison, weighted combinations, and full 13x13 matrix strategies.
These types are used by the TeleologicalComparator (TASK-LOGIC-004) and all search
operations.
</context>

<objective>
Create the ComparisonType enum, SearchMatrix type, and ComparisonResult struct that
define how teleological arrays are compared against each other.
</objective>

<rationale>
Multiple comparison strategies are needed for different use cases:
1. SingleEmbedder: Fast, when you know which dimension matters
2. EmbedderGroup: Compare subset of related embedders (temporal, lexical)
3. WeightedFull: Custom weights across all 13 embedders
4. MatrixStrategy: Full 13x13 matrix for cross-embedder correlations

Predefined matrices encode domain knowledge (semantic_focused, code_heavy, etc.)
while allowing custom matrices for advanced use cases.
</rationale>

<input_context_files>
  <file purpose="embedder_enum">crates/context-graph-core/src/teleology/embedder.rs</file>
  <file purpose="array_type">crates/context-graph-core/src/teleology/array.rs</file>
  <file purpose="mcp_comparison_schema">docs2/refactor/08-MCP-TOOLS.md#comparison-type-schema</file>
  <file purpose="matrix_definitions">docs2/refactor/08-MCP-TOOLS.md#predefined-search-matrices</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORE-003 complete (TeleologicalArray exists)</check>
  <check>Embedder enum exists with all 13 variants</check>
</prerequisites>

<scope>
  <in_scope>
    <item>Create ComparisonType enum with 4 strategies</item>
    <item>Create SearchMatrix type (13x13 weights)</item>
    <item>Create ComparisonResult struct with per-embedder scores</item>
    <item>Implement 8 predefined matrices</item>
    <item>Add matrix validation (weights sum, non-negative)</item>
    <item>Create EmbedderWeights type for weighted comparison</item>
  </in_scope>
  <out_of_scope>
    <item>Actual similarity computation (TASK-LOGIC-001 through 004)</item>
    <item>Search engine implementation (TASK-LOGIC-005 through 008)</item>
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/teleology/comparison.rs">
      use crate::teleology::embedder::{Embedder, EmbedderGroup, EmbedderMask};

      /// Strategy for comparing teleological arrays.
      #[derive(Debug, Clone, PartialEq)]
      pub enum ComparisonType {
          /// Compare using a single embedder dimension
          SingleEmbedder(Embedder),
          /// Compare using a predefined group of embedders
          EmbedderGroup(EmbedderGroup),
          /// Compare with custom weights per embedder
          WeightedFull(EmbedderWeights),
          /// Compare with full 13x13 matrix (enables cross-correlations)
          MatrixStrategy(SearchMatrix),
      }

      /// Per-embedder weights for weighted comparison.
      #[derive(Debug, Clone, PartialEq)]
      pub struct EmbedderWeights {
          weights: [f32; 13],
      }

      impl EmbedderWeights {
          pub fn new(weights: [f32; 13]) -> Result<Self, ValidationError>;
          pub fn uniform() -> Self;
          pub fn get(&self, embedder: Embedder) -> f32;
          pub fn active_embedders(&self) -> EmbedderMask;
          pub fn normalize(&mut self);
      }

      /// 13x13 weight matrix for cross-embedder correlations.
      #[derive(Debug, Clone, PartialEq)]
      pub struct SearchMatrix {
          weights: [[f32; 13]; 13],
          name: Option<String>,
      }

      impl SearchMatrix {
          pub fn new(weights: [[f32; 13]; 13]) -> Result<Self, ValidationError>;
          pub fn identity() -> Self;
          pub fn semantic_focused() -> Self;
          pub fn temporal_aware() -> Self;
          pub fn knowledge_graph() -> Self;
          pub fn emotional_resonance() -> Self;
          pub fn precision_retrieval() -> Self;
          pub fn correlation_aware() -> Self;
          pub fn code_heavy() -> Self;
          pub fn get(&self, row: Embedder, col: Embedder) -> f32;
          pub fn validate(&self) -> Result<(), ValidationError>;
      }

      /// Result of comparing two teleological arrays.
      #[derive(Debug, Clone)]
      pub struct ComparisonResult {
          pub overall_similarity: f32,
          pub per_embedder: [Option<f32>; 13],
          pub comparison_type: ComparisonType,
          pub coherence: Option<f32>,
          pub dominant_embedder: Option<Embedder>,
      }

      #[derive(Debug, Clone, thiserror::Error)]
      pub enum ValidationError {
          #[error("Weights must sum to 1.0, got {0}")]
          WeightsSumInvalid(f32),
          #[error("Negative weight at embedder {0}")]
          NegativeWeight(Embedder),
          #[error("Matrix diagonal must be non-negative")]
          NegativeDiagonal,
      }
    </signature>
  </signatures>

  <constraints>
    <constraint>Weights in EmbedderWeights must sum to 1.0 (within epsilon)</constraint>
    <constraint>No negative weights allowed</constraint>
    <constraint>Matrix diagonal must be non-negative</constraint>
    <constraint>All predefined matrices must pass validation</constraint>
    <constraint>Implements Serialize, Deserialize</constraint>
  </constraints>

  <verification>
    <command>cargo check -p context-graph-core</command>
    <command>cargo test -p context-graph-core comparison</command>
    <command>cargo test -p context-graph-core matrix_validation</command>
  </verification>
</definition_of_done>

<pseudo_code>
// crates/context-graph-core/src/teleology/comparison.rs

use crate::teleology::embedder::{Embedder, EmbedderGroup, EmbedderMask};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ComparisonType {
    SingleEmbedder(Embedder),
    EmbedderGroup(EmbedderGroup),
    WeightedFull(EmbedderWeights),
    MatrixStrategy(SearchMatrix),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbedderWeights {
    weights: [f32; 13],
}

impl EmbedderWeights {
    const EPSILON: f32 = 0.001;

    pub fn new(weights: [f32; 13]) -> Result<Self, ValidationError> {
        // Check no negative weights
        for (i, &w) in weights.iter().enumerate() {
            if w < 0.0 {
                return Err(ValidationError::NegativeWeight(
                    Embedder::from_index(i).unwrap()
                ));
            }
        }
        // Check sum to 1.0
        let sum: f32 = weights.iter().sum();
        if (sum - 1.0).abs() > Self::EPSILON {
            return Err(ValidationError::WeightsSumInvalid(sum));
        }
        Ok(Self { weights })
    }

    pub fn uniform() -> Self {
        Self { weights: [1.0 / 13.0; 13] }
    }

    pub fn get(&self, embedder: Embedder) -> f32 {
        self.weights[embedder.index()]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchMatrix {
    weights: [[f32; 13]; 13],
    name: Option<String>,
}

impl SearchMatrix {
    pub fn identity() -> Self {
        let mut weights = [[0.0; 13]; 13];
        for i in 0..13 {
            weights[i][i] = 1.0 / 13.0;
        }
        Self { weights, name: Some("identity".to_string()) }
    }

    pub fn semantic_focused() -> Self {
        let mut weights = [[0.0; 13]; 13];
        // 50% on E1 semantic
        weights[0][0] = 0.50;
        // Distribute rest across others
        for i in 1..13 {
            weights[i][i] = 0.50 / 12.0;
        }
        Self { weights, name: Some("semantic_focused".to_string()) }
    }

    pub fn code_heavy() -> Self {
        let mut weights = [[0.0; 13]; 13];
        // Heavy on E7 code
        weights[6][6] = 0.40;  // E7
        weights[0][0] = 0.25;  // E1
        weights[4][4] = 0.15;  // E5 causal
        // Rest distributed
        let remaining = 0.20 / 10.0;
        for i in [1, 2, 3, 5, 7, 8, 9, 10, 11, 12] {
            weights[i][i] = remaining;
        }
        Self { weights, name: Some("code_heavy".to_string()) }
    }

    // Additional predefined matrices...
}

#[derive(Debug, Clone)]
pub struct ComparisonResult {
    pub overall_similarity: f32,
    pub per_embedder: [Option<f32>; 13],
    pub comparison_type: ComparisonType,
    pub coherence: Option<f32>,
    pub dominant_embedder: Option<Embedder>,
}

#[derive(Debug, Clone, Error)]
pub enum ValidationError {
    #[error("Weights must sum to 1.0, got {0}")]
    WeightsSumInvalid(f32),
    #[error("Negative weight at embedder {0:?}")]
    NegativeWeight(Embedder),
    #[error("Matrix diagonal must be non-negative")]
    NegativeDiagonal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uniform_weights_valid() {
        let w = EmbedderWeights::uniform();
        let sum: f32 = (0..13).map(|i| w.weights[i]).sum();
        assert!((sum - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_identity_matrix_valid() {
        let m = SearchMatrix::identity();
        assert!(m.validate().is_ok());
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/teleology/comparison.rs">
    ComparisonType, SearchMatrix, EmbedderWeights, ComparisonResult
  </file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/teleology/mod.rs">
    Add: pub mod comparison;
  </file>
  <file path="crates/context-graph-core/Cargo.toml">
    Add: thiserror dependency
  </file>
</files_to_modify>

<validation_criteria>
  <criterion>EmbedderWeights::uniform() weights sum to 1.0</criterion>
  <criterion>All 8 predefined matrices pass validation</criterion>
  <criterion>Invalid weights rejected with appropriate error</criterion>
  <criterion>ComparisonResult stores per-embedder scores</criterion>
  <criterion>SearchMatrix::get() returns correct cell value</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-core comparison -- --nocapture</command>
  <command>cargo test -p context-graph-core matrix -- --nocapture</command>
</test_commands>
</task_spec>
```

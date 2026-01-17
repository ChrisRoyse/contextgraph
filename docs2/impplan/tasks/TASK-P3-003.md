# TASK-P3-003: Threshold Configurations

```xml
<task_spec id="TASK-P3-003" version="2.0">
<metadata>
  <title>Threshold Configuration Implementation</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>22</sequence>
  <phase>3</phase>
  <implements>
    <requirement_ref>REQ-P3-02</requirement_ref>
    <requirement_ref>REQ-P3-03</requirement_ref>
  </implements>
  <depends_on>
    <task_ref status="COMPLETE">TASK-P3-001</task_ref>
    <task_ref status="COMPLETE">TASK-P3-002</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
</metadata>

<context>
Implements threshold configuration types for similarity and divergence detection.
This task creates config.rs in the retrieval module with:
- PerSpaceThresholds: Per-embedder threshold values for all 13 spaces
- SpaceWeights: Category-based weights for weighted similarity calculation
- SimilarityThresholds: Container for high (relevance) and low (divergence) thresholds
- Static constructor functions with values from TECH-PHASE3-SIMILARITY-DIVERGENCE.md

COMPLETED DEPENDENCIES:
- TASK-P3-001: PerSpaceScores and SimilarityResult in similarity.rs (COMPLETE)
- TASK-P3-002: DivergenceAlert and DivergenceReport in divergence.rs (COMPLETE)
</context>

<source_of_truth>
PRIMARY SPECIFICATIONS:
  - Technical Spec: docs2/impplan/technical/TECH-PHASE3-SIMILARITY-DIVERGENCE.md#static_configuration
  - Constitution: CLAUDE.md (topic_system, embedder_categories, weighted_agreement)

EXISTING CODEBASE (READ BEFORE IMPLEMENTING):
  - Embedder enum: crates/context-graph-core/src/teleological/embedder.rs (lines 62-110)
  - EmbedderCategory: crates/context-graph-core/src/embeddings/category.rs
  - PerSpaceScores: crates/context-graph-core/src/retrieval/similarity.rs (lines 32-60)
  - DIVERGENCE_SPACES: crates/context-graph-core/src/retrieval/divergence.rs (lines 22-30)
  - retrieval/mod.rs: crates/context-graph-core/src/retrieval/mod.rs

EMBEDDER ENUM VARIANTS (EXACT NAMES - use these in match statements):
  Embedder::Semantic           (E1, index 0)
  Embedder::TemporalRecent     (E2, index 1)
  Embedder::TemporalPeriodic   (E3, index 2)
  Embedder::TemporalPositional (E4, index 3)
  Embedder::Causal             (E5, index 4)
  Embedder::Sparse             (E6, index 5)
  Embedder::Code               (E7, index 6)
  Embedder::Emotional          (E8, index 7) - NOT "Graph"
  Embedder::Hdc                (E9, index 8)
  Embedder::Multimodal         (E10, index 9)
  Embedder::Entity             (E11, index 10)
  Embedder::LateInteraction    (E12, index 11)
  Embedder::KeywordSplade      (E13, index 12)
</source_of_truth>

<input_context_files>
  <file purpose="threshold_values">docs2/impplan/technical/TECH-PHASE3-SIMILARITY-DIVERGENCE.md#static_configuration (lines 488-568)</file>
  <file purpose="embedder_definition">crates/context-graph-core/src/teleological/embedder.rs</file>
  <file purpose="category_system">crates/context-graph-core/src/embeddings/category.rs</file>
  <file purpose="existing_similarity">crates/context-graph-core/src/retrieval/similarity.rs</file>
</input_context_files>

<prerequisites>
  <check verified="true">Embedder enum exists at crates/context-graph-core/src/teleological/embedder.rs with 13 variants</check>
  <check verified="true">EmbedderCategory exists at crates/context-graph-core/src/embeddings/category.rs with topic_weight()</check>
  <check verified="true">PerSpaceScores exists at crates/context-graph-core/src/retrieval/similarity.rs</check>
  <check verified="true">DIVERGENCE_SPACES exists at crates/context-graph-core/src/retrieval/divergence.rs</check>
</prerequisites>

<scope>
  <in_scope>
    - Create PerSpaceThresholds struct with 13 f32 fields matching PerSpaceScores field names
    - Create SpaceWeights struct wrapping [f32; 13] array
    - Create SimilarityThresholds container with high/low PerSpaceThresholds
    - Implement high_thresholds() returning exact values from TECH-PHASE3
    - Implement low_thresholds() returning exact values from TECH-PHASE3
    - Implement default_weights() returning category-based weights
    - Implement get/set methods using Embedder enum
    - Implement normalization for SpaceWeights
    - Add constants RECENT_LOOKBACK_SECS and MAX_RECENT_MEMORIES
    - Derive Clone, Debug, Serialize, Deserialize, PartialEq
  </in_scope>
  <out_of_scope>
    - Threshold comparison logic (TASK-P3-005)
    - Runtime threshold adjustment
    - Threshold persistence to storage
    - Adaptive threshold calibration
  </out_of_scope>
</scope>

<architecture_rules>
  MUST COMPLY:
  - ARCH-09: Topic threshold is weighted_agreement >= 2.5
  - AP-60: Temporal embedders (E2-E4) weight = 0.0
  - AP-61: Topic threshold MUST be weighted_agreement >= 2.5
  - AP-10: No NaN/Infinity in similarity scores (clamp all values)

  CATEGORY WEIGHTS (from constitution):
    Semantic (E1, E5, E6, E7, E10, E12, E13): 1.0
    Temporal (E2, E3, E4): 0.0 (excluded)
    Relational (E8, E11): 0.5
    Structural (E9): 0.5
    MAX_WEIGHTED_AGREEMENT = 7*1.0 + 2*0.5 + 1*0.5 = 8.5
</architecture_rules>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/retrieval/config.rs">
/// Per-space threshold values for all 13 embedding spaces.
/// Field names MUST match PerSpaceScores field names exactly.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerSpaceThresholds {
    pub semantic: f32,           // E1
    pub temporal_recent: f32,    // E2
    pub temporal_periodic: f32,  // E3
    pub temporal_positional: f32,// E4
    pub causal: f32,             // E5
    pub sparse: f32,             // E6
    pub code: f32,               // E7
    pub emotional: f32,          // E8
    pub hdc: f32,                // E9
    pub multimodal: f32,         // E10
    pub entity: f32,             // E11
    pub late_interaction: f32,   // E12
    pub keyword_splade: f32,     // E13
}

impl PerSpaceThresholds {
    pub fn get_threshold(&amp;self, embedder: Embedder) -> f32;
    pub fn set_threshold(&amp;mut self, embedder: Embedder, threshold: f32);
    pub fn to_array(&amp;self) -> [f32; 13];
    pub fn from_array(arr: [f32; 13]) -> Self;
}

/// Category-based weights for similarity calculation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpaceWeights {
    weights: [f32; 13],
}

impl SpaceWeights {
    pub fn new(weights: [f32; 13]) -> Self;
    pub fn get_weight(&amp;self, embedder: Embedder) -> f32;
    pub fn set_weight(&amp;mut self, embedder: Embedder, weight: f32);
    pub fn normalize(&amp;mut self);
    pub fn normalized(&amp;self) -> Self;
    pub fn sum(&amp;self) -> f32;
    pub fn as_slice(&amp;self) -> &amp;[f32; 13];
}

/// Container for both high and low thresholds.
#[derive(Debug, Clone)]
pub struct SimilarityThresholds {
    pub high: PerSpaceThresholds,
    pub low: PerSpaceThresholds,
}

impl Default for SimilarityThresholds {
    fn default() -> Self;
}

impl SimilarityThresholds {
    pub fn new(high: PerSpaceThresholds, low: PerSpaceThresholds) -> Self;
}

/// HIGH_THRESHOLDS from TECH-PHASE3 spec (for relevance detection)
pub fn high_thresholds() -> PerSpaceThresholds;

/// LOW_THRESHOLDS from TECH-PHASE3 spec (for divergence detection)
pub fn low_thresholds() -> PerSpaceThresholds;

/// Category-based weights from constitution
pub fn default_weights() -> SpaceWeights;

/// Lookback duration for divergence detection (2 hours)
pub const RECENT_LOOKBACK_SECS: u64 = 2 * 60 * 60;

/// Maximum recent memories for divergence check
pub const MAX_RECENT_MEMORIES: usize = 50;
    </signature>
  </signatures>

  <constraints>
    - All threshold values MUST be in 0.0..=1.0 range (clamp in set_threshold)
    - All high threshold values MUST be > corresponding low threshold values
    - Weights MUST be >= 0.0 (clamp negative to 0.0)
    - Weight normalization produces sum = 13.0
    - Field names in PerSpaceThresholds MUST match PerSpaceScores field names exactly
    - Match statement arms MUST use exact Embedder variant names from embedder.rs
    - NO use of Embedder::index() in match statements - use explicit match arms
  </constraints>

  <verification>
    - Static configs have correct values from spec (see EXACT_VALUES below)
    - get/set work for all 13 embedders
    - Weight normalization produces sum ≈ 13.0 (tolerance 0.001)
    - All high thresholds > corresponding low thresholds
    - Serialization roundtrip works for all types
    - Threshold clamping enforces [0.0, 1.0] range
  </verification>
</definition_of_done>

<exact_values>
HIGH_THRESHOLDS (from TECH-PHASE3-SIMILARITY-DIVERGENCE.md lines 489-502):
  semantic: 0.75
  temporal_recent: 0.70
  temporal_periodic: 0.70
  temporal_positional: 0.70
  causal: 0.70
  sparse: 0.60
  code: 0.80
  emotional: 0.70
  hdc: 0.70
  multimodal: 0.70
  entity: 0.70
  late_interaction: 0.70
  keyword_splade: 0.60

LOW_THRESHOLDS (from TECH-PHASE3-SIMILARITY-DIVERGENCE.md lines 505-518):
  semantic: 0.30
  temporal_recent: 0.30
  temporal_periodic: 0.30
  temporal_positional: 0.30
  causal: 0.25
  sparse: 0.20
  code: 0.35
  emotional: 0.30
  hdc: 0.30
  multimodal: 0.30
  entity: 0.30
  late_interaction: 0.30
  keyword_splade: 0.20

SPACE_WEIGHTS (from TECH-PHASE3-SIMILARITY-DIVERGENCE.md lines 548-562):
  [1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.5, 0.5, 1.0, 0.5, 1.0, 1.0]
  // E1=1.0, E2=0.0, E3=0.0, E4=0.0, E5=1.0, E6=1.0, E7=1.0,
  // E8=0.5, E9=0.5, E10=1.0, E11=0.5, E12=1.0, E13=1.0
  // Sum = 8.5 (7 semantic×1.0 + 2 relational×0.5 + 1 structural×0.5)
</exact_values>

<boundary_edge_cases>
EDGE CASE 1: Threshold clamping
  Input: set_threshold(Embedder::Semantic, 1.5)
  Expected: threshold stored as 1.0 (clamped)
  Input: set_threshold(Embedder::Code, -0.3)
  Expected: threshold stored as 0.0 (clamped)

EDGE CASE 2: Weight normalization with zero weights
  Input: SpaceWeights with temporal weights = 0.0
  Expected: normalize() produces sum = 13.0 without dividing by zero
  Verify: Temporal weights remain 0.0 after normalization

EDGE CASE 3: High vs Low threshold invariant
  Verify: For all 13 embedders, high_thresholds().get_threshold(e) > low_thresholds().get_threshold(e)
  This is a CRITICAL invariant that must be verified in tests

EDGE CASE 4: Empty/default construction
  Input: SimilarityThresholds::default()
  Expected: high = high_thresholds(), low = low_thresholds()
  Verify: Not all zeros - uses spec values
</boundary_edge_cases>

<evidence_of_success>
Running tests MUST produce output showing:

1. Exact spec values verified:
   [PASS] high_thresholds().semantic == 0.75
   [PASS] high_thresholds().code == 0.80
   [PASS] low_thresholds().semantic == 0.30
   [PASS] low_thresholds().causal == 0.25

2. All high > low for every embedder:
   [PASS] Semantic: high 0.75 > low 0.30
   [PASS] Code: high 0.80 > low 0.35
   ... (all 13 verified)

3. Weight sum verification:
   [PASS] default_weights().sum() == 8.5 (before normalization)
   [PASS] normalized weights sum ≈ 13.0

4. Clamping verification:
   [PASS] set_threshold clamps 1.5 -> 1.0
   [PASS] set_threshold clamps -0.3 -> 0.0

5. Serialization roundtrip:
   [PASS] PerSpaceThresholds JSON roundtrip
   [PASS] SpaceWeights JSON roundtrip
</evidence_of_success>

<manual_verification>
After implementation, manually verify:

1. cargo check --package context-graph-core compiles without errors
2. cargo test --package context-graph-core config runs all tests
3. Test output shows [PASS] for all verification points
4. No clippy warnings: cargo clippy --package context-graph-core -- -D warnings
5. Verify mod.rs exports: pub mod config; and pub use config::*;
</manual_verification>

<files_to_create>
  <file path="crates/context-graph-core/src/retrieval/config.rs">
    Threshold and weight configurations with exact values from spec
  </file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/retrieval/mod.rs">
    Add: pub mod config;
    Add to pub use: config::*;
  </file>
</files_to_modify>

<validation_criteria>
  <criterion>HIGH_THRESHOLDS values match spec exactly (13 values)</criterion>
  <criterion>LOW_THRESHOLDS values match spec exactly (13 values)</criterion>
  <criterion>All high thresholds > corresponding low thresholds</criterion>
  <criterion>SPACE_WEIGHTS values match spec exactly (13 values)</criterion>
  <criterion>Weight sum before normalization = 8.5</criterion>
  <criterion>Weight normalization produces sum = 13.0</criterion>
  <criterion>get/set work for all embedders using exact variant names</criterion>
  <criterion>Threshold clamping enforces [0.0, 1.0]</criterion>
  <criterion>Weight clamping enforces >= 0.0</criterion>
  <criterion>Serialization roundtrip works</criterion>
</validation_criteria>

<test_commands>
  <command description="Check compilation">cargo check --package context-graph-core</command>
  <command description="Run config tests">cargo test --package context-graph-core config -- --nocapture</command>
  <command description="Verify no clippy warnings">cargo clippy --package context-graph-core -- -D warnings</command>
  <command description="Run all retrieval tests">cargo test --package context-graph-core retrieval -- --nocapture</command>
</test_commands>

<fail_fast_approach>
NO BACKWARDS COMPATIBILITY - fail fast with robust error logging:

1. Use panic!() or expect() with descriptive messages in debug builds
2. Use debug_assert!() for invariant checks that should never fail
3. Do NOT silently ignore invalid states
4. Log detailed error context before propagating errors
5. Prefer Result<T, Error> over Option<T> where errors are possible

Example:
  // GOOD: Fail fast with context
  debug_assert!(high > low, "Invariant violation: high {} must be > low {} for {:?}", high, low, embedder);

  // BAD: Silent failure
  if high <= low { return; }  // Don't do this
</fail_fast_approach>

<no_mock_data>
CRITICAL: Tests MUST use real values from the spec, not mock data.

WRONG:
  let thresholds = PerSpaceThresholds { semantic: 0.5, ... }; // Made up

RIGHT:
  let thresholds = high_thresholds(); // Uses exact spec values
  assert_eq!(thresholds.semantic, 0.75); // Verifies spec compliance
</no_mock_data>
</task_spec>
```

## Implementation Reference

### Complete PerSpaceThresholds Implementation

```rust
use serde::{Serialize, Deserialize};
use crate::teleological::Embedder;

/// Per-space threshold values for all 13 embedding spaces.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerSpaceThresholds {
    pub semantic: f32,
    pub temporal_recent: f32,
    pub temporal_periodic: f32,
    pub temporal_positional: f32,
    pub causal: f32,
    pub sparse: f32,
    pub code: f32,
    pub emotional: f32,
    pub hdc: f32,
    pub multimodal: f32,
    pub entity: f32,
    pub late_interaction: f32,
    pub keyword_splade: f32,
}

impl PerSpaceThresholds {
    pub fn get_threshold(&self, embedder: Embedder) -> f32 {
        match embedder {
            Embedder::Semantic => self.semantic,
            Embedder::TemporalRecent => self.temporal_recent,
            Embedder::TemporalPeriodic => self.temporal_periodic,
            Embedder::TemporalPositional => self.temporal_positional,
            Embedder::Causal => self.causal,
            Embedder::Sparse => self.sparse,
            Embedder::Code => self.code,
            Embedder::Emotional => self.emotional,
            Embedder::Hdc => self.hdc,
            Embedder::Multimodal => self.multimodal,
            Embedder::Entity => self.entity,
            Embedder::LateInteraction => self.late_interaction,
            Embedder::KeywordSplade => self.keyword_splade,
        }
    }

    pub fn set_threshold(&mut self, embedder: Embedder, threshold: f32) {
        let threshold = threshold.clamp(0.0, 1.0);
        match embedder {
            Embedder::Semantic => self.semantic = threshold,
            Embedder::TemporalRecent => self.temporal_recent = threshold,
            Embedder::TemporalPeriodic => self.temporal_periodic = threshold,
            Embedder::TemporalPositional => self.temporal_positional = threshold,
            Embedder::Causal => self.causal = threshold,
            Embedder::Sparse => self.sparse = threshold,
            Embedder::Code => self.code = threshold,
            Embedder::Emotional => self.emotional = threshold,
            Embedder::Hdc => self.hdc = threshold,
            Embedder::Multimodal => self.multimodal = threshold,
            Embedder::Entity => self.entity = threshold,
            Embedder::LateInteraction => self.late_interaction = threshold,
            Embedder::KeywordSplade => self.keyword_splade = threshold,
        }
    }

    pub fn to_array(&self) -> [f32; 13] {
        [
            self.semantic,
            self.temporal_recent,
            self.temporal_periodic,
            self.temporal_positional,
            self.causal,
            self.sparse,
            self.code,
            self.emotional,
            self.hdc,
            self.multimodal,
            self.entity,
            self.late_interaction,
            self.keyword_splade,
        ]
    }

    pub fn from_array(arr: [f32; 13]) -> Self {
        Self {
            semantic: arr[0],
            temporal_recent: arr[1],
            temporal_periodic: arr[2],
            temporal_positional: arr[3],
            causal: arr[4],
            sparse: arr[5],
            code: arr[6],
            emotional: arr[7],
            hdc: arr[8],
            multimodal: arr[9],
            entity: arr[10],
            late_interaction: arr[11],
            keyword_splade: arr[12],
        }
    }
}

/// High thresholds for relevance detection (EXACT values from spec)
pub fn high_thresholds() -> PerSpaceThresholds {
    PerSpaceThresholds {
        semantic: 0.75,
        temporal_recent: 0.70,
        temporal_periodic: 0.70,
        temporal_positional: 0.70,
        causal: 0.70,
        sparse: 0.60,
        code: 0.80,
        emotional: 0.70,
        hdc: 0.70,
        multimodal: 0.70,
        entity: 0.70,
        late_interaction: 0.70,
        keyword_splade: 0.60,
    }
}

/// Low thresholds for divergence detection (EXACT values from spec)
pub fn low_thresholds() -> PerSpaceThresholds {
    PerSpaceThresholds {
        semantic: 0.30,
        temporal_recent: 0.30,
        temporal_periodic: 0.30,
        temporal_positional: 0.30,
        causal: 0.25,
        sparse: 0.20,
        code: 0.35,
        emotional: 0.30,
        hdc: 0.30,
        multimodal: 0.30,
        entity: 0.30,
        late_interaction: 0.30,
        keyword_splade: 0.20,
    }
}
```

### Complete SpaceWeights Implementation

```rust
/// Category-based weights for similarity calculation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpaceWeights {
    weights: [f32; 13],
}

impl SpaceWeights {
    pub fn new(weights: [f32; 13]) -> Self {
        Self { weights }
    }

    pub fn get_weight(&self, embedder: Embedder) -> f32 {
        self.weights[embedder.index()]
    }

    pub fn set_weight(&mut self, embedder: Embedder, weight: f32) {
        self.weights[embedder.index()] = weight.max(0.0);
    }

    pub fn sum(&self) -> f32 {
        self.weights.iter().sum()
    }

    /// Normalize weights so they sum to 13.0 (one per space)
    pub fn normalize(&mut self) {
        let sum = self.sum();
        if sum > 0.0 {
            let factor = 13.0 / sum;
            for w in &mut self.weights {
                *w *= factor;
            }
        }
    }

    pub fn normalized(&self) -> Self {
        let mut result = self.clone();
        result.normalize();
        result
    }

    pub fn as_slice(&self) -> &[f32; 13] {
        &self.weights
    }
}

/// Category-based weights from constitution (EXACT values)
pub fn default_weights() -> SpaceWeights {
    SpaceWeights::new([
        1.0,  // E1 Semantic
        0.0,  // E2 Temporal Recent (excluded)
        0.0,  // E3 Temporal Periodic (excluded)
        0.0,  // E4 Temporal Positional (excluded)
        1.0,  // E5 Causal
        1.0,  // E6 Sparse
        1.0,  // E7 Code
        0.5,  // E8 Emotional (Relational)
        0.5,  // E9 Hdc (Structural)
        1.0,  // E10 Multimodal
        0.5,  // E11 Entity (Relational)
        1.0,  // E12 LateInteraction
        1.0,  // E13 KeywordSplade
    ])
}

/// Lookback duration for divergence detection (2 hours)
pub const RECENT_LOOKBACK_SECS: u64 = 2 * 60 * 60;

/// Maximum recent memories for divergence check
pub const MAX_RECENT_MEMORIES: usize = 50;
```

### Required Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_high_thresholds_exact_values() {
        let h = high_thresholds();
        assert_eq!(h.semantic, 0.75);
        assert_eq!(h.temporal_recent, 0.70);
        assert_eq!(h.causal, 0.70);
        assert_eq!(h.sparse, 0.60);
        assert_eq!(h.code, 0.80);
        assert_eq!(h.keyword_splade, 0.60);
        println!("[PASS] HIGH_THRESHOLDS match spec exactly");
    }

    #[test]
    fn test_low_thresholds_exact_values() {
        let l = low_thresholds();
        assert_eq!(l.semantic, 0.30);
        assert_eq!(l.causal, 0.25);
        assert_eq!(l.sparse, 0.20);
        assert_eq!(l.code, 0.35);
        assert_eq!(l.keyword_splade, 0.20);
        println!("[PASS] LOW_THRESHOLDS match spec exactly");
    }

    #[test]
    fn test_all_high_greater_than_low() {
        let high = high_thresholds();
        let low = low_thresholds();

        for embedder in Embedder::all() {
            let h = high.get_threshold(embedder);
            let l = low.get_threshold(embedder);
            assert!(h > l, "{:?}: high {} must be > low {}", embedder, h, l);
            println!("[PASS] {:?}: high {} > low {}", embedder, h, l);
        }
    }

    #[test]
    fn test_weights_sum_before_normalization() {
        let weights = default_weights();
        let sum = weights.sum();
        assert!((sum - 8.5).abs() < 0.001, "Expected 8.5, got {}", sum);
        println!("[PASS] default_weights().sum() = {} (expected 8.5)", sum);
    }

    #[test]
    fn test_weights_normalization() {
        let mut weights = default_weights();
        weights.normalize();
        let sum = weights.sum();
        assert!((sum - 13.0).abs() < 0.001, "Expected 13.0, got {}", sum);
        println!("[PASS] normalized weights sum = {} (expected 13.0)", sum);
    }

    #[test]
    fn test_threshold_clamping() {
        let mut t = high_thresholds();
        t.set_threshold(Embedder::Semantic, 1.5);
        assert_eq!(t.get_threshold(Embedder::Semantic), 1.0);
        println!("[PASS] set_threshold clamps 1.5 -> 1.0");

        t.set_threshold(Embedder::Code, -0.3);
        assert_eq!(t.get_threshold(Embedder::Code), 0.0);
        println!("[PASS] set_threshold clamps -0.3 -> 0.0");
    }

    #[test]
    fn test_serialization_roundtrip() {
        let t = high_thresholds();
        let json = serde_json::to_string(&t).expect("serialize");
        let recovered: PerSpaceThresholds = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(t, recovered);
        println!("[PASS] PerSpaceThresholds JSON roundtrip");

        let w = default_weights();
        let json = serde_json::to_string(&w).expect("serialize");
        let recovered: SpaceWeights = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(w, recovered);
        println!("[PASS] SpaceWeights JSON roundtrip");
    }
}
```

## Execution Checklist

- [ ] Read existing similarity.rs to understand PerSpaceScores field names
- [ ] Read existing embedder.rs to understand Embedder variant names
- [ ] Create config.rs in retrieval directory
- [ ] Implement PerSpaceThresholds with matching field names
- [ ] Implement high_thresholds() with exact spec values
- [ ] Implement low_thresholds() with exact spec values
- [ ] Implement SpaceWeights with category-based weights
- [ ] Implement default_weights() with exact spec values
- [ ] Implement SimilarityThresholds container
- [ ] Add RECENT_LOOKBACK_SECS and MAX_RECENT_MEMORIES constants
- [ ] Write unit tests verifying exact spec values
- [ ] Add pub mod config to mod.rs
- [ ] Add config exports to mod.rs pub use
- [ ] Run cargo check --package context-graph-core
- [ ] Run cargo test --package context-graph-core config -- --nocapture
- [ ] Verify all tests show [PASS]
- [ ] Run cargo clippy --package context-graph-core -- -D warnings
- [ ] Proceed to TASK-P3-004

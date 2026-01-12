# TASK-UTL-P1-004: Implement CrossModalEntropy for E10 (Multimodal/CLIP)

**Priority:** P1
**Status:** **COMPLETED**
**Spec Reference:** SPEC-UTL-003
**Estimated Effort:** 3-4 hours
**Implements:** REQ-UTL-003-03, REQ-UTL-003-04

---

## CRITICAL: Constitution Specification

**From `/home/cabdru/contextgraph/docs2/constitution.yaml` line 165:**
```yaml
delta_methods:
  ΔS: { E1: "GMM+Mahalanobis", E5: "Asymmetric KNN", E7: "GMM+KNN hybrid", E9: "Hamming", E10: "Cross-modal KNN", E11: "TransE ||h+r-t||", E12: "Token KNN", E13: "Jaccard", default: "KNN" }
```

**E10 (Multimodal) MUST use "Cross-modal KNN" - NOT default KNN.**

---

## Current State Analysis (Verified 2026-01-12)

### Factory Routing Status (MUST CHANGE)

**File:** `crates/context-graph-utl/src/surprise/embedder_entropy/factory.rs`
**Lines 78-87:**
```rust
// E2-E4, E6, E8, E10-E12: Default KNN-based entropy
Embedder::TemporalRecent
| Embedder::TemporalPeriodic
| Embedder::TemporalPositional
| Embedder::Sparse
| Embedder::Graph
| Embedder::Multimodal  // <-- THIS IS WRONG, must use CrossModalEntropy
| Embedder::Entity
| Embedder::LateInteraction => {
    Box::new(DefaultKnnEntropy::from_config(embedder, config))
}
```

**Current Behavior:** `Embedder::Multimodal` routes to `DefaultKnnEntropy`
**Required Behavior:** `Embedder::Multimodal` must route to `CrossModalEntropy`

### Files to Read Before Implementation

| File | Purpose | Lines of Interest |
|------|---------|-------------------|
| `crates/context-graph-utl/src/surprise/embedder_entropy/mod.rs` | `EmbedderEntropy` trait definition | Lines 63-92 |
| `crates/context-graph-utl/src/surprise/embedder_entropy/factory.rs` | Factory routing - needs update | Line 83 |
| `crates/context-graph-utl/src/surprise/embedder_entropy/hybrid_gmm_knn.rs` | Reference implementation pattern | Full file |
| `crates/context-graph-utl/src/surprise/embedder_entropy/default_knn.rs` | KNN implementation to reuse | sigmoid, distance |
| `crates/context-graph-utl/src/config/surprise.rs` | `SurpriseConfig` - add new fields | After line 118 |
| `crates/context-graph-utl/src/error.rs` | `UtlError`, `UtlResult` types | - |
| `crates/context-graph-utl/src/surprise/embedding_distance.rs` | `compute_cosine_distance` function | Line 47 |
| `crates/context-graph-core/src/teleological/embedder.rs` | `Embedder::Multimodal` = index 9, dim 768 | Line 54 |

### Embedder Specification

- **Embedder enum value:** `Embedder::Multimodal = 9`
- **Dimension:** 768 (CLIP embedding)
- **Model:** CLIP (Contrastive Language-Image Pre-training)
- **Purpose per constitution:** `V_multimodality` - semantic concepts spanning text/visual modalities

---

## Algorithm: Cross-Modal Entropy

### Modality Detection Heuristic

CLIP embeddings have different activation patterns for different modalities:
- **Text inputs:** Higher activation in lower dimensions (linguistic features)
- **Visual inputs:** Higher activation in higher dimensions (perceptual features)

**Detection formula:**
```rust
fn detect_modality_indicator(embedding: &[f32]) -> f32 {
    let half = embedding.len() / 2;
    let lower_energy: f32 = embedding[..half].iter().map(|x| x.powi(2)).sum();
    let upper_energy: f32 = embedding[half..].iter().map(|x| x.powi(2)).sum();
    let total = lower_energy + upper_energy;
    if total < 1e-8 { return 0.5; }
    upper_energy / total  // 0.0 = text-like, 1.0 = visual-like
}
```

### Cross-Modal Weighting

When comparing embeddings:
1. Compute modality indicator for current and each history embedding
2. If same modality (indicators close): use `intra_modal_weight` (default 0.7)
3. If different modality (indicators far apart): use `cross_modal_weight` (default 0.3)

**Distance calculation:**
```rust
fn compute_modal_distance(current: &[f32], other: &[f32],
                          current_mod: f32, other_mod: f32,
                          intra_weight: f32, cross_weight: f32) -> f32 {
    let base_distance = compute_cosine_distance(current, other);
    let modality_diff = (current_mod - other_mod).abs();

    // Blend weights based on modality similarity
    let weight = if modality_diff < 0.3 {
        intra_weight  // Same modality
    } else if modality_diff > 0.7 {
        cross_weight  // Different modality
    } else {
        // Interpolate between weights
        let t = (modality_diff - 0.3) / 0.4;
        intra_weight * (1.0 - t) + cross_weight * t
    };

    base_distance * weight
}
```

### Final ΔS Computation

```rust
fn compute_delta_s(current: &[f32], history: &[Vec<f32>], k: usize) -> f32 {
    if history.is_empty() { return 1.0; }
    if current.is_empty() { return Err(EmptyInput); }

    let current_modality = detect_modality_indicator(current);

    // Compute weighted distances to all history
    let mut distances: Vec<f32> = history.iter()
        .filter(|h| !h.is_empty())
        .map(|h| {
            let h_modality = detect_modality_indicator(h);
            compute_modal_distance(current, h, current_modality, h_modality,
                                   intra_modal_weight, cross_modal_weight)
        })
        .collect();

    // KNN-style: take k nearest
    distances.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    let k_actual = k.min(distances.len()).max(1);
    let mean_dist: f32 = distances[..k_actual].iter().sum::<f32>() / k_actual as f32;

    // Normalize via sigmoid
    let z = (mean_dist - running_mean) / running_std.max(0.1);
    sigmoid(z).clamp(0.0, 1.0)
}
```

---

## Implementation Requirements

### 1. Create New File

**Path:** `crates/context-graph-utl/src/surprise/embedder_entropy/cross_modal.rs`

### 2. Struct Definition

```rust
//! Cross-modal entropy for E10 (Multimodal/CLIP) embeddings.
//!
//! Formula: ΔS = weighted_knn_distance with modality-aware weighting
//! Per constitution.yaml delta_methods.ΔS E10: "Cross-modal KNN"

use super::EmbedderEntropy;
use crate::config::SurpriseConfig;
use crate::error::{UtlError, UtlResult};
use crate::surprise::compute_cosine_distance;
use context_graph_core::teleological::Embedder;

/// Default weight for same-modality comparisons.
const DEFAULT_INTRA_MODAL_WEIGHT: f32 = 0.7;

/// Default weight for cross-modality comparisons.
const DEFAULT_CROSS_MODAL_WEIGHT: f32 = 0.3;

/// E10 (Multimodal) entropy using cross-modal distance metrics.
///
/// CLIP embeddings have different activation patterns for text vs visual:
/// - Text: Higher activation in lower dimensions (linguistic features)
/// - Visual: Higher activation in higher dimensions (perceptual features)
///
/// This calculator weights distances based on modality similarity.
#[derive(Debug, Clone)]
pub struct CrossModalEntropy {
    /// Weight for intra-modal (same modality) comparisons. Default: 0.7
    intra_modal_weight: f32,
    /// Weight for cross-modal (different modality) comparisons. Default: 0.3
    cross_modal_weight: f32,
    /// Running mean for distance normalization.
    running_mean: f32,
    /// Running variance for distance normalization.
    running_variance: f32,
    /// Number of samples seen for EMA.
    sample_count: usize,
    /// EMA alpha for updating statistics.
    ema_alpha: f32,
    /// k neighbors for KNN component.
    k_neighbors: usize,
}
```

### 3. Required Methods

```rust
impl CrossModalEntropy {
    /// Create with default constitution values.
    pub fn new() -> Self;

    /// Create from SurpriseConfig.
    pub fn from_config(config: &SurpriseConfig) -> Self;

    /// Builder: set intra-modal weight.
    #[must_use]
    pub fn with_intra_weight(self, weight: f32) -> Self;

    /// Builder: set cross-modal weight.
    #[must_use]
    pub fn with_cross_weight(self, weight: f32) -> Self;

    /// Detect modality indicator: 0.0 = text, 1.0 = visual, 0.5 = hybrid
    fn detect_modality_indicator(&self, embedding: &[f32]) -> f32;

    /// Compute weighted distance between two embeddings.
    fn compute_modal_distance(
        &self,
        current: &[f32],
        other: &[f32],
        current_modality: f32,
        other_modality: f32,
    ) -> f32;

    /// Sigmoid normalization.
    #[inline]
    fn sigmoid(x: f32) -> f32 { 1.0 / (1.0 + (-x).exp()) }

    /// Update running statistics with new distance.
    fn update_statistics(&mut self, distance: f32);
}

impl Default for CrossModalEntropy {
    fn default() -> Self { Self::new() }
}

impl EmbedderEntropy for CrossModalEntropy {
    fn compute_delta_s(&self, current: &[f32], history: &[Vec<f32>], k: usize) -> UtlResult<f32>;
    fn embedder_type(&self) -> Embedder { Embedder::Multimodal }
    fn reset(&mut self);
}
```

### 4. Update SurpriseConfig

**File:** `crates/context-graph-utl/src/config/surprise.rs`

Add after line 118 (after `code_k_neighbors`):

```rust
// --- Cross-Modal (E10 Multimodal) ---

/// Weight for intra-modal comparisons (same modality).
/// Constitution: 0.7 (prefer same-modality similarity)
/// Range: `[0.0, 1.0]`
pub multimodal_intra_weight: f32,

/// Weight for cross-modal comparisons (different modality).
/// Constitution: 0.3 (still consider cross-modal alignment)
/// Range: `[0.0, 1.0]`
pub multimodal_cross_weight: f32,

/// k neighbors for multimodal KNN entropy.
/// Range: `[1, 20]`
pub multimodal_k_neighbors: usize,
```

Add in `Default::default()` after line 148:

```rust
// Cross-Modal (E10 Multimodal) - per constitution.yaml delta_methods.ΔS E10
multimodal_intra_weight: 0.7,
multimodal_cross_weight: 0.3,
multimodal_k_neighbors: 5,
```

### 5. Update Module Exports

**File:** `crates/context-graph-utl/src/surprise/embedder_entropy/mod.rs`

Add after line 40 (after `mod hybrid_gmm_knn;`):
```rust
mod cross_modal;
```

Add after line 48 (after `pub use hybrid_gmm_knn::HybridGmmKnnEntropy;`):
```rust
pub use cross_modal::CrossModalEntropy;
```

### 6. Update Factory Routing

**File:** `crates/context-graph-utl/src/surprise/embedder_entropy/factory.rs`

Add import after line 19:
```rust
use super::CrossModalEntropy;
```

**Change:** Remove `Embedder::Multimodal` from the fallback match arm (lines 78-87).

**Add new match arm before the fallback:**
```rust
// E10 (Multimodal): Cross-modal KNN per constitution.yaml delta_methods.ΔS E10
Embedder::Multimodal => Box::new(CrossModalEntropy::from_config(config)),
```

**Final factory match block:**
```rust
match embedder {
    // E1: GMM + Mahalanobis distance
    Embedder::Semantic => Box::new(GmmMahalanobisEntropy::from_config(config)),

    // E5: Asymmetric KNN with direction modifiers
    Embedder::Causal => Box::new(
        AsymmetricKnnEntropy::new(config.k_neighbors)
            .with_direction_modifiers(
                config.causal_cause_to_effect_mod,
                config.causal_effect_to_cause_mod,
            ),
    ),

    // E9: Hamming distance to prototypes
    Embedder::Hdc => Box::new(
        HammingPrototypeEntropy::new(config.hdc_max_prototypes)
            .with_threshold(config.hdc_binarization_threshold),
    ),

    // E13: Jaccard similarity of active dimensions
    Embedder::KeywordSplade => Box::new(
        JaccardActiveEntropy::new()
            .with_threshold(config.splade_activation_threshold)
            .with_smoothing(config.splade_smoothing),
    ),

    // E7 (Code): Hybrid GMM+KNN per constitution.yaml
    Embedder::Code => Box::new(HybridGmmKnnEntropy::from_config(config)),

    // E10 (Multimodal): Cross-modal KNN per constitution.yaml
    Embedder::Multimodal => Box::new(CrossModalEntropy::from_config(config)),

    // Fallback: Default KNN for remaining embedders
    Embedder::TemporalRecent
    | Embedder::TemporalPeriodic
    | Embedder::TemporalPositional
    | Embedder::Sparse
    | Embedder::Graph
    | Embedder::Entity
    | Embedder::LateInteraction => {
        Box::new(DefaultKnnEntropy::from_config(embedder, config))
    }
}
```

---

## Required Tests

**Location:** `crates/context-graph-utl/src/surprise/embedder_entropy/cross_modal.rs` (tests module)

| Test Name | Purpose | Expected Outcome |
|-----------|---------|------------------|
| `test_cross_modal_empty_history_returns_one` | Empty history = max surprise | `delta_s == 1.0` |
| `test_cross_modal_empty_input_error` | Empty current = error | `Err(UtlError::EmptyInput)` |
| `test_cross_modal_identical_returns_low` | Same embedding = low ΔS | `delta_s < 0.5` |
| `test_cross_modal_different_modality_weighted` | Cross-modal uses lower weight | Different weight applied |
| `test_cross_modal_same_modality_baseline` | Intra-modal uses higher weight | Higher weight applied |
| `test_cross_modal_embedder_type` | Returns Multimodal | `embedder_type() == Embedder::Multimodal` |
| `test_cross_modal_valid_range` | All outputs in [0, 1] | `0.0 <= delta_s <= 1.0` |
| `test_cross_modal_no_nan_infinity` | AP-10 compliance | `!is_nan() && !is_infinite()` |
| `test_cross_modal_modality_detection` | Text vs visual detection | Correct indicator values |
| `test_cross_modal_from_config` | Config values applied | Fields match config |
| `test_cross_modal_weight_range` | Weights in valid range | Both weights in [0.0, 1.0] |
| `test_cross_modal_reset` | State clears properly | Statistics reset |
| `test_factory_routes_multimodal` | Factory creates CrossModalEntropy | `embedder_type() == Embedder::Multimodal` |

### Test Code Template

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const E10_DIM: usize = 768;  // CLIP dimension

    #[test]
    fn test_cross_modal_empty_history_returns_one() {
        let calculator = CrossModalEntropy::new();
        let current = vec![0.5f32; E10_DIM];
        let history: Vec<Vec<f32>> = vec![];

        println!("BEFORE: history.len() = 0");
        let result = calculator.compute_delta_s(&current, &history, 5);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1.0);
        println!("AFTER: delta_s = 1.0");
        println!("[PASS] test_cross_modal_empty_history_returns_one");
    }

    #[test]
    fn test_cross_modal_empty_input_error() {
        let calculator = CrossModalEntropy::new();
        let empty: Vec<f32> = vec![];
        let history = vec![vec![0.5f32; E10_DIM]];

        let result = calculator.compute_delta_s(&empty, &history, 5);
        assert!(matches!(result, Err(UtlError::EmptyInput)));
        println!("[PASS] test_cross_modal_empty_input_error");
    }

    #[test]
    fn test_cross_modal_modality_detection() {
        let calculator = CrossModalEntropy::new();

        // Text-like: energy in lower dimensions
        let mut text_like = vec![0.0f32; E10_DIM];
        for i in 0..E10_DIM/2 { text_like[i] = 1.0; }
        let text_indicator = calculator.detect_modality_indicator(&text_like);
        println!("text_indicator = {} (expected < 0.3)", text_indicator);
        assert!(text_indicator < 0.3, "Text-like should have low indicator");

        // Visual-like: energy in upper dimensions
        let mut visual_like = vec![0.0f32; E10_DIM];
        for i in E10_DIM/2..E10_DIM { visual_like[i] = 1.0; }
        let visual_indicator = calculator.detect_modality_indicator(&visual_like);
        println!("visual_indicator = {} (expected > 0.7)", visual_indicator);
        assert!(visual_indicator > 0.7, "Visual-like should have high indicator");

        // Balanced: energy evenly distributed
        let balanced = vec![0.5f32; E10_DIM];
        let balanced_indicator = calculator.detect_modality_indicator(&balanced);
        println!("balanced_indicator = {} (expected ~0.5)", balanced_indicator);
        assert!((balanced_indicator - 0.5).abs() < 0.1, "Balanced should be ~0.5");

        println!("[PASS] test_cross_modal_modality_detection");
    }

    #[test]
    fn test_cross_modal_embedder_type() {
        let calculator = CrossModalEntropy::new();
        assert_eq!(calculator.embedder_type(), Embedder::Multimodal);
        println!("[PASS] test_cross_modal_embedder_type");
    }

    #[test]
    fn test_factory_routes_multimodal() {
        let config = SurpriseConfig::default();
        let calculator = EmbedderEntropyFactory::create(Embedder::Multimodal, &config);
        assert_eq!(calculator.embedder_type(), Embedder::Multimodal);
        println!("[PASS] test_factory_routes_multimodal");
    }
}
```

---

## Validation Commands

```bash
# 1. Check compilation
cargo build -p context-graph-utl

# 2. Run specific tests
cargo test -p context-graph-utl cross_modal -- --nocapture

# 3. Run factory tests (verify routing changed)
cargo test -p context-graph-utl factory -- --nocapture

# 4. Run all embedder_entropy tests
cargo test -p context-graph-utl embedder_entropy -- --nocapture

# 5. Check for warnings (MUST pass with no warnings)
cargo clippy -p context-graph-utl -- -D warnings

# 6. Verify no regressions in UTL crate
cargo test -p context-graph-utl --lib
```

---

## Full State Verification (MANDATORY)

### Source of Truth Locations

| Verification | Location | Command | Expected |
|--------------|----------|---------|----------|
| Factory routing | `EmbedderEntropyFactory::create(Embedder::Multimodal, &config)` | Run test | Returns `Embedder::Multimodal` |
| Module export | `use context_graph_utl::surprise::embedder_entropy::CrossModalEntropy;` | `cargo build` | Compiles |
| Config field | `SurpriseConfig::default().multimodal_intra_weight` | Print value | `0.7` |
| Config field | `SurpriseConfig::default().multimodal_cross_weight` | Print value | `0.3` |

### Evidence of Success Log

After running tests, capture output showing:
```
[PASS] test_cross_modal_empty_history_returns_one - delta_s=1.0
[PASS] test_cross_modal_empty_input_error - Err(EmptyInput)
[PASS] test_cross_modal_identical_returns_low - delta_s=<value>
[PASS] test_cross_modal_modality_detection - text=<v>, visual=<v>, balanced=<v>
[PASS] test_cross_modal_embedder_type - Embedder::Multimodal
[PASS] test_factory_routes_multimodal - Embedder::Multimodal
```

### Boundary/Edge Case Audit (MANDATORY)

Execute and print before/after state for these 3 edge cases:

**Edge Case 1: Pure Text-Like Input vs Visual-Like History**
```rust
// Input concentrated in lower dimensions (text)
let mut current = vec![0.0f32; 768];
for i in 0..384 { current[i] = 1.0; }

// History concentrated in upper dimensions (visual)
let history: Vec<Vec<f32>> = (0..10).map(|_| {
    let mut h = vec![0.0f32; 768];
    for i in 384..768 { h[i] = 1.0; }
    h
}).collect();

println!("BEFORE: current is text-like, history is visual-like");
let result = calculator.compute_delta_s(&current, &history, 5);
println!("AFTER: delta_s = {:?}", result);
// Expected: High surprise due to cross-modal comparison with lower weight
```

**Edge Case 2: Single History Item (k > history.len())**
```rust
let current = vec![0.5f32; 768];
let history = vec![vec![0.5f32; 768]];

println!("BEFORE: history.len()={}, k=5", history.len());
let result = calculator.compute_delta_s(&current, &history, 5);
println!("AFTER: delta_s = {:?}", result);
// Expected: Valid result, uses k=1
```

**Edge Case 3: Near-Zero Norm Embedding**
```rust
let current = vec![1e-10f32; 768];
let history = vec![vec![0.5f32; 768]; 10];

println!("BEFORE: current has near-zero norm");
let result = calculator.compute_delta_s(&current, &history, 5);
println!("AFTER: delta_s = {:?}, must not be NaN/Inf");
// Expected: Valid result (0.0-1.0), no NaN/Infinity
```

---

## Manual Testing Checklist

After implementation:

- [ ] `cargo test -p context-graph-utl cross_modal` - All tests pass
- [ ] `cargo test -p context-graph-utl factory` - Factory routes Multimodal to CrossModalEntropy
- [ ] `cargo build -p context-graph-utl` - No compilation errors
- [ ] `cargo clippy -p context-graph-utl -- -D warnings` - No warnings
- [ ] Verify `SurpriseConfig::default().multimodal_intra_weight == 0.7`
- [ ] Verify `SurpriseConfig::default().multimodal_cross_weight == 0.3`
- [ ] Run edge case 1 manually: text vs visual returns high surprise
- [ ] Run edge case 2 manually: single history item works
- [ ] Run edge case 3 manually: near-zero norm doesn't produce NaN

---

## Rollback Plan

If implementation causes issues:

1. Remove `Embedder::Multimodal` match arm from factory
2. Add `Embedder::Multimodal` back to fallback match arm
3. Remove `mod cross_modal;` from mod.rs
4. Remove `pub use cross_modal::CrossModalEntropy;` from mod.rs
5. Remove config fields from surprise.rs
6. Delete `cross_modal.rs`

---

## Anti-Patterns (DO NOT)

Per constitution.yaml:

- **AP-10:** No NaN or Infinity - clamp all outputs to [0.0, 1.0]
- **AP-02:** No cross-embedder comparison - only compare E10↔E10
- **AP-12:** No magic numbers - use named constants
- **AP-14:** No `.unwrap()` in library code - use `?` or proper error handling
- **NO BACKWARDS COMPATIBILITY:** If detection fails, error out with robust logging

---

## Architecture Compliance

- **ARCH-02:** Only compare compatible embedding types (E10↔E10)
- **UTL-003:** Each embedder uses constitution-specified ΔS method (E10 = Cross-modal KNN)
- **768D dimension:** E10 Multimodal uses CLIP 768-dimensional embeddings

---

## Related Tasks

- **TASK-UTL-P1-003:** HybridGmmKnnEntropy for E7 (COMPLETED - implementation exists)
- **TASK-UTL-P1-005:** TransEEntropy for E11
- **TASK-UTL-P1-006:** MaxSimTokenEntropy for E12

---

## File Locations Summary

| Action | File Path |
|--------|-----------|
| CREATE | `crates/context-graph-utl/src/surprise/embedder_entropy/cross_modal.rs` |
| MODIFY | `crates/context-graph-utl/src/surprise/embedder_entropy/mod.rs` |
| MODIFY | `crates/context-graph-utl/src/surprise/embedder_entropy/factory.rs` |
| MODIFY | `crates/context-graph-utl/src/config/surprise.rs` |

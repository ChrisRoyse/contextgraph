# Task: TASK-F002 - Implement TeleologicalFingerprint Struct

## Metadata
- **ID**: TASK-F002
- **Layer**: Foundation
- **Priority**: P0 (Critical Path)
- **Estimated Effort**: L (Large)
- **Status**: BLOCKED (Waiting on TASK-F003)
- **Dependencies**:
  - TASK-F001 (SemanticFingerprint) - **COMPLETE** ✅ (36 tests passing)
  - TASK-F003 (JohariFingerprint) - **NOT STARTED** ❌ (BLOCKING)
- **Traces To**: TS-102, FR-201, FR-202, FR-203, FR-204
- **Last Audited**: 2026-01-05

---

## ⚠️ CRITICAL: DEPENDENCY STATUS

### TASK-F001: SemanticFingerprint - ✅ COMPLETE
**Verified**: 36 tests passing as of 2026-01-05
```bash
$ cargo test -p context-graph-core semantic -- --nocapture
running 36 tests
test types::fingerprint::semantic::tests::test_compute_sparse_centroid ... ok
test types::fingerprint::semantic::tests::test_e6_sparse_validation ... ok
# ... all 36 tests pass
test result: ok. 36 passed; 0 failed; 0 ignored
```

**Location**: `crates/context-graph-core/src/types/fingerprint/semantic.rs`
**Exports**: `SemanticFingerprint`, `EmbeddingSlice`, dimension constants

### TASK-F003: JohariFingerprint - ❌ NOT STARTED (BLOCKING)
**Status**: JohariQuadrant enum exists, but JohariFingerprint struct does NOT exist
**Required Before F002**: YES - TeleologicalFingerprint depends on JohariFingerprint

**What Exists**:
- `crates/context-graph-core/src/types/johari/quadrant.rs` - JohariQuadrant enum only

**What Does NOT Exist**:
- `JohariFingerprint` struct (needed by TeleologicalFingerprint)
- Awareness classification per embedder
- Johari fingerprint tests

---

## Description

Implement the `TeleologicalFingerprint` struct that wraps `SemanticFingerprint` with purpose-aware metadata enabling goal-aligned retrieval. This is the complete node representation combining:

1. **SemanticFingerprint**: The raw 13-embedding array (from TASK-F001) - ✅ AVAILABLE
2. **PurposeVector**: 13D alignment signature to North Star goal
3. **JohariFingerprint**: Per-embedder awareness classification (from TASK-F003) - ❌ BLOCKED
4. **Purpose Evolution**: Time-series tracking of alignment changes
5. **StageScores**: 5-stage pipeline scores for retrieval quality tracking

The TeleologicalFingerprint enables queries like "find memories similar to X that serve the same purpose" rather than just "find memories similar to X".

### Theoretical Foundation (from constitution.yaml)

**Teleological Vector Architecture**:
- The 13-embedding array IS the teleological vector (E1-E13)
- Purpose Vector: `PV = [A(E1,V), A(E2,V), ..., A(E13,V)]` where `A(Ei, V) = cos(θ)` between embedder i and North Star goal V
- Stage Scores: `SS = [S1, S2, S3, S4, S5]` where each Si tracks 5-stage pipeline performance
- Learning equation: `L = f((ΔS × ΔC) · wₑ · cos φ)`
- Alignment thresholds from Royse 2026 research:
  - **Optimal**: θ ≥ 0.75
  - **Acceptable**: θ ∈ [0.70, 0.75)
  - **Warning**: θ ∈ [0.55, 0.70)
  - **Critical**: θ < 0.55
- Misalignment predictor: `delta_A < -0.15` predicts failure 72 hours ahead

---

## Codebase Audit (Verified 2026-01-05)

### Current File Structure
```
crates/context-graph-core/src/types/
├── fingerprint/
│   ├── mod.rs              # Exports SemanticFingerprint, SparseVector (MODIFY)
│   ├── semantic.rs         # SemanticFingerprint - TASK-F001 COMPLETE ✅
│   ├── sparse.rs           # SparseVector for E6 SPLADE - TASK-F001 COMPLETE ✅
│   ├── purpose.rs          # TO CREATE - PurposeVector, AlignmentThreshold
│   ├── evolution.rs        # TO CREATE - EvolutionTrigger, PurposeSnapshot
│   ├── teleological.rs     # TO CREATE - TeleologicalFingerprint
│   └── johari.rs           # TO CREATE (STUB) - JohariFingerprint placeholder
├── johari/
│   ├── mod.rs              # Exports JohariQuadrant
│   └── quadrant.rs         # JohariQuadrant enum - EXISTS ✅
├── mod.rs                  # Top-level types exports (MODIFY)
└── ... (other modules)
```

### Existing SemanticFingerprint Structure (Source of Truth)
From `semantic.rs` lines 200-230:
```rust
pub struct SemanticFingerprint {
    pub e1_openai_3large: Vec<f32>,     // 3072 dims
    pub e2_voyage_3large: Vec<f32>,     // 1024 dims
    pub e3_cohere_embed_v4: Vec<f32>,   // 1024 dims
    pub e4_gemini_text_005: Vec<f32>,   // 768 dims
    pub e5_jina_embeddings_v3: Vec<f32>,// 1024 dims
    pub e6_splade_v3: SparseVector,     // 30522 vocab sparse
    pub e7_bge_m3: Vec<f32>,            // 1024 dims
    pub e8_gte_qwen2: Vec<f32>,         // 1024 dims
    pub e9_arctic_embed_l: Vec<f32>,    // 1024 dims
    pub e10_nomic_embed: Vec<f32>,      // 768 dims
    pub e11_mxbai_large: Vec<f32>,      // 1024 dims
    pub e12_modernbert: Vec<f32>,       // 1024 dims (token-level)
}
```

### Existing JohariQuadrant (Source of Truth)
From `johari/quadrant.rs`:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum JohariQuadrant {
    Open,    // Known to self AND others - Low entropy, High coherence
    Hidden,  // Known to self, NOT others - Medium entropy, High coherence
    Blind,   // NOT known to self, Known to others - High entropy, Low coherence
    Unknown, // NOT known to self OR others - High entropy, Unknown coherence
}
```

### Dependencies in Cargo.toml (Verified)
```toml
# Already present - DO NOT ADD AGAIN
uuid = { version = "1.11", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
serde = { version = "1.0", features = ["derive"] }
```

---

## Acceptance Criteria

### Required Structs/Enums
- [ ] `AlignmentThreshold` enum (Optimal, Acceptable, Warning, Critical)
- [ ] `PurposeVector` struct with 13-element alignment array
- [ ] `EvolutionTrigger` enum for tracking purpose changes
- [ ] `PurposeSnapshot` struct for time-series data
- [ ] `JohariFingerprint` stub struct (placeholder until TASK-F003)
- [ ] `TeleologicalFingerprint` struct integrating all components
- [ ] `StageScores` struct with 5-element pipeline scores array

### Required Methods
- [ ] `AlignmentThreshold::classify(theta: f32) -> Self`
- [ ] `AlignmentThreshold::is_misaligned(&self) -> bool`
- [ ] `PurposeVector::new(alignments: [f32; 13]) -> Self`
- [ ] `PurposeVector::aggregate_alignment(&self) -> f32`
- [ ] `PurposeVector::threshold_status(&self) -> AlignmentThreshold`
- [ ] `PurposeVector::find_dominant(&self) -> u8`
- [ ] `PurposeVector::similarity(&self, other: &Self) -> f32`
- [ ] `StageScores::new(scores: [f32; 5]) -> Self`
- [ ] `StageScores::stage_name(idx: usize) -> &'static str`
- [ ] `StageScores::update_stage(&mut self, stage: usize, score: f32)`
- [ ] `TeleologicalFingerprint::new(...) -> Self`
- [ ] `TeleologicalFingerprint::record_snapshot(&mut self, trigger: EvolutionTrigger)`
- [ ] `TeleologicalFingerprint::compute_alignment_delta(&self) -> f32`
- [ ] `TeleologicalFingerprint::check_misalignment_warning(&self) -> Option<f32>`
- [ ] `TeleologicalFingerprint::alignment_status(&self) -> AlignmentThreshold`
- [ ] `TeleologicalFingerprint::update_stage_score(&mut self, stage: usize, score: f32)`

### Validation Requirements
- [ ] Misalignment warning detection: `delta_A < -0.15`
- [ ] MAX_EVOLUTION_SNAPSHOTS = 100 enforced
- [ ] All timestamps in UTC (chrono::Utc)
- [ ] UUID v4 for fingerprint ID
- [ ] SHA-256 content_hash (32 bytes)

---

## Implementation Files

### File 1: `crates/context-graph-core/src/types/fingerprint/purpose.rs`

```rust
//! Purpose Vector types for teleological alignment tracking.
//!
//! From constitution.yaml: Purpose Vector PV = [A(E1,V), ..., A(E12,V)]
//! where A(Ei, V) = cos(θ) between embedder i and North Star goal V.

use serde::{Deserialize, Serialize};

/// Number of embedders in the teleological vector architecture.
/// From constitution.yaml: 13 embedding models form the teleological vector (E1-E13).
pub const NUM_EMBEDDERS: usize = 13;

/// Number of stages in the 5-stage retrieval pipeline.
pub const NUM_STAGES: usize = 5;

/// Alignment threshold categories from Royse 2026 research.
///
/// From constitution.yaml:
/// - Optimal: θ ≥ 0.75
/// - Acceptable: θ ∈ [0.70, 0.75)
/// - Warning: θ ∈ [0.55, 0.70)
/// - Critical: θ < 0.55
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AlignmentThreshold {
    /// θ ≥ 0.75 - Strong alignment with North Star goal
    Optimal,
    /// θ ∈ [0.70, 0.75) - Acceptable alignment, monitor for drift
    Acceptable,
    /// θ ∈ [0.55, 0.70) - Alignment degrading, intervention recommended
    Warning,
    /// θ < 0.55 - Critical misalignment, immediate action required
    Critical,
}

impl AlignmentThreshold {
    /// Classify an alignment value into a threshold category.
    ///
    /// # Arguments
    /// * `theta` - Alignment value (cosine similarity to North Star), expected range [-1.0, 1.0]
    ///
    /// # Returns
    /// The appropriate threshold category based on Royse 2026 thresholds.
    ///
    /// # Example
    /// ```
    /// use context_graph_core::types::fingerprint::AlignmentThreshold;
    ///
    /// assert_eq!(AlignmentThreshold::classify(0.80), AlignmentThreshold::Optimal);
    /// assert_eq!(AlignmentThreshold::classify(0.72), AlignmentThreshold::Acceptable);
    /// assert_eq!(AlignmentThreshold::classify(0.60), AlignmentThreshold::Warning);
    /// assert_eq!(AlignmentThreshold::classify(0.40), AlignmentThreshold::Critical);
    /// ```
    #[inline]
    pub fn classify(theta: f32) -> Self {
        if theta >= 0.75 {
            Self::Optimal
        } else if theta >= 0.70 {
            Self::Acceptable
        } else if theta >= 0.55 {
            Self::Warning
        } else {
            Self::Critical
        }
    }

    /// Check if this threshold indicates misalignment requiring action.
    ///
    /// Warning and Critical thresholds are considered misaligned.
    #[inline]
    pub fn is_misaligned(&self) -> bool {
        matches!(self, Self::Warning | Self::Critical)
    }

    /// Check if this threshold is critical (requires immediate action).
    #[inline]
    pub fn is_critical(&self) -> bool {
        matches!(self, Self::Critical)
    }

    /// Get the minimum theta value for this threshold.
    #[inline]
    pub fn min_theta(&self) -> f32 {
        match self {
            Self::Optimal => 0.75,
            Self::Acceptable => 0.70,
            Self::Warning => 0.55,
            Self::Critical => f32::NEG_INFINITY,
        }
    }
}

impl std::fmt::Display for AlignmentThreshold {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Optimal => write!(f, "Optimal (θ ≥ 0.75)"),
            Self::Acceptable => write!(f, "Acceptable (0.70 ≤ θ < 0.75)"),
            Self::Warning => write!(f, "Warning (0.55 ≤ θ < 0.70)"),
            Self::Critical => write!(f, "Critical (θ < 0.55)"),
        }
    }
}

/// Purpose Vector: 13D alignment signature to North Star goal.
///
/// From constitution.yaml: `PV = [A(E1,V), A(E2,V), ..., A(E13,V)]`
/// where `A(Ei, V) = cos(θ)` between embedder i and North Star goal V.
///
/// Each element is the cosine similarity between that embedder's representation
/// and the North Star goal vector, measuring how well that semantic dimension
/// aligns with the system's ultimate purpose.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurposeVector {
    /// Alignment values for each of 13 embedders. Range: [-1.0, 1.0]
    /// Index mapping:
    /// - 0: E1 OpenAI text-embedding-3-large (Matryoshka)
    /// - 1: E2 Voyage-3-large
    /// - 2: E3 Cohere embed-v4
    /// - 3: E4 Gemini text-005
    /// - 4: E5 Jina embeddings v3
    /// - 5: E6 SPLADE v3 (legacy sparse slot)
    /// - 6: E7 BGE-M3
    /// - 7: E8 GTE-Qwen2
    /// - 8: E9 Arctic-embed-L
    /// - 9: E10 Nomic-embed
    /// - 10: E11 mxbai-large
    /// - 11: E12 ModernBERT (ColBERT)
    /// - 12: E13 SPLADE v3 (explicit sparse for 5-stage pipeline)
    pub alignments: [f32; NUM_EMBEDDERS],

    /// Index of the embedder with highest alignment (0-12).
    pub dominant_embedder: u8,

    /// Coherence score: standard deviation inverse of alignments.
    /// High coherence = all embedders agree on alignment direction.
    /// Range: [0.0, 1.0] where 1.0 = perfect agreement
    pub coherence: f32,

    /// Stability score: inverse of alignment variance over time.
    /// High stability = alignment doesn't fluctuate between accesses.
    /// Range: [0.0, 1.0] where 1.0 = perfectly stable
    pub stability: f32,
}

/// Stage Scores: 5-stage pipeline performance tracking.
///
/// Tracks how well this memory performs at each stage of the 5-stage retrieval pipeline.
/// Used for pipeline optimization and memory quality assessment.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct StageScores {
    /// Scores for each of 5 pipeline stages. Range: [0.0, 1.0]
    /// Index mapping:
    /// - 0: Stage 1 (Recall) - BM25 + SPLADE initial selection
    /// - 1: Stage 2 (Semantic) - Matryoshka 128D fast filtering
    /// - 2: Stage 3 (Precision) - Full E1-E12 dense ranking
    /// - 3: Stage 4 (Rerank) - Cross-encoder / ColBERT late interaction
    /// - 4: Stage 5 (Teleological) - Purpose vector alignment
    pub scores: [f32; NUM_STAGES],
}

impl StageScores {
    /// Stage names for display and logging.
    pub const STAGE_NAMES: [&'static str; NUM_STAGES] = [
        "Recall (BM25+SPLADE)",
        "Semantic (Matryoshka 128D)",
        "Precision (Dense E1-E12)",
        "Rerank (ColBERT/Cross-encoder)",
        "Teleological (Purpose Alignment)",
    ];

    /// Create new StageScores from scores array.
    pub fn new(scores: [f32; NUM_STAGES]) -> Self {
        Self { scores }
    }

    /// Get stage name by index.
    pub fn stage_name(idx: usize) -> Option<&'static str> {
        Self::STAGE_NAMES.get(idx).copied()
    }

    /// Update a specific stage score.
    pub fn update_stage(&mut self, stage: usize, score: f32) {
        if stage < NUM_STAGES {
            self.scores[stage] = score.clamp(0.0, 1.0);
        }
    }

    /// Get average score across all stages.
    pub fn average_score(&self) -> f32 {
        self.scores.iter().sum::<f32>() / NUM_STAGES as f32
    }
}

impl PurposeVector {
    /// Create a new PurposeVector from alignment values.
    ///
    /// Automatically computes dominant_embedder and coherence.
    /// Stability starts at 1.0 (no history to measure variance).
    ///
    /// # Arguments
    /// * `alignments` - Array of 12 alignment values (cosine similarities)
    ///
    /// # Panics
    /// Does not panic. Invalid alignment values are accepted but may produce
    /// unexpected results in downstream computations.
    pub fn new(alignments: [f32; NUM_EMBEDDERS]) -> Self {
        let dominant_embedder = Self::compute_dominant(&alignments);
        let coherence = Self::compute_coherence(&alignments);

        Self {
            alignments,
            dominant_embedder,
            coherence,
            stability: 1.0, // No history yet
        }
    }

    /// Create from 12-element array (legacy compatibility).
    /// E13 alignment defaults to 0.0.
    #[deprecated(since = "2.0.0", note = "Use new() with 13-element array")]
    pub fn from_12(alignments_12: [f32; 12]) -> Self {
        let mut alignments = [0.0f32; NUM_EMBEDDERS];
        alignments[..12].copy_from_slice(&alignments_12);
        Self::new(alignments)
    }

    /// Compute the aggregate (mean) alignment across all embedders.
    ///
    /// This is the primary measure of overall goal alignment.
    #[inline]
    pub fn aggregate_alignment(&self) -> f32 {
        let sum: f32 = self.alignments.iter().sum();
        sum / NUM_EMBEDDERS as f32
    }

    /// Get the threshold status based on aggregate alignment.
    #[inline]
    pub fn threshold_status(&self) -> AlignmentThreshold {
        AlignmentThreshold::classify(self.aggregate_alignment())
    }

    /// Find the index of the dominant (highest alignment) embedder.
    #[inline]
    pub fn find_dominant(&self) -> u8 {
        self.dominant_embedder
    }

    /// Compute cosine similarity between two PurposeVectors.
    ///
    /// Measures how similar the alignment profiles are between two memories.
    /// Used for "find memories serving the same purpose" queries.
    ///
    /// # Returns
    /// Cosine similarity in range [-1.0, 1.0]
    pub fn similarity(&self, other: &Self) -> f32 {
        let mut dot = 0.0f32;
        let mut norm_a = 0.0f32;
        let mut norm_b = 0.0f32;

        for i in 0..NUM_EMBEDDERS {
            dot += self.alignments[i] * other.alignments[i];
            norm_a += self.alignments[i] * self.alignments[i];
            norm_b += other.alignments[i] * other.alignments[i];
        }

        let denominator = (norm_a.sqrt()) * (norm_b.sqrt());
        if denominator < f32::EPSILON {
            0.0
        } else {
            dot / denominator
        }
    }

    /// Update stability based on comparison with previous state.
    ///
    /// # Arguments
    /// * `previous` - The previous PurposeVector to compare against
    /// * `decay` - Exponential decay factor for stability (default 0.9)
    pub fn update_stability(&mut self, previous: &Self, decay: f32) {
        let delta = self.similarity(previous);
        // High similarity = high stability, use exponential moving average
        self.stability = decay * self.stability + (1.0 - decay) * delta.abs();
    }

    /// Compute dominant embedder index from alignments.
    fn compute_dominant(alignments: &[f32; NUM_EMBEDDERS]) -> u8 {
        let mut max_idx = 0u8;
        let mut max_val = alignments[0];

        for (i, &val) in alignments.iter().enumerate().skip(1) {
            if val > max_val {
                max_val = val;
                max_idx = i as u8;
            }
        }
        max_idx
    }

    /// Compute coherence from alignments using inverse standard deviation.
    fn compute_coherence(alignments: &[f32; NUM_EMBEDDERS]) -> f32 {
        let mean: f32 = alignments.iter().sum::<f32>() / NUM_EMBEDDERS as f32;
        let variance: f32 = alignments.iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f32>() / NUM_EMBEDDERS as f32;
        let std_dev = variance.sqrt();

        // Inverse stddev normalized to [0, 1]
        // When std_dev = 0, coherence = 1.0 (perfect agreement)
        // As std_dev increases, coherence decreases toward 0
        1.0 / (1.0 + std_dev)
    }
}

impl Default for PurposeVector {
    fn default() -> Self {
        Self::new([0.0; NUM_EMBEDDERS])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== AlignmentThreshold Tests =====

    #[test]
    fn test_alignment_threshold_classify_optimal() {
        // Boundary: exactly 0.75
        assert_eq!(AlignmentThreshold::classify(0.75), AlignmentThreshold::Optimal);
        // Above boundary
        assert_eq!(AlignmentThreshold::classify(0.80), AlignmentThreshold::Optimal);
        assert_eq!(AlignmentThreshold::classify(0.99), AlignmentThreshold::Optimal);
        assert_eq!(AlignmentThreshold::classify(1.0), AlignmentThreshold::Optimal);

        println!("[PASS] Optimal threshold: θ >= 0.75 correctly classified");
    }

    #[test]
    fn test_alignment_threshold_classify_acceptable() {
        // Boundary: exactly 0.70
        assert_eq!(AlignmentThreshold::classify(0.70), AlignmentThreshold::Acceptable);
        // In range
        assert_eq!(AlignmentThreshold::classify(0.72), AlignmentThreshold::Acceptable);
        // Just below upper boundary
        assert_eq!(AlignmentThreshold::classify(0.749), AlignmentThreshold::Acceptable);

        println!("[PASS] Acceptable threshold: 0.70 <= θ < 0.75 correctly classified");
    }

    #[test]
    fn test_alignment_threshold_classify_warning() {
        // Boundary: exactly 0.55
        assert_eq!(AlignmentThreshold::classify(0.55), AlignmentThreshold::Warning);
        // In range
        assert_eq!(AlignmentThreshold::classify(0.60), AlignmentThreshold::Warning);
        // Just below upper boundary
        assert_eq!(AlignmentThreshold::classify(0.699), AlignmentThreshold::Warning);

        println!("[PASS] Warning threshold: 0.55 <= θ < 0.70 correctly classified");
    }

    #[test]
    fn test_alignment_threshold_classify_critical() {
        // Below 0.55
        assert_eq!(AlignmentThreshold::classify(0.54), AlignmentThreshold::Critical);
        assert_eq!(AlignmentThreshold::classify(0.40), AlignmentThreshold::Critical);
        assert_eq!(AlignmentThreshold::classify(0.0), AlignmentThreshold::Critical);
        // Negative values
        assert_eq!(AlignmentThreshold::classify(-0.5), AlignmentThreshold::Critical);

        println!("[PASS] Critical threshold: θ < 0.55 correctly classified");
    }

    #[test]
    fn test_alignment_threshold_is_misaligned() {
        assert!(!AlignmentThreshold::Optimal.is_misaligned());
        assert!(!AlignmentThreshold::Acceptable.is_misaligned());
        assert!(AlignmentThreshold::Warning.is_misaligned());
        assert!(AlignmentThreshold::Critical.is_misaligned());

        println!("[PASS] is_misaligned returns true for Warning and Critical only");
    }

    #[test]
    fn test_alignment_threshold_is_critical() {
        assert!(!AlignmentThreshold::Optimal.is_critical());
        assert!(!AlignmentThreshold::Acceptable.is_critical());
        assert!(!AlignmentThreshold::Warning.is_critical());
        assert!(AlignmentThreshold::Critical.is_critical());

        println!("[PASS] is_critical returns true for Critical only");
    }

    // ===== PurposeVector Tests =====

    #[test]
    fn test_purpose_vector_new() {
        let alignments = [0.8, 0.7, 0.9, 0.6, 0.75, 0.65, 0.85, 0.72, 0.78, 0.68, 0.82, 0.71];
        let pv = PurposeVector::new(alignments);

        assert_eq!(pv.alignments, alignments);
        assert_eq!(pv.dominant_embedder, 2); // Index of 0.9
        assert!(pv.coherence > 0.0 && pv.coherence <= 1.0);
        assert_eq!(pv.stability, 1.0); // Initial stability

        println!("[PASS] PurposeVector::new correctly initializes all fields");
        println!("  - alignments: {:?}", pv.alignments);
        println!("  - dominant_embedder: {} (value: {})", pv.dominant_embedder, alignments[pv.dominant_embedder as usize]);
        println!("  - coherence: {:.4}", pv.coherence);
    }

    #[test]
    fn test_purpose_vector_aggregate_alignment() {
        // All same value = easy to verify mean
        let uniform = PurposeVector::new([0.75; NUM_EMBEDDERS]);
        assert!((uniform.aggregate_alignment() - 0.75).abs() < f32::EPSILON);

        // Known sum
        let alignments = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.5, 0.5];
        let expected_mean = alignments.iter().sum::<f32>() / 12.0;
        let pv = PurposeVector::new(alignments);
        assert!((pv.aggregate_alignment() - expected_mean).abs() < f32::EPSILON);

        println!("[PASS] aggregate_alignment returns correct mean");
        println!("  - Uniform [0.75; 12] mean: {:.4}", uniform.aggregate_alignment());
        println!("  - Variable array mean: {:.4} (expected: {:.4})", pv.aggregate_alignment(), expected_mean);
    }

    #[test]
    fn test_purpose_vector_threshold_status() {
        // Optimal aggregate
        let optimal = PurposeVector::new([0.8; NUM_EMBEDDERS]);
        assert_eq!(optimal.threshold_status(), AlignmentThreshold::Optimal);

        // Critical aggregate
        let critical = PurposeVector::new([0.3; NUM_EMBEDDERS]);
        assert_eq!(critical.threshold_status(), AlignmentThreshold::Critical);

        println!("[PASS] threshold_status correctly classifies aggregate alignment");
    }

    #[test]
    fn test_purpose_vector_find_dominant() {
        // Clear dominant
        let mut alignments = [0.5; NUM_EMBEDDERS];
        alignments[7] = 0.95; // E8 is dominant
        let pv = PurposeVector::new(alignments);
        assert_eq!(pv.find_dominant(), 7);

        // First value is dominant (ties go to first)
        let tie = PurposeVector::new([0.9; NUM_EMBEDDERS]);
        assert_eq!(tie.find_dominant(), 0);

        println!("[PASS] find_dominant returns index of highest alignment");
    }

    #[test]
    fn test_purpose_vector_similarity_identical() {
        let pv = PurposeVector::new([0.7, 0.8, 0.6, 0.9, 0.75, 0.65, 0.85, 0.72, 0.78, 0.68, 0.82, 0.71]);
        let similarity = pv.similarity(&pv);
        assert!((similarity - 1.0).abs() < 1e-6);

        println!("[PASS] Identical vectors have similarity 1.0");
    }

    #[test]
    fn test_purpose_vector_similarity_orthogonal() {
        // Opposing alignment patterns
        let pv1 = PurposeVector::new([1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0]);
        let pv2 = PurposeVector::new([0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0]);
        let similarity = pv1.similarity(&pv2);
        assert!(similarity.abs() < 1e-6); // Orthogonal = 0

        println!("[PASS] Orthogonal vectors have similarity 0.0");
    }

    #[test]
    fn test_purpose_vector_similarity_opposite() {
        let pv1 = PurposeVector::new([0.5; NUM_EMBEDDERS]);
        let pv2 = PurposeVector::new([-0.5; NUM_EMBEDDERS]);
        let similarity = pv1.similarity(&pv2);
        assert!((similarity - (-1.0)).abs() < 1e-6);

        println!("[PASS] Opposite vectors have similarity -1.0");
    }

    #[test]
    fn test_purpose_vector_coherence_uniform() {
        // All same = perfect coherence
        let uniform = PurposeVector::new([0.8; NUM_EMBEDDERS]);
        assert!((uniform.coherence - 1.0).abs() < 1e-6);

        println!("[PASS] Uniform alignments have coherence 1.0 (no variance)");
    }

    #[test]
    fn test_purpose_vector_coherence_varied() {
        // High variance = lower coherence
        let varied = PurposeVector::new([0.1, 0.9, 0.1, 0.9, 0.1, 0.9, 0.1, 0.9, 0.1, 0.9, 0.1, 0.9]);
        assert!(varied.coherence < 1.0);
        assert!(varied.coherence > 0.0);

        println!("[PASS] Varied alignments have coherence < 1.0");
        println!("  - High variance coherence: {:.4}", varied.coherence);
    }

    #[test]
    fn test_purpose_vector_update_stability() {
        let mut pv1 = PurposeVector::new([0.8; NUM_EMBEDDERS]);
        let pv2 = PurposeVector::new([0.8; NUM_EMBEDDERS]); // Identical

        let initial_stability = pv1.stability;
        pv1.update_stability(&pv2, 0.9);

        // Identical vectors = high similarity = stability should remain high
        assert!(pv1.stability > 0.9);

        println!("[PASS] update_stability correctly updates based on similarity");
        println!("  - Initial: {:.4}, After: {:.4}", initial_stability, pv1.stability);
    }

    #[test]
    fn test_purpose_vector_default() {
        let pv = PurposeVector::default();
        assert_eq!(pv.alignments, [0.0; NUM_EMBEDDERS]);
        assert_eq!(pv.dominant_embedder, 0);

        println!("[PASS] Default PurposeVector has zero alignments");
    }

    // ===== Edge Cases =====

    #[test]
    fn test_purpose_vector_zero_vector() {
        let zero = PurposeVector::new([0.0; NUM_EMBEDDERS]);
        assert_eq!(zero.aggregate_alignment(), 0.0);
        assert_eq!(zero.threshold_status(), AlignmentThreshold::Critical);

        // Similarity with zero vector
        let nonzero = PurposeVector::new([0.5; NUM_EMBEDDERS]);
        let similarity = zero.similarity(&nonzero);
        assert_eq!(similarity, 0.0); // Zero norm = 0 similarity

        println!("[PASS] Zero vector edge case handled correctly");
    }

    #[test]
    fn test_alignment_threshold_boundary_values() {
        // Test exact boundaries with epsilon tolerance
        assert_eq!(AlignmentThreshold::classify(0.75 - f32::EPSILON), AlignmentThreshold::Acceptable);
        assert_eq!(AlignmentThreshold::classify(0.70 - f32::EPSILON), AlignmentThreshold::Warning);
        assert_eq!(AlignmentThreshold::classify(0.55 - f32::EPSILON), AlignmentThreshold::Critical);

        println!("[PASS] Boundary values classify correctly with epsilon tolerance");
    }
}
```

### File 2: `crates/context-graph-core/src/types/fingerprint/evolution.rs`

```rust
//! Purpose evolution tracking for teleological fingerprints.
//!
//! Tracks how a memory's alignment with North Star goals changes over time.
//! From constitution.yaml: delta_A < -0.15 predicts failure 72 hours ahead.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::purpose::PurposeVector;
use super::johari::JohariFingerprint;

/// Events that trigger a purpose evolution snapshot.
///
/// Each variant captures context about why the alignment was recalculated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvolutionTrigger {
    /// Memory was just created
    Created,

    /// Memory was accessed in a query
    Accessed {
        /// Context/query that accessed this memory
        query_context: String,
    },

    /// The North Star goal itself changed
    GoalChanged {
        /// Previous goal UUID
        old_goal: Uuid,
        /// New goal UUID
        new_goal: Uuid,
    },

    /// Periodic recalibration (scheduled maintenance)
    Recalibration,

    /// Misalignment warning was detected
    MisalignmentDetected {
        /// The alignment delta that triggered warning (< -0.15)
        delta_a: f32,
    },
}

impl EvolutionTrigger {
    /// Check if this trigger indicates a potential problem.
    pub fn is_warning(&self) -> bool {
        matches!(self, Self::MisalignmentDetected { .. })
    }

    /// Get a human-readable description of the trigger.
    pub fn description(&self) -> String {
        match self {
            Self::Created => "Memory created".to_string(),
            Self::Accessed { query_context } => format!("Accessed via: {}", query_context),
            Self::GoalChanged { old_goal, new_goal } => {
                format!("Goal changed: {} -> {}", old_goal, new_goal)
            }
            Self::Recalibration => "Scheduled recalibration".to_string(),
            Self::MisalignmentDetected { delta_a } => {
                format!("Misalignment detected: delta_A = {:.4}", delta_a)
            }
        }
    }
}

impl std::fmt::Display for EvolutionTrigger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

/// A snapshot of purpose state at a point in time.
///
/// Used to track how a memory's alignment evolves over its lifetime.
/// The system maintains up to MAX_EVOLUTION_SNAPSHOTS (100) snapshots;
/// older snapshots are archived to TimescaleDB in production.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurposeSnapshot {
    /// When this snapshot was taken (UTC)
    pub timestamp: DateTime<Utc>,

    /// The purpose vector at this point in time
    pub purpose: PurposeVector,

    /// The Johari classification at this point in time
    pub johari: JohariFingerprint,

    /// What triggered this snapshot
    pub trigger: EvolutionTrigger,
}

impl PurposeSnapshot {
    /// Create a new snapshot at the current time.
    pub fn new(purpose: PurposeVector, johari: JohariFingerprint, trigger: EvolutionTrigger) -> Self {
        Self {
            timestamp: Utc::now(),
            purpose,
            johari,
            trigger,
        }
    }

    /// Create a snapshot with a specific timestamp (for testing/import).
    pub fn with_timestamp(
        timestamp: DateTime<Utc>,
        purpose: PurposeVector,
        johari: JohariFingerprint,
        trigger: EvolutionTrigger,
    ) -> Self {
        Self {
            timestamp,
            purpose,
            johari,
            trigger,
        }
    }

    /// Get the aggregate alignment at this snapshot.
    pub fn aggregate_alignment(&self) -> f32 {
        self.purpose.aggregate_alignment()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::purpose::NUM_EMBEDDERS;
    use chrono::Duration;

    fn make_test_purpose() -> PurposeVector {
        PurposeVector::new([0.75; NUM_EMBEDDERS])
    }

    fn make_test_johari() -> JohariFingerprint {
        JohariFingerprint::default()
    }

    #[test]
    fn test_evolution_trigger_created() {
        let trigger = EvolutionTrigger::Created;
        assert!(!trigger.is_warning());
        assert_eq!(trigger.description(), "Memory created");

        println!("[PASS] EvolutionTrigger::Created works correctly");
    }

    #[test]
    fn test_evolution_trigger_accessed() {
        let trigger = EvolutionTrigger::Accessed {
            query_context: "Find related documents".to_string(),
        };
        assert!(!trigger.is_warning());
        assert!(trigger.description().contains("Find related documents"));

        println!("[PASS] EvolutionTrigger::Accessed captures query context");
    }

    #[test]
    fn test_evolution_trigger_goal_changed() {
        let old_goal = Uuid::new_v4();
        let new_goal = Uuid::new_v4();
        let trigger = EvolutionTrigger::GoalChanged { old_goal, new_goal };
        assert!(!trigger.is_warning());

        let desc = trigger.description();
        assert!(desc.contains(&old_goal.to_string()));
        assert!(desc.contains(&new_goal.to_string()));

        println!("[PASS] EvolutionTrigger::GoalChanged captures both UUIDs");
    }

    #[test]
    fn test_evolution_trigger_recalibration() {
        let trigger = EvolutionTrigger::Recalibration;
        assert!(!trigger.is_warning());
        assert_eq!(trigger.description(), "Scheduled recalibration");

        println!("[PASS] EvolutionTrigger::Recalibration works correctly");
    }

    #[test]
    fn test_evolution_trigger_misalignment_detected() {
        let trigger = EvolutionTrigger::MisalignmentDetected { delta_a: -0.20 };
        assert!(trigger.is_warning());
        assert!(trigger.description().contains("-0.20"));

        println!("[PASS] EvolutionTrigger::MisalignmentDetected is a warning");
    }

    #[test]
    fn test_purpose_snapshot_new() {
        let purpose = make_test_purpose();
        let johari = make_test_johari();

        let before = Utc::now();
        let snapshot = PurposeSnapshot::new(purpose.clone(), johari.clone(), EvolutionTrigger::Created);
        let after = Utc::now();

        // Timestamp should be between before and after
        assert!(snapshot.timestamp >= before);
        assert!(snapshot.timestamp <= after);

        // Data should match
        assert_eq!(snapshot.purpose.alignments, purpose.alignments);
        assert!(matches!(snapshot.trigger, EvolutionTrigger::Created));

        println!("[PASS] PurposeSnapshot::new captures current time");
        println!("  - Timestamp: {}", snapshot.timestamp);
    }

    #[test]
    fn test_purpose_snapshot_with_timestamp() {
        let purpose = make_test_purpose();
        let johari = make_test_johari();
        let custom_time = Utc::now() - Duration::hours(24);

        let snapshot = PurposeSnapshot::with_timestamp(
            custom_time,
            purpose,
            johari,
            EvolutionTrigger::Recalibration,
        );

        assert_eq!(snapshot.timestamp, custom_time);

        println!("[PASS] PurposeSnapshot::with_timestamp accepts custom timestamp");
    }

    #[test]
    fn test_purpose_snapshot_aggregate_alignment() {
        let purpose = PurposeVector::new([0.80; NUM_EMBEDDERS]);
        let johari = make_test_johari();
        let snapshot = PurposeSnapshot::new(purpose, johari, EvolutionTrigger::Created);

        assert!((snapshot.aggregate_alignment() - 0.80).abs() < f32::EPSILON);

        println!("[PASS] PurposeSnapshot::aggregate_alignment returns correct value");
    }

    #[test]
    fn test_evolution_trigger_display() {
        let triggers = vec![
            EvolutionTrigger::Created,
            EvolutionTrigger::Accessed { query_context: "test".to_string() },
            EvolutionTrigger::Recalibration,
            EvolutionTrigger::MisalignmentDetected { delta_a: -0.18 },
        ];

        for trigger in triggers {
            let display = format!("{}", trigger);
            assert!(!display.is_empty());
            println!("  - {}: {}", std::any::type_name::<EvolutionTrigger>(), display);
        }

        println!("[PASS] EvolutionTrigger Display trait works for all variants");
    }
}
```

### File 3: `crates/context-graph-core/src/types/fingerprint/johari.rs`

```rust
//! JohariFingerprint: Per-embedder awareness classification.
//!
//! **STATUS: STUB - AWAITING TASK-F003 COMPLETION**
//!
//! This module provides a minimal placeholder for JohariFingerprint
//! to allow TeleologicalFingerprint to compile. The full implementation
//! is defined in TASK-F003.
//!
//! From constitution.yaml, the Johari Window maps to ΔS × ΔC:
//! - Open: Low entropy (ΔS), High coherence (ΔC) - Known to self AND others
//! - Hidden: Medium entropy, High coherence - Known to self, NOT others
//! - Blind: High entropy, Low coherence - NOT known to self, Known to others
//! - Unknown: High entropy, Unknown coherence - NOT known to self OR others

use serde::{Deserialize, Serialize};
use crate::types::JohariQuadrant;

use super::purpose::NUM_EMBEDDERS;

/// Per-embedder Johari awareness classification.
///
/// Each of the 13 embedders has its own Johari quadrant, indicating
/// how "aware" the system is of that semantic dimension.
///
/// **NOTE**: This is a stub implementation. Full implementation in TASK-F003.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JohariFingerprint {
    /// Johari quadrant for each of the 13 embedders.
    /// Index mapping matches PurposeVector (E1-E13).
    pub quadrants: [JohariQuadrant; NUM_EMBEDDERS],

    /// The dominant (most common) quadrant across all embedders.
    pub dominant_quadrant: JohariQuadrant,

    /// Fraction of embedders in the Open quadrant [0.0, 1.0].
    pub openness: f32,
}

impl JohariFingerprint {
    /// Create a new JohariFingerprint from quadrant classifications.
    ///
    /// **NOTE**: Stub implementation - TASK-F003 will add full logic.
    pub fn new(quadrants: [JohariQuadrant; NUM_EMBEDDERS]) -> Self {
        let dominant_quadrant = Self::compute_dominant(&quadrants);
        let openness = Self::compute_openness(&quadrants);

        Self {
            quadrants,
            dominant_quadrant,
            openness,
        }
    }

    /// Create a stub with all embedders in Unknown quadrant.
    ///
    /// Used when TASK-F003 is not complete but F002 needs to compile.
    pub fn stub() -> Self {
        Self::new([JohariQuadrant::Unknown; NUM_EMBEDDERS])
    }

    /// Check if overall awareness is healthy (majority Open/Hidden).
    pub fn is_aware(&self) -> bool {
        self.openness >= 0.5
    }

    fn compute_dominant(quadrants: &[JohariQuadrant; NUM_EMBEDDERS]) -> JohariQuadrant {
        let mut counts = [0u8; 4]; // Open, Hidden, Blind, Unknown

        for q in quadrants {
            match q {
                JohariQuadrant::Open => counts[0] += 1,
                JohariQuadrant::Hidden => counts[1] += 1,
                JohariQuadrant::Blind => counts[2] += 1,
                JohariQuadrant::Unknown => counts[3] += 1,
            }
        }

        let max_idx = counts.iter()
            .enumerate()
            .max_by_key(|(_, &count)| count)
            .map(|(idx, _)| idx)
            .unwrap_or(3);

        match max_idx {
            0 => JohariQuadrant::Open,
            1 => JohariQuadrant::Hidden,
            2 => JohariQuadrant::Blind,
            _ => JohariQuadrant::Unknown,
        }
    }

    fn compute_openness(quadrants: &[JohariQuadrant; NUM_EMBEDDERS]) -> f32 {
        let open_count = quadrants.iter()
            .filter(|&&q| q == JohariQuadrant::Open)
            .count();
        open_count as f32 / NUM_EMBEDDERS as f32
    }
}

impl Default for JohariFingerprint {
    /// Default to all Unknown (stub behavior).
    fn default() -> Self {
        Self::stub()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_johari_fingerprint_stub() {
        let jf = JohariFingerprint::stub();

        assert_eq!(jf.quadrants, [JohariQuadrant::Unknown; NUM_EMBEDDERS]);
        assert_eq!(jf.dominant_quadrant, JohariQuadrant::Unknown);
        assert_eq!(jf.openness, 0.0);

        println!("[PASS] JohariFingerprint::stub creates all Unknown");
    }

    #[test]
    fn test_johari_fingerprint_new() {
        let mut quadrants = [JohariQuadrant::Open; NUM_EMBEDDERS];
        quadrants[0] = JohariQuadrant::Hidden;
        quadrants[1] = JohariQuadrant::Blind;

        let jf = JohariFingerprint::new(quadrants);

        assert_eq!(jf.dominant_quadrant, JohariQuadrant::Open); // 10/12 Open
        assert!((jf.openness - 10.0/12.0).abs() < f32::EPSILON);

        println!("[PASS] JohariFingerprint::new computes correct dominant and openness");
    }

    #[test]
    fn test_johari_fingerprint_is_aware() {
        // Majority Open = aware
        let aware = JohariFingerprint::new([JohariQuadrant::Open; NUM_EMBEDDERS]);
        assert!(aware.is_aware());

        // Majority Unknown = not aware
        let unaware = JohariFingerprint::stub();
        assert!(!unaware.is_aware());

        println!("[PASS] JohariFingerprint::is_aware returns correct value");
    }

    #[test]
    fn test_johari_fingerprint_default() {
        let jf = JohariFingerprint::default();
        assert_eq!(jf.quadrants, [JohariQuadrant::Unknown; NUM_EMBEDDERS]);

        println!("[PASS] JohariFingerprint::default is stub");
    }
}
```

### File 4: `crates/context-graph-core/src/types/fingerprint/teleological.rs`

```rust
//! TeleologicalFingerprint: Complete node representation with purpose-aware metadata.
//!
//! This is the top-level fingerprint type that wraps SemanticFingerprint with:
//! - Purpose Vector (12D alignment to North Star goal)
//! - Johari Fingerprint (per-embedder awareness classification)
//! - Purpose Evolution (time-series of alignment changes)
//!
//! Enables goal-aligned retrieval: "find memories similar to X that serve the same purpose"

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::purpose::{AlignmentThreshold, PurposeVector, NUM_EMBEDDERS};
use super::evolution::{EvolutionTrigger, PurposeSnapshot};
use super::johari::JohariFingerprint;
use super::SemanticFingerprint;

/// Complete teleological fingerprint for a memory node.
///
/// This struct combines semantic content (what) with purpose (why),
/// enabling retrieval that considers both similarity and goal alignment.
///
/// From constitution.yaml:
/// - Expected size: ~50KB per node (updated for E13 + stage_scores)
/// - MAX_EVOLUTION_SNAPSHOTS: 100 (older snapshots archived to TimescaleDB)
/// - Misalignment warning: delta_A < -0.15 predicts failure 72 hours ahead
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeleologicalFingerprint {
    /// Unique identifier for this fingerprint (UUID v4)
    pub id: Uuid,

    /// The 13-embedding semantic fingerprint (from TASK-F001)
    pub semantic: SemanticFingerprint,

    /// 13D alignment signature to North Star goal
    pub purpose_vector: PurposeVector,

    /// Per-embedder Johari awareness classification
    pub johari: JohariFingerprint,

    /// Time-series of purpose evolution snapshots
    pub purpose_evolution: Vec<PurposeSnapshot>,

    /// Current alignment angle to North Star goal (aggregate)
    pub theta_to_north_star: f32,

    /// 5-stage pipeline performance scores
    pub stage_scores: StageScores,

    /// SHA-256 hash of the source content (32 bytes)
    pub content_hash: [u8; 32],

    /// When this fingerprint was created
    pub created_at: DateTime<Utc>,

    /// When this fingerprint was last updated
    pub last_updated: DateTime<Utc>,

    /// Number of times this memory has been accessed
    pub access_count: u64,
}

impl TeleologicalFingerprint {
    /// Expected size in bytes for a complete teleological fingerprint.
    /// From constitution.yaml: ~50KB per node (updated for E13 + stage_scores).
    pub const EXPECTED_SIZE_BYTES: usize = 50_000;

    /// Maximum number of evolution snapshots to retain in memory.
    /// Older snapshots are archived to TimescaleDB in production.
    pub const MAX_EVOLUTION_SNAPSHOTS: usize = 100;

    /// Threshold for misalignment warning (from constitution.yaml).
    /// delta_A < -0.15 predicts failure 72 hours ahead.
    pub const MISALIGNMENT_THRESHOLD: f32 = -0.15;

    /// Create a new TeleologicalFingerprint.
    ///
    /// Automatically:
    /// - Generates a new UUID v4
    /// - Sets timestamps to now
    /// - Computes initial theta_to_north_star
    /// - Records initial evolution snapshot with Created trigger
    ///
    /// # Arguments
    /// * `semantic` - The semantic fingerprint (12 embeddings)
    /// * `purpose_vector` - The purpose alignment vector
    /// * `johari` - The Johari awareness classification
    /// * `content_hash` - SHA-256 hash of source content
    pub fn new(
        semantic: SemanticFingerprint,
        purpose_vector: PurposeVector,
        johari: JohariFingerprint,
        content_hash: [u8; 32],
    ) -> Self {
        let now = Utc::now();
        let theta_to_north_star = purpose_vector.aggregate_alignment();

        // Create initial snapshot
        let initial_snapshot = PurposeSnapshot::new(
            purpose_vector.clone(),
            johari.clone(),
            EvolutionTrigger::Created,
        );

        Self {
            id: Uuid::new_v4(),
            semantic,
            purpose_vector,
            johari,
            purpose_evolution: vec![initial_snapshot],
            theta_to_north_star,
            stage_scores: StageScores::default(),
            content_hash,
            created_at: now,
            last_updated: now,
            access_count: 0,
        }
    }

    /// Update a specific stage score from the 5-stage pipeline.
    ///
    /// # Arguments
    /// * `stage` - Stage index (0-4)
    /// * `score` - Score value [0.0, 1.0]
    pub fn update_stage_score(&mut self, stage: usize, score: f32) {
        self.stage_scores.update_stage(stage, score);
        self.last_updated = Utc::now();
    }

    /// Get all stage scores.
    pub fn get_stage_scores(&self) -> &StageScores {
        &self.stage_scores
    }

    /// Create a TeleologicalFingerprint with a specific ID (for testing/import).
    pub fn with_id(
        id: Uuid,
        semantic: SemanticFingerprint,
        purpose_vector: PurposeVector,
        johari: JohariFingerprint,
        content_hash: [u8; 32],
    ) -> Self {
        let mut fp = Self::new(semantic, purpose_vector, johari, content_hash);
        fp.id = id;
        fp
    }

    /// Record a new purpose evolution snapshot.
    ///
    /// Updates:
    /// - Adds snapshot to evolution history
    /// - Trims history if over MAX_EVOLUTION_SNAPSHOTS
    /// - Updates last_updated timestamp
    /// - Recalculates theta_to_north_star
    ///
    /// # Arguments
    /// * `trigger` - What caused this evolution event
    pub fn record_snapshot(&mut self, trigger: EvolutionTrigger) {
        let snapshot = PurposeSnapshot::new(
            self.purpose_vector.clone(),
            self.johari.clone(),
            trigger,
        );

        self.purpose_evolution.push(snapshot);

        // Trim if over limit (remove oldest)
        if self.purpose_evolution.len() > Self::MAX_EVOLUTION_SNAPSHOTS {
            // In production, archive to TimescaleDB before removing
            self.purpose_evolution.remove(0);
        }

        self.last_updated = Utc::now();
        self.theta_to_north_star = self.purpose_vector.aggregate_alignment();
    }

    /// Compute the alignment delta from the previous snapshot.
    ///
    /// Returns 0.0 if there is only one snapshot (no previous to compare).
    ///
    /// # Returns
    /// `current_alignment - previous_alignment`
    /// Negative values indicate alignment is degrading.
    pub fn compute_alignment_delta(&self) -> f32 {
        if self.purpose_evolution.len() < 2 {
            return 0.0;
        }

        let current = self.theta_to_north_star;
        let previous = self.purpose_evolution[self.purpose_evolution.len() - 2]
            .aggregate_alignment();

        current - previous
    }

    /// Check for misalignment warning.
    ///
    /// From constitution.yaml: delta_A < -0.15 predicts failure 72 hours ahead.
    ///
    /// # Returns
    /// `Some(delta_a)` if misalignment detected, `None` otherwise.
    pub fn check_misalignment_warning(&self) -> Option<f32> {
        let delta = self.compute_alignment_delta();
        if delta < Self::MISALIGNMENT_THRESHOLD {
            Some(delta)
        } else {
            None
        }
    }

    /// Get the current alignment status.
    pub fn alignment_status(&self) -> AlignmentThreshold {
        AlignmentThreshold::classify(self.theta_to_north_star)
    }

    /// Record an access event.
    ///
    /// Increments access_count and optionally records a snapshot.
    ///
    /// # Arguments
    /// * `query_context` - Description of the query that accessed this memory
    /// * `record_snapshot` - Whether to record an evolution snapshot
    pub fn record_access(&mut self, query_context: String, record_evolution: bool) {
        self.access_count += 1;
        self.last_updated = Utc::now();

        if record_evolution {
            self.record_snapshot(EvolutionTrigger::Accessed { query_context });
        }
    }

    /// Update purpose vector (e.g., after recalibration).
    ///
    /// Automatically records an evolution snapshot and checks for misalignment.
    pub fn update_purpose(&mut self, new_purpose: PurposeVector, trigger: EvolutionTrigger) {
        self.purpose_vector = new_purpose;
        self.record_snapshot(trigger);
    }

    /// Get the age of this fingerprint (time since creation).
    pub fn age(&self) -> chrono::Duration {
        Utc::now() - self.created_at
    }

    /// Get the number of evolution snapshots.
    pub fn evolution_count(&self) -> usize {
        self.purpose_evolution.len()
    }

    /// Check if this fingerprint has concerning alignment trends.
    ///
    /// Returns true if:
    /// - Current alignment is in Warning or Critical threshold
    /// - OR alignment delta indicates degradation (< -0.15)
    pub fn is_concerning(&self) -> bool {
        self.alignment_status().is_misaligned() || self.check_misalignment_warning().is_some()
    }

    /// Get a summary of alignment history.
    ///
    /// Returns (min, max, average) alignment across all snapshots.
    pub fn alignment_history_stats(&self) -> (f32, f32, f32) {
        if self.purpose_evolution.is_empty() {
            return (0.0, 0.0, 0.0);
        }

        let mut min = f32::MAX;
        let mut max = f32::MIN;
        let mut sum = 0.0f32;

        for snapshot in &self.purpose_evolution {
            let alignment = snapshot.aggregate_alignment();
            min = min.min(alignment);
            max = max.max(alignment);
            sum += alignment;
        }

        let avg = sum / self.purpose_evolution.len() as f32;
        (min, max, avg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::JohariQuadrant;

    // ===== Test Helpers =====

    fn make_test_semantic() -> SemanticFingerprint {
        SemanticFingerprint::default()
    }

    fn make_test_purpose(alignment: f32) -> PurposeVector {
        PurposeVector::new([alignment; NUM_EMBEDDERS])
    }

    fn make_test_johari() -> JohariFingerprint {
        JohariFingerprint::new([JohariQuadrant::Open; NUM_EMBEDDERS])
    }

    fn make_test_hash() -> [u8; 32] {
        let mut hash = [0u8; 32];
        hash[0] = 0xDE;
        hash[1] = 0xAD;
        hash[30] = 0xBE;
        hash[31] = 0xEF;
        hash
    }

    // ===== Creation Tests =====

    #[test]
    fn test_teleological_new() {
        let semantic = make_test_semantic();
        let purpose = make_test_purpose(0.80);
        let johari = make_test_johari();
        let hash = make_test_hash();

        let before = Utc::now();
        let fp = TeleologicalFingerprint::new(semantic, purpose, johari, hash);
        let after = Utc::now();

        // ID is valid UUID
        assert!(!fp.id.is_nil());

        // Timestamps are set
        assert!(fp.created_at >= before && fp.created_at <= after);
        assert!(fp.last_updated >= before && fp.last_updated <= after);

        // Initial snapshot exists
        assert_eq!(fp.purpose_evolution.len(), 1);
        assert!(matches!(fp.purpose_evolution[0].trigger, EvolutionTrigger::Created));

        // Theta is computed
        assert!((fp.theta_to_north_star - 0.80).abs() < f32::EPSILON);

        // Access count starts at 0
        assert_eq!(fp.access_count, 0);

        // Hash is stored
        assert_eq!(fp.content_hash, hash);

        println!("[PASS] TeleologicalFingerprint::new creates valid fingerprint");
        println!("  - ID: {}", fp.id);
        println!("  - Created: {}", fp.created_at);
        println!("  - Initial theta: {:.4}", fp.theta_to_north_star);
        println!("  - Evolution snapshots: {}", fp.purpose_evolution.len());
    }

    #[test]
    fn test_teleological_with_id() {
        let specific_id = Uuid::new_v4();
        let fp = TeleologicalFingerprint::with_id(
            specific_id,
            make_test_semantic(),
            make_test_purpose(0.75),
            make_test_johari(),
            make_test_hash(),
        );

        assert_eq!(fp.id, specific_id);

        println!("[PASS] TeleologicalFingerprint::with_id uses provided ID");
    }

    // ===== Snapshot Recording Tests =====

    #[test]
    fn test_teleological_record_snapshot() {
        let mut fp = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.80),
            make_test_johari(),
            make_test_hash(),
        );

        let initial_count = fp.evolution_count();
        let initial_updated = fp.last_updated;

        // Small delay to ensure timestamp difference
        std::thread::sleep(std::time::Duration::from_millis(10));

        fp.record_snapshot(EvolutionTrigger::Recalibration);

        assert_eq!(fp.evolution_count(), initial_count + 1);
        assert!(fp.last_updated > initial_updated);
        assert!(matches!(
            fp.purpose_evolution.last().unwrap().trigger,
            EvolutionTrigger::Recalibration
        ));

        println!("[PASS] record_snapshot adds to evolution and updates timestamp");
        println!("  - Before: {} snapshots", initial_count);
        println!("  - After: {} snapshots", fp.evolution_count());
    }

    #[test]
    fn test_teleological_record_snapshot_respects_limit() {
        let mut fp = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.80),
            make_test_johari(),
            make_test_hash(),
        );

        // Add MAX + 50 snapshots
        for i in 0..(TeleologicalFingerprint::MAX_EVOLUTION_SNAPSHOTS + 50) {
            fp.record_snapshot(EvolutionTrigger::Accessed {
                query_context: format!("query_{}", i),
            });
        }

        // Should be capped at MAX
        assert_eq!(fp.evolution_count(), TeleologicalFingerprint::MAX_EVOLUTION_SNAPSHOTS);

        // First snapshot should NOT be "Created" (it was trimmed)
        assert!(!matches!(
            fp.purpose_evolution[0].trigger,
            EvolutionTrigger::Created
        ));

        println!("[PASS] record_snapshot enforces MAX_EVOLUTION_SNAPSHOTS = {}",
                 TeleologicalFingerprint::MAX_EVOLUTION_SNAPSHOTS);
    }

    // ===== Alignment Delta Tests =====

    #[test]
    fn test_teleological_alignment_delta_single_snapshot() {
        let fp = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.80),
            make_test_johari(),
            make_test_hash(),
        );

        // Only one snapshot = delta is 0
        assert_eq!(fp.compute_alignment_delta(), 0.0);

        println!("[PASS] alignment_delta returns 0.0 with single snapshot");
    }

    #[test]
    fn test_teleological_alignment_delta_improvement() {
        let mut fp = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.70),
            make_test_johari(),
            make_test_hash(),
        );

        // Improve alignment
        fp.purpose_vector = make_test_purpose(0.85);
        fp.theta_to_north_star = fp.purpose_vector.aggregate_alignment();
        fp.record_snapshot(EvolutionTrigger::Recalibration);

        let delta = fp.compute_alignment_delta();
        assert!((delta - 0.15).abs() < f32::EPSILON);

        println!("[PASS] alignment_delta shows positive improvement: {:.4}", delta);
    }

    #[test]
    fn test_teleological_alignment_delta_degradation() {
        let mut fp = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.80),
            make_test_johari(),
            make_test_hash(),
        );

        // Degrade alignment
        fp.purpose_vector = make_test_purpose(0.60);
        fp.theta_to_north_star = fp.purpose_vector.aggregate_alignment();
        fp.record_snapshot(EvolutionTrigger::Recalibration);

        let delta = fp.compute_alignment_delta();
        assert!((delta - (-0.20)).abs() < f32::EPSILON);

        println!("[PASS] alignment_delta shows negative degradation: {:.4}", delta);
    }

    // ===== Misalignment Warning Tests =====

    #[test]
    fn test_teleological_misalignment_warning_not_triggered() {
        let mut fp = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.80),
            make_test_johari(),
            make_test_hash(),
        );

        // Small degradation (within threshold)
        fp.purpose_vector = make_test_purpose(0.75);
        fp.theta_to_north_star = fp.purpose_vector.aggregate_alignment();
        fp.record_snapshot(EvolutionTrigger::Recalibration);

        assert!(fp.check_misalignment_warning().is_none());

        println!("[PASS] No warning for small degradation (delta = -0.05)");
    }

    #[test]
    fn test_teleological_misalignment_warning_triggered() {
        let mut fp = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.80),
            make_test_johari(),
            make_test_hash(),
        );

        // Large degradation (exceeds threshold of -0.15)
        fp.purpose_vector = make_test_purpose(0.60);
        fp.theta_to_north_star = fp.purpose_vector.aggregate_alignment();
        fp.record_snapshot(EvolutionTrigger::Recalibration);

        let warning = fp.check_misalignment_warning();
        assert!(warning.is_some());

        let delta = warning.unwrap();
        assert!(delta < TeleologicalFingerprint::MISALIGNMENT_THRESHOLD);

        println!("[PASS] Warning triggered for large degradation: delta = {:.4} < {:.2}",
                 delta, TeleologicalFingerprint::MISALIGNMENT_THRESHOLD);
    }

    #[test]
    fn test_teleological_misalignment_warning_exact_threshold() {
        let mut fp = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.80),
            make_test_johari(),
            make_test_hash(),
        );

        // Exactly at threshold (delta = -0.15)
        fp.purpose_vector = make_test_purpose(0.65);
        fp.theta_to_north_star = fp.purpose_vector.aggregate_alignment();
        fp.record_snapshot(EvolutionTrigger::Recalibration);

        // -0.15 is NOT < -0.15, so no warning
        assert!(fp.check_misalignment_warning().is_none());

        println!("[PASS] No warning at exact threshold (-0.15)");
    }

    // ===== Alignment Status Tests =====

    #[test]
    fn test_teleological_alignment_status() {
        let fp_optimal = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.80),
            make_test_johari(),
            make_test_hash(),
        );
        assert_eq!(fp_optimal.alignment_status(), AlignmentThreshold::Optimal);

        let fp_critical = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.40),
            make_test_johari(),
            make_test_hash(),
        );
        assert_eq!(fp_critical.alignment_status(), AlignmentThreshold::Critical);

        println!("[PASS] alignment_status correctly classifies theta");
    }

    // ===== Access Recording Tests =====

    #[test]
    fn test_teleological_record_access() {
        let mut fp = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.80),
            make_test_johari(),
            make_test_hash(),
        );

        assert_eq!(fp.access_count, 0);
        let initial_evolution = fp.evolution_count();

        fp.record_access("test query".to_string(), false);
        assert_eq!(fp.access_count, 1);
        assert_eq!(fp.evolution_count(), initial_evolution); // No snapshot

        fp.record_access("another query".to_string(), true);
        assert_eq!(fp.access_count, 2);
        assert_eq!(fp.evolution_count(), initial_evolution + 1); // With snapshot

        println!("[PASS] record_access increments count and optionally records snapshot");
    }

    // ===== Concerning State Tests =====

    #[test]
    fn test_teleological_is_concerning() {
        // Not concerning: Optimal alignment
        let fp_ok = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.80),
            make_test_johari(),
            make_test_hash(),
        );
        assert!(!fp_ok.is_concerning());

        // Concerning: Critical alignment
        let fp_critical = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.40),
            make_test_johari(),
            make_test_hash(),
        );
        assert!(fp_critical.is_concerning());

        println!("[PASS] is_concerning detects problematic states");
    }

    // ===== History Stats Tests =====

    #[test]
    fn test_teleological_alignment_history_stats() {
        let mut fp = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.70),
            make_test_johari(),
            make_test_hash(),
        );

        // Add more snapshots with varying alignments
        fp.purpose_vector = make_test_purpose(0.80);
        fp.theta_to_north_star = 0.80;
        fp.record_snapshot(EvolutionTrigger::Recalibration);

        fp.purpose_vector = make_test_purpose(0.60);
        fp.theta_to_north_star = 0.60;
        fp.record_snapshot(EvolutionTrigger::Recalibration);

        let (min, max, avg) = fp.alignment_history_stats();

        assert!((min - 0.60).abs() < f32::EPSILON);
        assert!((max - 0.80).abs() < f32::EPSILON);
        assert!((avg - 0.70).abs() < f32::EPSILON); // (0.70 + 0.80 + 0.60) / 3

        println!("[PASS] alignment_history_stats computes correct min/max/avg");
        println!("  - Min: {:.2}, Max: {:.2}, Avg: {:.2}", min, max, avg);
    }

    // ===== Constants Tests =====

    #[test]
    fn test_teleological_constants() {
        assert_eq!(TeleologicalFingerprint::EXPECTED_SIZE_BYTES, 46_000);
        assert_eq!(TeleologicalFingerprint::MAX_EVOLUTION_SNAPSHOTS, 100);
        assert!((TeleologicalFingerprint::MISALIGNMENT_THRESHOLD - (-0.15)).abs() < f32::EPSILON);

        println!("[PASS] Constants match specification");
        println!("  - EXPECTED_SIZE_BYTES: {}", TeleologicalFingerprint::EXPECTED_SIZE_BYTES);
        println!("  - MAX_EVOLUTION_SNAPSHOTS: {}", TeleologicalFingerprint::MAX_EVOLUTION_SNAPSHOTS);
        println!("  - MISALIGNMENT_THRESHOLD: {}", TeleologicalFingerprint::MISALIGNMENT_THRESHOLD);
    }

    // ===== Edge Cases =====

    #[test]
    fn test_teleological_zero_alignment() {
        let fp = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.0),
            make_test_johari(),
            make_test_hash(),
        );

        assert_eq!(fp.theta_to_north_star, 0.0);
        assert_eq!(fp.alignment_status(), AlignmentThreshold::Critical);

        println!("[PASS] Zero alignment handled correctly");
    }

    #[test]
    fn test_teleological_negative_alignment() {
        let fp = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(-0.5),
            make_test_johari(),
            make_test_hash(),
        );

        assert_eq!(fp.theta_to_north_star, -0.5);
        assert_eq!(fp.alignment_status(), AlignmentThreshold::Critical);

        println!("[PASS] Negative alignment handled correctly");
    }

    #[test]
    fn test_teleological_serialization() {
        let fp = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.75),
            make_test_johari(),
            make_test_hash(),
        );

        // Test JSON serialization
        let json = serde_json::to_string(&fp).expect("Serialization should succeed");
        assert!(!json.is_empty());

        // Test deserialization
        let restored: TeleologicalFingerprint = serde_json::from_str(&json)
            .expect("Deserialization should succeed");
        assert_eq!(restored.id, fp.id);
        assert!((restored.theta_to_north_star - fp.theta_to_north_star).abs() < f32::EPSILON);

        println!("[PASS] TeleologicalFingerprint serializes/deserializes correctly");
    }
}
```

### File 5: `crates/context-graph-core/src/types/fingerprint/mod.rs` (UPDATED)

```rust
//! Fingerprint types for the Context Graph system.
//!
//! This module provides the complete teleological fingerprint hierarchy:
//! - SemanticFingerprint: 12-embedding array (TASK-F001) ✅
//! - SparseVector: SPLADE sparse vector for E6 (TASK-F001) ✅
//! - PurposeVector: 12D alignment to North Star (TASK-F002)
//! - JohariFingerprint: Per-embedder awareness (TASK-F003 stub)
//! - TeleologicalFingerprint: Complete node representation (TASK-F002)

mod semantic;
mod sparse;
mod purpose;
mod evolution;
mod johari;
mod teleological;

// Re-export SemanticFingerprint types (TASK-F001)
pub use semantic::{
    SemanticFingerprint, EmbeddingSlice,
    E1_DIM, E2_DIM, E3_DIM, E4_DIM, E5_DIM, E6_SPARSE_VOCAB,
    E7_DIM, E8_DIM, E9_DIM, E10_DIM, E11_DIM, E12_TOKEN_DIM,
    TOTAL_DENSE_DIMS,
};

// Re-export SparseVector types (TASK-F001)
pub use sparse::{
    SparseVector, SparseVectorError,
    SPARSE_VOCAB_SIZE, MAX_SPARSE_ACTIVE,
};

// Re-export Purpose types (TASK-F002)
pub use purpose::{
    AlignmentThreshold, PurposeVector, StageScores, NUM_EMBEDDERS, NUM_STAGES,
};

// Re-export Evolution types (TASK-F002)
pub use evolution::{
    EvolutionTrigger, PurposeSnapshot,
};

// Re-export Johari types (TASK-F003 stub)
pub use johari::JohariFingerprint;

// Re-export TeleologicalFingerprint (TASK-F002)
pub use teleological::TeleologicalFingerprint;
```

---

## Verification Commands

### Pre-Implementation Verification
```bash
# Verify TASK-F001 is still passing
cargo test -p context-graph-core semantic -- --nocapture 2>&1 | head -50

# Verify current fingerprint module structure
ls -la crates/context-graph-core/src/types/fingerprint/

# Verify JohariQuadrant exists
grep -n "JohariQuadrant" crates/context-graph-core/src/types/johari/quadrant.rs
```

### Post-Implementation Verification
```bash
# Compile check (MUST PASS)
cargo check -p context-graph-core 2>&1 | tee /tmp/f002-check.log

# Run all TASK-F002 tests
cargo test -p context-graph-core purpose -- --nocapture 2>&1 | tee /tmp/f002-purpose.log
cargo test -p context-graph-core evolution -- --nocapture 2>&1 | tee /tmp/f002-evolution.log
cargo test -p context-graph-core johari -- --nocapture 2>&1 | tee /tmp/f002-johari.log
cargo test -p context-graph-core teleological -- --nocapture 2>&1 | tee /tmp/f002-teleological.log

# Run ALL fingerprint tests (including F001)
cargo test -p context-graph-core fingerprint -- --nocapture 2>&1 | tee /tmp/f002-all.log

# Verify test counts
grep -c "test result:" /tmp/f002-*.log

# Check for any failures
grep -i "failed\|error\|panic" /tmp/f002-*.log
```

---

## Full State Verification Protocol

### Phase 1: Source of Truth Verification
Before modifying any files, verify current state:

```bash
# 1. Document current fingerprint module state
echo "=== BEFORE STATE ===" > /tmp/f002-state-before.log
ls -la crates/context-graph-core/src/types/fingerprint/ >> /tmp/f002-state-before.log
echo "" >> /tmp/f002-state-before.log

# 2. Run existing tests
echo "=== EXISTING TESTS ===" >> /tmp/f002-state-before.log
cargo test -p context-graph-core fingerprint 2>&1 | tail -20 >> /tmp/f002-state-before.log

# 3. Document mod.rs exports
echo "=== MOD.RS EXPORTS ===" >> /tmp/f002-state-before.log
cat crates/context-graph-core/src/types/fingerprint/mod.rs >> /tmp/f002-state-before.log

# Print for evidence
cat /tmp/f002-state-before.log
```

### Phase 2: Execute & Inspect
After creating each file, verify:

```bash
# After each file creation:
cargo check -p context-graph-core 2>&1 || echo "COMPILE ERROR - FIX BEFORE CONTINUING"

# After all files created:
echo "=== AFTER STATE ===" > /tmp/f002-state-after.log
ls -la crates/context-graph-core/src/types/fingerprint/ >> /tmp/f002-state-after.log

# Run all new tests
cargo test -p context-graph-core fingerprint 2>&1 | tee -a /tmp/f002-state-after.log
```

### Phase 3: Boundary Case Audits
Each test must print before/after state:

```rust
// Example pattern for all tests:
println!("[TEST] test_name - BEFORE STATE:");
println!("  - Input: {:?}", input);

// Execute test

println!("[TEST] test_name - AFTER STATE:");
println!("  - Output: {:?}", output);
println!("[PASS/FAIL] test_name - assertion result");
```

### Phase 4: Evidence of Success
Final verification must produce:

```bash
# Generate evidence file
cat << 'EOF' > /tmp/f002-evidence.md
# TASK-F002 Implementation Evidence

## Test Results
$(cargo test -p context-graph-core fingerprint 2>&1 | tail -30)

## File Structure
$(ls -la crates/context-graph-core/src/types/fingerprint/)

## Compile Status
$(cargo check -p context-graph-core 2>&1 | tail -10)

## New Type Exports
$(grep "pub use" crates/context-graph-core/src/types/fingerprint/mod.rs)
EOF

cat /tmp/f002-evidence.md
```

---

## Sherlock-Holmes Verification Task

After implementation, spawn a sherlock-holmes agent with this exact prompt:

```
FORENSIC INVESTIGATION: TASK-F002 TeleologicalFingerprint Implementation

TARGET FILES:
- crates/context-graph-core/src/types/fingerprint/purpose.rs
- crates/context-graph-core/src/types/fingerprint/evolution.rs
- crates/context-graph-core/src/types/fingerprint/johari.rs
- crates/context-graph-core/src/types/fingerprint/teleological.rs
- crates/context-graph-core/src/types/fingerprint/mod.rs

INVESTIGATION REQUIREMENTS:

1. CODE CORRECTNESS AUDIT:
   - Verify AlignmentThreshold boundaries match constitution.yaml (0.75, 0.70, 0.55)
   - Verify MISALIGNMENT_THRESHOLD = -0.15
   - Verify MAX_EVOLUTION_SNAPSHOTS = 100
   - Verify NUM_EMBEDDERS = 12 matches SemanticFingerprint
   - Verify PurposeVector uses [f32; 12] not Vec<f32>

2. TEST COVERAGE AUDIT:
   - Run: cargo test -p context-graph-core fingerprint -- --nocapture
   - Verify at least 20 new tests for F002 types
   - Verify all tests print before/after state
   - Verify no mock data (only computed values)

3. INTEGRATION AUDIT:
   - Verify mod.rs exports all new types
   - Verify TeleologicalFingerprint uses actual SemanticFingerprint (not stub)
   - Verify JohariFingerprint stub compiles with JohariQuadrant from types/johari/

4. BOUNDARY CASE VERIFICATION:
   - Test theta = 0.75 exactly (should be Optimal)
   - Test theta = 0.749999 (should be Acceptable)
   - Test delta = -0.15 exactly (should NOT trigger warning)
   - Test delta = -0.16 (should trigger warning)
   - Test evolution at MAX_EVOLUTION_SNAPSHOTS + 1

5. EVIDENCE COLLECTION:
   - Save all test output to /tmp/f002-sherlock-evidence.log
   - Report any discrepancies from specification
   - Verdict: PASS/FAIL with specific findings

ASSUME ALL CODE IS GUILTY UNTIL PROVEN INNOCENT.
```

---

## Constraints

- **Alignment thresholds**: From Royse 2026 research (constitution.yaml)
  - Optimal: θ ≥ 0.75
  - Acceptable: θ ∈ [0.70, 0.75)
  - Warning: θ ∈ [0.55, 0.70)
  - Critical: θ < 0.55
- **Misalignment predictor**: delta_A < -0.15 predicts failure 72 hours ahead
- **MAX_EVOLUTION_SNAPSHOTS**: 100 (older snapshots go to TimescaleDB in production)
- **Purpose vector**: 13D - one alignment per embedder (E1-E13)
- **Stage scores**: 5D - one score per pipeline stage
- **UUID**: v4 for fingerprint ID
- **Content hash**: SHA-256 (32 bytes)
- **Timestamps**: All in UTC (chrono::Utc)
- **No backwards compatibility**: Fail fast if dependencies missing
- **No mock data**: All test values must be computed, not hardcoded magic

## 5-Stage Pipeline Integration

The `stage_scores` field tracks how well each memory performs at each pipeline stage:

| Stage | Index | Name | Purpose |
|-------|-------|------|---------|
| 1 | 0 | Recall | BM25 + E13 SPLADE initial selection score |
| 2 | 1 | Semantic | E1 Matryoshka 128D fast filtering score |
| 3 | 2 | Precision | Full E1-E12 dense ranking score |
| 4 | 3 | Rerank | Cross-encoder / E12 ColBERT late interaction score |
| 5 | 4 | Teleological | Purpose vector alignment filtering score |

This enables:
- Pipeline stage performance analysis
- Memory quality assessment across stages
- Optimization of stage-specific retrieval strategies

---

## Dependency Graph

```
TASK-F001 (SemanticFingerprint)  ──────────────────┐
    Status: ✅ COMPLETE (36 tests)                 │
    Location: fingerprint/semantic.rs              │
                                                   ▼
TASK-F003 (JohariFingerprint)  ────────────► TASK-F002 (TeleologicalFingerprint)
    Status: ❌ NOT STARTED (BLOCKING)              Status: ⏳ BLOCKED
    Location: fingerprint/johari.rs (stub)         Location: fingerprint/teleological.rs
    Depends on: JohariQuadrant (exists)            Depends on: F001 ✅, F003 ❌

    ┌──────────────────────────────────────────────┘
    │
    ▼
JohariQuadrant (enum)
    Status: ✅ EXISTS
    Location: types/johari/quadrant.rs
```

**IMPLEMENTATION ORDER**:
1. ~~TASK-F001~~ ✅ DONE
2. TASK-F003 ❌ MUST COMPLETE FIRST
3. TASK-F002 (this task) - can proceed with stub once F003 is done

**WORKAROUND FOR NOW**: The johari.rs stub allows F002 to compile. Full JohariFingerprint implementation is in TASK-F003.

---

## Notes

- This task implements the core of the Teleological Vector Architecture from constitution.yaml
- The 12-embedding array IS the teleological vector - no fusion layer needed
- Purpose Vector tracks alignment to North Star goal across all 12 semantic dimensions
- Evolution tracking enables predictive misalignment detection (72 hours ahead)
- Reference: TECH-SPEC-001 Section 1.2 (TS-102)

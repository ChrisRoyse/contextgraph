# TASK-TCT-002 — Internal data models + error (ShrinkageOrigin, ChunkGtauOutput, ConstellationHingeOutput, VerdictGuardRejected, PredictorOutput, RollingWindow, TctError)

```xml
<task_spec id="TASK-TCT-002" version="1.0">

<metadata>
  <title>TCT internal types + error in `crates/context-graph-mejepa-tct/src/{shrinkage.rs, verdict.rs, hinge_types.rs, rate.rs (types only), error.rs}`</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>2</sequence>
  <implements>
    <item ref="data_model:ShrinkageOrigin">OwnCell, LanguageAggregate, EntityAggregate, CategoryAggregate</item>
    <item ref="data_model:ChunkGtauOutput">per_chunk_results, violating_chunks, aggregate_satisfied</item>
    <item ref="data_model:ConstellationHingeOutput">per_embedder_hinges, total_hinge, centroid_origin</item>
    <item ref="data_model:VerdictGuardRejected">violating_embedders + predictor_predicted + violating_chunks + constellation_version_id</item>
    <item ref="data_model:PredictorOutput">predicted_class + per_test_probabilities + confidence_interval + latent_density</item>
    <item ref="data_model:RollingWindow">window_size_calls + window_hours</item>
    <item ref="error_taxonomy">TctError + .code() taxonomy: MEJEPA_INFER_CONSTELLATION_VIOLATION, MEJEPA_INFER_GTAU_STALE_CONSTELLATION, MEJEPA_TCT_INSUFFICIENT_SAMPLES, MEJEPA_TCT_THRESHOLD_CALIBRATION_FAIL, MEJEPA_TCT_PROVENANCE_MISMATCH, MEJEPA_INSTR_FROZEN_VIOLATION, MEJEPA_INFER_DIM_MISMATCH</item>
    <item ref="REQ-TCT-10">Verdict::GuardRejected variant payload definition</item>
  </implements>
  <depends_on>
    <task_ref>TASK-TCT-001</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>5 created (shrinkage.rs, verdict.rs, hinge_types.rs, rate.rs, error.rs)</estimated_files>
</metadata>

<context>
This task lands the internal types that downstream tasks (003-012) all consume. TctError is THE
single failure surface; its `.code()` method must return SCREAMING_SNAKE tags listed verbatim in
constitution &lt;failure_taxonomy&gt; + this spec's error_states. ShrinkageOrigin replaces the
placeholder from TASK-TCT-001. VerdictGuardRejected is the cross-cutting payload that
SPEC-PHASE4-INFERENCE consumes via FailedGate::ConstellationGuardRejected.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (failure_taxonomy: MEJEPA_INFER_CONSTELLATION_VIOLATION, MEJEPA_INFER_GTAU_STALE_CONSTELLATION; tct_invariants rule 1)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-TCT.md (data_requirements VerdictGuardRejected; error_states ERR-TCT-01..06)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-TCT.md (data_models block; error_taxonomy)</file>
  <file purpose="upstream_types">crates/context-graph-mejepa-tct/src/{types.rs, constellation.rs} (TASK-TCT-001)</file>
  <file purpose="cross_domain_inference">crates/context-graph-mejepa/src/types.rs (Phase 4a FailedGate::ConstellationGuardRejected variant signature)</file>
</input_context_files>

<prerequisites>
  <check>TASK-TCT-001 complete; types.rs + constellation.rs ship leaf types + TctConstellation</check>
  <check>`thiserror = "1"` workspace dep</check>
  <check>`tracing = "0.1"` workspace dep</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `src/shrinkage.rs` with `ShrinkageOrigin` enum (4 variants: OwnCell, LanguageAggregate, EntityAggregate, CategoryAggregate); replace `ShrinkageOriginPlaceholder` references in TASK-TCT-001 with the new enum
    - Create `src/verdict.rs` with:
      - `VerdictGuardRejected { violating_embedders: Vec<GtauViolation>, predictor_predicted: PredictorOutput, violating_chunks: Vec<ChunkId>, constellation_version_id: [u8; 32] }`
      - `PredictorOutput { predicted_class: MutationCategory, per_test_probabilities: Vec<f32>, confidence_interval: (f32, f32), latent_density: f32 }`
      - Both with `try_new` validating finite / clamped fields
    - Create `src/hinge_types.rs` with:
      - `ConstellationHingeOutput { per_embedder_hinges: BTreeMap<EmbedderId, f32>, total_hinge: f32, centroid_origin: BTreeMap<EmbedderId, ShrinkageOrigin> }` with `try_new` validating non-negative + finite hinges + 21-entry coverage
      - `ChunkGtauOutput { per_chunk_results: Vec<(ChunkId, GtauOutput)>, violating_chunks: Vec<ChunkId>, aggregate_satisfied: bool }`
    - Create `src/rate.rs` (types only — actual aggregator logic in TASK-TCT-010):
      - `RollingWindow { window_size_calls: u32, window_hours: u32 }` with `Default` matching MEJEPA_TCT_RATE_WINDOW_SIZE=1000 + MEJEPA_TCT_RATE_WINDOW_HOURS=24
    - Create `src/error.rs` with `TctError` enum (8 variants per error_taxonomy in TECH-TCT) + `.code()` returning SCREAMING_SNAKE + `.log_context()` emitting structured tracing event
    - Update `crates/context-graph-mejepa-tct/src/types.rs` to replace `ShrinkageOriginPlaceholder` with `ShrinkageOrigin`; update `GtauOutput.centroid_origin` field type
    - Update `crates/context-graph-mejepa-tct/src/lib.rs`: `pub mod {types, constellation, shrinkage, verdict, hinge_types, rate, error};` + re-export
    - Replace TASK-TCT-001's placeholder `Result<_, &'static str>` returns in `try_new` constructors with `Result<_, TctError>` (NanDetected / DimMismatch / InsufficientSamples variants where appropriate)
    - Add unit tests:
      - `shrinkage_origin_serde_four_variants`
      - `predictor_output_rejects_per_test_probabilities_out_of_range`
      - `predictor_output_rejects_inverted_confidence_interval` (lo > hi)
      - `constellation_hinge_output_rejects_negative_hinge`
      - `constellation_hinge_output_rejects_partial_embedder_coverage` (only 20 keys)
      - `chunk_gtau_output_aggregate_satisfied_iff_violating_chunks_empty`
      - `verdict_guard_rejected_rejects_empty_violating_embedders` (otherwise Gτ would not have rejected per data_requirements)
      - `tct_error_code_taxonomy_matches_constitution`: 8 variants → 8 SCREAMING_SNAKE tags
      - `rolling_window_default_matches_doc_13`: window_size_calls=1000, window_hours=24
  </in_scope>
  <out_of_scope>
    - Actual hinge / Gτ / shrinkage logic (TASK-TCT-004/006/007)
    - RocksDB persistence (TASK-TCT-008)
    - Rolling-window aggregation (TASK-TCT-010)
    - MCP wiring (TASK-TCT-011)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-tct/src/shrinkage.rs">
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShrinkageOrigin {
    OwnCell, LanguageAggregate, EntityAggregate, CategoryAggregate,
}
    </signature>
    <signature file="crates/context-graph-mejepa-tct/src/verdict.rs">
use serde::{Deserialize, Serialize};
use crate::types::{ChunkId, GtauViolation, MutationCategory};
use crate::error::TctError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PredictorOutput {
    pub predicted_class: MutationCategory,
    pub per_test_probabilities: Vec&lt;f32&gt;,
    pub confidence_interval: (f32, f32),
    pub latent_density: f32,
}

impl PredictorOutput {
    pub fn try_new(predicted_class: MutationCategory,
                   per_test_probabilities: Vec&lt;f32&gt;,
                   confidence_interval: (f32, f32),
                   latent_density: f32)
        -&gt; Result&lt;Self, TctError&gt;;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VerdictGuardRejected {
    pub violating_embedders: Vec&lt;GtauViolation&gt;,
    pub predictor_predicted: PredictorOutput,
    pub violating_chunks: Vec&lt;ChunkId&gt;,
    pub constellation_version_id: [u8; 32],
}

impl VerdictGuardRejected {
    pub fn try_new(violating_embedders: Vec&lt;GtauViolation&gt;,
                   predictor_predicted: PredictorOutput,
                   violating_chunks: Vec&lt;ChunkId&gt;,
                   constellation_version_id: [u8; 32])
        -&gt; Result&lt;Self, TctError&gt;;
}
    </signature>
    <signature file="crates/context-graph-mejepa-tct/src/hinge_types.rs">
use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use crate::types::{ChunkId, EmbedderId, GtauOutput};
use crate::shrinkage::ShrinkageOrigin;
use crate::error::TctError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConstellationHingeOutput {
    pub per_embedder_hinges: BTreeMap&lt;EmbedderId, f32&gt;,
    pub total_hinge: f32,
    pub centroid_origin: BTreeMap&lt;EmbedderId, ShrinkageOrigin&gt;,
}

impl ConstellationHingeOutput {
    pub fn try_new(per_embedder_hinges: BTreeMap&lt;EmbedderId, f32&gt;,
                   centroid_origin: BTreeMap&lt;EmbedderId, ShrinkageOrigin&gt;)
        -&gt; Result&lt;Self, TctError&gt;;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChunkGtauOutput {
    pub per_chunk_results: Vec&lt;(ChunkId, GtauOutput)&gt;,
    pub violating_chunks: Vec&lt;ChunkId&gt;,
    pub aggregate_satisfied: bool,
}
    </signature>
    <signature file="crates/context-graph-mejepa-tct/src/rate.rs">
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct RollingWindow {
    pub window_size_calls: u32,
    pub window_hours: u32,
}

impl Default for RollingWindow {
    fn default() -&gt; Self { Self { window_size_calls: 1_000, window_hours: 24 } }
}
    </signature>
    <signature file="crates/context-graph-mejepa-tct/src/error.rs">
use crate::types::EmbedderId;

#[derive(Debug, thiserror::Error)]
pub enum TctError {
    #[error("constellation violation: {violating_count} embedders below threshold")]
    ConstellationViolation { violating_count: usize },
    #[error("gtau stale constellation: frozen_at={frozen_at_iso} age_days={age_days} max_age_days={max_age_days}")]
    StaleConstellation { frozen_at_iso: String, age_days: u32, max_age_days: u32 },
    #[error("insufficient samples: cell={cell} N={observed} required={required}")]
    InsufficientSamples { cell: String, observed: usize, required: usize },
    #[error("threshold calibration fail: cells_failed={cells_failed:?} target_fpr={target_fpr}")]
    ThresholdCalibrationFail { cells_failed: Vec&lt;String&gt;, observed_fpr: Vec&lt;f32&gt;, target_fpr: f32 },
    #[error("provenance mismatch: embedder={embedder:?} expected={expected:?} observed={observed:?}")]
    ProvenanceMismatch { embedder: EmbedderId, expected: [u8; 32], observed: [u8; 32] },
    #[error("frozen violation: nonzero gradient on centroid or embedder weight")]
    FrozenViolation { component: String },
    #[error("dim mismatch: expected={expected} actual={actual}")]
    DimMismatch { expected: usize, actual: usize },
    #[error("nan detected: field={field}")]
    NanDetected { field: String },
}

impl TctError {
    pub fn code(&amp;self) -&gt; &amp;'static str {
        match self {
            Self::ConstellationViolation { .. } =&gt; "MEJEPA_INFER_CONSTELLATION_VIOLATION",
            Self::StaleConstellation { .. } =&gt; "MEJEPA_INFER_GTAU_STALE_CONSTELLATION",
            Self::InsufficientSamples { .. } =&gt; "MEJEPA_TCT_INSUFFICIENT_SAMPLES",
            Self::ThresholdCalibrationFail { .. } =&gt; "MEJEPA_TCT_THRESHOLD_CALIBRATION_FAIL",
            Self::ProvenanceMismatch { .. } =&gt; "MEJEPA_TCT_PROVENANCE_MISMATCH",
            Self::FrozenViolation { .. } =&gt; "MEJEPA_INSTR_FROZEN_VIOLATION",
            Self::DimMismatch { .. } =&gt; "MEJEPA_INFER_DIM_MISMATCH",
            Self::NanDetected { .. } =&gt; "MEJEPA_INFER_DIM_MISMATCH",
        }
    }
    pub fn log_context(&amp;self, target: &amp;str);
}
    </signature>
  </signatures>
  <constraints>
    - `.code()` returns ONLY values from constitution &lt;failure_taxonomy&gt; + TECH-TCT error_taxonomy verbatim
    - PredictorOutput: per_test_probabilities all ∈ [0,1] and finite; confidence_interval lo ≤ hi and both ∈ [0,1]; latent_density finite ≥ 0
    - VerdictGuardRejected.violating_embedders MUST be non-empty (otherwise Gτ would not have rejected per AC-TCT-03-01)
    - ConstellationHingeOutput: per_embedder_hinges has all 21 EmbedderId keys (full coverage); each value ≥ 0.0 finite; total_hinge = sum (verified at try_new)
    - ChunkGtauOutput: aggregate_satisfied = violating_chunks.is_empty()
    - RollingWindow defaults match TECH-TCT data_models (window_size_calls=1000, window_hours=24)
    - log_context emits via `tracing::error!` with structured fields
    - Replace placeholder GtauOutput.centroid_origin type from TASK-TCT-001 with the new ShrinkageOrigin
    - All `try_new` constructors take ownership; never silently drop fields
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa-tct` succeeds zero warnings
    - `cargo test -p context-graph-mejepa-tct` shows ≥ 9 new tests (total ≥ 21 with TASK-TCT-001's 12)
    - `cargo clippy -p context-graph-mejepa-tct -- -D warnings` clean
    - tct_error_code_taxonomy returns 8 SCREAMING_SNAKE tags
    - VerdictGuardRejected rejects empty violating_embedders
    - ConstellationHingeOutput rejects partial 20-key coverage
    - PredictorOutput rejects inverted confidence_interval
    - GtauOutput.centroid_origin uses ShrinkageOrigin enum (not placeholder)
  </verification>
</definition_of_done>

<pseudo_code>
// verdict.rs
impl VerdictGuardRejected {
    pub fn try_new(violating_embedders, predictor_predicted, violating_chunks, version_id) -> Result<Self, TctError> {
        if violating_embedders.is_empty() {
            return Err(TctError::DimMismatch { expected: 1, actual: 0 });
        }
        Ok(Self { violating_embedders, predictor_predicted, violating_chunks, constellation_version_id: version_id })
    }
}

impl PredictorOutput {
    pub fn try_new(predicted_class, per_test_probabilities, confidence_interval, latent_density) -> Result<Self, TctError> {
        for p in &per_test_probabilities {
            if !p.is_finite() || !(0.0..=1.0).contains(p) {
                return Err(TctError::NanDetected { field: "per_test_probability".into() });
            }
        }
        let (lo, hi) = confidence_interval;
        if !lo.is_finite() || !hi.is_finite() || !(0.0..=1.0).contains(&lo) || !(0.0..=1.0).contains(&hi) || lo > hi {
            return Err(TctError::DimMismatch { expected: 1, actual: 0 });
        }
        if !latent_density.is_finite() || latent_density < 0.0 {
            return Err(TctError::NanDetected { field: "latent_density".into() });
        }
        Ok(Self { predicted_class, per_test_probabilities, confidence_interval, latent_density })
    }
}

// hinge_types.rs
impl ConstellationHingeOutput {
    pub fn try_new(per_embedder_hinges, centroid_origin) -> Result<Self, TctError> {
        if per_embedder_hinges.len() != 21 {
            return Err(TctError::DimMismatch { expected: 21, actual: per_embedder_hinges.len() });
        }
        let mut total: f32 = 0.0;
        for (_, h) in &per_embedder_hinges {
            if !h.is_finite() || *h < 0.0 {
                return Err(TctError::NanDetected { field: "hinge".into() });
            }
            total += h;
        }
        Ok(Self { per_embedder_hinges, total_hinge: total, centroid_origin })
    }
}

// error.rs
impl TctError {
    pub fn log_context(&self, target: &str) {
        use tracing::error;
        match self {
            Self::ConstellationViolation { violating_count } =>
                error!(target, code = self.code(), violating_count, "constellation violation"),
            Self::StaleConstellation { frozen_at_iso, age_days, max_age_days } =>
                error!(target, code = self.code(), frozen_at_iso, age_days, max_age_days, "stale constellation"),
            // ... per-variant ...
        }
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-tct/src/shrinkage.rs">ShrinkageOrigin enum + serde</file>
  <file path="crates/context-graph-mejepa-tct/src/verdict.rs">PredictorOutput + VerdictGuardRejected + try_new + tests</file>
  <file path="crates/context-graph-mejepa-tct/src/hinge_types.rs">ConstellationHingeOutput + ChunkGtauOutput + try_new + tests</file>
  <file path="crates/context-graph-mejepa-tct/src/rate.rs">RollingWindow + Default + tests</file>
  <file path="crates/context-graph-mejepa-tct/src/error.rs">TctError + .code() + log_context + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-tct/src/lib.rs">add module declarations + re-exports</file>
  <file path="crates/context-graph-mejepa-tct/src/types.rs">replace ShrinkageOriginPlaceholder with shrinkage::ShrinkageOrigin in GtauOutput.centroid_origin; update affected tests</file>
  <file path="crates/context-graph-mejepa-tct/src/constellation.rs">replace `Result&lt;_, &amp;'static str&gt;` with `Result&lt;_, TctError&gt;` in try_new; update tests</file>
  <file path="crates/context-graph-mejepa-tct/Cargo.toml">add `thiserror`, `tracing`, `chrono`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa-tct zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa-tct shows ≥ 21 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa-tct -- -D warnings clean</criterion>
  <criterion>TctError has 8 variants; .code() returns 7 unique SCREAMING_SNAKE tags (DimMismatch + NanDetected share MEJEPA_INFER_DIM_MISMATCH per error_taxonomy)</criterion>
  <criterion>VerdictGuardRejected rejects empty violating_embedders</criterion>
  <criterion>ConstellationHingeOutput requires 21-entry coverage</criterion>
  <criterion>PredictorOutput rejects inverted confidence_interval</criterion>
  <criterion>RollingWindow::default() == { 1000, 24 }</criterion>
  <criterion>GtauOutput.centroid_origin field type updated to ShrinkageOrigin</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-tct</command>
  <command>cargo test -p context-graph-mejepa-tct</command>
  <command>cargo clippy -p context-graph-mejepa-tct -- -D warnings</command>
</test_commands>

</task_spec>
```

# TASK-TCT-005 — ThresholdCalibrator: per-embedder + per-chunk-type τ_m on held-out KnownGood; FPR ≤ 5%

```xml
<task_spec id="TASK-TCT-005" version="1.0">

<metadata>
  <title>ThresholdCalibrator in `crates/context-graph-mejepa-tct/src/calibrator.rs`</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>5</sequence>
  <implements>
    <item ref="REQ-TCT-04">τ_m calibration with FPR ≤ 5% per (embedder, language) cell with ≥ 30 samples; per-chunk-type sharper when ≥ 50</item>
    <item ref="AC-TCT-02-01">held-out KnownGood validation slice; empirical FPR ≤ 0.05</item>
    <item ref="AC-TCT-02-02">Calibration that exceeds 5% FPR fails closed with MEJEPA_TCT_THRESHOLD_CALIBRATION_FAIL</item>
    <item ref="AC-TCT-02-03">Per-chunk-type thresholds sharper than panel-level when N ≥ 50</item>
    <item ref="EC-TCT-03">N=29 cells emit warning + skip strict enforcement</item>
    <item ref="constitution:testing_requirements">Constellation Gτ false-positive rate validation: ≤5%</item>
  </implements>
  <depends_on>
    <task_ref>TASK-TCT-004</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (calibrator.rs)</estimated_files>
</metadata>

<context>
For each (embedder, language) cell with ≥ 30 held-out KnownGood samples, ThresholdCalibrator
computes the per-sample cosine to the cell's centroid, sorts ascending, picks τ_m = the 5th
percentile (i.e., FPR exactly 5% by construction). Verifies empirical FPR on the slice ≤ 0.05;
if not, fails closed. For (entity_type, embedder) cells with ≥ 50 samples, computes sharper
per-chunk-type thresholds analogously. Calibration is deterministic given the same validation
slice + code_version.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (testing_requirements; failure_taxonomy MEJEPA_TCT_THRESHOLD_CALIBRATION_FAIL)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-TCT.md (REQ-TCT-04; AC-TCT-02-01..03; BR-TCT-03; EC-TCT-03; ERR-TCT-04)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-TCT.md (component_contracts ThresholdCalibrator.calibrate; implementation_note threshold sweep)</file>
  <file purpose="upstream_constellation">crates/context-graph-mejepa-tct/src/constellation.rs (TctConstellation, lookup_centroid; TASK-TCT-001/004)</file>
  <file purpose="instruments_panel">crates/context-graph-mejepa-instruments/src/lib.rs (Panel.slot_for)</file>
</input_context_files>

<prerequisites>
  <check>TASK-TCT-001/002/003/004 complete</check>
  <check>Held-out KnownGood validation slice available; ≥ 30 per (embedder, language)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa-tct/src/calibrator.rs` with:
      - `pub struct HeldOutValidation { pub knowngood_samples: Vec<HeldOutSample> }`
      - `pub struct HeldOutSample { pub language: Language, pub entity_type: EntityType, pub mutation: MutationCategory /* always KnownGood */, pub panel: Panel }`
      - `pub struct Thresholds { pub panel_level: BTreeMap<EmbedderId, f32>, pub per_chunk_type: BTreeMap<(EntityType, EmbedderId), f32> }`
      - `pub struct CalibrationDecision { pub cell: String, pub observed_fpr: f32, pub chosen_tau: f32, pub sample_count: usize, pub strict_enforcement: bool }`
      - `pub const TARGET_FPR: f32 = 0.05;`
      - `pub const MIN_SAMPLES_PANEL_LEVEL: usize = 30;`
      - `pub const MIN_SAMPLES_CHUNK_TYPE: usize = 50;`
      - `pub fn calibrate(centroids: &TctConstellation, validation: &HeldOutValidation) -> Result<(Thresholds, Vec<CalibrationDecision>), TctError>` orchestrating panel-level + chunk-level threshold sweeps
    - For each (embedder, language) cell:
      - If sample count < MIN_SAMPLES_PANEL_LEVEL (30): emit warning, skip strict enforcement, fall back to conservative panel-level placeholder (τ = -1.0 = fully permissive); strict_enforcement=false
      - Else: compute cosines → sort ascending → pick τ at the 5th percentile → verify empirical FPR ≤ TARGET_FPR; if violated → return ThresholdCalibrationFail
    - For each (entity_type, embedder) cell with ≥ 50 samples: compute sharper per-chunk-type τ analogously; only persist if value tighter than panel-level
    - Threshold sweep: 100 candidates between observed min and max cosine; pick smallest τ yielding FPR ≤ TARGET_FPR
    - Add unit tests with synthetic samples:
      - `calibrate_fpr_at_target_passes`: 30 samples with cosines [0.95..0.99] → τ at 5th percentile; verify FPR ≤ 0.05
      - `calibrate_below_30_emits_warning_skips_enforcement`: 29 samples → CalibrationDecision.strict_enforcement=false; conservative threshold
      - `calibrate_fpr_exceeds_target_fails_closed`: synthesize cosines such that no τ achieves FPR ≤ 0.05 → ThresholdCalibrationFail with cells_failed list
      - `calibrate_per_chunk_threshold_sharper_than_panel`: per-chunk N=60, panel N=200 → chunk τ &gt; panel τ (tighter)
      - `calibrate_per_chunk_below_50_skips`: per-chunk N=49 → only panel-level τ used at runtime
      - `calibrate_deterministic_across_two_runs`: same validation slice → byte-equal Thresholds
      - `calibrate_records_decision_per_cell`: 100 cells → 100 CalibrationDecision entries
  </in_scope>
  <out_of_scope>
    - Gτ check (TASK-TCT-006)
    - Hinge primitive (TASK-TCT-007)
    - RocksDB persistence (TASK-TCT-008)
    - Forward-pass over validation slice — caller produces panels
    - Phase 5 self-healing recalibration trigger
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-tct/src/calibrator.rs">
use std::collections::BTreeMap;
use crate::types::{Language, EntityType, MutationCategory, EmbedderId};
use crate::constellation::TctConstellation;
use crate::error::TctError;
use context_graph_mejepa_instruments::Panel;

pub const TARGET_FPR: f32 = 0.05;
pub const MIN_SAMPLES_PANEL_LEVEL: usize = 30;
pub const MIN_SAMPLES_CHUNK_TYPE: usize = 50;
pub const THRESHOLD_SWEEP_CANDIDATES: usize = 100;

pub struct HeldOutValidation {
    pub knowngood_samples: Vec&lt;HeldOutSample&gt;,
}

pub struct HeldOutSample {
    pub language: Language,
    pub entity_type: EntityType,
    pub mutation: MutationCategory,    // always KnownGood per spec
    pub panel: Panel,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Thresholds {
    pub panel_level: BTreeMap&lt;EmbedderId, f32&gt;,
    pub per_chunk_type: BTreeMap&lt;(EntityType, EmbedderId), f32&gt;,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct CalibrationDecision {
    pub cell: String,
    pub observed_fpr: f32,
    pub chosen_tau: f32,
    pub sample_count: usize,
    pub strict_enforcement: bool,
}

pub fn calibrate(
    centroids: &amp;TctConstellation,
    validation: &amp;HeldOutValidation,
) -&gt; Result&lt;(Thresholds, Vec&lt;CalibrationDecision&gt;), TctError&gt;;
    </signature>
  </signatures>
  <constraints>
    - TARGET_FPR = 0.05 strict (per constitution testing_requirements + REQ-TCT-04)
    - MIN_SAMPLES_PANEL_LEVEL = 30; below → emit `tracing::warn!` + skip strict enforcement; threshold = -1.0 (permissive placeholder)
    - MIN_SAMPLES_CHUNK_TYPE = 50; below → no chunk-type entry; runtime falls back to panel-level
    - Threshold sweep: 100 candidates linearly between observed min and max cosine; pick smallest τ yielding FPR ≤ TARGET_FPR
    - Per (embedder, language) ALL cells with ≥ 30 samples MUST satisfy FPR ≤ TARGET_FPR (else ThresholdCalibrationFail)
    - Determinism: BTreeMap iteration; sort with total_cmp
    - All cosines validated finite; NaN treated as failure (NanDetected)
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa-tct` zero warnings
    - `cargo test -p context-graph-mejepa-tct calibrator` shows ≥ 7 passing tests
    - `cargo clippy -p context-graph-mejepa-tct -- -D warnings` clean
    - All FPR test scenarios pass
    - Per-chunk threshold tighter than panel-level when N ≥ 50
    - N=29 emits warning, skips strict enforcement
    - Determinism: byte-equal Thresholds across two runs
  </verification>
</definition_of_done>

<pseudo_code>
pub fn calibrate(centroids, validation) -> Result<(Thresholds, Vec<CalibrationDecision>), TctError> {
    let mut panel_level: BTreeMap<EmbedderId, f32> = BTreeMap::new();
    let mut per_chunk: BTreeMap<(EntityType, EmbedderId), f32> = BTreeMap::new();
    let mut decisions = Vec::new();
    let mut failed_cells = Vec::new();
    let mut observed_fprs = Vec::new();

    // Group samples by (embedder, language)
    let by_embedder_language = group_by_embedder_language(&validation.knowngood_samples);

    for emb in EmbedderId::all() {
        // For each language with N >= 30, compute cosines and tau
        let mut all_cosines_for_emb: Vec<f32> = Vec::new();
        for lang in Language::iter() {
            let samples = by_embedder_language.get(&(emb, lang)).unwrap_or(&vec![]);
            if samples.len() < MIN_SAMPLES_PANEL_LEVEL {
                decisions.push(CalibrationDecision {
                    cell: format!("(panel, {:?}, {:?})", emb, lang),
                    observed_fpr: 0.0, chosen_tau: -1.0,
                    sample_count: samples.len(),
                    strict_enforcement: false,
                });
                tracing::warn!(?emb, ?lang, n = samples.len(), "skipping strict enforcement");
                continue;
            }
            let cosines: Vec<f32> = samples.iter().map(|s| {
                let centroid = centroids.lookup_centroid(s.mutation, s.language, s.entity_type, emb)
                    .map(|(c, _)| c).unwrap_or(&[]);
                cosine_similarity(s.panel.slot_for(emb), centroid)
            }).collect();
            let (tau, fpr) = sweep_tau(&cosines, TARGET_FPR)?;
            if fpr > TARGET_FPR {
                failed_cells.push(format!("({:?}, {:?})", emb, lang));
                observed_fprs.push(fpr);
            }
            decisions.push(CalibrationDecision {
                cell: format!("(panel, {:?}, {:?})", emb, lang),
                observed_fpr: fpr, chosen_tau: tau,
                sample_count: samples.len(),
                strict_enforcement: true,
            });
            all_cosines_for_emb.extend(cosines);
        }
        // Aggregate panel-level threshold across languages (smallest acceptable tau)
        if !all_cosines_for_emb.is_empty() {
            let (tau, _) = sweep_tau(&all_cosines_for_emb, TARGET_FPR)?;
            panel_level.insert(emb, tau);
        }
    }

    // Same logic for per-chunk-type thresholds with MIN_SAMPLES_CHUNK_TYPE
    // ...

    if !failed_cells.is_empty() {
        return Err(TctError::ThresholdCalibrationFail {
            cells_failed: failed_cells,
            observed_fpr: observed_fprs,
            target_fpr: TARGET_FPR,
        });
    }
    Ok((Thresholds { panel_level, per_chunk_type: per_chunk }, decisions))
}

fn sweep_tau(cosines: &[f32], target_fpr: f32) -> Result<(f32, f32), TctError> {
    let mut sorted: Vec<f32> = cosines.iter().copied().collect();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let lo = sorted[0];
    let hi = *sorted.last().unwrap();
    let mut best: Option<(f32, f32)> = None;
    for i in 0..THRESHOLD_SWEEP_CANDIDATES {
        let tau = lo + (hi - lo) * (i as f32 / (THRESHOLD_SWEEP_CANDIDATES - 1) as f32);
        let fpr = sorted.iter().filter(|c| **c < tau).count() as f32 / sorted.len() as f32;
        if fpr <= target_fpr {
            match best {
                None => best = Some((tau, fpr)),
                Some((bt, _)) if tau < bt => best = Some((tau, fpr)),
                _ => {},
            }
        }
    }
    best.ok_or(TctError::ThresholdCalibrationFail {
        cells_failed: vec!["no candidate".into()],
        observed_fpr: vec![1.0],
        target_fpr,
    })
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-tct/src/calibrator.rs">HeldOutValidation + Thresholds + CalibrationDecision + calibrate + sweep_tau + 7 tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-tct/src/lib.rs">add `pub mod calibrator;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa-tct zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa-tct calibrator shows ≥ 7 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa-tct -- -D warnings clean</criterion>
  <criterion>FPR ≤ 0.05 verified on every cell with ≥ 30 samples</criterion>
  <criterion>N=29 cells emit warning, skip strict enforcement</criterion>
  <criterion>Per-chunk τ tighter than panel-level when N ≥ 50</criterion>
  <criterion>Cells violating FPR fail closed with MEJEPA_TCT_THRESHOLD_CALIBRATION_FAIL</criterion>
  <criterion>Determinism: byte-equal Thresholds across two runs</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-tct</command>
  <command>cargo test -p context-graph-mejepa-tct calibrator</command>
  <command>cargo clippy -p context-graph-mejepa-tct -- -D warnings</command>
</test_commands>

</task_spec>
```

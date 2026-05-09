# TASK-HEAL-012 — continuous_calibration_if_due (sliding 1,000 examples; τ + σ² recompute)

```xml
<task_spec id="TASK-HEAL-012" version="1.0">

<metadata>
  <title>Continuous calibration scheduler in `crates/context-graph-mejepa/src/heal/calibration.rs`</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>12</sequence>
  <implements>
    <item ref="REQ-HEAL-06">continuous_calibration_if_due every 1,000 obs; sliding-window τ + σ² recompute</item>
    <item ref="ERR-HEAL-09">MEJEPA_CALIBRATION_INSUFFICIENT_SAMPLES (status_change=degraded; fall back to last-known)</item>
    <item ref="EC-HEAL-09">σ² binary search produces non-monotone calibration on noisy holdout — emit warning + use median value</item>
  </implements>
  <depends_on>
    <task_ref>TASK-HEAL-004</task_ref>
    <task_ref>TASK-HEAL-005</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (src/heal/calibration.rs); 1 modified (src/heal/mod.rs)</estimated_files>
</metadata>

<context>
Logic 7 of 10. Implements `SelfHealingPipeline::continuous_calibration_if_due()` — the
sliding-window calibration recomputer that fires every 1,000 observations to keep the conformal
threshold τ and OOD scale σ² in sync with the live distribution. Reads the last 1,000
CalibrationExample records (per-prediction non-conformity score + OOD score), recomputes τ via
the `(⌈(N+1)·(1−α)⌉/N)`-quantile of non-conformity scores, recomputes σ² via binary search so
calibration examples score &lt; 0.30, and persists a new CalibrationRecord with provenance metadata.
On insufficient samples, returns ConformalInsufficientSamples (informational, not an error in
production: the heal pipeline falls back to the last-known CalibrationRecord and stamps every
in-flight RealityPrediction with `status_change=degraded` per ERR-HEAL-09 user_message).
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (`<failure_taxonomy>` MEJEPA_CALIBRATION_INSUFFICIENT_SAMPLES; `<provenance>` corpus_sha + embedder_versions + frozen_at on every persistent artifact)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-SELF-HEALING.md (REQ-HEAL-06; AC-HEAL-07-01..03; EC-HEAL-09; ERR-HEAL-09)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-SELF-HEALING.md (component_contracts SelfHealingPipeline::continuous_calibration_if_due 7-step body; CalibrationWriter data model)</file>
  <file purpose="technical_spec_inference">docs/ruvectorfindings/specs/technical/TECH-INFERENCE.md (CalibrationRecord schema; non-conformity score definition; σ² binary search semantics)</file>
  <file purpose="prior_task">TASK-HEAL-004 (HealError, CalibrationWriter struct, MeJepaHealConfig.continuous_calibration_period field)</file>
  <file purpose="prior_task">TASK-HEAL-005 (CF_MEJEPA_CALIBRATION_HISTORY, CF_MEJEPA_ACTIVE_POINTERS["active_calibration"])</file>
</input_context_files>

<prerequisites>
  <check>TASK-HEAL-004..005 complete</check>
  <check>Phase 4a InferenceEngine produces CalibrationExample { non_conformity_score: f32, ood_score: f32, witness_chain_offset: u64, signal_clarity: f32 } records pushed into CalibrationWriter.sliding_window</check>
  <check>CalibrationRecord schema (from TECH-INFERENCE) is stable: { tau: f32, sigma_squared: f32, alpha: f32, num_samples: usize, frozen_at: i64, corpus_sha: [u8; 32], embedder_versions: HashMap<EmbedderId, String>, calibration_version: String }</check>
  <check>HealStatus.observation_counter exposed via Arc&lt;Mutex&lt;HealStatus&gt;&gt;</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/heal/calibration.rs`:
      - `pub fn continuous_calibration_if_due(pipeline: &mut SelfHealingPipeline) -> Result<Option<CalibrationRecord>, HealError>` per component_contracts (7-step body):
        1. Read `observation_counter` from `pipeline.status.lock().unwrap().observation_counter`
        2. If `counter == 0 OR counter % continuous_calibration_period != 0`: return `Ok(None)`
        3. Pull last 1,000 CalibrationExample records from `pipeline.calibration_writer.sliding_window` (a VecDeque capped at 1,000 per TECH data model)
        4. If `sliding_window.len() < 1000`:
           - Stamp `pipeline.status.lock().unwrap().status_change = StatusChange::Degraded` so in-flight predictions inherit the degraded status
           - Log info-level `MEJEPA_CALIBRATION_INSUFFICIENT_SAMPLES`-tagged message with observed/required counts
           - Fall back to last-known CalibrationRecord (ALREADY active; no rotation needed)
           - Return `Err(HealError::ConformalInsufficientSamples { observed: sliding_window.len(), required: 1000 })`
        5. Compute new τ:
           - Sort non-conformity scores ascending: `scores: Vec<f32>` len 1000 (NaN-aware: any NaN → return Err(BatchNan { component: "calibration_tau", witness_chain_offset: 0 }))
           - Quantile index = `((N + 1) as f32 * (1.0 - alpha)).ceil() as usize` (with alpha = pipeline.calibration_writer.alpha; default 0.10)
           - `new_tau = scores[quantile_index.min(N - 1)]` (clamp to bounds)
        6. Compute new σ² via binary search:
           - Goal: find σ² such that `count(s in scores | s &lt; 0.30) / N` is >= target_fraction (default 0.5)
           - Lower bound = 1e-6; upper bound = max OOD-score among examples * 10
           - Iterate up to 50 times; tolerance 1e-4
           - On non-monotone behavior (EC-HEAL-09): emit `tracing::warn!` "σ² binary search non-monotone on noisy holdout" + use median candidate value
        7. Build new CalibrationRecord with provenance: { tau, sigma_squared, alpha, num_samples=1000, frozen_at=now_secs(), corpus_sha=current_corpus_sha(), embedder_versions=current_embedder_versions(), calibration_version=format!("v{}", new_version_int) }
        8. Persist to CF_MEJEPA_CALIBRATION_HISTORY keyed by frozen_at (big-endian i64 per TECH §rocksdb_column_families)
        9. Update CF_MEJEPA_ACTIVE_POINTERS["active_calibration"] atomically via write_text_checked (staging + fsync + atomic-rename)
        10. Stamp `pipeline.status.lock().unwrap().last_calibration_at = now_secs()`
        11. Stamp `pipeline.status.lock().unwrap().active_calibration_version = new_version_string`
        12. Restore `pipeline.status.lock().unwrap().status_change = StatusChange::Active` (if previously Degraded due to insufficient samples)
        13. Return `Ok(Some(CalibrationRecord))`
      - Helper: `pub fn compute_conformal_tau(scores: &[f32], alpha: f32) -> Result<f32, HealError>` — pure function: NaN-aware sort, quantile index per `(⌈(N+1)·(1−α)⌉/N)`-quantile
      - Helper: `pub fn binary_search_sigma_squared(ood_scores: &[f32], non_conformity_scores: &[f32], target_fraction: f32, max_iter: u32) -> Result<f32, HealError>` — returns σ² + emits warning if non-monotone
      - Helper: `pub fn try_acquire_calibration_writer_lock(writer: &Arc<Mutex<CalibrationWriter>>) -> MutexGuard<CalibrationWriter>` — exclusive write during recompute
    - Tests:
      - `continuous_calibration_returns_none_below_period`
      - `continuous_calibration_fires_at_exactly_1000_observations`
      - `continuous_calibration_fires_at_2000_3000_etc`
      - `continuous_calibration_returns_insufficient_samples_when_window_lt_1000` (ERR-HEAL-09)
      - `continuous_calibration_stamps_status_change_degraded_on_insufficient_samples`
      - `continuous_calibration_falls_back_to_last_known_on_insufficient_samples`
      - `continuous_calibration_restores_active_status_after_recovery`
      - `compute_conformal_tau_quantile_textbook_correct` (assert against scipy.stats.quantile output for known input)
      - `compute_conformal_tau_rejects_nan_scores` (BatchNan)
      - `binary_search_sigma_squared_converges_within_50_iterations`
      - `binary_search_sigma_squared_emits_warning_on_non_monotone` (EC-HEAL-09)
      - `calibration_record_provenance_has_corpus_sha_and_embedder_versions`
      - `calibration_record_persisted_to_history_with_frozen_at_key` (big-endian i64 key format)
      - `active_calibration_pointer_atomic_after_history_persist`
      - End-to-end synthetic 5,000-obs run: assert exactly 5 CalibrationRecord rotations + verify τ trajectory non-degenerate
    - FSV evidence emission for TASK-HEAL-019: `CalibrationEvidence { observation_counter_at_fire, num_samples, tau_before, tau_after, sigma_squared_before, sigma_squared_after, calibration_version_before, calibration_version_after, fallback_used }`
  </in_scope>
  <out_of_scope>
    - The CalibrationExample producer (Phase 4a InferenceEngine; cross-cutting)
    - The non-conformity score formula itself (TECH-INFERENCE)
    - Phase 6 hygiene retention pruning of old CalibrationRecords (cross-cutting)
    - schedule_full_retrain (TASK-HEAL-011)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/heal/calibration.rs">
use std::sync::{Arc, Mutex};
use crate::heal::{pipeline::SelfHealingPipeline, errors::HealError, status::StatusChange};
use context_graph_mejepa_inference::CalibrationRecord;

pub fn continuous_calibration_if_due(
    pipeline: &amp;mut SelfHealingPipeline,
) -&gt; Result&lt;Option&lt;CalibrationRecord&gt;, HealError&gt;;

pub fn compute_conformal_tau(scores: &amp;[f32], alpha: f32) -&gt; Result&lt;f32, HealError&gt;;

pub fn binary_search_sigma_squared(
    ood_scores: &amp;[f32],
    non_conformity_scores: &amp;[f32],
    target_fraction: f32,
    max_iter: u32,
) -&gt; Result&lt;f32, HealError&gt;;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CalibrationEvidence {
    pub observation_counter_at_fire: u64,
    pub num_samples: usize,
    pub tau_before: f32,
    pub tau_after: f32,
    pub sigma_squared_before: f32,
    pub sigma_squared_after: f32,
    pub calibration_version_before: String,
    pub calibration_version_after: String,
    pub fallback_used: bool,
}
    </signature>
  </signatures>
  <constraints>
    - Insufficient samples is ERR-HEAL-09 returned as Err(ConformalInsufficientSamples) but is NON-FATAL to the surrounding heal cycle: the caller (typically observe()) catches it, falls back to last-known calibration, stamps status_change=degraded, and continues
    - τ formula MUST be `(⌈(N+1)·(1−α)⌉/N)`-quantile (per technical spec; NOT (N+1)·(1−α) without ceil; NOT N·(1−α))
    - σ² binary search bounded to 50 iterations; tolerance 1e-4; NaN/Inf in scores → return Err(BatchNan)
    - EC-HEAL-09 non-monotone behavior: warn + use median candidate (do NOT silently pick lower bound; do NOT loop indefinitely)
    - Provenance metadata REQUIRED on every CalibrationRecord per constitution `<provenance>` rule (corpus_sha, embedder_versions, frozen_at, calibration_version)
    - Atomic write semantics: history persist FIRST, then active-pointer update (history is the durable record; active-pointer is the read path)
    - File &lt; 500 lines
    - SCREAMING_SNAKE error codes: MEJEPA_CALIBRATION_INSUFFICIENT_SAMPLES, MEJEPA_OBSERVE_BATCH_NAN
    - No mock data in tests; real RocksDB tempdir + real CalibrationExample fixtures (synthetic but deterministic)
    - No `unwrap_or(0.5)` fallback on ANY field per constitution `<anti_patterns>`
  </constraints>
  <verification>
    - `cargo test -p context-graph-mejepa heal::calibration` shows ≥ 15 passing tests
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - 999 samples → ConformalInsufficientSamples (status_change stamped Degraded)
    - 1000 samples → Ok(Some(record)); persisted; active-pointer rotated
    - 5,000-obs synthetic run produces 5 CalibrationRecord rotations
    - τ recomputed across rotations — values trend with synthetic distribution shift
    - σ² binary search converges within 50 iterations on textbook input
    - Non-monotone holdout emits tracing::warn! AND falls back to median (not Err)
  </verification>
</definition_of_done>

<pseudo_code>
pub fn continuous_calibration_if_due(pipeline: &mut SelfHealingPipeline) -> Result<Option<CalibrationRecord>, HealError> {
    let counter = pipeline.status.lock().unwrap().observation_counter;
    if counter == 0 || counter % pipeline.config.continuous_calibration_period != 0 {
        return Ok(None);
    }
    let writer_guard = pipeline.calibration_writer.lock().unwrap();
    let window: Vec<CalibrationExample> = writer_guard.sliding_window.iter().cloned().collect();
    drop(writer_guard);
    if window.len() < 1000 {
        pipeline.status.lock().unwrap().status_change = StatusChange::Degraded;
        tracing::info!(error_code = "MEJEPA_CALIBRATION_INSUFFICIENT_SAMPLES", observed = window.len(), required = 1000);
        return Err(HealError::ConformalInsufficientSamples { observed: window.len(), required: 1000 });
    }
    let scores: Vec<f32> = window.iter().map(|ex| ex.non_conformity_score).collect();
    let alpha = pipeline.calibration_writer.lock().unwrap().alpha;
    let new_tau = compute_conformal_tau(&scores, alpha)?;
    let ood_scores: Vec<f32> = window.iter().map(|ex| ex.ood_score).collect();
    let new_sigma_squared = binary_search_sigma_squared(&ood_scores, &scores, 0.5, 50)?;
    let new_record = CalibrationRecord {
        tau: new_tau,
        sigma_squared: new_sigma_squared,
        alpha,
        num_samples: 1000,
        frozen_at: now_secs(),
        corpus_sha: pipeline.current_corpus_sha(),
        embedder_versions: pipeline.current_embedder_versions(),
        calibration_version: format!("v{}", pipeline.next_calibration_version()),
    };
    pipeline.storage.put(CF_MEJEPA_CALIBRATION_HISTORY, &new_record.frozen_at.to_be_bytes(), &bincode::serialize(&new_record)?)?;
    pipeline.storage.put_atomic(CF_MEJEPA_ACTIVE_POINTERS, b"active_calibration", &bincode::serialize(&ActivePointerValue { theta_sha_or_version: new_record.calibration_version.as_bytes().to_vec(), frozen_at: new_record.frozen_at })?)?;
    {
        let mut status = pipeline.status.lock().unwrap();
        status.last_calibration_at = new_record.frozen_at;
        status.active_calibration_version = new_record.calibration_version.clone();
        if matches!(status.status_change, StatusChange::Degraded) { status.status_change = StatusChange::Active; }
    }
    Ok(Some(new_record))
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/heal/calibration.rs">continuous_calibration_if_due + compute_conformal_tau + binary_search_sigma_squared + CalibrationEvidence + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/heal/mod.rs">add `pub mod calibration;`; re-export continuous_calibration_if_due</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo test -p context-graph-mejepa heal::calibration shows ≥ 15 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>Counter at 999 → Ok(None); counter at 1000 → fires recompute</criterion>
  <criterion>&lt; 1000 samples → Err(ConformalInsufficientSamples) + status_change=Degraded</criterion>
  <criterion>τ formula matches `(⌈(N+1)·(1−α)⌉/N)`-quantile (verified against textbook reference)</criterion>
  <criterion>σ² binary search converges within 50 iterations</criterion>
  <criterion>Non-monotone behavior emits warn! + falls back to median (NOT Err)</criterion>
  <criterion>CalibrationRecord persisted with provenance metadata (corpus_sha + embedder_versions + frozen_at)</criterion>
  <criterion>5,000-obs run produces 5 CalibrationRecord rotations + active-pointer updated each time</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa heal::calibration</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```

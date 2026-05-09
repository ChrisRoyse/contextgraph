# TASK-HEAL-004 — Pipeline-level data models + errors (HealReport, HealError, IntegrityViolationKind, CriticalBugKind, LoraRefresher, OnlineDistiller, ChainIntegrityChecker, CalibrationWriter, PromotionLockState, HealStatus)

```xml
<task_spec id="TASK-HEAL-004" version="1.0">

<metadata>
  <title>Pipeline + error data models in `crates/context-graph-mejepa/src/heal/{errors,status,calibration,integrity,lora_refresh,distill}.rs` (struct definitions only; method bodies in subsequent logic tasks)</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>4</sequence>
  <implements>
    <item ref="REQ-HEAL-14">HealError taxonomy with .code() returning SCREAMING_SNAKE per constitution failure_taxonomy</item>
    <item ref="REQ-HEAL-17">HealStatus snapshot for mejepa_heal_status MCP tool</item>
    <item ref="data_model:HealReport">mode_winner + per-mode scores + weights_sha_winner + evaluation_summary_sha + witness_chain_offset + promotion_latency_seconds</item>
    <item ref="data_model:HealError">12 variants per data_models block</item>
    <item ref="data_model:IntegrityViolationKind">WrongSize / BrokenAt</item>
    <item ref="data_model:CriticalBugKind">TargetCollapseNonZero / StabilityFloorZero</item>
    <item ref="data_model:LoraRefresher">handle + rank=32 + epochs=10 + alpha=16 + dropout=0.1</item>
    <item ref="data_model:OnlineDistiller">handle + ema_tau=0.996 + quality_gate=true + signal_clarity_threshold=0.5</item>
    <item ref="data_model:ChainIntegrityChecker">chain_path + last_verified_offset + rollback_target</item>
    <item ref="data_model:CalibrationWriter">sliding_window + alpha=0.10 + last_record</item>
    <item ref="data_model:PromotionLockState">held_by + acquired_at + trigger_reason</item>
    <item ref="data_model:HealStatus">drift_severity + last_promotion_at + observation_counter + ewc_lambda + fisher_rank + active_weights_sha</item>
  </implements>
  <depends_on>
    <task_ref>TASK-HEAL-001</task_ref>
    <task_ref>TASK-HEAL-002</task_ref>
    <task_ref>TASK-HEAL-003</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>6 created (errors.rs, status.rs, calibration.rs, integrity.rs, lora_refresh.rs, distill.rs)</estimated_files>
</metadata>

<context>
Fourth foundation task. Lands the pipeline-orchestration data shapes that wire EWC++ (TASK-HEAL-001),
DriftDetector (TASK-HEAL-002), and AbcPromoter (TASK-HEAL-003) into a single SelfHealingPipeline
contract. Also defines the canonical HealError taxonomy with `.code()` returning SCREAMING_SNAKE
tags per constitution `<failure_taxonomy>`. Six small files (one per concern) keep each under the
500-line cap. Method bodies for verify / refresh / consume / sliding-window calibration / EMA update
are deferred to the corresponding logic tasks (013, 014, 015, 012). The shipped LoRA + distillation
+ witness-chain crates are wrapped here via Arc handles — NOT reinvented per integration_notes.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (`<failure_taxonomy>` all MEJEPA_HEAL_* + MEJEPA_OBSERVE_* codes; `<error_handling>` rule on `.code()` SCREAMING_SNAKE)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-SELF-HEALING.md (REQ-HEAL-14 taxonomy; REQ-HEAL-17 status; ERR-HEAL-01..10 user/internal messages)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-SELF-HEALING.md (data_models block: HealReport, HealError, IntegrityViolationKind, CriticalBugKind, LoraRefresher, OnlineDistiller, ChainIntegrityChecker, CalibrationWriter, PromotionLockState, HealStatus)</file>
  <file purpose="prior_task">TASK-HEAL-001 (EwcPlusPlus types for HealStatus.fisher_rank etc.)</file>
  <file purpose="prior_task">TASK-HEAL-002 (DriftSeverity for HealStatus.drift_severity)</file>
  <file purpose="prior_task">TASK-HEAL-003 (TriggerReason, ModeWinner, HoldoutEval for HealReport)</file>
  <file purpose="reuse_lora">crates/context-graph-embeddings/src/training/lora.rs (LoraTrainer Arc handle reference; do NOT modify)</file>
  <file purpose="reuse_distill">crates/context-graph-embeddings/src/training/distillation.rs (DistillationTrainer Arc handle reference)</file>
</input_context_files>

<prerequisites>
  <check>TASK-HEAL-001, 002, 003 complete</check>
  <check>`crates/context-graph-embeddings/src/training/lora.rs` exposes a `LoraTrainer` type</check>
  <check>`crates/context-graph-embeddings/src/training/distillation.rs` exposes a `DistillationTrainer` type</check>
</prerequisites>

<scope>
  <in_scope>
    - Fill `crates/context-graph-mejepa/src/heal/errors.rs`:
      - `IntegrityViolationKind` enum: `WrongSize { actual: u64, expected_modulo: u64 } | BrokenAt { offset: usize, expected_prev_hash: [u8; 32], actual_prev_hash: [u8; 32] }`
      - `CriticalBugKind` enum: `TargetCollapseNonZero { value: f32, contributing_instruments: Vec<u8> } | StabilityFloorZero { value: f32, last_good_step: u64 }` (instrument-slot tag is u8 since the InstrumentSlot enum lives in a different crate)
      - `HealError` enum with 12 variants exactly per data_models block:
        `BatchNan { component: String, witness_chain_offset: u64 }`,
        `DriftDetected { severity: crate::heal::drift::DriftSeverity, empirical_coverage: f32 }`,
        `IntegrityViolation { kind: IntegrityViolationKind }`,
        `HoldoutRegression { mode_a_score: crate::heal::promote::HoldoutEval, mode_b_score: crate::heal::promote::HoldoutEval, mode_c_score: crate::heal::promote::HoldoutEval }`,
        `QuotaExceeded { used_gb: f32, quota_gb: f32 }`,
        `FisherRankDeficient { rank: usize, dim: usize }`,
        `LoraRefreshFail { embedder_id: u32, cause: String }`,
        `PromotionDeadlock { holder: String }`,
        `ConformalInsufficientSamples { observed: usize, required: usize }`,
        `CriticalBug { kind: CriticalBugKind }`,
        `RollbackTargetGone { theta_sha: [u8; 32], gced_at: i64 }`,
        `FeatureDisabled`
      - Implement `impl HealError { pub fn code(&self) -> &'static str { match self { ... } } }` returning canonical SCREAMING_SNAKE tags per constitution failure_taxonomy: `"MEJEPA_OBSERVE_BATCH_NAN"`, `"MEJEPA_OBSERVE_DRIFT_DETECTED"`, `"MEJEPA_OBSERVE_INTEGRITY_VIOLATION"`, `"MEJEPA_RETRAIN_HOLDOUT_REGRESSION"`, `"MEJEPA_RETRAIN_QUOTA_EXCEEDED"`, `"MEJEPA_HEAL_FISHER_RANK_DEFICIENT"`, `"MEJEPA_HEAL_LORA_REFRESH_FAIL"`, `"MEJEPA_HEAL_PROMOTION_DEADLOCK"`, `"MEJEPA_CALIBRATION_INSUFFICIENT_SAMPLES"`, `"MEJEPA_HEAL_CRITICAL_BUG"`, `"MEJEPA_HEAL_ROLLBACK_TARGET_GONE"`, `"MEJEPA_HEAL_FEATURE_DISABLED"`.
      - Implement `impl std::fmt::Display for HealError` and `impl std::error::Error for HealError`.
    - Fill `crates/context-graph-mejepa/src/heal/status.rs`:
      - `StatusChange` enum: `{Active, Retraining, Degraded, Paused}` with `#[serde(rename_all = "snake_case")]`.
      - `HealStatus { drift_severity: DriftSeverity, last_promotion_at: i64, last_calibration_at: i64, last_integrity_check_at: i64, observation_counter: u64, ewc_lambda: f32, fisher_rank: usize, active_weights_sha: [u8; 32], active_calibration_version: String, active_constellation_version: String, status_change: StatusChange }`. Implement `Default::default()` returning a blank/initial state with `last_promotion_at = -1`.
      - `PromotionLockState { held_by: Option<String>, acquired_at: i64, trigger_reason: Option<TriggerReason> }` with `Default::default()` returning unlocked state.
      - `HealReport { mode_winner: ModeWinner, mode_a_score: HoldoutEval, mode_b_score: HoldoutEval, mode_c_score: HoldoutEval, mode_c_weights: (f32, f32), weights_sha_winner: [u8; 32], evaluation_summary_sha: [u8; 32], witness_chain_offset: u64, promotion_latency_seconds: u64, status_change: StatusChange, trigger_reason: TriggerReason }` with `try_new` validating `mode_c_weights.0 + mode_c_weights.1 == 1.0` within 1e-3.
    - Fill `crates/context-graph-mejepa/src/heal/calibration.rs`:
      - `pub const DEFAULT_CALIBRATION_ALPHA: f32 = 0.10;`
      - `pub const DEFAULT_CALIBRATION_WINDOW_SIZE: usize = 1000;`
      - `CalibrationExample { non_conformity_score: f32, ood_score: f32, frozen_at: i64 }` with try_new validating finite + non-conformity ∈ [0,1].
      - `CalibrationRecord { tau: f32, sigma_squared: f32, frozen_at: i64, corpus_sha: [u8; 32], embedder_versions_sha: [u8; 32], num_samples: usize }` with try_new validating tau ∈ [0,1], sigma² > 0, num_samples > 0.
      - `CalibrationWriter { sliding_window: std::collections::VecDeque<CalibrationExample>, alpha: f32, last_record: Option<CalibrationRecord>, capacity: usize }` with `try_new(alpha: f32, capacity: usize) -> Result<Self, &'static str>` validating alpha ∈ (0,1) and capacity > 0.
    - Fill `crates/context-graph-mejepa/src/heal/integrity.rs`:
      - `ChainIntegrityChecker { chain_path: std::path::PathBuf, last_verified_offset: u64, rollback_target: Option<[u8; 32]> }` with `try_new(chain_path: PathBuf) -> Result<Self, &'static str>` validating non-empty path.
      - `ChainIntegrityReport { variant: IntegrityVerdict, entry_count: u64, replay_duration_ms: u64 }` where `IntegrityVerdict` is enum `Valid | LegacyRepaired { entries_repaired: u64 }`. (Broken cases are returned as Err(HealError::IntegrityViolation) per spec.)
    - Fill `crates/context-graph-mejepa/src/heal/lora_refresh.rs`:
      - `pub const DEFAULT_LORA_RANK: usize = 32;`
      - `pub const DEFAULT_LORA_EPOCHS: u32 = 10;`
      - `pub const DEFAULT_LORA_ALPHA: f32 = 16.0;`
      - `pub const DEFAULT_LORA_DROPOUT: f32 = 0.1;`
      - `pub const DEFAULT_LORA_LR: f32 = 1e-4;`
      - `pub const PLASTICITY_REGULATE_FLOOR: f32 = 0.4;` (per UTML §IX coaching threshold)
      - `LoraRefresher { lora_handle_sha: [u8; 32], rank: usize, epochs: u32, alpha: f32, dropout: f32, lr: f32 }` (the actual `Arc<LoraTrainer>` is injected at method-body time in TASK-HEAL-014 to avoid forward import; here we only carry the handle digest for provenance). With try_new validating rank > 0, epochs > 0, dropout ∈ [0,1], alpha > 0, lr > 0.
      - `LoraRefreshReport { embedder_id: u32, pre_base_sha: [u8; 32], post_base_sha: [u8; 32], pre_lora_sha: [u8; 32], post_lora_sha: [u8; 32], pre_plasticity: f32, post_plasticity: f32, epochs_completed: u32, fresh_corpus_slice_sha: [u8; 32] }` with try_new asserting pre_base_sha == post_base_sha (the BR-HEAL-06 frozen-base invariant).
    - Fill `crates/context-graph-mejepa/src/heal/distill.rs`:
      - `pub const DEFAULT_DISTILL_EMA_TAU: f32 = 0.996;`
      - `pub const DEFAULT_DISTILL_INFO_NCE_TEMPERATURE: f32 = 0.07;`
      - `pub const DEFAULT_SIGNAL_CLARITY_THRESHOLD: f32 = 0.5;`
      - `OnlineDistiller { distill_handle_sha: [u8; 32], ema_tau: f32, quality_gate: bool, signal_clarity_threshold: f32, info_nce_temperature: f32 }` with try_new validating ema_tau ∈ (0,1), info_nce_temperature > 0, signal_clarity_threshold ∈ [0,1].
      - `DistillStep` enum: `Skipped { signal_clarity: f32 } | Applied { teacher_score: f32, student_score: f32, info_nce_loss: f32, ema_norm_pre: f32, ema_norm_post: f32 }`.
    - Add unit tests in each file; canonical set:
      - `errors.rs`: `heal_error_code_returns_screaming_snake_for_each_variant` (12 assertions)
      - `errors.rs`: `heal_error_implements_display`
      - `errors.rs`: `heal_error_implements_std_error`
      - `status.rs`: `heal_status_default_last_promotion_minus_one`
      - `status.rs`: `heal_report_rejects_mode_c_weights_not_summing_to_one`
      - `status.rs`: `status_change_serializes_snake_case`
      - `calibration.rs`: `calibration_writer_rejects_alpha_zero_or_one`
      - `calibration.rs`: `calibration_record_rejects_tau_above_one`
      - `calibration.rs`: `default_calibration_constants_match_spec`
      - `integrity.rs`: `chain_integrity_checker_rejects_empty_path`
      - `integrity.rs`: `serde_round_trip_chain_integrity_report`
      - `lora_refresh.rs`: `lora_refresher_constants_match_spec` (rank=32, epochs=10, alpha=16, dropout=0.1)
      - `lora_refresh.rs`: `lora_refresh_report_rejects_base_sha_changed` (BR-HEAL-06 invariant)
      - `lora_refresh.rs`: `plasticity_regulate_floor_is_zero_point_four`
      - `distill.rs`: `online_distiller_rejects_tau_one`
      - `distill.rs`: `default_distill_tau_is_zero_point_996`
  </in_scope>
  <out_of_scope>
    - update_fisher_online / detect_drift / observe / retrain_and_promote / refresh / consume / verify method bodies (TASK-HEAL-006 .. 015)
    - Arc<LoraTrainer> / Arc<DistillationTrainer> / Arc<RocksDbStore> / Arc<WitnessChainAppender> live wiring (deferred to TASK-HEAL-008 SelfHealingPipeline construction)
    - SelfHealingPipeline top-level struct (TASK-HEAL-008)
    - Full Arc<RwLock<Predictor>> wiring (TASK-HEAL-008)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/heal/errors.rs">
use serde::{Deserialize, Serialize};
use crate::heal::drift::DriftSeverity;
use crate::heal::promote::HoldoutEval;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IntegrityViolationKind {
    WrongSize { actual: u64, expected_modulo: u64 },
    BrokenAt { offset: usize, expected_prev_hash: [u8; 32], actual_prev_hash: [u8; 32] },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CriticalBugKind {
    TargetCollapseNonZero { value: f32, contributing_instruments: Vec&lt;u8&gt; },
    StabilityFloorZero { value: f32, last_good_step: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HealError {
    BatchNan { component: String, witness_chain_offset: u64 },
    DriftDetected { severity: DriftSeverity, empirical_coverage: f32 },
    IntegrityViolation { kind: IntegrityViolationKind },
    HoldoutRegression { mode_a_score: HoldoutEval, mode_b_score: HoldoutEval, mode_c_score: HoldoutEval },
    QuotaExceeded { used_gb: f32, quota_gb: f32 },
    FisherRankDeficient { rank: usize, dim: usize },
    LoraRefreshFail { embedder_id: u32, cause: String },
    PromotionDeadlock { holder: String },
    ConformalInsufficientSamples { observed: usize, required: usize },
    CriticalBug { kind: CriticalBugKind },
    RollbackTargetGone { theta_sha: [u8; 32], gced_at: i64 },
    FeatureDisabled,
}

impl HealError {
    pub fn code(&amp;self) -&gt; &amp;'static str;
}

impl std::fmt::Display for HealError { /* ... */ }
impl std::error::Error for HealError {}
    </signature>
    <signature file="crates/context-graph-mejepa/src/heal/status.rs">
use serde::{Deserialize, Serialize};
use crate::heal::drift::DriftSeverity;
use crate::heal::promote::{HoldoutEval, ModeWinner, TriggerReason};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusChange { Active, Retraining, Degraded, Paused }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealStatus { /* fields per data_models */ }

impl Default for HealStatus { fn default() -&gt; Self; }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromotionLockState {
    pub held_by: Option&lt;String&gt;,
    pub acquired_at: i64,
    pub trigger_reason: Option&lt;TriggerReason&gt;,
}

impl Default for PromotionLockState { fn default() -&gt; Self; }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealReport { /* fields per data_models */ }

impl HealReport {
    pub fn try_new(/* ... */) -&gt; Result&lt;Self, &amp;'static str&gt;;
}
    </signature>
    <signature file="crates/context-graph-mejepa/src/heal/calibration.rs">
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

pub const DEFAULT_CALIBRATION_ALPHA: f32 = 0.10;
pub const DEFAULT_CALIBRATION_WINDOW_SIZE: usize = 1000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct CalibrationExample {
    pub non_conformity_score: f32, pub ood_score: f32, pub frozen_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CalibrationRecord {
    pub tau: f32, pub sigma_squared: f32, pub frozen_at: i64,
    pub corpus_sha: [u8; 32], pub embedder_versions_sha: [u8; 32],
    pub num_samples: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CalibrationWriter {
    pub sliding_window: VecDeque&lt;CalibrationExample&gt;,
    pub alpha: f32,
    pub last_record: Option&lt;CalibrationRecord&gt;,
    pub capacity: usize,
}

impl CalibrationWriter {
    pub fn try_new(alpha: f32, capacity: usize) -&gt; Result&lt;Self, &amp;'static str&gt;;
}
    </signature>
    <signature file="crates/context-graph-mejepa/src/heal/integrity.rs">
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChainIntegrityChecker {
    pub chain_path: PathBuf,
    pub last_verified_offset: u64,
    pub rollback_target: Option&lt;[u8; 32]&gt;,
}

impl ChainIntegrityChecker {
    pub fn try_new(chain_path: PathBuf) -&gt; Result&lt;Self, &amp;'static str&gt;;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IntegrityVerdict {
    Valid,
    LegacyRepaired { entries_repaired: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChainIntegrityReport {
    pub variant: IntegrityVerdict,
    pub entry_count: u64,
    pub replay_duration_ms: u64,
}
    </signature>
    <signature file="crates/context-graph-mejepa/src/heal/lora_refresh.rs">
use serde::{Deserialize, Serialize};

pub const DEFAULT_LORA_RANK: usize = 32;
pub const DEFAULT_LORA_EPOCHS: u32 = 10;
pub const DEFAULT_LORA_ALPHA: f32 = 16.0;
pub const DEFAULT_LORA_DROPOUT: f32 = 0.1;
pub const DEFAULT_LORA_LR: f32 = 1e-4;
pub const PLASTICITY_REGULATE_FLOOR: f32 = 0.4;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoraRefresher {
    pub lora_handle_sha: [u8; 32],
    pub rank: usize, pub epochs: u32, pub alpha: f32,
    pub dropout: f32, pub lr: f32,
}

impl LoraRefresher {
    pub fn try_new(lora_handle_sha: [u8; 32], rank: usize, epochs: u32,
                   alpha: f32, dropout: f32, lr: f32)
        -&gt; Result&lt;Self, &amp;'static str&gt;;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoraRefreshReport {
    pub embedder_id: u32,
    pub pre_base_sha: [u8; 32], pub post_base_sha: [u8; 32],
    pub pre_lora_sha: [u8; 32], pub post_lora_sha: [u8; 32],
    pub pre_plasticity: f32, pub post_plasticity: f32,
    pub epochs_completed: u32,
    pub fresh_corpus_slice_sha: [u8; 32],
}

impl LoraRefreshReport {
    pub fn try_new(/* args */) -&gt; Result&lt;Self, &amp;'static str&gt;;
}
    </signature>
    <signature file="crates/context-graph-mejepa/src/heal/distill.rs">
use serde::{Deserialize, Serialize};

pub const DEFAULT_DISTILL_EMA_TAU: f32 = 0.996;
pub const DEFAULT_DISTILL_INFO_NCE_TEMPERATURE: f32 = 0.07;
pub const DEFAULT_SIGNAL_CLARITY_THRESHOLD: f32 = 0.5;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OnlineDistiller {
    pub distill_handle_sha: [u8; 32],
    pub ema_tau: f32,
    pub quality_gate: bool,
    pub signal_clarity_threshold: f32,
    pub info_nce_temperature: f32,
}

impl OnlineDistiller {
    pub fn try_new(distill_handle_sha: [u8; 32], ema_tau: f32, quality_gate: bool,
                   signal_clarity_threshold: f32, info_nce_temperature: f32)
        -&gt; Result&lt;Self, &amp;'static str&gt;;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DistillStep {
    Skipped { signal_clarity: f32 },
    Applied {
        teacher_score: f32, student_score: f32,
        info_nce_loss: f32, ema_norm_pre: f32, ema_norm_post: f32,
    },
}
    </signature>
  </signatures>
  <constraints>
    - Each file &lt; 500 lines
    - HealError has exactly 12 variants matching data_models block
    - `.code()` covers all 12 variants exhaustively (compiler-checked match)
    - `BR-HEAL-06` enforced: LoraRefreshReport::try_new asserts `pre_base_sha == post_base_sha`
    - HealReport::try_new asserts `(mode_c_weights.0 + mode_c_weights.1 - 1.0).abs() &lt; 1e-3`
    - All numeric defaults exact: ALPHA=0.10, EMA_TAU=0.996, RANK=32, EPOCHS=10, ALPHA_LORA=16.0, DROPOUT=0.1, LR=1e-4, INFO_NCE_TEMP=0.07, SIGNAL_CLARITY=0.5, PLASTICITY_FLOOR=0.4
    - StatusChange serializes as snake_case ("active", "retraining", "degraded", "paused")
    - PromotionLockState::default() returns unlocked state (held_by=None, acquired_at=-1)
    - HealStatus::default() returns last_promotion_at=-1, last_calibration_at=-1, last_integrity_check_at=-1
    - All public types `Debug + Clone + Serialize + Deserialize + PartialEq`
    - File begin comments tag the deferred method-bodies (e.g., `// CalibrationWriter struct shape; sliding-window τ + σ² recompute body in TASK-HEAL-012`)
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa` zero warnings
    - `cargo test -p context-graph-mejepa heal::{errors,status,calibration,integrity,lora_refresh,distill}` shows ≥ 16 passing tests across the six files
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - HealError::code() returns the exact SCREAMING_SNAKE strings listed under failure_taxonomy
    - LoraRefreshReport::try_new rejects pre_base_sha != post_base_sha with the BR-HEAL-06 message
    - HealReport::try_new rejects mode_c_weights == (0.4, 0.4) (sum != 1)
  </verification>
</definition_of_done>

<pseudo_code>
impl HealError {
    pub fn code(&self) -> &'static str {
        match self {
            HealError::BatchNan { .. } => "MEJEPA_OBSERVE_BATCH_NAN",
            HealError::DriftDetected { .. } => "MEJEPA_OBSERVE_DRIFT_DETECTED",
            HealError::IntegrityViolation { .. } => "MEJEPA_OBSERVE_INTEGRITY_VIOLATION",
            HealError::HoldoutRegression { .. } => "MEJEPA_RETRAIN_HOLDOUT_REGRESSION",
            HealError::QuotaExceeded { .. } => "MEJEPA_RETRAIN_QUOTA_EXCEEDED",
            HealError::FisherRankDeficient { .. } => "MEJEPA_HEAL_FISHER_RANK_DEFICIENT",
            HealError::LoraRefreshFail { .. } => "MEJEPA_HEAL_LORA_REFRESH_FAIL",
            HealError::PromotionDeadlock { .. } => "MEJEPA_HEAL_PROMOTION_DEADLOCK",
            HealError::ConformalInsufficientSamples { .. } => "MEJEPA_CALIBRATION_INSUFFICIENT_SAMPLES",
            HealError::CriticalBug { .. } => "MEJEPA_HEAL_CRITICAL_BUG",
            HealError::RollbackTargetGone { .. } => "MEJEPA_HEAL_ROLLBACK_TARGET_GONE",
            HealError::FeatureDisabled => "MEJEPA_HEAL_FEATURE_DISABLED",
        }
    }
}

impl LoraRefreshReport {
    pub fn try_new(embedder_id, pre_base_sha, post_base_sha, ...) -> Result<Self, &'static str> {
        if pre_base_sha != post_base_sha {
            return Err("BR-HEAL-06 violated: frozen base embedder weights changed during LoRA refresh");
        }
        if pre_lora_sha == post_lora_sha {
            return Err("LoRA adapter unchanged after refresh; expected mutation");
        }
        if !pre_plasticity.is_finite() || !post_plasticity.is_finite() {
            return Err("non-finite plasticity");
        }
        Ok(Self { /* ... */ })
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/heal/errors.rs">HealError + IntegrityViolationKind + CriticalBugKind + .code() + Display + std::error::Error + tests</file>
  <file path="crates/context-graph-mejepa/src/heal/status.rs">StatusChange + HealStatus + PromotionLockState + HealReport + tests</file>
  <file path="crates/context-graph-mejepa/src/heal/calibration.rs">CalibrationExample + CalibrationRecord + CalibrationWriter + DEFAULT_* + tests</file>
  <file path="crates/context-graph-mejepa/src/heal/integrity.rs">ChainIntegrityChecker + ChainIntegrityReport + IntegrityVerdict + tests</file>
  <file path="crates/context-graph-mejepa/src/heal/lora_refresh.rs">LoraRefresher + LoraRefreshReport + DEFAULT_* + PLASTICITY_REGULATE_FLOOR + tests</file>
  <file path="crates/context-graph-mejepa/src/heal/distill.rs">OnlineDistiller + DistillStep + DEFAULT_* + tests</file>
</files_to_create>

<files_to_modify>
  <!-- heal/mod.rs already declares the modules from TASK-HEAL-001 -->
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa zero warnings</criterion>
  <criterion>≥ 16 passing tests across the six new files</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>HealError has exactly 12 variants</criterion>
  <criterion>HealError::code() returns exact SCREAMING_SNAKE strings per failure_taxonomy</criterion>
  <criterion>LoraRefreshReport::try_new rejects base_sha mutation (BR-HEAL-06)</criterion>
  <criterion>HealReport::try_new rejects mode_c_weights not summing to 1</criterion>
  <criterion>StatusChange serializes "retraining"/"degraded"/"paused"/"active"</criterion>
  <criterion>All DEFAULT_* constants exact: 0.10, 1000, 0.996, 0.07, 0.5, 32, 10, 16.0, 0.1, 1e-4, 0.4</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa heal::errors heal::status heal::calibration heal::integrity heal::lora_refresh heal::distill</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```

# TASK-TRAIN-002 — TrainerError + TrainerErrorCode + LossCoefficients + TrainingConfig + extended SkipReason

```xml
<task_spec id="TASK-TRAIN-002" version="1.0">

<metadata>
  <title>Foundation enums + config: TrainerError + TrainerErrorCode (10 variants) + LossCoefficients (13 default coefficients) + TrainingConfig (full TOML-loadable) + SkipReason extension (8 variants)</title>
  <status>blocked</status>
  <layer>foundation</layer>
  <sequence>2</sequence>
  <implements>
    <item ref="REQ-TRAIN-01..12">all training error codes + config knobs</item>
    <item ref="entity:TrainerError">data_models TrainerError</item>
    <item ref="entity:TrainerErrorCode">data_models TrainerErrorCode (10 variants)</item>
    <item ref="entity:LossCoefficients">data_models LossCoefficients</item>
    <item ref="entity:TrainingConfig">data_models TrainingConfig</item>
    <item ref="entity:SkipReason">extends UTML-002 SkipReason with GradExplode + VramOom (now 8 variants)</item>
  </implements>
  <depends_on>
    <task_ref>../utml-learning-formula/TASK-UTML-001</task_ref>
    <task_ref>../utml-learning-formula/TASK-UTML-002</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>3 created, 2 modified</estimated_files>
</metadata>

<context>
This task ships every error variant the trainer can emit, every default coefficient for the 13-term multi-objective loss, the full TOML-loadable training config, and extends UTML-002's SkipReason enum with two trainer-only variants (GradExplode, VramOom). All 10 error codes appear in `<failure_taxonomy>` and SPEC-TRAINING.error_states.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (failure_taxonomy)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-TRAINING.md (data_requirements TrainingConfig + LossCoefficients; error_states ERR-TRAIN-01..07)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-TRAINING.md (data_models TrainerErrorCode 10 variants; SkipReason 8 variants; MutationCategory 8; Lang 11)</file>
  <file purpose="prior_tasks">../utml-learning-formula/TASK-UTML-001.md (UtmlError pattern); ../utml-learning-formula/TASK-UTML-002.md (SkipReason with 6 variants)</file>
</input_context_files>

<prerequisites>
  <check>UTML-001 + UTML-002 complete</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `src/error.rs` with `TrainerError` struct + `TrainerErrorCode` enum (10 variants per TECH-TRAINING data_models): MejepaTrainLossNan, MejepaTrainGradExplode, MejepaTrainVramOom, MejepaTrainCheckpointCorrupt, MejepaTrainCertChainBroken, MejepaTrainHoldoutRegression, MejepaInstrumentGradientLeak, MejepaTrainDodFailed, MejepaTrainConfigInvalid, MejepaTrainEmbedderDigestMismatch
    - Each variant maps to its SCREAMING_SNAKE code via `as_screaming_snake() -&gt; &amp;'static str`
    - Display + std::error::Error impls (Source-of-truth file paths in errors start with `file:` per `<file_organization>`)
    - From&lt;UtmlError&gt; conversion: any UtmlError reaching the trainer becomes TrainerError preserving the original .code()
    - Create `src/config.rs` with `TrainingConfig` struct (all 25 fields per TECH-TRAINING data_models) and `LossCoefficients` (13 fields with defaults: λ_var=25, λ_cov=1, λ_inv=25, α_warn=0.5, α_runtime=0.3, α_rss=0.2, α_trace=0.5, α_hunk=1.0, α_adjacent=0.5, α_reasoning=0.3, α_overrides=5.0, α_counterfactual=0.2, δ=2.0)
    - Default impl for both; Default::default() returns the canonical defaults
    - TOML loader: `pub fn load_from_toml(path: &amp;Path) -&gt; Result&lt;TrainingConfig, TrainerError&gt;` using toml crate; CLI args override TOML per CLI design
    - Validate ranges: lr &gt; 0, weight_decay &gt;= 0, batch_size &gt; 0, epochs &gt; 0, lora_dropout ∈ [0, 0.5], cross_task_transfer_probability ∈ [0, 1], cross_task_cosine_threshold ∈ [0, 1], all interval_steps &gt; 0; emit MejepaTrainConfigInvalid on any violation
    - Modify `src/learning_signal/skip_reason.rs` (UTML-002) to add GradExplode + VramOom — 8 variants total
    - Verify the SkipReason variants match TECH-TRAINING data_models.SkipReason exactly
    - Add `toml = "0.8"` and `clap = "4"` to Cargo.toml (for TASK-TRAIN-013 CLI)
  </in_scope>
  <out_of_scope>
    - Trainer constructor (TASK-TRAIN-012)
    - CLI binary (TASK-TRAIN-013)
    - Full LoRA wiring (TASK-TRAIN-012)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-train/src/error.rs">
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::error_utml::UtmlError; // re-export for clarity

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrainerErrorCode {
    MejepaTrainLossNan,
    MejepaTrainGradExplode,
    MejepaTrainVramOom,
    MejepaTrainCheckpointCorrupt,
    MejepaTrainCertChainBroken,
    MejepaTrainHoldoutRegression,
    MejepaInstrumentGradientLeak,
    MejepaTrainDodFailed,
    MejepaTrainConfigInvalid,
    MejepaTrainEmbedderDigestMismatch,
}

impl TrainerErrorCode {
    pub fn as_screaming_snake(&amp;self) -&gt; &amp;'static str;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainerError {
    pub code: TrainerErrorCode,
    pub message: String,
    pub step: Option&lt;u64&gt;,
    pub context: Value,
}

impl TrainerError {
    pub fn new(code: TrainerErrorCode, message: impl Into&lt;String&gt;) -&gt; Self;
    pub fn with_step(self, step: u64) -&gt; Self;
    pub fn with_context(self, ctx: Value) -&gt; Self;
    pub fn code(&amp;self) -&gt; &amp;'static str;
}
impl std::fmt::Display for TrainerError {}
impl std::error::Error for TrainerError {}
impl From&lt;crate::UtmlError&gt; for TrainerError { /* maps Utml codes to Train codes when applicable */ }
    </signature>
    <signature file="crates/context-graph-mejepa-train/src/config.rs">
use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::error::{TrainerError, TrainerErrorCode};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LossCoefficients {
    pub lambda_var: f32,        // default 25.0
    pub lambda_cov: f32,        // default 1.0
    pub lambda_inv: f32,        // default 25.0
    pub alpha_warn: f32,        // default 0.5
    pub alpha_runtime: f32,     // default 0.3
    pub alpha_rss: f32,         // default 0.2
    pub alpha_trace: f32,       // default 0.5
    pub alpha_hunk: f32,        // default 1.0
    pub alpha_adjacent: f32,    // default 0.5
    pub alpha_reasoning: f32,   // default 0.3
    pub alpha_overrides: f32,   // default 5.0
    pub alpha_counterfactual: f32, // default 0.2
    pub delta: f32,             // default 2.0; 0.0 fallback when Phase 4b not shipped
}

impl Default for LossCoefficients { /* defaults per REQ-TRAIN-02 */ }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingConfig {
    pub lr: f64,                                 // 5e-4
    pub weight_decay: f64,                       // 0.04
    pub batch_size: usize,                       // 64
    pub epochs: u32,                             // 10
    pub warmup_steps: u32,                       // 500
    pub max_grad_norm: f32,                      // 1.0
    pub mixed_precision: bool,                   // true
    pub lora_rank: usize,                        // 32
    pub lora_alpha: usize,                       // 16
    pub lora_dropout: f32,                       // 0.05
    pub full_finetune: bool,                     // false
    pub loss_coefficients: LossCoefficients,
    pub checkpoint_interval_steps: u64,          // 2000
    pub holdout_eval_interval_steps: u64,        // 1000
    pub counterfactual_interval_steps: u64,      // 5
    pub counterfactual_warmup_steps: u64,        // 1000
    pub distillation_interval_steps: u64,        // 1000
    pub cross_task_transfer_probability: f32,    // 0.3
    pub cross_task_cosine_threshold: f32,        // 0.7
    pub sampling_drop_threshold: f32,            // 0.05
    pub sampling_force_threshold: f32,           // 1.0
    pub sampling_age_decay: f32,                 // 0.995
    pub holdout_promotion_threshold: f32,        // 0.005
    pub holdout_regression_threshold: f32,       // 0.02
    pub phase3_dod_min_agreement: f32,           // 0.75
    pub random_seed: u64,                        // required
}

impl Default for TrainingConfig { /* canonical defaults */ }

impl TrainingConfig {
    pub fn validate(&amp;self) -&gt; Result&lt;(), TrainerError&gt;;
    pub fn load_from_toml(path: &amp;Path) -&gt; Result&lt;Self, TrainerError&gt;;
}
    </signature>
  </signatures>
  <constraints>
    - All 10 TrainerErrorCode variants map to SCREAMING_SNAKE strings exactly per `<failure_taxonomy>`
    - From&lt;UtmlError&gt; for TrainerError preserves the Utml code unchanged when no trainer-side mapping fits (re-uses .code())
    - LossCoefficients::default() returns exactly the values per REQ-TRAIN-02
    - TrainingConfig::default() returns canonical defaults; random_seed defaults to 0 but validate() requires explicit user override (warn if 0)
    - validate() emits MejepaTrainConfigInvalid with structured context on any range violation
    - load_from_toml: parse → deserialize → validate; chained errors preserve TrainerError code
    - File &lt; 500 lines per file
    - Modify UTML-002's SkipReason: add GradExplode + VramOom — 8 variants total per TECH-TRAINING data_models.SkipReason
  </constraints>
  <verification>
    - cargo test shows TrainingConfig::default().validate() == Ok
    - Out-of-range value (e.g., lr=-1) → MejepaTrainConfigInvalid
    - TOML round-trip test: serialize default → toml::to_string → toml::from_str → equals original
    - From&lt;UtmlError&gt; conversion preserves error code text
  </verification>
</definition_of_done>

<pseudo_code>
impl TrainingConfig {
    pub fn validate(&amp;self) -&gt; Result&lt;(), TrainerError&gt; {
        let bad = |field, why| Err(TrainerError::new(MejepaTrainConfigInvalid, format!("{field}: {why}")));
        if self.lr &lt;= 0.0 { return bad("lr", "must be positive"); }
        if self.weight_decay &lt; 0.0 { return bad("weight_decay", "must be non-negative"); }
        if self.batch_size == 0 { return bad("batch_size", "must be positive"); }
        if self.epochs == 0 { return bad("epochs", "must be positive"); }
        if !(0.0..=0.5).contains(&amp;self.lora_dropout) { return bad("lora_dropout", "must be in [0, 0.5]"); }
        if !(0.0..=1.0).contains(&amp;self.cross_task_transfer_probability) { return bad("...", ...); }
        // ... all 25 field validations
        Ok(())
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-train/src/error.rs">TrainerError + TrainerErrorCode + From&lt;UtmlError&gt;</file>
  <file path="crates/context-graph-mejepa-train/src/config.rs">TrainingConfig + LossCoefficients + Default + validate + load_from_toml</file>
  <file path="crates/context-graph-mejepa-train/src/error_utml.rs">re-export crate::UtmlError for clarity</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/skip_reason.rs">add GradExplode + VramOom variants (8 total)</file>
  <file path="crates/context-graph-mejepa-train/src/lib.rs">pub mod error; pub mod config; re-export TrainerError + TrainingConfig + LossCoefficients</file>
  <file path="crates/context-graph-mejepa-train/Cargo.toml">add toml 0.8, clap 4 deps</file>
</files_to_modify>

<validation_criteria>
  <criterion>10 TrainerErrorCode variants map correctly to SCREAMING_SNAKE</criterion>
  <criterion>From&lt;UtmlError&gt; conversion preserves codes</criterion>
  <criterion>TrainingConfig::default().validate() returns Ok</criterion>
  <criterion>Range violations (lr&lt;0, batch_size=0, dropout&gt;0.5) → MejepaTrainConfigInvalid</criterion>
  <criterion>TOML round-trip stable</criterion>
  <criterion>SkipReason has 8 variants matching TECH-TRAINING</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-train</command>
  <command>cargo test -p context-graph-mejepa-train --lib config</command>
  <command>cargo test -p context-graph-mejepa-train --lib error</command>
  <command>cargo clippy -p context-graph-mejepa-train -- -D warnings</command>
</test_commands>

</task_spec>
```

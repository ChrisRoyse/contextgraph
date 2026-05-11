## 🔍 AI-Agent Implementation Mandate (read FIRST — fresh-context onboarding)

> **You are an AI coding assistant inheriting this task with no prior context. This block gives you everything you need.**

### 1. Project state pointers — read these before touching code

- **Live shipping status table:** `docs/ruvectorfindings/specs/_README.md` Domain map (one row per phase, current as of 2026-05-10).
- **Per-phase deliverable status:** `docs/ruvectorfindings/specs/implementation-plan.md` "Per-phase deliverable table".
- **Constitution (immutable rules):** `docs/ruvectorfindings/specs/constitution.md`.
- **This phase's status snapshot:** the `_index.md` in this same directory.
- **Recent activity:** `git log --oneline -30` shows the last 30 commits.
- **Architecture intent:** `docs/ruvectorfindings/{09,10,11,12,13,14}*.md` (read 09 master plan + 10 pivot first).
- **Project rules:** `CLAUDE.md` at workspace root.

### 2. ⚠ ASSUME THIS TASK IS WRONG — audit before you implement

This task body was authored 2026-05-09. Before implementing, audit it against the current codebase:

1. Do the file paths it references still exist? `find` / `ls` them.
2. Do the type signatures it specifies still match what's in tree? `grep` for the type names in `crates/`.
3. Has the crate it tells you to "create" already been created? Check `crates/`. If it exists, STOP — read what's there, reconcile against the task, and update the task itself before implementing more.
4. Does the spec it cites (e.g., `TECH-<DOMAIN>.md`) still describe the same data shapes? Re-read those specs.

If you find drift, document it in your handoff (`memory/handoff/`) and either (a) update the task to match reality before continuing, or (b) escalate to the operator if scope changed materially.

### 3. Hard rules — ZERO tolerance, fail-closed

1. **No backwards compatibility.** Schema changes wipe + recreate; never write migration shims. Synthetic data only.
2. **No mock data in tests.** Use real types, real corpus rows, real RocksDB CFs, real file SHAs. Tests must FAIL when the system is broken — never write a test that passes against a broken state.
3. **Fail closed with structured error codes** (e.g., `MEJEPA_*`, `UTML_*`). No `unwrap_or(default)` on confidence / quota / correlation / signature / chain-integrity paths. No silent fallback. Every error has `.code()` returning `SCREAMING_SNAKE_CASE`.
4. **Robust error logging.** Every fail-path emits `tracing::error!` with the structured code + actionable context (which file, which CF, which byte offset, which step). When something breaks, the log must say exactly what failed and how to fix it.
5. **No comments unless they encode non-obvious WHY.** Don't explain WHAT — well-named identifiers do that.
6. **Files under 500 lines** (split if exceeded).
7. **Provenance comment** at top of every new `.rs` file: `// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).` Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`.

### 4. Mandatory Full State Verification (FSV) — every task ships with it

Do not rely on return values alone. After the logic runs, you MUST:

1. **Define the Source of Truth.** State explicitly where the final result lives: a RocksDB column family, a `/tmp/contextgraph-*-fsv/*.json` file, a witness-chain segment, a tier-compressed panel blob, a CF watermark row, a tensor allocator's `live_tensors` map, etc.
2. **Execute & Inspect.** Run the logic, then perform a SEPARATE READ on the Source of Truth to verify the data was processed correctly. Cite the readback command (e.g., `cargo run --release -p <crate> --example <fsv_example>` or a direct CF dump).
3. **Boundary & Edge Case Audit.** Manually simulate AT LEAST 3 edge cases — empty inputs, maximum limits, invalid formats, NaN/Inf in floats, control chars in strings, byte offsets at file boundary, etc. For each, print the system state BEFORE and AFTER the action.
4. **Evidence of Success.** Provide a log showing the actual data residing in the system after execution. Cite the log path in the task's `<verification>` block.

### 5. Trigger → outcome verification (mandatory database / artifact check)

> Every triggered event exists to produce a specific intended outcome. The trigger is observable; the outcome is queryable. If your task writes anything to a persistent surface, you MUST query that surface AFTER running and confirm the expected data is there. Do not trust return values alone.

Synthetic protocol: pick an input where you KNOW the expected output. Run it. Manually check the source of truth for the expected output (e.g., open RocksDB and dump the CF, read the FSV JSON, replay the witness chain). Document the trigger → outcome chain in the FSV evidence. Examples:

- Trigger: `compute_l_step({delta_p:0, delta_k:1, delta_omega:1, delta_xi:1})` → Outcome: returns `0.0` exactly (multiplicative gate; any factor at zero zeros the product). FSV: dump training cert, confirm `l_step == 0.0`.
- Trigger: insert `ShiftEntry { shift_id: "01J0123456789ABCDEFGHIJK", ... }` → Outcome: row in `CF_MEJEPA_SHIFT_WATERMARK` with that exact key. FSV: open RocksDB, `cf.get(b"01J0123456789ABCDEFGHIJK")` returns Some.

### 6. Occam's razor

The simplest implementation that satisfies the contract is the correct one. Do NOT add features, abstractions, or fallbacks beyond the task scope. A bug fix doesn't need surrounding cleanup. Three similar lines beats a premature abstraction. If the task says "create struct X with 5 fields", create exactly that — not a builder, not a derive macro family, not a generic over a placeholder.

### 7. Subagent / verification expectations

After implementation:
- Run `cargo build --release -p <crate>` AND `cargo test -p <crate>` AND `cargo clippy -p <crate> -- -D warnings`.
- Spawn the `code-simplifier` subagent (or equivalent) on your changes; apply BLOCKER fixes.
- Write a final SMAP report at `memory/reports/agent-{N}-{role}--final-report.md`.
- Touch the corresponding row in `tasks/_traceability.md` if the REQ→TASK mapping changed.

---

## 📋 2026-05-11 STATE AUDIT (sharpened against verified codebase)

> **Codebase reality table:** see `_index.md` in this directory for the full audit table (2026-05-11). What follows is THIS task's scope-specific drift.

**Confirmed by `ls`/`grep` 2026-05-11:**
- `crates/context-graph-mejepa/src/eval/` does NOT exist; this task creates the subtree fresh.
- `crates/context-graph-mejepa/src/types.rs` SHIPPED with `Language` (11 variants), `OracleOutcome`, `TaskId`, `EmbedderId`, `ChunkId`. TASK-INFER-001 dependency satisfied.
- `crates/context-graph-mejepa/src/compiler.rs` re-exports `MutationCategory as TctMutationCategory` (line 11). The Phase 8 `MutationCategory` (8 variants per doc 09 §3.1) is the canonical eval type for this domain — verify it matches or replaces the TCT re-export rather than colliding.
- **Type collision risk:** `crates/context-graph-mejepa-train/src/eval/mod.rs:14` already defines a train-internal `MutationCategory` AND a train-internal `Lang` (line 40, NOT the canonical 11-variant Language). Both are train-internal and SHOULD NOT be propagated. Add `pub use crate::types::Language;` (or fully qualify) — DO NOT redeclare a competing `Lang` enum in `eval/types.rs`.
- Train-internal holdout-eval skeleton at `crates/context-graph-mejepa-train/src/eval/{holdout,slicing,regression,epoch_witness,mod}.rs` is SHIPPED but train-internal — Phase 8 lands the operator-facing surface in `mejepa::eval` instead.

**Decided scope for this task (unchanged from author intent + tightened):**
1. Create `crates/context-graph-mejepa/src/eval/{mod.rs, types.rs, report.rs}` with all 16 public enums/structs documented in `<scope>` below.
2. Use BTreeMap (not HashMap) on the public surface for determinism per TECH-EVALUATION implementation_notes.
3. `EvalReport::try_new` validates finite + clamped at construction; rejects per_category != 8, per_language != 11, open_research_questions != 8.
4. `serialize_canonical` uses `serde_json::to_vec` with sorted-key BTreeMap-backed maps; reject when len > 1 MiB.
5. `hash_for_determinism` is SHA-256 of canonical bytes with `report_date` field stripped.
6. Forward-declare `EvalError` as `&'static str` placeholder; replace in TASK-EVAL-002.

**FSV for this task:** unit tests cover (a) NaN rejection in `overall_correlation`, (b) per_category map size invariant, (c) per_language map size invariant, (d) `serialize_canonical` byte-equal across runs, (e) `hash_for_determinism` identical when only `report_date` differs, (f) `seed_initial_research_questions().len() == 8` with all `Active`. Source of Truth: typed structures + serde JSON byte-equal round trips + boundary error returns.

---

# TASK-EVAL-001 — Public evaluation data models (EvalReport, ShipGateStatus, ConformalHealthEntry, ActiveLearningSummary, StateTransferDiagnostic, RegressionCheck, OpenResearchQuestionStatus, MutationCategory, supporting enums)

```xml
<task_spec id="TASK-EVAL-001" version="1.0">

<metadata>
  <title>Define public evaluation data models in `crates/context-graph-mejepa/src/eval/types.rs` (and `src/eval/report.rs` for `EvalReport::try_new`)</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>1</sequence>
  <implements>
    <item ref="REQ-EVAL-02">EvalReport struct shape + invariants (per_category 8 entries, per_language 11 entries, per_cell up to 88 entries, ship_gate_passed, ood_calibration_health, conformal_coverage_health, gtau_pass_rate, q1_pass_rate, q2_correlation, q3_side_effect_agreement, drift_severity_history, model_promotion_history, state_transfer_diagnostic, aux_head_distillation, open_research_questions)</item>
    <item ref="REQ-EVAL-17">Determinism — byte-equal serialize modulo report_date; `hash_for_determinism()` excludes wall-clock fields</item>
    <item ref="REQ-EVAL-18">Provenance binding — corpus_sha, model_weights_sha, calibration_version, constellation_version, code_version, holdout_split_sha</item>
    <item ref="REQ-EVAL-19">Open-research-question tracking (8 entries Q1..Q8 with mitigation_state)</item>
    <item ref="component:EvalReport">try_new validation gate + serialize_canonical + hash_for_determinism</item>
    <item ref="component:ShipGateStatus">passed, per_category, per_language, blockers, report_date, overall_correlation</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INFER-001</task_ref><!-- Language enum, OracleOutcome, TaskId, EmbedderId, ChunkId already shipped in src/types.rs -->
    <task_ref>TASK-CORPUS-003</task_ref><!-- MutationCategory re-export source if applicable -->
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>3 created (src/eval/mod.rs, src/eval/types.rs, src/eval/report.rs)</estimated_files>
</metadata>

<context>
This task lands the public eval-domain data models that downstream tasks (004 runner, 008 ship-gate,
009 active-learning, 010 graph, 011 state-transfer, 012 Q1/Q2/Q3, 013-014 MCP, 015 weekly-report,
017 FSV) consume. The structs encode the contract between the evaluation pipeline and the operator-
facing report. `EvalReport::try_new` MUST validate finite + clamped at construction (no `unwrap_or`
on confidence-related fields per constitution `<error_handling>`); `serialize_canonical` MUST use
BTreeMap-backed sorted-key JSON for determinism (per TECH-EVALUATION implementation_notes).
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (failure_taxonomy MEJEPA_RETRAIN_HOLDOUT_REGRESSION; performance_budgets phase8_ship_gate; provenance rule)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-EVALUATION.md (REQ-EVAL-02, REQ-EVAL-17, REQ-EVAL-18, REQ-EVAL-19; EC-EVAL-02 NaN; EC-EVAL-06 missing language)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-EVALUATION.md (data_models block: EvalReport, ShipGateThresholds, ConformalHealthEntry, ActiveLearningSummary, DriftSeverityEntry, ModelPromotionEntry, StateTransferDiagnostic, AuxHeadDistillationSummary, OpenResearchQuestionStatus, ShipGateStatus, MutationCategory, ResearchQuestionId, MitigationState, DriftSeverity, ModelMode, SplitMode, RegressionCheck, CellDelta)</file>
  <file purpose="cross_domain_types">crates/context-graph-mejepa/src/types.rs (Language, OracleOutcome, TaskId, EmbedderId, ChunkId — shipped by TASK-INFER-001)</file>
  <file purpose="naming_conventions">docs/ruvectorfindings/specs/constitution.md#coding_standards (snake_case files, PascalCase types, SCREAMING_SNAKE error codes)</file>
</input_context_files>

<prerequisites>
  <check>`crates/context-graph-mejepa` exists in workspace (TASK-INFER-001 shipped Cargo.toml + lib.rs)</check>
  <check>`crates/context-graph-mejepa/src/types.rs` exposes `Language` (11 variants), `OracleOutcome`, `TaskId`, `EmbedderId`, `ChunkId`</check>
  <check>`chrono`, `ulid`, `serde`, `serde_json`, `sha2`, `bincode` available as workspace deps (or addable to mejepa Cargo.toml)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/eval/mod.rs` declaring `pub mod types;` + `pub mod report;` (other modules added in later tasks)
    - Create `crates/context-graph-mejepa/src/eval/types.rs` with the public eval enums and structs:
      - `MutationCategory` enum (8 variants: KnownGood, SubtleFlip, OffByOne, SwapVariable, DeleteTestCall, WrongFile, OverEngineer, CompileError)
      - `ResearchQuestionId` enum (Q1..Q8)
      - `MitigationState` enum (Active, Resolved, Deferred)
      - `DriftSeverity` enum (Healthy, Soft, Hard, Critical)
      - `ModelMode` enum (A, B, C)
      - `SplitMode` enum (RepoAtomic [default], InstanceAtomic)
      - `LabelMethod` enum (Manual, DockerRerun, AdjacentTestRerun) — referenced by ActiveLearningSummary; full ActiveLearningLabel struct lands in TASK-EVAL-002
      - `ConformalHealthEntry` struct (stated, empirical, delta_pct, num_samples, passes_band)
      - `ActiveLearningSummary` struct (queue_depth, labels_added_this_week, corpus_growth_pct, below_target, ood_escalations_pending, label_method_distribution: BTreeMap<LabelMethod, usize>)
      - `DriftSeverityEntry` struct (week, severity)
      - `ModelPromotionEntry` struct (date, mode_winner, weights_sha, calibration_version, overall_correlation, regression_triggered, regressed_cells)
      - `StateTransferDiagnostic` struct (d_train_deploy, lambda, t_value, m_t_estimate, performance_deploy_predicted, attribution, wasserstein_samples_used)
      - `AuxHeadDistillationSummary` struct (last_distillation_step, distillation_loss, aux_heads_in_consensus, aux_heads_diverged)
      - `OpenResearchQuestionStatus` struct (question_id, question_text, mitigation_state, evidence_pointers, last_updated)
      - `CellDelta` struct (cell, prior, current, delta_pct, is_regression)
      - `RegressionCheck` enum (NoPriorReport, Compared{prior_date, deltas}, CorpusShaChanged{prior_corpus_sha, current_corpus_sha})
      - `ShipGateThresholds` struct (overall_min, per_category_min, per_language_min, per_cell_min, ood_auc_min, conformal_band_pct, q1_min, q3_min, weekly_regression_pct) with `Default` returning constitution-pinned defaults (0.95, 0.95, 0.95, 0.90, 0.85, 2.0, 0.99, 0.90, 2.0)
      - `ShipGateStatus` struct (passed, per_category, per_language, blockers, report_date, overall_correlation)
      - `seed_initial_research_questions() -> Vec<OpenResearchQuestionStatus>` — canonical 8 entries Q1..Q8 with text from doc 09 §8, mitigation_state=Active, evidence_pointers=[]
    - Create `crates/context-graph-mejepa/src/eval/report.rs` with:
      - `EvalReport` struct carrying every field per TECH-EVALUATION data_models (≥ 28 fields)
      - `EvalReportBuilder` mirror struct (all fields `pub`)
      - `EvalReport::try_new(builder: EvalReportBuilder) -> Result<Self, EvalError>` (error type from TASK-EVAL-002 — for this task forward-declare with `Result<Self, &'static str>` placeholder; replace with structured error in TASK-EVAL-002 ONLY without changing call sites — use `pub(crate) fn try_new_strict` accepting placeholder error and `pub fn try_new` re-exporting in TASK-EVAL-002)
      - `EvalReport::serialize_canonical(&self) -> Result<Vec<u8>, &'static str>` using `serde_json::to_vec` with BTreeMap-backed maps for determinism; reject when len > 1 MiB
      - `EvalReport::hash_for_determinism(&self) -> [u8; 32]` (SHA-256 of canonical bytes with `report_date` field stripped)
    - Add `serde::{Serialize, Deserialize}` derives on all public types with `#[serde(rename_all = "snake_case")]` on enums and `#[serde(deny_unknown_fields)]` on structs (per audit-9 lesson)
    - Convert HashMap fields to BTreeMap on the public surface (fixes determinism per TECH-EVALUATION implementation_notes/determinism)
    - Add unit tests in `src/eval/types.rs` and `src/eval/report.rs` `#[cfg(test)] mod tests` for:
      - `EvalReport::try_new` rejects NaN in `overall_correlation`
      - `EvalReport::try_new` rejects per_category map size != 8
      - `EvalReport::try_new` rejects per_language map size != 11
      - `EvalReport::try_new` rejects open_research_questions.len() != 8
      - `EvalReport::serialize_canonical` is deterministic across runs (sorted-key BTreeMap-backed)
      - `EvalReport::hash_for_determinism` returns identical SHA-256 when only `report_date` differs
      - `ShipGateThresholds::default()` matches constitution-pinned values
      - `MutationCategory` exhaustively constructible (8 variants)
      - `ResearchQuestionId` exhaustively constructible (Q1..Q8)
      - `seed_initial_research_questions().len() == 8` with all `mitigation_state == Active`
      - `RegressionCheck::NoPriorReport` serde round-trip
      - `ConformalHealthEntry::passes_band` correctly true on |delta_pct| ≤ 2.0
  </in_scope>
  <out_of_scope>
    - `EvalError`, `QueueError`, `GraphError` (TASK-EVAL-002)
    - `ActiveLearningQueueState`, `ActiveLearningQueueEntry`, `ActiveLearningLabel`, `PatchGraph`, `PatchGraphEdge` (TASK-EVAL-002)
    - `EvalConfig` (TASK-EVAL-002)
    - `EvalRunner` impl (TASK-EVAL-004 onward)
    - `check_ship_gate` body (TASK-EVAL-008)
    - RocksDB CFs (TASK-EVAL-003)
    - MCP tool schemas (TASK-EVAL-013, TASK-EVAL-014)
    - CLI subcommands (TASK-EVAL-016)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/eval/mod.rs">
pub mod types;
pub mod report;

pub use report::{EvalReport, EvalReportBuilder};
pub use types::{
    ActiveLearningSummary, AuxHeadDistillationSummary, CellDelta, ConformalHealthEntry,
    DriftSeverity, DriftSeverityEntry, LabelMethod, MitigationState, ModelMode,
    ModelPromotionEntry, MutationCategory, OpenResearchQuestionStatus, RegressionCheck,
    ResearchQuestionId, ShipGateStatus, ShipGateThresholds, SplitMode, StateTransferDiagnostic,
    seed_initial_research_questions,
};
    </signature>
    <signature file="crates/context-graph-mejepa/src/eval/types.rs">
use std::collections::BTreeMap;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use crate::types::{EmbedderId, Language, OracleOutcome, TaskId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationCategory {
    KnownGood, SubtleFlip, OffByOne, SwapVariable,
    DeleteTestCall, WrongFile, OverEngineer, CompileError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchQuestionId { Q1, Q2, Q3, Q4, Q5, Q6, Q7, Q8 }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MitigationState { Active, Resolved, Deferred }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftSeverity { Healthy, Soft, Hard, Critical }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelMode { A, B, C }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitMode { RepoAtomic, InstanceAtomic }

impl Default for SplitMode { fn default() -> Self { SplitMode::RepoAtomic } }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelMethod { Manual, DockerRerun, AdjacentTestRerun }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ConformalHealthEntry {
    pub stated: f32,
    pub empirical: f32,
    pub delta_pct: f32,
    pub num_samples: usize,
    pub passes_band: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ActiveLearningSummary {
    pub queue_depth: usize,
    pub labels_added_this_week: usize,
    pub corpus_growth_pct: f32,
    pub below_target: bool,
    pub ood_escalations_pending: usize,
    pub label_method_distribution: BTreeMap&lt;LabelMethod, usize&gt;,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DriftSeverityEntry { pub week: NaiveDate, pub severity: DriftSeverity }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ModelPromotionEntry {
    pub date: NaiveDate,
    pub mode_winner: ModelMode,
    pub weights_sha: [u8; 32],
    pub calibration_version: String,
    pub overall_correlation: f32,
    pub regression_triggered: bool,
    pub regressed_cells: Vec&lt;(MutationCategory, Language)&gt;,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StateTransferDiagnostic {
    pub d_train_deploy: f32,
    pub lambda: f32,
    pub t_value: f32,
    pub m_t_estimate: f32,
    pub performance_deploy_predicted: f32,
    pub attribution: String,
    pub wasserstein_samples_used: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AuxHeadDistillationSummary {
    pub last_distillation_step: u64,
    pub distillation_loss: f32,
    pub aux_heads_in_consensus: Vec&lt;String&gt;,
    pub aux_heads_diverged: Vec&lt;String&gt;,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OpenResearchQuestionStatus {
    pub question_id: ResearchQuestionId,
    pub question_text: String,
    pub mitigation_state: MitigationState,
    pub evidence_pointers: Vec&lt;String&gt;,
    pub last_updated: NaiveDate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CellDelta {
    pub cell: (MutationCategory, Language),
    pub prior: f32,
    pub current: f32,
    pub delta_pct: f32,
    pub is_regression: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RegressionCheck {
    NoPriorReport,
    Compared { prior_date: NaiveDate, deltas: Vec&lt;CellDelta&gt; },
    CorpusShaChanged { prior_corpus_sha: [u8; 32], current_corpus_sha: [u8; 32] },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ShipGateThresholds {
    pub overall_min: f32,
    pub per_category_min: f32,
    pub per_language_min: f32,
    pub per_cell_min: f32,
    pub ood_auc_min: f32,
    pub conformal_band_pct: f32,
    pub q1_min: f32,
    pub q3_min: f32,
    pub weekly_regression_pct: f32,
}

impl Default for ShipGateThresholds {
    fn default() -> Self {
        Self {
            overall_min: 0.95, per_category_min: 0.95, per_language_min: 0.95,
            per_cell_min: 0.90, ood_auc_min: 0.85, conformal_band_pct: 2.0,
            q1_min: 0.99, q3_min: 0.90, weekly_regression_pct: 2.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ShipGateStatus {
    pub passed: bool,
    pub per_category: BTreeMap&lt;MutationCategory, f32&gt;,
    pub per_language: BTreeMap&lt;Language, Option&lt;f32&gt;&gt;,
    pub blockers: Vec&lt;String&gt;,
    pub report_date: NaiveDate,
    pub overall_correlation: f32,
}

pub fn seed_initial_research_questions() -&gt; Vec&lt;OpenResearchQuestionStatus&gt;;
    </signature>
    <signature file="crates/context-graph-mejepa/src/eval/report.rs">
use std::collections::BTreeMap;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use crate::types::{Language, OracleOutcome};
use super::types::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EvalReport {
    pub report_date: NaiveDate,
    pub corpus_sha: [u8; 32],
    pub model_weights_sha: [u8; 32],
    pub calibration_version: String,
    pub constellation_version: String,
    pub code_version: String,
    pub holdout_split_sha: [u8; 32],
    pub rolling_window_size: usize,
    pub overall_correlation: f32,
    pub per_category: BTreeMap&lt;MutationCategory, f32&gt;,
    pub per_language: BTreeMap&lt;Language, Option&lt;f32&gt;&gt;,
    pub per_cell: Option&lt;BTreeMap&lt;(MutationCategory, Language), Option&lt;f32&gt;&gt;&gt;,
    pub ship_gate_passed: bool,
    pub ship_gate_failures: Vec&lt;String&gt;,
    pub ship_gate_thresholds: ShipGateThresholds,
    pub ood_calibration_health: BTreeMap&lt;Language, f32&gt;,
    pub ood_calibration_health_history: Vec&lt;(NaiveDate, BTreeMap&lt;Language, f32&gt;)&gt;,
    pub conformal_coverage_health: BTreeMap&lt;Language, ConformalHealthEntry&gt;,
    pub gtau_pass_rate: BTreeMap&lt;Language, f32&gt;,
    pub q1_pass_rate: f32,
    pub q2_correlation: f32,
    pub q3_side_effect_agreement: Option&lt;f32&gt;,
    pub active_learning: ActiveLearningSummary,
    pub drift_severity_history: Vec&lt;DriftSeverityEntry&gt;,
    pub model_promotion_history: Vec&lt;ModelPromotionEntry&gt;,
    pub state_transfer_diagnostic: Option&lt;StateTransferDiagnostic&gt;,
    pub aux_head_distillation: Option&lt;AuxHeadDistillationSummary&gt;,
    pub open_research_questions: Vec&lt;OpenResearchQuestionStatus&gt;,
    pub per_cell_warnings: Vec&lt;String&gt;,
    pub shrinkage_applied: Vec&lt;String&gt;,
    pub split_mode: SplitMode,
    pub latency_budget_exceeded: bool,
    pub wall_clock_seconds: f64,
    pub unsupported_languages: Vec&lt;Language&gt;,
    pub regression_check: RegressionCheck,
}

#[derive(Debug, Clone)]
pub struct EvalReportBuilder { /* mirror of all EvalReport fields, all pub */ }

impl EvalReport {
    /// Validates: every f32 finite; correlations ∈ [-1, 1]; q1/q3 ∈ [0, 1];
    /// per_category has exactly 8 keys; per_language has exactly 11 keys;
    /// open_research_questions.len() == 8; provenance fields non-empty.
    /// Placeholder error type until TASK-EVAL-002 lands EvalError.
    pub fn try_new(b: EvalReportBuilder) -&gt; Result&lt;Self, &amp;'static str&gt;;

    /// Serialize via serde_json with BTreeMap-backed maps for determinism;
    /// reject when len &gt; 1 MiB (REPORT_TOO_LARGE).
    pub fn serialize_canonical(&amp;self) -&gt; Result&lt;Vec&lt;u8&gt;, &amp;'static str&gt;;

    /// SHA-256 of canonical bytes with `report_date` field stripped.
    pub fn hash_for_determinism(&amp;self) -&gt; [u8; 32];
}
    </signature>
  </signatures>
  <constraints>
    - All public structs derive `Debug`, `Clone`, `Serialize`, `Deserialize`, `PartialEq`. Enums also derive `Eq`, `Hash`, `Copy` where natural.
    - `#[serde(rename_all = "snake_case")]` on every enum
    - `#[serde(deny_unknown_fields)]` on every struct that traverses MCP / RocksDB boundary (per audit-9 lesson + TECH-EVALUATION implementation_notes/determinism)
    - All HashMap fields converted to BTreeMap on the public surface for determinism (per TECH-EVALUATION implementation_notes/determinism: serde_json::to_vec with default ordering is NOT deterministic over HashMap)
    - `EvalReport::try_new` rejects:
      - NaN/Inf in any f32 field
      - `overall_correlation` outside [-1, 1]
      - any `per_category` value outside [-1, 1]
      - any `per_language[Some]` value outside [-1, 1]
      - any `per_cell[Some][Some]` value outside [-1, 1]
      - `q1_pass_rate` outside [0, 1]
      - `q3_side_effect_agreement` (when Some) outside [0, 1]
      - `per_category` map size != 8 (the 8 mutation categories)
      - `per_language` map size != 11 (the 11 supported languages)
      - `open_research_questions.len() != 8`
      - empty `corpus_sha`, `model_weights_sha`, `calibration_version`, `constellation_version`, `code_version`, or `holdout_split_sha`
    - `serialize_canonical` rejects when output > 1 MiB (per NFR-EVAL-09)
    - `hash_for_determinism` MUST exclude `report_date` AND every wall-clock-dependent field (`wall_clock_seconds`, `built_at`-style timestamps in nested structs); the test verifies SHA-256 equality across runs that differ only in `report_date`
    - `seed_initial_research_questions` returns exactly 8 entries Q1..Q8 with canonical text from doc 09 §8 (operator questions: JEPA cross-repo generalization; 5,120-d expressiveness; per-language vs unified; mutation diversity; novel APIs; λ_ewc; non-SWE-bench code; self-disagreement); all `mitigation_state == Active`; `evidence_pointers == []`; `last_updated == today`
    - `ShipGateThresholds::default()` returns the constitution-pinned values verbatim (cannot diverge without explicit `<spec_version>` bump)
    - No `unwrap_or` on any correlation/confidence-related field
    - All field names match TECH-EVALUATION data_models block byte-for-byte (consumer contract)
    - Rust 2021 edition; Rust 1.84+ per constitution
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa` succeeds with zero warnings
    - `cargo test -p context-graph-mejepa eval::types` shows ≥ 6 passing tests
    - `cargo test -p context-graph-mejepa eval::report` shows ≥ 6 passing tests
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - Test `try_new_rejects_nan_overall_correlation` passes
    - Test `try_new_rejects_per_category_size_not_8` passes
    - Test `try_new_rejects_per_language_size_not_11` passes
    - Test `try_new_rejects_open_research_questions_size_not_8` passes
    - Test `serialize_canonical_is_deterministic` passes (3 serialize calls produce byte-equal output)
    - Test `hash_for_determinism_excludes_report_date` passes (two reports differing only in report_date hash equal)
    - Test `ship_gate_thresholds_default_constitution_pinned` passes
    - Test `seed_initial_research_questions_returns_8_active` passes
    - Test `mutation_category_eight_variants` passes
  </verification>
</definition_of_done>

<pseudo_code>
crates/context-graph-mejepa/src/eval/types.rs:
  // ... struct + enum definitions per signatures block ...

  pub fn seed_initial_research_questions() -> Vec<OpenResearchQuestionStatus> {
      use ResearchQuestionId::*;
      let today = chrono::Utc::now().date_naive();
      [
          (Q1, "Will JEPA cross-repository generalization hold once we expand beyond SWE-bench Lite?"),
          (Q2, "Are 5,120-d panels expressive enough as the corpus + language coverage grows?"),
          (Q3, "Per-language ME-JEPAs or one unified model across the 11-language matrix?"),
          (Q4, "Is the Phase 0 mutation corpus diverse enough to span real-world bug classes?"),
          (Q5, "Can ME-JEPA predict reliably on novel APIs not seen in training?"),
          (Q6, "What is the right λ_ewc to balance plasticity vs catastrophic forgetting?"),
          (Q7, "Will the model generalize to non-SWE-bench code (different test harnesses, different CI)?"),
          (Q8, "How much does ME-JEPA self-disagree across retraining cycles, and does it stabilize?"),
      ].into_iter().map(|(qid, text)| OpenResearchQuestionStatus {
          question_id: qid,
          question_text: text.to_string(),
          mitigation_state: MitigationState::Active,
          evidence_pointers: vec![],
          last_updated: today,
      }).collect()
  }

crates/context-graph-mejepa/src/eval/report.rs:
  impl EvalReport {
      pub fn try_new(b: EvalReportBuilder) -> Result<Self, &'static str> {
          // 1. validate every f32 finite
          if !b.overall_correlation.is_finite() { return Err("overall_correlation non-finite"); }
          if !(-1.0..=1.0).contains(&b.overall_correlation) { return Err("overall_correlation out of range"); }
          if !b.q1_pass_rate.is_finite() || !(0.0..=1.0).contains(&b.q1_pass_rate) { return Err("q1_pass_rate invalid"); }
          if let Some(q3) = b.q3_side_effect_agreement {
              if !q3.is_finite() || !(0.0..=1.0).contains(&q3) { return Err("q3 invalid"); }
          }
          // 2. validate map sizes
          if b.per_category.len() != 8 { return Err("per_category must have exactly 8 keys"); }
          if b.per_language.len() != 11 { return Err("per_language must have exactly 11 keys"); }
          if b.open_research_questions.len() != 8 { return Err("open_research_questions must have 8 entries"); }
          // 3. validate per_category/per_language/per_cell ranges
          for (_, v) in &b.per_category {
              if !v.is_finite() || !(-1.0..=1.0).contains(v) { return Err("per_category value invalid"); }
          }
          for (_, opt) in &b.per_language {
              if let Some(v) = opt {
                  if !v.is_finite() || !(-1.0..=1.0).contains(v) { return Err("per_language value invalid"); }
              }
          }
          if let Some(cell_map) = &b.per_cell {
              for (_, opt) in cell_map {
                  if let Some(v) = opt {
                      if !v.is_finite() || !(-1.0..=1.0).contains(v) { return Err("per_cell value invalid"); }
                  }
              }
          }
          // 4. validate provenance
          if b.calibration_version.is_empty() { return Err("calibration_version empty"); }
          if b.constellation_version.is_empty() { return Err("constellation_version empty"); }
          if b.code_version.is_empty() { return Err("code_version empty"); }
          // 5. construct
          Ok(Self { /* fields from builder */ })
      }

      pub fn serialize_canonical(&self) -> Result<Vec<u8>, &'static str> {
          let bytes = serde_json::to_vec(self).map_err(|_| "serialize_failed")?;
          if bytes.len() > 1024 * 1024 { return Err("REPORT_TOO_LARGE"); }
          Ok(bytes)
      }

      pub fn hash_for_determinism(&self) -> [u8; 32] {
          // strip report_date + wall_clock_seconds + any timestamp-bearing nested field
          let mut clone = self.clone();
          clone.report_date = NaiveDate::from_ymd_opt(2000, 1, 1).unwrap();
          clone.wall_clock_seconds = 0.0;
          let bytes = serde_json::to_vec(&clone).unwrap_or_default();
          use sha2::{Digest, Sha256};
          let mut hasher = Sha256::new();
          hasher.update(&bytes);
          hasher.finalize().into()
      }
  }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/eval/mod.rs">module declarations + pub use re-exports of public types</file>
  <file path="crates/context-graph-mejepa/src/eval/types.rs">all 17 public eval enums and structs + seed_initial_research_questions + tests module</file>
  <file path="crates/context-graph-mejepa/src/eval/report.rs">EvalReport + EvalReportBuilder + try_new + serialize_canonical + hash_for_determinism + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/lib.rs">add `pub mod eval;` and `pub use eval::*;` re-export of public types</file>
  <file path="crates/context-graph-mejepa/Cargo.toml">add deps `chrono = { workspace = true }`, `sha2 = { workspace = true }` if not already present</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa succeeds with zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa eval shows ≥ 12 passing tests across types + report</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>EvalReport::try_new returns Err on NaN in overall_correlation</criterion>
  <criterion>EvalReport::try_new returns Err on per_category map size != 8</criterion>
  <criterion>EvalReport::try_new returns Err on per_language map size != 11</criterion>
  <criterion>EvalReport::try_new returns Err on open_research_questions.len() != 8</criterion>
  <criterion>EvalReport::serialize_canonical produces byte-equal output across 3 invocations on identical input</criterion>
  <criterion>EvalReport::hash_for_determinism returns identical SHA-256 when only report_date differs</criterion>
  <criterion>ShipGateThresholds::default() == { 0.95, 0.95, 0.95, 0.90, 0.85, 2.0, 0.99, 0.90, 2.0 }</criterion>
  <criterion>seed_initial_research_questions().len() == 8 with all mitigation_state == Active</criterion>
  <criterion>MutationCategory enum has exactly 8 variants matching SPEC-CORPUS</criterion>
  <criterion>ResearchQuestionId enum has exactly 8 variants Q1..Q8</criterion>
  <criterion>SplitMode::default() == RepoAtomic</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa eval</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```

# TASK-UTML-002 — Enums: SkipReason + UtmlState + ClassifierVerdict + DeltaPAggregator + Intervention

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

## 📋 2026-05-11 STATE AUDIT — TASK PARTIALLY COMPLETE

> **Codebase reality table:** see `_index.md` in this directory.

**Status:** `SkipReason` shipped (used by `BatchSampler::should_drop` / `should_force_include` in `learning_signal.rs:259+`). `UtmlState` and `ClassifierVerdict` enums DO NOT exist yet — they are the data models for the pending UtmlClassifier (TASK-UTML-009).

**Decided scope for the remaining work:**
1. Add `UtmlState` enum (7 variants per UTML §VII-VIII): `OnTrack`, `OverwhelmedAtBoundary`, `BoredAtMastery`, `RegressedAfterGain`, `StuckUninformative`, `OverPlasticDriftRisk`, `RedundantInputs`. `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]`.
2. Add `ClassifierVerdict { state: UtmlState, recommendation: ClassifierRecommendation, evidence: ClassifierEvidence }` struct.
3. Add `ClassifierRecommendation` enum: `KeepTraining`, `ReduceLearningRate`, `IncreasePlasticity`, `Prune`, `RewindAndRetry`, `EscalateToOperator`.
4. Add `DeltaPAggregator` struct for per-chunk aggregation (sum + count + clamp `[0, 1]`).
5. All types `#[derive(Serialize, Deserialize)]` with `deny_unknown_fields` on structs.
6. Add to `crates/context-graph-mejepa-train/src/learning_signal.rs` OR split into `learning_signal/classifier_types.rs` if file approaches 500 lines.

**Cross-phase dep:** TASK-UTML-009 consumes these enums; this task lands the data models alone.

**FSV for this task:** unit tests exhaustively constructing each `UtmlState` variant; serde round-trip every public type; `DeltaPAggregator::aggregate` produces clamped finite output. Source of Truth: typed structures + serde JSON.

---

```xml
<task_spec id="TASK-UTML-002" version="1.0">

<metadata>
  <title>Foundation enums: SkipReason (6 variants) + UtmlState (7 variants) + Intervention + ClassifierVerdict + DeltaPAggregator</title>
  <status>blocked</status>
  <layer>foundation</layer>
  <sequence>2</sequence>
  <implements>
    <item ref="REQ-UTML-08">UTML cycle classifier 7-state enum</item>
    <item ref="REQ-UTML-02">DeltaPAggregator (Mean | Max | TopKMean | EntityWeighted)</item>
    <item ref="REQ-UTML-07">SkipReason variants populated when L_step==0</item>
    <item ref="component:UtmlState">crates/context-graph-mejepa-train/src/learning_signal/classifier.rs</item>
    <item ref="component:DeltaPAggregator">crates/context-graph-mejepa-train/src/learning_signal/delta_p.rs</item>
  </implements>
  <depends_on>
    <task_ref>TASK-UTML-001</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>3 created, 2 modified</estimated_files>
</metadata>

<context>
The classifier (TASK-UTML-009) and the per-chunk ΔP aggregator (TASK-UTML-003) consume these enums. They are simple discriminated unions but must ship before any compute function references them. Pure type definitions + serde + display impls; no logic.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (utml_v1_invariants — coaching order)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-UTML.md (US-UTML-08 7-state classifier; EC-UTML-12 aggregator)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-UTML.md (data_models: UtmlState, ClassifierVerdict, SkipReason, DeltaPAggregator)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/utml-learning-formula/TASK-UTML-001.md (UtmlError already shipped)</file>
</input_context_files>

<prerequisites>
  <check>TASK-UTML-001 complete (UtmlError + components shipped)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `src/learning_signal/skip_reason.rs` with `SkipReason` enum (6 variants: PerfectPrediction, CatastrophicForgetting, NanInf, GoodhartCatastrophe, SnrBelowFloor, OperatorAbort) per TECH-UTML data_models
    - Create `src/learning_signal/state.rs` with `UtmlState` enum (7 variants: Stuck, OptimizerDysregulated, LatentCollapsing, Confused, Optimal, Boring, Dissipating); `Intervention` enum (CurriculumPrune, RegulateOmega, RegulateXi, AdvanceCurriculum, None, StateRegulate); `ClassifierVerdict` struct (state, recommended_intervention, diagnostic_payload, confidence)
    - Create `src/learning_signal/aggregator.rs` with `DeltaPAggregator` enum (Mean, Max, TopKMean { k: usize }, EntityWeighted)
    - Modify `src/learning_signal/mod.rs` to re-export new types
    - Modify `src/cert/mod.rs` to change `skip_reason: Option<String>` to `skip_reason: Option<SkipReason>` (typed)
    - All enums serialize with `#[serde(rename_all = "snake_case")]` matching TECH-UTML constraints
    - Display + FromStr impls on each enum for SCREAMING_SNAKE/snake_case conversions where useful
  </in_scope>
  <out_of_scope>
    - Classifier logic (TASK-UTML-009)
    - Aggregator math (TASK-UTML-003)
    - Skip-reason emission rules (TASK-UTML-007 / TRAINING)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-train/src/learning_signal/skip_reason.rs">
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    PerfectPrediction,
    CatastrophicForgetting,
    NanInf,
    GoodhartCatastrophe,
    SnrBelowFloor,
    OperatorAbort,
}

impl SkipReason {
    pub fn as_str(&amp;self) -&gt; &amp;'static str;
}
    </signature>
    <signature file="crates/context-graph-mejepa-train/src/learning_signal/state.rs">
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UtmlState {
    Stuck,
    OptimizerDysregulated,
    LatentCollapsing,
    Confused,
    Optimal,
    Boring,
    Dissipating,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intervention {
    CurriculumPrune,
    RegulateOmega,
    RegulateXi,
    AdvanceCurriculum,
    None,
    StateRegulate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassifierVerdict {
    pub state: UtmlState,
    pub recommended_intervention: Intervention,
    pub diagnostic_payload: Value,
    pub confidence: f32,
}

impl UtmlState {
    pub fn default_intervention(&amp;self) -&gt; Intervention;
}
    </signature>
    <signature file="crates/context-graph-mejepa-train/src/learning_signal/aggregator.rs">
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeltaPAggregator {
    Mean,
    Max,
    TopKMean { k: usize },
    EntityWeighted,
}

impl DeltaPAggregator {
    pub fn label(&amp;self) -&gt; String; // "Mean" | "Max" | "TopKMean(k=N)" | "EntityWeighted"
}
    </signature>
  </signatures>
  <constraints>
    - All enums Clone+Copy+Eq where the variant payload allows (TopKMean{k} blocks Copy; OK to leave it Clone-only)
    - serde rename_all = "snake_case" on simple enums; tag = "kind" on aggregator (so TopKMean{k} round-trips)
    - `default_intervention(&amp;self)` mirrors UTML §IX coaching order: Stuck=>CurriculumPrune, OptimizerDysregulated=>RegulateOmega, LatentCollapsing=>RegulateXi, Confused=>None, Optimal=>None, Boring=>AdvanceCurriculum, Dissipating=>StateRegulate
    - cert.skip_reason changes type from Option<String> to Option<SkipReason>; existing serde JSON form remains backwards-compatible because untagged unit variants serialize as strings already (test the round-trip)
    - All files under 200 lines
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa-train` succeeds
    - `cargo test -p context-graph-mejepa-train` shows ≥ 4 new tests (one round-trip per enum + default_intervention table)
    - JSON for `SkipReason::PerfectPrediction` is `"perfect_prediction"` (lowercase snake_case)
    - JSON for `DeltaPAggregator::TopKMean{k:5}` is `{"kind":"top_k_mean","k":5}`
  </verification>
</definition_of_done>

<pseudo_code>
// state.rs
impl UtmlState {
    pub fn default_intervention(&amp;self) -&gt; Intervention {
        match self {
            UtmlState::Stuck => Intervention::CurriculumPrune,
            UtmlState::OptimizerDysregulated => Intervention::RegulateOmega,
            UtmlState::LatentCollapsing => Intervention::RegulateXi,
            UtmlState::Confused => Intervention::None,
            UtmlState::Optimal => Intervention::None,
            UtmlState::Boring => Intervention::AdvanceCurriculum,
            UtmlState::Dissipating => Intervention::StateRegulate,
        }
    }
}

// aggregator.rs
impl DeltaPAggregator {
    pub fn label(&amp;self) -&gt; String {
        match self {
            Self::Mean => "Mean".into(),
            Self::Max => "Max".into(),
            Self::TopKMean { k } => format!("TopKMean(k={})", k),
            Self::EntityWeighted => "EntityWeighted".into(),
        }
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/skip_reason.rs">SkipReason enum</file>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/state.rs">UtmlState + Intervention + ClassifierVerdict</file>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/aggregator.rs">DeltaPAggregator</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/mod.rs">re-export new modules</file>
  <file path="crates/context-graph-mejepa-train/src/cert/mod.rs">TrainingCertificate.skip_reason: Option&lt;SkipReason&gt; (was Option&lt;String&gt;)</file>
</files_to_modify>

<validation_criteria>
  <criterion>SkipReason has exactly 6 variants matching TECH-UTML</criterion>
  <criterion>UtmlState has exactly 7 variants</criterion>
  <criterion>default_intervention table matches UTML §IX coaching order</criterion>
  <criterion>Serde round-trips for all enums</criterion>
  <criterion>cargo build/test/clippy clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-train</command>
  <command>cargo test -p context-graph-mejepa-train</command>
  <command>cargo clippy -p context-graph-mejepa-train -- -D warnings</command>
</test_commands>

</task_spec>
```

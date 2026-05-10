# TASK-UTML-009 — UtmlClassifier (7 states) + prune_decision + Consolidation M(t) + StateTransfer T + Continual-Backprop dormant-unit re-init

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

## 📋 2026-05-10 STATE AUDIT — TASK NOT SUPERSEDED (PENDING — clean implementation work)

**Status: PENDING. No supersession.** Host crate `context-graph-mejepa-train` exists, but NONE of the symbols this task introduces are in tree.

**NOT FOUND in `crates/context-graph-mejepa-train/src/` (full grep `UtmlClassifier|EmbedderCoherence|compute_embedder_coherence|UtmlState|ClassifierVerdict|update_grad_ema|update_m_t|detect_dissipating|reinit_dormant|prune_decision|state_transfer|dormant_unit|continual_backprop|active_learning_trigger` returns zero hits):**

- `UtmlClassifier::classify` (7-state coaching-order priority classifier).
- `prune_decision` (3-condition agreement gate).
- `Consolidation::update_m_t` (consolidation metric tracker).
- `Consolidation::detect_dissipating`.
- `StateTransfer::compute_t` (state-transfer T detector for REQ-UTML-08 active-learning trigger).
- `ContinualBackprop::reinit_dormant_units` (dormant-unit re-init per REQ-UTML-07).

**Direction for future agents:** Task remains fully actionable as authored. Implement against the existing `src/learning_signal.rs` module (extend with classifier/consolidation/state-transfer/continual-backprop sub-modules) rather than creating a separate crate. The dependencies on TASK-UTML-002 (`UtmlState`, `ClassifierVerdict`, `Intervention`) remain pending — sequence accordingly.

---

```xml
<task_spec id="TASK-UTML-009" version="1.0">

<metadata>
  <title>UtmlClassifier::classify (7 states with coaching-order priority) + prune_decision (3-condition agreement) + Consolidation::update_m_t + Consolidation::detect_dissipating + StateTransfer::compute_t + ContinualBackprop::reinit_dormant_units</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>9</sequence>
  <implements>
    <item ref="REQ-UTML-08">7-state classifier; coaching order ΔΩ/ΔΞ first</item>
    <item ref="REQ-UTML-10">M(t) consolidation integral + Dissipating detector</item>
    <item ref="REQ-UTML-11">T(train, deploy) divergence; performance_deploy = M(t) · T</item>
    <item ref="REQ-UTML-12">Pruning rule: 3-condition agreement</item>
    <item ref="AC-UTML-08-01..07">Each of 7 state transitions</item>
    <item ref="AC-UTML-10-01..03">M(t) discrete cumulative + Dissipating threshold</item>
    <item ref="AC-UTML-11-01..02">T(train,deploy) divergence + performance_deploy</item>
    <item ref="EC-UTML-07,08,09">OptimizerDysregulated / LatentCollapsing / Dissipating handling</item>
    <item ref="component:UtmlClassifier">crates/context-graph-mejepa-train/src/learning_signal/classifier.rs</item>
    <item ref="component:Consolidation">crates/context-graph-mejepa-train/src/learning_signal/consolidation.rs</item>
    <item ref="component:StateTransfer">crates/context-graph-mejepa-train/src/learning_signal/state_transfer.rs</item>
  </implements>
  <depends_on>
    <task_ref>TASK-UTML-002</task_ref>
    <task_ref>TASK-UTML-007</task_ref>
    <task_ref>TASK-UTML-008</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>4 created</estimated_files>
</metadata>

<context>
The classifier surfaces the right intervention to Phase 5 self-healing per UTML §IX coaching order: regulate ΔΩ + ΔΞ FIRST, then ΔP/ΔK. The pruning rule requires ALL THREE signals (L_step low, ΔΩ &gt; 0.4, ΔΞ &gt; 0.4) to agree — wrong-cause pruning permanently excludes useful training data. M(t) is the consolidation integral (UTML §VII): updates without representational integrity (ΔΞ ≈ 0) cannot consolidate. T(train, deploy) is the cross-language transfer-bandwidth measure (UTML §VIII). Continual-backprop dormant-unit re-init is the ΔΩ.effective_plasticity intervention (Sutton 2024).
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (utml_v1_invariants rule 3 coaching order; rule 5 pruning)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-UTML.md (US-UTML-08, US-UTML-10, US-UTML-11; AC-UTML-08-01..07, AC-UTML-10-01..03, AC-UTML-11-01..02; EC-UTML-07..09; ERR-UTML-04)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-UTML.md (components UtmlClassifier, Consolidation; LStepWindow data model)</file>
  <file purpose="prior_tasks">TASK-UTML-002 (UtmlState/Intervention/ClassifierVerdict); TASK-UTML-007 (compute_l_step); TASK-UTML-008 (predictor_redundancy)</file>
</input_context_files>

<prerequisites>
  <check>TASK-UTML-002 complete (UtmlState + Intervention + ClassifierVerdict)</check>
  <check>TASK-UTML-007 complete (l_step + sampling_weight)</check>
  <check>TASK-UTML-008 complete (pairwise_mi providing predictor_redundancy)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `src/learning_signal/classifier.rs` with `LStepWindow` ring buffer struct (`l_step: VecDeque&lt;f32&gt;`, `delta_omega: VecDeque&lt;f32&gt;`, `delta_xi: VecDeque&lt;f32&gt;`) + `UtmlClassifier::classify(window, consolidation_gain_ratio, n_low_threshold) -&gt; Result&lt;ClassifierVerdict, UtmlError&gt;` + `prune_decision(window, n_threshold) -&gt; PruneDecision`
    - Implement coaching-order precedence: LatentCollapsing &gt; OptimizerDysregulated &gt; Stuck &gt; Dissipating &gt; Confused &gt; Optimal &gt; Boring (per TECH-UTML behavior step 9)
    - Confidence threshold (default 0.7); below threshold → emit UTML_CLASSIFIER_AMBIGUOUS warning + default to most-conservative intervention (StateRegulate)
    - `PruneDecision { Prune { evidence }, DoNotPrune { reason: String } }` with reason ∈ {"omega_unhealthy_diagnose_optimizer", "xi_unhealthy_diagnose_latent", "l_step_not_persistently_low"}
    - Create `src/learning_signal/consolidation.rs` with:
      - `pub fn update_m_t(m_prev: f32, l_step: f32, delta_xi: f32, k_replay: f32, tau_decay_days: f32, delta_t_days: f32) -&gt; f32`
      - `pub fn detect_dissipating(window_l_step: &amp;[f32], delta_m_over_window: f32) -&gt; Option&lt;DissipatingDiagnostic&gt;`
      - `pub struct DissipatingDiagnostic { pub ratio: f32, pub mean_l_step: f32 }`
      - `pub const DEFAULT_K_REPLAY_NORMAL: f32 = 1.0;` `DEFAULT_TAU_DECAY_DAYS_UN_EWC: f32 = 7.0;` `DEFAULT_TAU_DECAY_DAYS_HIGH_FISHER: f32 = 365.0;`
    - Create `src/learning_signal/state_transfer.rs` with:
      - `pub enum DivergenceMetric { Wasserstein, Mmd, Kl }`
      - `pub fn compute_t(p_train_centroid: &amp;[f32], p_deploy_centroid: &amp;[f32], lambda: f32, metric: DivergenceMetric) -&gt; Result&lt;f32, UtmlError&gt;`
      - `pub fn performance_deploy(m_t: f32, t: f32) -&gt; f32` (= m_t * t)
    - Create `src/learning_signal/continual_backprop.rs` with:
      - `pub struct DormantUnitDetector { pub dormancy_threshold: f32 /* default 0.99 fraction of zero activations */ }`
      - `pub fn detect_dormant_units(activations: &amp;[Vec&lt;f32&gt;]) -&gt; Vec&lt;usize&gt;` returns indices of dormant units
      - `pub fn reinit_dormant_units(weights: &amp;mut [Vec&lt;f32&gt;], biases: &amp;mut [f32], dormant_idx: &amp;[usize], rng_seed: u64)` re-initializes via Xavier/Glorot using seeded RNG
    - Unit tests: 5 synthetic regimes (Stuck, OptimizerDysregulated, LatentCollapsing, Optimal, Dissipating) → classifier identifies each; OptimizerDysregulated does NOT prune; LatentCollapsing does NOT prune; Stuck DOES prune; pruning 3-condition AND-agreement; M(t) integrand zeroed when ΔΞ=0; Dissipating detected at threshold; T identical → 1.0; T cross-language &lt; 1.0; dormant detection at 0.99 zero fraction; reinit changes weights at dormant indices only
  </in_scope>
  <out_of_scope>
    - Phase 5 A/B/C retrain orchestration
    - Phase 4 conformal calibration
    - Phase 4b TCT constellation
    - Trainer integration (TASK-TRAIN-005..012)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-train/src/learning_signal/classifier.rs">
use std::collections::VecDeque;
use serde_json::json;
use crate::error::{UtmlError, UtmlErrorCode};
use crate::learning_signal::state::{UtmlState, Intervention, ClassifierVerdict};

pub const DEFAULT_N_LOW_THRESHOLD: u64 = 100;
pub const DEFAULT_WINDOW_SIZE: usize = 1000;
pub const DEFAULT_CONFIDENCE_THRESHOLD: f32 = 0.7;
pub const PRUNE_OMEGA_THROUGHOUT_THRESHOLD: f32 = 0.4;
pub const PRUNE_XI_THROUGHOUT_THRESHOLD: f32 = 0.4;
pub const PRUNE_L_STEP_LOW_THRESHOLD: f32 = 0.1;

#[derive(Debug, Clone)]
pub struct LStepWindow {
    pub l_step: VecDeque&lt;f32&gt;,
    pub delta_omega: VecDeque&lt;f32&gt;,
    pub delta_xi: VecDeque&lt;f32&gt;,
    pub max_size: usize,
}

impl LStepWindow {
    pub fn new(max_size: usize) -&gt; Self;
    pub fn push(&amp;mut self, l: f32, o: f32, x: f32);
    pub fn len(&amp;self) -&gt; usize;
}

pub fn classify(
    window: &amp;LStepWindow,
    consolidation_gain_ratio: f32,
    n_low_threshold: u64,
    confidence_threshold: f32,
) -&gt; Result&lt;ClassifierVerdict, UtmlError&gt;;

#[derive(Debug, Clone)]
pub enum PruneDecision {
    Prune { evidence: serde_json::Value },
    DoNotPrune { reason: String },
}

pub fn prune_decision(window: &amp;LStepWindow, n_threshold: u64) -&gt; PruneDecision;
    </signature>
    <signature file="crates/context-graph-mejepa-train/src/learning_signal/consolidation.rs">
pub const DEFAULT_K_REPLAY_NORMAL: f32 = 1.0;
pub const DEFAULT_TAU_DECAY_DAYS_UN_EWC: f32 = 7.0;
pub const DEFAULT_TAU_DECAY_DAYS_HIGH_FISHER: f32 = 365.0;
pub const DISSIPATING_RATIO_THRESHOLD: f32 = 0.2;
pub const DISSIPATING_MEAN_L_STEP_GATE: f32 = 0.3;

#[derive(Debug, Clone, Copy)]
pub struct DissipatingDiagnostic { pub ratio: f32, pub mean_l_step: f32 }

pub fn update_m_t(m_prev: f32, l_step: f32, delta_xi: f32, k_replay: f32, tau_decay_days: f32, delta_t_days: f32) -&gt; f32;

pub fn detect_dissipating(window_l_step: &amp;[f32], delta_m_over_window: f32) -&gt; Option&lt;DissipatingDiagnostic&gt;;
    </signature>
    <signature file="crates/context-graph-mejepa-train/src/learning_signal/state_transfer.rs">
use crate::error::UtmlError;

#[derive(Debug, Clone, Copy)]
pub enum DivergenceMetric { Wasserstein, Mmd, Kl }

pub fn compute_t(
    p_train_centroid: &amp;[f32],
    p_deploy_centroid: &amp;[f32],
    lambda: f32,
    metric: DivergenceMetric,
) -&gt; Result&lt;f32, UtmlError&gt;;

pub fn performance_deploy(m_t: f32, t: f32) -&gt; f32;
    </signature>
    <signature file="crates/context-graph-mejepa-train/src/learning_signal/continual_backprop.rs">
pub struct DormantUnitDetector {
    pub dormancy_threshold: f32, // default 0.99
}

impl DormantUnitDetector {
    pub fn new(threshold: f32) -&gt; Self;
    pub fn detect_dormant_units(&amp;self, activations: &amp;[Vec&lt;f32&gt;]) -&gt; Vec&lt;usize&gt;;
}

pub fn reinit_dormant_units(
    weights: &amp;mut [Vec&lt;f32&gt;],
    biases: &amp;mut [f32],
    dormant_idx: &amp;[usize],
    rng_seed: u64,
);
    </signature>
  </signatures>
  <constraints>
    - Coaching-order priority hard-coded per UTML §IX
    - Pruning rule: ALL THREE conditions must agree (n_low &gt;= n_threshold AND every omega &gt; 0.4 AND every xi &gt; 0.4)
    - PruneDecision::DoNotPrune.reason explains diagnosis: "omega_unhealthy_diagnose_optimizer" / "xi_unhealthy_diagnose_latent" / "l_step_not_persistently_low"
    - update_m_t: m_t = m_prev * exp(-delta_t/tau) + (l_step * delta_xi * k_replay)
    - detect_dissipating: returns Some only if mean(window) &gt; DISSIPATING_MEAN_L_STEP_GATE AND ratio &lt; DISSIPATING_RATIO_THRESHOLD
    - compute_t: T = exp(-d / lambda); validate centroids same length, lambda &gt; 0; UtmlFactorOutOfRange otherwise
    - Wasserstein: sliced 1D approximation between sorted centroids; MMD: RBF kernel; KL: discretized
    - performance_deploy = (m_t * t).max(0.0)
    - dormant unit detection: count activations near zero (|act| &lt; 1e-6); fraction &gt;= threshold → dormant
    - reinit_dormant_units uses SplitMix64 (seed) → Xavier init with fan_in/fan_out from weight shape
    - All files &lt; 500 lines
  </constraints>
  <verification>
    - cargo test shows ≥ 25 tests pass across the four files
    - 5-regime synthetic trace test passes (TC-UTML-05)
    - Pruning rule three-negative-cases test passes (TC-UTML-10)
    - M(t) Dissipating test passes (TC-UTML-11)
    - T identical-distribution → 1.0 test passes
  </verification>
</definition_of_done>

<pseudo_code>
fn classify(window, gain_ratio, n_low_thr, conf_thr) -&gt; Result&lt;ClassifierVerdict, UtmlError&gt; {
    if window.len() == 0 { return Err(UtmlClassifierAmbiguous); }
    let mean_l = window.l_step.iter().sum::&lt;f32&gt;() / window.len() as f32;
    let mean_o = window.delta_omega.iter().sum::&lt;f32&gt;() / window.len() as f32;
    let mean_x = window.delta_xi.iter().sum::&lt;f32&gt;() / window.len() as f32;
    let n_low = window.l_step.iter().filter(|&amp;&amp;l| l &lt; PRUNE_L_STEP_LOW_THRESHOLD).count() as u64;
    // Coaching order: ΔΩ/ΔΞ first
    if mean_l &lt; 0.1 &amp;&amp; mean_x &lt; 0.4 {
        return Ok(ClassifierVerdict { state: LatentCollapsing, recommended_intervention: RegulateXi, ... });
    }
    if mean_l &lt; 0.1 &amp;&amp; mean_o &lt; 0.4 {
        return Ok(ClassifierVerdict { state: OptimizerDysregulated, recommended_intervention: RegulateOmega, ... });
    }
    if n_low &gt;= n_low_thr &amp;&amp; window.delta_omega.iter().all(|&amp;o| o &gt; 0.4) &amp;&amp; window.delta_xi.iter().all(|&amp;x| x &gt; 0.4) {
        return Ok(ClassifierVerdict { state: Stuck, recommended_intervention: CurriculumPrune, ... });
    }
    if mean_l &gt; 0.3 &amp;&amp; gain_ratio &lt; 0.2 {
        return Ok(ClassifierVerdict { state: Dissipating, recommended_intervention: StateRegulate, ... });
    }
    if mean_l &lt; 0.3 { return Ok(...Confused, None); }
    if mean_l &lt; 0.7 { return Ok(...Optimal, None); }
    Ok(...Boring, AdvanceCurriculum)
}

fn prune_decision(window, n_thr) -&gt; PruneDecision {
    let n_low = window.l_step.iter().filter(|&amp;&amp;l| l &lt; 0.1).count() as u64;
    let omega_ok = window.delta_omega.iter().all(|&amp;o| o &gt; 0.4);
    let xi_ok = window.delta_xi.iter().all(|&amp;x| x &gt; 0.4);
    if n_low &gt;= n_thr &amp;&amp; omega_ok &amp;&amp; xi_ok {
        return PruneDecision::Prune { evidence: json!({"n_low": n_low, "mean_omega": ..., "mean_xi": ...}) };
    }
    PruneDecision::DoNotPrune { reason:
        if !omega_ok { "omega_unhealthy_diagnose_optimizer" }
        else if !xi_ok { "xi_unhealthy_diagnose_latent" }
        else { "l_step_not_persistently_low" }.into()
    }
}

fn update_m_t(m, l, x, k, tau, dt) -&gt; f32 { m * (-dt / tau).exp() + l * x * k }
fn detect_dissipating(w, dm) -&gt; Option&lt;DissipatingDiagnostic&gt; {
    let mean = w.iter().sum::&lt;f32&gt;() / w.len() as f32;
    if mean &lt;= DISSIPATING_MEAN_L_STEP_GATE { return None; }
    let r = dm / mean;
    if r &lt; DISSIPATING_RATIO_THRESHOLD { Some(DissipatingDiagnostic{ratio:r, mean_l_step:mean}) } else { None }
}

fn compute_t(p, q, lambda, metric) -&gt; Result&lt;f32, UtmlError&gt; {
    if p.len() != q.len() || lambda &lt;= 0.0 { return Err(...); }
    let d = match metric {
        Wasserstein => sliced_w_1d(p, q),
        Mmd => mmd_rbf(p, q),
        Kl => kl_discretized(p, q),
    };
    Ok((-d / lambda).exp().clamp(0.0, 1.0))
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/classifier.rs">LStepWindow + classify + prune_decision + tests</file>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/consolidation.rs">update_m_t + detect_dissipating + DissipatingDiagnostic + tests</file>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/state_transfer.rs">compute_t + performance_deploy + DivergenceMetric + tests</file>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/continual_backprop.rs">DormantUnitDetector + reinit_dormant_units + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/mod.rs">re-export new modules</file>
</files_to_modify>

<validation_criteria>
  <criterion>5-regime synthetic trace correctly classified</criterion>
  <criterion>Pruning rule rejects all 3 negative cases (TC-UTML-10)</criterion>
  <criterion>OptimizerDysregulated and LatentCollapsing surface BEFORE Stuck (coaching order)</criterion>
  <criterion>Dissipating detected at threshold; M(t) integrand zeroed when ΔΞ=0</criterion>
  <criterion>compute_t identical distribution → 1.0; lambda &lt; 0 → error</criterion>
  <criterion>Dormant unit detection at 0.99 zero-fraction threshold</criterion>
  <criterion>reinit_dormant_units changes only flagged indices; deterministic given seed</criterion>
  <criterion>cargo build/test/clippy clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-train</command>
  <command>cargo test -p context-graph-mejepa-train --lib classifier</command>
  <command>cargo test -p context-graph-mejepa-train --lib consolidation</command>
  <command>cargo test -p context-graph-mejepa-train --lib state_transfer</command>
  <command>cargo test -p context-graph-mejepa-train --lib continual_backprop</command>
  <command>cargo clippy -p context-graph-mejepa-train -- -D warnings</command>
</test_commands>

</task_spec>
```

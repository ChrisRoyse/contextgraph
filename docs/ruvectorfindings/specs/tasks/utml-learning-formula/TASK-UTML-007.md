# TASK-UTML-007 — `compute_l_step` (multiplicative ΔP × ΔK × ΔΩ × ΔΞ) + sampling_weight + drop/force gates

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

## 📋 2026-05-11 STATE AUDIT — TASK COMPLETE

> **Codebase reality table:** see `_index.md` in this directory.

**Status: COMPLETE.** All four pieces SHIPPED in `crates/context-graph-mejepa-train/src/learning_signal.rs`:
- `compute_l_step(...) -> Result<f32, UtmlError>` at line 232 — multiplicative `L_step = ΔP × ΔK × ΔΩ × ΔΞ` per UTML §II; clamped `[0, 1]`; NaN/Inf → structured error.
- `sampling_weight(l_step, operator_override, age_days, age_decay) -> f32` at line 259 — sampling reweighting per UTML §X.
- `should_drop(l_step) -> Option<SkipReason>` and `should_force_include(operator_override) -> bool` — drop/force gates used by `BatchSampler::sample` (sister `sampler/mod.rs:103`).
- `BatchSampler` integrates all three (sister `sampler/mod.rs:34-103`).

**Per-step training cert with `sampling_decision` field** SHIPPED via `cert/writer.rs::TrainCertWriter::emit` — every cert records `dropped_l_step_below_threshold | observed | force_included | replay`.

**No further work needed for the core compute + reweighting + drop/force gates.** Future enhancements (e.g., curriculum-prune support per UTML §X) could extend `should_drop` with N-step-history checking — that goes in PENDING TASK-UTML-009 (the classifier owns multi-step state).

**Verification commands:**
- `cargo test -p context-graph-mejepa-train learning_signal::tests::compute_l_step` — passes
- `cargo test -p context-graph-mejepa-train sampler::tests` — passes

---

```xml
<task_spec id="TASK-UTML-007" version="1.0">

<metadata>
  <title>compute_l_step (multiplicative product clamped [0,1]) + sampling_weight (base · L · (1+5·override) · 0.995^age) + should_drop/should_force_include thresholds</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>7</sequence>
  <implements>
    <item ref="REQ-UTML-01">L_step = ΔP × ΔK × ΔΩ × ΔΞ multiplicative; clamped [0,1]</item>
    <item ref="REQ-UTML-06">sampling_weight = base · L_step · (1+5·override) · decay^age; drop&lt;0.05; force&gt;1.0</item>
    <item ref="AC-UTML-01-01">Multiplicative product; deterministic byte-equal under same code_version</item>
    <item ref="AC-UTML-01-02">Any factor==0 → L_step==0 exactly</item>
    <item ref="AC-UTML-06-01..03">Sampling weight gates</item>
    <item ref="component:LStep">crates/context-graph-mejepa-train/src/learning_signal/l_step.rs</item>
    <item ref="component:SamplingWeight">crates/context-graph-mejepa-train/src/learning_signal/sampling_weight.rs</item>
  </implements>
  <depends_on>
    <task_ref>TASK-UTML-003</task_ref>
    <task_ref>TASK-UTML-004</task_ref>
    <task_ref>TASK-UTML-005</task_ref>
    <task_ref>TASK-UTML-006</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>2 created</estimated_files>
</metadata>

<context>
The hot-path L_step product and the sampling_weight that consumes it. Per UTML §I multiplication is non-negotiable: any single factor at zero zeros the entire learning signal. Per UTML §6 the sampling_weight replaces doc 09 §3.7.2's older `(1 + 2·residual_error_z_score)` term — L_step already captures hard-example mining via ΔP AND enforces ΔΞ-safety + ΔΩ-safety + ΔK-coherence which residual-z-score did not.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (utml_v1_invariants rule 1)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-UTML.md (US-UTML-01, US-UTML-06; AC-UTML-01-01..03, AC-UTML-06-01..03; EC-UTML-01..06)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-UTML.md (components LStep + SamplingWeight)</file>
  <file purpose="prior_tasks">TASK-UTML-003.md, TASK-UTML-004.md, TASK-UTML-005.md, TASK-UTML-006.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-UTML-003..006 all complete (compute_delta_p/k/omega/xi all shipped)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `src/learning_signal/l_step.rs` with `compute_l_step(delta_p, delta_k, delta_omega, delta_xi) -&gt; f32`
    - Create `src/learning_signal/sampling_weight.rs` with `sampling_weight(l_step, override, age_days, decay) -&gt; f32`, `should_drop(weight) -&gt; bool`, `should_force_include(weight) -&gt; bool`, plus `DROP_THRESHOLD = 0.05`, `FORCE_THRESHOLD = 1.0`, `DEFAULT_DECAY = 0.995`, `DEFAULT_BASE_WEIGHT = 1.0`, `OVERRIDE_MULTIPLIER = 6.0` constants
    - compute_l_step is hot-path: validation in debug only; release fast-path; clamp post-product to [0,1] for f32 epsilon
    - sampling_weight: validate in debug; release fast-path; non-negative output; not clamped to [0,1] (override pushes above)
    - Property tests via proptest crate: 10,000 random (ΔP, ΔK, ΔΩ, ΔΞ) tuples in [0,1]^4 → output ∈ [0,1] equals product
    - Unit tests: each factor=0 → 0; all factors=1 → 1; deterministic byte-equal across two runs with same input; sampling_weight=0.04 → drop; weight=0.95 → keep; weight=1.05 → force; operator_override=true with L_step=0.167 + age=0 → weight=1.0+ → force; age decay 140-day half-life sanity check
  </in_scope>
  <out_of_scope>
    - BatchSampler (TRAINING phase TASK-TRAIN-005)
    - Cert chain hashing (TASK-UTML-007 stops at producing the LearningSignal struct; cert chain hashing happens in TASK-TRAIN-009)
    - Classifier (TASK-UTML-009)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-train/src/learning_signal/l_step.rs">
/// L_step = ΔP × ΔK × ΔΩ × ΔΞ; multiplicative gating per constitution &lt;utml_v1_invariants&gt; rule 1.
/// Hot-path: validation in debug only; release uses upstream gates (compute_delta_* return Result).
pub fn compute_l_step(delta_p: f32, delta_k: f32, delta_omega: f32, delta_xi: f32) -&gt; f32;
    </signature>
    <signature file="crates/context-graph-mejepa-train/src/learning_signal/sampling_weight.rs">
pub const DROP_THRESHOLD: f32 = 0.05;
pub const FORCE_THRESHOLD: f32 = 1.0;
pub const DEFAULT_BASE_WEIGHT: f32 = 1.0;
pub const DEFAULT_DECAY: f32 = 0.995;
pub const OVERRIDE_MULTIPLIER: f32 = 6.0; // (1 + 5·1)

pub fn sampling_weight(
    l_step: f32,
    operator_override: bool,
    age_days: f32,
    decay: f32, // default DEFAULT_DECAY
) -&gt; f32;

pub fn should_drop(weight: f32) -&gt; bool;
pub fn should_force_include(weight: f32) -&gt; bool;
    </signature>
  </signatures>
  <constraints>
    - compute_l_step debug_asserts each input ∈ [0, 1]; release does not assert (upstream `compute_delta_*` already returns Err on out-of-range)
    - Output clamped to [0, 1] post-product for f32 rounding (1.0 + 1e-7 → 1.0)
    - sampling_weight debug_asserts l_step ∈ [0,1], age_days &gt;= 0, decay ∈ (0,1)
    - sampling_weight = base * l_step * (override ? OVERRIDE_MULTIPLIER : 1.0) * decay.powf(age_days)
    - Output non-negative; NOT clamped to [0,1]
    - should_drop: weight &lt; 0.05 (strict)
    - should_force_include: weight &gt; 1.0 (strict)
    - Both files &lt; 250 lines
    - Property test must pass: forall (a,b,c,d) ∈ [0,1]^4, compute_l_step(a,b,c,d) ∈ [0,1] AND equals a*b*c*d after clamp
    - Determinism test: same inputs produce byte-equal outputs (call twice, compare bits)
  </constraints>
  <verification>
    - cargo test shows ≥ 12 tests pass across both files
    - proptest with 10,000 cases in [0,1]^4 passes
    - Determinism: f32::to_bits comparison stable across two runs
  </verification>
</definition_of_done>

<pseudo_code>
pub fn compute_l_step(p: f32, k: f32, o: f32, x: f32) -&gt; f32 {
    debug_assert!((0.0..=1.0).contains(&amp;p), "delta_p={p} out of [0,1]");
    debug_assert!((0.0..=1.0).contains(&amp;k));
    debug_assert!((0.0..=1.0).contains(&amp;o));
    debug_assert!((0.0..=1.0).contains(&amp;x));
    (p * k * o * x).clamp(0.0, 1.0)
}

pub fn sampling_weight(l: f32, ovr: bool, age: f32, decay: f32) -&gt; f32 {
    debug_assert!((0.0..=1.0).contains(&amp;l));
    debug_assert!(age &gt;= 0.0);
    debug_assert!((0.0..1.0).contains(&amp;decay) &amp;&amp; decay &gt; 0.0);
    let mult = if ovr { OVERRIDE_MULTIPLIER } else { 1.0 };
    let recency = decay.powf(age);
    DEFAULT_BASE_WEIGHT * l * mult * recency
}

pub fn should_drop(w: f32) -&gt; bool { w &lt; DROP_THRESHOLD }
pub fn should_force_include(w: f32) -&gt; bool { w &gt; FORCE_THRESHOLD }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/l_step.rs">compute_l_step + tests</file>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/sampling_weight.rs">sampling_weight + should_drop + should_force_include + constants + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/mod.rs">pub mod l_step + pub mod sampling_weight + re-exports</file>
</files_to_modify>

<validation_criteria>
  <criterion>compute_l_step(0, 1, 1, 1) == 0 exactly</criterion>
  <criterion>compute_l_step(1, 1, 1, 1) == 1 exactly</criterion>
  <criterion>Property test 10,000 cases: l_step ∈ [0,1] forall inputs in [0,1]^4</criterion>
  <criterion>Determinism: two calls with same f32 bits produce same f32 bits</criterion>
  <criterion>sampling_weight(0, true, 0, 0.995) == 0; sampling_weight(0.5, true, 0, 0.995) == 3.0</criterion>
  <criterion>should_drop(0.04) true; should_drop(0.05) false; should_force_include(1.001) true; should_force_include(1.0) false</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-train</command>
  <command>cargo test -p context-graph-mejepa-train --lib l_step</command>
  <command>cargo test -p context-graph-mejepa-train --lib sampling_weight</command>
  <command>cargo clippy -p context-graph-mejepa-train -- -D warnings</command>
</test_commands>

</task_spec>
```

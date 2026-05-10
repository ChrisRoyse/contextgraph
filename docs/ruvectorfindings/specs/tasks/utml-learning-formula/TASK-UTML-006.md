# TASK-UTML-006 — `compute_delta_xi` (CRITICAL `target_collapse == 0.0` assertion)

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

## 📋 2026-05-10 STATE AUDIT — TASK PARTIALLY SUPERSEDED (CRITICAL SEMANTIC DRIFT)

**Status: PARTIALLY SUPERSEDED with CRITICAL SEMANTIC DRIFT.** Host crate `context-graph-mejepa-train` exists.

**Shipped (grep evidence):**

- `compute_delta_xi(c: DeltaXiComponents) -> Result<f32, UtmlError>` — `src/learning_signal.rs` L184. Current body at L185-L188:
  ```
  mean_checked("delta_xi", [c.target_collapse, c.predictor_collapse, c.redundancy_mi])
  ```
  **CRITICAL DRIFT:** task title specifies `(1−tc)^0.5 · (1−red)^0.3 · (1−viol)^0.2` with a HARD `target_collapse == 0.0` assertion (`MEJEPA_INSTRUMENT_GRADIENT_LEAK` on violation). The shipped form does NOT include the hard assertion. This is the constitution `target_collapse_zero` invariant — failure to enforce here lets gradient leakage from the EMA target encoder go undetected. The shipped body also fails to apply the (1−x)^w power weighting; it is unweighted mean.
- `DeltaXiComponents` carrier — `src/learning_signal.rs` L90.

**NOT FOUND:**

- Explicit `target_collapse == 0.0` guard that emits `UTML_INSTRUMENT_GRADIENT_LEAK` or `MEJEPA_INSTRUMENT_GRADIENT_LEAK`.
- The `(1−tc)^0.5 · (1−red)^0.3 · (1−viol)^0.2` power-weighted product.

**Direction for future agents:** This is a SAFETY-CRITICAL gap. Replace the shipped body to enforce the hard assertion (`if tc != 0.0 { fail }`) BEFORE computing the product — gradient leakage from the EMA target is the single failure mode that silently corrupts ME-JEPA training. Add `UtmlErrorCode::InstrumentGradientLeak` if missing, then ship the power-weighted product. Add a fail-test that constructs `target_collapse = 1e-12` and asserts the function errors.

---

```xml
<task_spec id="TASK-UTML-006" version="1.0">

<metadata>
  <title>compute_delta_xi (target_collapse, predictor_redundancy, constellation_violation_rate) returning (1−tc)^0.5 · (1−red)^0.3 · (1−viol)^0.2 with hard target_collapse==0.0 assertion (MEJEPA_INSTRUMENT_GRADIENT_LEAK on violation)</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>6</sequence>
  <implements>
    <item ref="REQ-UTML-05">ΔΞ representational non-collapse; target_collapse asserted == 0</item>
    <item ref="AC-UTML-05-01">Geometric exponents 0.5/0.3/0.2; first factor locked at 1.0 always</item>
    <item ref="AC-UTML-05-02">target_collapse&gt;0 → MEJEPA_INSTRUMENT_GRADIENT_LEAK CRITICAL fail-closed</item>
    <item ref="AC-UTML-05-03">predictor_redundancy &gt; 0.5 → trigger VICReg bump (downstream signal)</item>
    <item ref="AC-UTML-05-04">constellation_violation_rate from Phase 4b Gτ; &gt;5% triggers re-fit (downstream signal)</item>
    <item ref="EC-UTML-04">target_collapse&gt;0 is the central architectural integrity check</item>
    <item ref="ERR-UTML-02">MEJEPA_INSTRUMENT_GRADIENT_LEAK fail-closed</item>
    <item ref="component:DeltaXi">crates/context-graph-mejepa-train/src/learning_signal/delta_xi.rs</item>
  </implements>
  <depends_on>
    <task_ref>TASK-UTML-001</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 created</estimated_files>
</metadata>

<context>
ΔΞ is THE central architectural integrity check of ME-JEPA. The frozen target panel is produced by externally-defined instruments with zero trainable parameters (constitution &lt;anti_patterns&gt; "no learnable target encoder"). target_collapse is therefore zero by construction. The assertion `target_collapse == 0.0` is the runtime measurement of that structural property: any nonzero value indicates an instrument has accidentally become trainable (gradient flow through what should be a frozen function), which is a CRITICAL bug requiring immediate halt and operator inspection.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (utml_v1_invariants rule 2; anti_patterns no learnable target encoder; failure_taxonomy MEJEPA_INSTRUMENT_GRADIENT_LEAK is implicitly added to extend taxonomy)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-UTML.md (US-UTML-05; AC-UTML-05-01..05; EC-UTML-04; ERR-UTML-02)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-UTML.md (component DeltaXi)</file>
  <file purpose="design_doc">docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §6</file>
  <file purpose="prior_tasks">TASK-UTML-001.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-UTML-001 complete</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `src/learning_signal/delta_xi.rs` with `compute_delta_xi(target_collapse, predictor_redundancy, constellation_violation_rate) -&gt; Result&lt;f32, UtmlError&gt;`
    - Validate redundancy in [0, 1] AND violation in [0, 1]; UtmlFactorOutOfRange otherwise
    - HARD CHECK: `if target_collapse != 0.0 { return Err(MejepaInstrumentGradientLeak with full diagnostic) }` (any nonzero is a critical bug; no epsilon tolerance)
    - In debug builds additionally: `debug_assert_eq!(target_collapse, 0.0, "ME-JEPA architectural integrity violation: target_collapse must be 0.0 by construction; an instrument has accidentally become trainable")`
    - first_factor = (1.0 - target_collapse).powf(0.5) — locked at exactly 1.0 always
    - second_factor = (1.0 - redundancy).powf(0.3)
    - third_factor = (1.0 - violation).powf(0.2)
    - delta_xi = clamp(first*second*third, 0.0, 1.0)
    - Unit tests: target_collapse=0, redundancy=0.1, violation=0.05 → ≈ 0.964; target_collapse=0.001 → fail-closed; target_collapse=NaN → fail-closed; redundancy out of [0,1] → fail-closed
    - Provide a `pub const TARGET_COLLAPSE_TOLERANCE: f32 = 0.0;` (DOCUMENT that no epsilon tolerance is allowed)
  </in_scope>
  <out_of_scope>
    - Predictor redundancy computation (TASK-UTML-008 pairwise MI audit)
    - Constellation Gτ rejection rate (Phase 4b TCT)
    - VICReg bump intervention (Phase 5)
    - L_step assembly (TASK-UTML-007)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-train/src/learning_signal/delta_xi.rs">
use crate::error::{UtmlError, UtmlErrorCode};
use serde_json::json;

/// ME-JEPA architectural invariant: target_collapse MUST be exactly 0.0 by construction.
/// No epsilon tolerance; any nonzero value is a CRITICAL bug indicator.
pub const TARGET_COLLAPSE_TOLERANCE: f32 = 0.0;

pub fn compute_delta_xi(
    target_collapse: f32,
    predictor_redundancy: f32,
    constellation_violation_rate: f32,
) -&gt; Result&lt;f32, UtmlError&gt;;
    </signature>
  </signatures>
  <constraints>
    - target_collapse comparison is EXACT (`!= 0.0`), not `abs() &gt; ε` — by spec; an "instrument has gradient flow" is detected at the first nonzero observation
    - Error context includes `target_collapse`, `predictor_redundancy`, `constellation_violation_rate` as JSON for operator forensics
    - Error message verbatim: "ME-JEPA target_collapse must be 0.0 by construction; instrument has gradient flow"
    - debug_assert_eq! fires only in debug builds (release uses Err return path)
    - On NaN target_collapse: still fails closed (`NaN != 0.0` is true in IEEE 754)
    - File &lt; 200 lines
  </constraints>
  <verification>
    - cargo test shows ≥ 7 tests pass: target_collapse=0 healthy, target_collapse=0.001 critical, target_collapse=NaN critical, target_collapse=-0.0 healthy (since -0.0 == 0.0 in f32 is true), redundancy=1 → ΔΞ=0, violation=1 → ΔΞ=0, exponent table 0.5/0.3/0.2 sums to 1.0
    - Error context includes all three input values
  </verification>
</definition_of_done>

<pseudo_code>
pub fn compute_delta_xi(tc: f32, red: f32, viol: f32) -&gt; Result&lt;f32, UtmlError&gt; {
    if !(0.0..=1.0).contains(&amp;red) { return Err(UtmlError::new(UtmlFactorOutOfRange, format!("redundancy={red} out of [0,1]"))); }
    if !(0.0..=1.0).contains(&amp;viol) { return Err(UtmlError::new(UtmlFactorOutOfRange, format!("violation={viol}"))); }
    if tc != 0.0 {
        debug_assert_eq!(tc, 0.0, "ME-JEPA architectural integrity violation");
        return Err(UtmlError::new(MejepaInstrumentGradientLeak,
                                  "ME-JEPA target_collapse must be 0.0 by construction; instrument has gradient flow")
                          .with_context(json!({"target_collapse": tc, "predictor_redundancy": red, "constellation_violation_rate": viol})));
    }
    let f1 = (1.0 - tc).powf(0.5);   // == 1.0 always
    let f2 = (1.0 - red).powf(0.3);
    let f3 = (1.0 - viol).powf(0.2);
    Ok((f1 * f2 * f3).clamp(0.0, 1.0))
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/delta_xi.rs">compute_delta_xi + tests + provenance comment</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/mod.rs">pub mod delta_xi; pub use delta_xi::*;</file>
</files_to_modify>

<validation_criteria>
  <criterion>target_collapse=0 produces first_factor exactly 1.0</criterion>
  <criterion>target_collapse=0.001 returns Err(MejepaInstrumentGradientLeak)</criterion>
  <criterion>target_collapse=NaN returns Err (NaN != 0.0)</criterion>
  <criterion>Error context JSON includes all three inputs</criterion>
  <criterion>Geometric exponents 0.5+0.3+0.2 sum to 1.0</criterion>
  <criterion>cargo build/test/clippy clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-train</command>
  <command>cargo test -p context-graph-mejepa-train --lib delta_xi</command>
  <command>cargo test -p context-graph-mejepa-train --lib delta_xi -- --release</command>
  <command>cargo clippy -p context-graph-mejepa-train -- -D warnings</command>
</test_commands>

</task_spec>
```

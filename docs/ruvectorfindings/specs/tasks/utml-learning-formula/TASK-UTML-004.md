# TASK-UTML-004 — `compute_delta_k` + `update_grad_ema`

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

**Status: COMPLETE.** `compute_delta_k(c: DeltaKComponents) -> Result<f32, UtmlError>` SHIPPED at `crates/context-graph-mejepa-train/src/learning_signal.rs:171`. Returns clamped `[0, 1]` per UTML §IV formula.

**EMA grad update** (`update_grad_ema`) is integrated into the `Trainer` loop (`crates/context-graph-mejepa-train/src/trainer/`) — exponential moving average of gradient norms feeds into `DeltaKComponents` per step.

**No further work needed.** Verify by:
- `cargo test -p context-graph-mejepa-train learning_signal::tests::compute_delta_k` — passes
- `grep "ema_grad\|grad_ema" crates/context-graph-mejepa-train/src/trainer/` — confirms EMA integration

---

```xml
<task_spec id="TASK-UTML-004" version="1.0">

<metadata>
  <title>compute_delta_k (grad, ema_grad, fisher, ECE, embedder_coherence) returning weighted sum 0.5·cos_align + 0.3·(1-fisher_violation) + 0.15·(1-ECE) + 0.05·embedder_coherence; update_grad_ema in-place EMA update</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>4</sequence>
  <implements>
    <item ref="REQ-UTML-03">ΔK four-term weighted sum + cos_align + EWC++ + ECE + embedder_coherence</item>
    <item ref="AC-UTML-03-01">Weight (0.5, 0.3, 0.15, 0.05) sum to 1.0; embedder_coherence default 0.5 for first 1,000 corpus entries</item>
    <item ref="AC-UTML-03-02">cos_align &lt; 0 → ΔK collapses; UTML_CATASTROPHIC_FORGETTING; rollback</item>
    <item ref="ERR-UTML-07">Catastrophic forgetting structured error</item>
    <item ref="component:DeltaK">crates/context-graph-mejepa-train/src/learning_signal/delta_k.rs</item>
  </implements>
  <depends_on>
    <task_ref>TASK-UTML-001</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (FisherMatrix stub may be co-located)</estimated_files>
</metadata>

<context>
ΔK measures gradient alignment with the prior EMA gradient (NTK alignment, Atanasov 2021), Fisher-violation against EWC++ protected weights (Kirkpatrick 2017), calibration health (ECE early-Goodhart indicator, Gao 2022), and cross-embedder coherence (doc 12 §5.1). Catastrophic-forgetting detection emits structured error when cos_align &lt; threshold. The full Fisher matrix construction lives in Phase 5 self-healing; this task accepts a `FisherMatrix` trait/struct producing `compute_violation(grad)` and consumes it.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (failure_taxonomy: UTML_CATASTROPHIC_FORGETTING)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-UTML.md (US-UTML-03; AC-UTML-03-01..03; ERR-UTML-07; EC-UTML-02)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-UTML.md (component DeltaK — compute_delta_k + update_grad_ema)</file>
  <file purpose="design_doc">docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §4</file>
  <file purpose="design_doc">docs/ruvectorfindings/12_EMBEDDER_INTEGRATION.md §5.1 (embedder_coherence)</file>
  <file purpose="prior_tasks">TASK-UTML-001.md (UtmlError shipped)</file>
</input_context_files>

<prerequisites>
  <check>TASK-UTML-001 complete</check>
  <check>candle-core dep available (added in TASK-UTML-003)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `src/learning_signal/delta_k.rs` with `compute_delta_k`, `update_grad_ema`, and a minimal `FisherMatrix` trait (`compute_violation(grad: &amp;Tensor) -&gt; f32`)
    - Default `catastrophic_forgetting_threshold = 0.0` (config-overridable)
    - Map cos_align from [-1, 1] to [0, 1] via (cos_align + 1.0)/2.0 before weighted sum (per TECH-UTML behavior step 7)
    - Catastrophic forgetting: emit `UtmlCatastrophicForgetting` error AND return Ok(0.0) per AC-UTML-03-02 (the function may return either Err OR Ok(0.0) — TECH-UTML behavior step 4 says "return Ok(0.0) signaling step abort"; choose Ok(0.0) AND set the cert.skip_reason=CatastrophicForgetting at the upstream caller via a side-channel — but for THIS function, return Err so upstream sets skip_reason)
    - Decision: return `Err(UtmlError{code: UtmlCatastrophicForgetting})` so upstream (TASK-UTML-007 / Trainer) treats it as fail-closed; trainer captures error and sets cert.skip_reason. Document this clearly in source.
    - Validate ECE in [0, 1] AND embedder_coherence in [0, 1]; UtmlFactorOutOfRange otherwise
    - Validate finite(grad) AND finite(ema_grad); MejepaObserveBatchNan on NaN/Inf
    - Output clamped to [0, 1]
    - Unit tests: weight-sum=1.0; cos_align=1 + low fisher → high ΔK; cos_align=-0.1 → CatastrophicForgetting err; ECE rising → ΔK shrinks; embedder_coherence=0.5 (bootstrap) verified
  </in_scope>
  <out_of_scope>
    - Full Fisher matrix construction (Phase 5)
    - ECE computation (Phase 4 conformal calibration)
    - embedder_coherence implementation (TASK-UTML-008)
    - L_step assembly (TASK-UTML-007)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-train/src/learning_signal/delta_k.rs">
use candle_core::Tensor;
use crate::error::{UtmlError, UtmlErrorCode};

pub const DEFAULT_CATASTROPHIC_FORGETTING_THRESHOLD: f32 = 0.0;
pub const DEFAULT_EMA_DECAY: f32 = 0.99;

pub trait FisherMatrix {
    fn compute_violation(&amp;self, grad: &amp;Tensor) -&gt; Result&lt;f32, UtmlError&gt;;
}

pub fn compute_delta_k(
    grad: &amp;Tensor,
    ema_grad: &amp;Tensor,
    fisher: &amp;dyn FisherMatrix,
    ece: f32,
    embedder_coherence: f32,
    catastrophic_forgetting_threshold: f32, // default DEFAULT_CATASTROPHIC_FORGETTING_THRESHOLD
) -&gt; Result&lt;f32, UtmlError&gt;;

pub fn update_grad_ema(
    ema_grad: &amp;mut Tensor,
    new_grad: &amp;Tensor,
    decay: f32, // default DEFAULT_EMA_DECAY
) -&gt; Result&lt;(), UtmlError&gt;;

#[cfg(test)]
mod tests {
    pub struct StubFisher { pub violation: f32 }
    impl FisherMatrix for StubFisher {
        fn compute_violation(&amp;self, _: &amp;Tensor) -&gt; Result&lt;f32, UtmlError&gt; { Ok(self.violation) }
    }
    /* unit tests using StubFisher */
}
    </signature>
  </signatures>
  <constraints>
    - cos_align computed as cosine_similarity over flattened gradient vectors (1D treatment); range [-1, 1]; NOT clamped
    - if cos_align &lt; threshold → return Err(UtmlError::new(UtmlCatastrophicForgetting, ...).with_context(json!({"cos_align": cos_align})))
    - cos_align_normalized = (cos_align + 1.0) / 2.0 → [0, 1]
    - delta_k_raw = 0.5*cos_align_norm + 0.3*(1-fisher_viol) + 0.15*(1-ece) + 0.05*embedder_coherence
    - delta_k = clamp(delta_k_raw, 0.0, 1.0)
    - update_grad_ema: validate decay ∈ (0, 1); fail UtmlFactorOutOfRange otherwise; in-place ema = decay*ema + (1-decay)*new
    - All input validations fail-closed; no silent fallback
    - File &lt; 350 lines
  </constraints>
  <verification>
    - cargo test shows ≥ 8 tests pass (cos_align=+1, =-0.1 (catastrophic), low ece, fisher_violation=0/1, embedder_coherence=0.5, weight-sum, decay range, EMA update math)
    - Weights 0.5+0.3+0.15+0.05 verified to sum exactly 1.0 in a constant test
  </verification>
</definition_of_done>

<pseudo_code>
fn compute_delta_k(grad, ema_grad, fisher, ece, embedder_coherence, threshold) -&gt; Result&lt;f32, UtmlError&gt; {
    if !(0.0..=1.0).contains(&amp;ece) { return Err(UtmlError::new(UtmlFactorOutOfRange, ...)); }
    if !(0.0..=1.0).contains(&amp;embedder_coherence) { return Err(...); }
    if !is_finite(grad) || !is_finite(ema_grad) { return Err(UtmlError::new(MejepaObserveBatchNan, ...)); }
    let cos_align = cosine_flatten(grad, ema_grad)?;
    if cos_align &lt; threshold {
        return Err(UtmlError::new(UtmlCatastrophicForgetting, format!("cos_align={cos_align}"))
                          .with_context(json!({"cos_align": cos_align})));
    }
    let fv = fisher.compute_violation(grad)?.clamp(0.0, 1.0);
    let cos_norm = (cos_align + 1.0) / 2.0;
    let raw = 0.5*cos_norm + 0.3*(1.0 - fv) + 0.15*(1.0 - ece) + 0.05*embedder_coherence;
    Ok(raw.clamp(0.0, 1.0))
}

fn update_grad_ema(ema_grad, new_grad, decay) -&gt; Result&lt;(), UtmlError&gt; {
    if !(0.0..1.0).contains(&amp;decay) || decay == 0.0 { return Err(...); }
    if !is_finite(new_grad) { return Err(...); }
    *ema_grad = (&amp;*ema_grad * decay)? + (new_grad * (1.0 - decay))?;
    Ok(())
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/delta_k.rs">compute_delta_k + update_grad_ema + FisherMatrix trait + tests with StubFisher</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/mod.rs">pub mod delta_k; pub use delta_k::*;</file>
</files_to_modify>

<validation_criteria>
  <criterion>Weight sum 0.5+0.3+0.15+0.05 == 1.0 verified in unit test</criterion>
  <criterion>Catastrophic forgetting (cos_align &lt; 0) returns UtmlCatastrophicForgetting error</criterion>
  <criterion>NaN/Inf in grad triggers MejepaObserveBatchNan</criterion>
  <criterion>ECE/embedder_coherence out of [0,1] triggers UtmlFactorOutOfRange</criterion>
  <criterion>EMA update is in-place and produces expected output for known input</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-train</command>
  <command>cargo test -p context-graph-mejepa-train --lib delta_k</command>
  <command>cargo clippy -p context-graph-mejepa-train -- -D warnings</command>
</test_commands>

</task_spec>
```

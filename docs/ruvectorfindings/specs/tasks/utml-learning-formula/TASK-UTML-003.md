# TASK-UTML-003 — `compute_delta_p` + per-chunk aggregation

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

## 📋 2026-05-10 STATE AUDIT — TASK PARTIALLY SUPERSEDED

**Status: PARTIALLY SUPERSEDED.** Host crate `context-graph-mejepa-train` exists.

**Shipped (grep evidence):**

- `compute_delta_p(c: DeltaPComponents) -> Result<f32, UtmlError>` — `src/learning_signal.rs` L159; current implementation delegates to `mean_checked` over the four fields (`predicted`/`target`/`snr`/`exploration_bonus`-derived). **NOTE the semantic drift:** the task title specifies arguments `(predicted, target, snr, exploration_bonus, gamma, imagined)` returning a weighted score with γ-blending of imagined-step bonus; the shipped function is the simpler mean-of-components form per `DeltaPComponents` struct (L69).

**NOT FOUND in `crates/context-graph-mejepa-train/src/` (pending):**

- `compute_delta_p_aggregate` (per-chunk Mean | Max | TopKMean | EntityWeighted reducer). No `pub fn compute_delta_p_aggregate` in `learning_signal.rs`.
- `DeltaPAggregator` enum (see TASK-UTML-002).

**Direction for future agents:** AUDIT the shipped `compute_delta_p` body at L159 against this task's `<signatures>` block — confirm whether the spec was relaxed (mean-of-fields) or the shipped form is incomplete. Either update the task to match the simpler shipped form (and document why) OR extend the function to accept `(predicted, target, snr, exploration_bonus, gamma, imagined)` with `DeltaPComponents` as the carrier. Then ship the missing `compute_delta_p_aggregate` + `DeltaPAggregator`.

---

```xml
<task_spec id="TASK-UTML-003" version="1.0">

<metadata>
  <title>compute_delta_p (predicted, target, snr, exploration_bonus, gamma, imagined) + compute_delta_p_aggregate (per-chunk Mean | Max | TopKMean | EntityWeighted) returning f32 in [0, 1]</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>3</sequence>
  <implements>
    <item ref="REQ-UTML-02">JEPA latent prediction error + SNR norm + exploration bonus + imagination γ</item>
    <item ref="AC-UTML-02-01">clamp_[0,1](max(ΔP_real, γ·ΔP_imagined) + 0.5·exploration_bonus) / SNR(t)</item>
    <item ref="AC-UTML-02-02">Per-chunk aggregation strategies</item>
    <item ref="AC-UTML-02-03">ΔP_real==0 → L_step=0 (perfect prediction)</item>
    <item ref="AC-UTML-02-04">SNR floor enforced; fail-closed on below-floor</item>
    <item ref="EC-UTML-12">Aggregator switches Mean→Max when Gτ flagged a violating chunk</item>
    <item ref="EC-UTML-15">SNR below floor → structured error</item>
    <item ref="component:DeltaP">crates/context-graph-mejepa-train/src/learning_signal/delta_p.rs</item>
  </implements>
  <depends_on>
    <task_ref>TASK-UTML-001</task_ref>
    <task_ref>TASK-UTML-002</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created</estimated_files>
</metadata>

<context>
ΔP measures latent prediction error between the predictor's output `g_φ(f_θ(panel_t01))` and the frozen target `f_target(panel_t2)`. Higher ΔP = more learning signal. `1 - cosine(predicted, target)` is the base; SNR normalization (Liu 2020) shrinks ΔP under uninformative gradients; +0.5 active-learning bonus surfaces wide-conformal-interval examples; γ ≈ 0.7 imagination discount per Dreamer. Per-chunk aggregation per doc 14 AST-aware chunking; Max aggregator activates when Phase 4b Gτ flagged a violating chunk.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (utml_v1_invariants rule 1; failure_taxonomy: UTML_FACTOR_OUT_OF_RANGE, UTML_SNR_BELOW_FLOOR, MEJEPA_OBSERVE_BATCH_NAN)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-UTML.md (US-UTML-02; AC-UTML-02-01..04; EC-UTML-12, EC-UTML-15; ERR-UTML-06)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-UTML.md (component DeltaP — compute_delta_p + compute_delta_p_aggregate behaviors)</file>
  <file purpose="design_doc">docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §2-3</file>
  <file purpose="prior_tasks">docs/ruvectorfindings/specs/tasks/utml-learning-formula/TASK-UTML-001.md, TASK-UTML-002.md</file>
  <file purpose="instrument_panel_dim">crates/context-graph-mejepa-instruments/src/lib.rs (PANEL_DIM=5120)</file>
</input_context_files>

<prerequisites>
  <check>TASK-UTML-001 complete (UtmlError + DeltaPComponents shipped)</check>
  <check>TASK-UTML-002 complete (DeltaPAggregator shipped)</check>
  <check>candle-core 0.9.x in workspace deps for Tensor type</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `src/learning_signal/delta_p.rs` with `compute_delta_p` and `compute_delta_p_aggregate` per TECH-UTML
    - Add candle-core dep to `crates/context-graph-mejepa-train/Cargo.toml` (already in workspace)
    - Validation gates per behavior table (dim mismatch, NaN/Inf, SNR floor, gamma range, exploration bonus ∈ {0.0, 0.5})
    - Cosine similarity batch-mean computation
    - Configurable SNR floor (default 0.05) via `DeltaPConfig` struct (or fn arg)
    - Unit tests: perfect prediction, total miss, high-noise (low SNR), exploration-bonus active, γ scaling, dim mismatch, NaN/Inf, SNR below floor
    - Per-chunk aggregator unit tests for Mean/Max/TopKMean/EntityWeighted
  </in_scope>
  <out_of_scope>
    - SNR computation itself (TASK-UTML-005's plasticity helper or upstream caller)
    - Embedder coherence (TASK-UTML-008)
    - Predictor architecture (TECH-PHASE2-PREDICTOR domain)
    - L_step assembly (TASK-UTML-007)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-train/src/learning_signal/delta_p.rs">
use candle_core::Tensor;
use crate::error::{UtmlError, UtmlErrorCode};
use crate::learning_signal::aggregator::DeltaPAggregator;

pub const DEFAULT_SNR_FLOOR: f32 = 0.05;
pub const DEFAULT_GAMMA: f32 = 0.7;

pub fn compute_delta_p(
    predicted: &amp;Tensor,
    target: &amp;Tensor,
    snr: f32,
    exploration_bonus: f32,
    gamma: f32,
    imagined: Option&lt;&amp;Tensor&gt;,
    snr_floor: f32, // default DEFAULT_SNR_FLOOR
) -&gt; Result&lt;f32, UtmlError&gt;;

pub fn compute_delta_p_aggregate(
    per_chunk: &amp;[f32],
    aggregator: DeltaPAggregator,
    entity_weights: Option&lt;&amp;[f32]&gt;,
) -&gt; Result&lt;f32, UtmlError&gt;;

// Helper: select Max when Gτ flagged a violating chunk per EC-UTML-12
pub fn select_aggregator(gtau_violation: bool) -&gt; DeltaPAggregator;
    </signature>
  </signatures>
  <constraints>
    - Validate `predicted.dims() == target.dims()`; UtmlFactorOutOfRange on mismatch
    - Validate finite(predicted) AND finite(target); MejepaObserveBatchNan on NaN/Inf
    - Validate snr &gt;= snr_floor; UtmlSnrBelowFloor otherwise
    - Validate gamma in [0.6, 0.8]; UtmlFactorOutOfRange otherwise
    - Validate exploration_bonus ∈ {0.0, 0.5}; UtmlFactorOutOfRange otherwise
    - cos = batch-mean cosine similarity
    - delta_p_real = clamp(1.0 - cos, 0.0, 1.0)
    - delta_p_imagined: if Some, clamp(1.0 - cos_imagined_target, 0.0, 1.0); else 0.0
    - base = max(delta_p_real, gamma * delta_p_imagined)
    - raw = base + 0.5 * exploration_bonus
    - delta_p = clamp(raw / snr, 0.0, 1.0)
    - For aggregator: validate per_chunk non-empty AND all in [0, 1]
    - EntityWeighted requires entity_weights.len() == per_chunk.len(); compute Σ(w_i·v_i)/Σ(w_i)
    - All output clamped to [0, 1] post-compute
    - File &lt; 350 lines
    - No `unwrap_or` on confidence; no silent fallbacks
  </constraints>
  <verification>
    - `cargo test -p context-graph-mejepa-train --lib delta_p` shows ≥ 12 tests pass
    - Property-test: 1,000 random (predicted, target, snr) inputs with `proptest` → output always in [0, 1] when inputs valid
    - Mean/Max/TopKMean/EntityWeighted aggregator round-trip correctness
    - `cargo clippy -p context-graph-mejepa-train -- -D warnings` clean
  </verification>
</definition_of_done>

<pseudo_code>
fn compute_delta_p(predicted, target, snr, exploration_bonus, gamma, imagined, snr_floor) -&gt; Result&lt;f32, UtmlError&gt; {
    // 1. Shape validation
    if predicted.dims() != target.dims() {
        return Err(UtmlError::new(UtmlFactorOutOfRange, format!("dim mismatch: {:?} vs {:?}", ...)));
    }
    // 2. NaN/Inf checks via Tensor::abs().sum() finite check
    if !is_finite(predicted) || !is_finite(target) {
        return Err(UtmlError::new(MejepaObserveBatchNan, ...));
    }
    // 3. SNR floor
    if snr < snr_floor {
        return Err(UtmlError::new(UtmlSnrBelowFloor, format!("snr={} < floor={}", snr, snr_floor)));
    }
    // 4. gamma + exploration_bonus range checks
    if !(gamma &gt;= 0.6 &amp;&amp; gamma &lt;= 0.8) { return Err(...); }
    if !(exploration_bonus == 0.0 || exploration_bonus == 0.5) { return Err(...); }
    // 5. cosine
    let cos_pt = batch_mean_cosine(predicted, target)?;
    let dp_real = (1.0 - cos_pt).clamp(0.0, 1.0);
    let dp_imag = match imagined {
        Some(im) => (1.0 - batch_mean_cosine(im, target)?).clamp(0.0, 1.0),
        None => 0.0,
    };
    let base = dp_real.max(gamma * dp_imag);
    let raw = base + 0.5 * exploration_bonus;
    let dp = (raw / snr).clamp(0.0, 1.0);
    Ok(dp)
}

fn compute_delta_p_aggregate(per_chunk, aggregator, entity_weights) -&gt; Result&lt;f32, UtmlError&gt; {
    if per_chunk.is_empty() { return Err(...); }
    for v in per_chunk { if !(0.0..=1.0).contains(v) { return Err(...); } }
    let agg = match aggregator {
        Mean => per_chunk.iter().sum::&lt;f32&gt;() / per_chunk.len() as f32,
        Max => per_chunk.iter().copied().fold(0.0, f32::max),
        TopKMean { k } => { let mut v = per_chunk.to_vec(); v.sort_by(|a,b| b.partial_cmp(a).unwrap()); v.iter().take(k).sum::&lt;f32&gt;() / k.min(v.len()) as f32 },
        EntityWeighted => {
            let w = entity_weights.ok_or(...)?;
            if w.len() != per_chunk.len() { return Err(...); }
            let sw: f32 = w.iter().sum();
            if sw &lt;= 0.0 { return Err(...); }
            per_chunk.iter().zip(w).map(|(v,wi)| v*wi).sum::&lt;f32&gt;() / sw
        }
    };
    Ok(agg.clamp(0.0, 1.0))
}

fn select_aggregator(gtau_violation: bool) -&gt; DeltaPAggregator {
    if gtau_violation { Mean::Max } else { DeltaPAggregator::Mean }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/delta_p.rs">compute_delta_p + compute_delta_p_aggregate + select_aggregator + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/mod.rs">pub mod delta_p; pub use delta_p::*;</file>
  <file path="crates/context-graph-mejepa-train/Cargo.toml">add candle-core 0.9 dep + proptest dev-dep if missing</file>
</files_to_modify>

<validation_criteria>
  <criterion>All 6 input validations fail-closed with structured UtmlError + correct .code() tag</criterion>
  <criterion>Perfect prediction (cos=1.0) → ΔP=0</criterion>
  <criterion>Total miss (cos=-1.0) → ΔP=1 (after clamp)</criterion>
  <criterion>SNR halving doubles ΔP up to clamp ceiling</criterion>
  <criterion>Aggregator EntityWeighted with mismatched len fails-closed</criterion>
  <criterion>select_aggregator(true) returns Max; select_aggregator(false) returns Mean</criterion>
  <criterion>cargo build/test/clippy clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-train</command>
  <command>cargo test -p context-graph-mejepa-train --lib delta_p</command>
  <command>cargo clippy -p context-graph-mejepa-train -- -D warnings</command>
</test_commands>

</task_spec>
```

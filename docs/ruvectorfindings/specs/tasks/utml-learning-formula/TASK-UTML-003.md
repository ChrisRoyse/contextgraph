# TASK-UTML-003 — `compute_delta_p` + per-chunk aggregation

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

# TASK-TCT-007 — ConstellationHingeOp: L_constellation_centroid loss term (frozen embedder + centroid; gradient only through predictor)

```xml
<task_spec id="TASK-TCT-007" version="1.0">

<metadata>
  <title>ConstellationHingeOp in `crates/context-graph-mejepa-tct/src/hinge.rs`</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>7</sequence>
  <implements>
    <item ref="REQ-TCT-07">constellation_hinge primitive: Σ_m max(0, τ_m − cos(g_φ(panel)_m, c_m))</item>
    <item ref="AC-TCT-05-01">21-embedder hinge sum; centroid + embedder DETACHED from autograd</item>
    <item ref="AC-TCT-05-02">cos == τ_m boundary → exact 0 contribution</item>
    <item ref="BR-TCT-09">Backward pass gradient ONLY through predictor</item>
    <item ref="constitution:tct_invariants">rule 1: same 21 frozen embedders serve THREE roles without parameter update</item>
    <item ref="constitution:anti_patterns">No learnable target encoder</item>
  </implements>
  <depends_on>
    <task_ref>TASK-TCT-003</task_ref>
    <task_ref>TASK-TCT-004</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (hinge.rs)</estimated_files>
</metadata>

<context>
The L_constellation_centroid loss term is consumed by SPEC-PHASE3-TRAINING. This task ships the
primitive: `Σ_m max(0, τ_m − cos(slot_m, c_m))` over 21 embedders. The centroid + embedder are
DETACHED from autograd — gradient flows ONLY through the predictor's panel slot. Boundary case at
cos == τ_m yields exactly 0 contribution. A non-differentiable `f32` variant is provided here; the
candle differentiable variant (`constellation_hinge_op`) is referenced but its candle-Tensor
implementation is deferred until candle-cuda + Phase 2 predictor land.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (`<tct_invariants>` rule 1; `<anti_patterns>` "No learnable target encoder")</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-TCT.md (REQ-TCT-07; AC-TCT-05-01..02; BR-TCT-09)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-TCT.md (component_contracts ConstellationHingeOp.constellation_hinge + constellation_hinge_op (candle))</file>
  <file purpose="upstream_constellation">crates/context-graph-mejepa-tct/src/constellation.rs (lookup_centroid, threshold; TASK-TCT-001/004)</file>
  <file purpose="upstream_hinge_types">crates/context-graph-mejepa-tct/src/hinge_types.rs (ConstellationHingeOutput; TASK-TCT-002)</file>
  <file purpose="upstream_gtau">crates/context-graph-mejepa-tct/src/gtau.rs (cosine_similarity; TASK-TCT-006)</file>
</input_context_files>

<prerequisites>
  <check>TASK-TCT-003/004 complete; lookup_centroid + threshold available</check>
  <check>TASK-TCT-006 complete; cosine_similarity primitive available</check>
  <check>TASK-TCT-002 complete; ConstellationHingeOutput exists</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa-tct/src/hinge.rs` with:
      - `pub fn constellation_hinge(predicted_panel: &Panel, predicted_class: MutationCategory, language: Language, entity_type: EntityType, constellation: &TctConstellation) -> Result<ConstellationHingeOutput, TctError>` (non-differentiable f32 variant; for inference + tests + cert recording)
      - For each emb m: lookup centroid via lookup_centroid (with shrinkage fallback); compute cos = cosine_similarity(panel.slot_for(m), centroid); compute hinge_m = (τ_m - cos).max(0.0); insert into per_embedder_hinges + centroid_origin
      - Total hinge = sum of 21 entries
      - Document the differentiable candle variant `constellation_hinge_op` as a STUB (returns NotImplemented or feature-gated to `feature = "candle"` until Phase 2 predictor ships); body lives in TECH-PHASE3-TRAINING (training-loop wiring; out of scope here per file_layout.cross_cutting note)
    - Add unit tests with synthetic panels:
      - `constellation_hinge_zero_when_all_above_threshold`: every cos ≥ τ_m → total=0
      - `constellation_hinge_one_embedder_below_threshold_contributes`: only that emb has hinge > 0
      - `constellation_hinge_boundary_cos_equals_tau_yields_exact_zero`: cos == τ_m → hinge = 0.0 exactly (per AC-TCT-05-02; verify via 1-ULP straddle test)
      - `constellation_hinge_shrinkage_origin_recorded_per_embedder`: 21 entries
      - `constellation_hinge_total_equals_sum_of_per_embedder`: invariant verified analytically
      - `constellation_hinge_rejects_no_centroid`: lookup_centroid returns None → InsufficientSamples
      - `constellation_hinge_rejects_nan_panel`: NaN slot → DimMismatch
      - `constellation_hinge_deterministic_across_two_runs`: byte-equal output
      - `constellation_hinge_op_candle_stub_returns_feature_disabled` (when feature off)
    - Document the gradient-flow invariant in module rustdoc: "When the differentiable candle variant ships in TECH-PHASE3-TRAINING, the centroid + embedder weights MUST be created with `requires_grad = false`. Debug builds install a gradient hook asserting |grad| == 0 every backward pass; nonzero gradient is MEJEPA_INSTR_FROZEN_VIOLATION."
  </in_scope>
  <out_of_scope>
    - Differentiable candle Tensor variant (TECH-PHASE3-TRAINING owns the body once candle backend stable; this task ships only the public function signature gated behind `feature = "candle"`)
    - Gradient hook installation (TECH-PHASE3-TRAINING)
    - L_full = α·L_panel + β·L_pairwise + γ·L_action + δ·L_constellation_centroid loss surface integration (TECH-PHASE3-TRAINING)
    - RocksDB persistence (TASK-TCT-008)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-tct/src/hinge.rs">
use crate::types::{MutationCategory, Language, EntityType};
use crate::constellation::TctConstellation;
use crate::hinge_types::ConstellationHingeOutput;
use crate::error::TctError;
use context_graph_mejepa_instruments::Panel;

pub fn constellation_hinge(
    predicted_panel: &amp;Panel,
    predicted_class: MutationCategory,
    language: Language,
    entity_type: EntityType,
    constellation: &amp;TctConstellation,
) -&gt; Result&lt;ConstellationHingeOutput, TctError&gt;;

#[cfg(feature = "candle")]
pub fn constellation_hinge_op(
    predicted_panel: &amp;candle_core::Tensor,
    predicted_class: MutationCategory,
    language: Language,
    entity_type: EntityType,
    constellation: &amp;TctConstellation,
) -&gt; Result&lt;candle_core::Tensor, TctError&gt;;
    </signature>
  </signatures>
  <constraints>
    - Hinge formula: `max(0.0, τ_m - cos)` — boundary cos == τ_m → 0 exactly (not 1-ULP nudge)
    - All 21 EmbedderId iterations populated (full per_embedder_hinges coverage; any missing → DimMismatch)
    - lookup_centroid hierarchy fallback used (OwnCell/Language/Entity/Category)
    - centroid_origin recorded per embedder
    - Determinism: BTreeMap order; cosine_similarity (Task 006) deterministic
    - candle variant feature-gated; without `feature = "candle"`, `constellation_hinge_op` is unavailable (compile-time absent or returns FeatureDisabled error)
    - rustdoc on module documents the gradient-flow invariant (centroid + embedder DETACHED) and references TECH-PHASE3-TRAINING for the loss-surface integration
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa-tct` zero warnings
    - `cargo test -p context-graph-mejepa-tct hinge` shows ≥ 8 passing tests
    - `cargo clippy -p context-graph-mejepa-tct -- -D warnings` clean
    - Boundary test (cos == τ_m → 0.0 exact) passes
    - 21-entry coverage verified
    - Determinism verified
  </verification>
</definition_of_done>

<pseudo_code>
pub fn constellation_hinge(predicted, predicted_class, language, entity_type, constellation)
    -> Result<ConstellationHingeOutput, TctError>
{
    let mut per_embedder_hinges: BTreeMap<EmbedderId, f32> = BTreeMap::new();
    let mut centroid_origin: BTreeMap<EmbedderId, ShrinkageOrigin> = BTreeMap::new();
    for emb in EmbedderId::all() {
        let slot = predicted.slot_for(emb).map_err(|_| TctError::DimMismatch{expected:1, actual:0})?;
        if slot.iter().any(|x| !x.is_finite()) {
            return Err(TctError::DimMismatch { expected: 1, actual: 0 });
        }
        let (centroid, origin) = constellation.lookup_centroid(predicted_class, language, entity_type, emb)
            .ok_or_else(|| TctError::InsufficientSamples {
                cell: format!("({:?}, {:?}, {:?}, {:?})", predicted_class, entity_type, language, emb),
                observed: 0, required: 5 })?;
        let cos = crate::gtau::cosine_similarity(slot, centroid);
        let tau = constellation.threshold(emb, Some(entity_type));
        let hinge = (tau - cos).max(0.0);
        per_embedder_hinges.insert(emb, hinge);
        centroid_origin.insert(emb, origin);
    }
    ConstellationHingeOutput::try_new(per_embedder_hinges, centroid_origin)
}

#[cfg(feature = "candle")]
pub fn constellation_hinge_op(predicted, predicted_class, language, entity_type, constellation)
    -> Result<candle_core::Tensor, TctError>
{
    // STUB: full body ships in TECH-PHASE3-TRAINING. The differentiable variant slices each
    // embedder slot from predicted (as Tensor view, preserving autograd), loads centroid as a
    // detached Tensor (requires_grad = false), computes cos via Tensor::matmul + Tensor::norm,
    // subtracts τ_m, ReLU, sums over 21. Returns scalar Tensor for backprop into predictor only.
    Err(TctError::DimMismatch { expected: 1, actual: 0 }) // Placeholder; see Phase 3
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-tct/src/hinge.rs">constellation_hinge + (feature-gated) constellation_hinge_op + 8 tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-tct/src/lib.rs">add `pub mod hinge;`</file>
  <file path="crates/context-graph-mejepa-tct/Cargo.toml">add optional `[features] candle = ["candle-core"]` and optional `candle-core = { version = "0.9", optional = true }` dep</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa-tct zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa-tct hinge shows ≥ 8 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa-tct -- -D warnings clean</criterion>
  <criterion>Boundary cos == τ_m → hinge = 0.0 exact</criterion>
  <criterion>21-entry coverage in per_embedder_hinges</criterion>
  <criterion>centroid_origin recorded per embedder</criterion>
  <criterion>Determinism verified</criterion>
  <criterion>rustdoc documents centroid + embedder DETACHED invariant</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-tct</command>
  <command>cargo test -p context-graph-mejepa-tct hinge</command>
  <command>cargo clippy -p context-graph-mejepa-tct -- -D warnings</command>
</test_commands>

</task_spec>
```

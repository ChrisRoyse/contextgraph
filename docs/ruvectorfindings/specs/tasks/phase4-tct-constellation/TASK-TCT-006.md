# TASK-TCT-006 — GtauChecker: deterministic O(N · d_max) panel + chunk Gτ test

```xml
<task_spec id="TASK-TCT-006" version="1.0">

<metadata>
  <title>GtauChecker in `crates/context-graph-mejepa-tct/src/gtau.rs`</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>6</sequence>
  <implements>
    <item ref="REQ-TCT-08">gtau_check (panel-level) deterministic O(N · d_max); 21 embedders evaluated; per-violation diagnostic</item>
    <item ref="REQ-TCT-09">gtau_check_chunks (per-chunk localization); chunk-level threshold preferred when present</item>
    <item ref="REQ-TCT-10">Verdict::GuardRejected emission with violating_chunks</item>
    <item ref="constitution:tct_invariants">rule 3: deterministic O(N·d_max); rule 4: per-chunk localization</item>
    <item ref="EC-TCT-04">cos == τ_m boundary case (inclusive ≥)</item>
    <item ref="EC-TCT-05">Unseen mutation_category falls back to category-aggregate</item>
    <item ref="EC-TCT-12">NaN/Inf in panel slot fails-closed</item>
    <item ref="NFR-TCT-01">Gτ latency p50 &lt; 10 ms with cached embedders</item>
  </implements>
  <depends_on>
    <task_ref>TASK-TCT-003</task_ref>
    <task_ref>TASK-TCT-004</task_ref>
    <task_ref>TASK-TCT-005</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>1 created (gtau.rs)</estimated_files>
</metadata>

<context>
The Gτ test is the deterministic geometric guard that runs at every verify() call. For each of 21
embedders m: compute cos(panel.slot_for(m), centroid_m); compare to threshold τ_m; gtau_satisfied
iff `cos ≥ τ_m` for ALL embedders. Runtime O(21 · d_max ≈ 1536) ~60 µs core compute. Determinism
requires fixed-order pairwise summation in cosine_similarity. Per-chunk variant runs the same test
on each chunk and aggregates violating_chunks.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (`<tct_invariants>` rule 3, 4; performance_budgets single_prediction_latency)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-TCT.md (REQ-TCT-08, 09, 10; AC-TCT-03-01..03; ERR-TCT-01, 06; EC-TCT-04, 05, 12)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-TCT.md (component_contracts GtauChecker.gtau_check + gtau_check_chunks + cosine_similarity)</file>
  <file purpose="upstream_constellation">crates/context-graph-mejepa-tct/src/constellation.rs (lookup_centroid, threshold; TASK-TCT-001/004)</file>
  <file purpose="upstream_calibrator">crates/context-graph-mejepa-tct/src/calibrator.rs (Thresholds; TASK-TCT-005)</file>
  <file purpose="instruments_panel">crates/context-graph-mejepa-instruments/src/lib.rs (Panel.slot_for)</file>
</input_context_files>

<prerequisites>
  <check>TASK-TCT-003/004/005 complete; TctConstellation has lookup_centroid + threshold + per_chunk_type_thresholds</check>
  <check>Panel exposes `.slot_for(EmbedderId) -> Result<&[f32], _>` and `.is_finite() -> bool`</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa-tct/src/gtau.rs` with:
      - `pub fn gtau_check(predicted_panel: &Panel, predicted_class: MutationCategory, language: Language, entity_type: EntityType, constellation: &TctConstellation) -> Result<GtauOutput, TctError>` per TECH-TCT behavior
      - `pub fn gtau_check_chunks(chunk_panels: &[(ChunkId, Panel)], predicted_class: MutationCategory, constellation: &TctConstellation) -> Result<ChunkGtauOutput, TctError>` per TECH-TCT behavior; per-chunk threshold preferred when present
      - `pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32` deterministic pairwise summation; zero-norm returns 0.0
    - Add `threshold(embedder, entity_type)` accessor on `TctConstellation` (per `TECH-TCT.threshold` signature) returning per-chunk-type τ if present else panel-level
    - Add unit tests with synthetic panels:
      - `gtau_check_accept_when_all_cos_above_threshold`
      - `gtau_check_reject_when_one_embedder_below_threshold`: violating_embedders has exactly that one
      - `gtau_check_records_centroid_origin_per_embedder`: per emb a ShrinkageOrigin entry
      - `gtau_check_inclusive_at_boundary_cos_equals_tau`: cos == τ_m → accept (per EC-TCT-04)
      - `gtau_check_rejects_nan_panel`: NaN in slot → DimMismatch (per EC-TCT-12)
      - `gtau_check_unseen_mutation_falls_back_to_category`: panel for unseen mutation → uses category-aggregate via lookup_centroid
      - `gtau_check_deterministic_across_two_runs`: same panel + constellation → byte-equal output
      - `gtau_check_chunks_aggregate_satisfied_iff_all_pass`: 3 chunks all pass → aggregate=true; one fails → aggregate=false; violating_chunks contains failing chunk
      - `gtau_check_chunks_uses_per_chunk_threshold_when_present`: synthetic constellation with sharper per-chunk-type τ → chunks evaluated against sharper τ
      - `gtau_check_chunks_falls_back_to_panel_threshold_when_chunk_type_absent`
      - `cosine_similarity_zero_norm_returns_zero` (not NaN)
      - `cosine_similarity_orthogonal_vectors_returns_zero`
      - `cosine_similarity_identical_vectors_returns_one`
      - `gtau_check_latency_under_10ms_p50_synthetic`: 100-call latency profiling on synthetic panels (best-effort; informational rather than CI-gated)
  </in_scope>
  <out_of_scope>
    - Real predictor — uses synthetic panels
    - Hinge primitive (TASK-TCT-007)
    - RocksDB persistence (TASK-TCT-008)
    - SIMD optimization beyond naive deterministic pairwise summation
    - Sub-percentile threshold sweep — TASK-TCT-005
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-tct/src/gtau.rs">
use crate::types::{ChunkId, MutationCategory, Language, EntityType, EmbedderId, GtauOutput, GtauViolation};
use crate::hinge_types::ChunkGtauOutput;
use crate::constellation::TctConstellation;
use crate::error::TctError;
use context_graph_mejepa_instruments::Panel;

pub fn gtau_check(
    predicted_panel: &amp;Panel,
    predicted_class: MutationCategory,
    language: Language,
    entity_type: EntityType,
    constellation: &amp;TctConstellation,
) -&gt; Result&lt;GtauOutput, TctError&gt;;

pub fn gtau_check_chunks(
    chunk_panels: &amp;[(ChunkId, Panel)],
    predicted_class: MutationCategory,
    constellation: &amp;TctConstellation,
) -&gt; Result&lt;ChunkGtauOutput, TctError&gt;;

pub fn cosine_similarity(a: &amp;[f32], b: &amp;[f32]) -&gt; f32;
    </signature>
    <signature file="crates/context-graph-mejepa-tct/src/constellation.rs">
impl TctConstellation {
    pub fn threshold(&amp;self, embedder: EmbedderId, entity_type: Option&lt;EntityType&gt;) -&gt; f32;
}
    </signature>
  </signatures>
  <constraints>
    - Cosine `>= τ_m` (inclusive) — boundary case `cos == τ_m` accepts per EC-TCT-04
    - All 21 embedders evaluated even if early-exit could save cost (preserves full diagnostic completeness)
    - Determinism: pairwise summation in fixed loop order; BTreeMap iteration order
    - Cosine zero-norm (||a|| < 1e-12 or ||b|| < 1e-12) returns 0.0 (not NaN)
    - NaN/Inf in panel → DimMismatch
    - centroid lookup falls back via lookup_centroid hierarchy (Task 004); NoCentroid → InsufficientSamples
    - violating_chunks: each entry is the ChunkId that failed; per-chunk thresholds applied when (entity_type, embedder) ∈ per_chunk_type_thresholds
    - elapsed_ms recorded via `Instant::now()` for FSV
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa-tct` zero warnings
    - `cargo test -p context-graph-mejepa-tct gtau` shows ≥ 13 passing tests
    - `cargo clippy -p context-graph-mejepa-tct -- -D warnings` clean
    - All boundary tests pass
    - Determinism test passes (byte-equal across runs)
    - Per-chunk threshold preference verified
    - p50 latency informational note: typically < 10 ms on synthetic panels
  </verification>
</definition_of_done>

<pseudo_code>
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    let mut dot = 0.0_f64;
    let mut na  = 0.0_f64;
    let mut nb  = 0.0_f64;
    for i in 0..a.len() {
        dot += a[i] as f64 * b[i] as f64;
        na  += a[i] as f64 * a[i] as f64;
        nb  += b[i] as f64 * b[i] as f64;
    }
    let denom = na.sqrt() * nb.sqrt();
    if denom < 1e-12 { 0.0 } else { (dot / denom) as f32 }
}

pub fn gtau_check(predicted, predicted_class, language, entity_type, constellation) -> Result<GtauOutput, TctError> {
    let t0 = std::time::Instant::now();
    let mut violations: Vec<GtauViolation> = Vec::new();
    let mut centroid_origin: BTreeMap<EmbedderId, ShrinkageOrigin> = BTreeMap::new();

    for emb in EmbedderId::all() {
        let slot = predicted.slot_for(emb).map_err(|_| TctError::DimMismatch{expected:1, actual:0})?;
        if slot.iter().any(|x| !x.is_finite()) {
            return Err(TctError::DimMismatch { expected: 1, actual: 0 });
        }
        let (centroid, origin) = constellation.lookup_centroid(predicted_class, language, entity_type, emb)
            .ok_or_else(|| TctError::InsufficientSamples { cell: format!("({:?}, {:?}, {:?}, {:?})", predicted_class, entity_type, language, emb), observed: 0, required: 5 })?;
        let cos = cosine_similarity(slot, centroid);
        let tau = constellation.threshold(emb, Some(entity_type));
        centroid_origin.insert(emb, origin);
        if cos < tau {
            violations.push(GtauViolation::try_new(emb, cos, tau, None)?);
        }
    }
    let elapsed_ms = t0.elapsed().as_secs_f32() * 1000.0;
    let satisfied = violations.is_empty();
    Ok(GtauOutput { gtau_satisfied: satisfied, violations, centroid_origin, elapsed_ms })
}

pub fn gtau_check_chunks(chunk_panels, predicted_class, constellation) -> Result<ChunkGtauOutput, TctError> {
    let mut per_chunk_results = Vec::with_capacity(chunk_panels.len());
    let mut violating_chunks = Vec::new();
    for (chunk_id, panel) in chunk_panels {
        let mut output = gtau_check(panel, predicted_class, chunk_id.language, chunk_id.entity_type, constellation)?;
        // Patch each violation with chunk_id for localization
        for v in &mut output.violations {
            v.violating_chunk = Some(chunk_id.clone());
        }
        if !output.gtau_satisfied {
            violating_chunks.push(chunk_id.clone());
        }
        per_chunk_results.push((chunk_id.clone(), output));
    }
    let aggregate_satisfied = violating_chunks.is_empty();
    Ok(ChunkGtauOutput { per_chunk_results, violating_chunks, aggregate_satisfied })
}

impl TctConstellation {
    pub fn threshold(&self, embedder: EmbedderId, entity_type: Option<EntityType>) -> f32 {
        if let Some(et) = entity_type {
            if let Some(t) = self.per_chunk_type_thresholds.get(&(et, embedder)) {
                return *t;
            }
        }
        *self.thresholds.get(&embedder).unwrap_or(&-1.0_f32)  // permissive fallback
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-tct/src/gtau.rs">gtau_check + gtau_check_chunks + cosine_similarity + 13 tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-tct/src/constellation.rs">add `threshold(embedder, entity_type)` method</file>
  <file path="crates/context-graph-mejepa-tct/src/lib.rs">add `pub mod gtau;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa-tct zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa-tct gtau shows ≥ 13 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa-tct -- -D warnings clean</criterion>
  <criterion>cos == τ_m accepts (inclusive)</criterion>
  <criterion>NaN panel → DimMismatch</criterion>
  <criterion>Determinism: byte-equal across two runs</criterion>
  <criterion>Per-chunk threshold preferred when present</criterion>
  <criterion>cosine_similarity zero-norm returns 0.0 not NaN</criterion>
  <criterion>21 embedders always evaluated</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-tct</command>
  <command>cargo test -p context-graph-mejepa-tct gtau</command>
  <command>cargo clippy -p context-graph-mejepa-tct -- -D warnings</command>
</test_commands>

</task_spec>
```

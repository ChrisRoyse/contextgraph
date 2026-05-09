# TASK-TCT-004 — Per-chunk-type centroids (24,024 cells) + Bayesian shrinkage (N<50 → aggregate; N<5 → fail-closed)

```xml
<task_spec id="TASK-TCT-004" version="1.0">

<metadata>
  <title>Per-chunk-type shrinkage in `crates/context-graph-mejepa-tct/src/shrinkage.rs` (extends ShrinkageOrigin from 002) + `builder.rs::finalize`</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>4</sequence>
  <implements>
    <item ref="REQ-TCT-03">N&lt;50 → collapse to language-aggregate then entity-aggregate then category-aggregate; N&lt;5 → fail-closed MEJEPA_TCT_INSUFFICIENT_SAMPLES</item>
    <item ref="REQ-TCT-06">lookup_centroid with shrinkage fallback (placed on TctConstellation; consumed by hinge + Gτ)</item>
    <item ref="constitution:tct_invariants">rule 5: 8 × 13 × 11 × 21 = 24,024 cells with Bayesian shrinkage</item>
    <item ref="EC-TCT-01">N=50 boundary: keeps own centroid (strict `&lt;` 50)</item>
    <item ref="EC-TCT-02">N=4 fails closed</item>
    <item ref="BR-TCT-02">Shrinkage hierarchy ordering: language → entity → category</item>
  </implements>
  <depends_on>
    <task_ref>TASK-TCT-003</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>1 created (shrinkage_engine.rs); 2 modified (builder.rs, constellation.rs)</estimated_files>
</metadata>

<context>
This task implements Bayesian shrinkage on the 24,024 per-(mutation × entity × language × embedder)
cells. Cells with N≥50 keep their own centroid. Cells with 5≤N<50 fall back via the strict order:
language-aggregate → entity-aggregate → category-aggregate. Cells with N<5 fail-closed regardless
of aggregate availability. The shrinkage decision per cell is recorded for FSV evidence + future
provenance (per AC-TCT-01-03). Also lands `lookup_centroid()` method on TctConstellation as
specified in REQ-TCT-06.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (`<tct_invariants>` rule 5; doc 13 §10 risk #5 mitigation)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-TCT.md (REQ-TCT-03/06; AC-TCT-01-03; BR-TCT-02; EC-TCT-01/02)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-TCT.md (component_contracts ConstellationBuilder.finalize step 3; TctConstellation.lookup_centroid behavior)</file>
  <file purpose="upstream_builder">crates/context-graph-mejepa-tct/src/builder.rs (TASK-TCT-003 — PanelLevelCentroids + per_chunk_type_buckets_passthrough)</file>
  <file purpose="upstream_shrinkage_origin">crates/context-graph-mejepa-tct/src/shrinkage.rs (TASK-TCT-002 — ShrinkageOrigin enum)</file>
</input_context_files>

<prerequisites>
  <check>TASK-TCT-003 complete; PanelLevelCentroids carries per_chunk_type_buckets_passthrough</check>
  <check>TASK-TCT-002 complete; ShrinkageOrigin enum exists</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa-tct/src/shrinkage_engine.rs` with:
      - `pub const SHRINKAGE_FULL_THRESHOLD: usize = 50;` (cells N&lt;50 → aggregate fallback)
      - `pub const SHRINKAGE_HARD_FLOOR: usize = 5;` (cells N&lt;5 → fail-closed)
      - `pub struct ShrinkageDecision { pub cell: (MutationCategory, EntityType, Language, EmbedderId), pub origin: ShrinkageOrigin, pub observed_n: usize }`
      - `pub fn apply_shrinkage(per_chunk_type_buckets: &BTreeMap<(MutationCategory, EntityType, Language, EmbedderId), CentroidBucket>, panel_level: &PanelLevelCentroids) -> Result<(BTreeMap<(MutationCategory, EntityType, Language, EmbedderId), Vec<f32>>, Vec<ShrinkageDecision>), TctError>` returning the final per-chunk-type centroid map + per-cell shrinkage decisions
    - Apply algorithm: for each cell:
      1. If `bucket.count >= 50`: compute centroid normally; origin = OwnCell
      2. Else if `bucket.count >= 5`: try `panel_level.per_language_centroids[(language, mutation)][embedder]` (origin = LanguageAggregate); else build entity-aggregate centroid by iterating per_chunk_type buckets matching (mutation, entity_type, *, embedder) where TOTAL count ≥ 50 (origin = EntityAggregate); else use `panel_level.per_category_centroids[mutation][embedder]` (origin = CategoryAggregate)
      3. Else (`count < 5`): return `Err(TctError::InsufficientSamples { cell: format!("{cell:?}"), observed: count, required: 5 })`
    - Extend `ConstellationBuilder` (in `builder.rs`) with `pub fn finalize(self) -> Result<(TctConstellation, Vec<ShrinkageDecision>), TctError>`:
      - Calls `finalize_panel_level()` first
      - Calls `apply_shrinkage(...)` for per-chunk-type cells
      - Calls `ThresholdCalibrator::calibrate(...)` (Task 005 — pass-through for now via dependency injection: `finalize` accepts `thresholds: BTreeMap<EmbedderId, f32>` + `per_chunk_type_thresholds: BTreeMap<(EntityType, EmbedderId), f32>` from caller; TASK-TCT-005 produces them)
      - Constructs final `TctConstellation::try_new(...)`
      - Returns (constellation, shrinkage_decisions)
    - Add `pub fn lookup_centroid(&self, mutation, language, entity_type, embedder) -> Option<(&[f32], ShrinkageOrigin)>` to `impl TctConstellation`:
      1. Try `self.per_chunk_type_centroids[(mutation, entity_type, language, embedder)]` → OwnCell
      2. Try `self.per_language_centroids[(language, mutation)][embedder]` → LanguageAggregate
      3. Try entity-aggregate fallback (search across languages for matching entity_type) → EntityAggregate
      4. Try `self.per_category_centroids[mutation][embedder]` → CategoryAggregate
      5. None
    - Add unit tests:
      - `apply_shrinkage_n50_keeps_own_centroid`: bucket count=50 → OwnCell; norm 1.0
      - `apply_shrinkage_n49_falls_to_language_aggregate`: count=49, language-aggregate available with N≥50 → LanguageAggregate
      - `apply_shrinkage_n10_no_language_falls_to_entity`: count=10, language-aggregate also short → EntityAggregate
      - `apply_shrinkage_n5_no_aggregate_falls_to_category`: count=5, only category aggregate available → CategoryAggregate
      - `apply_shrinkage_n4_fails_closed`: count=4 → InsufficientSamples
      - `apply_shrinkage_records_origin_per_cell`: 100 cells with mixed counts → 100 ShrinkageDecision entries
      - `lookup_centroid_returns_own_cell_when_present`: synthetic constellation with cell populated → OwnCell
      - `lookup_centroid_falls_through_hierarchy`: cell missing, language present → LanguageAggregate; entity present → EntityAggregate; category present → CategoryAggregate; all missing → None
      - `lookup_centroid_zero_copy_borrows_constellation`: returned slice lifetime tied to &self
      - `finalize_returns_constellation_and_decisions`: synthetic builder → (TctConstellation, Vec<ShrinkageDecision>)
  </in_scope>
  <out_of_scope>
    - Threshold calibration (TASK-TCT-005)
    - Gτ check (TASK-TCT-006)
    - Hinge primitive (TASK-TCT-007)
    - RocksDB persistence (TASK-TCT-008)
    - L_constellation_centroid loss surface integration (TECH-PHASE3-TRAINING — out of phase scope)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-tct/src/shrinkage_engine.rs">
use std::collections::BTreeMap;
use crate::types::{MutationCategory, EntityType, Language, EmbedderId};
use crate::shrinkage::ShrinkageOrigin;
use crate::builder::{CentroidBucket, PanelLevelCentroids};
use crate::error::TctError;

pub const SHRINKAGE_FULL_THRESHOLD: usize = 50;
pub const SHRINKAGE_HARD_FLOOR: usize = 5;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ShrinkageDecision {
    pub cell: (MutationCategory, EntityType, Language, EmbedderId),
    pub origin: ShrinkageOrigin,
    pub observed_n: usize,
}

pub fn apply_shrinkage(
    per_chunk_type_buckets:
        &amp;BTreeMap&lt;(MutationCategory, EntityType, Language, EmbedderId), CentroidBucket&gt;,
    panel_level: &amp;PanelLevelCentroids,
) -&gt; Result&lt;
    (BTreeMap&lt;(MutationCategory, EntityType, Language, EmbedderId), Vec&lt;f32&gt;&gt;,
     Vec&lt;ShrinkageDecision&gt;),
    TctError,
&gt;;
    </signature>
    <signature file="crates/context-graph-mejepa-tct/src/constellation.rs">
impl TctConstellation {
    pub fn lookup_centroid(
        &amp;self,
        mutation: MutationCategory,
        language: Language,
        entity_type: EntityType,
        embedder: EmbedderId,
    ) -&gt; Option&lt;(&amp;[f32], ShrinkageOrigin)&gt;;
}
    </signature>
    <signature file="crates/context-graph-mejepa-tct/src/builder.rs">
impl ConstellationBuilder {
    pub fn finalize(
        self,
        thresholds: BTreeMap&lt;EmbedderId, f32&gt;,
        per_chunk_type_thresholds: BTreeMap&lt;(EntityType, EmbedderId), f32&gt;,
    ) -&gt; Result&lt;(crate::constellation::TctConstellation, Vec&lt;crate::shrinkage_engine::ShrinkageDecision&gt;), TctError&gt;;
}
    </signature>
  </signatures>
  <constraints>
    - Strict `&lt;` SHRINKAGE_FULL_THRESHOLD (50): boundary case N=50 keeps OwnCell
    - Strict `&lt;` SHRINKAGE_HARD_FLOOR (5): boundary case N=5 still attempts aggregate fallback
    - Hierarchy order: language → entity → category (deterministic per BR-TCT-02)
    - Entity-aggregate computed by averaging per_chunk_type_buckets matching (mutation, entity_type, *, embedder) where SUM of counts ≥ SHRINKAGE_FULL_THRESHOLD (50)
    - All shrunken centroids re-L2-normalized after aggregate
    - ShrinkageDecision records observed_n (the per-cell count, not the aggregate's count)
    - Determinism: BTreeMap iteration; centroid selection stable across runs
    - lookup_centroid returns zero-copy slice (`&[f32]`) borrowed from self
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa-tct` zero warnings
    - `cargo test -p context-graph-mejepa-tct shrinkage_engine constellation::lookup` shows ≥ 10 passing tests
    - `cargo clippy -p context-graph-mejepa-tct -- -D warnings` clean
    - N=50 boundary keeps OwnCell
    - N=4 fails closed with InsufficientSamples
    - Hierarchy order verified across all 4 levels
    - lookup_centroid round-trip with synthetic constellation
  </verification>
</definition_of_done>

<pseudo_code>
pub fn apply_shrinkage(per_chunk_type_buckets, panel_level)
    -> Result<(BTreeMap<...>, Vec<ShrinkageDecision>), TctError>
{
    let mut final_centroids = BTreeMap::new();
    let mut decisions = Vec::new();
    for (cell, bucket) in per_chunk_type_buckets {
        let (mutation, entity_type, language, embedder) = *cell;
        let (centroid, origin) = if bucket.count >= SHRINKAGE_FULL_THRESHOLD {
            let mut c: Vec<f32> = bucket.sum.iter().map(|s| s / bucket.count as f32).collect();
            l2_normalize_in_place(&mut c)?;
            (c, ShrinkageOrigin::OwnCell)
        } else if bucket.count >= SHRINKAGE_HARD_FLOOR {
            // Try language aggregate
            if let Some(by_emb) = panel_level.per_language_centroids.get(&(language, mutation)) {
                if let Some(c) = by_emb.get(&embedder) {
                    (c.clone(), ShrinkageOrigin::LanguageAggregate)
                } else {
                    try_entity_aggregate_or_category(per_chunk_type_buckets, panel_level, mutation, entity_type, embedder)?
                }
            } else {
                try_entity_aggregate_or_category(per_chunk_type_buckets, panel_level, mutation, entity_type, embedder)?
            }
        } else {
            return Err(TctError::InsufficientSamples {
                cell: format!("{:?}", cell),
                observed: bucket.count,
                required: SHRINKAGE_HARD_FLOOR,
            });
        };
        final_centroids.insert(*cell, centroid);
        decisions.push(ShrinkageDecision { cell: *cell, origin, observed_n: bucket.count });
    }
    Ok((final_centroids, decisions))
}

impl TctConstellation {
    pub fn lookup_centroid(&self, mutation, language, entity_type, embedder)
        -> Option<(&[f32], ShrinkageOrigin)>
    {
        if let Some(c) = self.per_chunk_type_centroids.get(&(mutation, entity_type, language, embedder)) {
            return Some((c, ShrinkageOrigin::OwnCell));
        }
        if let Some(by_emb) = self.per_language_centroids.get(&(language, mutation)) {
            if let Some(c) = by_emb.get(&embedder) {
                return Some((c, ShrinkageOrigin::LanguageAggregate));
            }
        }
        // entity-aggregate: search per_chunk_type for matching entity_type
        for ((m, e, _l, em), c) in &self.per_chunk_type_centroids {
            if *m == mutation && *e == entity_type && *em == embedder {
                return Some((c, ShrinkageOrigin::EntityAggregate));
            }
        }
        if let Some(by_emb) = self.per_category_centroids.get(&mutation) {
            if let Some(c) = by_emb.get(&embedder) {
                return Some((c, ShrinkageOrigin::CategoryAggregate));
            }
        }
        None
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-tct/src/shrinkage_engine.rs">apply_shrinkage + ShrinkageDecision + helpers + 6 tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-tct/src/builder.rs">add `finalize(self, thresholds, per_chunk_thresholds)` consuming PanelLevelCentroids + apply_shrinkage</file>
  <file path="crates/context-graph-mejepa-tct/src/constellation.rs">add `lookup_centroid` method + 4 hierarchy tests</file>
  <file path="crates/context-graph-mejepa-tct/src/lib.rs">add `pub mod shrinkage_engine;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa-tct zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa-tct shows ≥ 10 new passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa-tct -- -D warnings clean</criterion>
  <criterion>N=50 boundary keeps OwnCell</criterion>
  <criterion>N=4 fails closed with MEJEPA_TCT_INSUFFICIENT_SAMPLES</criterion>
  <criterion>Hierarchy order language → entity → category verified</criterion>
  <criterion>lookup_centroid round-trips for OwnCell, LanguageAggregate, EntityAggregate, CategoryAggregate, None</criterion>
  <criterion>finalize returns (TctConstellation, Vec&lt;ShrinkageDecision&gt;)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-tct</command>
  <command>cargo test -p context-graph-mejepa-tct</command>
  <command>cargo clippy -p context-graph-mejepa-tct -- -D warnings</command>
</test_commands>

</task_spec>
```

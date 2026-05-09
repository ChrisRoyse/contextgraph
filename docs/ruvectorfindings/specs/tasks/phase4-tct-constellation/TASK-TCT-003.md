# TASK-TCT-003 — ConstellationBuilder: per-category × per-language × outcome centroids

```xml
<task_spec id="TASK-TCT-003" version="1.0">

<metadata>
  <title>ConstellationBuilder panel-level centroids in `crates/context-graph-mejepa-tct/src/builder.rs`</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>3</sequence>
  <implements>
    <item ref="REQ-TCT-02">ConstellationBuilder per-cell mean → L2 normalize; deterministic given inputs</item>
    <item ref="component:ConstellationBuilder.new">accept corpus_sha + embedder_versions + code_version inputs</item>
    <item ref="component:ConstellationBuilder.ingest_corpus_entry">accumulate sum + count per cell</item>
    <item ref="AC-TCT-01-01">Per-category 8 × 21 = 168; per-language 9 × 8 × 21 = 1,512; outcome 4 × 21 = 84</item>
    <item ref="AC-TCT-01-02">Same inputs → byte-equal SHA-256 of serialized constellation</item>
  </implements>
  <depends_on>
    <task_ref>TASK-TCT-001</task_ref>
    <task_ref>TASK-TCT-002</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (builder.rs)</estimated_files>
</metadata>

<context>
This task lands the panel-level centroid construction. Per-cell centroid = mean(panels in cell)
followed by L2-normalize. Sum + count buckets accumulated incrementally via `ingest_corpus_entry`.
The chunk-level bucket (24,024 cells) is added in TASK-TCT-004 (Bayesian shrinkage); this task
covers the 1,764 panel-level cells (168 + 1,512 + 84). `finalize` is split: this task ships
`finalize_panel_level` returning a partial constellation; TASK-TCT-004 extends to per-chunk and
TASK-TCT-005 calibrates thresholds.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (`<tct_invariants>` rule 2 frozenness; rule 5 cell math)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-TCT.md (REQ-TCT-02; AC-TCT-01-01..02; US-TCT-01)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-TCT.md (component_contracts ConstellationBuilder.new + ingest_corpus_entry + finalize step 1-2)</file>
  <file purpose="upstream_types">crates/context-graph-mejepa-tct/src/{types.rs, constellation.rs, error.rs} (TASK-TCT-001/002)</file>
  <file purpose="instruments_panel">crates/context-graph-mejepa-instruments/src/lib.rs (Panel, slot_for accessor)</file>
</input_context_files>

<prerequisites>
  <check>TASK-TCT-001/002 complete</check>
  <check>`crates/context-graph-mejepa-instruments` exposes `Panel::slot_for(EmbedderId) -> &[f32]` (or via a method coming from EMBEDDERS Phase 1b)</check>
  <check>Phase 0 corpus iterator yields tuples `(panel, mutation_category, oracle_outcome, language, entity_type, chunk_panels)` — ≥ 2,000 entries</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa-tct/src/builder.rs` with:
      - `pub struct CentroidBucket { pub sum: Vec<f32>, pub count: usize }` (sum + count for incremental mean)
      - `pub struct ConstellationBuilder { corpus_sha, embedder_versions, code_version, per_category_buckets: BTreeMap<(MutationCategory, EmbedderId), CentroidBucket>, per_language_buckets: BTreeMap<(Language, MutationCategory, EmbedderId), CentroidBucket>, outcome_buckets: BTreeMap<(OracleOutcome, EmbedderId), CentroidBucket>, per_chunk_type_buckets: BTreeMap<(MutationCategory, EntityType, Language, EmbedderId), CentroidBucket> }`
      - `pub fn new(corpus_sha: [u8; 32], embedder_versions: BTreeMap<EmbedderId, [u8; 32]>, code_version: String) -> Result<Self, TctError>` validating provenance inputs (21 embedder versions, code_version 40-hex)
      - `pub fn ingest_corpus_entry(&mut self, panel: &Panel, mutation: MutationCategory, oracle_outcome: OracleOutcome, language: Language, entity_type: EntityType, chunk_panels: &[(ChunkId, Panel)]) -> Result<(), TctError>` accumulating sum + count for ALL bucket maps
      - `pub fn finalize_panel_level(&self) -> Result<PanelLevelCentroids, TctError>` returning struct with the 3 panel-level centroid maps (per-category, per-language, outcome) computed as `bucket.sum / bucket.count` THEN L2-normalized; reject zero-norm cells with TctError::DimMismatch
      - `pub struct PanelLevelCentroids { per_category_centroids, per_language_centroids, outcome_centroids, per_chunk_type_buckets_for_004: BTreeMap<...> }` — passes the chunk_panels buckets through to TASK-TCT-004 unmodified
    - Bucket ingestion validates panel slot dim matches embedder dim; rejects NaN/Inf via DimMismatch / NanDetected
    - Determinism: BTreeMap iteration order is canonical Rust default; sum-and-count reductions in fixed iteration order
    - Add unit tests with synthetic panels:
      - `new_rejects_partial_embedder_versions`: 20 entries → DimMismatch
      - `new_rejects_invalid_code_version`: not 40 hex → DimMismatch
      - `ingest_corpus_entry_accumulates_buckets_correctly`: 3 panels → 3-count, sum equals component-wise sum
      - `ingest_corpus_entry_rejects_nan_panel`: NaN in slot → NanDetected
      - `finalize_panel_level_l2_normalizes_centroids`: synthesized 5-panel cell → centroid norm == 1.0 ± 1e-6
      - `finalize_panel_level_rejects_zero_norm_cell`: synthesize panels summing to zero → DimMismatch
      - `finalize_panel_level_cell_counts_match_doc`: synthesize 8 categories × 21 embedders × 1 entry each → per_category_centroids has 8 × 21 = 168 cells
      - `byte_equal_finalize_across_two_runs`: build two times with same inputs → SHA-256 of serialize_finalize equals
  </in_scope>
  <out_of_scope>
    - Per-chunk-type centroid finalization with Bayesian shrinkage (TASK-TCT-004)
    - Threshold calibration (TASK-TCT-005)
    - Gτ check (TASK-TCT-006)
    - RocksDB persistence (TASK-TCT-008)
    - Real corpus iteration (Phase 0 owns the corpus walker; this task accepts pre-iterated tuples)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-tct/src/builder.rs">
use std::collections::BTreeMap;
use crate::types::{MutationCategory, OracleOutcome, Language, EntityType, EmbedderId, ChunkId};
use crate::error::TctError;
use context_graph_mejepa_instruments::Panel;

#[derive(Debug, Clone)]
pub struct CentroidBucket {
    pub sum: Vec&lt;f32&gt;,
    pub count: usize,
}

pub struct ConstellationBuilder {
    pub(crate) corpus_sha: [u8; 32],
    pub(crate) embedder_versions: BTreeMap&lt;EmbedderId, [u8; 32]&gt;,
    pub(crate) code_version: String,
    pub(crate) per_category_buckets:
        BTreeMap&lt;(MutationCategory, EmbedderId), CentroidBucket&gt;,
    pub(crate) per_language_buckets:
        BTreeMap&lt;(Language, MutationCategory, EmbedderId), CentroidBucket&gt;,
    pub(crate) outcome_buckets:
        BTreeMap&lt;(OracleOutcome, EmbedderId), CentroidBucket&gt;,
    pub(crate) per_chunk_type_buckets:
        BTreeMap&lt;(MutationCategory, EntityType, Language, EmbedderId), CentroidBucket&gt;,
}

impl ConstellationBuilder {
    pub fn new(
        corpus_sha: [u8; 32],
        embedder_versions: BTreeMap&lt;EmbedderId, [u8; 32]&gt;,
        code_version: String,
    ) -&gt; Result&lt;Self, TctError&gt;;

    pub fn ingest_corpus_entry(
        &amp;mut self,
        panel: &amp;Panel,
        mutation: MutationCategory,
        oracle_outcome: OracleOutcome,
        language: Language,
        entity_type: EntityType,
        chunk_panels: &amp;[(ChunkId, Panel)],
    ) -&gt; Result&lt;(), TctError&gt;;

    pub fn finalize_panel_level(&amp;self) -&gt; Result&lt;PanelLevelCentroids, TctError&gt;;
}

pub struct PanelLevelCentroids {
    pub per_category_centroids:
        BTreeMap&lt;MutationCategory, BTreeMap&lt;EmbedderId, Vec&lt;f32&gt;&gt;&gt;,
    pub per_language_centroids:
        BTreeMap&lt;(Language, MutationCategory), BTreeMap&lt;EmbedderId, Vec&lt;f32&gt;&gt;&gt;,
    pub outcome_centroids:
        BTreeMap&lt;OracleOutcome, BTreeMap&lt;EmbedderId, Vec&lt;f32&gt;&gt;&gt;,
    pub per_chunk_type_buckets_passthrough:
        BTreeMap&lt;(MutationCategory, EntityType, Language, EmbedderId), CentroidBucket&gt;,
}
    </signature>
  </signatures>
  <constraints>
    - All accumulation uses `f32` arithmetic; pairwise summation in fixed order for determinism
    - BTreeMap chosen for canonical iteration
    - 21 embedder versions required; 40-hex code_version required
    - Reject zero-norm cells with DimMismatch (norm &lt; 1e-12)
    - Reject NaN/Inf panel inputs with NanDetected
    - L2-normalization: c = c / ||c||; check `(||c|| - 1.0).abs() &lt; 1e-3` post-normalize
    - No mock data in tests beyond synthetic panels — synthesized via `Panel::test_fixture(...)` from the instruments crate
    - Module is `pub` so TASK-TCT-004 can import `CentroidBucket` + `PanelLevelCentroids`
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa-tct` zero warnings
    - `cargo test -p context-graph-mejepa-tct builder` shows ≥ 8 passing tests
    - `cargo clippy -p context-graph-mejepa-tct -- -D warnings` clean
    - Cell counts match constitution math (168 + 1,512 + 84)
    - Determinism: byte_equal_finalize_across_two_runs passes
    - Zero-norm cell rejected
    - NaN panel rejected
  </verification>
</definition_of_done>

<pseudo_code>
impl ConstellationBuilder {
    pub fn new(corpus_sha, embedder_versions, code_version) -> Result<Self, TctError> {
        if embedder_versions.len() != 21 {
            return Err(TctError::DimMismatch { expected: 21, actual: embedder_versions.len() });
        }
        if code_version.len() != 40 || !code_version.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(TctError::DimMismatch { expected: 40, actual: code_version.len() });
        }
        Ok(Self { corpus_sha, embedder_versions, code_version,
                  per_category_buckets: BTreeMap::new(),
                  per_language_buckets: BTreeMap::new(),
                  outcome_buckets: BTreeMap::new(),
                  per_chunk_type_buckets: BTreeMap::new() })
    }

    pub fn ingest_corpus_entry(&mut self, panel, mutation, oracle_outcome, language, entity_type, chunk_panels)
        -> Result<(), TctError>
    {
        for emb in EmbedderId::all() {
            let slot = panel.slot_for(emb).map_err(|_| TctError::DimMismatch { expected: 1, actual: 0 })?;
            if slot.iter().any(|x| !x.is_finite()) {
                return Err(TctError::NanDetected { field: format!("panel.slot_for({:?})", emb) });
            }
            accumulate_bucket(&mut self.per_category_buckets, (mutation, emb), slot);
            accumulate_bucket(&mut self.per_language_buckets, (language, mutation, emb), slot);
            accumulate_bucket(&mut self.outcome_buckets, (oracle_outcome, emb), slot);
        }
        for (chunk_id, chunk_panel) in chunk_panels {
            for emb in EmbedderId::all() {
                let slot = chunk_panel.slot_for(emb)?;
                accumulate_bucket(&mut self.per_chunk_type_buckets,
                    (mutation, chunk_id.entity_type, chunk_id.language, emb), slot);
            }
        }
        Ok(())
    }

    pub fn finalize_panel_level(&self) -> Result<PanelLevelCentroids, TctError> {
        let per_category   = compute_centroids(&self.per_category_buckets, |(c, e)| (*c, *e))?;
        let per_language   = compute_centroids(&self.per_language_buckets, |(l, c, e)| ((*l, *c), *e))?;
        let outcome        = compute_centroids(&self.outcome_buckets, |(o, e)| (*o, *e))?;
        Ok(PanelLevelCentroids {
            per_category_centroids: per_category,
            per_language_centroids: per_language,
            outcome_centroids: outcome,
            per_chunk_type_buckets_passthrough: self.per_chunk_type_buckets.clone(),
        })
    }
}

fn accumulate_bucket<K: Ord>(buckets: &mut BTreeMap<K, CentroidBucket>, key: K, slot: &[f32]) {
    let entry = buckets.entry(key).or_insert_with(|| CentroidBucket { sum: vec![0.0; slot.len()], count: 0 });
    if entry.sum.len() != slot.len() { /* dim mismatch — caller should validate */ return; }
    for (s, x) in entry.sum.iter_mut().zip(slot.iter()) { *s += *x; }
    entry.count += 1;
}

fn compute_centroids<K: Ord, OuterK: Ord, F>(buckets: &BTreeMap<K, CentroidBucket>, group: F)
    -> Result<BTreeMap<OuterK, BTreeMap<EmbedderId, Vec<f32>>>, TctError>
where F: Fn(&K) -> (OuterK, EmbedderId) // simplified
{
    // Compute mean per cell, L2-normalize, reject zero-norm
    // group by outer key into nested BTreeMap<OuterK, BTreeMap<EmbedderId, Vec<f32>>>
    // ...
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-tct/src/builder.rs">CentroidBucket + ConstellationBuilder + PanelLevelCentroids + 8 tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-tct/src/lib.rs">add `pub mod builder;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa-tct zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa-tct builder shows ≥ 8 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa-tct -- -D warnings clean</criterion>
  <criterion>Cell counts: per_category 168, outcome 84 verified analytically</criterion>
  <criterion>L2 norms equal 1.0 ± 1e-6 on every centroid</criterion>
  <criterion>Zero-norm rejected with DimMismatch</criterion>
  <criterion>NaN panel rejected with NanDetected</criterion>
  <criterion>byte_equal_finalize_across_two_runs deterministic</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-tct</command>
  <command>cargo test -p context-graph-mejepa-tct builder</command>
  <command>cargo clippy -p context-graph-mejepa-tct -- -D warnings</command>
</test_commands>

</task_spec>
```

# TASK-TCT-001 — Public TCT data models (TctConstellation, CorpusProvenance, MutationCategory, OracleOutcome, Language, EntityType, EmbedderId, ChunkId, GtauViolation, GtauOutput)

```xml
<task_spec id="TASK-TCT-001" version="1.0">

<metadata>
  <title>TCT public data models in `crates/context-graph-mejepa-tct/src/types.rs` + `constellation.rs`</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>1</sequence>
  <implements>
    <item ref="REQ-TCT-01">TctConstellation data type with all centroid maps + thresholds + corpus_provenance + frozen_at + version_id</item>
    <item ref="data_model:TctConstellation">FROZEN immutable post-construction</item>
    <item ref="data_model:CorpusProvenance">corpus_sha + per-embedder weight SHA-256 + frozen_at + code_version</item>
    <item ref="data_model:ChunkId">file_sha + language + entity_type + line_start + line_end + parent_chain_hash</item>
    <item ref="data_model:GtauViolation">embedder + observed_cos + threshold + violating_chunk</item>
    <item ref="data_model:GtauOutput">satisfied + violations + centroid_origin + elapsed_ms</item>
    <item ref="constitution:tct_invariants">centroid frozenness; cell math 24,024</item>
  </implements>
  <depends_on><!-- foundation; cross-domain shipped --></depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>3 created (Cargo.toml, src/lib.rs, src/types.rs, src/constellation.rs — split for &lt;500 lines per file)</estimated_files>
</metadata>

<context>
This task creates the new `crates/context-graph-mejepa-tct/` crate skeleton and lands the public
data model contract that downstream tasks (003 builder, 004 chunk shrinkage, 005 calibrator, 006
Gτ, 007 hinge, 008 store, 011 MCP) all consume. Field shapes match TECH-TCT data_models block
byte-for-byte. TctConstellation is FROZEN by construction: immutable post-build; updates require
new version_id with new corpus_provenance. Centroids are L2-normalized to within f32 ULP.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (`<tct_invariants>` rules 1-5; rocksdb_cfs naming)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-TCT.md (REQ-TCT-01; data_requirements TctConstellation, ChunkId, GtauViolation; CorpusProvenance)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-TCT.md (data_models block; panel_layout_referenced_diagram)</file>
  <file purpose="ast_chunker_dep">crates/context-graph-core/src/memory/ast_chunker.rs (Language, EntityType reference for cross-crate compat)</file>
  <file purpose="instruments_dep">crates/context-graph-mejepa-instruments/src/lib.rs (Panel layout reference; not consumed directly here)</file>
  <file purpose="cross_domain_inference">crates/context-graph-mejepa/src/types.rs (Phase 4a Language, EmbedderId, ChunkId, EntityType — for parity)</file>
  <file purpose="naming_conventions">docs/ruvectorfindings/specs/constitution.md#coding_standards (snake_case files, PascalCase types, BTreeMap canonical ordering)</file>
</input_context_files>

<prerequisites>
  <check>Workspace builds: `cargo build --release --workspace`</check>
  <check>`crates/context-graph-mejepa/src/types.rs` (TASK-INFER-001) ships compatible Language/EntityType/EmbedderId/ChunkId enums to import or mirror</check>
  <check>`serde = "1"`, `serde_json = "1"` workspace deps</check>
  <check>`bincode = "1"` workspace dep (needed by 008 but referenced via Cargo.toml here)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa-tct/Cargo.toml` (workspace member; deps to mejepa-instruments, mejepa, witness, storage, serde, bincode, rocksdb [optional later], blake3)
    - Create `crates/context-graph-mejepa-tct/src/lib.rs` declaring `pub mod {types, constellation};`
    - Create `crates/context-graph-mejepa-tct/src/types.rs` with leaf types:
      - `MutationCategory` enum (8 variants: KnownGood, SubtleFlip, OffByOne, SwapVariable, DeleteTestCall, WrongFile, OverEngineer, CompileError) — re-export from mejepa-corpus if visible; otherwise duplicate
      - `OracleOutcome` enum (Pass, Fail, OutOfDistribution, Abstain) — re-export from mejepa-corpus or mejepa
      - `Language` enum (11 variants) — re-export from mejepa
      - `EntityType` enum (13 variants: Function, Method, Class, Struct, Impl, Module, Trait, Interface, Macro, Enum, Union, Namespace, TopLevelStmt) — re-export from mejepa
      - `EmbedderId` enum (21 variants E1..E21) — re-export from mejepa
      - `ChunkId { file_sha: [u8; 32], language: Language, entity_type: EntityType, line_start: u32, line_end: u32, parent_chain_hash: [u8; 16] }` with `try_new` validating line_end ≥ line_start
      - `GtauViolation { embedder: EmbedderId, observed_cos: f32, threshold: f32, violating_chunk: Option<ChunkId> }` with `try_new` validating finite + cos ∈ [-1, 1]
      - `GtauOutput { gtau_satisfied: bool, violations: Vec<GtauViolation>, centroid_origin: BTreeMap<EmbedderId, ShrinkageOrigin>, elapsed_ms: f32 }` (ShrinkageOrigin lands in TASK-TCT-002; forward-declare via opaque alias)
      - `CorpusProvenance { corpus_sha: [u8; 32], embedder_versions: BTreeMap<EmbedderId, [u8; 32]>, frozen_at: SystemTime, code_version: String }` with `try_new` validating embedder_versions has 21 entries + code_version is 40-hex git SHA
    - Create `crates/context-graph-mejepa-tct/src/constellation.rs` with:
      - `TctConstellation` struct per data_models block (per_category_centroids, per_language_centroids, outcome_centroids, per_chunk_type_centroids, thresholds, per_chunk_type_thresholds, corpus_provenance, frozen_at, version_id)
      - `pub fn version_id(&self) -> [u8; 32]` returning SHA-256 of canonical bincode-serialization (computed once at construction; cached)
      - `pub fn try_new(...)` constructor enforcing L2-normalization + finiteness invariants on every centroid; rejects with TctError on violation (TctError lands in TASK-TCT-002 — use placeholder `Result<_, &'static str>` here, replaced in TASK-TCT-002)
    - All public types derive `Debug`, `Clone`, `Serialize`, `Deserialize`, `PartialEq`. Where natural also `Eq`, `Hash`, `Copy`. `#[serde(rename_all = "snake_case")]` on every enum.
    - Add unit tests in `src/types.rs` and `src/constellation.rs`:
      - `mutation_category_serde_eight_variants`: snake_case round-trip
      - `language_serde_eleven_variants`
      - `entity_type_serde_thirteen_variants`
      - `embedder_id_serde_twenty_one_variants`
      - `chunk_id_rejects_line_end_lt_start`
      - `gtau_violation_rejects_nan_cos`
      - `gtau_violation_rejects_cos_above_one`
      - `gtau_output_serde_round_trip`
      - `corpus_provenance_rejects_partial_embedder_versions` (only 20 entries → reject)
      - `tct_constellation_try_new_rejects_unnormalized_centroid` (norm 0.5 → reject)
      - `tct_constellation_try_new_rejects_nan_centroid`
      - `tct_constellation_version_id_deterministic`: build twice with same fields → byte-equal version_id
  </in_scope>
  <out_of_scope>
    - TctError + variants (TASK-TCT-002)
    - ShrinkageOrigin / ChunkGtauOutput / VerdictGuardRejected / ConstellationHingeOutput / RollingWindow / PredictorOutput (TASK-TCT-002)
    - Actual centroid construction (TASK-TCT-003)
    - Bayesian shrinkage logic (TASK-TCT-004)
    - Gτ test (TASK-TCT-006)
    - RocksDB persistence (TASK-TCT-008)
    - Hinge primitive (TASK-TCT-007)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-tct/src/types.rs">
use std::collections::BTreeMap;
use std::time::SystemTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationCategory {
    KnownGood, SubtleFlip, OffByOne, SwapVariable,
    DeleteTestCall, WrongFile, OverEngineer, CompileError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OracleOutcome { Pass, Fail, OutOfDistribution, Abstain }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    Rust, Python, Javascript, Typescript, Go, Java, C, Cpp, CSharp, Ruby, Php,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Function, Method, Class, Struct, Impl, Module, Trait, Interface,
    Macro, Enum, Union, Namespace, TopLevelStmt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbedderId {
    E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11,
    E12, E13, E14, E15, E16, E17, E18, E19, E20, E21,
}

impl EmbedderId {
    pub fn all() -&gt; [EmbedderId; 21];
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ChunkId {
    pub file_sha: [u8; 32],
    pub language: Language,
    pub entity_type: EntityType,
    pub line_start: u32,
    pub line_end: u32,
    pub parent_chain_hash: [u8; 16],
}

impl ChunkId {
    pub fn try_new(file_sha: [u8; 32], language: Language, entity_type: EntityType,
                   line_start: u32, line_end: u32, parent_chain_hash: [u8; 16])
        -&gt; Result&lt;Self, &amp;'static str&gt;;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GtauViolation {
    pub embedder: EmbedderId,
    pub observed_cos: f32,
    pub threshold: f32,
    pub violating_chunk: Option&lt;ChunkId&gt;,
}

impl GtauViolation {
    pub fn try_new(embedder: EmbedderId, observed_cos: f32, threshold: f32,
                   violating_chunk: Option&lt;ChunkId&gt;)
        -&gt; Result&lt;Self, &amp;'static str&gt;;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GtauOutput {
    pub gtau_satisfied: bool,
    pub violations: Vec&lt;GtauViolation&gt;,
    pub centroid_origin: BTreeMap&lt;EmbedderId, ShrinkageOriginPlaceholder&gt;,
    pub elapsed_ms: f32,
}

/// Placeholder; lands as `ShrinkageOrigin` enum in TASK-TCT-002.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShrinkageOriginPlaceholder(pub u8);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CorpusProvenance {
    pub corpus_sha: [u8; 32],
    pub embedder_versions: BTreeMap&lt;EmbedderId, [u8; 32]&gt;,
    pub frozen_at: SystemTime,
    pub code_version: String,
}

impl CorpusProvenance {
    pub fn try_new(corpus_sha: [u8; 32],
                   embedder_versions: BTreeMap&lt;EmbedderId, [u8; 32]&gt;,
                   frozen_at: SystemTime,
                   code_version: String)
        -&gt; Result&lt;Self, &amp;'static str&gt;;
}
    </signature>
    <signature file="crates/context-graph-mejepa-tct/src/constellation.rs">
use std::collections::BTreeMap;
use std::time::SystemTime;
use serde::{Deserialize, Serialize};
use crate::types::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TctConstellation {
    pub per_category_centroids:
        BTreeMap&lt;MutationCategory, BTreeMap&lt;EmbedderId, Vec&lt;f32&gt;&gt;&gt;,
    pub per_language_centroids:
        BTreeMap&lt;(Language, MutationCategory), BTreeMap&lt;EmbedderId, Vec&lt;f32&gt;&gt;&gt;,
    pub outcome_centroids:
        BTreeMap&lt;OracleOutcome, BTreeMap&lt;EmbedderId, Vec&lt;f32&gt;&gt;&gt;,
    pub per_chunk_type_centroids:
        BTreeMap&lt;(MutationCategory, EntityType, Language, EmbedderId), Vec&lt;f32&gt;&gt;,
    pub thresholds: BTreeMap&lt;EmbedderId, f32&gt;,
    pub per_chunk_type_thresholds: BTreeMap&lt;(EntityType, EmbedderId), f32&gt;,
    pub corpus_provenance: CorpusProvenance,
    pub frozen_at: SystemTime,
    pub version_id: [u8; 32],
}

impl TctConstellation {
    pub fn try_new(
        per_category_centroids: BTreeMap&lt;MutationCategory, BTreeMap&lt;EmbedderId, Vec&lt;f32&gt;&gt;&gt;,
        per_language_centroids: BTreeMap&lt;(Language, MutationCategory), BTreeMap&lt;EmbedderId, Vec&lt;f32&gt;&gt;&gt;,
        outcome_centroids: BTreeMap&lt;OracleOutcome, BTreeMap&lt;EmbedderId, Vec&lt;f32&gt;&gt;&gt;,
        per_chunk_type_centroids: BTreeMap&lt;(MutationCategory, EntityType, Language, EmbedderId), Vec&lt;f32&gt;&gt;,
        thresholds: BTreeMap&lt;EmbedderId, f32&gt;,
        per_chunk_type_thresholds: BTreeMap&lt;(EntityType, EmbedderId), f32&gt;,
        corpus_provenance: CorpusProvenance,
    ) -&gt; Result&lt;Self, &amp;'static str&gt;;

    pub fn version_id(&amp;self) -&gt; [u8; 32];
}
    </signature>
  </signatures>
  <constraints>
    - Every centroid Vec&lt;f32&gt; L2-normalized within f32 ULP at construction (`try_new` validates)
    - Every centroid finite (no NaN/Inf)
    - Every threshold ∈ [-1, 1] and finite
    - 21 EmbedderId entries required in CorpusProvenance.embedder_versions
    - code_version is exactly 40 hex chars (git SHA-1)
    - frozen_at not in the future (SystemTime::now()-tolerant)
    - line_end ≥ line_start in ChunkId
    - observed_cos ∈ [-1, 1] in GtauViolation; threshold ∈ [-1, 1]; both finite
    - BTreeMap chosen for deterministic iteration order (per implementation_notes serialization rule)
    - All structs derive `serde(deny_unknown_fields)` where appropriate
    - NO `unwrap_or` on confidence/threshold values
    - Crate name is `context-graph-mejepa-tct` per constitution kebab-case rule
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa-tct` succeeds with zero warnings
    - `cargo test -p context-graph-mejepa-tct types constellation` shows ≥ 12 passing tests
    - `cargo clippy -p context-graph-mejepa-tct -- -D warnings` clean
    - All enum variant counts match constitution: MutationCategory=8, OracleOutcome=4, Language=11, EntityType=13, EmbedderId=21
    - CorpusProvenance rejects partial embedder_versions
    - TctConstellation rejects unnormalized centroid
    - version_id is deterministic across two builds with same fields
  </verification>
</definition_of_done>

<pseudo_code>
// types.rs
impl ChunkId {
    pub fn try_new(file_sha, language, entity_type, line_start, line_end, parent_chain_hash) -> Result<Self, &'static str> {
        if line_end < line_start { return Err("line_end &lt; line_start"); }
        if line_start == 0 { return Err("line_start must be ≥ 1 (1-indexed)"); }
        Ok(Self { file_sha, language, entity_type, line_start, line_end, parent_chain_hash })
    }
}

impl GtauViolation {
    pub fn try_new(embedder, observed_cos, threshold, violating_chunk) -> Result<Self, &'static str> {
        if !observed_cos.is_finite() || !(-1.0..=1.0).contains(&observed_cos) {
            return Err("observed_cos out of [-1, 1] or non-finite");
        }
        if !threshold.is_finite() || !(-1.0..=1.0).contains(&threshold) {
            return Err("threshold out of [-1, 1] or non-finite");
        }
        Ok(Self { embedder, observed_cos, threshold, violating_chunk })
    }
}

// constellation.rs
fn assert_l2_normalized(v: &[f32]) -> Result<(), &'static str> {
    if v.is_empty() { return Err("empty centroid"); }
    let n: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if !n.is_finite() { return Err("non-finite centroid norm"); }
    if (n - 1.0).abs() > 1e-3 { return Err("centroid not L2-normalized within tolerance"); }
    if v.iter().any(|x| !x.is_finite()) { return Err("non-finite element"); }
    Ok(())
}

impl TctConstellation {
    pub fn try_new(...) -> Result<Self, &'static str> {
        for (_, by_emb) in &per_category_centroids {
            for (_, c) in by_emb { assert_l2_normalized(c)?; }
        }
        // ... same for per_language, outcome, per_chunk_type ...
        for (_, t) in &thresholds {
            if !t.is_finite() || !(-1.0..=1.0).contains(t) { return Err("threshold out of range"); }
        }
        let frozen_at = SystemTime::now();
        let mut s = Self {
            per_category_centroids, per_language_centroids, outcome_centroids,
            per_chunk_type_centroids, thresholds, per_chunk_type_thresholds,
            corpus_provenance, frozen_at,
            version_id: [0u8; 32], // placeholder; computed next
        };
        s.version_id = compute_version_id(&s);
        Ok(s)
    }

    pub fn version_id(&self) -> [u8; 32] { self.version_id }
}

fn compute_version_id(c: &TctConstellation) -> [u8; 32] {
    use sha2::{Sha256, Digest};
    let bytes = bincode::serialize(c).expect("bincode fixed");
    let digest = Sha256::digest(&bytes);
    digest.into()
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-tct/Cargo.toml">workspace member metadata + deps</file>
  <file path="crates/context-graph-mejepa-tct/src/lib.rs">crate-root with `pub mod {types, constellation};`</file>
  <file path="crates/context-graph-mejepa-tct/src/types.rs">leaf types + enums + ChunkId/GtauViolation/GtauOutput/CorpusProvenance + tests</file>
  <file path="crates/context-graph-mejepa-tct/src/constellation.rs">TctConstellation + try_new + version_id + tests</file>
</files_to_create>

<files_to_modify>
  <file path="Cargo.toml (workspace root)">add `crates/context-graph-mejepa-tct` to `[workspace.members]`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa-tct zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa-tct shows ≥ 12 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa-tct -- -D warnings clean</criterion>
  <criterion>MutationCategory enum has exactly 8 variants in canonical order</criterion>
  <criterion>OracleOutcome enum has exactly 4 variants</criterion>
  <criterion>Language enum has exactly 11 variants</criterion>
  <criterion>EntityType enum has exactly 13 variants</criterion>
  <criterion>EmbedderId enum has exactly 21 variants E1..E21</criterion>
  <criterion>ChunkId rejects line_end &lt; line_start</criterion>
  <criterion>GtauViolation rejects NaN/Inf cos and out-of-range threshold</criterion>
  <criterion>CorpusProvenance rejects 20-entry embedder_versions</criterion>
  <criterion>TctConstellation rejects unnormalized centroid</criterion>
  <criterion>version_id deterministic across two identical builds</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-tct</command>
  <command>cargo test -p context-graph-mejepa-tct</command>
  <command>cargo clippy -p context-graph-mejepa-tct -- -D warnings</command>
</test_commands>

</task_spec>
```

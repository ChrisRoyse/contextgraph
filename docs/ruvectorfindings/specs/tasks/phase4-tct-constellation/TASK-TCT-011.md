# TASK-TCT-011 — mcp__cgreality__mejepa_constellation_inspect MCP tool

```xml
<task_spec id="TASK-TCT-011" version="1.0">

<metadata>
  <title>MCP inspector tool in `crates/context-graph-mcp/src/handlers/tools/mejepa_tct.rs`</title>
  <status>blocked</status>
  <layer>surface</layer>
  <sequence>11</sequence>
  <implements>
    <item ref="REQ-TCT-15">mcp__cgreality__mejepa_constellation_inspect returns metadata + cell counts + thresholds; never raw centroid byte arrays</item>
    <item ref="AC-TCT-04-02">Provenance mismatch surfaced via load error</item>
    <item ref="component:McpInspectTool">mejepa_constellation_inspect output schema</item>
    <item ref="constitution:mcp_tool_namespace_lock">mcp__cgreality__mejepa_*</item>
  </implements>
  <depends_on>
    <task_ref>TASK-TCT-008</task_ref>
    <task_ref>TASK-TCT-009</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 created (handler); 1 modified (mod.rs dispatch)</estimated_files>
</metadata>

<context>
This task wires the constellation inspector into the MCP server. The tool returns a JSON-
serializable summary (version_id, corpus_provenance, frozen_at, cell counts per category × language
× entity_type, sample-support histogram, calibrated threshold table) for operator inspection.
Never returns raw centroid byte arrays — only summary statistics — to keep responses lightweight
and non-leaky (raw centroids are constellation IP).
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (mcp_tool_namespace_lock)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-TCT.md (REQ-TCT-15; BR-TCT-10)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-TCT.md (api_contracts mcp__cgreality__mejepa_constellation_inspect; component_contracts McpInspectTool.mejepa_constellation_inspect)</file>
  <file purpose="upstream_store">crates/context-graph-mejepa-tct/src/store.rs (ConstellationStore.load)</file>
  <file purpose="upstream_freshness">crates/context-graph-mejepa-tct/src/freshness.rs (read_freshness_config)</file>
  <file purpose="mcp_pattern">crates/context-graph-mcp/src/handlers/tools/mejepa.rs (TASK-INFER-011 schema patterns)</file>
</input_context_files>

<prerequisites>
  <check>TASK-TCT-008/009 complete</check>
  <check>`crates/context-graph-mcp` builds with mejepa.rs handler</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mcp/src/handlers/tools/mejepa_tct.rs` with:
      - `pub const TOOL_NAME_INSPECT: &str = "mcp__cgreality__mejepa_constellation_inspect";`
      - JSON schema `additionalProperties: false` for input `{ version_id: Option<String hex 64>, ... }`
      - `pub async fn handle_constellation_inspect(args: serde_json::Value, store: &ConstellationStore, runtime_embedder_versions: &BTreeMap<EmbedderId, [u8; 32]>) -> Result<serde_json::Value, JsonRpcError>`
      - Loads constellation via store.load (with check_freshness layered); computes summary (cell_counts, sample_support_histogram, threshold table, per_chunk_type_threshold_summary); serializes per TECH-TCT api_contracts response shape
      - `pub fn build_inspect_summary(constellation: &TctConstellation, shrinkage_decisions: Option<&[ShrinkageDecision]>) -> ConstellationInspectOutput`
      - `pub struct ConstellationInspectOutput { version_id: String, corpus_provenance: ..., frozen_at: String, cell_counts: { panel_level_cells: u32, per_chunk_type_cells: u32, shrunken_cells: u32 }, sample_support_histogram: { N_lt_5: u32, N_5_to_50: u32, N_50_plus: u32 }, thresholds: BTreeMap<EmbedderId, f32>, per_chunk_type_threshold_summary: BTreeMap<EntityType, EntityThresholdSummary> }`
    - Register handler in `crates/context-graph-mcp/src/handlers/tools/mod.rs` dispatch table
    - Add integration tests:
      - `inspect_returns_summary_for_loaded_version`: persist + load via tempdir → response has version_id, cell_counts, thresholds
      - `inspect_default_version_id_uses_currently_loaded`: no version_id arg → uses default
      - `inspect_provenance_mismatch_returns_error`: runtime embedder digest mismatch → MEJEPA_TCT_PROVENANCE_MISMATCH
      - `inspect_stale_constellation_returns_error`: 91-day-old constellation, allow_stale=false → MEJEPA_INFER_GTAU_STALE_CONSTELLATION
      - `inspect_never_returns_raw_centroids`: response JSON does NOT contain any field whose value is a Vec&lt;f32&gt; centroid byte array (only summary stats)
      - `inspect_rejects_extra_fields`: additionalProperties: false enforced
      - `inspect_rejects_invalid_hex_version_id`: 63-char hex → invalid_params
  </in_scope>
  <out_of_scope>
    - Mejepa_verify (TASK-INFER-011 owns)
    - Mejepa_predict_latest (TASK-INFER-012 owns)
    - Constellation refit trigger (Phase 5)
    - Live constellation_violation_rate query (could be added later as `mejepa_violation_rate_inspect` separate tool)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mcp/src/handlers/tools/mejepa_tct.rs">
use std::collections::BTreeMap;
use context_graph_mejepa_tct::types::{EmbedderId, EntityType};
use context_graph_mejepa_tct::constellation::TctConstellation;
use context_graph_mejepa_tct::store::ConstellationStore;
use context_graph_mejepa_tct::shrinkage_engine::ShrinkageDecision;

pub const TOOL_NAME_INSPECT: &amp;str = "mcp__cgreality__mejepa_constellation_inspect";

pub fn schema_inspect() -&gt; serde_json::Value;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ConstellationInspectOutput {
    pub version_id: String,
    pub corpus_sha: String,
    pub embedder_versions: BTreeMap&lt;EmbedderId, String&gt;,
    pub frozen_at: String,
    pub code_version: String,
    pub cell_counts: CellCounts,
    pub sample_support_histogram: SampleSupportHistogram,
    pub thresholds: BTreeMap&lt;EmbedderId, f32&gt;,
    pub per_chunk_type_threshold_summary: BTreeMap&lt;EntityType, EntityThresholdSummary&gt;,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct CellCounts {
    pub panel_level_cells: u32,
    pub per_chunk_type_cells: u32,
    pub shrunken_cells: u32,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct SampleSupportHistogram {
    pub n_lt_5: u32,
    pub n_5_to_50: u32,
    pub n_50_plus: u32,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct EntityThresholdSummary {
    pub mean_threshold: f32,
    pub n_cells: u32,
}

pub fn build_inspect_summary(
    constellation: &amp;TctConstellation,
    shrinkage_decisions: Option&lt;&amp;[ShrinkageDecision]&gt;,
) -&gt; ConstellationInspectOutput;

pub async fn handle_constellation_inspect(
    args: serde_json::Value,
    store: &amp;ConstellationStore,
    runtime_embedder_versions: &amp;BTreeMap&lt;EmbedderId, [u8; 32]&gt;,
) -&gt; Result&lt;serde_json::Value, JsonRpcError&gt;;
    </signature>
  </signatures>
  <constraints>
    - Tool name namespace-locked: `mcp__cgreality__mejepa_constellation_inspect`
    - Read-only; cannot mutate constellation
    - additionalProperties: false on input schema
    - Response NEVER includes raw centroid byte arrays (Vec&lt;f32&gt;) — only summary statistics
    - version_id input: 64-char hex; default = currently-loaded
    - On load error (provenance/staleness), surface code via JSON-RPC application error with `error_code = e.code()`
    - Handler runs check_freshness implicitly via store.load (TASK-TCT-008 +TASK-TCT-009 layering)
    - hex-encoded fields: corpus_sha, embedder_versions values, version_id
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mcp` zero warnings
    - `cargo test -p context-graph-mcp mejepa_constellation_inspect` shows ≥ 7 passing tests
    - `cargo clippy -p context-graph-mcp -- -D warnings` clean
    - Tool name namespace-locked
    - additionalProperties: false rejects extra fields
    - Response never includes raw centroid Vec&lt;f32&gt;
    - Provenance + staleness errors surface correctly
  </verification>
</definition_of_done>

<pseudo_code>
pub fn build_inspect_summary(constellation, shrinkage_decisions) -> ConstellationInspectOutput {
    let panel_level_cells =
        constellation.per_category_centroids.values().map(|m| m.len() as u32).sum::<u32>()
        + constellation.per_language_centroids.values().map(|m| m.len() as u32).sum::<u32>()
        + constellation.outcome_centroids.values().map(|m| m.len() as u32).sum::<u32>();
    let per_chunk_type_cells = constellation.per_chunk_type_centroids.len() as u32;
    let shrunken_cells = shrinkage_decisions.map(|d|
        d.iter().filter(|x| !matches!(x.origin, context_graph_mejepa_tct::shrinkage::ShrinkageOrigin::OwnCell)).count() as u32
    ).unwrap_or(0);

    let mut hist = SampleSupportHistogram { n_lt_5: 0, n_5_to_50: 0, n_50_plus: 0 };
    if let Some(d) = shrinkage_decisions {
        for decision in d {
            match decision.observed_n {
                n if n < 5 => hist.n_lt_5 += 1,
                n if n < 50 => hist.n_5_to_50 += 1,
                _ => hist.n_50_plus += 1,
            }
        }
    }

    let mut by_entity: BTreeMap<EntityType, Vec<f32>> = BTreeMap::new();
    for ((entity, _emb), tau) in &constellation.per_chunk_type_thresholds {
        by_entity.entry(*entity).or_default().push(*tau);
    }
    let per_chunk_type_threshold_summary: BTreeMap<EntityType, EntityThresholdSummary> = by_entity.into_iter()
        .map(|(et, taus)| {
            let mean = taus.iter().sum::<f32>() / taus.len() as f32;
            (et, EntityThresholdSummary { mean_threshold: mean, n_cells: taus.len() as u32 })
        }).collect();

    ConstellationInspectOutput {
        version_id: hex::encode(constellation.version_id),
        corpus_sha: hex::encode(constellation.corpus_provenance.corpus_sha),
        embedder_versions: constellation.corpus_provenance.embedder_versions.iter()
            .map(|(e, sha)| (*e, hex::encode(sha))).collect(),
        frozen_at: chrono::DateTime::<chrono::Utc>::from(constellation.frozen_at).to_rfc3339(),
        code_version: constellation.corpus_provenance.code_version.clone(),
        cell_counts: CellCounts { panel_level_cells, per_chunk_type_cells, shrunken_cells },
        sample_support_histogram: hist,
        thresholds: constellation.thresholds.clone(),
        per_chunk_type_threshold_summary,
    }
}

pub async fn handle_constellation_inspect(args, store, runtime_embedder_versions)
    -> Result<Value, JsonRpcError>
{
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Req { #[serde(default)] version_id: Option<String> }

    let req: Req = serde_json::from_value(args)
        .map_err(|e| JsonRpcError::invalid_params(e.to_string()))?;
    let mut vid = [0u8; 32];
    if let Some(v) = req.version_id {
        if v.len() != 64 || !v.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(JsonRpcError::invalid_params("version_id must be 64 hex chars"));
        }
        hex::decode_to_slice(&v, &mut vid).map_err(|e| JsonRpcError::invalid_params(e.to_string()))?;
    } else {
        // default to currently-loaded version_id
        vid = current_loaded_version_id();
    }

    let constellation = store.load(vid, runtime_embedder_versions)
        .map_err(|e| JsonRpcError::application(e.code(), e.to_string()))?;
    let summary = build_inspect_summary(&constellation, None);
    Ok(serde_json::to_value(summary)?)
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa_tct.rs">handler + schema + summary builder + 7 integration tests</file>
  <file path="crates/context-graph-mcp/tests/mejepa_constellation_inspect_integration.rs">end-to-end JSON-RPC tests with tempdir</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mcp/src/handlers/tools/mod.rs">register `handle_constellation_inspect` in dispatch table; export TOOL_NAME_INSPECT</file>
  <file path="crates/context-graph-mcp/Cargo.toml">add `context-graph-mejepa-tct` path dep + `hex` + dev-dep `tempfile`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mcp zero warnings</criterion>
  <criterion>cargo test -p context-graph-mcp mejepa_constellation_inspect shows ≥ 7 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mcp -- -D warnings clean</criterion>
  <criterion>Tool name == "mcp__cgreality__mejepa_constellation_inspect"</criterion>
  <criterion>additionalProperties: false enforced</criterion>
  <criterion>Response never contains raw centroid Vec&lt;f32&gt;</criterion>
  <criterion>Provenance mismatch returns MEJEPA_TCT_PROVENANCE_MISMATCH</criterion>
  <criterion>Stale constellation returns MEJEPA_INFER_GTAU_STALE_CONSTELLATION</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mcp</command>
  <command>cargo test -p context-graph-mcp mejepa_constellation_inspect</command>
  <command>cargo clippy -p context-graph-mcp -- -D warnings</command>
</test_commands>

</task_spec>
```

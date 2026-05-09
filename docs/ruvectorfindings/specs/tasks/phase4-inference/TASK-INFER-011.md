# TASK-INFER-011 — mcp__cgreality__mejepa_verify MCP tool (with include_provenance flag)

```xml
<task_spec id="TASK-INFER-011" version="1.0">

<metadata>
  <title>mcp__cgreality__mejepa_verify handler in `crates/context-graph-mcp/src/handlers/tools/mejepa.rs`</title>
  <status>blocked</status>
  <layer>surface</layer>
  <sequence>11</sequence>
  <implements>
    <item ref="REQ-INFER-16">mcp__cgreality__mejepa_verify is the primary inference path</item>
    <item ref="AC-INFER-01-05">include_provenance: true returns provenance bundle</item>
    <item ref="EC-INFER-10">unloadable calibration/constellation → Option::None + provenance_warnings</item>
    <item ref="BR-INFER-01">External callers go through MCP only</item>
    <item ref="BR-INFER-06">Retired tools fail-closed with CCREALITY_ENGINE_RETIRED</item>
    <item ref="component:mcp_endpoint">JSON schema with additionalProperties: false; path canonicalization at boundary</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INFER-004</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (mejepa.rs handler); 1 modified (handlers/tools/mod.rs)</estimated_files>
</metadata>

<context>
This task wires `MeJepaCompiler::verify` into the MCP server as `mcp__cgreality__mejepa_verify`.
The handler is a thin wrapper: deserialize JSON → validate canonically → call verify() → serialize.
Schema enforces `additionalProperties: false` per audit-9 lesson and rejects path traversal at the
boundary per SEC-MEJEPA-01. include_provenance: true returns the ProvenanceBundle (panel SHA-256,
embedder versions, calibration version, training cert offset, constellation version, code_version).
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (mcp_tool_namespace_lock; SEC-MEJEPA-01; failure_taxonomy CCREALITY_ENGINE_RETIRED)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INFERENCE.md (REQ-INFER-16; AC-INFER-01-05; EC-INFER-10; BR-INFER-01/06)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INFERENCE.md (api_contracts mcp__cgreality__mejepa_verify)</file>
  <file purpose="upstream_compiler">crates/context-graph-mejepa/src/compiler.rs (verify method)</file>
  <file purpose="upstream_provenance">crates/context-graph-mejepa/src/provenance.rs (ProvenanceBundle)</file>
  <file purpose="mcp_handler_pattern">crates/context-graph-mcp/src/handlers/tools/ (existing handler examples for schema patterns)</file>
  <file purpose="naming_conventions">docs/ruvectorfindings/specs/constitution.md#coding_standards (mcp_tools snake_case prefix)</file>
</input_context_files>

<prerequisites>
  <check>TASK-INFER-004 complete; verify() returns Result&lt;VerifyVerdict, MejepaInferError&gt;</check>
  <check>`crates/context-graph-mejepa/src/provenance.rs` exists (TASK-INFER-002)</check>
  <check>`crates/context-graph-mcp` builds; existing tool handler pattern visible in `handlers/tools/mod.rs`</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mcp/src/handlers/tools/mejepa.rs` with:
      - `pub const TOOL_NAME_VERIFY: &str = "mcp__cgreality__mejepa_verify";`
      - JSON-schema definition (`additionalProperties: false` on every nested object) for input { patch: PatchBundle, context: TaskContext, include_provenance: bool? }
      - `pub async fn handle_mejepa_verify(args: serde_json::Value, compiler: &MeJepaCompiler, repo_root: &Path) -> Result<serde_json::Value, JsonRpcError>` — deserialize → validate path canonicalization on PatchBundle.ast_diff.touched_paths via `gates::canonicalize_under_root(p, repo_root)?` → call `compiler.verify(&patch, &context)` → serialize via serde_json
      - On `Err(MejepaInferError)`: convert to JSON-RPC error with `error_code = e.code()` and `message = e.to_string()` and structured `payload`
      - On `Ok(VerifyVerdict)`: serialize per technical spec response shape `{ verdict: "approve" | "escalate_to_human", reality_prediction, gates_passed?, failed_gate?, provenance? }`
      - When `include_provenance: true`: build ProvenanceBundle from the loaded calibration version (`calibration.load_active().ok().map(|r| r.version)`), constellation version (`constellation.version_id()`), embedder_versions (from compiler.config), training_cert_chain_offset (from compiler.storage), code_version (`env!("CARGO_PKG_VERSION")` + `git rev-parse HEAD`), `panel_sha` (computed from materialized panel at t=2 via SHA-256); on Option::None for any unloadable component, emit `provenance_warnings: ["calibration_version unloadable", ...]`
      - Reject `problem_statement.len() > 65536` at boundary (SEC-MEJEPA-01)
      - Reject control characters in any path string (0x00-0x1F or 0x7F)
    - Register handler in `crates/context-graph-mcp/src/handlers/tools/mod.rs` dispatch table
    - Confirm retired tool aliases (`mcp__cgreality__reality_run_attempt`) remain fail-closed with CCREALITY_ENGINE_RETIRED — do NOT add a compatibility shim for the new tool name
    - Add integration tests in `crates/context-graph-mcp/tests/mejepa_verify_integration.rs`:
      - `mejepa_verify_round_trip_approve`: synthesize a request → response decodes to `verdict: "approve"`
      - `mejepa_verify_escalate_returns_failed_gate`: synthesize a SHA-drifting bundle → `verdict: "escalate_to_human"` with `failed_gate.kind: "source_sha_drift"`
      - `mejepa_verify_include_provenance_returns_bundle`: include_provenance: true → response has `provenance` field with all fields
      - `mejepa_verify_include_provenance_unloadable_components_emit_warnings`: mock unloadable calibration → provenance.calibration_version: null + warnings array
      - `mejepa_verify_rejects_extra_fields`: request with unlisted top-level field → JSON-RPC validation error
      - `mejepa_verify_rejects_path_traversal`: PatchBundle with `../etc/passwd` → SourceShaDrift error
      - `mejepa_verify_rejects_oversized_problem_statement`: 65537-byte problem_statement → validation error
      - `retired_reality_run_attempt_returns_engine_retired`: invoke retired tool name → CCREALITY_ENGINE_RETIRED
  </in_scope>
  <out_of_scope>
    - mejepa_predict_latest (TASK-INFER-012)
    - infer-test CLI (TASK-INFER-013)
    - FSV harness (TASK-INFER-014)
    - Live wiring against real predictor weights — tests use mocks
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mcp/src/handlers/tools/mejepa.rs">
use std::path::Path;
use context_graph_mejepa::compiler::MeJepaCompiler;

pub const TOOL_NAME_VERIFY: &amp;str = "mcp__cgreality__mejepa_verify";

pub fn schema_verify() -&gt; serde_json::Value;

pub async fn handle_mejepa_verify(
    args: serde_json::Value,
    compiler: &amp;MeJepaCompiler,
    repo_root: &amp;Path,
) -&gt; Result&lt;serde_json::Value, JsonRpcError&gt;;
    </signature>
  </signatures>
  <constraints>
    - Tool name namespace-locked to `mcp__cgreality__mejepa_verify` per constitution mcp_tool_namespace_lock
    - JSON schema: `"additionalProperties": false` on every nested object (audit-9 lesson)
    - JSON schema: `problem_statement` maxLength 65536; `pre_patch_sha`/`post_patch_sha` `pattern: "^[0-9a-f]{64}$"`; `witness_chain_segment` base64-encoded with hard cap 1MB
    - Path canonicalization at boundary BEFORE calling verify (defense in depth — verify also validates internally)
    - Retired tools (`reality_run_attempt`, `reality_drip_attempt`, etc.) MUST remain fail-closed with code `CCREALITY_ENGINE_RETIRED`; this task does NOT add backward-compat aliasing
    - Error responses carry `error_code` (SCREAMING_SNAKE per .code()), `message` (e.to_string()), `payload` (struct with relevant fields per error variant)
    - include_provenance default: false; explicit true required to opt in
    - All handler paths log the tool name + caller info via `tracing::info!` before delegating
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mcp` succeeds
    - `cargo test -p context-graph-mcp mejepa_verify_integration` shows ≥ 8 passing tests
    - `cargo clippy -p context-graph-mcp -- -D warnings` clean
    - JSON schema validates round-trip on synthetic Approve bundle
    - additionalProperties: false rejects extra top-level fields
    - Path traversal `../etc/passwd` rejected at boundary
    - Retired tool fails closed with CCREALITY_ENGINE_RETIRED
  </verification>
</definition_of_done>

<pseudo_code>
pub async fn handle_mejepa_verify(args, compiler, repo_root) -> Result<Value, JsonRpcError> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Req {
        patch: PatchBundle,
        context: TaskContext,
        #[serde(default)]
        include_provenance: bool,
    }

    let req: Req = serde_json::from_value(args)
        .map_err(|e| JsonRpcError::invalid_params(format!("schema validation: {e}")))?;

    if req.context.problem_statement.len() > 65_536 {
        return Err(JsonRpcError::invalid_params("problem_statement too long"));
    }
    for hunk in &req.patch.ast_diff.hunks {
        gates::canonicalize_under_root(&hunk.path, repo_root)
            .map_err(|e| JsonRpcError::application(e.code(), e.to_string()))?;
    }

    tracing::info!(tool = TOOL_NAME_VERIFY, task_id = ?req.context.task_id, "mejepa_verify");

    let verdict = match compiler.verify(&req.patch, &req.context) {
        Ok(v) => v,
        Err(e) => {
            e.log_context(file!());
            return Err(JsonRpcError::application(e.code(), e.to_string()));
        }
    };

    let mut out = serialize_verdict(&verdict);
    if req.include_provenance {
        let prov = build_provenance_bundle(compiler, &verdict).await;
        out["provenance"] = serde_json::to_value(prov)?;
    }
    Ok(out)
}

fn build_provenance_bundle(compiler, verdict) -> ProvenanceBundle {
    let mut warnings = Vec::new();
    let calibration_version = match compiler.calibration.load_active() {
        Ok(r) => Some(r.version.clone()),
        Err(_) => { warnings.push("calibration_version unloadable".into()); None }
    };
    let constellation_version = match compiler.constellation.version_id() {
        Some(v) => Some(v),
        None => { warnings.push("constellation_version unloadable".into()); None }
    };
    ProvenanceBundle {
        panel_sha: sha256_of_panel(verdict.reality_prediction()),
        embedder_versions: load_embedder_versions(),
        calibration_version,
        training_cert_chain_offset: compiler.storage.witness_offset(),
        constellation_version,
        code_version: format!("{}+{}", env!("CARGO_PKG_VERSION"), git_sha()),
        provenance_warnings: warnings,
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa.rs">handle_mejepa_verify + schema_verify + provenance builder + 8 integration tests</file>
  <file path="crates/context-graph-mcp/tests/mejepa_verify_integration.rs">end-to-end JSON-RPC tests with mocked compiler</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mcp/src/handlers/tools/mod.rs">register `mejepa::handle_mejepa_verify` in dispatch table; export `TOOL_NAME_VERIFY`</file>
  <file path="crates/context-graph-mcp/Cargo.toml">add `context-graph-mejepa` path dep + `tracing` + dev-dep mocks</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mcp zero warnings</criterion>
  <criterion>cargo test -p context-graph-mcp mejepa_verify shows ≥ 8 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mcp -- -D warnings clean</criterion>
  <criterion>Tool name namespace-locked to mcp__cgreality__mejepa_verify</criterion>
  <criterion>additionalProperties: false rejects extra fields</criterion>
  <criterion>Path traversal rejected at boundary</criterion>
  <criterion>Retired reality_run_attempt fails closed with CCREALITY_ENGINE_RETIRED</criterion>
  <criterion>include_provenance: true populates provenance + warnings</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mcp</command>
  <command>cargo test -p context-graph-mcp mejepa_verify</command>
  <command>cargo clippy -p context-graph-mcp -- -D warnings</command>
</test_commands>

</task_spec>
```

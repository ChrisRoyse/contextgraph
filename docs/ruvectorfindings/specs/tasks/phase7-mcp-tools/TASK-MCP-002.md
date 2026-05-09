<task_spec id="TASK-MCP-002" version="1.0">
<metadata>
  <title>Logic — `mejepa_verify` MCP shim that delegates to `MeJepaCompiler::verify` (body owned by SPEC-INFERENCE)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>2</sequence>
  <implements>
    <item ref="REQ-SHIFT-07">`mejepa_verify` registered + responds; `additionalProperties: false`</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#api_contracts[mejepa_verify]">request/response wire contract</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#component_contracts[MejepaVerifyHandler]">behaviour: parse → dispatch → wrap with subscriber state</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#implementation_notes[cross_cutting_inference]">~50-line shim; seven-gate body owned by SPEC-INFERENCE</item>
    <item ref="SEC-MEJEPA-01">Path canonicalization at MCP boundary; reject `../`, control chars</item>
  </implements>
  <depends_on>
    <task_ref>TASK-MCP-001</task_ref>
    <!-- Cross-phase: subscriber state snapshot reader from sister TASK-SHIFT-007 -->
    <task_ref>TASK-SHIFT-007</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>2 files</estimated_files>
</metadata>

<context>
The `mejepa_verify` MCP tool is the operator-facing surface for the seven-gate verify ladder. The seven-gate body (OOD detection, conformal calibration, Granger F-test, witness replay, SHA pin, Gτ guard, etc.) is owned by SPEC-INFERENCE. This task ships ONLY the MCP shim: parse args (PatchBundle + TaskContext + include_provenance), canonicalize paths, dispatch to `MeJepaCompiler::verify(&amp;patch, &amp;context)` via an injected `Arc<dyn MeJepaVerifier>` trait, attach subscriber state (`last_consumed_shift_id` + `subscriber_lag_at_call_time` from TASK-SHIFT-007's metrics snapshot), serialize the verdict + optional ProvenanceBundle. The shim itself is ~50 lines per `<implementation_notes>`.
</context>

<input_context_files>
  <file purpose="api_contract">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#api_contracts[mejepa_verify]</file>
  <file purpose="component_contract">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#component_contracts[MejepaVerifyHandler]</file>
  <file purpose="cross_cutting_note">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#implementation_notes (cross_cutting_inference row)</file>
  <file purpose="security_rule">docs/ruvectorfindings/specs/constitution.md#security_requirements (SEC-MEJEPA-01)</file>
  <file purpose="prior_skeleton">docs/ruvectorfindings/specs/tasks/phase7-mcp-tools/TASK-MCP-001.md</file>
  <file purpose="subscriber_state_snapshot">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-007.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-MCP-001 complete: handler tree skeleton + VerifyArgs DTO + stub `handle()` exist</check>
  <check>TASK-SHIFT-007 complete: `SubscriberMetrics` exposes `last_consumed_shift_id_per_session()` and `current_lag()` accessors</check>
  <check>Cross-cutting trait `MeJepaVerifier` is declared in this crate as `Arc<dyn MeJepaVerifier + Send + Sync>` injection point — concrete impl ships in `crates/context-graph-mejepa/` (SPEC-INFERENCE) at a later phase</check>
</prerequisites>

<scope>
  <in_scope>
    - Replace stub body in `crates/context-graph-mcp/src/handlers/tools/mejepa/verify.rs` with the real shim
    - Declare cross-cutting trait `MeJepaVerifier` (in `crates/context-graph-mcp/src/handlers/tools/mejepa/traits.rs` — NEW file shared with subsequent MCP tasks):
      `pub trait MeJepaVerifier: Send + Sync { async fn verify(&amp;self, patch: PatchBundle, context: TaskContext) -&gt; Result<VerifyVerdict, McpError>; }`
    - Carrier types `PatchBundle`, `TaskContext`, `VerifyVerdict`, `ProvenanceBundle` defined in `traits.rs` as opaque structs whose internal fields are owned by SPEC-INFERENCE — this task only declares the wire shape needed for serialization (deny_unknown_fields, path-canonicalized fields)
    - Path canonicalization helper `canonicalize_or_reject(path: &amp;str) -&gt; Result<PathBuf, McpError>` used on every path field inside PatchBundle (rejects `../`, control characters per SEC-MEJEPA-01)
    - Dispatch: `let verdict = ctx.mejepa_verifier.verify(patch, context).await?;`
    - Attach subscriber snapshot: `last_consumed_shift_id` (from `ctx.subscriber_metrics.last_consumed_per_session()`) and `subscriber_lag_at_call_time` (from `ctx.subscriber_metrics.current_lag()`)
    - Serialize: `{ verdict, provenance: Option<ProvenanceBundle>, subscriber_state: { last_consumed_shift_id, subscriber_lag } }`
    - `include_provenance` flag drives whether `provenance` is populated (false → null)
    - Unit tests with a mock `MeJepaVerifier` impl in `#[cfg(test)] mod` (real trait, not mock data); 4 path-traversal payloads asserted-rejected; success-path test asserts the response shape
  </in_scope>
  <out_of_scope>
    - The seven-gate verify body itself (SPEC-INFERENCE)
    - Conformal calibration, OOD detection, Granger F-test (SPEC-INFERENCE)
    - Wiring `Arc<dyn MeJepaVerifier>` into the MCP server bootstrap (covered when SPEC-INFERENCE ships its concrete impl)
    - FSV roundtrip (TASK-MCP-007)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mcp/src/handlers/tools/mejepa/traits.rs">
      use std::sync::Arc;
      use async_trait::async_trait;

      #[async_trait]
      pub trait MeJepaVerifier: Send + Sync {
          async fn verify(
              &amp;self,
              patch: PatchBundle,
              context: TaskContext,
          ) -&gt; Result&lt;VerifyVerdict, crate::error::McpError&gt;;
      }

      #[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
      #[serde(deny_unknown_fields)]
      pub struct PatchBundle { /* opaque to this task; fields owned by SPEC-INFERENCE */ }

      #[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
      #[serde(deny_unknown_fields)]
      pub struct TaskContext { /* opaque to this task; fields owned by SPEC-INFERENCE */ }

      #[derive(serde::Serialize, Debug, Clone)]
      pub struct VerifyVerdict { /* opaque to this task */ }

      #[derive(serde::Serialize, Debug, Clone)]
      pub struct ProvenanceBundle {
          pub corpus_sha: [u8; 32],
          pub embedder_versions: std::collections::HashMap&lt;String, String&gt;,
          pub frozen_at: i64,
          pub code_version: String,
      }
    </signature>
    <signature file="crates/context-graph-mcp/src/handlers/tools/mejepa/verify.rs">
      pub async fn handle(
          args: serde_json::Value,
          ctx: &amp;crate::handlers::McpContext,
      ) -&gt; Result&lt;serde_json::Value, crate::error::McpError&gt; {
          let parsed: VerifyArgs = parse_args_strict(args)?;
          let patch    = canonicalize_patch_paths(parsed.patch)?;
          let context  = canonicalize_context_paths(parsed.context)?;
          let verdict  = ctx.mejepa_verifier.verify(patch, context).await?;
          let provenance = if parsed.include_provenance {
              Some(ctx.provenance_snapshot()?)
          } else { None };
          let last_consumed_shift_id  = ctx.subscriber_metrics.last_consumed_per_session();
          let subscriber_lag          = ctx.subscriber_metrics.current_lag();
          Ok(serde_json::json!({
              "verdict": verdict,
              "provenance": provenance,
              "subscriber_state": {
                  "last_consumed_shift_id": last_consumed_shift_id,
                  "subscriber_lag_at_call_time": subscriber_lag,
              },
          }))
      }

      fn canonicalize_or_reject(path: &amp;str) -&gt; Result&lt;std::path::PathBuf, crate::error::McpError&gt;;
      fn canonicalize_patch_paths(p: serde_json::Value)   -&gt; Result&lt;PatchBundle, crate::error::McpError&gt;;
      fn canonicalize_context_paths(c: serde_json::Value) -&gt; Result&lt;TaskContext, crate::error::McpError&gt;;
    </signature>
  </signatures>

  <constraints>
    - The shim body is ≤ 100 LOC excluding declarations + tests (per `<implementation_notes>` "~50 lines"); if it grows past 100, fields are leaking from SPEC-INFERENCE — refactor instead
    - All input parsing routed through `reality_loop::schema_validator` (SHIPPED) — never roll a per-handler unknown-field check
    - Path canonicalization MUST reject: any segment equal to `..`; any control character (0x00-0x1F or 0x7F); any symlink whose target escapes the configured repo root; any path containing `\x00`
    - Path canonicalization SHALL surface `MEJEPA_INFER_SOURCE_SHA_DRIFT` (per NFR-SHIFT-09) on rejection — match the existing failure taxonomy
    - `include_provenance: false` → response field `provenance` is exactly `null` (not omitted; deterministic shape)
    - Subscriber state snapshot is BEST-EFFORT: if `subscriber_metrics` is unavailable (e.g., subscriber Tokio task dead), respond `subscriber_state: { last_consumed_shift_id: {}, subscriber_lag_at_call_time: 0 }` rather than failing the verify call (debuggability) — but log `tracing::warn!` with `subscriber_unavailable=true`
    - This task MUST NOT touch the seven-gate body or any `crates/context-graph-mejepa/` file (SPEC-INFERENCE owned)
    - File length under 500 lines
  </constraints>

  <verification>
    - Test 1 (success path): mock `MeJepaVerifier` returns `VerifyVerdict::Pass`; response JSON has `verdict.kind == "Pass"`, `provenance == null`, `subscriber_state` populated
    - Test 2 (include_provenance: true): `provenance` is populated with all four fields (corpus_sha hex string, embedder_versions HashMap, frozen_at i64, code_version string)
    - Test 3 (path-traversal rejection): 4 attack payloads `../etc/passwd`, `subdir/../../escape`, `\x00null-byte`, symlink-escape — all rejected with `MEJEPA_INFER_SOURCE_SHA_DRIFT`
    - Test 4 (unknown field rejection): args with extra `evil_extra: 1` rejected by `deny_unknown_fields`
    - Test 5 (subscriber dead): mock metrics returns `task_alive_since: None`; verify still succeeds; response carries empty `last_consumed_shift_id` map; `subscriber_lag_at_call_time: 0`; `tracing::warn!` recorded
    - Test 6 (verifier error): mock returns `McpError::structured("MEJEPA_INFER_OOD_REFUSE", ...)`; the error is propagated unchanged (the shim does NOT swallow errors)
    - `cargo build --release -p context-graph-mcp` clean
    - `cargo clippy -p context-graph-mcp -- -D warnings` clean
  </verification>
</definition_of_done>

<pseudo_code>
verify::handle(args, ctx):
  let parsed = schema_validator::parse_strict::<VerifyArgs>(args)?;       // deny_unknown_fields enforced
  let patch   = canonicalize_patch_paths(parsed.patch)?;                  // rejects ../, control chars
  let context = canonicalize_context_paths(parsed.context)?;              // same
  let verdict = ctx.mejepa_verifier.verify(patch, context).await?;        // SPEC-INFERENCE owned body
  let provenance = if parsed.include_provenance { Some(ctx.provenance_snapshot()?) } else { None };
  let lcsid = ctx.subscriber_metrics.last_consumed_per_session();          // best-effort; empty map if dead
  let lag   = ctx.subscriber_metrics.current_lag();
  Ok(serde_json::json!({
    "verdict": verdict,
    "provenance": provenance,
    "subscriber_state": {
      "last_consumed_shift_id": lcsid,
      "subscriber_lag_at_call_time": lag,
    }
  }))

canonicalize_or_reject(path):
  if path.contains('\x00') || path.bytes().any(|b| b < 0x20 || b == 0x7F) { return Err(MEJEPA_INFER_SOURCE_SHA_DRIFT) }
  let p = std::path::Path::new(path);
  let canonical = p.canonicalize().map_err(|_| MEJEPA_INFER_SOURCE_SHA_DRIFT)?;
  if !canonical.starts_with(ctx.repo_root) { return Err(MEJEPA_INFER_SOURCE_SHA_DRIFT) }
  Ok(canonical)
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa/traits.rs">MeJepaVerifier trait + carrier types (PatchBundle, TaskContext, VerifyVerdict, ProvenanceBundle)</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa/mod.rs">add `pub mod traits;` declaration</file>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa/verify.rs">replace stub body with the real shim + path-canonicalization helpers + 6 unit tests</file>
  <file path="crates/context-graph-mcp/Cargo.toml">add `async-trait` workspace dep if not present (for `#[async_trait]` on MeJepaVerifier)</file>
</files_to_modify>

<validation_criteria>
  <criterion>All 6 unit tests pass</criterion>
  <criterion>Path-traversal rejection covers `..`, control bytes, null byte, symlink-escape</criterion>
  <criterion>`include_provenance` flag deterministically toggles the response field</criterion>
  <criterion>Subscriber-dead branch is debuggable (warn-logged, response still returns)</criterion>
  <criterion>Shim body ≤ 100 LOC excluding declarations + tests</criterion>
  <criterion>No file exceeds 500 lines</criterion>
  <criterion>This task does NOT modify any file under `crates/context-graph-mejepa/`</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mcp</command>
  <command>cargo test -p context-graph-mcp mejepa::verify</command>
  <command>cargo clippy -p context-graph-mcp -- -D warnings</command>
</test_commands>
</task_spec>

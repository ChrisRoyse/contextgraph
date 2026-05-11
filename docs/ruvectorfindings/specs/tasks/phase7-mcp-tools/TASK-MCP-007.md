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

## 📋 2026-05-11 STATE AUDIT (sharpened against verified codebase)

> **Codebase reality table:** see `_index.md` in this directory.

**This task ships the surface FSV harness.**

**Drift from original task body:**
- "FSV for all 5 tools + retired-tool round-trip" is now "FSV for the 3 NEW tools (`observe_shift`, `subscriber_status`, `capture_audit`) + 2 HARDENED tools (`verify`, `predict_latest`) + retired-name fail-closed regression on the 5-name retired set".
- Existing FSV evidence directory pattern in this codebase: `/tmp/contextgraph-mejepa-<phase-or-feature>-fsv/<sub-feature>/<artifact>.json`. Use `/tmp/contextgraph-mejepa-phase7-mcp-fsv/`.
- Per SEC-MEJEPA-07, file mode `0o600`.

**Decided scope for this task:**
1. New example binary at `crates/context-graph-mcp/examples/mejepa_phase7_mcp_roundtrip_fsv.rs` (clean-room, provenance comment per CLAUDE.md).
2. The example spawns the full MCP server (or the handler dispatch surface in-process) against a tempdir-backed RocksDB and a synthetic `SubscriberMetrics`/`MejepaStore`. For each of the 3 NEW tools + 2 HARDENED tools: invoke via JSON-RPC envelope, assert response shape, write `/tmp/contextgraph-mejepa-phase7-mcp-fsv/<tool_name>/{happy_path,empty_session,oversize_payload,invalid_args}.json`.
3. For the retired-name set: invoke each of `mcp__cgreality__reality_run_attempt`, `_alter_file`, `_alter_run`, `_alter_apply`, `_drip_attempt` → assert response is `{"error": {"code": "CCREALITY_ENGINE_RETIRED", "message": "...", "payload": {"attempted_tool": "..."}}}` → write `/tmp/contextgraph-mejepa-phase7-mcp-fsv/retired/{tool_name}.json`.
4. Per `<fsv_protocol>` in constitution: every evidence file is read back from disk after writing → asserted byte-equal to the in-memory representation → cite the readback assertion in the example output. SHA-256 of each evidence file recorded into a top-level `manifest.json`.
5. NO mock data — use real `RocksDbInferStore` against a tempdir. NO test that passes when the system is broken (e.g., if a tool is renamed or a schema breaks, the FSV must fail).

**Cross-phase dep:** sister TASK-SHIFT-013 publishes the upstream subscriber FSV that provides the synthetic shift-log fixture for `observe_shift` and `capture_audit`.

**FSV evidence:**
- `/tmp/contextgraph-mejepa-phase7-mcp-fsv/manifest.json` — SHA-256s of every artifact + tool roster
- `/tmp/contextgraph-mejepa-phase7-mcp-fsv/<tool_name>/<scenario>.json` — one per tool × scenario
- `/tmp/contextgraph-mejepa-phase7-mcp-fsv/retired/<tool_name>.json` — one per retired tool

---

<task_spec id="TASK-MCP-007" version="1.0">
<metadata>
  <title>Surface — FSV: MCP roundtrip evidence for all 5 mejepa_* tools + retired-tool fail-closed (CCREALITY_ENGINE_RETIRED)</title>
  <status>ready</status>
  <layer>surface</layer>
  <sequence>7</sequence>
  <implements>
    <item ref="REQ-SHIFT-07">Each new tool round-trips over JSON-RPC with documented response shape</item>
    <item ref="REQ-SHIFT-13">Retired surface fail-closed `CCREALITY_ENGINE_RETIRED` round-trip captured</item>
    <item ref="TC-SHIFT-08">5 new MCP tools register, respond, and reject `additionalProperties`</item>
    <item ref="TC-SHIFT-09">Retired `reality_run_attempt` returns `CCREALITY_ENGINE_RETIRED`</item>
    <item ref="constitution#fsv_protocol">FSV evidence written + re-read + assert before reporting success; mode 0o600</item>
    <item ref="SEC-MEJEPA-07">FSV evidence files in /tmp/ created with mode 0o600</item>
    <item ref="testing_requirements[fsv_evidence_paths]">/tmp/contextgraph-mejepa-mcp-tools-fsv/* mode 0o600</item>
  </implements>
  <depends_on>
    <task_ref>TASK-MCP-002</task_ref>
    <task_ref>TASK-MCP-003</task_ref>
    <task_ref>TASK-MCP-004</task_ref>
    <task_ref>TASK-MCP-005</task_ref>
    <task_ref>TASK-MCP-006</task_ref>
    <!-- Cross-phase: subscriber FSV for the test fixture pattern -->
    <task_ref>TASK-SHIFT-013</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>2 files</estimated_files>
</metadata>

<context>
The surface FSV harness for Phase 7 MCP tools. A `cargo run --release --example mejepa_mcp_tools_fsv -p context-graph-mcp` binary stands up an in-process MCP server (no stdio transport — uses the existing in-process dispatcher), populates the four ME-JEPA RocksDB CFs (CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_ORACLE_VERDICTS, CF_MEJEPA_TRAIN_CERTS, CF_MEJEPA_PANELS) with synthetic-but-realistic rows, registers test-trait collaborators (mock `MeJepaVerifier`, mock `SubscriberIndex`, real `WatermarkReader`, real `LivePredictionWriter`), and drives one happy-path JSON-RPC round-trip per new tool plus one fail-closed round-trip per retired tool name.

Output: 6 evidence files under `/tmp/contextgraph-mejepa-mcp-tools-fsv/` (mode 0o600 per SEC-MEJEPA-07):
- `verify-roundtrip.json`           — request + response + verdict shape
- `predict_latest-roundtrip.json`   — request + response with N predictions
- `observe_shift-roundtrip.json`    — request + response with replay outcome
- `subscriber_status-roundtrip.json` — request + response with all status fields
- `capture_audit-roundtrip.json`    — request + response with full bundle (page 0)
- `retired-fail-closed.json`        — round-trip on `reality_run_attempt` showing `CCREALITY_ENGINE_RETIRED` error

Each file is re-read after writing and asserted against the documented response shape per `<fsv_protocol>`. Final stdout line `FSV-OK: mejepa_mcp_tools` on success.

This task ALSO writes the 5th file referenced (but explicitly NOT produced) by sister TASK-SHIFT-013: `mcp-tool-roundtrip-evidence.json` — written under `/tmp/contextgraph-mejepa-shift-subscriber-fsv/` (the sister harness's evidence dir) so the sister harness's "presence assertion" passes.
</context>

<input_context_files>
  <file purpose="fsv_protocol">docs/ruvectorfindings/specs/constitution.md#fsv_protocol</file>
  <file purpose="evidence_paths">docs/ruvectorfindings/specs/constitution.md#testing_requirements[fsv_evidence_paths]</file>
  <file purpose="security_mode_rule">docs/ruvectorfindings/specs/constitution.md#security_requirements (SEC-MEJEPA-07)</file>
  <file purpose="retired_contract">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-003.md</file>
  <file purpose="sister_fsv">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-013.md</file>
  <file purpose="all_handlers">docs/ruvectorfindings/specs/tasks/phase7-mcp-tools/TASK-MCP-002.md..TASK-MCP-006.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-MCP-002..006 complete: all 5 handlers ship real bodies (no `MEJEPA_TOOL_STUB_NOT_WIRED` returns)</check>
  <check>TASK-SHIFT-013 complete: sister FSV harness pattern exists; `FsvFixture` builder helper available for re-use</check>
  <check>The MCP server provides an in-process dispatcher entry point: `crate::handlers::dispatch_in_process(tool_name, args, ctx)` (existing pattern, used by other test/example binaries)</check>
  <check>Test trait impls (mock `MeJepaVerifier`, etc.) reachable from `pub(crate)` test helpers (or via dev-dep on a test-utils crate)</check>
</prerequisites>

<scope>
  <in_scope>
    - `crates/context-graph-mcp/examples/mejepa_mcp_tools_fsv.rs` — FSV harness binary
    - `crates/context-graph-mcp/examples/mejepa_fsv_fixtures.rs` — synthetic-data fixture builders (one helper per tool):
      - `populate_live_predictions(db, session_id, n) -> ()`
      - `populate_oracle_verdicts(db, task_id, mut_cat) -> ()`
      - `populate_train_certs(db, step) -> ()`
      - `populate_panels(db, attempt_id, timesteps) -> ()`
      - `build_subscriber_index(...) -> Arc<SubscriberIndex>`
      - `build_test_mejepa_verifier() -> Arc<dyn MeJepaVerifier>`
    - Stand up an in-memory tempdir RocksDB with all required CFs
    - Build an `McpContext` populated with: `Arc<RocksDbStore>`, `Arc<SubscriberMetrics>`, `Arc<SubscriberIndex>`, `Arc<dyn MeJepaVerifier>` (mock), `Arc<dyn LivePredictionWriter>`, `Arc<WatermarkReader>`, `replay_tx: mpsc::Sender<ReplayRequest>` (paired with a mock receiver task that responds Ok)
    - Drive 6 round-trips:
      1. `mejepa_verify {patch, context, include_provenance: true}` → asserts verdict shape + provenance populated + subscriber_state populated
      2. `mejepa_predict_latest {session_id: "sess-fsv", limit: 3}` → asserts 3 rows in DESC offset order
      3. `mejepa_observe_shift {shift_id: "01J..."}` → asserts `shift_found: true` + replay_outcome
      4. `mejepa_subscriber_status {}` → asserts all documented fields present + finite
      5. `mejepa_capture_audit {attempt_id: "A1", page: 0}` → asserts `attempt_found: true`, full bundle, total_pages ≥ 1
      6. `reality_run_attempt {}` → asserts error response carries `CCREALITY_ENGINE_RETIRED` + retired-tool message
    - Write 6 evidence files (mode 0o600, Unix-only; non-Unix prints stderr warning per the sister FSV pattern)
    - Re-read each file; assert documented invariants:
      - `verify-roundtrip.json`: response.verdict.kind matches enum variant; response.provenance is Object (not null) when include_provenance=true; response.subscriber_state has both fields
      - `predict_latest-roundtrip.json`: `session_known: true`, predictions.length == 3, witness_chain_offsets are strictly DESC
      - `observe_shift-roundtrip.json`: `shift_found: true`, replay_outcome.path matches mock outcome
      - `subscriber_status-roundtrip.json`: every documented field present (lag, lag_alert_active, error_rate_per_1000, instrument_cache_hit_rate, capacity, entries, rss_bytes, task_alive_since, dropped_l_step_below_threshold_count, last_panic, last_watermark_per_session)
      - `capture_audit-roundtrip.json`: `attempt_found: true`; bundle has at least one populated panel timestep; missing_slots is Vec (possibly empty); total_pages ≥ 1
      - `retired-fail-closed.json`: response.error.code == "CCREALITY_ENGINE_RETIRED"; payload.attempted_tool == "reality_run_attempt"
    - ALSO write `/tmp/contextgraph-mejepa-shift-subscriber-fsv/mcp-tool-roundtrip-evidence.json` — a roll-up of the 6 round-trips so sister TASK-SHIFT-013's presence-assertion passes
    - Print `FSV-OK: mejepa_mcp_tools` on success; non-zero exit on any assert failure
    - Document 5 retired tool aliases in the harness (`reality_run_attempt`, `reality_alter_create_file`, `reality_alter_edit`, `reality_alter_write`, `reality_alter_delete`); round-trip each and assert ALL return `CCREALITY_ENGINE_RETIRED` (regression guard against accidental aliasing/shimming)
  </in_scope>
  <out_of_scope>
    - The handler bodies themselves (TASK-MCP-002..006)
    - The subscriber's stress-test FSV (TASK-SHIFT-013)
    - The Phase 7 DoD verify-CLI (sister TASK-SHIFT-015 / SPEC-INFERENCE)
    - Real predictor weights (mock verifier deterministic)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mcp/examples/mejepa_mcp_tools_fsv.rs">
      #[tokio::main]
      async fn main() -&gt; anyhow::Result&lt;()&gt; {
          let evidence_dir = std::path::Path::new("/tmp/contextgraph-mejepa-mcp-tools-fsv/");
          std::fs::create_dir_all(evidence_dir)?;

          let (db, ctx) = build_mejepa_test_context().await?;
          populate_synthetic_data(&amp;db).await?;

          let verify_rt           = roundtrip_verify(&amp;ctx).await?;
          let predict_latest_rt   = roundtrip_predict_latest(&amp;ctx).await?;
          let observe_shift_rt    = roundtrip_observe_shift(&amp;ctx).await?;
          let subscriber_status_rt = roundtrip_subscriber_status(&amp;ctx).await?;
          let capture_audit_rt    = roundtrip_capture_audit(&amp;ctx).await?;
          let retired_rt          = roundtrip_retired_tools(&amp;ctx).await?;

          write_evidence_0o600(evidence_dir.join("verify-roundtrip.json"), &amp;verify_rt)?;
          write_evidence_0o600(evidence_dir.join("predict_latest-roundtrip.json"), &amp;predict_latest_rt)?;
          write_evidence_0o600(evidence_dir.join("observe_shift-roundtrip.json"), &amp;observe_shift_rt)?;
          write_evidence_0o600(evidence_dir.join("subscriber_status-roundtrip.json"), &amp;subscriber_status_rt)?;
          write_evidence_0o600(evidence_dir.join("capture_audit-roundtrip.json"), &amp;capture_audit_rt)?;
          write_evidence_0o600(evidence_dir.join("retired-fail-closed.json"), &amp;retired_rt)?;

          // Sister FSV roll-up
          let sister_dir = std::path::Path::new("/tmp/contextgraph-mejepa-shift-subscriber-fsv/");
          std::fs::create_dir_all(sister_dir)?;
          write_evidence_0o600(sister_dir.join("mcp-tool-roundtrip-evidence.json"), &amp;mcp_rollup(&amp;[verify_rt.clone(), predict_latest_rt.clone(), observe_shift_rt.clone(), subscriber_status_rt.clone(), capture_audit_rt.clone(), retired_rt.clone()]))?;

          assert_re_read_invariants(evidence_dir)?;
          println!("FSV-OK: mejepa_mcp_tools");
          Ok(())
      }

      async fn roundtrip_verify(ctx: &amp;McpContext)             -&gt; anyhow::Result&lt;serde_json::Value&gt;;
      async fn roundtrip_predict_latest(ctx: &amp;McpContext)     -&gt; anyhow::Result&lt;serde_json::Value&gt;;
      async fn roundtrip_observe_shift(ctx: &amp;McpContext)      -&gt; anyhow::Result&lt;serde_json::Value&gt;;
      async fn roundtrip_subscriber_status(ctx: &amp;McpContext)  -&gt; anyhow::Result&lt;serde_json::Value&gt;;
      async fn roundtrip_capture_audit(ctx: &amp;McpContext)      -&gt; anyhow::Result&lt;serde_json::Value&gt;;
      async fn roundtrip_retired_tools(ctx: &amp;McpContext)      -&gt; anyhow::Result&lt;serde_json::Value&gt;;

      fn write_evidence_0o600(path: std::path::PathBuf, value: &amp;serde_json::Value) -&gt; anyhow::Result&lt;()&gt;;
      fn assert_re_read_invariants(dir: &amp;std::path::Path) -&gt; anyhow::Result&lt;()&gt;;
      fn mcp_rollup(rts: &amp;[serde_json::Value]) -&gt; serde_json::Value;
    </signature>
  </signatures>

  <constraints>
    - Every evidence file written with mode 0o600 via `std::os::unix::fs::OpenOptionsExt::mode(0o600)` (Unix); on non-Unix print stderr warning once and continue (matches sister TASK-SHIFT-013 pattern)
    - Re-read-and-assert MANDATORY per `<fsv_protocol>`; the binary fails (non-zero exit) on any assert failure
    - Synthetic data deterministic: SplitMix64 PRNG seeded fixed; ULIDs counted; same fixture across runs
    - JSON written via `serde_json::to_string_pretty` with sorted keys (BTreeMap) for byte-stable diff
    - Retired-tool round-trip MUST exercise ALL 5 known retired aliases (`reality_run_attempt`, `reality_alter_create_file`, `reality_alter_edit`, `reality_alter_write`, `reality_alter_delete`) — accidental aliasing of any one to a new tool would be caught by this regression guard
    - Path canonicalization: evidence dir is `/tmp/...` not `~/...`; `std::fs::create_dir_all` before any write
    - Mock `MeJepaVerifier` returns a deterministic `VerifyVerdict::Pass` — real predictor weights NOT required for this surface FSV
    - This task MUST NOT touch handler bodies (any change to a handler belongs in TASK-MCP-002..006)
    - File length under 500 lines per file (split fixture helpers if needed)
    - Reuse `FsvFixture` helper from TASK-SHIFT-013 where the data shapes overlap (avoid duplicating ULID generation, session_id_hash6, etc.)
  </constraints>

  <verification>
    - `cargo run --release --example mejepa_mcp_tools_fsv -p context-graph-mcp` exits 0
    - 6 evidence files present in `/tmp/contextgraph-mejepa-mcp-tools-fsv/` with mode 0o600 (Unix)
    - 1 roll-up evidence file present at `/tmp/contextgraph-mejepa-shift-subscriber-fsv/mcp-tool-roundtrip-evidence.json`
    - Re-read assertions pass for all 6 documented invariants
    - Stdout final line == `FSV-OK: mejepa_mcp_tools`
    - All 5 retired tool aliases return `CCREALITY_ENGINE_RETIRED` (regression guard)
    - `cargo build --release --example mejepa_mcp_tools_fsv -p context-graph-mcp` clean
    - `cargo clippy -p context-graph-mcp --examples -- -D warnings` clean
  </verification>
</definition_of_done>

<pseudo_code>
main:
  evidence_dir = /tmp/contextgraph-mejepa-mcp-tools-fsv/
  fs::create_dir_all(evidence_dir)
  let (db, ctx) = build_mejepa_test_context().await?
  populate_synthetic_data(&db).await?
  for each of 5 new tools:
    let request = build_request_for(tool)
    let response = handlers::dispatch_in_process(tool_name, request, &ctx).await?
    let rt = json!({"request": request, "response": response, "tool": tool_name, "timestamp_unix": now()})
    write_0o600(evidence_dir.join(format!("{tool}-roundtrip.json")), &rt)
  for each of 5 retired aliases:
    let response = handlers::dispatch_in_process(alias, json!({}), &ctx).await
    assert response.is_err() && response.err().code == "CCREALITY_ENGINE_RETIRED"
  write_0o600(evidence_dir.join("retired-fail-closed.json"), &json!({"aliases_tested": 5, "all_returned_ccreality_engine_retired": true, "samples": [...]}))
  // sister roll-up
  fs::create_dir_all("/tmp/contextgraph-mejepa-shift-subscriber-fsv/")
  write_0o600("/tmp/contextgraph-mejepa-shift-subscriber-fsv/mcp-tool-roundtrip-evidence.json", &mcp_rollup(...))
  assert_re_read_invariants(evidence_dir)?
  println!("FSV-OK: mejepa_mcp_tools")

assert_re_read_invariants(dir):
  for each evidence file:
    let v = serde_json::from_reader(fs::File::open(&path)?)?
    match file:
      "verify-roundtrip.json"           => assert v["response"]["verdict"].is_object() && v["response"]["provenance"].is_object() && v["response"]["subscriber_state"].is_object()
      "predict_latest-roundtrip.json"   => let preds = v["response"]["predictions"].as_array()?;
                                           assert preds.len() == 3 && preds[0]["witness_chain_offset"] > preds[1]["witness_chain_offset"] > preds[2]["witness_chain_offset"];
                                           assert v["response"]["session_known"] == true
      "observe_shift-roundtrip.json"    => assert v["response"]["shift_found"] == true && v["response"]["replay_outcome"].is_object()
      "subscriber_status-roundtrip.json" => for f in REQUIRED_STATUS_FIELDS: assert v["response"][f].is_some()
      "capture_audit-roundtrip.json"    => assert v["response"]["attempt_found"] == true && v["response"]["bundle"].is_object() && v["response"]["total_pages"] >= 1
      "retired-fail-closed.json"        => assert v["all_returned_ccreality_engine_retired"] == true
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mcp/examples/mejepa_mcp_tools_fsv.rs">FSV harness binary; 6 roundtrips + 6 evidence files + sister roll-up</file>
  <file path="crates/context-graph-mcp/examples/mejepa_fsv_fixtures.rs">synthetic-data builders (live_predictions, oracle_verdicts, train_certs, panels, subscriber_index)</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mcp/Cargo.toml">add `anyhow` + `tempfile` to [dev-dependencies]; declare both `[[example]]` entries</file>
</files_to_modify>

<validation_criteria>
  <criterion>Binary exits 0 on a clean run</criterion>
  <criterion>6 evidence files in /tmp/contextgraph-mejepa-mcp-tools-fsv/ with mode 0o600 on Unix</criterion>
  <criterion>Sister roll-up file present at /tmp/contextgraph-mejepa-shift-subscriber-fsv/mcp-tool-roundtrip-evidence.json</criterion>
  <criterion>Re-read-and-assert covers every documented invariant per tool</criterion>
  <criterion>All 5 retired tool aliases return CCREALITY_ENGINE_RETIRED (regression guard against shimming)</criterion>
  <criterion>Stdout success line matches `FSV-OK: mejepa_mcp_tools`</criterion>
  <criterion>No file exceeds 500 lines</criterion>
  <criterion>Synthetic data byte-stable across runs (deterministic PRNG)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release --example mejepa_mcp_tools_fsv -p context-graph-mcp</command>
  <command>cargo run --release --example mejepa_mcp_tools_fsv -p context-graph-mcp</command>
  <command>ls -la /tmp/contextgraph-mejepa-mcp-tools-fsv/</command>
  <command>ls -la /tmp/contextgraph-mejepa-shift-subscriber-fsv/mcp-tool-roundtrip-evidence.json</command>
  <command>cargo build --release -p context-graph-mcp</command>
  <command>cargo test -p context-graph-mcp</command>
  <command>cargo clippy -p context-graph-mcp --examples -- -D warnings</command>
</test_commands>
</task_spec>

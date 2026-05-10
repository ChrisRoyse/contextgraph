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

## 📋 2026-05-10 STATE AUDIT — REQUIRES REWORK

**Drift summary:** This task body assumes a sub-module file at `crates/context-graph-mcp/src/handlers/tools/mejepa/capture_audit.rs`. The codebase took a different path: MCP MEJEPA handlers live in flat files (`mejepa_tools.rs` + `embedder_tools.rs`). **`mejepa_capture_audit` is PENDING** — it has NOT been shipped yet.

**Actual file locations (verified 2026-05-10):**

- `crates/context-graph-mcp/src/handlers/tools/mejepa_tools.rs` — flat file containing 5 shipped handlers. **Does NOT contain `capture_audit`.**
- `crates/context-graph-mcp/src/handlers/tools/embedder_tools.rs` — contains 3 more shipped handlers.
- `crates/context-graph-mcp/src/tools/names.rs` — no `MEJEPA_CAPTURE_AUDIT` constant yet.
- The `crates/context-graph-mcp/src/handlers/tools/mejepa/` directory **does not exist**.
- The 4 RocksDB CFs referenced (`CF_MEJEPA_LIVE_PREDICTIONS`, `CF_MEJEPA_ORACLE_VERDICTS`, `CF_MEJEPA_TRAIN_CERTS`, `CF_MEJEPA_PANELS`) need re-verification against the current storage crate.

**MEJEPA MCP tools — SHIPPED (8) vs PENDING (10):**

SHIPPED: `mejepa_verify`, `mejepa_predict_latest`, `mejepa_constellation_inspect`, `mejepa_heal_status`, `mejepa_rollback_to`, `mejepa_load_learner_state`, `mejepa_routing_lookup`, `mejepa_vram_budget_report`.

PENDING (this task targets one): `mejepa_observe_shift`, `mejepa_subscriber_status`, **`mejepa_capture_audit`** (Phase 7); `mejepa_gc_run`, `mejepa_quota_status`, `mejepa_witness_compress` (Phase 6); `mejepa_eval_run`, `mejepa_eval_ship_gate_status`, `mejepa_active_learning_queue`, `mejepa_active_learning_label` (Phase 8).

**Specific path discrepancies in this task body:**

- `<files_to_create>` paths under `crates/context-graph-mcp/src/handlers/tools/mejepa/capture_audit.rs` — non-existent target tree. Should be new `call_mejepa_capture_audit` (+ optional helper module) in the flat-file layout.
- `CaptureAuditBundle` carrier — verify the type does not yet exist in tree (sister TASK-SHIFT-002 / TASK-SHIFT-007 own it).
- `<depends_on>TASK-MCP-001</depends_on>` — TASK-MCP-001 was never implemented as described.
- "high complexity, 2 files" — under flat-file structure, may consolidate to 1 file if under 500 lines, or split as a sibling flat file (`mejepa_capture_audit.rs`).

**Recommended rework before implementing:**

1. Block until Phase 7 shift-subscriber crate ships the writer paths for the 4 CFs (TASK-SHIFT-002, TASK-SHIFT-007) — without populated CFs this handler has nothing to return.
2. Rewrite `<files_to_create>` to add `call_mejepa_capture_audit` as a new function in `mejepa_tools.rs` (or a new sibling flat file if size grows past 500 lines).
3. Add `MEJEPA_CAPTURE_AUDIT` constant to `crates/context-graph-mcp/src/tools/names.rs`.
4. Confirm pagination chunk size + canonicalize-floats behavior against the actual `CaptureAuditBundle` shape once it ships.
5. Confirm dispatcher wiring path — the sub-module dispatch this task assumes does not exist.

---

<task_spec id="TASK-MCP-006" version="1.0">
<metadata>
  <title>Logic — `mejepa_capture_audit` full-signal-capture handler returning every E_* / aux-head / counterfactual for an attempt_id</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>6</sequence>
  <implements>
    <item ref="REQ-SHIFT-07">`mejepa_capture_audit` registered + responds; `additionalProperties: false`</item>
    <item ref="REQ-SHIFT-10">Complete signal capture per attempt_id: panels × timesteps + aux-head predictions + counterfactuals + L_step + per-factor decomposition + bracketing witness offsets + cache hit flags + panel SHAs + embedder versions + missing slots</item>
    <item ref="REQ-SHIFT-18">Provenance metadata in every persisted record (echoed in bundle)</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#api_contracts[mejepa_capture_audit]">request (attempt_id, page) / response (bundle, total_pages, current_page)</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#component_contracts[MejepaCaptureAuditHandler]">behaviour: lookup CFs → assemble → paginate &gt; chunk_size → canonicalize floats</item>
    <item ref="EC-SHIFT-10">Missing slots surfaced explicitly (NOT zero-filled)</item>
    <item ref="NFR-SHIFT-05">p50 &lt; 100 ms on populated attempt_id</item>
    <item ref="CaptureAuditBundle">data entity</item>
  </implements>
  <depends_on>
    <task_ref>TASK-MCP-001</task_ref>
    <!-- Cross-phase: CaptureAuditBundle carrier + writer paths populating CFs -->
    <task_ref>TASK-SHIFT-002</task_ref>
    <task_ref>TASK-SHIFT-007</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>2 files</estimated_files>
</metadata>

<context>
The most expressive of the 5 MCP tools. Returns the COMPLETE signal capture for a given `attempt_id`: every panel slot value across all available timesteps (t=0/1/2), every aux-head prediction, the `RealityPrediction`, the `VerifyVerdict` (if `verify()` was invoked), the `OracleOutcome` (if test artifacts shifted), the L_step plus per-factor ΔP/ΔK/ΔΩ/ΔΞ decomposition, every counterfactual perturbation tested by the predictor, the bracketing witness-chain offsets, per-instrument cache hit/miss flags, the panel SHA-256s, the embedder version pins per E1-E21.

Reads from FOUR column families:
- `CF_MEJEPA_PANELS` keyed by `(attempt_id, time_step)` for t=0/1/2 panel slot vectors
- `CF_MEJEPA_LIVE_PREDICTIONS` keyed by `(session_id derived from attempt, witness_chain_offset)` for the prediction
- `CF_MEJEPA_ORACLE_VERDICTS` keyed by `(task_id, mutation_category)` derived from the attempt
- `CF_MEJEPA_TRAIN_CERTS` for the L_step + factor decomposition associated with the shift

Pagination: bundles can exceed 1 MB for fully-populated attempts. The handler chunks the response by panel slot when serialized JSON exceeds `audit_chunk_size_bytes` (default 1 MB). Pagination is transparent to MCP-spec clients.

Missing-slot policy: each instrument slot may have failed to materialize (BR-SHIFT-06); the bundle's `missing_slots` field SHALL surface those slots explicitly with the underlying error code. NEVER zero-fill.

Determinism: floats canonicalized via `ryu` (round-trip, shortest-roundtrip representation) so byte-stable diffs across replays are meaningful. All keys sorted lexicographically (BTreeMap).

Not-found policy: when CF lookup yields zero rows for `attempt_id`, query the secondary index for the 5 nearest-neighbor attempts by witness_chain_offset distance; respond `attempt_found: false, nearest_neighbors: [...]` (no error).
</context>

<input_context_files>
  <file purpose="api_contract">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#api_contracts[mejepa_capture_audit]</file>
  <file purpose="component_contract">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#component_contracts[MejepaCaptureAuditHandler]</file>
  <file purpose="data_entity">docs/ruvectorfindings/specs/functional/SPEC-SHIFT-SUBSCRIBER.md (CaptureAuditBundle entity definition)</file>
  <file purpose="rocksdb_schema">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#rocksdb_schema</file>
  <file purpose="prior_skeleton">docs/ruvectorfindings/specs/tasks/phase7-mcp-tools/TASK-MCP-001.md</file>
  <file purpose="bundle_carrier_owner">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-002.md</file>
  <file purpose="cf_writers">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-007.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-MCP-001 complete: skeleton + CaptureAuditArgs + stub handle()</check>
  <check>TASK-SHIFT-002 complete: `CaptureAuditBundle` carrier struct + `PanelSnapshot`, `AuxHeadPrediction`, `CounterfactualResult`, `UtmlFactorBundle` types shipped</check>
  <check>TASK-SHIFT-007 complete: writers populating CF_MEJEPA_LIVE_PREDICTIONS + CF_MEJEPA_ORACLE_VERDICTS + CF_MEJEPA_TRAIN_CERTS exist; SubscriberIndex maps attempt_id → coordinates</check>
</prerequisites>

<scope>
  <in_scope>
    - Replace stub body in `crates/context-graph-mcp/src/handlers/tools/mejepa/capture_audit.rs`
    - Helper module `crates/context-graph-mcp/src/handlers/tools/mejepa/audit_assembler.rs` containing:
      - `assemble_bundle(attempt_id, ctx) -&gt; Result<CaptureAuditBundle, McpError>`
      - `lookup_panels_by_attempt(...)`, `lookup_prediction_by_attempt(...)`, `lookup_oracle_by_attempt(...)`, `lookup_train_cert_by_attempt(...)`
      - `nearest_neighbors_by_offset(attempt_id, k=5, ctx) -&gt; Vec<AttemptId>`
      - `paginate_bundle(bundle, page, max_bytes) -&gt; (paginated_value, total_pages, current_page)`
      - `canonicalize_floats(value: serde_json::Value) -&gt; serde_json::Value` using `ryu` for f64 fields
    - Lookup attempt_id → (session_id, witness_chain_offset) via `ctx.subscriber_index.get_attempt(&amp;attempt_id)` (sister carrier from TASK-SHIFT-007)
    - Read panels (t=0/1/2) from CF_MEJEPA_PANELS; missing time_step is normal (e.g., observe path didn't run → no t=2)
    - Read prediction from CF_MEJEPA_LIVE_PREDICTIONS using the 10-byte key (re-using `session_id_hash6` helper from TASK-MCP-003)
    - Read oracle verdict from CF_MEJEPA_ORACLE_VERDICTS using `(task_id, mutation_category)` (extracted from attempt metadata in SubscriberIndex)
    - Read train cert from CF_MEJEPA_TRAIN_CERTS using step number
    - Surface `missing_slots: Vec<InstrumentSlot>` per BR-SHIFT-06 (NEVER zero-fill missing panels)
    - Pagination: serialize bundle; if `serialized.len() > audit_chunk_size_bytes` (config default 1_048_576 = 1 MiB), split by panel slot — slot 0..K on page 0, slot K+1..N on page 1, ...; respond `total_pages` + `current_page` + slice
    - Float canonicalization: walk JSON tree; replace every f64 leaf with its `ryu`-formatted shortest round-trip string-then-parse representation so byte-equal across runs
    - All output keys sorted lexicographically (BTreeMap) at every nesting level
    - Not-found path: 5 nearest-neighbor attempts by witness_chain_offset distance via SubscriberIndex; response carries `attempt_found: false, nearest_neighbors: [...]`
    - `attempt_id` length validation: ≤ 128 chars, UTF-8 sanitized (reject control chars, NUL); structured error `MEJEPA_CAPTURE_AUDIT_ATTEMPT_ID_INVALID` on violation
    - Unit tests with tempdir RocksDB + populated rows: 8 tests (happy, partial-panels-missing, not-found-with-NN, paginate, paginate-page-out-of-range, attempt_id too long, attempt_id with control char, latency)
  </in_scope>
  <out_of_scope>
    - The writer side that populates the 4 CFs (sister phase)
    - The carrier struct definitions (sister TASK-SHIFT-002 owns)
    - Tier-compression on cold panels (SPEC-HYGIENE)
    - The aux-head implementation (SPEC-PREDICTOR)
    - Counterfactual perturbation generation (SPEC-INFERENCE Phase 4)
    - FSV roundtrip (TASK-MCP-007)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mcp/src/handlers/tools/mejepa/capture_audit.rs">
      pub async fn handle(
          args: serde_json::Value,
          ctx: &amp;crate::handlers::McpContext,
      ) -&gt; Result&lt;serde_json::Value, crate::error::McpError&gt;;

      const DEFAULT_AUDIT_CHUNK_SIZE_BYTES: usize = 1_048_576;

      fn validate_attempt_id(s: &amp;str) -&gt; Result&lt;(), crate::error::McpError&gt;;
    </signature>
    <signature file="crates/context-graph-mcp/src/handlers/tools/mejepa/audit_assembler.rs">
      pub fn assemble_bundle(
          attempt_id: &amp;str,
          ctx: &amp;crate::handlers::McpContext,
      ) -&gt; Result&lt;crate::handlers::tools::mejepa::traits::CaptureAuditBundle, crate::error::McpError&gt;;

      pub fn paginate_bundle(
          bundle: &amp;crate::handlers::tools::mejepa::traits::CaptureAuditBundle,
          page: u32,
          max_bytes: usize,
      ) -&gt; Result&lt;(serde_json::Value, u32, u32), crate::error::McpError&gt;;

      pub fn canonicalize_floats(value: serde_json::Value) -&gt; serde_json::Value;

      pub fn nearest_neighbors_by_offset(
          attempt_id: &amp;str,
          k: usize,
          ctx: &amp;crate::handlers::McpContext,
      ) -&gt; Vec&lt;String&gt;;
    </signature>
    <signature file="crates/context-graph-mcp/src/handlers/tools/mejepa/traits.rs">
      // Re-export carrier types from sister phase:
      pub use crate::handlers::tools::mejepa::external::CaptureAuditBundle;
      pub use crate::handlers::tools::mejepa::external::PanelSnapshot;
      pub use crate::handlers::tools::mejepa::external::AuxHeadPrediction;
      pub use crate::handlers::tools::mejepa::external::CounterfactualResult;
      pub use crate::handlers::tools::mejepa::external::UtmlFactorBundle;
      pub use crate::handlers::tools::mejepa::external::OracleOutcome;
    </signature>
  </signatures>

  <constraints>
    - All input parsing routed through SHIPPED `reality_loop::schema_validator`; deny_unknown_fields
    - `attempt_id` ≤ 128 chars, ASCII-printable + dashes/underscores only (regex `^[A-Za-z0-9_\-]{1,128}$`); rejection code `MEJEPA_CAPTURE_AUDIT_ATTEMPT_ID_INVALID`
    - Missing slots policy: explicit `missing_slots: Vec<InstrumentSlot>` field surfaced; the slot vector positions for missing slots are ABSENT from `panels.<timestep>.slot_values` (not zero-filled, not NaN-filled, not present)
    - Float canonicalization MANDATORY: `canonicalize_floats` walks the entire `serde_json::Value` tree; every f64 leaf rewritten via `ryu::Buffer::format(f).to_string().parse::<f64>()` — guarantees stable hex bits across serialize-deserialize cycles
    - All keys at every nesting level sorted via BTreeMap construction (do NOT rely on serde_json::Value ordering, which is HashMap-backed pre-1.0.108)
    - Pagination boundary is per-slot (not per-byte mid-slot); a slot's full vector is on exactly one page
    - `page == 0` is the default; `page > total_pages - 1` returns structured error `MEJEPA_CAPTURE_AUDIT_PAGE_OUT_OF_RANGE`
    - Not-found is NOT an error: respond `{ attempt_found: false, nearest_neighbors: [...up to 5...], bundle: null, total_pages: 0, current_page: 0 }`
    - All 4 CF reads use cached column-family handles (existing pattern); do NOT open per request
    - This task MUST NOT modify the carrier struct definitions or the writer side
    - Latency budget per NFR-SHIFT-05: p50 &lt; 100 ms on populated attempt_id; assert in test
    - File length under 500 lines per file (split assembler if needed)
  </constraints>

  <verification>
    - Test 1 (happy populated): tempdir DB has full population for `attempt_id="A1"` (3 timesteps × 15 slots, prediction, oracle, train cert with 4 factors); response has every documented field; `attempt_found: true`; missing_slots is empty; total_pages == 1
    - Test 2 (partial panels missing): t=2 absent (no observe path ran); response has `panels.t0`, `panels.t1` populated; `panels.t2` ABSENT (not null-filled); `missing_slots: ["E_Oracle"]` since t=2 was the oracle slot
    - Test 3 (not-found with neighbors): query non-existent `attempt_id="ZZZ"`; SubscriberIndex returns 5 nearest by offset; response `attempt_found: false, nearest_neighbors: ["A1","A2",...]`; no error
    - Test 4 (paginate): inflate panel vector sizes so serialized bundle is 2.3 MB; with default 1 MiB max_bytes → total_pages == 3; page=1 returns 2nd-third of slots; page=2 returns last third
    - Test 5 (page out of range): `page=99` on a single-page bundle → `MEJEPA_CAPTURE_AUDIT_PAGE_OUT_OF_RANGE`
    - Test 6 (attempt_id too long): 129-char attempt_id rejected
    - Test 7 (attempt_id control char): `"A\x01B"` rejected
    - Test 8 (determinism): serialize same populated bundle twice (cold call + warm call); SHA-256 of canonical JSON identical
    - Test 9 (latency): 100 calls on populated attempt_id; p50 &lt; 100 ms
    - `cargo build --release -p context-graph-mcp` clean
    - `cargo clippy -p context-graph-mcp -- -D warnings` clean
  </verification>
</definition_of_done>

<pseudo_code>
capture_audit::handle(args, ctx):
  let parsed: CaptureAuditArgs = schema_validator::parse_strict(args)?;
  validate_attempt_id(&parsed.attempt_id)?;

  // Try to assemble a populated bundle
  let bundle_opt = match assemble_bundle(&parsed.attempt_id, ctx) {
    Ok(b) => Some(b),
    Err(e) if e.code() == "MEJEPA_CAPTURE_AUDIT_NOT_FOUND" => None,
    Err(e) => return Err(e),
  };

  match bundle_opt {
    None => {
      // Not-found path
      let nn = nearest_neighbors_by_offset(&parsed.attempt_id, 5, ctx);
      Ok(json!({
        "attempt_found": false,
        "nearest_neighbors": nn,
        "bundle": null,
        "total_pages": 0,
        "current_page": 0,
      }))
    },
    Some(bundle) => {
      let max_bytes = ctx.config.audit_chunk_size_bytes.unwrap_or(DEFAULT_AUDIT_CHUNK_SIZE_BYTES);
      let (sliced, total_pages, current_page) = paginate_bundle(&bundle, parsed.page, max_bytes)?;
      let canonical = canonicalize_floats(sliced);
      Ok(json!({
        "attempt_found": true,
        "bundle": canonical,
        "total_pages": total_pages,
        "current_page": current_page,
      }))
    }
  }

assemble_bundle(attempt_id, ctx):
  let coords = ctx.subscriber_index.get_attempt(attempt_id)
    .ok_or_else(|| McpError::structured("MEJEPA_CAPTURE_AUDIT_NOT_FOUND", json!({"attempt_id": attempt_id})))?;
  let panels = lookup_panels_by_attempt(attempt_id, ctx, &mut missing_slots)?;
  let prediction = lookup_prediction_by_attempt(coords.session_id, coords.witness_chain_offset, ctx)?;
  let oracle = lookup_oracle_by_attempt(coords.task_id, coords.mutation_category, ctx);
  let train_cert = lookup_train_cert_by_attempt(coords.step, ctx);
  let l_step = train_cert.as_ref().map(|c| c.factors.l_step);
  let utml_factors = train_cert.map(|c| c.factors);
  let counterfactuals = panels.t1.map(|t1| extract_counterfactuals(&t1, &prediction)).unwrap_or_default();
  let bracketing = (coords.witness_chain_offset.saturating_sub(1), coords.witness_chain_offset.saturating_add(1));
  let cache_hit_flags = ctx.subscriber_metrics.cache_hit_flags_for_attempt(attempt_id);
  let panel_shas = panels.iter().map(|(t, p)| (t, sha256(&p.slot_values))).collect();
  let embedder_versions = ctx.provenance.embedder_versions.clone();
  Ok(CaptureAuditBundle{ attempt_id, attempt_found: true, panels, missing_slots, aux_head_predictions: prediction.aux_heads.clone(), reality_prediction: Some(prediction), verify_verdict: ..., oracle_outcome: oracle, l_step, utml_factors, counterfactual_perturbations: counterfactuals, bracketing_witness_offsets: bracketing, cache_hit_flags, panel_shas, embedder_versions, nearest_neighbors: vec![] })

paginate_bundle(bundle, page, max_bytes):
  let serialized = serde_json::to_vec(bundle)?;
  if serialized.len() <= max_bytes {
    if page > 0 { return Err(McpError::structured("MEJEPA_CAPTURE_AUDIT_PAGE_OUT_OF_RANGE", ...)); }
    return Ok((serde_json::to_value(bundle)?, 1, 0));
  }
  // Split by slot; estimate slots-per-page from serialized size
  let slots_per_page = max(1, max_bytes / (serialized.len() / total_slot_count(bundle)));
  let total_pages = ((total_slot_count(bundle) + slots_per_page - 1) / slots_per_page) as u32;
  if page >= total_pages { return Err(McpError::structured("MEJEPA_CAPTURE_AUDIT_PAGE_OUT_OF_RANGE", ...)); }
  let slice = bundle.slice_slots(page as usize * slots_per_page, slots_per_page);
  Ok((serde_json::to_value(slice)?, total_pages, page))

canonicalize_floats(value):
  match value {
    Number(n) if n.is_f64() => {
      let f = n.as_f64().unwrap_or(0.0);  // guard the unwrap; clamp non-finite to 0.0 with warn (non-finite should never appear; fail-closed at writer)
      let s = ryu::Buffer::new().format(f).to_string();
      Number(s.parse::<f64>().unwrap_or(0.0).into())
    },
    Object(map) => {
      let mut sorted = BTreeMap::new();
      for (k, v) in map { sorted.insert(k, canonicalize_floats(v)); }
      Object(sorted.into_iter().collect())
    },
    Array(items) => Array(items.into_iter().map(canonicalize_floats).collect()),
    other => other,
  }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa/audit_assembler.rs">assemble_bundle + paginate_bundle + canonicalize_floats + nearest_neighbors_by_offset (split out for &lt;500 LOC compliance)</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa/capture_audit.rs">replace stub with real handler + validate_attempt_id + 9 unit tests</file>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa/mod.rs">add `pub mod audit_assembler;`</file>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa/traits.rs">re-export `CaptureAuditBundle` + sub-types from sister phase</file>
  <file path="crates/context-graph-mcp/Cargo.toml">add `ryu` workspace dep if not present (canonical f64 formatting)</file>
</files_to_modify>

<validation_criteria>
  <criterion>All 9 unit tests pass</criterion>
  <criterion>Determinism test: SHA-256 of canonical JSON equal across two serializations</criterion>
  <criterion>Pagination: 2.3 MB bundle splits into 3 pages with default 1 MiB chunk</criterion>
  <criterion>Page-out-of-range surfaces structured error</criterion>
  <criterion>Not-found returns 5 nearest neighbors and `attempt_found: false`</criterion>
  <criterion>Missing slots ABSENT from panel.slot_values (not zero-filled, not NaN-filled)</criterion>
  <criterion>Latency p50 &lt; 100 ms on populated attempt_id</criterion>
  <criterion>No file exceeds 500 lines</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mcp</command>
  <command>cargo test -p context-graph-mcp mejepa::capture_audit</command>
  <command>cargo test -p context-graph-mcp mejepa::audit_assembler</command>
  <command>cargo clippy -p context-graph-mcp -- -D warnings</command>
</test_commands>
</task_spec>

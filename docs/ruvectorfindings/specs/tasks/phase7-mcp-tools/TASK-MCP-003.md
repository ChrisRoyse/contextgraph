<task_spec id="TASK-MCP-003" version="1.0">
<metadata>
  <title>Logic — `mejepa_predict_latest` CF prefix-scan handler reading CF_MEJEPA_LIVE_PREDICTIONS by session_id</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>3</sequence>
  <implements>
    <item ref="REQ-SHIFT-07">`mejepa_predict_latest` registered + responds; `additionalProperties: false`</item>
    <item ref="REQ-SHIFT-05">Predict-only path persists to CF_MEJEPA_LIVE_PREDICTIONS — this tool is the read side</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#api_contracts[mejepa_predict_latest]">request/response wire contract</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#component_contracts[MejepaPredictLatestHandler]">behaviour: prefix scan + descending sort + clamp limit + empty-friendly</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#rocksdb_schema[CF_MEJEPA_LIVE_PREDICTIONS]">10-byte key format: session_id_hash[6] || witness_chain_offset_be[8]</item>
    <item ref="EC-SHIFT-11">Empty result is a valid response (`session_known: false`); never an error</item>
    <item ref="NFR-SHIFT-03">p50 &lt; 50 ms</item>
  </implements>
  <depends_on>
    <task_ref>TASK-MCP-001</task_ref>
    <!-- Cross-phase: writer side that produces the rows we read -->
    <task_ref>TASK-SHIFT-007</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
This handler is the read side of `CF_MEJEPA_LIVE_PREDICTIONS` (the writer is sister TASK-SHIFT-007). It performs a RocksDB prefix scan keyed by `session_id_hash[6]`, takes the last `limit` rows in descending witness_chain_offset order, deserializes each row's `RealityPrediction`, and packages a JSON response. Empty results are a first-class response shape (`session_known: false`) per EC-SHIFT-11, never an error. `limit` is clamped server-side to [1, 100] (default 1) regardless of caller input.
</context>

<input_context_files>
  <file purpose="api_contract">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#api_contracts[mejepa_predict_latest]</file>
  <file purpose="component_contract">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#component_contracts[MejepaPredictLatestHandler]</file>
  <file purpose="rocksdb_schema">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#rocksdb_schema[CF_MEJEPA_LIVE_PREDICTIONS]</file>
  <file purpose="edge_case_empty_session">docs/ruvectorfindings/specs/functional/SPEC-SHIFT-SUBSCRIBER.md (EC-SHIFT-11)</file>
  <file purpose="prior_skeleton">docs/ruvectorfindings/specs/tasks/phase7-mcp-tools/TASK-MCP-001.md</file>
  <file purpose="cf_writer_side">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-007.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-MCP-001 complete: skeleton with PredictLatestArgs + stub handle()</check>
  <check>TASK-SHIFT-007 complete: `LivePredictionWriter` trait shipped + `RealityPrediction` carrier type defined; `CF_MEJEPA_LIVE_PREDICTIONS` exists on the RocksDB store</check>
  <check>The MCP context exposes `Arc<RocksDbStore>` for read-only queries (existing accessor in handlers/mod.rs)</check>
</prerequisites>

<scope>
  <in_scope>
    - Replace stub body in `crates/context-graph-mcp/src/handlers/tools/mejepa/predict_latest.rs`
    - Helper `session_id_hash6(session_id: &amp;str) -&gt; [u8; 6]` (SipHash or FxHash truncated to 6 bytes; deterministic; documented to match the writer's helper from TASK-SHIFT-007 — re-export the writer's helper instead of re-implementing if shipped publicly)
    - Prefix scan on CF_MEJEPA_LIVE_PREDICTIONS with prefix `session_id_hash6(session_id)` (6 bytes)
    - Parse keys: last 8 bytes are big-endian `witness_chain_offset: u64`
    - Sort results by witness_chain_offset DESC; take first `limit_clamped` rows
    - Deserialize each value as JSON `RealityPrediction` (carrier from TASK-MCP-002 traits.rs / cross-cutting with SPEC-INFERENCE)
    - Build response: `{ predictions: [...], session_known: bool }` where `session_known = !predictions.is_empty()`
    - `limit` clamping: `limit.clamp(1, 100)` server-side regardless of input default
    - `session_id` length validation: ≤ 64 chars per `<schema_constraints>`; reject `MEJEPA_INFER_DIM_MISMATCH`-style structured error if violated
    - Unit tests using a tempdir RocksDB store (no mock data; real CF) — populate 5 known rows for one session, query with limit=3, assert descending order + correct rows
  </in_scope>
  <out_of_scope>
    - The writer side that populates the CF (TASK-SHIFT-007)
    - Tier-compression policy on the CF (SPEC-HYGIENE)
    - FSV roundtrip evidence (TASK-MCP-007)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mcp/src/handlers/tools/mejepa/predict_latest.rs">
      pub async fn handle(
          args: serde_json::Value,
          ctx: &amp;crate::handlers::McpContext,
      ) -&gt; Result&lt;serde_json::Value, crate::error::McpError&gt;;

      fn session_id_hash6(session_id: &amp;str) -&gt; [u8; 6];

      #[derive(serde::Serialize, Debug)]
      pub struct PredictLatestRow {
          pub session_id: String,
          pub witness_chain_offset: u64,
          pub last_consumed_shift_id: String,
          pub prediction: crate::handlers::tools::mejepa::traits::RealityPrediction,
          pub persisted_at_unix_seconds: i64,
      }

      #[derive(serde::Serialize, Debug)]
      pub struct PredictLatestResponse {
          pub predictions: Vec&lt;PredictLatestRow&gt;,
          pub session_known: bool,
      }
    </signature>
    <signature file="crates/context-graph-mcp/src/handlers/tools/mejepa/traits.rs">
      // Add to traits.rs (extends TASK-MCP-002's file):
      #[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
      pub struct RealityPrediction { /* opaque to this task; owned by SPEC-INFERENCE */ }
    </signature>
  </signatures>

  <constraints>
    - All input parsing routed through SHIPPED `reality_loop::schema_validator`; deny_unknown_fields enforced at both DTO and JSON-Schema layer
    - `limit` SHALL be clamped to [1, 100] server-side; do NOT trust the caller-provided value beyond 100
    - `session_id` SHALL be ASCII + max 64 chars; reject otherwise with structured error code `MEJEPA_PREDICT_LATEST_SESSION_ID_INVALID`
    - Prefix scan uses RocksDB `read_options.set_prefix_same_as_start(true)` AND iterates with the first 6 bytes as the prefix; do NOT iterate the whole CF
    - Empty session result returns `{ predictions: [], session_known: false }` per EC-SHIFT-11; this MUST NOT be an error
    - The handler MUST NOT consume more than `limit_clamped` rows from disk — early-break out of the iterator after collecting that many in DESC order (use a small max-heap of size `limit` to avoid full-prefix sort on hot sessions)
    - `session_id_hash6` MUST be byte-equal to the writer's helper from TASK-SHIFT-007; if the writer's helper is not pub-exported yet, this task SHALL re-export the helper rather than duplicate it (single source of truth)
    - Latency budget per NFR-SHIFT-03: p50 &lt; 50 ms; assert in test with a 1,000-row population
    - Persistent RocksDB column-family handles cached on `McpContext` at construction (existing pattern); do NOT open/close per request
    - File length under 500 lines
  </constraints>

  <verification>
    - Test 1 (basic prefix scan): tempdir DB populated with 5 rows for `session_id="A"` + 5 rows for `session_id="B"`; query `{session_id:"A", limit:3}` returns 3 rows from A in DESC offset order
    - Test 2 (empty session): query non-existent session — returns `{ predictions: [], session_known: false }`, no error
    - Test 3 (limit clamping): query `limit:1000` → server returns at most 100; `limit:0` clamped to 1
    - Test 4 (session_id too long): 65-char session_id rejected with `MEJEPA_PREDICT_LATEST_SESSION_ID_INVALID`
    - Test 5 (unknown field): args with `evil:1` rejected by deny_unknown_fields
    - Test 6 (latency): populate 1,000 rows for one session, run 100 query iterations, assert p50 &lt; 50 ms (use `std::time::Instant` per call; record histogram)
    - Test 7 (hash collision robustness): two distinct session_ids that collide on `session_id_hash6` — verify response correctly filters by full session_id (the prefix match narrows; the full equality check rejects collisions); skip if hash function is collision-free in test inputs
    - `cargo build --release -p context-graph-mcp` clean
    - `cargo clippy -p context-graph-mcp -- -D warnings` clean
  </verification>
</definition_of_done>

<pseudo_code>
predict_latest::handle(args, ctx):
  let parsed: PredictLatestArgs = schema_validator::parse_strict(args)?;
  if parsed.session_id.len() > 64 { return Err(McpError::structured("MEJEPA_PREDICT_LATEST_SESSION_ID_INVALID", ...)); }
  let limit = parsed.limit.clamp(1, 100) as usize;
  let prefix = session_id_hash6(&parsed.session_id);

  let cf = ctx.rocksdb.cf_handle("CF_MEJEPA_LIVE_PREDICTIONS")?;
  let mut read_opts = rocksdb::ReadOptions::default();
  read_opts.set_prefix_same_as_start(true);

  // Collect into a min-heap of size `limit` keyed by witness_chain_offset (so we keep the largest)
  let mut top: BinaryHeap<Reverse<(u64, Vec<u8>, Vec<u8>)>> = BinaryHeap::with_capacity(limit + 1);
  let iter = ctx.rocksdb.prefix_iterator_cf_opt(&cf, &prefix, read_opts);
  for kv in iter {
    let (k, v) = kv?;
    if k.len() != 10 || k[..6] != prefix { continue; }
    let offset = u64::from_be_bytes(k[6..14].try_into().unwrap_or([0u8;8]));
    top.push(Reverse((offset, k.into_vec(), v.into_vec())));
    if top.len() > limit { top.pop(); }
  }

  // Drain heap; sort DESC for response
  let mut rows: Vec<_> = top.into_sorted_vec().into_iter().rev().collect();
  let predictions: Vec<PredictLatestRow> = rows.into_iter().map(|Reverse((offset, _k, v))| {
    let pred: RealityPrediction = serde_json::from_slice(&v)?;
    let row_meta: { last_consumed_shift_id, persisted_at_unix_seconds } = extract_meta_from_value_or_record(&pred);
    PredictLatestRow { session_id: parsed.session_id.clone(), witness_chain_offset: offset, ..row_meta, prediction: pred }
  }).collect::<Result<_, _>>()?;

  Ok(serde_json::json!({
    "predictions": predictions,
    "session_known": !predictions.is_empty(),
  }))
</pseudo_code>

<files_to_create>
  <!-- (none — this task only modifies/extends existing files) -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa/predict_latest.rs">replace stub with real handler + helpers + 7 unit tests</file>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa/traits.rs">add `RealityPrediction` opaque struct (consumed by predict_latest + capture_audit + verify)</file>
</files_to_modify>

<validation_criteria>
  <criterion>All 7 unit tests pass</criterion>
  <criterion>Empty session returns `session_known: false` with no error</criterion>
  <criterion>Limit clamped to [1, 100]</criterion>
  <criterion>Latency p50 &lt; 50 ms on 1,000-row population</criterion>
  <criterion>Prefix scan uses CF prefix iterator (not full scan)</criterion>
  <criterion>session_id_hash6 helper is single-source-of-truth (re-exported from TASK-SHIFT-007's writer)</criterion>
  <criterion>No file exceeds 500 lines</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mcp</command>
  <command>cargo test -p context-graph-mcp mejepa::predict_latest</command>
  <command>cargo clippy -p context-graph-mcp -- -D warnings</command>
</test_commands>
</task_spec>

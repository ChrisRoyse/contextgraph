<task_spec id="TASK-SHIFT-008" version="1.0">
<metadata>
  <title>Logic — WatermarkWriter durable, idempotent, restart-safe advance (CF_MEJEPA_SHIFT_WATERMARK)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>8</sequence>
  <implements>
    <item ref="REQ-SHIFT-02">Watermark advance only after persistence-and-cert success; durable fsync</item>
    <item ref="REQ-SHIFT-18">Provenance metadata in every watermark record</item>
    <item ref="ERR-SHIFT-04">MEJEPA_SHIFT_SUBSCRIBER_WATERMARK_BACKWARDS</item>
    <item ref="EC-SHIFT-06">SIGKILL between persist and watermark advance is recoverable</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#component_contracts[WatermarkWriter]">3 methods</item>
  </implements>
  <depends_on>
    <task_ref>TASK-SHIFT-002</task_ref>
    <task_ref>TASK-SHIFT-003</task_ref>
    <task_ref>TASK-SHIFT-004</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file (watermark.rs body) + tests</estimated_files>
</metadata>

<context>
Implements durable per-session watermark writes. `write_watermark` validates monotonicity, serializes WatermarkRecord to JSON, calls `rocksdb::DB::put_cf_opt` with `WriteOptions { sync: true }`, returns Ok only after fsync. `read_watermark` does point-read on `wm:{session_id}`. `read_all_watermarks` iterates with prefix `wm:` for restart hydration. NEVER batches watermark writes (per implementation_notes.durability rule).
</context>

<input_context_files>
  <file purpose="component_contract">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#component_contracts[WatermarkWriter]</file>
  <file purpose="durability_note">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#implementation_notes (durability + ordering rows)</file>
  <file purpose="cf_keys">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-004.md</file>
  <file purpose="watermark_record">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-002.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-SHIFT-002, TASK-SHIFT-003, TASK-SHIFT-004 complete</check>
  <check>`rocksdb` workspace dep available</check>
</prerequisites>

<scope>
  <in_scope>
    - Implement `WatermarkWriter::new`, `write_watermark`, `read_watermark`, `read_all_watermarks`
    - WriteOptions with `set_sync(true)` per durability invariant; one watermark write per shift (no batching)
    - Monotonicity validation: read existing record; if `persisted.last_consumed_shift_id >= record.last_consumed_shift_id`, return `WatermarkBackwards` (using ULID lexicographic ordering)
    - JSON serialization via `serde_json::to_vec`; deserialization via `serde_json::from_slice`
    - Tests: write → read round-trip; backwards write rejected; iter returns all written records; SIGKILL drill (write, kill simulation, reopen DB, verify last write is durable)
  </in_scope>
  <out_of_scope>
    - Tier-compression policy (SPEC-HYGIENE)
    - Watermark CF retention pruning (SPEC-HYGIENE)
    - run() startup hydration call (TASK-SHIFT-010)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-shift-subscriber/src/watermark.rs">
      use std::sync::Arc;
      use rocksdb::{DB, ColumnFamily, WriteOptions};
      use crate::cf::{CF_MEJEPA_SHIFT_WATERMARK, watermark_key};
      use crate::entry::SessionId;
      use crate::error::SubscriberError;

      pub struct WatermarkWriter {
          rocksdb: Arc&lt;DB&gt;,
          cf_name: String, // resolved to handle on each call (rocksdb crate idiom)
          write_options: WriteOptions,
      }

      impl WatermarkWriter {
          pub fn new(rocksdb: Arc&lt;DB&gt;) -&gt; Self;

          pub fn write_watermark(&amp;self, record: &amp;WatermarkRecord) -&gt; Result&lt;(), SubscriberError&gt;;

          pub fn read_watermark(&amp;self, session_id: &amp;SessionId) -&gt; Result&lt;Option&lt;WatermarkRecord&gt;, SubscriberError&gt;;

          pub fn read_all_watermarks(&amp;self) -&gt; Result&lt;Vec&lt;WatermarkRecord&gt;, SubscriberError&gt;;
      }
    </signature>
  </signatures>

  <constraints>
    - `WriteOptions::set_sync(true)` MUST be set on every write (no batching)
    - Monotonicity check uses ULID lexicographic ordering (`record.last_consumed_shift_id.as_str() &lt; persisted.as_str()` triggers WatermarkBackwards)
    - NO `unwrap()` outside tests
    - Iterator close is implicit via Drop; no leaked handles
    - File length under 500 lines
  </constraints>

  <verification>
    - Round-trip test: write WatermarkRecord, read back, assert byte-identical
    - Monotonicity test: write {shift_id: A}, attempt write {shift_id: A-1} → WatermarkBackwards
    - read_all_watermarks returns all written sessions
    - SIGKILL drill: write watermark, drop+reopen DB instance, read_watermark returns the record (proves fsync semantic)
    - Concurrent test: 8 threads each writing distinct sessions → no conflict; all 8 records readable
  </verification>
</definition_of_done>

<pseudo_code>
write_watermark:
  let cf = self.rocksdb.cf_handle(&self.cf_name).ok_or(...)?;
  let key = watermark_key(&record.session_id);
  if let Some(existing) = self.read_watermark(&record.session_id)? {
    if existing.last_consumed_shift_id.as_str() > record.last_consumed_shift_id.as_str() {
      return Err(SubscriberError::WatermarkBackwards{ persisted: existing.last_consumed_shift_id, observed: record.last_consumed_shift_id.clone(), session_id: record.session_id.clone() });
    }
  }
  let value = serde_json::to_vec(record)?;
  self.rocksdb.put_cf_opt(cf, &key, &value, &self.write_options)
    .map_err(|e| SubscriberError::RocksDbWriteFailed{ cf: CF_MEJEPA_SHIFT_WATERMARK.to_string(), key: key.clone(), retry: 0, source: e })?;
  Ok(())
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-shift-subscriber/tests/watermark_durability.rs">round-trip + monotonicity + SIGKILL drill</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/watermark.rs">implement 3 methods + ctor</file>
</files_to_modify>

<validation_criteria>
  <criterion>WriteOptions::sync is true on every write</criterion>
  <criterion>Monotonicity violation returns WatermarkBackwards</criterion>
  <criterion>SIGKILL drill survives DB reopen</criterion>
  <criterion>Iterator-based read_all returns all rows</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-shift-subscriber</command>
  <command>cargo test -p context-graph-mejepa-shift-subscriber watermark::</command>
  <command>cargo test -p context-graph-mejepa-shift-subscriber --test watermark_durability</command>
  <command>cargo clippy -p context-graph-mejepa-shift-subscriber -- -D warnings</command>
</test_commands>
</task_spec>

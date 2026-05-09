<task_spec id="TASK-SHIFT-004" version="1.0">
<metadata>
  <title>Foundation — RocksDB CF_MEJEPA_SHIFT_WATERMARK schema + per-shift cert key layout</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>4</sequence>
  <implements>
    <item ref="REQ-SHIFT-02">CF_MEJEPA_SHIFT_WATERMARK persistence + restart-safe resume</item>
    <item ref="REQ-SHIFT-18">Provenance metadata embedded in each persisted record</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#rocksdb_schema[CF_MEJEPA_SHIFT_WATERMARK]">key/value/write_options</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#rocksdb_schema[CF_MEJEPA_LIVE_PREDICTIONS]">key layout (10-byte composite)</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#rocksdb_schema[CF_MEJEPA_TRAIN_CERTS]">per-shift cert key layout (step number BE u64)</item>
  </implements>
  <depends_on>
    <task_ref>TASK-SHIFT-001</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>2 files</estimated_files>
</metadata>

<context>
Adds RocksDB column-family descriptors and key-builder helpers consumed by TASK-SHIFT-008 (WatermarkWriter), TASK-SHIFT-007 (process_shift), and the sister phase7-mcp-tools handlers. No I/O happens in this task — only deterministic key-builder functions + CF descriptor declarations + key-format unit tests.
</context>

<input_context_files>
  <file purpose="cf_schema">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#rocksdb_schema</file>
  <file purpose="cf_naming_convention">docs/ruvectorfindings/specs/constitution.md#coding_standards (rocksdb_cfs row)</file>
  <file purpose="prior_storage_pattern">crates/context-graph-storage/ (existing CF descriptor patterns)</file>
  <file purpose="watermark_record_struct">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-002.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-SHIFT-001 complete</check>
  <check>TASK-SHIFT-002 complete (WatermarkRecord exists)</check>
  <check>`rocksdb` crate is a workspace dependency</check>
</prerequisites>

<scope>
  <in_scope>
    - `crates/context-graph-mejepa-shift-subscriber/src/cf.rs` declaring CF name constants (`CF_MEJEPA_SHIFT_WATERMARK`, `CF_MEJEPA_LIVE_PREDICTIONS`, `CF_MEJEPA_ORACLE_VERDICTS`, `CF_MEJEPA_TRAIN_CERTS`, `CF_MEJEPA_PANELS`)
    - Key-builder helpers (pure functions, no I/O):
      - `watermark_key(session_id) -> Vec<u8>` (= `b"wm:" || session_id` UTF-8)
      - `live_prediction_key(session_id_hash6, witness_chain_offset_be8) -> [u8; 10]`
      - `oracle_verdict_key(task_id, mutation_category) -> Vec<u8>`
      - `train_cert_key(step_be_u64) -> [u8; 8]`
    - CF descriptor function: `pub fn shift_subscriber_cf_descriptors() -> Vec<rocksdb::ColumnFamilyDescriptor>` returning the 5 CFs with appropriate per-CF Options
    - Unit tests: each key-builder is deterministic, length-stable, prefix-scannable
  </in_scope>
  <out_of_scope>
    - Any RocksDB open/write code (TASK-SHIFT-008)
    - Tier-compression or hygiene policy (SPEC-HYGIENE)
    - Secondary index `SubscriberIndex` (lives in TASK-SHIFT-007)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-shift-subscriber/src/cf.rs">
      pub const CF_MEJEPA_SHIFT_WATERMARK: &amp;str = "CF_MEJEPA_SHIFT_WATERMARK";
      pub const CF_MEJEPA_LIVE_PREDICTIONS: &amp;str = "CF_MEJEPA_LIVE_PREDICTIONS";
      pub const CF_MEJEPA_ORACLE_VERDICTS: &amp;str = "CF_MEJEPA_ORACLE_VERDICTS";
      pub const CF_MEJEPA_TRAIN_CERTS: &amp;str = "CF_MEJEPA_TRAIN_CERTS";
      pub const CF_MEJEPA_PANELS: &amp;str = "CF_MEJEPA_PANELS";

      pub fn watermark_key(session_id: &amp;crate::entry::SessionId) -&gt; Vec&lt;u8&gt;;
      pub fn live_prediction_key(session_id_hash6: [u8; 6], witness_chain_offset: u64) -&gt; [u8; 10];
      pub fn live_prediction_prefix(session_id_hash6: [u8; 6]) -&gt; [u8; 6];
      pub fn oracle_verdict_key(task_id: &amp;str, mutation_category: &amp;str) -&gt; Vec&lt;u8&gt;;
      pub fn train_cert_key(step: u64) -&gt; [u8; 8];

      pub fn session_id_hash6(session_id: &amp;crate::entry::SessionId) -&gt; [u8; 6];
        // SHA-256(session_id.as_str())[..6]; deterministic; collision-tolerant for prefix scan
        // (collisions resolve via subsequent (offset_be8) tail or a follow-up lookup)

      pub fn shift_subscriber_cf_descriptors() -&gt; Vec&lt;rocksdb::ColumnFamilyDescriptor&gt;;
    </signature>
  </signatures>

  <constraints>
    - All CF names exactly as specified (SCREAMING_SNAKE with `CF_` prefix per constitution `<naming_conventions>`)
    - `live_prediction_key` MUST be 10 bytes total (6-byte hashed session + 8-byte big-endian offset) for prefix-scannability per `<rocksdb_schema>`
    - `train_cert_key` MUST be 8-byte big-endian u64 (lexicographic == numerical order)
    - `oracle_verdict_key` is `task_id || ":" || mutation_category` UTF-8; reject task/category strings containing `:` (must be Docker-name-safe per SEC-MEJEPA-05)
    - Per-CF `rocksdb::Options`:
      - All 5 CFs: `set_compression_type(rocksdb::DBCompressionType::Lz4)` (default)
      - `CF_MEJEPA_SHIFT_WATERMARK`: small CF; `set_max_write_buffer_number(2)`
      - `CF_MEJEPA_LIVE_PREDICTIONS`, `CF_MEJEPA_TRAIN_CERTS`, `CF_MEJEPA_PANELS`: prefix extractor on `[..6]` to enable bloom filters on prefix scans
    - NO `unwrap()` outside tests
  </constraints>

  <verification>
    - Unit test asserts `watermark_key(SessionId("abc"))` == `b"wm:abc"`
    - Unit test asserts `live_prediction_key(_, 0)` ends in `0x00..0x00` (8 zero bytes); `live_prediction_key(_, u64::MAX)` ends in `0xFF..0xFF`
    - Unit test asserts `train_cert_key(1)` == `[0,0,0,0,0,0,0,1]`
    - Unit test asserts `live_prediction_prefix(h)` is a prefix of `live_prediction_key(h, _)`
    - Unit test asserts `oracle_verdict_key("t", "c")` == `b"t:c"`
    - Unit test rejects `oracle_verdict_key("a:b", "c")` with structured error
    - 5 CF descriptors returned by `shift_subscriber_cf_descriptors()`; names match constants
  </verification>
</definition_of_done>

<pseudo_code>
session_id_hash6(sid):
  let mut hasher = sha2::Sha256::new();
  hasher.update(sid.as_str().as_bytes());
  let digest = hasher.finalize();
  digest[..6].try_into().unwrap()  -- safe: SHA-256 always 32 bytes

live_prediction_key(h6, offset):
  let mut k = [0u8; 10];
  k[..6].copy_from_slice(&h6);
  k[6..].copy_from_slice(&offset.to_be_bytes());
  k
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/cf.rs">CF name constants + key builders + CF descriptors</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/lib.rs">`pub mod cf; pub use cf::*;`</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/Cargo.toml">add `sha2` workspace dep + ensure `rocksdb` enabled</file>
</files_to_modify>

<validation_criteria>
  <criterion>5 CF descriptors returned; names match constants byte-for-byte</criterion>
  <criterion>Key-builders deterministic across runs (test asserts identity over 100 invocations)</criterion>
  <criterion>Length invariants: watermark_key &lt;= 67 bytes (3 + 64); live_prediction_key == 10; train_cert_key == 8</criterion>
  <criterion>oracle_verdict_key rejects `:` in task_id or mutation_category</criterion>
  <criterion>Build clean; no warnings</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-shift-subscriber</command>
  <command>cargo test -p context-graph-mejepa-shift-subscriber cf::tests</command>
  <command>cargo clippy -p context-graph-mejepa-shift-subscriber -- -D warnings</command>
</test_commands>
</task_spec>

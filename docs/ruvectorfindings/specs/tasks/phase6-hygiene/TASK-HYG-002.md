# TASK-HYG-002 — HygieneError + OpsErrorKind taxonomy

```xml
<task_spec id="TASK-HYG-002" version="1.0">
<metadata>
  <title>HygieneError + OpsErrorKind taxonomy with .code() arms</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>2</sequence>
  <implements>
    <requirement_ref>REQ-HYG-01</requirement_ref>
    <requirement_ref>REQ-HYG-05</requirement_ref>
    <requirement_ref>REQ-HYG-06</requirement_ref>
    <requirement_ref>REQ-HYG-08</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-HYG-001</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>2 files</estimated_files>
</metadata>

<context>
Defines the typed error surface every other task imports. The 6 SCREAMING_SNAKE codes from
TECH-HYGIENE `<implementation_notes category="error-handling">` map 1:1 to OpsErrorKind variants.
Plus `CrossCuttingDeferred` for SPEC-SELF-HEALING shim and `Internal` for catch-all RocksDB / IO
upstream causes. Every variant carries structured context (entry_id, category, segment_id) so
the reporting surface can build actionable error JSON.
</context>

<input_context_files>
  <file purpose="failure_taxonomy">../../constitution.md#failure_taxonomy</file>
  <file purpose="error_states">../../functional/SPEC-HYGIENE.md#error_states</file>
  <file purpose="error_handling">../../technical/TECH-HYGIENE.md#implementation_notes</file>
  <file purpose="categories_enum">crates/context-graph-mejepa-hygiene/src/categories.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-HYG-001 shipped (Category enum exists)</check>
  <check>thiserror in workspace dependencies</check>
</prerequisites>

<scope>
  <in_scope>
    - CREATE `crates/context-graph-mejepa-hygiene/src/error.rs` with `OpsError` struct +
      `OpsErrorKind` enum with these variants:
      * QuotaExceeded { category: Category, used_bytes: u64, budget_bytes: u64 }
      * QuotaUnrecoverable { category: Category, pass_freed_bytes: u64, pass_duration_ms: u64 }
      * TierDecompressFail { entry_id_cf: String, entry_id_key_hex: String, expected_magic: &'static str, actual_magic_hex: String, sub_error: String }
      * GcIntegrityFail { gc_run_id: uuid::Uuid, break_offset: usize, expected_prev_hash_hex: String, actual_prev_hash_hex: String }
      * MerkleVerifyFail { segment_id: u64, stored_root_hex: String, recomputed_root_hex: String, archive_path: String }
      * CheckpointPruneRefused { checkpoint_id: String, theta_sha_hex: String, tagged_at_ts: u64 }
      * CrossCuttingDeferred { spec: &'static str, op_name: &'static str }
      * Internal { reason: &'static str, source: Option<Box<dyn std::error::Error + Send + Sync>> }
    - `OpsError::code(&self) -> &'static str` returning the SCREAMING_SNAKE tag per variant
      (MEJEPA_HYG_QUOTA_EXCEEDED, MEJEPA_HYG_QUOTA_UNRECOVERABLE, MEJEPA_HYG_TIER_DECOMPRESS_FAIL,
       MEJEPA_HYG_GC_INTEGRITY_FAIL, MEJEPA_HYG_MERKLE_VERIFY_FAIL,
       MEJEPA_HYG_CHECKPOINT_PRUNE_REFUSED, MEJEPA_HYG_CROSS_CUTTING_DEFERRED,
       MEJEPA_HYG_INTERNAL).
    - `OpsError::context_json(&self) -> serde_json::Value` returning structured payload.
    - Type alias `pub type OpsResult<T> = Result<T, OpsError>;`
    - Unit tests in `src/tests.rs`: `error_codes_match_taxonomy()` covers each variant.
  </in_scope>
  <out_of_scope>
    - Wiring errors into ops::* (TASK-HYG-021).
    - Wiring errors into gc steps (TASK-HYG-012..019).
    - Schema-level error response shapes for MCP (TASK-HYG-022..024).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-hygiene/src/error.rs">
    pub type OpsResult&lt;T&gt; = Result&lt;T, OpsError&gt;;

    #[derive(Debug, thiserror::Error)]
    #[error("{kind:?}: {}", self.code())]
    pub struct OpsError {
        pub kind: OpsErrorKind,
    }

    #[derive(Debug)]
    pub enum OpsErrorKind {
        QuotaExceeded { category: crate::categories::Category, used_bytes: u64, budget_bytes: u64 },
        QuotaUnrecoverable { category: crate::categories::Category, pass_freed_bytes: u64, pass_duration_ms: u64 },
        TierDecompressFail { entry_id_cf: String, entry_id_key_hex: String, expected_magic: &amp;'static str, actual_magic_hex: String, sub_error: String },
        GcIntegrityFail { gc_run_id: uuid::Uuid, break_offset: usize, expected_prev_hash_hex: String, actual_prev_hash_hex: String },
        MerkleVerifyFail { segment_id: u64, stored_root_hex: String, recomputed_root_hex: String, archive_path: String },
        CheckpointPruneRefused { checkpoint_id: String, theta_sha_hex: String, tagged_at_ts: u64 },
        CrossCuttingDeferred { spec: &amp;'static str, op_name: &amp;'static str },
        Internal { reason: &amp;'static str, source: Option&lt;Box&lt;dyn std::error::Error + Send + Sync&gt;&gt; },
    }

    impl OpsError {
        pub fn code(&amp;self) -&gt; &amp;'static str;
        pub fn context_json(&amp;self) -&gt; serde_json::Value;
        pub fn quota_exceeded(category: crate::categories::Category, used: u64, budget: u64) -&gt; Self;
        pub fn cross_cutting_deferred(op_name: &amp;'static str) -&gt; Self;
        pub fn internal(reason: &amp;'static str) -&gt; Self;
    }
    </signature>
  </signatures>

  <constraints>
    - All `.code()` arms return SCREAMING_SNAKE strings exactly as listed in
      TECH-HYGIENE `<implementation_notes category="error-handling">`.
    - `MEJEPA_HYG_*` prefix on every code (constitution `error_codes` rule).
    - No `Box<dyn Error>` except behind `Internal::source` (typed variants only for the rest).
    - `Internal::source` is `Send + Sync` so OpsError remains thread-safe across spawn_blocking.
    - Files under 500 lines.
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-mejepa-hygiene` succeeds.
    - `cargo test -p context-graph-mejepa-hygiene -- error_codes_match_taxonomy` passes.
    - `cargo clippy -p context-graph-mejepa-hygiene -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
impl OpsError {
    pub fn code(&amp;self) -&gt; &amp;'static str {
        match self.kind {
            OpsErrorKind::QuotaExceeded { .. } =&gt; "MEJEPA_HYG_QUOTA_EXCEEDED",
            OpsErrorKind::QuotaUnrecoverable { .. } =&gt; "MEJEPA_HYG_QUOTA_UNRECOVERABLE",
            OpsErrorKind::TierDecompressFail { .. } =&gt; "MEJEPA_HYG_TIER_DECOMPRESS_FAIL",
            OpsErrorKind::GcIntegrityFail { .. } =&gt; "MEJEPA_HYG_GC_INTEGRITY_FAIL",
            OpsErrorKind::MerkleVerifyFail { .. } =&gt; "MEJEPA_HYG_MERKLE_VERIFY_FAIL",
            OpsErrorKind::CheckpointPruneRefused { .. } =&gt; "MEJEPA_HYG_CHECKPOINT_PRUNE_REFUSED",
            OpsErrorKind::CrossCuttingDeferred { .. } =&gt; "MEJEPA_HYG_CROSS_CUTTING_DEFERRED",
            OpsErrorKind::Internal { .. } =&gt; "MEJEPA_HYG_INTERNAL",
        }
    }
}

#[test]
fn error_codes_match_taxonomy() {
    let e = OpsError::quota_exceeded(Category::MutationCorpus, 1, 1);
    assert_eq!(e.code(), "MEJEPA_HYG_QUOTA_EXCEEDED");
    // ... one assert per variant
}
</pseudo_code>

<files_to_create></files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-hygiene/src/error.rs">populate with OpsError + OpsErrorKind + .code() + context_json + tests</file>
  <file path="crates/context-graph-mejepa-hygiene/src/tests.rs">add error_codes_match_taxonomy unit test</file>
</files_to_modify>

<validation_criteria>
  <criterion>8 OpsErrorKind variants with SCREAMING_SNAKE .code() arms</criterion>
  <criterion>context_json returns valid serde_json::Value for every variant</criterion>
  <criterion>OpsError implements std::error::Error via thiserror</criterion>
  <criterion>cargo test error_codes_match_taxonomy passes; clippy clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-hygiene</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- error_codes_match_taxonomy</command>
  <command>cargo clippy -p context-graph-mejepa-hygiene -- -D warnings</command>
</test_commands>
</task_spec>
```

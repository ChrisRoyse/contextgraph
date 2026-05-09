# TASK-HYG-011 — witness_verify_integrity + audit_verify_old_entry

```xml
<task_spec id="TASK-HYG-011" version="1.0">
<metadata>
  <title>witness_verify_integrity (live + compressed-segment chain) + audit_verify_old_entry (Merkle re-verify)</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>11</sequence>
  <implements>
    <requirement_ref>REQ-HYG-03</requirement_ref>
    <requirement_ref>REQ-HYG-04</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-HYG-010</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file (extends witness_compress.rs)</estimated_files>
</metadata>

<context>
Two verification operations:
- `witness_verify_integrity` walks the live chain (mix of live entries + compressed-segment
  entries) and verifies every prev_hash links correctly. For compressed-segment entries it
  verifies the metadata sidecar exists in CF_MEJEPA_WITNESS_SEGMENT_META — it does NOT
  re-fetch + recompute the Merkle root (that's audit_verify_old_entry's job, on demand).
  Latency must scale linearly with chain length.
- `audit_verify_old_entry(segment_id, offset)` is the auditor's verification: re-fetch the
  archived 1024-entry blob from cold archive, verify SHA-256 matches stored archive_sha256,
  recompute Merkle root, compare byte-equal vs stored merkle_root, return MerkleVerifyFail
  on mismatch. Used by US-HYG-03 (auditor at offset 137 of segment 42).
</context>

<input_context_files>
  <file purpose="verify_spec">../../technical/TECH-HYGIENE.md#component_contracts</file>
  <file purpose="merkle_primitive">crates/context-graph-witness/src/cert.rs</file>
  <file purpose="compress_module">crates/context-graph-mejepa-hygiene/src/witness_compress.rs</file>
  <file purpose="error">crates/context-graph-mejepa-hygiene/src/error.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-HYG-010 shipped (CF_MEJEPA_WITNESS_SEGMENT_META + WITNESS_TYPE_COMPRESSED_SEGMENT)</check>
</prerequisites>

<scope>
  <in_scope>
    - EXTEND `src/witness_compress.rs` (or split):
      * `pub fn verify_chain_integrity(env: &HygieneEnv) -> OpsResult<ChainIntegrityReport>`:
        1. Iterate live chain entries in chain order.
        2. For each entry: hash = sha3::Shake256(entry_bytes) → 32 bytes.
           Verify next_entry.prev_hash == hash.
        3. For compressed-segment entries (witness_type byte == 0x80):
           - Verify metadata sidecar exists in CF_MEJEPA_WITNESS_SEGMENT_META
             keyed by segment_id (parse segment_id from entry's payload region).
           - Do NOT re-fetch + recompute Merkle root (cheap-path).
           - Verify the entry's action_hash byte-equals sidecar.merkle_root.
        4. On any link break or missing sidecar: return ChainIntegrityReport
           { passed: false, break_offset: Some(offset), entries_verified, duration_ms }.
        5. On full pass: return passed: true.
        6. NEVER throws OpsError for verification failure — verification failure is data
           in the report, not an exception. OpsError reserved for I/O failures during read.
      * `pub fn audit_verify_old_entry(env: &HygieneEnv, segment_id: u64, offset: u32)
        -> OpsResult<()>`:
        1. Read CF_MEJEPA_WITNESS_SEGMENT_META[segment_id] → CompressedSegmentMetadata.
        2. Read file at meta.cold_archive_path → segment_bytes.
        3. Verify segment_bytes.len() == 74_752.
        4. Verify SHA-256(segment_bytes) == meta.archive_sha256; on mismatch return
           OpsError::MerkleVerifyFail { ..., sub_error: "archive_sha256_mismatch" }.
        5. Reconstruct leaves: chunk segment_bytes into 1024 × 73-byte slices.
        6. Recompute root via MerkleTree::from_leaves; compare byte-equal stored root.
        7. On match: Ok(()). On mismatch: OpsError::MerkleVerifyFail with both roots.
        8. If offset is provided: also verify offset &lt; 1024 (sanity).
      * EC-HYG-07: cold archive unavailable → OpsError::Internal { reason:
        "cold_archive_unavailable" } (not MerkleVerifyFail; verification couldn't complete).
    - Unit + integration tests:
      * Healthy chain → passed=true.
      * Inject break (modify one entry's prev_hash) → passed=false, break_offset set.
      * Compressed-segment entry without sidecar → passed=false, break_offset at that entry.
      * audit_verify_old_entry on healthy archive → Ok.
      * Tamper archive bytes → archive_sha256_mismatch.
      * Replace stored Merkle root in sidecar → MerkleVerifyFail with stored ≠ recomputed.
      * Cold archive file missing → OpsError::Internal "cold_archive_unavailable".
  </in_scope>
  <out_of_scope>
    - Operator MCP audit tool (out of v1.0 scope; auditor calls library directly).
    - GC step 7 wrapper (TASK-HYG-018).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-hygiene/src/witness_compress.rs">
    pub fn verify_chain_integrity(env: &amp;crate::HygieneEnv) -&gt; crate::error::OpsResult&lt;crate::reports::ChainIntegrityReport&gt;;
    pub fn audit_verify_old_entry(env: &amp;crate::HygieneEnv, segment_id: u64, offset: u32) -&gt; crate::error::OpsResult&lt;()&gt;;
    </signature>
  </signatures>

  <constraints>
    - verify_chain_integrity NEVER returns OpsError for verification failures (data in report).
      Only I/O failures during chain read return OpsError.
    - audit_verify_old_entry returns MerkleVerifyFail on root mismatch with both roots in error.
    - SHAKE-256 hashing matches the SHIPPED witness chain primitive.
    - Verifier latency scales linearly with chain length (single pass; no quadratic patterns).
    - File still ≤ 500 lines (split if necessary).
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-mejepa-hygiene` succeeds.
    - `cargo test -p context-graph-mejepa-hygiene -- verify_chain_integrity_healthy` passes.
    - `cargo test -p context-graph-mejepa-hygiene -- verify_chain_integrity_break_offset` passes.
    - `cargo test -p context-graph-mejepa-hygiene -- audit_verify_old_entry_healthy` passes.
    - `cargo test -p context-graph-mejepa-hygiene -- audit_verify_old_entry_tamper` passes.
    - clippy clean.
  </verification>
</definition_of_done>

<pseudo_code>
pub fn verify_chain_integrity(env: &amp;HygieneEnv) -&gt; OpsResult&lt;ChainIntegrityReport&gt; {
    let started = std::time::Instant::now();
    let entries = read_witness_chain_in_order(env)?;
    let meta_cf = env.db.cf_handle(CF_MEJEPA_WITNESS_SEGMENT_META)
        .ok_or_else(|| OpsError::internal("meta_cf_missing"))?;
    let mut prev_hash: [u8; 32] = [0u8; 32];
    for (i, entry) in entries.iter().enumerate() {
        if entry.bytes.len() != 73 {
            return Ok(ChainIntegrityReport { entries_verified: i as u64, passed: false,
                break_offset: Some(i), duration_ms: started.elapsed().as_millis() as u64 });
        }
        let observed_prev: [u8; 32] = entry.bytes[0..32].try_into().unwrap();
        if i &gt; 0 &amp;&amp; observed_prev != prev_hash {
            return Ok(ChainIntegrityReport { entries_verified: i as u64, passed: false,
                break_offset: Some(i), duration_ms: started.elapsed().as_millis() as u64 });
        }
        let witness_type = entry.bytes[72];
        if witness_type == WITNESS_TYPE_COMPRESSED_SEGMENT {
            let segment_id = derive_segment_id_from_entry(entry);
            let raw = env.db.get_cf(&amp;meta_cf, segment_id.to_be_bytes())?;
            if raw.is_none() {
                return Ok(ChainIntegrityReport { entries_verified: i as u64, passed: false,
                    break_offset: Some(i), duration_ms: started.elapsed().as_millis() as u64 });
            }
            let meta: CompressedSegmentMetadata = serde_json::from_slice(&amp;raw.unwrap())
                .map_err(|e| OpsError::internal_with_source("meta_parse", Box::new(e)))?;
            let action_hash: [u8; 32] = entry.bytes[32..64].try_into().unwrap();
            if action_hash != meta.merkle_root {
                return Ok(ChainIntegrityReport { entries_verified: i as u64, passed: false,
                    break_offset: Some(i), duration_ms: started.elapsed().as_millis() as u64 });
            }
        }
        // hash this entry → next prev_hash
        let mut hasher = sha3::Shake256::default();
        sha3::digest::Update::update(&amp;mut hasher, &amp;entry.bytes);
        let mut out = [0u8; 32];
        sha3::digest::ExtendableOutput::finalize_xof(hasher).read(&amp;mut out);
        prev_hash = out;
    }
    Ok(ChainIntegrityReport { entries_verified: entries.len() as u64, passed: true,
        break_offset: None, duration_ms: started.elapsed().as_millis() as u64 })
}
</pseudo_code>

<files_to_create></files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-hygiene/src/witness_compress.rs">add verify_chain_integrity + audit_verify_old_entry</file>
  <file path="crates/context-graph-mejepa-hygiene/src/tests.rs">add 6 verification tests</file>
</files_to_modify>

<validation_criteria>
  <criterion>Healthy chain → passed=true, break_offset=None</criterion>
  <criterion>Single-byte tamper → passed=false, break_offset at the entry</criterion>
  <criterion>Compressed-segment without sidecar → passed=false at that entry</criterion>
  <criterion>audit_verify on healthy archive → Ok</criterion>
  <criterion>Tampered archive bytes → MerkleVerifyFail "archive_sha256_mismatch"</criterion>
  <criterion>Replaced sidecar root → MerkleVerifyFail with both roots</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-hygiene</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- verify_chain_integrity_healthy</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- verify_chain_integrity_break_offset</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- audit_verify_old_entry_healthy</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- audit_verify_old_entry_tamper</command>
  <command>cargo clippy -p context-graph-mejepa-hygiene -- -D warnings</command>
</test_commands>
</task_spec>
```

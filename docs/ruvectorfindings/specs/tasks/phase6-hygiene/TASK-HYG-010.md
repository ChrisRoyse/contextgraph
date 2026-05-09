# TASK-HYG-010 — witness_compress::compress_segments + atomic archive + Merkle build

```xml
<task_spec id="TASK-HYG-010" version="1.0">
<metadata>
  <title>witness_compress::compress_segments + archive_segment_atomic + 1024-entry Merkle build (reuse SHIPPED context-graph-witness::cert)</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>10</sequence>
  <implements>
    <requirement_ref>REQ-HYG-04</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-HYG-003</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
The Phase 6 §6.4 deliverable: groups consecutive 1024-entry witness chain segments older than
the threshold (default 90 days) into a single 73-byte "compressed-segment" entry whose
`action_hash` field carries the Merkle root of the 1024 originals. Original 1024×73=74,752 bytes
are atomically archived to `<runtime_root>/mejepa/witness-archive/<segment_id>.bin` BEFORE
the live chain is replaced (atomic copy-then-replace per BR-HYG-03). Disk-full during archive
write rolls back: original segment stays in live chain unchanged (per EC-HYG-04). Merkle
construction reuses SHIPPED `context-graph-witness::cert::MerkleTree::from_leaves` with
`MERKLE_LEAF_TAG`/`MERKLE_NODE_TAG` constants (cross-host determinism per NFR-HYG-05).
witness_type=0x80 (high-bit set) discriminates compressed-segment entries from live entries
(low-byte range).
</context>

<input_context_files>
  <file purpose="merkle_primitive">crates/context-graph-witness/src/cert.rs</file>
  <file purpose="witness_chain_format">crates/context-graph-witness/src/lib.rs</file>
  <file purpose="compression_spec">../../technical/TECH-HYGIENE.md#component_contracts</file>
  <file purpose="diagram">../../technical/TECH-HYGIENE.md#architecture_diagram</file>
  <file purpose="entry_data_model">../../technical/TECH-HYGIENE.md#data_models</file>
  <file purpose="reuse_note">../../technical/TECH-HYGIENE.md#implementation_notes</file>
</input_context_files>

<prerequisites>
  <check>TASK-HYG-003 shipped (CompressReport + ChainIntegrityReport types)</check>
  <check>SHIPPED context-graph-witness::cert::MerkleTree::from_leaves available</check>
  <check>SHIPPED MERKLE_LEAF_TAG + MERKLE_NODE_TAG constants accessible</check>
</prerequisites>

<scope>
  <in_scope>
    - CREATE `src/witness_compress.rs` with:
      * `pub const WITNESS_TYPE_COMPRESSED_SEGMENT: u8 = 0x80;`
      * `pub const SEGMENT_ENTRIES: usize = 1024;`
      * `pub const SEGMENT_BYTES: usize = SEGMENT_ENTRIES * 73; // 74752`
      * `pub const CF_MEJEPA_WITNESS_SEGMENT_META: &str = "CF_MEJEPA_WITNESS_SEGMENT_META";`
      * `pub struct CompressedSegmentMetadata { segment_id: u64, merkle_root: [u8;32],
        start_timestamp_ns: u64, end_timestamp_ns: u64, entry_count: u32,
        cold_archive_path: String, archive_sha256: [u8;32], corpus_sha: [u8;32] }`
        with serde::Serialize/Deserialize.
      * `pub fn compress_segments(env: &HygieneEnv, threshold_age: chrono::Duration)
        -> OpsResult<CompressReport>`:
        1. Refuse `threshold_age &lt; chrono::Duration::days(1)` → OpsError::Internal.
        2. Iterate witness chain entries in chain order.
        3. Group consecutive 1024-entry runs where every entry's
           `timestamp_ns &lt; (now - threshold_age).timestamp_nanos()`.
        4. For each candidate segment:
           a. Build leaves: each entry's full 73 bytes is one leaf.
           b. Compute Merkle root: `MerkleTree::from_leaves(&leaves)?.root()` —
              uses MERKLE_LEAF_TAG / MERKLE_NODE_TAG → cross-host deterministic.
           c. Call `archive_segment_atomic(...)` — see below.
           d. Construct compressed-segment entry: 73 bytes:
              - bytes 0..32: prev_hash (link to previous live or compressed entry)
              - bytes 32..64: action_hash = merkle_root
              - bytes 64..72: end_timestamp_ns big-endian
              - byte 72: WITNESS_TYPE_COMPRESSED_SEGMENT (0x80)
           e. Build a single RocksDB write_batch:
              - delete the 1024 original entries
              - put the single compressed-segment entry at the chain key
              - put CompressedSegmentMetadata into CF_MEJEPA_WITNESS_SEGMENT_META
                keyed by segment_id big-endian
           f. Commit batch atomically.
           g. Roll back on commit failure: remove the archive file (it was written first).
        5. After all segments: invoke witness_verify_integrity (forward-declared; populated in
           TASK-HYG-011).
        6. Return CompressReport.
      * `fn archive_segment_atomic(archive_root: &Path, segment_id: u64,
        segment_bytes: &[u8], meta_partial: &CompressedSegmentMetadata)
        -> OpsResult<(String, [u8;32])>`:
        1. tmp_path = archive_root.join(format!("{}.bin.tmp", segment_id))
        2. Write bytes; fsync.
        3. final_path = archive_root.join(format!("{}.bin", segment_id))
        4. Rename tmp → final (atomic on POSIX).
        5. Compute archive_sha256 = sha2::Sha256::digest(segment_bytes).
        6. Write sidecar `{segment_id}.meta.json` (serde_json pretty) with
           corpus_sha + start_ts + end_ts + entry_count + archive_sha256.
        7. fsync the dir entry to make rename durable.
        8. On any IO failure: clean partial tmp file; return
           OpsError::QuotaExceeded { reason: "archive_disk_full" } OR Internal { reason: "io_error", source: Some(Box::new(e)) }.
        9. Return (final_path, archive_sha256).
    - Cold archive root: defaults to `<env.runtime_root>/mejepa/witness-archive/`;
      operator override via `CG_MEJEPA_WITNESS_ARCHIVE_ROOT` env (per Q-HYG-03).
    - Unit + integration tests:
      * 2,048-entry synthetic chain, threshold=0 → 2 compressed-segment entries +
        2 archive files; live chain bytes ≈ 2*73 = 146 bytes (replaced).
      * Disk-full simulation (archive on a tmpfs limited to 100 bytes) → rollback;
        live chain unchanged; QuotaExceeded returned.
      * Cross-host determinism: identical input on two hosts produces byte-equal Merkle roots.
      * Merkle root verifiable: re-fetch archive, recompute root, byte-equal stored root.
      * Refuse threshold < 1 day.
      * Refuse partial segment (< 1024 entries).
  </in_scope>
  <out_of_scope>
    - witness_verify_integrity body (TASK-HYG-011).
    - GC step 6 wrapper (TASK-HYG-017).
    - MCP shim (TASK-HYG-024).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-hygiene/src/witness_compress.rs">
    pub const WITNESS_TYPE_COMPRESSED_SEGMENT: u8 = 0x80;
    pub const SEGMENT_ENTRIES: usize = 1024;
    pub const SEGMENT_BYTES: usize = 74_752;
    pub const CF_MEJEPA_WITNESS_SEGMENT_META: &amp;str = "CF_MEJEPA_WITNESS_SEGMENT_META";

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct CompressedSegmentMetadata {
        pub segment_id: u64,
        pub merkle_root: [u8; 32],
        pub start_timestamp_ns: u64,
        pub end_timestamp_ns: u64,
        pub entry_count: u32,
        pub cold_archive_path: String,
        pub archive_sha256: [u8; 32],
        pub corpus_sha: [u8; 32],
    }

    pub fn compress_segments(
        env: &amp;crate::HygieneEnv,
        threshold_age: chrono::Duration,
    ) -&gt; crate::error::OpsResult&lt;crate::reports::CompressReport&gt;;
    </signature>
  </signatures>

  <constraints>
    - Atomic copy-then-replace per BR-HYG-03 — archive MUST be durable before live replacement.
    - Cross-host Merkle determinism uses SHIPPED MERKLE_LEAF_TAG/MERKLE_NODE_TAG (do NOT
      define new tags here).
    - Reject threshold_age < 1 day fail-closed.
    - Reject partial segment (&lt; 1024 entries) per EC-HYG-01.
    - Reject empty witness chain → returns CompressReport with zero counts (vacuous).
    - On IO failure: clean partial tmp; original chain unchanged; structured error.
    - File ≤ 500 lines (split into witness_compress_archive.rs if needed).
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-mejepa-hygiene` succeeds.
    - `cargo test -p context-graph-mejepa-hygiene -- compress_segments_2048_entries` passes.
    - `cargo test -p context-graph-mejepa-hygiene -- compress_segments_disk_full_rollback` passes.
    - `cargo test -p context-graph-mejepa-hygiene -- compress_segments_partial_segment_skipped` passes.
    - Cross-host: SHA-256(merkle_roots blob) on host A == host B for identical input.
    - clippy clean.
  </verification>
</definition_of_done>

<pseudo_code>
pub fn compress_segments(env: &amp;HygieneEnv, threshold_age: Duration) -&gt; OpsResult&lt;CompressReport&gt; {
    if threshold_age &lt; Duration::days(1) {
        return Err(OpsError::internal("threshold_age &lt; 1 day"));
    }
    let now = env.now().timestamp_nanos_opt().unwrap_or(0);
    let cutoff_ts = now - threshold_age.num_nanoseconds().unwrap_or(0);
    let candidates = group_into_segments(env, cutoff_ts)?;
    let mut roots = Vec::with_capacity(candidates.len());
    let mut paths = Vec::with_capacity(candidates.len());
    let mut bytes_before = 0u64;
    let mut bytes_after = 0u64;
    let archive_root = resolve_archive_root(env)?;
    std::fs::create_dir_all(&amp;archive_root)?;

    for seg in candidates {
        let leaves: Vec&lt;Vec&lt;u8&gt;&gt; = seg.entries.iter().map(|e| e.bytes.clone()).collect();
        let tree = context_graph_witness::cert::MerkleTree::from_leaves(&amp;leaves)
            .map_err(|e| OpsError::internal_with_source("merkle_build", Box::new(e)))?;
        let root = tree.root();
        let segment_bytes: Vec&lt;u8&gt; = seg.entries.iter().flat_map(|e| e.bytes.clone()).collect();
        let mut meta = CompressedSegmentMetadata { segment_id: seg.id, merkle_root: root,
            start_timestamp_ns: seg.start_ts, end_timestamp_ns: seg.end_ts,
            entry_count: SEGMENT_ENTRIES as u32, cold_archive_path: String::new(),
            archive_sha256: [0u8; 32], corpus_sha: env.corpus_sha };
        let (path, sha) = archive_segment_atomic(&amp;archive_root, seg.id, &amp;segment_bytes, &amp;meta)?;
        meta.cold_archive_path = path.clone();
        meta.archive_sha256 = sha;

        let compressed_entry = build_compressed_segment_entry(seg.prev_hash, root, seg.end_ts);
        let mut batch = WriteBatch::default();
        for e in &amp;seg.entries { batch.delete_cf(&amp;witness_cf, &amp;e.key); }
        batch.put_cf(&amp;witness_cf, &amp;seg.first_key, &amp;compressed_entry);
        batch.put_cf(&amp;meta_cf, seg.id.to_be_bytes(), serde_json::to_vec(&amp;meta)?);
        if let Err(e) = env.db.write(batch) {
            // rollback: remove the archive
            let _ = std::fs::remove_file(&amp;path);
            return Err(OpsError::internal_with_source("rocksdb_commit_failed", Box::new(e)));
        }
        roots.push(root); paths.push(path);
        bytes_before += SEGMENT_BYTES as u64;
        bytes_after += 73;
    }

    let integrity = crate::witness_compress::verify_chain_integrity(env)?;
    Ok(CompressReport { threshold_days: threshold_age.num_days() as u32,
        segments_compressed: roots.len() as u64,
        entries_compressed: roots.len() as u64 * SEGMENT_ENTRIES as u64,
        bytes_before, bytes_after,
        compression_ratio: bytes_before as f64 / bytes_after.max(1) as f64,
        merkle_roots: roots, cold_archive_paths: paths,
        integrity_check_passed: integrity.passed })
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-hygiene/src/witness_compress.rs">compress_segments + archive_segment_atomic + helpers</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-hygiene/Cargo.toml">add sha2 dep if not present</file>
  <file path="crates/context-graph-mejepa-hygiene/src/tests.rs">add compression integration tests</file>
</files_to_modify>

<validation_criteria>
  <criterion>2048-entry chain → 2 compressed-segment entries; archive files written; SHA-256 matches</criterion>
  <criterion>Disk-full simulation rolls back; live chain unchanged; QuotaExceeded returned</criterion>
  <criterion>Partial &lt; 1024-entry segment skipped (per EC-HYG-01)</criterion>
  <criterion>Cross-host Merkle determinism: identical inputs → byte-equal roots</criterion>
  <criterion>Refuse threshold &lt; 1 day</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-hygiene</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- compress_segments_2048_entries</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- compress_segments_disk_full_rollback</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- compress_segments_partial_segment_skipped</command>
  <command>cargo clippy -p context-graph-mejepa-hygiene -- -D warnings</command>
</test_commands>
</task_spec>
```

# TASK-HYG-010 — witness_compress::compress_segments + atomic archive + Merkle build

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

## 📋 2026-05-10 STATE AUDIT — clean

Verified `crates/context-graph-witness/src/lib.rs` exists (shipped 2026-05-08) with cert/Merkle support. `CF_MEJEPA_WITNESS_SEGMENT_META` is NOT yet declared in heal/cf.rs — this task is its first declarer (correct). Task body accurate.

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

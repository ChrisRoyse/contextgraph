# TASK-HYG-006 — tier_frozen 1-bit CGTF encoder + decoder

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

CGTF (1-bit sign-only) is the magic-distinct tier-frozen format that complements the existing context-graph-tier-compression CGTC variants; no existing CGTF impl in tree. Task body accurate.

```xml
<task_spec id="TASK-HYG-006" version="1.0">
<metadata>
  <title>tier_frozen 1-bit sign-only encoder + decoder (CGTF magic; ~32× compression)</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>6</sequence>
  <implements>
    <requirement_ref>REQ-HYG-02</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-HYG-005</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
The 5th tier — "Frozen" 1-bit sign-only encoding — is NOT supplied by the shipped
context-graph-tier-compression crate (which supports only 8/7/5/3-bit per its public BitWidth
enum). This task ships a CLEAN-ROOM separate encoder with `CGTF` magic (4 bytes: 0x43 0x47 0x54
0x46) so the shipped crate's contract stays unchanged. The encoder writes a 22-byte header
mirroring the CGTC layout (magic + version=1 + bits=1 + n + min + max) plus a payload of
ceil(n/8) bytes where bit i is set iff `values[i] >= 0.0`. The decoder reconstructs to
±max/2 (or ±sigma_estimate; spec is ±max/2 per data_models). This is "fidelity_lost: true"
territory — the panel meta records that the values lost magnitude, and the operator can
re-materialize from upstream source if available (per <note category="frozen-tier-fidelity">).
</context>

<input_context_files>
  <file purpose="frozen_tier_spec">../../technical/TECH-HYGIENE.md#component_contracts</file>
  <file purpose="cgtc_layout_reference">crates/context-graph-tier-compression/src/lib.rs</file>
  <file purpose="error">crates/context-graph-mejepa-hygiene/src/error.rs</file>
  <file purpose="tier_module">crates/context-graph-mejepa-hygiene/src/tier.rs</file>
  <file purpose="provenance_decision">../../../../memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-HYG-005 shipped (decode_for_tier dispatch ready to call tier_frozen)</check>
  <check>CGTC magic bytes confirmed in shipped tier-compression</check>
</prerequisites>

<scope>
  <in_scope>
    - CREATE `src/tier_frozen.rs` with:
      * `pub const MAGIC_CGTF: [u8; 4] = *b"CGTF";`
      * `pub const VERSION: u8 = 1;`
      * `pub const HEADER_LEN: usize = 22;` (matches CGTC: 4 magic + 1 version + 1 bits + 8 n
        + 4 min + 4 max)
      * `pub fn encode_sign_only(values: &[f32]) -> OpsResult<Vec<u8>>`:
        - Refuse empty input → OpsError::Internal { reason: "empty_values" }.
        - Refuse non-finite values → OpsError::Internal { reason: "non_finite" }.
        - Compute actual min/max of values.
        - Allocate `HEADER_LEN + ceil(n/8)` bytes.
        - Write header: MAGIC_CGTF, VERSION, bits=1, n.to_le_bytes(),
          min.to_le_bytes(), max.to_le_bytes().
        - Write payload bits: byte_idx = i/8; bit_idx = i%8; set bit iff values[i] >= 0.0.
      * `pub fn decode_sign_only(blob: &[u8]) -> OpsResult<Vec<f32>>`:
        - Validate len >= HEADER_LEN.
        - Validate magic == MAGIC_CGTF.
        - Validate version == 1; bits == 1.
        - Parse n: u64 from header; refuse n > MAX_N (e.g., 1 << 28 to bound allocation).
        - Validate payload length == ceil(n/8).
        - Reconstruct: bit set → +max/2; bit clear → -max/2 (literally max/2, NOT (max-min)/2).
          NOTE: max/2 is the agreed reconstruction value per data_models. Reject if
          max == 0.0 AND min == 0.0 (degenerate; reconstruction would always be 0).
      * Mark fidelity_lost in panel meta — caller responsibility, not encoder's. Encoder
        returns reconstructed Vec<f32>; the calling tier_promote / tier_demote sets the flag
        on the EntryAccessFrequency sidecar.
      * Cross-host determinism: little-endian everywhere; no platform-dependent floats.
    - Unit tests:
      * Round-trip 1024 random ±1.0 vectors → all signs preserved → reconstruction sign matches input sign.
      * Header byte-equal across re-encode of identical input.
      * Decode rejects: bad magic, wrong version, bits≠1, truncated payload, n exceeds MAX_N.
      * Empty input rejected.
      * Compression ratio ≈ 32× on 768-d input (3072 raw bytes → 22+96 = 118 bytes ≈ 26×).
  </in_scope>
  <out_of_scope>
    - Wiring fidelity_lost flag (TASK-HYG-007 in tier_promote / tier_demote).
    - Decoding upstream source from cold archive (out of spec).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-hygiene/src/tier_frozen.rs">
    pub const MAGIC_CGTF: [u8; 4] = *b"CGTF";
    pub const VERSION: u8 = 1;
    pub const HEADER_LEN: usize = 22;
    pub const MAX_N: u64 = 1 &lt;&lt; 28;

    pub fn encode_sign_only(values: &amp;[f32]) -&gt; crate::error::OpsResult&lt;Vec&lt;u8&gt;&gt;;
    pub fn decode_sign_only(blob: &amp;[u8]) -&gt; crate::error::OpsResult&lt;Vec&lt;f32&gt;&gt;;
    </signature>
  </signatures>

  <constraints>
    - Provenance comment top of file: "Inspired by ruvnet/RuVector crates/ruvector-temporal-tensor/src/<file>".
    - Little-endian everywhere; no host-dependent ordering.
    - Reject MAX_N to bound allocations from malformed blobs.
    - Reject max == min == 0.0 fail-closed (degenerate reconstruction).
    - File ≤ 500 lines.
    - clippy clean.
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-mejepa-hygiene` succeeds.
    - `cargo test -p context-graph-mejepa-hygiene -- tier_frozen_round_trip` passes
      (signs preserved on 1024 vectors).
    - Cross-host: compute SHA-256 of encoded blob on host A and host B for identical input;
      bytes equal.
    - Compression ratio measured on 768-d float input: bytes_after / bytes_before ≤ 0.04 (≥ 25×).
    - Decode malformed input → TierDecompressFail or Internal.
  </verification>
</definition_of_done>

<pseudo_code>
pub fn encode_sign_only(values: &amp;[f32]) -&gt; OpsResult&lt;Vec&lt;u8&gt;&gt; {
    if values.is_empty() { return Err(OpsError::internal("empty_values")); }
    if values.iter().any(|v| !v.is_finite()) { return Err(OpsError::internal("non_finite")); }
    let n = values.len() as u64;
    let min = values.iter().cloned().fold(f32::INFINITY, f32::min);
    let max = values.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let payload_len = (n as usize + 7) / 8;
    let mut out = Vec::with_capacity(HEADER_LEN + payload_len);
    out.extend_from_slice(&amp;MAGIC_CGTF);
    out.push(VERSION);
    out.push(1u8);                       // bits
    out.extend_from_slice(&amp;n.to_le_bytes());
    out.extend_from_slice(&amp;min.to_le_bytes());
    out.extend_from_slice(&amp;max.to_le_bytes());
    let mut payload = vec![0u8; payload_len];
    for (i, &amp;v) in values.iter().enumerate() {
        if v &gt;= 0.0 { payload[i/8] |= 1u8 &lt;&lt; (i%8); }
    }
    out.extend_from_slice(&amp;payload);
    Ok(out)
}

pub fn decode_sign_only(blob: &amp;[u8]) -&gt; OpsResult&lt;Vec&lt;f32&gt;&gt; {
    if blob.len() &lt; HEADER_LEN { return Err(decompress_fail("blob_too_short", blob)); }
    if blob[0..4] != MAGIC_CGTF { return Err(decompress_fail("bad_magic", blob)); }
    if blob[4] != VERSION { return Err(decompress_fail("bad_version", blob)); }
    if blob[5] != 1 { return Err(decompress_fail("bad_bits", blob)); }
    let n = u64::from_le_bytes(blob[6..14].try_into().unwrap());
    if n &gt; MAX_N { return Err(decompress_fail("n_exceeds_max", blob)); }
    let min = f32::from_le_bytes(blob[14..18].try_into().unwrap());
    let max = f32::from_le_bytes(blob[18..22].try_into().unwrap());
    if max == 0.0 &amp;&amp; min == 0.0 { return Err(decompress_fail("degenerate_zero_range", blob)); }
    let expected_payload = (n as usize + 7) / 8;
    if blob.len() != HEADER_LEN + expected_payload { return Err(decompress_fail("payload_len_mismatch", blob)); }
    let half = max / 2.0;
    let mut out = Vec::with_capacity(n as usize);
    for i in 0..(n as usize) {
        let bit = (blob[HEADER_LEN + i/8] &gt;&gt; (i%8)) &amp; 1;
        out.push(if bit == 1 { half } else { -half });
    }
    Ok(out)
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-hygiene/src/tier_frozen.rs">CGTF 1-bit sign-only encoder + decoder + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-hygiene/src/tests.rs">tier_frozen tests already inline in tier_frozen.rs; add cross-host determinism test here</file>
</files_to_modify>

<validation_criteria>
  <criterion>Sign preservation on 1024 random ±1 inputs</criterion>
  <criterion>Header byte-equal across re-encode (cross-host determinism proxy)</criterion>
  <criterion>Compression ratio ≥ 25× on 768-d Gaussian</criterion>
  <criterion>Bad magic / wrong version / truncated → TierDecompressFail</criterion>
  <criterion>Empty input + non-finite + degenerate-zero rejected</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-hygiene</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- tier_frozen_round_trip</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- tier_frozen_compression_ratio</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- tier_frozen_reject_malformed</command>
  <command>cargo clippy -p context-graph-mejepa-hygiene -- -D warnings</command>
</test_commands>
</task_spec>
```

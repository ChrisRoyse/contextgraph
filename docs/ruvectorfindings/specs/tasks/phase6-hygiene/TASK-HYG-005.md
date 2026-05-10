# TASK-HYG-005 — tier::encode_for_tier + decode_for_tier (CGTC dispatch)

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

Verified `crates/context-graph-tier-compression/src/lib.rs` exists (shipped 2026-05-09) with 8/7/5/3-bit groupwise scalar quantization + CGTC magic header. Task body accurately depends on this shipped crate.

```xml
<task_spec id="TASK-HYG-005" version="1.0">
<metadata>
  <title>tier::encode_for_tier + decode_for_tier (8/7/5/3-bit dispatch via SHIPPED context-graph-tier-compression)</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>5</sequence>
  <implements>
    <requirement_ref>REQ-HYG-02</requirement_ref>
    <requirement_ref>REQ-HYG-06</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-HYG-002</task_ref>
    <task_ref>TASK-HYG-004</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file (extends src/tier.rs)</estimated_files>
</metadata>

<context>
Wires the SHIPPED `context-graph-tier-compression` crate (8/7/5/3-bit groupwise linear scalar
quantization with 22-byte CGTC magic+version-headed serialization) into the tier policy.
Hot tier passes raw f32 bytes (no encoding); Warm/Cool/Cold dispatch to the shipped encoder
at BitWidth::Eight/Five/Three respectively; Frozen tier delegates to TASK-HYG-006 (CGTF magic).
decode_for_tier inspects the first 4 magic bytes and dispatches to either CGTC (shipped crate)
or CGTF (TASK-HYG-006) — magic mismatch returns `MEJEPA_HYG_TIER_DECOMPRESS_FAIL` fail-closed,
no silent zero-fill (per BR-HYG-04, EC-HYG-05).
</context>

<input_context_files>
  <file purpose="tier_compression_api">crates/context-graph-tier-compression/src/lib.rs</file>
  <file purpose="tier_enum">crates/context-graph-mejepa-hygiene/src/tier.rs</file>
  <file purpose="error">crates/context-graph-mejepa-hygiene/src/error.rs</file>
  <file purpose="encode_decode_spec">../../technical/TECH-HYGIENE.md#component_contracts</file>
  <file purpose="reuse_note">../../technical/TECH-HYGIENE.md#implementation_notes</file>
</input_context_files>

<prerequisites>
  <check>TASK-HYG-002 + TASK-HYG-004 shipped</check>
  <check>context-graph-tier-compression dep added in TASK-HYG-001 Cargo.toml</check>
  <check>SHIPPED crate has BitWidth::Eight/Seven/Five/Three + encode/decode/serialize/deserialize</check>
</prerequisites>

<scope>
  <in_scope>
    - EXTEND `src/tier.rs` (still under 500 lines; if it exceeds, split into `tier_codec.rs`):
      * `pub fn encode_for_tier(values: &[f32], tier: Tier) -> OpsResult<Vec<u8>>`
        Hot → return `bytemuck::cast_slice(values).to_vec()` (raw f32 little-endian);
        Warm → tier_compression::encode(values, BitWidth::Eight)?.serialize()?;
        Cool → BitWidth::Five;
        Cold → BitWidth::Three;
        Frozen → call `tier_frozen::encode_sign_only(values)` (forward declared; populated in TASK-HYG-006).
      * `pub fn decode_for_tier(blob: &[u8]) -> OpsResult<Vec<f32>>`
        Read first 4 magic bytes; if "CGTC" → tier_compression::deserialize(blob)?.decode();
        if "CGTF" → tier_frozen::decode_sign_only(blob);
        else → return `OpsError { kind: TierDecompressFail { expected_magic: "CGTC|CGTF",
        actual_magic_hex: hex(first 4), entry_id_cf: "", entry_id_key_hex: "", sub_error: "magic_mismatch" } }`.
        For raw f32 bytes (Hot tier round-trip), require EXACTLY `len % 4 == 0` AND a sentinel
        prefix (non-CGTC/CGTF) — design choice: Hot blobs are kept in RAM only and never
        round-tripped through decode_for_tier. decode_for_tier only handles persisted
        (Warm/Cool/Cold/Frozen) blobs. Hot reads bypass this function.
      * Map upstream `tier_compression::CompressionError` to `OpsError::TierDecompressFail` with
        a structured sub_error string (preserve the upstream `.code()` if available).
    - Reject empty input slice fail-closed with `OpsError::Internal { reason: "empty_values" }`.
    - Reject non-finite f32 fail-closed (NaN/Inf) — match the shipped crate's invariants.
    - Unit tests:
      * Hot encode = raw bytes (len * 4) byte-equal `bytemuck::cast_slice`.
      * Warm/Cool/Cold round-trip on 1000 768-d Gaussian vectors with cosine ≥ shipped FSV
        evidence (0.99999 / 0.99960 / 0.99192).
      * decode_for_tier with random 4-byte garbage → TierDecompressFail.
      * decode_for_tier with CGTC blob → success.
      * Empty input → Internal error.
  </in_scope>
  <out_of_scope>
    - Frozen 1-bit body (TASK-HYG-006 — only forward-declares `tier_frozen::encode_sign_only` /
      `decode_sign_only`).
    - tier_demote / tier_promote (TASK-HYG-007).
    - Per-entry locking (TASK-HYG-007).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-hygiene/src/tier.rs">
    pub fn encode_for_tier(values: &amp;[f32], tier: Tier) -&gt; crate::error::OpsResult&lt;Vec&lt;u8&gt;&gt;;
    pub fn decode_for_tier(blob: &amp;[u8]) -&gt; crate::error::OpsResult&lt;Vec&lt;f32&gt;&gt;;
    pub const MAGIC_CGTC: [u8; 4] = *b"CGTC";
    pub const MAGIC_CGTF: [u8; 4] = *b"CGTF";
    </signature>
  </signatures>

  <constraints>
    - NEVER silently zero-fill on decompression failure (per anti_patterns + EC-HYG-05).
    - Magic byte check is the FIRST gate; truncated input (&lt; 4 bytes) is also TierDecompressFail.
    - Map upstream tier_compression errors via From / via match — do NOT unwrap_or on confidence.
    - Reject NaN/Inf values fail-closed.
    - File still ≤ 500 lines (split into tier_codec.rs if necessary).
    - clippy clean.
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-mejepa-hygiene` succeeds.
    - `cargo test -p context-graph-mejepa-hygiene -- encode_for_tier` passes.
    - `cargo test -p context-graph-mejepa-hygiene -- decode_for_tier_magic_mismatch` passes.
    - Round-trip min cosine matches shipped FSV (0.99999 / 0.99960 / 0.99192) ± 1e-4.
    - `cargo clippy -p context-graph-mejepa-hygiene -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
pub fn encode_for_tier(values: &amp;[f32], tier: Tier) -&gt; OpsResult&lt;Vec&lt;u8&gt;&gt; {
    if values.is_empty() { return Err(OpsError::internal("empty_values")); }
    if values.iter().any(|v| !v.is_finite()) { return Err(OpsError::internal("non_finite")); }
    use context_graph_tier_compression as tc;
    match tier {
        Tier::Hot =&gt; Ok(bytemuck::cast_slice(values).to_vec()),
        Tier::Warm =&gt; tc::encode(values, tc::BitWidth::Eight).and_then(|b| b.serialize())
            .map_err(|e| OpsError { kind: OpsErrorKind::TierDecompressFail {
                entry_id_cf: String::new(), entry_id_key_hex: String::new(),
                expected_magic: "CGTC", actual_magic_hex: String::new(),
                sub_error: format!("encode_warm: {e}") } }),
        Tier::Cool =&gt; /* BitWidth::Five */,
        Tier::Cold =&gt; /* BitWidth::Three */,
        Tier::Frozen =&gt; crate::tier_frozen::encode_sign_only(values),
    }
}

pub fn decode_for_tier(blob: &amp;[u8]) -&gt; OpsResult&lt;Vec&lt;f32&gt;&gt; {
    if blob.len() &lt; 4 {
        return Err(OpsError { kind: OpsErrorKind::TierDecompressFail {
            entry_id_cf: String::new(), entry_id_key_hex: String::new(),
            expected_magic: "CGTC|CGTF", actual_magic_hex: hex::encode(blob),
            sub_error: "blob_too_short".into() } });
    }
    let magic: [u8; 4] = blob[..4].try_into().unwrap();
    match &amp;magic {
        b"CGTC" =&gt; { let cb = tc::deserialize(blob)?; Ok(tc::decode(&amp;cb)?) },
        b"CGTF" =&gt; crate::tier_frozen::decode_sign_only(blob),
        _ =&gt; Err(OpsError { kind: OpsErrorKind::TierDecompressFail {
            entry_id_cf: String::new(), entry_id_key_hex: String::new(),
            expected_magic: "CGTC|CGTF", actual_magic_hex: hex::encode(magic),
            sub_error: "magic_mismatch".into() } }),
    }
}
</pseudo_code>

<files_to_create></files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-hygiene/src/tier.rs">add encode_for_tier + decode_for_tier + magic constants</file>
  <file path="crates/context-graph-mejepa-hygiene/src/tests.rs">add round-trip + magic-mismatch tests</file>
  <file path="crates/context-graph-mejepa-hygiene/Cargo.toml">add hex dep if not present</file>
</files_to_modify>

<validation_criteria>
  <criterion>Warm/Cool/Cold round-trip cosine ≥ shipped FSV minus 1e-4</criterion>
  <criterion>Hot encode = raw f32 bytes (4 * len)</criterion>
  <criterion>decode_for_tier on garbage → TierDecompressFail with actual_magic_hex set</criterion>
  <criterion>Empty input → Internal error; NaN/Inf → Internal error</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-hygiene</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- encode_for_tier_round_trip</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- decode_for_tier_magic_mismatch</command>
  <command>cargo clippy -p context-graph-mejepa-hygiene -- -D warnings</command>
</test_commands>
</task_spec>
```

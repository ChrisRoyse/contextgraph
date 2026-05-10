# TASK-HYG-002 — HygieneError + OpsErrorKind taxonomy

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

Verified against codebase at 2026-05-10: error.rs lives in the not-yet-existent `context-graph-mejepa-hygiene` crate; no existing `OpsError`/`OpsErrorKind`/`MEJEPA_HYG_*` taxonomy in tree. Task body accurate.

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

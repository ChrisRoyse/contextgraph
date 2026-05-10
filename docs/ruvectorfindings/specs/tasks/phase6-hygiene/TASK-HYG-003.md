# TASK-HYG-003 — Reports + status data models

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

No existing GcReport/EvictReport/CompressReport/QuotaStatus structs in tree. Task body accurate.

```xml
<task_spec id="TASK-HYG-003" version="1.0">
<metadata>
  <title>GcReport + GcStepReport + EvictReport + CompressReport + ChainIntegrityReport + QuotaStatus + CategoryStatus + EntryId + DegradationStatus + EvictTrigger + TierTransition + EntryAccessFrequency</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>3</sequence>
  <implements>
    <requirement_ref>REQ-HYG-03</requirement_ref>
    <requirement_ref>REQ-HYG-04</requirement_ref>
    <requirement_ref>REQ-HYG-05</requirement_ref>
    <requirement_ref>REQ-HYG-10</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-HYG-001</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>4 files</estimated_files>
</metadata>

<context>
All persistent + over-the-wire reports for Phase 6. Every type is serde-serializable so the
MCP tools (TASK-HYG-022..024) and the GC report writer (TASK-HYG-019) can emit JSON without
custom serialization code. Constraints (length == 9, ≥ 1.0 ratios, ISO 8601 timestamps) are
encoded as serde validation in `try_from_*` helpers per data integrity. EntryAccessFrequency
is the per-entry sidecar that drives tier policy.
</context>

<input_context_files>
  <file purpose="data_requirements">../../functional/SPEC-HYGIENE.md#data_requirements</file>
  <file purpose="data_models">../../technical/TECH-HYGIENE.md#data_models</file>
  <file purpose="categories_enum">crates/context-graph-mejepa-hygiene/src/categories.rs</file>
  <file purpose="error">crates/context-graph-mejepa-hygiene/src/error.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-HYG-001 shipped</check>
  <check>chrono with serde + clock features available</check>
  <check>uuid with serde feature available</check>
</prerequisites>

<scope>
  <in_scope>
    - CREATE `src/reports.rs` with serde-serializable structs:
      * GcReport (gc_run_id, started_at, completed_at, deferred, deferred_reason,
        aborted, abort_reason, step_reports: Vec<GcStepReport>, total_disk_used_gb_before,
        total_disk_used_gb_after, integrity_check_passed)
      * GcStepReport (step_index: u8, step_name: String, duration_ms: u64,
        entries_processed: u64, bytes_freed: i64, errors_encountered: Vec<String>)
      * EvictReport (trigger: EvictTrigger, category: Category, entries_evicted: u64,
        bytes_freed: u64, reached_under_budget: bool, duration_ms: u64)
      * EvictTrigger enum (EmergencyOverBudget | NightlyBottom1Pct | OperatorManual)
      * CompressReport (threshold_days, segments_compressed, entries_compressed,
        bytes_before, bytes_after, compression_ratio: f64, merkle_roots: Vec<[u8;32]>,
        cold_archive_paths: Vec<String>, integrity_check_passed: bool)
      * ChainIntegrityReport (entries_verified, passed, break_offset: Option<usize>,
        duration_ms)
    - CREATE `src/quota_types.rs` with:
      * QuotaStatus (effective_quota_gb, total_disk_used_gb, categories: Vec<CategoryStatus>,
        system_accepts_new_data_in_categories: HashSet<Category>, snapshot_at)
      * CategoryStatus (category, budget_bytes, used_bytes, degradation_status,
        tier_distribution: BTreeMap<Tier, u64>, oldest_entry_age_seconds: Option<u64>,
        last_gc_attempt_ts: Option<DateTime<Utc>>, last_gc_outcome: Option<String>)
      * DegradationStatus enum (Healthy | Approaching | OverBudget | Unrecoverable)
    - CREATE `src/entry.rs` with:
      * EntryId { cf_name: String, key: Vec<u8> } + helpers to_hex_key/from_hex_key
      * EntryAccessFrequency { entry_id, frequency: f32, last_read_ts_ns: u64, last_demote_ts_ns: u64, last_promote_ts_ns: u64 }
      * TierTransition { from_tier: Tier, to_tier: Tier, at_ts_ns: u64, freq_at_transition: f32 }
        (Tier enum forward-declared; populated in TASK-HYG-004)
    - try_from validators that fail-closed on:
      * GcReport with deferred=true AND deferred_reason=None
      * GcReport with aborted=true AND abort_reason=None
      * QuotaStatus with categories.len() != 9
      * CompressReport with merkle_roots.len() != cold_archive_paths.len()
      * CompressReport with compression_ratio < 1.0
      * GcStepReport with step_index outside 1..=8
    - Unit tests in `src/tests.rs` validating each invariant.
  </in_scope>
  <out_of_scope>
    - Tier enum body (TASK-HYG-004; this task only forward-declares it).
    - Quota math (TASK-HYG-008).
    - CF schema definitions (TASK-HYG-007 / TASK-HYG-010 will use these in writes).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-hygiene/src/reports.rs">
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct GcReport {
        pub gc_run_id: uuid::Uuid,
        pub started_at: chrono::DateTime&lt;chrono::Utc&gt;,
        pub completed_at: Option&lt;chrono::DateTime&lt;chrono::Utc&gt;&gt;,
        pub deferred: bool,
        pub deferred_reason: Option&lt;String&gt;,
        pub aborted: bool,
        pub abort_reason: Option&lt;String&gt;,
        pub step_reports: Vec&lt;GcStepReport&gt;,
        pub total_disk_used_gb_before: f64,
        pub total_disk_used_gb_after: f64,
        pub integrity_check_passed: bool,
    }

    impl GcReport {
        pub fn validate(&amp;self) -&gt; Result&lt;(), crate::error::OpsError&gt;;
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct GcStepReport { pub step_index: u8, pub step_name: String, pub duration_ms: u64,
        pub entries_processed: u64, pub bytes_freed: i64, pub errors_encountered: Vec&lt;String&gt; }

    #[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum EvictTrigger { EmergencyOverBudget, NightlyBottom1Pct, OperatorManual }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct EvictReport { /* fields as above */ }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct CompressReport { /* fields as above */ }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct ChainIntegrityReport { /* fields as above */ }
    </signature>
    <signature file="crates/context-graph-mejepa-hygiene/src/quota_types.rs">
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum DegradationStatus { Healthy, Approaching, OverBudget, Unrecoverable }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct CategoryStatus { /* fields as above */ }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct QuotaStatus { /* fields as above */ }

    impl QuotaStatus { pub fn validate(&amp;self) -&gt; Result&lt;(), crate::error::OpsError&gt;; }
    </signature>
    <signature file="crates/context-graph-mejepa-hygiene/src/entry.rs">
    #[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
    pub struct EntryId { pub cf_name: String, pub key: Vec&lt;u8&gt; }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct EntryAccessFrequency { pub entry_id: EntryId, pub frequency: f32,
        pub last_read_ts_ns: u64, pub last_demote_ts_ns: u64, pub last_promote_ts_ns: u64 }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct TierTransition { pub from_tier: crate::tier::Tier, pub to_tier: crate::tier::Tier,
        pub at_ts_ns: u64, pub freq_at_transition: f32 }
    </signature>
  </signatures>

  <constraints>
    - All structs `#[derive(Serialize, Deserialize)]`.
    - All timestamp fields are `chrono::DateTime<Utc>` (ISO 8601 on serialize).
    - validate() methods return `OpsError::Internal { reason: ... }` on invariant violations.
    - tier_distribution uses BTreeMap (deterministic JSON key order).
    - No silent fallback in validators; they fail-closed.
    - Files under 500 lines.
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-mejepa-hygiene` succeeds.
    - `cargo test -p context-graph-mejepa-hygiene -- reports` passes.
    - JSON round-trip test on a populated GcReport, QuotaStatus, CompressReport.
    - Validator rejects invalid inputs (length mismatch, ratio < 1.0, missing reason).
    - `cargo clippy -p context-graph-mejepa-hygiene -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
impl GcReport {
    pub fn validate(&amp;self) -&gt; OpsResult&lt;()&gt; {
        if self.deferred &amp;&amp; self.deferred_reason.is_none() {
            return Err(OpsError::internal("GcReport.deferred=true but deferred_reason=None"));
        }
        if self.aborted &amp;&amp; self.abort_reason.is_none() {
            return Err(OpsError::internal("GcReport.aborted=true but abort_reason=None"));
        }
        for step in &amp;self.step_reports {
            if !(1..=8).contains(&amp;step.step_index) {
                return Err(OpsError::internal("step_index out of [1,8]"));
            }
        }
        Ok(())
    }
}

impl QuotaStatus {
    pub fn validate(&amp;self) -&gt; OpsResult&lt;()&gt; {
        if self.categories.len() != 9 {
            return Err(OpsError::internal("QuotaStatus.categories.len() must be 9"));
        }
        Ok(())
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-hygiene/src/reports.rs">GC + Evict + Compress + ChainIntegrity reports + validators</file>
  <file path="crates/context-graph-mejepa-hygiene/src/quota_types.rs">QuotaStatus + CategoryStatus + DegradationStatus</file>
  <file path="crates/context-graph-mejepa-hygiene/src/entry.rs">EntryId + EntryAccessFrequency + TierTransition</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-hygiene/src/lib.rs">add `pub mod reports; pub mod quota_types; pub mod entry;`</file>
  <file path="crates/context-graph-mejepa-hygiene/src/tests.rs">add reports_round_trip + quota_status_validate tests</file>
</files_to_modify>

<validation_criteria>
  <criterion>JSON round-trip of populated GcReport / QuotaStatus / CompressReport equals original</criterion>
  <criterion>Validators reject invariant violations with OpsError</criterion>
  <criterion>tier_distribution serializes with deterministic key order</criterion>
  <criterion>All structs compile + clippy clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-hygiene</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- reports</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- quota_status_validate</command>
  <command>cargo clippy -p context-graph-mejepa-hygiene -- -D warnings</command>
</test_commands>
</task_spec>
```

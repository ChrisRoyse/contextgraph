# TASK-HYG-001 — Crate scaffold + HygieneEnv + categories::BUDGETS table

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

Verified against codebase at 2026-05-10:
- `crates/context-graph-mejepa-hygiene/` does NOT exist — task's "create new crate" instruction is accurate.
- Foundation crates `context-graph-tier-compression`, `context-graph-mincut`, `context-graph-witness` DO exist (shipped 2026-05-08/09); path-dep references in `<scope>` are valid.
- All Phase 6 hygiene ops (Merkle compression, nightly GC, 115 GB quota, `me_jepa::ops::*` MCP wrappers) remain PENDING.

```xml
<task_spec id="TASK-HYG-001" version="1.0">
<metadata>
  <title>Crate scaffold + HygieneEnv + categories::BUDGETS table (9 categories × 115 GB total)</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>1</sequence>
  <implements>
    <requirement_ref>REQ-HYG-01</requirement_ref>
  </implements>
  <depends_on></depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>5 files</estimated_files>
</metadata>

<context>
Foundation task: stand up the new crate `context-graph-mejepa-hygiene` and define the immutable
budget table that every subsequent task imports. The 9-category budget table from doc 09 §6.1
totals 115 GB with `CG_MEJEPA_DISK_QUOTA_GB` env override (EC-HYG-08 proportional scaling).
HygieneEnv is the lifecycle struct that threads RocksDB handle + runtime root + cold archive root
+ injectable now_fn through every operation. No quota math, no tiering — just types + table.
</context>

<input_context_files>
  <file purpose="constitution">../../constitution.md</file>
  <file purpose="functional_spec">../../functional/SPEC-HYGIENE.md#requirements</file>
  <file purpose="technical_spec">../../technical/TECH-HYGIENE.md#crate_layout</file>
  <file purpose="technical_spec">../../technical/TECH-HYGIENE.md#data_models</file>
  <file purpose="naming_conventions">../../constitution.md#coding_standards</file>
  <file purpose="anti_patterns">../../constitution.md#anti_patterns</file>
  <file purpose="provenance_pattern">../../../../memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md</file>
</input_context_files>

<prerequisites>
  <check>cargo workspace builds clean on baseline</check>
  <check>workspace Cargo.toml exists at /home/cabdru/contextgraph/Cargo.toml</check>
  <check>rocksdb 0.21.x already in workspace dependencies</check>
  <check>chrono with `serde` + `clock` features available in workspace</check>
</prerequisites>

<scope>
  <in_scope>
    - CREATE `crates/context-graph-mejepa-hygiene/Cargo.toml` with workspace inheritance
      and dependencies: rocksdb 0.21, chrono { features = ["serde","clock"] }, serde,
      serde_json, thiserror, tracing, uuid { features = ["v4","serde"] }, fs2 (flock),
      bytemuck, dashmap (per-entry locks), context-graph-tier-compression (path),
      context-graph-mincut (path), context-graph-witness (path).
    - CREATE `crates/context-graph-mejepa-hygiene/src/lib.rs` with `HygieneEnv` struct
      and `from_runtime_config` constructor; pub mod re-exports for ops, gc, quota,
      witness_compress, tier, tier_frozen, categories, error, mcp.
    - CREATE `crates/context-graph-mejepa-hygiene/src/categories.rs` with `Category`
      enum (9 variants: MutationCorpus, LiveAttemptCorpus, ModelCheckpoints,
      WitnessChains, LivePanelCacheRam, LivePanelDiskOverflow, CalibrationSnapshots,
      AuxSignalsOverrides, ShiftLogSubscriberState), the BUDGETS table as `pub const`
      values, `effective_quota_bytes()`, and `budget_for(Category) -> u64` with
      proportional scaling when `CG_MEJEPA_DISK_QUOTA_GB` is below default.
    - CREATE `crates/context-graph-mejepa-hygiene/src/tests.rs` covering: BUDGETS sum
      equals 116.5 GB upper bound (allowing 1.5 GB buffer to 115 default); env override
      parses; sub-default override scales proportionally; invalid env value falls back to
      default with WARN.
    - ADD the new crate to root `Cargo.toml` `members` array.
    - Every .rs file carries the constitution provenance comment.
  </in_scope>
  <out_of_scope>
    - Quota math beyond budget lookup (TASK-HYG-008).
    - Tier enum, tier compression, tier frozen (TASK-HYG-004, 005, 006).
    - Error taxonomy (TASK-HYG-002).
    - Reports / status data models (TASK-HYG-003).
    - Any GC step (TASK-HYG-012..019).
    - MCP tool dispatch (TASK-HYG-022..024).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-hygiene/src/categories.rs">
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum Category {
        MutationCorpus, LiveAttemptCorpus, ModelCheckpoints, WitnessChains,
        LivePanelCacheRam, LivePanelDiskOverflow, CalibrationSnapshots,
        AuxSignalsOverrides, ShiftLogSubscriberState,
    }

    impl Category {
        pub const fn all() -&gt; &amp;'static [Category; 9];
        pub const fn slug(&amp;self) -&gt; &amp;'static str;
    }

    pub const GB: u64 = 1024 * 1024 * 1024;
    pub const MUTATION_CORPUS_BYTES: u64 = 5 * GB;
    pub const LIVE_ATTEMPT_CORPUS_BYTES: u64 = 50 * GB;
    pub const MODEL_CHECKPOINTS_BYTES: u64 = 10 * GB;
    pub const WITNESS_CHAINS_BYTES: u64 = 20 * GB;
    pub const LIVE_PANEL_CACHE_RAM_BYTES: u64 = 4 * GB;
    pub const LIVE_PANEL_DISK_OVERFLOW_BYTES: u64 = 20 * GB;
    pub const CALIBRATION_SNAPSHOTS_BYTES: u64 = 5 * GB;
    pub const AUX_SIGNALS_OVERRIDES_BYTES: u64 = 2 * GB;
    pub const SHIFT_LOG_SUBSCRIBER_STATE_BYTES: u64 = (1024 * 1024 * 1024) / 2;
    pub const DEFAULT_TOTAL_QUOTA_BYTES: u64 = 115 * GB;

    pub fn effective_quota_bytes() -&gt; u64;
    pub fn budget_for(category: Category) -&gt; u64;
    </signature>
    <signature file="crates/context-graph-mejepa-hygiene/src/lib.rs">
    pub mod categories;
    pub mod error;     // populated by TASK-HYG-002
    pub mod tier;      // populated by TASK-HYG-004
    pub mod tier_frozen;
    pub mod quota;
    pub mod witness_compress;
    pub mod gc;
    pub mod gc_steps;
    pub mod ops;
    pub mod mcp;

    pub struct HygieneEnv {
        pub db: std::sync::Arc&lt;rocksdb::DB&gt;,
        pub runtime_root: std::path::PathBuf,
        pub cold_archive_root: std::path::PathBuf,
        pub witness_threshold_days: u32,
        pub self_healing_handle: Option&lt;std::sync::Arc&lt;dyn SelfHealingHandle + Send + Sync&gt;&gt;,
        pub now_fn: std::sync::Arc&lt;dyn Fn() -&gt; chrono::DateTime&lt;chrono::Utc&gt; + Send + Sync&gt;,
    }

    pub trait SelfHealingHandle {
        fn is_retrain_in_progress(&amp;self) -&gt; bool;
    }

    impl HygieneEnv {
        pub fn from_runtime_config(cfg: &amp;RuntimeConfig) -&gt; Result&lt;Self, error::OpsError&gt;;
        pub fn now(&amp;self) -&gt; chrono::DateTime&lt;chrono::Utc&gt;;
    }

    pub struct RuntimeConfig {
        pub db: std::sync::Arc&lt;rocksdb::DB&gt;,
        pub runtime_root: std::path::PathBuf,
        pub cold_archive_root: Option&lt;std::path::PathBuf&gt;,
        pub witness_threshold_days: Option&lt;u32&gt;,
    }
    </signature>
  </signatures>

  <constraints>
    - Crate name matches kebab-case `context-graph-mejepa-*` convention.
    - All numeric constants use `* GB` form (no magic literals).
    - `budget_for` MUST scale all 9 categories proportionally when env override is below
      `DEFAULT_TOTAL_QUOTA_BYTES`; emits `tracing::warn!` once at startup with
      override value + scale factor.
    - Invalid env values (non-finite, ≤ 0, non-numeric) MUST log WARN and fall back to
      `DEFAULT_TOTAL_QUOTA_BYTES`.
    - Files under 500 lines.
    - Provenance comment at top of every .rs file.
    - No `unwrap_or` on quota / confidence paths (per anti_patterns).
    - `#[serde(rename_all = "snake_case")]` on Category enum so JSON keys are
      `mutation_corpus`, `live_attempt_corpus`, etc.
    - No clippy warnings under `-D warnings`.
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-mejepa-hygiene` succeeds.
    - `cargo test -p context-graph-mejepa-hygiene -- categories` passes.
    - `cargo clippy -p context-graph-mejepa-hygiene -- -D warnings` clean.
    - `wc -l crates/context-graph-mejepa-hygiene/src/*.rs` shows max ≤ 500.
  </verification>
</definition_of_done>

<pseudo_code>
// categories.rs
pub fn effective_quota_bytes() -&gt; u64 {
    match std::env::var("CG_MEJEPA_DISK_QUOTA_GB") {
        Ok(raw) =&gt; match raw.parse::&lt;f64&gt;() {
            Ok(gb) if gb.is_finite() &amp;&amp; gb &gt;= 1.0 =&gt; (gb * GB as f64).clamp(GB as f64, (u64::MAX/2) as f64) as u64,
            _ =&gt; { tracing::warn!(target:"hygiene", "invalid CG_MEJEPA_DISK_QUOTA_GB={raw}; falling back to default 115 GB"); DEFAULT_TOTAL_QUOTA_BYTES }
        },
        Err(_) =&gt; DEFAULT_TOTAL_QUOTA_BYTES,
    }
}

pub fn budget_for(category: Category) -&gt; u64 {
    let raw = match category { Category::MutationCorpus =&gt; MUTATION_CORPUS_BYTES, ... };
    let effective = effective_quota_bytes();
    if effective &gt;= DEFAULT_TOTAL_QUOTA_BYTES { return raw; }
    // scale proportionally
    let scale = effective as f64 / DEFAULT_TOTAL_QUOTA_BYTES as f64;
    (raw as f64 * scale) as u64
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-hygiene/Cargo.toml">workspace member with dependencies</file>
  <file path="crates/context-graph-mejepa-hygiene/src/lib.rs">HygieneEnv + module re-exports + SelfHealingHandle trait</file>
  <file path="crates/context-graph-mejepa-hygiene/src/categories.rs">Category enum + BUDGETS table + effective_quota_bytes/budget_for</file>
  <file path="crates/context-graph-mejepa-hygiene/src/tests.rs">unit tests for budgets sum + env override + proportional scaling</file>
  <file path="crates/context-graph-mejepa-hygiene/src/error.rs">empty placeholder; populated in TASK-HYG-002</file>
</files_to_create>

<files_to_modify>
  <file path="Cargo.toml">add `crates/context-graph-mejepa-hygiene` to workspace members array</file>
</files_to_modify>

<validation_criteria>
  <criterion>9 Category variants; slug round-trip works for all</criterion>
  <criterion>Sum of all 9 BUDGETS equals 116.5 GB upper bound (≤ 116.5 * GB)</criterion>
  <criterion>DEFAULT_TOTAL_QUOTA_BYTES = 115 * GB</criterion>
  <criterion>effective_quota_bytes returns env override when valid</criterion>
  <criterion>budget_for proportionally scales when env override < default</criterion>
  <criterion>Crate compiles release; tests pass; clippy clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-hygiene</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- categories</command>
  <command>cargo clippy -p context-graph-mejepa-hygiene -- -D warnings</command>
  <command>find crates/context-graph-mejepa-hygiene/src -name "*.rs" -exec wc -l {} +</command>
</test_commands>
</task_spec>
```

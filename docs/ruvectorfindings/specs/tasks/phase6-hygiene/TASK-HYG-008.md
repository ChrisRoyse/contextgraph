# TASK-HYG-008 — quota::snapshot + degradation_for + score_for_eviction

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

`CF_MEJEPA_QUOTA_STATE` is NOT in existing `heal/cf.rs` declarations — this task is its first declarer (correct). Task body accurate.

```xml
<task_spec id="TASK-HYG-008" version="1.0">
<metadata>
  <title>quota::snapshot + degradation_for + score_for_eviction (recency exp decay; uplift × freq × recency)</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>8</sequence>
  <implements>
    <requirement_ref>REQ-HYG-04</requirement_ref>
    <requirement_ref>REQ-HYG-10</requirement_ref>
    <requirement_ref>REQ-HYG-05</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-HYG-003</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
Pure-math quota layer. `snapshot` reads cached per-CF size estimates and per-CF Unrecoverable
flags from CF_MEJEPA_QUOTA_STATE and assembles a QuotaStatus that the MCP `mejepa_quota_status`
tool serializes. `degradation_for` maps `(used, budget, unrecoverable)` → DegradationStatus
(Healthy &lt; 80%, Approaching 80-95%, OverBudget 95-100%, Unrecoverable post-failed-GC, sticky).
`score_for_eviction` is the deterministic ranking function for both nightly bottom-1% and
emergency eviction (`uplift × access_frequency × recency_factor`; recency_factor =
exp(-recency_seconds / 30d)). Latency target &lt; 50 ms p50 (NFR-HYG-03) — read cache, not full CF.
</context>

<input_context_files>
  <file purpose="quota_component">../../technical/TECH-HYGIENE.md#component_contracts</file>
  <file purpose="degradation_thresholds">../../functional/SPEC-HYGIENE.md#requirements</file>
  <file purpose="reports_types">crates/context-graph-mejepa-hygiene/src/reports.rs</file>
  <file purpose="quota_types">crates/context-graph-mejepa-hygiene/src/quota_types.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-HYG-003 shipped (QuotaStatus + CategoryStatus + DegradationStatus exist)</check>
</prerequisites>

<scope>
  <in_scope>
    - CREATE `src/quota.rs` with:
      * `pub fn snapshot(env: &HygieneEnv) -> OpsResult<QuotaStatus>`:
        For each Category: read cached size from CF_MEJEPA_QUOTA_STATE entry "size_<slug>"
        OR fall back to `db.get_property_cf_int("rocksdb.estimate-num-keys")` + sum-of-value-sizes
        (cached at end of each GC pass per <note category="performance">). Compute degradation
        via `degradation_for`. Build CategoryStatus with tier_distribution (read from
        CF_MEJEPA_PANEL_META aggregated counts; empty for non-tiered categories).
        Assemble QuotaStatus; latency &lt; 50 ms p50.
      * `pub fn degradation_for(used: u64, budget: u64, unrecoverable: bool) -> DegradationStatus`:
        if unrecoverable: Unrecoverable
        else if used >= budget: OverBudget
        else if used * 100 >= budget * 95: OverBudget    (95-100% bucket merged with OverBudget per spec table)
        else if used * 100 >= budget * 80: Approaching
        else: Healthy
      * `pub fn score_for_eviction(uplift: f32, access_freq: f32, recency_seconds: u64) -> f32`:
        const SECONDS_PER_30_DAYS: f64 = 30.0 * 86400.0;
        let recency_factor = (-(recency_seconds as f64) / SECONDS_PER_30_DAYS).exp() as f32;
        uplift * access_freq * recency_factor
        Pure function; no I/O. Lowest score evicted first.
      * `pub fn read_unrecoverable_set(env: &HygieneEnv) -> OpsResult<HashSet<Category>>`:
        Read CF_MEJEPA_QUOTA_STATE; collect Categories with unrecoverable_since_ts: Some(_).
      * `pub fn write_unrecoverable(env: &HygieneEnv, category: Category, since_ts: u64,
        last_gc_outcome: &str) -> OpsResult<()>`: write to CF_MEJEPA_QUOTA_STATE.
    - DEFINE `pub const CF_MEJEPA_QUOTA_STATE: &str = "CF_MEJEPA_QUOTA_STATE";`
    - Unit tests:
      * degradation_for boundaries: 79% Healthy, 80% Approaching, 94% Approaching, 95% OverBudget,
        100% OverBudget, anything + unrecoverable=true → Unrecoverable.
      * score_for_eviction monotonic: doubling uplift doubles score; exp decay matches reference
        (recency=0 → factor 1.0; recency=30d → factor 1/e ≈ 0.368).
      * snapshot with empty CFs returns 9 Healthy CategoryStatus entries; total_disk_used_gb = 0.
  </in_scope>
  <out_of_scope>
    - Eviction itself (TASK-HYG-009 + TASK-HYG-014).
    - GC step 3 sweeping (TASK-HYG-014).
    - MCP shim (TASK-HYG-023).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-hygiene/src/quota.rs">
    pub const CF_MEJEPA_QUOTA_STATE: &amp;str = "CF_MEJEPA_QUOTA_STATE";

    pub fn snapshot(env: &amp;crate::HygieneEnv) -&gt; crate::error::OpsResult&lt;crate::quota_types::QuotaStatus&gt;;
    pub fn degradation_for(used: u64, budget: u64, unrecoverable: bool) -&gt; crate::quota_types::DegradationStatus;
    pub fn score_for_eviction(uplift: f32, access_freq: f32, recency_seconds: u64) -&gt; f32;
    pub fn read_unrecoverable_set(env: &amp;crate::HygieneEnv) -&gt; crate::error::OpsResult&lt;std::collections::HashSet&lt;crate::categories::Category&gt;&gt;;
    pub fn write_unrecoverable(env: &amp;crate::HygieneEnv, category: crate::categories::Category, since_ts: u64, last_gc_outcome: &amp;str) -&gt; crate::error::OpsResult&lt;()&gt;;
    </signature>
  </signatures>

  <constraints>
    - degradation_for is pure (no I/O); thresholds are integer ratios (used*100 / budget) to avoid float drift.
    - score_for_eviction is deterministic given inputs.
    - snapshot has p50 &lt; 50 ms target; uses cached values, not full CF scan.
    - No unwrap_or on confidence; no silent fallback.
    - File ≤ 500 lines.
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-mejepa-hygiene` succeeds.
    - `cargo test -p context-graph-mejepa-hygiene -- degradation_for_thresholds` passes.
    - `cargo test -p context-graph-mejepa-hygiene -- score_for_eviction_monotonic` passes.
    - `cargo test -p context-graph-mejepa-hygiene -- snapshot_empty_returns_9_healthy` passes.
    - clippy clean.
  </verification>
</definition_of_done>

<pseudo_code>
pub fn degradation_for(used: u64, budget: u64, unrecoverable: bool) -&gt; DegradationStatus {
    if unrecoverable { return DegradationStatus::Unrecoverable; }
    if budget == 0 { return DegradationStatus::OverBudget; } // defensive
    if used &gt;= budget { return DegradationStatus::OverBudget; }
    let pct = (used as u128 * 100) / budget as u128;
    if pct &gt;= 95 { DegradationStatus::OverBudget }
    else if pct &gt;= 80 { DegradationStatus::Approaching }
    else { DegradationStatus::Healthy }
}

pub fn score_for_eviction(uplift: f32, access_freq: f32, recency_seconds: u64) -&gt; f32 {
    const SECONDS_PER_30_DAYS: f64 = 30.0 * 86400.0;
    let recency_factor = (-(recency_seconds as f64) / SECONDS_PER_30_DAYS).exp() as f32;
    uplift * access_freq * recency_factor
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-hygiene/src/quota.rs">snapshot + degradation_for + score_for_eviction + CF state helpers</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-hygiene/src/lib.rs">module already declared in TASK-HYG-001; this populates body</file>
  <file path="crates/context-graph-mejepa-hygiene/src/tests.rs">add quota math tests</file>
</files_to_modify>

<validation_criteria>
  <criterion>degradation_for: 79%→Healthy, 80%→Approaching, 94%→Approaching, 95%→OverBudget, 100%→OverBudget</criterion>
  <criterion>score_for_eviction(1,1,0) ≈ 1.0; score(1,1,30d) ≈ 1/e ≈ 0.368</criterion>
  <criterion>snapshot on empty DB returns 9 Healthy entries totaling 0 GB</criterion>
  <criterion>read_unrecoverable_set + write_unrecoverable round-trip</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-hygiene</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- degradation_for_thresholds</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- score_for_eviction_monotonic</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- snapshot_empty_returns_9_healthy</command>
  <command>cargo clippy -p context-graph-mejepa-hygiene -- -D warnings</command>
</test_commands>
</task_spec>
```

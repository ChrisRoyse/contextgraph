# TASK-HYG-009 — quota_check_and_evict + Unrecoverable transition

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

## 📋 2026-05-10 STATE AUDIT — flagged

Task body references `CF_MEJEPA_WEIGHT_BLOBS_META` (line 63) and `CF_MEJEPA_QUOTA_STATE`. NEITHER is declared in `crates/context-graph-mejepa/src/heal/cf.rs` yet. `CF_MEJEPA_WEIGHT_BLOBS` IS already declared in heal/cf.rs — the task should re-use that const (via re-export or import) rather than redeclare. Add `CF_MEJEPA_WEIGHT_BLOBS_META` as a NEW declaration in the new hygiene crate. No other drift.

```xml
<task_spec id="TASK-HYG-009" version="1.0">
<metadata>
  <title>quota_check_and_evict + Unrecoverable transition (emergency eviction; gold-tier exclusion; alert)</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>9</sequence>
  <implements>
    <requirement_ref>REQ-HYG-05</requirement_ref>
    <requirement_ref>REQ-HYG-04</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-HYG-008</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
Hard quota enforcement: triggers immediately (not nightly) when any category exceeds budget.
Reuses the same `score_for_eviction` formula as nightly step 3 (TASK-HYG-014) but evicts
until-under-budget rather than bottom-1%. Excludes gold-tier (tagged_release on checkpoints,
operator-tagged overrides on aux_signals). On exhaustion of candidates → category transitions
to Unrecoverable (CF_MEJEPA_QUOTA_STATE), `tracing::error!` alert raised, subsequent writers
get `OpsError::QuotaUnrecoverable` (per ERR-HYG-02 / EC-HYG-03).
</context>

<input_context_files>
  <file purpose="emergency_eviction">../../technical/TECH-HYGIENE.md#architecture_diagram</file>
  <file purpose="ops_method">../../technical/TECH-HYGIENE.md#component_contracts</file>
  <file purpose="business_rule">../../functional/SPEC-HYGIENE.md#business_rules</file>
  <file purpose="quota_module">crates/context-graph-mejepa-hygiene/src/quota.rs</file>
  <file purpose="error">crates/context-graph-mejepa-hygiene/src/error.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-HYG-008 shipped (snapshot + score_for_eviction + write_unrecoverable)</check>
</prerequisites>

<scope>
  <in_scope>
    - EXTEND `src/quota.rs` (or split to `src/quota_evict.rs` if &gt; 500 lines):
      * `pub fn quota_check_and_evict(env: &HygieneEnv, category: Category) -> OpsResult<EvictReport>`:
        1. Read current usage of `category`.
        2. If `used &lt; budget`: return `EvictReport { trigger: EmergencyOverBudget, entries_evicted: 0, bytes_freed: 0, reached_under_budget: true, ... }`.
        3. Build candidate set: all entries in category's CFs EXCLUDING gold-tier
           (tagged_release on checkpoints; operator-tagged on aux_signals; constellation
           centroids in aux_signals).
        4. For each candidate, compute `score_for_eviction(uplift, freq, recency_seconds)`
           reading from CF_MEJEPA_PANEL_META + uplift sidecar (uplift defaults to 1.0 when
           category has no uplift signal — pure freq*recency for non-attempt corpus categories).
        5. Sort candidates ascending by score.
        6. Iterate from lowest: db.delete_cf for each entry; sum bytes_freed; stop when
           `used &lt;= budget` OR no candidates left.
        7. If reached under budget: return success EvictReport.
        8. If exhausted: write_unrecoverable(env, category, now_ts, "no_progress");
           tracing::error!(target:"hygiene", "category {:?} unrecoverable", category);
           return Err(OpsError { kind: QuotaUnrecoverable { category, pass_freed_bytes,
           pass_duration_ms } }).
      * Identify gold-tier entries via:
        - ModelCheckpoints: read sidecar `tagged_release: bool` (CF_MEJEPA_WEIGHT_BLOBS_META).
        - AuxSignalsOverrides: read `operator_tagged: bool` flag on the entry.
        - All other categories: no gold-tier exclusion (every entry is fair game).
      * Acquire the global GC flock for the duration of emergency eviction (per
        <note category="locking">) to prevent overlap with nightly GC.
    - Unit tests:
      * Synthetic 1 MB category at 110% with mix of evictable + gold-tier; first call frees
        space; category Healthy.
      * Synthetic category with all gold-tier entries at 110%; call returns
        QuotaUnrecoverable; CF_MEJEPA_QUOTA_STATE has Unrecoverable entry; subsequent call
        also fails.
      * EvictReport.duration_ms is positive.
      * Concurrent emergency evictions on different categories run in parallel? No — the
        global flock serializes them. Document this in the test.
  </in_scope>
  <out_of_scope>
    - Nightly bottom-1% sweep (TASK-HYG-014).
    - Manual operator override unwind (out of spec — operator runbook).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-hygiene/src/quota.rs">
    pub fn quota_check_and_evict(
        env: &amp;crate::HygieneEnv,
        category: crate::categories::Category,
    ) -&gt; crate::error::OpsResult&lt;crate::reports::EvictReport&gt;;
    </signature>
  </signatures>

  <constraints>
    - Acquires global GC flock at `~/.cache/cgreality/mejepa-gc.lock` for duration.
    - Gold-tier exclusion is checked PER ENTRY (not per category) — supports mixed scenarios.
    - Unrecoverable is sticky: once written, only operator action clears it.
    - tracing::error! is the alert mechanism (logged + observable).
    - Subsequent writes to Unrecoverable category fail at the writer side with
      QuotaUnrecoverable (caller of writer checks via snapshot before writing).
    - File ≤ 500 lines (split if needed).
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-mejepa-hygiene` succeeds.
    - `cargo test -p context-graph-mejepa-hygiene -- quota_evict_recovers` passes.
    - `cargo test -p context-graph-mejepa-hygiene -- quota_evict_unrecoverable` passes.
    - `cargo test -p context-graph-mejepa-hygiene -- quota_evict_gold_tier_excluded` passes.
    - clippy clean.
  </verification>
</definition_of_done>

<pseudo_code>
pub fn quota_check_and_evict(env: &amp;HygieneEnv, category: Category) -&gt; OpsResult&lt;EvictReport&gt; {
    let _flock = acquire_global_gc_lock()?;
    let started = std::time::Instant::now();
    let used = read_category_usage(env, category)?;
    let budget = budget_for(category);
    if used &lt; budget {
        return Ok(EvictReport { trigger: EvictTrigger::EmergencyOverBudget,
            category, entries_evicted: 0, bytes_freed: 0, reached_under_budget: true,
            duration_ms: started.elapsed().as_millis() as u64 });
    }
    let mut candidates = collect_candidates(env, category)?;
    candidates.retain(|c| !is_gold_tier(env, category, &amp;c.entry_id));
    candidates.sort_by(|a, b| a.score.partial_cmp(&amp;b.score).unwrap_or(std::cmp::Ordering::Equal));
    let mut total_freed = 0u64;
    let mut count = 0u64;
    let mut current_used = used;
    for c in &amp;candidates {
        if current_used &lt;= budget { break; }
        let n = delete_entry(env, &amp;c.entry_id)?;
        total_freed += n;
        current_used = current_used.saturating_sub(n);
        count += 1;
    }
    let dur = started.elapsed().as_millis() as u64;
    if current_used &lt;= budget {
        Ok(EvictReport { trigger: EvictTrigger::EmergencyOverBudget,
            category, entries_evicted: count, bytes_freed: total_freed,
            reached_under_budget: true, duration_ms: dur })
    } else {
        let now = env.now().timestamp_nanos_opt().unwrap_or(0) as u64;
        write_unrecoverable(env, category, now, "no_progress")?;
        tracing::error!(target:"hygiene", category=?category, pass_freed_bytes=total_freed, "category unrecoverable");
        Err(OpsError { kind: OpsErrorKind::QuotaUnrecoverable { category,
            pass_freed_bytes: total_freed, pass_duration_ms: dur } })
    }
}
</pseudo_code>

<files_to_create></files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-hygiene/src/quota.rs">add quota_check_and_evict + helpers</file>
  <file path="crates/context-graph-mejepa-hygiene/src/tests.rs">add 3 emergency eviction tests</file>
</files_to_modify>

<validation_criteria>
  <criterion>Mixed evictable+gold tier at 110% recovers to Healthy after eviction</criterion>
  <criterion>All gold-tier scenario transitions to Unrecoverable + alert logged</criterion>
  <criterion>Subsequent call on Unrecoverable category still returns QuotaUnrecoverable</criterion>
  <criterion>EvictReport fields populated correctly</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-hygiene</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- quota_evict_recovers</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- quota_evict_unrecoverable</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- quota_evict_gold_tier_excluded</command>
  <command>cargo clippy -p context-graph-mejepa-hygiene -- -D warnings</command>
</test_commands>
</task_spec>
```

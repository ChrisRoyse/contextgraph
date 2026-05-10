# TASK-HYG-014 — gc_steps::step3_evict (bottom-1pct + optional Stoer-Wagner mincut leakage)

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

Verified `crates/context-graph-mincut/src/lib.rs` exists (shipped 2026-05-09) with Stoer-Wagner global mincut. Path-dep usage in task is correct.

```xml
<task_spec id="TASK-HYG-014" version="1.0">
<metadata>
  <title>gc_steps::step3_evict — bottom-1% by score; optional Stoer-Wagner leakage penalty (reuse SHIPPED context-graph-mincut)</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>14</sequence>
  <implements>
    <requirement_ref>REQ-HYG-03</requirement_ref>
    <requirement_ref>REQ-HYG-05</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-HYG-008</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
Step 3: when total corpus exceeds budget, evict bottom-1% by score (uplift × access_frequency
× recency_factor). For corpus categories that span repo-atomic split pairs (training / holdout),
optionally apply Stoer-Wagner mincut to find the cut minimizing cross-set edge weight; entries
on the heavy side of the cut are deprioritized for eviction (preserves split stability per
<note category="reuse">). v1.0 simplification: leakage penalty may be SKIPPED on the first impl
cut and added after FSV stress run is green. Gold-tier excluded same as TASK-HYG-009.
</context>

<input_context_files>
  <file purpose="step_spec">../../technical/TECH-HYGIENE.md#component_contracts</file>
  <file purpose="mincut_api">crates/context-graph-mincut/src/lib.rs</file>
  <file purpose="quota_helpers">crates/context-graph-mejepa-hygiene/src/quota.rs</file>
  <file purpose="reuse_note">../../technical/TECH-HYGIENE.md#implementation_notes</file>
</input_context_files>

<prerequisites>
  <check>TASK-HYG-008 shipped (score_for_eviction)</check>
  <check>SHIPPED context-graph-mincut available + dep added in TASK-HYG-001</check>
</prerequisites>

<scope>
  <in_scope>
    - CREATE `src/gc_steps/step3_evict.rs`:
      * `pub fn run(env: &HygieneEnv, report: &mut GcReport) -> OpsResult<()>`:
        1. For each tiered category: read used + budget.
        2. If used &lt; budget: continue (no eviction needed for this category).
        3. Build candidate set: all entries excluding gold-tier (per category rules from TASK-HYG-009).
        4. Score each: score_for_eviction(uplift, freq, recency_seconds).
        5. Sort ascending; take bottom 1% (count = max(1, candidates.len() / 100)).
        6. (OPTIONAL leakage penalty, behind a feature flag `mincut-leakage` default-on
           but SKIPPABLE for v1.0 first cut):
           - Build a graph: nodes = candidates; edges = "co-task" (entries from same task_id
             share an edge with weight = co-occurrence count); use SHIPPED
             context_graph_mincut::stoer_wagner to find min cut. Entries on the heavy side
             of the cut shift their score upward by a fixed penalty (less likely to evict).
        7. Iterate candidates in score order; delete from CF; sum bytes_freed.
        8. Build GcStepReport; push to report.step_reports.
    - Cargo feature `mincut-leakage` default = on; if off, skip step 6 and use raw scores.
    - Unit / integration tests:
      * Category at 110% budget; 1000 entries; bottom-1% (10) evicted; bytes_freed reported.
      * Empty category → entries_evicted = 0.
      * All gold-tier scenario → entries_evicted = 0; logged WARN.
      * Mincut leakage feature off: pure score order. Feature on: heavy-side entries kept.
  </in_scope>
  <out_of_scope>
    - Emergency eviction (TASK-HYG-009; this step is the nightly bottom-1% sweep only).
    - Unrecoverable transition (handled in step 8 / orchestrator).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-hygiene/src/gc_steps/step3_evict.rs">
    pub fn run(env: &amp;crate::HygieneEnv, report: &amp;mut crate::reports::GcReport) -&gt; crate::error::OpsResult&lt;()&gt;;
    </signature>
    <signature file="crates/context-graph-mejepa-hygiene/Cargo.toml">
    [features]
    default = ["mincut-leakage"]
    mincut-leakage = []
    </signature>
  </signatures>

  <constraints>
    - Reuses score_for_eviction from TASK-HYG-008 unchanged (deterministic ordering).
    - Mincut leakage is OPTIONAL via cargo feature (per <note category="reuse"> v1.0 simplification).
    - Gold-tier exclusion identical rule to TASK-HYG-009 (tagged_release on checkpoints; operator-tagged on aux_signals).
    - Step does not transition to Unrecoverable on its own — that's emergency eviction's job;
      this step just reports entries_evicted; orchestrator's step 8 reconciles.
    - File ≤ 500 lines.
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-mejepa-hygiene` succeeds (default features).
    - `cargo build --release -p context-graph-mejepa-hygiene --no-default-features` succeeds (no mincut-leakage).
    - `cargo test -p context-graph-mejepa-hygiene -- step3_evict_bottom_1pct` passes.
    - `cargo test -p context-graph-mejepa-hygiene -- step3_evict_all_gold_skips` passes.
    - clippy clean both feature configs.
  </verification>
</definition_of_done>

<pseudo_code>
pub fn run(env: &amp;HygieneEnv, report: &amp;mut GcReport) -&gt; OpsResult&lt;()&gt; {
    let started = std::time::Instant::now();
    let mut entries_evicted = 0u64;
    let mut bytes_freed = 0i64;
    let mut errs = Vec::new();
    for category in Category::all().iter().copied() {
        let used = read_category_usage(env, category)?;
        let budget = budget_for(category);
        if used &lt; budget { continue; }
        let mut cands = collect_candidates(env, category)?;
        cands.retain(|c| !is_gold_tier(env, category, &amp;c.entry_id));
        if cands.is_empty() { errs.push("no_evictable_candidates".into()); continue; }
        for c in &amp;mut cands {
            c.score = score_for_eviction(c.uplift, c.access_freq, c.recency_seconds);
        }
        #[cfg(feature = "mincut-leakage")]
        apply_mincut_leakage_penalty(&amp;mut cands, env, category)?;
        cands.sort_by(|a, b| a.score.partial_cmp(&amp;b.score).unwrap_or(std::cmp::Ordering::Equal));
        let n = (cands.len() / 100).max(1);
        for c in cands.iter().take(n) {
            let bytes = delete_entry(env, &amp;c.entry_id)?;
            bytes_freed += bytes as i64;
            entries_evicted += 1;
        }
    }
    report.step_reports.push(GcStepReport { step_index: 3, step_name: "evict".into(),
        duration_ms: started.elapsed().as_millis() as u64, entries_processed: entries_evicted,
        bytes_freed, errors_encountered: errs });
    Ok(())
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-hygiene/src/gc_steps/step3_evict.rs">bottom-1% sweep + optional mincut</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-hygiene/src/gc_steps/mod.rs">declare step3_evict</file>
  <file path="crates/context-graph-mejepa-hygiene/Cargo.toml">add mincut-leakage feature flag</file>
</files_to_modify>

<validation_criteria>
  <criterion>1000-entry category at 110%: bottom 10 (1%) evicted</criterion>
  <criterion>--no-default-features build skips mincut, uses pure score order</criterion>
  <criterion>All gold-tier → entries_evicted=0, "no_evictable_candidates" logged</criterion>
  <criterion>Mincut leakage feature: heavy-side cut entries deprioritized</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-hygiene</command>
  <command>cargo build --release -p context-graph-mejepa-hygiene --no-default-features</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- step3_evict_bottom_1pct</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- step3_evict_all_gold_skips</command>
  <command>cargo clippy -p context-graph-mejepa-hygiene -- -D warnings</command>
</test_commands>
</task_spec>
```

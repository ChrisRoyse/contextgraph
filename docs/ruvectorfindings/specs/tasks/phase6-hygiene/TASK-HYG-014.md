# TASK-HYG-014 — gc_steps::step3_evict (bottom-1pct + optional Stoer-Wagner mincut leakage)

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

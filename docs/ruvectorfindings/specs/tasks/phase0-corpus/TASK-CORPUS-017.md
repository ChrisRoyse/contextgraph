# TASK-CORPUS-017 — split_corpus + assert_no_test_patch_leakage

```xml
<task_spec id="TASK-CORPUS-017" version="1.0">
<metadata>
  <title>split_corpus + assert_no_test_patch_leakage</title>
  <status>complete</status>
  <implementation_state as_of="2026-05-09">
    Shipped: `split_corpus(entries, config) -> SplitResult<SplitReport>` in `split.rs:294-475` — InstanceAtomic (default) places every instance independently into the lowest-current-ratio bucket, RepoAtomic assigns whole repos to one bucket using LPT pre-sort + SplitMix64 shuffle. `assert_no_test_patch_leakage(assignments, sha_lookup)` (split.rs:484-512) reports a `LeakageDetected`-shaped error (currently re-using `OracleError`-style payload) when the same test_patch SHA lands in multiple buckets. `split_is_leakage_free(report)` (split.rs:618) used as a Stoer-Wagner-mincut-backed cross-check. 50 unit tests + FSV at `/mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/{split-evidence.json,split-assignments.json}`.
  </implementation_state>
  <layer>logic</layer>
  <sequence>17</sequence>
  <implements>
    <requirement_ref>REQ-CORPUS-06</requirement_ref>
    <requirement_ref>REQ-CORPUS-07</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORPUS-005</task_ref>
    <task_ref>TASK-CORPUS-011</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
</metadata>

<context>
Logic task. Repo-atomic / instance-atomic split with deterministic
SplitMix64 shuffle, LPT pre-sort, lowest-current/target-ratio
bin-pack. Plus the structural test-patch leakage assertion.
Already shipped per project memory; this task verifies the
shipped state, locks tests for EC-CORPUS-04 (1e-9 tolerance) and
EC-CORPUS-05 (RepoAtomic structural bound).
</context>

<input_context_files>
  <file purpose="component_contract">../../technical/TECH-CORPUS.md#component_contracts (SplitEngine)</file>
  <file purpose="api_contract">../../technical/TECH-CORPUS.md#api_contracts (split_corpus, assert_no_test_patch_leakage)</file>
  <file purpose="edge_cases">../../functional/SPEC-CORPUS.md (EC-CORPUS-04, EC-CORPUS-05, BR-CORPUS-04)</file>
  <file purpose="test_plan">../../functional/SPEC-CORPUS.md (TC-CORPUS-06, 07, 08)</file>
  <file purpose="existing">crates/context-graph-mejepa-corpus/src/split.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORPUS-005 complete (SplitConfig / SplitReport / TaskAssignment exist)</check>
  <check>TASK-CORPUS-011 complete (shuffle_in_place + pick_index_or_fail exist)</check>
  <check>TASK-CORPUS-001 complete (MutationError::LeakageDetected exists)</check>
</prerequisites>

<scope>
  <in_scope>
    - Verify `pub fn split_corpus(entries: &amp;[CorpusEntry],
      config: &amp;SplitConfig) -&gt; MutationResult&lt;SplitReport&gt;`:
      1. Validate fractions sum to 1.0 ± 1e-9 (EC-CORPUS-04);
         non-negative finite.
      2. Validate entries non-empty, no duplicate task_ids,
         non-empty repo + task_id strings.
      3. Build assignment units (tasks for InstanceAtomic, repos
         for RepoAtomic).
      4. SplitMix64-shuffle units by `config.seed` via
         `shuffle_in_place`.
      5. LPT pre-sort by descending unit size (no-op for
         InstanceAtomic).
      6. For each unit in order: bin-pack into the bucket with
         the lowest `current_count / target_count` ratio.
      7. Compute bucket_sizes / bucket_fractions /
         repo_assignments / leakage_check_passed.
      8. Emit stderr warning if RepoAtomic ratio deviation &gt; 5%
         (BR-CORPUS-04).
    - Verify `pub fn assert_no_test_patch_leakage(assignments:
      &amp;[TaskAssignment], test_patch_shas:
      &amp;BTreeMap&lt;String, String&gt;) -&gt;
      MutationResult&lt;()&gt;`:
      1. Build inverse map `BTreeMap&lt;sha,
         BTreeMap&lt;bucket, Vec&lt;task_id&gt;&gt;&gt;`.
      2. Any sha appearing in 2+ buckets returns
         `MEJEPA_CORPUS_LEAKAGE_DETECTED` with offending sha and
         all (task_id, bucket) collisions.
    - Verify deterministic test (NFR-CORPUS-04) — split twice
      with same SplitConfig.seed → byte-equal SplitReport.
    - Verify TC-CORPUS-06: InstanceAtomic on real 300-task
      manifest produces [240, 30, 30].
    - Verify TC-CORPUS-07: RepoAtomic on real 300-task manifest
      produces a valid whole-repo partition (concrete shape
      depends on SplitMix64 shuffle).
  </in_scope>
  <out_of_scope>
    - The split_fsv example binary (TASK-CORPUS-025).
    - generate_swe_bench_mutation_corpus (TASK-CORPUS-023).
    - Stoer-Wagner mincut wrapper for fairness check (deferred —
      the leakage assertion is the structural guard).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-corpus/src/split.rs">
    pub fn split_corpus(
        entries: &amp;[CorpusEntry],
        config: &amp;SplitConfig,
    ) -&gt; crate::MutationResult&lt;SplitReport&gt;;

    pub fn assert_no_test_patch_leakage(
        assignments: &amp;[TaskAssignment],
        test_patch_shas: &amp;std::collections::BTreeMap&lt;String, String&gt;,
    ) -&gt; crate::MutationResult&lt;()&gt;;
    </signature>
  </signatures>

  <constraints>
    - Fraction validation: `(t + c + h - 1.0).abs() &lt; 1e-9`.
      Tighter rejection breaks JSON round-trip (EC-CORPUS-04).
    - All fractions non-negative finite (`is_finite() &amp;&amp; *f &gt;= 0.0`).
    - Duplicate task_id detection via `BTreeSet&lt;&amp;str&gt;`.
    - bucket_sizes is `[train, cal, hold]` — canonical order
      enforced.
    - leakage_check_passed: `true` for InstanceAtomic always;
      `true` for RepoAtomic iff every repo's tasks landed in the
      same bucket (computed during split).
    - The stderr warning text MUST include the requested target
      ratios AND the achieved ratios for operator visibility.
    - assert_no_test_patch_leakage is O(n log n) — no O(n²) passes.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-corpus -- split_corpus`
      passes (≥ 8 tests including TC-CORPUS-06, 07).
    - `cargo test -p context-graph-mejepa-corpus -- assert_no_test_patch_leakage`
      passes (≥ 3 tests including TC-CORPUS-08).
    - `cargo build --release -p context-graph-mejepa-corpus` clean.
    - `cargo clippy -p context-graph-mejepa-corpus -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
pub fn split_corpus(entries: &amp;[CorpusEntry], config: &amp;SplitConfig) -&gt; MutationResult&lt;SplitReport&gt; {
    validate_config(config)?; // 1e-9 tolerance, non-negative, finite
    validate_entries(entries)?;
    let mut units = match config.mode {
        SplitMode::InstanceAtomic =&gt; entries.iter().map(|e| Unit::task(e)).collect::&lt;Vec&lt;_&gt;&gt;(),
        SplitMode::RepoAtomic =&gt; group_by_repo(entries),
    };
    crate::prng::shuffle_in_place(&amp;mut units, config.seed);
    units.sort_by(|a, b| b.size().cmp(&amp;a.size())); // LPT
    let targets = compute_targets(entries.len(), config);
    let mut counts = [0usize; 3];
    let mut assignments = Vec::with_capacity(entries.len());
    for unit in units {
        let bucket = pick_bucket(counts, targets);
        counts[bucket] += unit.size();
        for ti in unit.task_ids() { assignments.push(TaskAssignment { task_id: ti, repo: unit.repo, bucket: bucket_to_enum(bucket) }); }
    }
    let leakage_check_passed = matches!(config.mode, SplitMode::InstanceAtomic) || repo_atomic_invariant(&amp;assignments);
    Ok(SplitReport { ..., bucket_sizes: counts, ..., leakage_check_passed })
}

#[test]
fn split_fractions_tolerate_1e_minus_9() {
    let cfg = SplitConfig { train_fraction: 0.8 + 1e-10, calibration_fraction: 0.1, holdout_fraction: 0.1, seed: 0, mode: SplitMode::InstanceAtomic };
    let entries = vec![CorpusEntry { task_id: "a".into(), repo: "r".into() }];
    assert!(split_corpus(&amp;entries, &amp;cfg).is_ok());
}

#[test]
fn split_instance_atomic_300_tasks_produces_240_30_30() {
    let entries = load_swebench_lite_entries();
    let r = split_corpus(&amp;entries, &amp;SplitConfig::default()).unwrap();
    assert_eq!(r.bucket_sizes, [240, 30, 30]);
    assert!(r.leakage_check_passed);
}
</pseudo_code>

<files_to_create>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-corpus/src/split.rs">Verify split_corpus + assert_no_test_patch_leakage; add EC-CORPUS-04 / -05 regression tests.</file>
  <file path="crates/context-graph-mejepa-corpus/src/split_tests.rs">Add 8 unit tests + 3 leakage tests.</file>
</files_to_modify>

<validation_criteria>
  <criterion>InstanceAtomic on 300-task SWE-bench Lite manifest produces [240, 30, 30]</criterion>
  <criterion>RepoAtomic produces a valid whole-repo partition with leakage_check_passed = true</criterion>
  <criterion>Fraction validation tolerates 1e-9 deviation (EC-CORPUS-04)</criterion>
  <criterion>Determinism: split with same seed produces byte-equal report</criterion>
  <criterion>assert_no_test_patch_leakage detects fabricated leakage (TC-CORPUS-08)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-corpus</command>
  <command>cargo test -p context-graph-mejepa-corpus -- split_corpus</command>
  <command>cargo test -p context-graph-mejepa-corpus -- assert_no_test_patch_leakage</command>
  <command>cargo clippy -p context-graph-mejepa-corpus -- -D warnings</command>
</test_commands>
</task_spec>
```

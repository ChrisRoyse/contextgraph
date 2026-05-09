# TASK-CORPUS-028 — CLI stats Subcommand

```xml
<task_spec id="TASK-CORPUS-028" version="1.0">
<metadata>
  <title>mejepa-corpus stats Subcommand (read-only inspector)</title>
  <status>complete</status>
  <implementation_state as_of="2026-05-09">
    Shipped: `mejepa-corpus stats --corpus <path>` (bin/mejepa-corpus.rs) — opens the `OracleStore`, prints CF entry counts (`CF_MEJEPA_MUTATION_CORPUS`, `CF_MEJEPA_ORACLE_VERDICTS`), groups index entries by bucket (Train/Calibration/Holdout) and category (8 doc-09 §3.1 categories), and emits a `oracle_all_passed` summary per category. Read-only — never opens RocksDB write handles. Output asserted by FSV in the test suite.
  </implementation_state>
  <layer>surface</layer>
  <sequence>28</sequence>
  <implements>
    <requirement_ref>REQ-CORPUS-09</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORPUS-021</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
</metadata>

<context>
Surface task. The read-only corpus inspector. Iterates the
RocksDB CFs and produces an aggregate JSON report covering
counts by category, by oracle outcome, by bucket, and the
leakage_check status. Always exits 0 (no Docker re-run).
</context>

<input_context_files>
  <file purpose="api_contract">../../technical/TECH-CORPUS.md#api_contracts (mejepa-corpus stats)</file>
  <file purpose="story">../../functional/SPEC-CORPUS.md (US-CORPUS-05, AC-CORPUS-05-A)</file>
  <file purpose="existing">crates/context-graph-mejepa-corpus/src/bin/mejepa-corpus.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORPUS-021 complete (OracleStore + iter_corpus_rows exist)</check>
  <check>TASK-CORPUS-026 complete (CLI scaffold ready)</check>
</prerequisites>

<scope>
  <in_scope>
    - Implement `run_stats(args: StatsArgs) -&gt;
      CliResult&lt;()&gt;`:
      1. Parse `--corpus &lt;path&gt;`.
      2. `OracleStore::open(corpus.join("store"))?`.
      3. Read `provenance` via store.get_provenance.
      4. Read corpus-index.json (for bucket assignments).
      5. Iterate corpus rows; aggregate:
         - `total_entries: usize`
         - `by_category: BTreeMap&lt;String, usize&gt;` (8 keys)
         - `by_oracle: { resolved_true: N, resolved_false: N,
           exception_classes: BTreeMap&lt;String, usize&gt; }`
         - `by_bucket: { Train: N, Calibration: N, Holdout: N }`
         - `leakage_check: { passed: bool, mode: String }`
         - `provenance: { corpus_sha, embedder_version,
           frozen_at, code_version, language_set }`
      6. Print compact-or-pretty JSON to stdout.
      7. Exit 0 always.
    - Add tests:
      - Stubbed corpus with 16 entries (2 tasks × 8 categories)
        produces correct aggregation (TC-CORPUS-11 follow-up).
      - Empty corpus exits 0 with total_entries=0.
      - leakage_check field reflects the mode + passed status
        from corpus-index.json.
  </in_scope>
  <out_of_scope>
    - Docker re-run (TASK-CORPUS-027).
    - Per-row dump (out-of-scope of stats; future debug
      subcommand).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-corpus/src/bin/mejepa-corpus.rs">
    #[derive(clap::Args)]
    struct StatsArgs {
        #[arg(long)] corpus: std::path::PathBuf,
        #[arg(long)] pretty: bool,
    }

    fn run_stats(args: StatsArgs) -&gt; CliResult&lt;()&gt;;
    </signature>
  </signatures>

  <constraints>
    - Read-only: NEVER writes to RocksDB, the corpus dir, or
      `/tmp/contextgraph-mejepa-corpus-fsv/`.
    - JSON keys use snake_case, sorted (BTreeMap not HashMap)
      so output is reproducible.
    - `exception_classes` map keys are the lowercase slugs
      from `ExceptionClass` (e.g., "assertion_error",
      "type_error", "other").
    - Empty corpus is NOT an error — total_entries: 0 is valid.
    - Always exit 0 unless an actual error occurs (CF missing,
      DB open fails, etc.).
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-mejepa-corpus --bin mejepa-corpus` clean.
    - `cargo test -p context-graph-mejepa-corpus -- run_stats_aggregates` passes.
    - `cargo test -p context-graph-mejepa-corpus -- run_stats_empty_corpus` passes.
    - `cargo clippy -p context-graph-mejepa-corpus -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
fn run_stats(a: StatsArgs) -&gt; CliResult&lt;()&gt; {
    let store = OracleStore::open(&amp;a.corpus.join("store"))?;
    let provenance = store.get_provenance(1, EMBEDDER_VERSION_V1)?;
    let index_path = a.corpus.join("corpus-index.json");
    let index: Option&lt;CorpusIndex&gt; = std::fs::read_to_string(&amp;index_path).ok()
        .and_then(|s| serde_json::from_str(&amp;s).ok());
    let mut by_category = std::collections::BTreeMap::new();
    let mut by_oracle_resolved_true = 0usize;
    let mut by_oracle_resolved_false = 0usize;
    let mut by_oracle_exceptions: std::collections::BTreeMap&lt;String, usize&gt; = Default::default();
    let mut total = 0usize;
    for r in store.iter_corpus_rows() {
        let (_tid, cat, _outcome, verdict) = r?;
        total += 1;
        *by_category.entry(cat.slug().to_string()).or_default() += 1;
        if verdict.resolved { by_oracle_resolved_true += 1 } else { by_oracle_resolved_false += 1 }
        if let Some(ex) = &amp;verdict.exception {
            *by_oracle_exceptions.entry(ex.slug().to_string()).or_default() += 1;
        }
    }
    let by_bucket = index.as_ref().map(|i| compute_bucket_counts(&amp;i.split_assignments)).unwrap_or_default();
    let leakage_check = serde_json::json!({ "passed": ..., "mode": ... });
    let report = serde_json::json!({
        "total_entries": total,
        "by_category": by_category,
        "by_oracle": { "resolved_true": by_oracle_resolved_true,
                       "resolved_false": by_oracle_resolved_false,
                       "exception_classes": by_oracle_exceptions },
        "by_bucket": by_bucket,
        "leakage_check": leakage_check,
        "provenance": provenance,
    });
    let out = if a.pretty { serde_json::to_string_pretty(&amp;report)? } else { report.to_string() };
    println!("{out}");
    Ok(())
}

#[test]
fn run_stats_aggregates_16_entries() {
    let store = build_temp_store_with_2_tasks_x_8_categories();
    let args = StatsArgs { corpus: store_dir, pretty: false };
    let captured = run_stats_capture_stdout(args).unwrap();
    let v: serde_json::Value = serde_json::from_str(&amp;captured).unwrap();
    assert_eq!(v["total_entries"], 16);
    assert_eq!(v["by_category"]["known_good"], 2);
}
</pseudo_code>

<files_to_create>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-corpus/src/bin/mejepa-corpus.rs">Add StatsArgs + run_stats.</file>
  <file path="crates/context-graph-mejepa-corpus/src/tests.rs">Add 3 stats tests.</file>
</files_to_modify>

<validation_criteria>
  <criterion>Aggregate counts correct for stubbed 16-entry corpus</criterion>
  <criterion>Empty corpus prints JSON with total_entries: 0 and exits 0</criterion>
  <criterion>by_category has 8 keys (one per MutationCategory slug)</criterion>
  <criterion>by_bucket reflects corpus-index.json split assignments</criterion>
  <criterion>provenance block surfaced when present, null when absent</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-corpus --bin mejepa-corpus</command>
  <command>cargo test -p context-graph-mejepa-corpus -- run_stats</command>
  <command>cargo clippy -p context-graph-mejepa-corpus -- -D warnings</command>
</test_commands>
</task_spec>
```

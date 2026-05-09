# TASK-CORPUS-027 — CLI verify Subcommand + --sample-fraction

```xml
<task_spec id="TASK-CORPUS-027" version="1.0">
<metadata>
  <title>mejepa-corpus verify Subcommand (5% Docker re-run + JSON diff)</title>
  <status>partial</status>
  <implementation_state as_of="2026-05-09">
    Shipped: `mejepa-corpus verify --corpus <path>` (bin/mejepa-corpus.rs:303+) — re-reads `index.json`, validates the `complete: true` flag, opens the `OracleStore`, and re-hashes every `patch_path` (under the corpus dir) against `entry.patch_sha256`, reporting any mismatch as a JSON failure record. Duplicate-key detection over (`task_id`, `category`) pairs included.
    Pending: a `--sample-fraction <0.0..=1.0>` flag that re-runs `run_swebench_lite_oracle` against a deterministic 5% sample of stored entries and produces a per-test JSON diff vs. the stored verdict — the spec deliverable for this task. Without it, `verify` is a SHA cross-check only and cannot detect verdict drift caused by upstream Docker image changes.
  </implementation_state>
  <layer>surface</layer>
  <sequence>27</sequence>
  <implements>
    <requirement_ref>REQ-CORPUS-15</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORPUS-020</task_ref>
    <task_ref>TASK-CORPUS-021</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
</metadata>

<context>
Surface task. Re-runs the Docker oracle on a deterministic
sample of corpus rows and compares the new verdict to the
persisted verdict on the four invariant fields per
`<determinism_contract>`. Sample seed = first 8 bytes of
corpus_sha (per CorpusProvenance). Per US-CORPUS-04: catches
silent corruption (RocksDB, Docker image drift, hand-edited
JSON) before Phase 3 training depends on bad labels.
</context>

<input_context_files>
  <file purpose="api_contract">../../technical/TECH-CORPUS.md#api_contracts (mejepa-corpus verify)</file>
  <file purpose="story">../../functional/SPEC-CORPUS.md (US-CORPUS-04, AC-CORPUS-04-A, EC-CORPUS-10)</file>
  <file purpose="determinism">../../technical/TECH-CORPUS.md#determinism_contract</file>
  <file purpose="existing">crates/context-graph-mejepa-corpus/src/bin/mejepa-corpus.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORPUS-020 complete (run_swebench_lite_oracle exists)</check>
  <check>TASK-CORPUS-021 complete (OracleStore exists)</check>
  <check>TASK-CORPUS-022 complete (verify_provenance_consistent exists)</check>
  <check>TASK-CORPUS-026 complete (CLI scaffold ready)</check>
</prerequisites>

<scope>
  <in_scope>
    - Implement `run_verify(args: VerifyArgs) -&gt;
      CliResult&lt;()&gt;`:
      1. Parse `--corpus &lt;path&gt;` and `--sample-fraction
         &lt;f64&gt;` (default 0.05; min 0.0001; max 1.0).
      2. `OracleStore::open(corpus.join("store"))?`.
      3. Read CorpusProvenance via
         `store.get_provenance(1, embedder_v)?`. If missing →
         `MEJEPA_CORPUS_INVALID_INPUT { field: "corpus", message:
         "corpus is incomplete (no provenance row)" }`.
      4. Compute deterministic sample seed: first 8 bytes of
         `provenance.corpus_sha` interpreted as `u64` LE.
      5. Iterate all rows; sample fraction × N rounds up; min
         1 per category (per AC-CORPUS-04-A).
      6. For each sampled row:
         a. Re-run `run_swebench_lite_oracle(...)`.
         b. Compare four invariant fields:
            (resolved, FAIL_TO_PASS.success.len(),
            PASS_TO_PASS.success.len(), exception_class).
         c. Collect mismatches with the per-row diff.
      7. Print JSON to stdout: `{ "sampled_pairs": N,
         "mismatches": [...] }` (mismatches is empty on
         success).
      8. Exit 0 iff zero mismatches; else non-zero.
    - Implement `--sample-fraction` CLAP arg with validation
      (range 0.0001..=1.0).
    - Implement `sample_corpus_rows(rows:
      &amp;[(String, MutationCategory)], fraction: f64,
      sample_seed: u64) -&gt; Vec&lt;(String,
      MutationCategory)&gt;` — deterministic sampler that:
      1. Uses SplitMix64 seeded with sample_seed to shuffle.
      2. Takes the first `ceil(fraction * N)` rows.
      3. Guarantees at least 1 row per category present in the
         input (post-hoc top-up if the random draw missed a
         category).
    - Add tests:
      - sample_corpus_rows is deterministic for fixed
        sample_seed.
      - sample_corpus_rows guarantees ≥ 1 per category.
      - Stubbed oracle returning matching verdicts → exit 0.
      - Stubbed oracle returning mismatched verdict on one row →
        exit non-zero with that row's diff (TC-CORPUS-13).
  </in_scope>
  <out_of_scope>
    - The `stats` subcommand (TASK-CORPUS-028).
    - Concurrency in re-run (sequential per
      `<concurrency_model>`).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-corpus/src/bin/mejepa-corpus.rs">
    #[derive(clap::Args)]
    struct VerifyArgs {
        #[arg(long)] corpus: std::path::PathBuf,
        #[arg(long, default_value_t = 0.05)] sample_fraction: f64,
    }

    fn run_verify(args: VerifyArgs) -&gt; CliResult&lt;()&gt;;

    fn sample_corpus_rows(
        rows: &amp;[(String, MutationCategory)],
        fraction: f64,
        sample_seed: u64,
    ) -&gt; Vec&lt;(String, MutationCategory)&gt;;
    </signature>
  </signatures>

  <constraints>
    - The 4 invariant fields in the diff are EXACTLY:
      - `resolved`
      - `fail_to_pass_success_count`
      - `pass_to_pass_success_count`
      - `exception_class` (Option&lt;ExceptionClass&gt;
        slug-stringified)
    - Other fields (per_test ordering, runtime_seconds) are
      NOT compared (the determinism contract excludes them).
    - Sampling uses SplitMix64 (TASK-CORPUS-011); no other PRNG.
    - On mismatch, the JSON diff includes both old and new
      values for human inspection.
    - The verifier MUST refuse to run if the corpus has
      `provenance.entry_count` 0 (`complete: false` corpus).
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-mejepa-corpus --bin mejepa-corpus` clean.
    - `cargo test -p context-graph-mejepa-corpus -- run_verify_no_mismatch` passes.
    - `cargo test -p context-graph-mejepa-corpus -- run_verify_one_mismatch_exits_nonzero` passes (TC-CORPUS-13).
    - `cargo test -p context-graph-mejepa-corpus -- sample_corpus_rows_deterministic` passes.
    - `cargo test -p context-graph-mejepa-corpus -- sample_corpus_rows_min_one_per_category` passes.
    - `cargo clippy -p context-graph-mejepa-corpus -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
fn run_verify(a: VerifyArgs) -&gt; CliResult&lt;()&gt; {
    let store = OracleStore::open(&amp;a.corpus.join("store"))?;
    let prov = store.get_provenance(1, EMBEDDER_VERSION_V1)?
        .ok_or_else(|| CliError::corpus_invalid("no provenance row"))?;
    let sample_seed = u64::from_le_bytes(decode_first_8_hex_bytes(&amp;prov.corpus_sha));
    let all_rows = store.iter_corpus_rows().collect::&lt;Result&lt;Vec&lt;_&gt;, _&gt;&gt;()?;
    let pairs: Vec&lt;_&gt; = all_rows.iter().map(|(t, c, _, _)| (t.clone(), *c)).collect();
    let sampled = sample_corpus_rows(&amp;pairs, a.sample_fraction, sample_seed);
    let mut mismatches = vec![];
    for (tid, cat) in &amp;sampled {
        let persisted = store.get_corpus_row(tid, *cat)?.unwrap().1;
        let cfg = build_swebench_oracle_config(...);
        let fresh = run_swebench_lite_oracle(&amp;cfg)?.verdict;
        if !invariant_fields_match(&amp;persisted, &amp;fresh) {
            mismatches.push(diff_json(tid, cat, &amp;persisted, &amp;fresh));
        }
    }
    let out = serde_json::json!({ "sampled_pairs": sampled.len(), "mismatches": mismatches });
    println!("{}", out);
    if mismatches.is_empty() { Ok(()) } else { Err(CliError::VerifyMismatch) }
}

fn sample_corpus_rows(rows: &amp;[(String, MutationCategory)], fraction: f64, seed: u64) -&gt; Vec&lt;(String, MutationCategory)&gt; {
    let n = ((fraction * rows.len() as f64).ceil() as usize).max(1);
    let mut shuffled = rows.to_vec();
    crate::prng::shuffle_in_place(&amp;mut shuffled, seed);
    let mut picked: Vec&lt;_&gt; = shuffled.into_iter().take(n).collect();
    // top up: ensure every category present in `rows` is present in `picked`
    let cats_in_picked: std::collections::HashSet&lt;_&gt; = picked.iter().map(|p| p.1).collect();
    for &amp;c in MutationCategory::all() {
        if !cats_in_picked.contains(&amp;c) {
            if let Some(r) = rows.iter().find(|p| p.1 == c).cloned() { picked.push(r); }
        }
    }
    picked
}
</pseudo_code>

<files_to_create>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-corpus/src/bin/mejepa-corpus.rs">Add VerifyArgs + run_verify + sample_corpus_rows.</file>
  <file path="crates/context-graph-mejepa-corpus/src/tests.rs">Add 4 verify tests.</file>
</files_to_modify>

<validation_criteria>
  <criterion>Sampling is deterministic from corpus_sha-derived seed</criterion>
  <criterion>At least 1 row per category in the sample (AC-CORPUS-04-A)</criterion>
  <criterion>Verifier exits 0 on no mismatches</criterion>
  <criterion>Verifier exits non-zero with diff JSON on any mismatch (TC-CORPUS-13)</criterion>
  <criterion>Refuses to run on corpus without provenance row (complete:false)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-corpus --bin mejepa-corpus</command>
  <command>cargo test -p context-graph-mejepa-corpus -- run_verify_no_mismatch</command>
  <command>cargo test -p context-graph-mejepa-corpus -- run_verify_one_mismatch_exits_nonzero</command>
  <command>cargo test -p context-graph-mejepa-corpus -- sample_corpus_rows</command>
  <command>cargo clippy -p context-graph-mejepa-corpus -- -D warnings</command>
</test_commands>
</task_spec>
```

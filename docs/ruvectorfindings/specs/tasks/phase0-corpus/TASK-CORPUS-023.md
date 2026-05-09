# TASK-CORPUS-023 — generate_swe_bench_mutation_corpus Library Entry Point

```xml
<task_spec id="TASK-CORPUS-023" version="1.0">
<metadata>
  <title>generate_swe_bench_mutation_corpus Library Entry Point</title>
  <status>partial</status>
  <implementation_state as_of="2026-05-09">
    The end-to-end orchestration is shipped, BUT it lives inline in `bin/mejepa-corpus.rs::generate` (lines 139-301), not as a reusable library entry. That CLI function: loads task manifests from `tasks/swebench-lite/`, loads official patches via the venv Python, runs `split_corpus` in InstanceAtomic mode, builds source-backed mutated patches via `mutate_task_source_patch` (commits b6b4810c / 3f46ff78), opens the `OracleStore`, and for each (task × category) pair writes the mutation, runs `run_swebench_lite_oracle`, writes the verdict, and updates the on-disk `index.json` (with `complete: false` until the last entry is written, then `complete: true`).
    Pending: extract this into a public `generate_swe_bench_mutation_corpus(config) -> CorpusResult<CorpusReport>` function in `lib.rs` so non-CLI callers (Phase 3 training loop, Phase 8 evaluation harness) can drive it programmatically. Also pending: integrating the future `CorpusProvenance` write path (TASK-CORPUS-022) once 007/008 land.
  </implementation_state>
  <layer>logic</layer>
  <sequence>23</sequence>
  <implements>
    <requirement_ref>REQ-CORPUS-01</requirement_ref>
    <requirement_ref>REQ-CORPUS-12</requirement_ref>
    <requirement_ref>REQ-CORPUS-13</requirement_ref>
    <requirement_ref>REQ-CORPUS-16</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORPUS-016</task_ref>
    <task_ref>TASK-CORPUS-017</task_ref>
    <task_ref>TASK-CORPUS-020</task_ref>
    <task_ref>TASK-CORPUS-021</task_ref>
    <task_ref>TASK-CORPUS-022</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
</metadata>

<context>
Logic task. The pure-library batch orchestrator. Loads task
manifests, computes the split, iterates (task × category),
applies mutations, runs the Docker oracle, persists rows,
optionally finalizes provenance, writes corpus-index.json.
Surfaces fail-closed behavior per REQ-CORPUS-16 (`complete:
false` AND non-zero exit on any oracle failure).
</context>

<input_context_files>
  <file purpose="api_contract">../../technical/TECH-CORPUS.md#api_contracts (generate_swe_bench_mutation_corpus)</file>
  <file purpose="story">../../functional/SPEC-CORPUS.md (US-CORPUS-01, AC-CORPUS-01-A/B, BR-CORPUS-01)</file>
  <file purpose="business_rules">../../functional/SPEC-CORPUS.md (BR-CORPUS-01, BR-CORPUS-04, BR-CORPUS-06)</file>
  <file purpose="task_manifests">tasks/swebench-lite/*.json (read-only — corpus task list)</file>
  <file purpose="existing">crates/context-graph-mejepa-corpus/src/lib.rs</file>
</input_context_files>

<prerequisites>
  <check>All 8 mutation operators ship (TASK-CORPUS-012..016)</check>
  <check>split_corpus + assert_no_test_patch_leakage ship (TASK-CORPUS-017)</check>
  <check>run_swebench_lite_oracle ships (TASK-CORPUS-020)</check>
  <check>OracleStore ships (TASK-CORPUS-021)</check>
  <check>build_corpus_provenance ships (TASK-CORPUS-022)</check>
</prerequisites>

<scope>
  <in_scope>
    - Implement `pub fn generate_swe_bench_mutation_corpus(
      swe_bench_lite_root: &amp;Path, output_root: &amp;Path,
      categories: &amp;[MutationCategory], languages:
      &amp;[Language]) -&gt;
      Result&lt;CorpusReport, MejepaCorpusError&gt;` per
      TECH-CORPUS API contract.
    - Implement supporting types:
      - `pub struct CorpusReport { pub complete: bool, pub
        entry_count: usize, pub split: SplitReport, pub
        provenance: Option&lt;CorpusProvenance&gt;, pub
        index_path: PathBuf }`.
      - `pub enum MejepaCorpusError { Mutation(MutationError),
        Oracle(OracleError), Io(std::io::Error),
        UnsupportedLanguage(Language) }` with `.code()`
        returning the appropriate SCREAMING_SNAKE tag.
    - Steps:
      1. BR-CORPUS-01: every language must `is_supported()`;
         else `MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE`.
      2. Default `categories` to `MutationCategory::all()` if
         caller passes empty (or refuse — the function signature
         takes a slice; document either choice).
      3. `load_task_manifests(swe_bench_lite_root)?` — reads
         every `tasks/swebench-lite/*.json` into
         `Vec&lt;CorpusEntry&gt;` plus a parallel
         `BTreeMap&lt;task_id, test_patch_sha256&gt;` for the
         leakage check.
      4. `split_corpus(&amp;entries, &amp;SplitConfig::default())?` →
         SplitReport.
      5. `assert_no_test_patch_leakage(&amp;split.assignments,
         &amp;test_patch_shas)?`.
      6. `OracleStore::open(output_root.join("store"))?`.
      7. For each (task, category) cross-product (sequential per
         `<concurrency_model>` rule):
         a. Read primary_source from manifest.
         b. `apply_mutation(category, &amp;primary_source,
            mut_config)?` — fail-closed, no silent skip.
         c. `run_swebench_lite_oracle(&amp;config_for_task)?`
            — fail-closed.
         d. `store.put_corpus_row(...)`.
         e. Append GeneratedEntry to in-memory index.
         f. SHA-pin patch_path under
            `output_root/patches/<task_id>/<category>.diff`
            (file mode 0o600 per SEC-MEJEPA-07).
      8. On any per-(task, category) failure (REQ-CORPUS-16):
         persist `corpus-index.json` with `complete: false`;
         return `Err`.
      9. On full success: `finalize_provenance(...)`; persist
         `corpus-index.json` with `complete: true`.
    - Add unit tests:
      - BR-CORPUS-01: passing `&[Language::Rust]` returns
        UnsupportedLanguage.
      - BR-CORPUS-04 stub: with synthetic `--max-tasks 2` and a
        mocked oracle, full path persists corpus-index.json with
        complete: true and provenance row.
      - REQ-CORPUS-16: with one stubbed oracle failure, complete
        is false AND function returns Err.
    - The function MUST be testable WITHOUT real Docker by
      injecting an oracle trait
      (`pub trait Oracle { fn run(&amp;self, config:
      &amp;SwebenchOracleConfig) -&gt;
      OracleResult&lt;SwebenchOracleResult&gt;; }`) so unit
      tests stub it. The default impl wraps
      `run_swebench_lite_oracle`.
  </in_scope>
  <out_of_scope>
    - The CLI parser (TASK-CORPUS-026).
    - SIGINT handler (TASK-CORPUS-026).
    - The 5%-sample verifier (TASK-CORPUS-027).
    - The stats inspector (TASK-CORPUS-028).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-corpus/src/lib.rs">
    pub fn generate_swe_bench_mutation_corpus(
        swe_bench_lite_root: &amp;std::path::Path,
        output_root: &amp;std::path::Path,
        categories: &amp;[MutationCategory],
        languages: &amp;[Language],
    ) -&gt; Result&lt;CorpusReport, MejepaCorpusError&gt;;

    pub fn generate_swe_bench_mutation_corpus_with_oracle&lt;O: Oracle&gt;(
        swe_bench_lite_root: &amp;std::path::Path,
        output_root: &amp;std::path::Path,
        categories: &amp;[MutationCategory],
        languages: &amp;[Language],
        oracle: &amp;O,
    ) -&gt; Result&lt;CorpusReport, MejepaCorpusError&gt;;

    pub trait Oracle {
        fn run(&amp;self, config: &amp;crate::swebench::SwebenchOracleConfig)
            -&gt; crate::swebench::OracleResult&lt;crate::swebench::SwebenchOracleResult&gt;;
    }

    pub struct CorpusReport {
        pub complete: bool,
        pub entry_count: usize,
        pub split: crate::split::SplitReport,
        pub provenance: Option&lt;CorpusProvenance&gt;,
        pub index_path: std::path::PathBuf,
    }

    #[derive(Debug, thiserror::Error)]
    pub enum MejepaCorpusError {
        Mutation(MutationError),
        Oracle(crate::swebench::OracleError),
        Io { context: String, source: std::io::Error },
        UnsupportedLanguage(Language),
    }

    impl MejepaCorpusError { pub fn code(&amp;self) -&gt; &amp;'static str; }
    </signature>
  </signatures>

  <constraints>
    - Sequential execution (per `<concurrency_model>`) — Docker is
      the bottleneck; parallelism is deferred (Q-CORPUS-01).
    - Patch file mode 0o600 on Unix; non-Unix unsupported.
    - Source-of-truth file paths in error messages start with
      `file:`.
    - The Oracle trait + `_with_oracle` overload is the testability
      seam — the public `generate_swe_bench_mutation_corpus`
      delegates to it with a real-Docker default impl.
    - On error: corpus-index.json is STILL written with
      complete: false so downstream tooling can read partial
      progress.
    - Total file length stays under 500 lines; split into
      `generate.rs` if needed.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-corpus -- generate_unsupported_language` passes.
    - `cargo test -p context-graph-mejepa-corpus -- generate_with_stubbed_oracle_complete_true` passes.
    - `cargo test -p context-graph-mejepa-corpus -- generate_with_stubbed_oracle_one_failure_complete_false` passes.
    - `cargo build --release -p context-graph-mejepa-corpus` clean.
    - `cargo clippy -p context-graph-mejepa-corpus -- -D warnings` clean.
  </verification>
</definition_of_done>

<pseudo_code>
pub fn generate_swe_bench_mutation_corpus_with_oracle&lt;O: Oracle&gt;(...) -&gt; Result&lt;CorpusReport, MejepaCorpusError&gt; {
    for &amp;l in languages { if !l.is_supported() { return Err(MejepaCorpusError::UnsupportedLanguage(l)); } }
    let entries_with_shas = load_task_manifests(swe_bench_lite_root)?; // (Vec&lt;CorpusEntry&gt;, BTreeMap&lt;task_id, sha&gt;)
    let split = split_corpus(&amp;entries_with_shas.0, &amp;SplitConfig::default()).map_err(MejepaCorpusError::Mutation)?;
    assert_no_test_patch_leakage(&amp;split.assignments, &amp;entries_with_shas.1).map_err(MejepaCorpusError::Mutation)?;
    let store = OracleStore::open(&amp;output_root.join("store")).map_err(MejepaCorpusError::Oracle)?;
    let mut index_entries = vec![];
    let mut complete = true;
    for entry in &amp;entries_with_shas.0 {
        for &amp;cat in categories {
            match step(entry, cat, &amp;store, oracle) {
                Ok(g) =&gt; index_entries.push(g),
                Err(e) =&gt; { complete = false; persist_index(...complete=false...)?; return Err(e); }
            }
        }
    }
    let provenance = if complete {
        Some(finalize_provenance(&amp;store, EMBEDDER_VERSION_V1, &amp;current_git_sha(), languages)?)
    } else { None };
    persist_index(...complete=true...)?;
    Ok(CorpusReport { complete, entry_count: index_entries.len(), split, provenance, index_path })
}

#[test]
fn generate_unsupported_language_returns_error() {
    let r = generate_swe_bench_mutation_corpus(...&amp;[Language::Rust]).unwrap_err();
    assert_eq!(r.code(), "MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE");
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-corpus/src/generate.rs">Module containing the orchestrator (if lib.rs grows past 500 lines).</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-corpus/src/lib.rs">Add Oracle trait + types + entry-point delegating to _with_oracle.</file>
  <file path="crates/context-graph-mejepa-corpus/src/tests.rs">Add 3 orchestrator tests.</file>
</files_to_modify>

<validation_criteria>
  <criterion>BR-CORPUS-01 enforced (unsupported language fails closed)</criterion>
  <criterion>complete=true path writes provenance row + index</criterion>
  <criterion>complete=false path on any oracle error writes index with complete=false AND returns Err</criterion>
  <criterion>Oracle trait enables stubbed unit tests without Docker</criterion>
  <criterion>Patch files written with mode 0o600</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-corpus</command>
  <command>cargo test -p context-graph-mejepa-corpus -- generate_unsupported_language</command>
  <command>cargo test -p context-graph-mejepa-corpus -- generate_with_stubbed_oracle</command>
  <command>cargo clippy -p context-graph-mejepa-corpus -- -D warnings</command>
</test_commands>
</task_spec>
```

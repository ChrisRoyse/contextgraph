# TASK-CORPUS-024 — mejepa_corpus_fsv Example Binary

```xml
<task_spec id="TASK-CORPUS-024" version="1.0">
<metadata>
  <title>mejepa_corpus_fsv Example Binary (FSV harness with py_compile)</title>
  <status>complete</status>
  <implementation_state as_of="2026-05-09">
    Shipped: `examples/mejepa_corpus_fsv.rs` (53-LOC main + helpers) — invokes the operator suite through `apply_mutation`, writes evidence JSON to `/mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/`, and calls `python -m py_compile` on each mutated source per the FSV protocol introduced in commit `b6b4810c`. Sister harnesses also shipped: `oracle_fsv.rs`, `oracle_store_fsv.rs`, `swebench_oracle_fsv.rs` — five total under `examples/`.
  </implementation_state>
  <layer>logic</layer>
  <sequence>24</sequence>
  <implements>
    <requirement_ref>REQ-CORPUS-10</requirement_ref>
    <requirement_ref>NFR-CORPUS-06</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORPUS-023</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
</metadata>

<context>
Logic task. The forensic source verification harness for the
corpus generator. Builds a small in-memory fixture (1–3 real
Python sources from `tasks/swebench-lite/*.json`), applies all
8 mutation categories, persists each artifact under
`/mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/sources/` with mode 0o600,
runs `python3 -m py_compile` against every emitted Python
artifact (real toolchain — NO mock), and writes evidence JSON
that the operator can re-read. Already shipped per project
memory; this task verifies the shipped state and re-confirms
SEC-MEJEPA-07 mode-0o600 compliance.
</context>

<input_context_files>
  <file purpose="component_contract">../../technical/TECH-CORPUS.md#component_contracts (MejepaCorpusFsv)</file>
  <file purpose="fsv_protocol">../../constitution.md#coding_standards (fsv_protocol)</file>
  <file purpose="evidence_paths">../../constitution.md#testing_requirements (fsv_evidence_paths)</file>
  <file purpose="security_rule">../../constitution.md#security_requirements (SEC-MEJEPA-07)</file>
  <file purpose="existing">crates/context-graph-mejepa-corpus/examples/mejepa_corpus_fsv.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORPUS-023 complete (apply_mutation accessible from examples)</check>
  <check>python3 with py_compile module available in PATH (real toolchain)</check>
</prerequisites>

<scope>
  <in_scope>
    - Verify shipped `examples/mejepa_corpus_fsv.rs` `fn main()`:
      1. Build in-memory fixture: 1-3 real Python sources from
         `tasks/swebench-lite/{first,middle,last}.json`'s gold
         patch.
      2. For each (source, category) combination: call
         `apply_mutation(category, &amp;source,
         MutationConfig::default())` (with alternate_source for
         WrongFile).
      3. Persist each `mutated_source` under
         `/mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/sources/{task_id}-{category}.py`
         using `OpenOptions::new().mode(0o600)` (Unix only).
      4. Run `python3 -m py_compile <path>` against the file —
         capture exit code AND check for `.pyc` presence in the
         `__pycache__` directory.
      5. Append a row to
         `/mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/mejepa-corpus-evidence.json`
         containing: { task_id, category, mutated_source_sha256,
         py_compile_passed, expected_outcome }.
      6. Re-read the evidence JSON; for each row, recompute
         `sha256(file)` and assert equal to
         `mutated_source_sha256`.
      7. Assert expected_outcome agrees with observed:
         - KnownGood, OverEngineer, SwapVariable, SubtleFlip,
           OffByOne, DeleteTestCall, WrongFile → py_compile_passed
           = true (these MAY produce semantically wrong but
           syntactically valid Python).
         - CompileError → py_compile_passed = false.
      8. Print `FSV PASSED: N rows` on success; non-zero exit on
         assertion failure.
    - Add a non-Unix early-exit branch with a clear stderr
      message: "FSV requires Unix mode-0o600 file APIs;
      skipping on this platform" + exit 0 (so CI on macOS / win
      isn't a hard fail).
  </in_scope>
  <out_of_scope>
    - The split FSV harness (TASK-CORPUS-025).
    - The CLI generate path (TASK-CORPUS-026).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-corpus/examples/mejepa_corpus_fsv.rs">
    fn main() -&gt; std::process::ExitCode;
    </signature>
  </signatures>

  <constraints>
    - Real py_compile invocation — no mock, no stub.
    - File mode 0o600 enforced on every artifact AND every
      evidence file (NFR-CORPUS-06 / SEC-MEJEPA-07).
    - Evidence path canon:
      `/mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/mejepa-corpus-evidence.json`.
    - Sources path canon:
      `/mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/sources/`.
    - The harness creates the directory tree on first run; if
      it exists and contains stale evidence, OVERWRITES (not
      append) so consecutive runs don't bloat.
    - SHA-256 via `sha2::Sha256` (workspace dep).
    - The 1-3 Python source fixtures MUST come from the real
      SWE-bench Lite manifest — no synthetic fixtures.
  </constraints>

  <verification>
    - `cargo run --release -p context-graph-mejepa-corpus --example mejepa_corpus_fsv`
      exits 0.
    - `stat -c %a /mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/mejepa-corpus-evidence.json`
      returns `600`.
    - Evidence file has at least 8 rows (1 fixture × 8 categories) and
      sha256-matches every persisted source.
    - CompileError row has `py_compile_passed: false`.
    - All other category rows have `py_compile_passed: true`.
  </verification>
</definition_of_done>

<pseudo_code>
fn main() -&gt; ExitCode {
    if !cfg!(unix) { eprintln!("FSV requires Unix; skipping"); return ExitCode::SUCCESS; }
    let evidence_path = "/mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/mejepa-corpus-evidence.json";
    fs::create_dir_all("/mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/sources/")?;
    let fixtures = load_three_real_swebench_lite_python_sources();
    let mut rows = vec![];
    for (task_id, source) in fixtures {
        for cat in MutationCategory::all() {
            let cfg = if matches!(cat, MutationCategory::WrongFile) {
                MutationConfig { seed: 0, alternate_source: Some(load_alternate_python()) }
            } else { MutationConfig::default() };
            let outcome = apply_mutation(*cat, &amp;source, cfg).unwrap();
            let path = format!("/mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/sources/{task_id}-{}.py", cat.slug());
            write_with_mode_0o600(&amp;path, outcome.mutated_source.as_bytes())?;
            let exit = Command::new("python3").args(&amp;["-m", "py_compile", &amp;path]).status()?;
            let py_compile_passed = exit.success();
            let expected_passed = !matches!(cat, MutationCategory::CompileError);
            assert_eq!(py_compile_passed, expected_passed,
                "category {:?} produced unexpected py_compile result", cat);
            rows.push(serde_json::json!({
                "task_id": task_id, "category": cat.slug(),
                "mutated_source_sha256": sha256_hex(&amp;outcome.mutated_source),
                "py_compile_passed": py_compile_passed,
                "expected_outcome": if expected_passed { "py_compile_pass" } else { "py_compile_fail" },
            }));
        }
    }
    write_with_mode_0o600(evidence_path, serde_json::to_string_pretty(&amp;rows)?.as_bytes())?;
    // Re-read and re-verify
    let back: Vec&lt;serde_json::Value&gt; = serde_json::from_str(&amp;fs::read_to_string(evidence_path)?)?;
    for row in back { assert_sha_matches_disk(...); }
    println!("FSV PASSED: {} rows", rows.len());
    ExitCode::SUCCESS
}
</pseudo_code>

<files_to_create>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-corpus/examples/mejepa_corpus_fsv.rs">Verify shipped harness; add mode-0o600 + py_compile assertions.</file>
</files_to_modify>

<validation_criteria>
  <criterion>FSV exits 0 with at least 8 rows of evidence</criterion>
  <criterion>Every persisted file is mode 0o600 (NFR-CORPUS-06)</criterion>
  <criterion>CompileError row has py_compile_passed=false</criterion>
  <criterion>All other category rows have py_compile_passed=true</criterion>
  <criterion>Evidence JSON sha256 matches on-disk file content for every row</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-corpus --example mejepa_corpus_fsv</command>
  <command>cargo run --release -p context-graph-mejepa-corpus --example mejepa_corpus_fsv</command>
  <command>stat -c %a /mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/mejepa-corpus-evidence.json</command>
</test_commands>
</task_spec>
```

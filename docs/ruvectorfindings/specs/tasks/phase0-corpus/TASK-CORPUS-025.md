# TASK-CORPUS-025 — split_fsv Example Binary

```xml
<task_spec id="TASK-CORPUS-025" version="1.0">
<metadata>
  <title>split_fsv Example Binary (Split Helpers FSV Harness)</title>
  <status>complete</status>
  <implementation_state as_of="2026-05-09">
    Shipped: `examples/split_fsv.rs` — calls `split_corpus` and `assert_no_test_patch_leakage`, writes `split-evidence.json` and `split-assignments.json` to `/mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/`. Memory-recorded results (project_phase0_mejepa_mutation_corpus.md, 2026-05-09b): 25 repos × 300 tasks → 76.3/13.3/10.3 cross-cut weights all 0; InstanceAtomic mode on Lite achieves clean [240, 30, 30].
  </implementation_state>
  <layer>logic</layer>
  <sequence>25</sequence>
  <implements>
    <requirement_ref>REQ-CORPUS-10</requirement_ref>
    <requirement_ref>NFR-CORPUS-04</requirement_ref>
    <requirement_ref>NFR-CORPUS-06</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORPUS-017</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
</metadata>

<context>
Logic task. The forensic source verification harness for the
split engine. Loads the real `tasks/swebench-lite/*.json`
manifest, runs `split_corpus` in BOTH InstanceAtomic and
RepoAtomic modes, writes evidence files, re-reads them, and
asserts byte-equality. Already shipped; this task verifies the
shipped state.
</context>

<input_context_files>
  <file purpose="component_contract">../../technical/TECH-CORPUS.md#component_contracts (SplitFsv)</file>
  <file purpose="fsv_protocol">../../constitution.md#coding_standards (fsv_protocol)</file>
  <file purpose="evidence_paths">../../constitution.md#testing_requirements (fsv_evidence_paths)</file>
  <file purpose="determinism_rule">../../functional/SPEC-CORPUS.md (NFR-CORPUS-04)</file>
  <file purpose="existing">crates/context-graph-mejepa-corpus/examples/split_fsv.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORPUS-017 complete (split_corpus + assert_no_test_patch_leakage exist)</check>
  <check>tasks/swebench-lite/*.json manifest exists with 300 entries</check>
</prerequisites>

<scope>
  <in_scope>
    - Verify shipped `examples/split_fsv.rs` `fn main()`:
      1. Load entries from `tasks/swebench-lite/*.json` into
         Vec&lt;CorpusEntry&gt;.
      2. Run `split_corpus(&amp;entries, &amp;SplitConfig {
         mode: SplitMode::InstanceAtomic, ..Default::default()
         })`.
      3. Run `split_corpus(&amp;entries, &amp;SplitConfig {
         mode: SplitMode::RepoAtomic, ..Default::default() })`.
      4. Write
         `/mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/split-evidence.json`
         with both reports' bucket_sizes, bucket_fractions,
         leakage_check_passed, repo_assignments.
      5. Write
         `/mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/split-assignments.json`
         with the per-task TaskAssignment list for
         InstanceAtomic mode.
      6. Re-read both files; assert byte-equality with what was
         written.
      7. Verify file mode is 0o600 via `Metadata::permissions().mode()`.
      8. Run a determinism check (NFR-CORPUS-04): split a second
         time with the same SplitConfig and assert SplitReports
         are byte-equal.
      9. Print `FSV PASSED: split-evidence + split-assignments OK
         (instance=[240,30,30], repo=[N,M,K])` on success.
  </in_scope>
  <out_of_scope>
    - The corpus FSV (TASK-CORPUS-024).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-corpus/examples/split_fsv.rs">
    fn main() -&gt; std::process::ExitCode;
    </signature>
  </signatures>

  <constraints>
    - File mode 0o600 enforced on both evidence files.
    - Byte-equality asserted by re-reading the file and
      comparing the raw bytes — NOT serde round-trip equality
      (which would tolerate whitespace differences).
    - Determinism check: byte-equal SplitReport from two split
      runs with same SplitConfig.seed.
    - The harness creates the
      `/mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/` directory if missing.
    - InstanceAtomic MUST yield `bucket_sizes = [240, 30, 30]`;
      this is asserted in code (the harness fails if not).
    - RepoAtomic bucket_sizes are not pinned (depends on shuffle
      seed) but `leakage_check_passed = true` MUST hold.
  </constraints>

  <verification>
    - `cargo run --release -p context-graph-mejepa-corpus --example split_fsv`
      exits 0.
    - `stat -c %a /mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/split-evidence.json` returns `600`.
    - `stat -c %a /mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/split-assignments.json` returns `600`.
    - Stdout contains "FSV PASSED" and the bucket sizes.
  </verification>
</definition_of_done>

<pseudo_code>
fn main() -&gt; ExitCode {
    if !cfg!(unix) { eprintln!("FSV requires Unix; skipping"); return ExitCode::SUCCESS; }
    let entries = load_swebench_lite_entries("tasks/swebench-lite/")?;
    let instance = split_corpus(&amp;entries, &amp;SplitConfig::default())?;
    assert_eq!(instance.bucket_sizes, [240, 30, 30]);
    let repo_cfg = SplitConfig { mode: SplitMode::RepoAtomic, ..Default::default() };
    let repo = split_corpus(&amp;entries, &amp;repo_cfg)?;
    assert!(repo.leakage_check_passed);
    let split_evidence_path = "/mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/split-evidence.json";
    let assignments_path = "/mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/split-assignments.json";
    let evidence = serde_json::json!({
        "instance_atomic": { "bucket_sizes": instance.bucket_sizes,
                             "bucket_fractions": instance.bucket_fractions,
                             "leakage_check_passed": instance.leakage_check_passed },
        "repo_atomic": { ... },
    });
    write_with_mode_0o600(split_evidence_path, evidence.to_string().as_bytes())?;
    write_with_mode_0o600(assignments_path, serde_json::to_string(&amp;instance.assignments)?.as_bytes())?;
    let back_evidence = fs::read(split_evidence_path)?;
    assert_eq!(back_evidence, evidence.to_string().as_bytes());
    let determinism_check = split_corpus(&amp;entries, &amp;SplitConfig::default())?;
    assert_eq!(serde_json::to_vec(&amp;determinism_check)?, serde_json::to_vec(&amp;instance)?);
    println!("FSV PASSED: split-evidence + split-assignments OK (instance=[240,30,30], repo={:?})", repo.bucket_sizes);
    ExitCode::SUCCESS
}
</pseudo_code>

<files_to_create>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-corpus/examples/split_fsv.rs">Verify shipped harness; add determinism check + 0o600 mode.</file>
</files_to_modify>

<validation_criteria>
  <criterion>Both evidence files exist with mode 0o600 (NFR-CORPUS-06)</criterion>
  <criterion>InstanceAtomic produces bucket_sizes=[240, 30, 30]</criterion>
  <criterion>RepoAtomic leakage_check_passed=true</criterion>
  <criterion>Re-read byte-equal to written content</criterion>
  <criterion>NFR-CORPUS-04 determinism: same seed → byte-equal report</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-corpus --example split_fsv</command>
  <command>cargo run --release -p context-graph-mejepa-corpus --example split_fsv</command>
  <command>stat -c %a /mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/split-evidence.json</command>
  <command>stat -c %a /mnt/d/contextgraph/fsv/context-graph-mejepa-corpus/split-assignments.json</command>
</test_commands>
</task_spec>
```

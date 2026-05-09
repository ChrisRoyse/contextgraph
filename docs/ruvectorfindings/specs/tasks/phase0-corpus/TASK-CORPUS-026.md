# TASK-CORPUS-026 — CLI generate Subcommand + SIGINT Trap

```xml
<task_spec id="TASK-CORPUS-026" version="1.0">
<metadata>
  <title>mejepa-corpus generate Subcommand (clap parse + SIGINT trap + complete=false on interrupt)</title>
  <status>partial</status>
  <implementation_state as_of="2026-05-09">
    Shipped: clap-parsed `mejepa-corpus generate` subcommand in `bin/mejepa-corpus.rs:43-73, 139-301` exposing `--output`, `--tasks-dir`, `--venv-python`, `--instance-id` (multi), `--category` (multi), `--max-tasks`, `--repo-cache-dir`, `--source-work-root`, `--run-id-prefix`, `--instance-timeout-secs`, `--overall-timeout-secs`. Source-backed patch generation pipeline (commits b6b4810c, 3f46ff78) builds canonical mutated patches before invoking the Docker oracle. The on-disk `index.json` writes `complete: false` initially and only flips to `complete: true` after the last entry — fail-closed per REQ-CORPUS-16.
    Pending: SIGINT trap (`ctrlc::set_handler` or `tokio::signal::ctrl_c`) so an operator-initiated `^C` mid-batch leaves `complete: false` permanently and prints the partial-progress summary, plus a graceful-shutdown unit test that verifies the trap behaviour without spawning Docker.
  </implementation_state>
  <layer>surface</layer>
  <sequence>26</sequence>
  <implements>
    <requirement_ref>REQ-CORPUS-09</requirement_ref>
    <requirement_ref>REQ-CORPUS-01</requirement_ref>
    <requirement_ref>REQ-CORPUS-13</requirement_ref>
    <requirement_ref>REQ-CORPUS-16</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORPUS-023</task_ref>
    <task_ref>TASK-CORPUS-024</task_ref>
    <task_ref>TASK-CORPUS-025</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
</metadata>

<context>
Surface task. The operator-facing CLI for the batch generator.
Parses clap args, validates, runs
`generate_swe_bench_mutation_corpus_with_oracle(...)`, traps
SIGINT to flush partial corpus-index.json with
`complete: false`, exits 0 on success / non-zero on any
failure. Per EC-CORPUS-12, mid-run Ctrl-C → flush + exit
non-zero (no resume support in v1).
</context>

<input_context_files>
  <file purpose="api_contract">../../technical/TECH-CORPUS.md#api_contracts (mejepa-corpus generate)</file>
  <file purpose="component_contract">../../technical/TECH-CORPUS.md#component_contracts (MejepaCorpusCli)</file>
  <file purpose="story">../../functional/SPEC-CORPUS.md (US-CORPUS-01, US-CORPUS-04, US-CORPUS-06, EC-CORPUS-12)</file>
  <file purpose="business_rule">../../functional/SPEC-CORPUS.md (BR-CORPUS-01)</file>
  <file purpose="usability">../../functional/SPEC-CORPUS.md (NFR-CORPUS-07)</file>
  <file purpose="existing">crates/context-graph-mejepa-corpus/src/bin/mejepa-corpus.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORPUS-023 complete (library entry point exists)</check>
  <check>TASK-CORPUS-024 + 025 complete (FSV harnesses exist)</check>
  <check>clap 4.5+ in Cargo.toml</check>
</prerequisites>

<scope>
  <in_scope>
    - Implement `clap::Cli` enum with subcommands `Generate`,
      `Verify`, `Stats` (the latter two ship in TASK-CORPUS-027,
      028; this task scaffolds the dispatcher).
    - `Generate` flags per TECH-CORPUS API contract:
      `--output`, `--tasks-dir`, `--venv-python`, `--languages`,
      `--instance-id` (repeatable), `--category` (repeatable),
      `--max-tasks`, `--seed`, `--run-id-prefix`,
      `--instance-timeout-secs`, `--overall-timeout-secs`,
      `--alternate-wrong-file-path`,
      `--sample-fraction` (ignored in generate; documented).
    - `validate_generate_args(args: &amp;GenerateArgs) -&gt;
      CliResult&lt;()&gt;` per TECH-CORPUS:
      1. validate_run_id(&amp;args.run_id_prefix).
      2. parse_languages(&amp;args.languages); reject any
         non-Python with MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE.
      3. WrongFile category requires
         --alternate-wrong-file-path.
      4. Verify --venv-python exists and is executable.
    - `parse_categories(raw: &amp;[String]) -&gt;
      CliResult&lt;Vec&lt;MutationCategory&gt;&gt;`: maps each
      `--category` flag value via
      `MutationCategory::from_slug`; default = all 8 if empty.
    - `parse_languages(raw: &amp;[String]) -&gt;
      CliResult&lt;Vec&lt;Language&gt;&gt;`: thin wrapper over
      `lib::parse_languages`.
    - SIGINT trap: install `tokio::signal::ctrl_c` (or
      `signal-hook` if not async) before generation starts.
      When fired:
      1. Set an `AtomicBool` shutdown flag.
      2. The main loop in
         `generate_swe_bench_mutation_corpus_with_oracle` polls
         the flag between (task, category) iterations and
         returns `MEJEPA_CORPUS_INTERRUPTED` (NEW variant; OR
         re-uses `MutationError::OperatorFailed` with field
         "shutdown" — choose one and document).
      3. The CLI catches the error, persists corpus-index.json
         with `complete: false`, exits non-zero.
    - `--help` output includes a usage example
      (`--output ./data/mejepa-corpus --languages python`) per
      NFR-CORPUS-07.
    - On success: invoke FSV evidence write (the FSV file from
      TASK-CORPUS-024 / TASK-CORPUS-025 is written by the
      orchestrator OR by the CLI as a final step — the CLI
      runs both FSV harnesses' `fn main()` programmatically
      OR re-invokes them as subprocesses).
    - Add CLI smoke tests:
      - `--help` exits 0 and mentions every flag (NFR-CORPUS-07).
      - Bad `--run-id-prefix` exits non-zero with
        MEJEPA_CORPUS_DOCKER_RUN_ID_UNSAFE.
      - Bad `--languages rust` exits non-zero with
        MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE.
      - `--max-tasks 2` against a stubbed Oracle produces
        16 entries (TC-CORPUS-11 smoke; integration test
        gated `#[ignore]` for real Docker).
  </in_scope>
  <out_of_scope>
    - `verify` subcommand (TASK-CORPUS-027).
    - `stats` subcommand (TASK-CORPUS-028).
    - Resume support after Ctrl-C (Q-CORPUS-01 — deferred).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-corpus/src/bin/mejepa-corpus.rs">
    fn main() -&gt; std::process::ExitCode;

    #[derive(clap::Parser)]
    #[command(name = "mejepa-corpus", about = "...")]
    struct Cli { #[command(subcommand)] cmd: Cmd }

    #[derive(clap::Subcommand)]
    enum Cmd { Generate(GenerateArgs), Verify(VerifyArgs), Stats(StatsArgs) }

    #[derive(clap::Args)]
    struct GenerateArgs { /* all flags per TECH-CORPUS */ }

    fn validate_generate_args(args: &amp;GenerateArgs) -&gt; CliResult&lt;()&gt;;
    fn parse_categories(raw: &amp;[String]) -&gt; CliResult&lt;Vec&lt;MutationCategory&gt;&gt;;

    fn run_generate(args: GenerateArgs) -&gt; CliResult&lt;()&gt;;
    </signature>
  </signatures>

  <constraints>
    - clap version pinned to workspace 4.5.x.
    - SIGINT shutdown is GRACEFUL: in-flight (task, category)
      finishes if possible; loop checks flag BETWEEN iterations.
    - Source-of-truth file paths in error messages start with
      `file:`.
    - Stop-hook recommendation: when wrapped in a /loop turn,
      surface the FSV evidence path in the recommendation JSON's
      `evidence` field.
    - The CLI MUST NOT use `unwrap()` on user-controlled input.
    - File MUST stay under 500 lines (constitution); split into
      `bin/mejepa-corpus/{main, generate, args}.rs` if needed.
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-mejepa-corpus --bin mejepa-corpus` clean.
    - `cargo test -p context-graph-mejepa-corpus -- cli_help_example` passes.
    - `cargo test -p context-graph-mejepa-corpus -- cli_bad_run_id` passes.
    - `cargo test -p context-graph-mejepa-corpus -- cli_unsupported_language` passes.
    - `target/release/mejepa-corpus --help` exits 0 and lists all flags.
    - `target/release/mejepa-corpus generate --output /tmp/x --languages rust ...`
      exits non-zero with MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE.
  </verification>
</definition_of_done>

<pseudo_code>
fn main() -&gt; ExitCode {
    let cli = Cli::parse();
    let result = match cli.cmd {
        Cmd::Generate(a) =&gt; run_generate(a),
        Cmd::Verify(a) =&gt; run_verify(a),
        Cmd::Stats(a) =&gt; run_stats(a),
    };
    match result {
        Ok(()) =&gt; ExitCode::SUCCESS,
        Err(e) =&gt; { eprintln!("error[{}]: {e}", e.code()); ExitCode::FAILURE }
    }
}

fn run_generate(a: GenerateArgs) -&gt; CliResult&lt;()&gt; {
    validate_generate_args(&amp;a)?;
    let categories = parse_categories(&amp;a.category)?;
    let languages = parse_languages(&amp;a.languages)?;
    let shutdown = Arc::new(AtomicBool::new(false));
    install_sigint_trap(shutdown.clone())?;
    let report = generate_swe_bench_mutation_corpus_with_oracle(
        Path::new(&amp;a.tasks_dir), Path::new(&amp;a.output),
        &amp;categories, &amp;languages, &amp;RealDockerOracle, shutdown,
    ).map_err(CliError::from)?;
    if !report.complete { return Err(CliError::IncompleteCorpus); }
    println!("complete: {}, entries: {}, evidence: file://{}", report.complete, report.entry_count, report.index_path.display());
    Ok(())
}

#[test]
fn cli_unsupported_language_exits_nonzero() {
    let out = Command::new(env!("CARGO_BIN_EXE_mejepa-corpus"))
        .args(&amp;["generate", "--output", "/tmp/x", "--languages", "rust",
                 "--venv-python", "/usr/bin/python3", "--tasks-dir", "tasks/swebench-lite/"])
        .output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&amp;out.stderr).contains("MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE"));
}
</pseudo_code>

<files_to_create>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-corpus/src/bin/mejepa-corpus.rs">Implement Cli + run_generate + SIGINT trap; verify shipped scaffolding.</file>
  <file path="crates/context-graph-mejepa-corpus/Cargo.toml">Add signal-hook crate dep if needed for SIGINT.</file>
  <file path="crates/context-graph-mejepa-corpus/src/tests.rs">Add 4 CLI smoke tests via assert_cmd or std::process::Command.</file>
</files_to_modify>

<validation_criteria>
  <criterion>--help exits 0 and lists all flags + usage example (NFR-CORPUS-07)</criterion>
  <criterion>Bad --run-id-prefix exits non-zero with MEJEPA_CORPUS_DOCKER_RUN_ID_UNSAFE</criterion>
  <criterion>--languages rust exits non-zero with MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE</criterion>
  <criterion>SIGINT during generate leaves corpus-index.json with complete:false and exits non-zero (EC-CORPUS-12)</criterion>
  <criterion>--max-tasks 2 + stubbed Oracle produces 16 entries with complete:true</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-corpus --bin mejepa-corpus</command>
  <command>cargo test -p context-graph-mejepa-corpus -- cli_help_example</command>
  <command>cargo test -p context-graph-mejepa-corpus -- cli_bad_run_id</command>
  <command>cargo test -p context-graph-mejepa-corpus -- cli_unsupported_language</command>
  <command>./target/release/mejepa-corpus --help</command>
  <command>cargo clippy -p context-graph-mejepa-corpus -- -D warnings</command>
</test_commands>
</task_spec>
```

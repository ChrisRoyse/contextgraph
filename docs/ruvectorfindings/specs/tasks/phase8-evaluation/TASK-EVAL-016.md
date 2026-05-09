# TASK-EVAL-016 — CLI subcommands `eval-run` (Phase 8 DoD command; exit 0/1/2) + `eval-build-graph`

```xml
<task_spec id="TASK-EVAL-016" version="1.0">

<metadata>
  <title>Implement CLI subcommands `eval-run` + `eval-build-graph` in `crates/context-graph-mejepa/src/bin/mejepa.rs` + `src/cli/eval.rs`</title>
  <status>blocked</status>
  <layer>surface</layer>
  <sequence>16</sequence>
  <implements>
    <item ref="REQ-EVAL-15">Phase 8 DoD CLI command: `cargo run --release -p context-graph-mejepa -- eval-run --window-days 7 --include-per-cell` writes weekly report; exit 0 ship-pass; exit 1 ship-fail; exit 2 hard error</item>
    <item ref="REQ-EVAL-16">CLI must hit &lt; 30 min p50 budget on RTX 5090 (NFR-EVAL-02 echo)</item>
    <item ref="REQ-EVAL-07">eval-build-graph subcommand triggers `build_patch_similarity_graph` independently of full eval</item>
    <item ref="component:Phase8DoDCommand">Mirror MCP eval_run behavior; structured stdout JSON; structured exit codes</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EVAL-010</task_ref><!-- build_patch_similarity_graph for eval-build-graph -->
    <task_ref>TASK-EVAL-015</task_ref><!-- write_weekly_report wired through runner -->
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (cli/eval.rs); 1 modified (bin/mejepa.rs to register subcommands)</estimated_files>
</metadata>

<context>
This task lands the **Phase 8 DoD CLI command** alongside the SHIFT-014 `verify` subcommand on the same `mejepa` top-level binary. Two new subcommands:

1. **`eval-run`**: triggers a full holdout evaluation (delegates to `EvalRunner::run_holdout_eval`); writes the weekly report via TASK-EVAL-015's `write_weekly_report`; prints a single-line stdout JSON summary; exits with structured code:
   - exit 0: ship-gate passed
   - exit 1: ship-gate failed (any of 8 conditions OR week-over-week regression)
   - exit 2: hard error (corpus not found, holdout leakage, RocksDB error, &gt; 30 min timeout, etc.)

2. **`eval-build-graph`**: triggers `build_patch_similarity_graph` standalone; useful for offline graph rebuilds when the TRAINING phase wants a fresh graph mid-week. Idempotent on corpus_sha per TASK-EVAL-010. Prints stdout JSON with edge count + node count + corpus_sha. Exit 0 success; exit 2 on error.

Both subcommands honor `--workspace-root`, `--corpus-root`, `--rocksdb-path` global flags consistent with the existing `verify` subcommand. Determinism: identical inputs → identical stdout JSON (modulo wall-clock fields that participate in `hash_for_determinism` exclusion).

Performance target: `eval-run` must complete &lt; 30 min p50 per NFR-EVAL-02 on RTX 5090 with cached panels. The CLI does NOT spawn the eval in a separate process — it runs in-process so we can capture the structured EvalReport directly.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (performance_budgets phase8_ship_gate; eval_pipeline_walltime_p50_max_min 30)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-EVALUATION.md (REQ-EVAL-15; REQ-EVAL-16; AC-EVAL-01-* DoD scenarios)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-EVALUATION.md (Phase 8 DoD command; CLI subcommand layout)</file>
  <file purpose="prior_task">TASK-EVAL-010 (graph build), TASK-EVAL-015 (weekly report writer); TASK-SHIFT-014 (CLI binary scaffold + clap pattern)</file>
</input_context_files>

<prerequisites>
  <check>TASK-EVAL-010 + 015 shipped: build_patch_similarity_graph + write_weekly_report callable</check>
  <check>TASK-SHIFT-014 shipped: `crates/context-graph-mejepa/src/bin/mejepa.rs` exists with clap scaffolding (this task adds 2 subcommands)</check>
  <check>`clap` v4 with derive feature available</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/cli/mod.rs` with `pub mod eval;`
    - Create `crates/context-graph-mejepa/src/cli/eval.rs` with:
      - `pub fn run_eval_command(args: EvalRunArgs, ctx: CliContext) -> ExitCode` — full body of the DoD command:
        1. **Construct EvalRunner**: load `HoldoutDataset`, `CalibrationDataset`, `Predictor`, `FrozenTarget`, `CalibrationStore`, `OodDetector`, `ConstellationGuard`, `RocksDbStore`, `WitnessChainReader` from ctx; `EvalConfig` from `args` (override min_samples_per_cell, alpha, etc. via flags)
        2. **Call** `runner.run_holdout_eval(args.window_days, args.include_per_cell)` with `tokio::time::timeout(30 min)` if running on a runtime; otherwise direct call
        3. **Print stdout JSON** (single-line, NOT pretty) with shape:
           ```json
           {
             "report_date": "2026-05-09",
             "report_path": "file:./data/mejepa-eval/mejepa-weekly-report-2026-05-09.json",
             "ship_gate_passed": true,
             "blockers": [],
             "overall_correlation": 0.962,
             "wall_clock_seconds": 423.7,
             "exit_code": 0
           }
           ```
        4. **Exit code mapping**:
           - `report.ship_gate_passed == true` → exit 0
           - `report.ship_gate_passed == false` → exit 1
           - On `EvalError`: exit 2; print stderr with `e.code()` + payload
           - On timeout: exit 2; print `MEJEPA_EVAL_TIMEOUT`
      - `pub fn run_build_graph_command(args: EvalBuildGraphArgs, ctx: CliContext) -> ExitCode`:
        1. Load corpus via `Corpus::load(&ctx.corpus_root)?`
        2. Load E_Diff embedder; call `build_patch_similarity_graph(&corpus, &e_diff, &EvalConfig::default(), &storage)`
        3. Print stdout JSON: `{"corpus_sha": "...", "node_count": ..., "edge_count": ..., "built_at": "...", "exit_code": 0}`
        4. Exit 0 success; exit 2 on `GraphError` (with stderr error code)
      - **Args** with `clap::Parser`:
        ```rust
        #[derive(Parser)]
        pub struct EvalRunArgs {
            #[arg(long, default_value_t = 7)]
            pub window_days: u32,
            #[arg(long)]
            pub include_per_cell: bool,
            #[arg(long, default_value = "./data/mejepa-eval/")]
            pub output_dir: PathBuf,
            #[arg(long)]
            pub disable_bayesian_shrinkage: bool,
            #[arg(long, default_value_t = 30)]
            pub min_samples_per_cell: usize,
        }

        #[derive(Parser)]
        pub struct EvalBuildGraphArgs {
            #[arg(long, default_value = "./data/mejepa-corpus/")]
            pub corpus_root: PathBuf,
            #[arg(long, default_value_t = 0.7)]
            pub cosine_threshold: f32,
            #[arg(long, default_value_t = 32)]
            pub top_k: usize,
        }
        ```
    - Update `crates/context-graph-mejepa/src/bin/mejepa.rs` (already exists from TASK-SHIFT-014):
      - Add `EvalRun(EvalRunArgs)` and `EvalBuildGraph(EvalBuildGraphArgs)` variants to the `Cmd` enum
      - Wire `match` arms to `cli::eval::run_eval_command(args, ctx)` and `cli::eval::run_build_graph_command(args, ctx)`
    - Update `crates/context-graph-mejepa/src/lib.rs`: add `pub mod cli;`
    - Add unit + integration tests in `cli/eval.rs` `#[cfg(test)] mod tests`:
      - `eval_run_returns_exit_0_on_ship_pass` (mock runner returns ship_gate_passed=true)
      - `eval_run_returns_exit_1_on_ship_fail` (ship_gate_passed=false)
      - `eval_run_returns_exit_2_on_eval_error` (mock runner returns Err(EmptyHoldout))
      - `eval_run_returns_exit_2_on_timeout` (artificial 35-min mock; expect timeout exit)
      - `eval_run_stdout_is_single_line_json` (no embedded newlines)
      - `eval_run_stdout_parses_to_canonical_struct`
      - `eval_run_window_days_from_flag_overrides_default` (verify clap parsing)
      - `eval_run_disable_bayesian_shrinkage_flag_propagates_to_config` (verify config propagation)
      - `build_graph_returns_exit_0_on_success`
      - `build_graph_returns_exit_2_on_graph_error`
      - `build_graph_stdout_includes_corpus_sha_and_edge_count`
      - `build_graph_idempotent_on_unchanged_corpus_sha` (2 runs in row → second exits 0 with same SHA, &lt; 100 ms wall-clock)
  </in_scope>
  <out_of_scope>
    - The MCP handler bodies (TASK-EVAL-013, TASK-EVAL-014)
    - The FSV harness (TASK-EVAL-017)
    - Implementation of the eval/build-graph algorithms (TASK-EVAL-004..012)
    - Other unrelated subcommands (e.g., training, distillation — those land in their own SPEC's CLI tasks)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/cli/eval.rs">
use std::path::PathBuf;
use std::process::ExitCode;
use clap::Parser;
use serde::Serialize;

#[derive(Debug, Clone, Parser)]
pub struct EvalRunArgs {
    #[arg(long, default_value_t = 7)]
    pub window_days: u32,
    #[arg(long)]
    pub include_per_cell: bool,
    #[arg(long, default_value = "./data/mejepa-eval/")]
    pub output_dir: PathBuf,
    #[arg(long)]
    pub disable_bayesian_shrinkage: bool,
    #[arg(long, default_value_t = 30)]
    pub min_samples_per_cell: usize,
}

#[derive(Debug, Clone, Parser)]
pub struct EvalBuildGraphArgs {
    #[arg(long, default_value = "./data/mejepa-corpus/")]
    pub corpus_root: PathBuf,
    #[arg(long, default_value_t = 0.7)]
    pub cosine_threshold: f32,
    #[arg(long, default_value_t = 32)]
    pub top_k: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvalRunStdoutSummary {
    pub report_date: chrono::NaiveDate,
    pub report_path: String,
    pub ship_gate_passed: bool,
    pub blockers: Vec&lt;String&gt;,
    pub overall_correlation: f32,
    pub wall_clock_seconds: f64,
    pub exit_code: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct BuildGraphStdoutSummary {
    pub corpus_sha: String, // hex
    pub node_count: usize,
    pub edge_count: usize,
    pub built_at: i64,
    pub exit_code: u8,
}

pub struct CliContext { /* workspace_root, rocksdb_path, etc. */ }

pub fn run_eval_command(args: EvalRunArgs, ctx: CliContext) -&gt; ExitCode;
pub fn run_build_graph_command(args: EvalBuildGraphArgs, ctx: CliContext) -&gt; ExitCode;
    </signature>
  </signatures>
  <constraints>
    - Stdout output is a SINGLE LINE of `serde_json::to_string` (NOT `to_string_pretty`); preceded by tracing logs on stderr; clean separation for shell `| jq` pipelines
    - Exit codes strictly 0 / 1 / 2 (no other values)
    - Timeout 30 min via `tokio::time::timeout` if a runtime is available; otherwise std::sync::mpsc + thread::spawn pattern
    - All paths canonicalized + checked against workspace root (per SEC-MEJEPA-01)
    - `corpus_sha` in stdout is hex-encoded (64 chars) for terminal portability
    - All Err returns wrapped with `.log_context("cli::eval")` per `&lt;error_handling&gt;`
    - File length budget: &lt; 500 lines per CLAUDE.md §4
    - The `Cmd` enum in `bin/mejepa.rs` extends the existing TASK-SHIFT-014 `Verify` variant — does NOT replace it
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa --bin mejepa` clean
    - `cargo run --release -p context-graph-mejepa -- eval-run --window-days 7 --include-per-cell` exits 0 on ship-pass fixture, 1 on ship-fail, 2 on hard error
    - `cargo run --release -p context-graph-mejepa -- eval-build-graph` exits 0 on success
    - ≥ 12 passing tests
    - cargo clippy clean
  </verification>
</definition_of_done>

<pseudo_code>
crates/context-graph-mejepa/src/cli/eval.rs:
  pub fn run_eval_command(args, ctx) -&gt; ExitCode {
      let runner = match construct_eval_runner(&ctx, &args) {
          Ok(r) =&gt; r,
          Err(e) =&gt; { eprintln!("{}: {}", e.code(), e); return ExitCode::from(2); }
      };
      let report = match runner.run_holdout_eval(args.window_days, args.include_per_cell) {
          Ok(r) =&gt; r,
          Err(e) =&gt; { eprintln!("{}: {}", e.code(), e); return ExitCode::from(2); }
      };
      let summary = EvalRunStdoutSummary {
          report_date: report.report_date,
          report_path: format!("file:{}", construct_canonical_path(&args.output_dir, report.report_date).display()),
          ship_gate_passed: report.ship_gate_passed,
          blockers: report.ship_gate_failures.clone(),
          overall_correlation: report.overall_correlation,
          wall_clock_seconds: report.wall_clock_seconds,
          exit_code: if report.ship_gate_passed { 0 } else { 1 },
      };
      println!("{}", serde_json::to_string(&summary).unwrap_or_default());
      ExitCode::from(summary.exit_code)
  }

  pub fn run_build_graph_command(args, ctx) -&gt; ExitCode {
      let corpus = match Corpus::load(&args.corpus_root) {
          Ok(c) =&gt; c,
          Err(e) =&gt; { eprintln!("MEJEPA_EVAL_CORPUS_LOAD_FAIL: {e}"); return ExitCode::from(2); }
      };
      let e_diff = match ctx.e_diff_encoder() {
          Ok(e) =&gt; e,
          Err(e) =&gt; { eprintln!("MEJEPA_EVAL_EMBEDDER_UNAVAILABLE: {e}"); return ExitCode::from(2); }
      };
      let mut config = EvalConfig::default();
      config.task_graph_top_k = args.top_k;
      config.task_graph_cosine_threshold = args.cosine_threshold;
      let storage = ctx.rocksdb_store();
      let graph = match build_patch_similarity_graph(&corpus, e_diff.as_ref(), &config, &storage) {
          Ok(g) =&gt; g,
          Err(e) =&gt; { eprintln!("{}: {}", e.code(), e); return ExitCode::from(2); }
      };
      let summary = BuildGraphStdoutSummary {
          corpus_sha: hex::encode(graph.corpus_sha),
          node_count: graph.adjacency.len(),
          edge_count: graph.edges.len(),
          built_at: graph.built_at,
          exit_code: 0,
      };
      println!("{}", serde_json::to_string(&summary).unwrap_or_default());
      ExitCode::from(0)
  }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/cli/mod.rs">declare `pub mod eval;`</file>
  <file path="crates/context-graph-mejepa/src/cli/eval.rs">EvalRunArgs + EvalBuildGraphArgs + run_eval_command + run_build_graph_command + CliContext + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/bin/mejepa.rs">extend Cmd enum with EvalRun + EvalBuildGraph variants; wire match arms (preserves Verify variant from TASK-SHIFT-014)</file>
  <file path="crates/context-graph-mejepa/src/lib.rs">add `pub mod cli;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build / test / clippy clean</criterion>
  <criterion>`mejepa eval-run --window-days 7` exits 0 on ship-pass fixture</criterion>
  <criterion>`mejepa eval-run` exits 1 on ship-fail fixture</criterion>
  <criterion>`mejepa eval-run` exits 2 on hard error (RocksDB missing, etc.)</criterion>
  <criterion>`mejepa eval-build-graph` exits 0 on success</criterion>
  <criterion>`mejepa eval-build-graph` exits 2 on GraphError</criterion>
  <criterion>Stdout is single-line JSON parseable to summary struct</criterion>
  <criterion>Tracing logs go to stderr (not stdout)</criterion>
  <criterion>--disable-bayesian-shrinkage flag propagates to EvalConfig</criterion>
  <criterion>--include-per-cell flag propagates to runner</criterion>
  <criterion>build-graph idempotent: 2nd run on unchanged corpus completes &lt; 100 ms</criterion>
  <criterion>Both subcommands coexist with `verify` subcommand from TASK-SHIFT-014</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa --bin mejepa</command>
  <command>cargo run --release -p context-graph-mejepa -- eval-run --window-days 7 --include-per-cell</command>
  <command>cargo run --release -p context-graph-mejepa -- eval-build-graph</command>
  <command>cargo test -p context-graph-mejepa cli::eval</command>
  <command>cargo clippy -p context-graph-mejepa --bin mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```

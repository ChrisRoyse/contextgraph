# TASK-INFER-013 — CLI `infer-test` subcommand (Phase 4 DoD verification)

```xml
<task_spec id="TASK-INFER-013" version="1.0">

<metadata>
  <title>`cargo run --release -p context-graph-mejepa -- infer-test` in `crates/context-graph-mejepa/src/cli.rs` + `bin/mejepa.rs`</title>
  <status>blocked</status>
  <layer>surface</layer>
  <sequence>13</sequence>
  <implements>
    <item ref="REQ-INFER-20">Phase 4 DoD CLI: stated_coverage 0.90, empirical 0.88-0.92, prediction_latency_p50 &lt; 100ms, ood_separation_auc &gt; 0.85</item>
    <item ref="REQ-INFER-18">Performance budgets verified by the CLI run</item>
    <item ref="AC-INFER-02-01">CLI driver per US-INFER-02</item>
    <item ref="AC-INFER-02-02">non-zero exit on out-of-range coverage</item>
    <item ref="component:cli">infer-test CLI flags</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INFER-005</task_ref>
    <task_ref>TASK-INFER-006</task_ref>
    <task_ref>TASK-INFER-010</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>2 created (cli.rs, bin/mejepa.rs)</estimated_files>
</metadata>

<context>
The Phase 4 DoD verification command is THE check that gates promotion of a calibration version
to production. It loads the 10% calibration split (stratified by language), runs forward passes,
computes τ via TASK-INFER-005, σ² via TASK-INFER-006, persists the resulting CalibrationRecord via
TASK-INFER-010, then measures (a) empirical coverage on a held-out subset, (b) p50 prediction
latency over 1,000 verify() calls, (c) OOD separation AUC between corpus panels and synthetic OOD
panels. Reports JSON to stdout; FSV evidence to /tmp/contextgraph-mejepa-infer-fsv/ at mode 0o600.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (performance_budgets; SEC-MEJEPA-07; fsv_protocol)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INFERENCE.md (REQ-INFER-18, REQ-INFER-20; AC-INFER-02-01..03)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INFERENCE.md (api_contracts cli; phase4_dod block)</file>
  <file purpose="upstream_calib">crates/context-graph-mejepa/src/calibration.rs (CalibrationStore.calibrate)</file>
  <file purpose="upstream_compile">crates/context-graph-mejepa/src/compiler.rs (MeJepaCompiler::verify for latency)</file>
  <file purpose="upstream_ood">crates/context-graph-mejepa/src/ood.rs (separation_auc)</file>
</input_context_files>

<prerequisites>
  <check>TASK-INFER-005, 006, 010 complete</check>
  <check>Phase 0 calibration split available at `./data/mejepa-corpus/calibration` (stratified by language; ≥ 30 per language)</check>
  <check>`clap = "4"` workspace dep available</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/cli.rs` with:
      - `pub struct InferTestArgs { calibration: PathBuf, alpha: f32, output_fsv: PathBuf }` parsed via clap
      - `pub fn run_infer_test(args: InferTestArgs, compiler: &MeJepaCompiler, store: &CalibrationStore) -> Result<InferTestReport, MejepaInferError>` — load stratified calibration examples; forward-pass each to obtain (predicted_test_pass, actual_test_pass, norm_sq); call store.calibrate(...); evaluate empirical coverage on held-out 20% subset of the calibration split; run 1,000 verify() calls measuring per-call latency; compute OOD AUC against synthetic OOD panels (random AST diffs from a held-out repo loaded from `args.calibration / "ood_synthetic"`); write FSV evidence
      - `pub struct InferTestReport { stated_coverage: f32, empirical_coverage: f32, prediction_latency_p50_ms: f32, prediction_latency_p99_ms: f32, ood_separation_auc: f32, num_calibration_samples: usize, calibration_version: String }`
      - Exit code: 0 if `empirical_coverage ∈ [0.88, 0.92] AND prediction_latency_p50_ms < 100 AND ood_separation_auc > 0.85`; else 1 (per AC-INFER-02-02)
      - On staleness (`MEJEPA_INFER_CALIBRATION_STALE`) emit on stderr with recovery action; exit 2
      - On insufficient samples emit `MEJEPA_INFER_CONFORMAL_INSUFFICIENT_SAMPLES` with the under-represented language; exit 3
    - Create `crates/context-graph-mejepa/src/bin/mejepa.rs` binary entry point dispatching `infer-test` subcommand to `cli::run_infer_test`
    - Initialize MeJepaCompiler with mock predictor/frozen_target/oracle_head until Phase 2 ships; document mock semantics in --help
    - Write FSV evidence files at /tmp/contextgraph-mejepa-infer-fsv/ with mode 0o600 (TASK-INFER-014 owns the formal harness; this task writes a minimal subset for the CLI's own verification):
      - `infer-test-report.json` (the InferTestReport struct)
      - `latency-histogram.json` (raw latency measurements for the 1,000 verify calls)
    - Add integration tests:
      - `infer_test_synthetic_corpus_passes_dod`: tempdir corpus with synthetic predicted/actual probabilities yielding empirical ∈ [0.88, 0.92] → exit 0
      - `infer_test_out_of_range_coverage_exits_nonzero`: synthesize corpus that calibrates poorly → exit 1
      - `infer_test_insufficient_samples_per_language_exits_3`: <30 Python examples → exit 3 with structured stderr
      - `infer_test_writes_fsv_files_with_mode_0600`: stat FSV files → mode bits 0o600
      - `infer_test_help_contains_required_flags`: --help mentions `--calibration`, `--alpha`, `--output-fsv`
  </in_scope>
  <out_of_scope>
    - Full FSV harness writing all 4 evidence files (TASK-INFER-014 — verify-evidence.json, conformal-calibration-evidence.json, ood-evidence.json with full ROC curves)
    - Real predictor weights (Phase 2)
    - Continuous-calibration scheduling (Phase 5)
    - SWE-bench Lite oracle hot-path (Phase 0)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/cli.rs">
use std::path::PathBuf;
use crate::compiler::MeJepaCompiler;
use crate::calibration::CalibrationStore;
use crate::error::MejepaInferError;

#[derive(Debug, clap::Args)]
pub struct InferTestArgs {
    #[arg(long)]
    pub calibration: PathBuf,
    #[arg(long, default_value_t = 0.10)]
    pub alpha: f32,
    #[arg(long, default_value = "/tmp/contextgraph-mejepa-infer-fsv/")]
    pub output_fsv: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InferTestReport {
    pub stated_coverage: f32,
    pub empirical_coverage: f32,
    pub prediction_latency_p50_ms: f32,
    pub prediction_latency_p99_ms: f32,
    pub ood_separation_auc: f32,
    pub num_calibration_samples: usize,
    pub calibration_version: String,
}

pub fn run_infer_test(
    args: InferTestArgs,
    compiler: &amp;MeJepaCompiler,
    store: &amp;CalibrationStore,
) -&gt; Result&lt;InferTestReport, MejepaInferError&gt;;

pub fn dod_satisfied(report: &amp;InferTestReport) -&gt; bool;
    </signature>
    <signature file="crates/context-graph-mejepa/src/bin/mejepa.rs">
#[derive(clap::Parser)]
#[command(name = "mejepa")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(clap::Subcommand)]
enum Cmd {
    /// Phase 4 DoD verification
    InferTest(crate::cli::InferTestArgs),
}

fn main() -&gt; std::process::ExitCode;
    </signature>
  </signatures>
  <constraints>
    - DoD pass: `empirical_coverage ∈ [0.88, 0.92] AND prediction_latency_p50_ms &lt; 100.0 AND ood_separation_auc &gt; 0.85`
    - Exit codes: 0 = pass; 1 = DoD violated; 2 = stale calibration; 3 = insufficient samples; 4 = other error
    - FSV file mode 0o600 (Unix `OpenOptions::new().mode(0o600)`)
    - JSON output to stdout pretty-printed (operator readable); structured error diagnostics to stderr
    - Latency measurement: `Instant::now()` around verify(); compute p50 via sort + index; p99 via sort + index
    - OOD AUC: load `args.calibration/ood_synthetic/*.panel` files (panels from a held-out repo); compute ood_score on each via `ood::compute_ood_score`; call `ood::separation_auc(in_dist_scores, ood_scores)`
    - Determinism: same calibration corpus + same code_version + same predictor weights → byte-equal report (modulo timing fields)
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa --bin mejepa` succeeds
    - `cargo test -p context-graph-mejepa cli` shows ≥ 5 passing tests
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - `cargo run --release -p context-graph-mejepa -- infer-test --help` lists all flags
    - DoD-passing synthetic corpus exits 0
    - DoD-failing synthetic corpus exits 1
    - Insufficient-sample stratum exits 3
  </verification>
</definition_of_done>

<pseudo_code>
pub fn run_infer_test(args, compiler, store) -> Result<InferTestReport, MejepaInferError> {
    let examples = load_calibration_examples(&args.calibration)?;
    let norm_sq = examples.iter().map(|e| compute_predictor_residual_norm_sq(compiler, e)).collect::<Vec<_>>();
    let record = store.calibrate(&examples, &norm_sq, args.alpha, 30, 0.30, corpus_sha, embedder_versions)?;

    // Held-out 20% subset
    let holdout_idx_start = (examples.len() as f32 * 0.8) as usize;
    let holdout = &examples[holdout_idx_start..];
    let count_in: usize = holdout.iter().filter(|e| {
        let s = non_conformity_score(&e.predicted_test_pass, &e.actual_test_pass).unwrap();
        s <= record.tau
    }).count();
    let empirical_coverage = count_in as f32 / holdout.len() as f32;

    // Latency
    let mut latencies_ms = Vec::with_capacity(1_000);
    for ex in examples.iter().take(1_000) {
        let bundle = synthesize_patch_bundle(ex);
        let context = synthesize_task_context(ex);
        let t0 = Instant::now();
        let _ = compiler.verify(&bundle, &context);
        latencies_ms.push(t0.elapsed().as_secs_f32() * 1000.0);
    }
    latencies_ms.sort_by(|a, b| a.total_cmp(b));
    let p50 = latencies_ms[latencies_ms.len() / 2];
    let p99 = latencies_ms[(latencies_ms.len() * 99) / 100];

    // OOD AUC
    let ood_scores: Vec<f32> = load_synthetic_ood_panels(&args.calibration.join("ood_synthetic"))?
        .iter().map(|p| ood::compute_ood_score_from_panel(compiler, p)).collect();
    let in_dist_scores: Vec<f32> = examples.iter().map(|e| ood::compute_ood_score_for_example(compiler, e)).collect();
    let auc = ood::separation_auc(&in_dist_scores, &ood_scores)?;

    let report = InferTestReport {
        stated_coverage: 1.0 - args.alpha,
        empirical_coverage,
        prediction_latency_p50_ms: p50,
        prediction_latency_p99_ms: p99,
        ood_separation_auc: auc,
        num_calibration_samples: examples.len(),
        calibration_version: record.version,
    };
    write_fsv_0600(&args.output_fsv.join("infer-test-report.json"), &report)?;
    write_fsv_0600(&args.output_fsv.join("latency-histogram.json"), &latencies_ms)?;
    Ok(report)
}

pub fn dod_satisfied(r: &InferTestReport) -> bool {
    r.empirical_coverage >= 0.88 && r.empirical_coverage <= 0.92
        && r.prediction_latency_p50_ms < 100.0
        && r.ood_separation_auc > 0.85
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::InferTest(args) => {
            let (compiler, store) = init_compiler_and_store();
            match run_infer_test(args, &compiler, &store) {
                Ok(r) => {
                    println!("{}", serde_json::to_string_pretty(&r).unwrap());
                    if dod_satisfied(&r) { ExitCode::SUCCESS } else { ExitCode::from(1) }
                },
                Err(e @ MejepaInferError::CalibrationStale { .. }) => { eprintln!("{}", e); ExitCode::from(2) },
                Err(e @ MejepaInferError::ConformalInsufficientSamples { .. }) => { eprintln!("{}", e); ExitCode::from(3) },
                Err(e) => { eprintln!("{}", e); ExitCode::from(4) },
            }
        }
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/cli.rs">InferTestArgs + InferTestReport + run_infer_test + dod_satisfied + 5 tests</file>
  <file path="crates/context-graph-mejepa/src/bin/mejepa.rs">binary entry point with clap dispatch</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/lib.rs">add `pub mod cli;`</file>
  <file path="crates/context-graph-mejepa/Cargo.toml">add `clap = { version = "4", features = ["derive"] }` + `[[bin]] name = "mejepa" path = "src/bin/mejepa.rs"`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa --bin mejepa zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa cli shows ≥ 5 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>cargo run --release -p context-graph-mejepa -- infer-test --help works</criterion>
  <criterion>DoD-passing synthetic corpus exits 0 with all 4 metrics in range</criterion>
  <criterion>DoD-failing exits 1</criterion>
  <criterion>Insufficient samples exits 3 with structured stderr</criterion>
  <criterion>FSV files mode 0o600 verified via stat</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa --bin mejepa</command>
  <command>cargo test -p context-graph-mejepa cli</command>
  <command>cargo run --release -p context-graph-mejepa -- infer-test --help</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```

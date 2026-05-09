# TASK-PRED-012 — Phase 2 DoD CLI: `bin/mejepa.rs train --dry-run --batches 1 --batch-size 64`

```xml
<task_spec id="TASK-PRED-012" version="1.0">

<metadata>
  <title>Phase 2 Definition-of-Done binary: cargo run --release -p context-graph-mejepa -- train --dry-run reports forward latency &lt; 50 ms, loss finite ~10², no NaN/Inf</title>
  <status>ready</status>
  <layer>surface</layer>
  <sequence>12</sequence>
  <implements>
    <item ref="REQ-PRED-15">CLI exit 0 with FSV evidence written + re-read + asserted; matches doc 09 §7 Phase 2 DoD</item>
    <item ref="REQ-PRED-03">forward_latency_ms_p50 &lt; 50 reported on stdout</item>
    <item ref="REQ-PRED-04">No NaN/Inf in any loss component or forward output</item>
    <item ref="REQ-PRED-13">vram_resident_bytes &lt; 8 GB reported on stdout</item>
    <item ref="REQ-PRED-06">L_total ∈ [10, 1000] enforced as DoD threshold</item>
    <item ref="component:MejepaTrainBin">bin/mejepa.rs `train --dry-run` subcommand</item>
  </implements>
  <depends_on>
    <task_ref>TASK-PRED-011</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (bin/mejepa.rs), 1 modified (Cargo.toml [[bin]])</estimated_files>
</metadata>

<context>
This is the Phase 2 closing surface: a separate `bin/` entry point that an operator (or CI)
can invoke to verify the doc 09 §7 Phase 2 Definition of Done in a single command:

```
cargo run --release -p context-graph-mejepa -- train --dry-run --batches 1 --batch-size 64
```

Distinct from `examples/predictor_fsv.rs` (TASK-PRED-011): the example harness runs the wider
adversarial battery; this CLI runs ONLY the nominal DoD path with optional --corpus loading
(synthetic when omitted). Both write to the same FSV dir and reuse the same DTOs.

Per TECH-PREDICTOR `<note category="phase2-dod-binary">`: the DoD-checking binary is
`bin/mejepa.rs`, NOT `examples/`. The two binaries share the underlying `MeJepaPredictor` API
+ `vicreg_loss` + `check_vram_steady_state` + the FSV writer helpers from TASK-PRED-011's
`fsv_writer.rs`.

Iron Rule observed: this CLI ONLY consumes APIs that have shipped (TASK-PRED-001..011). It
does not wire backward / optimizer.step (Phase 3), does not invoke the Oracle head's softmax
or any conformal calibration (Phase 4), does not register MCP tools (Phase 7).
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (testing_requirements; mcp_tool_namespace_lock — this binary registers NO MCP tools per the lock)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-PREDICTOR.md (REQ-PRED-15; AC-PRED-06-01..02; US-PRED-06)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-PREDICTOR.md (api_contracts bin_entrypoint mejepa; component_contracts MejepaTrainBin; implementation_notes "phase2-dod-binary")</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase2-predictor/TASK-PRED-011.md (FSV writer helpers + evidence DTOs + adversarial harness)</file>
  <file purpose="cli_pattern">crates/context-graph-mejepa-corpus/src/bin/mejepa-corpus.rs (sibling crate's CLI binary — pattern reference)</file>
  <file purpose="prior_files">crates/context-graph-mejepa/src/{lib.rs, fsv_writer.rs, evidence.rs}</file>
</input_context_files>

<prerequisites>
  <check>TASK-PRED-001..011 complete (full library + FSV harness shipped)</check>
  <check>cargo build --release -p context-graph-mejepa --features cuda succeeds on RTX 5090</check>
  <check>FSV harness `cargo run --example predictor_fsv` exits 0 with all evidence files</check>
  <check>`fsv_writer` module exposes `ensure_fsv_dir` + `write_evidence_file` + `read_evidence_file`</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/bin/mejepa.rs` with a `train` subcommand
    - Use `clap` (workspace dep) for argv parsing: `--dry-run` (bool), `--batches usize` (default 1), `--batch-size usize` (default 64), `--corpus PathBuf` (optional), `--output PathBuf` (optional override of FSV_DIR)
    - When `--dry-run`:
        1. ensure_fsv_dir + clear_fsv_dir
        2. Construct MeJepaPredictor (default config; FrozenTargetAdapter::empty_for_test; num_tests=30; Device::Cuda(0))
        3. Allocate panels: synthetic random when --corpus absent; first `batches * batch_size` from corpus when --corpus given (corpus loading path is gracefully fallible — a corpus-load failure logs warning + falls back to synthetic + sets `corpus_path: Some(...)`/`fallback_used: true` in evidence)
        4. **Warmup** 10 calls + **timing loop** `100 * batches` calls
        5. Compute p50/p99 latency
        6. Compute vicreg_loss(predicted, target=encode_target(synthetic_panel_t2))
        7. Probe VRAM (best-effort; if probe fails, fill VramReport with zeros and dod_pass_8gb=false)
        8. Write `forward-pass-evidence.json` + `vicreg-loss-evidence.json` to FSV_DIR (or --output dir if given)
        9. Re-read each file; assert: panel_dim==5120, output_finite==true, formula_check==true, l_total ∈ [10, 1000], vram_resident_bytes < 8 GB, p50 < 50.0
        10. Print structured stdout (one key=value per line):
            forward_latency_ms_p50=...
            forward_latency_ms_p99=...
            vram_resident_bytes=...
            l_predict=...
            l_variance=...
            l_covariance=...
            l_invariance=...
            l_total=...
            dod_pass=...
            corpus_path=... (or "synthetic")
        11. Exit 0 on dod_pass; exit 2 on first failing metric (with the failing metric on stderr); exit 3 on harness assertion failure (write/re-read mismatch)
    - When NOT --dry-run: print "training mode requires Phase 3 mejepa-train; this binary supports --dry-run only in Phase 2" and exit 64 (EX_USAGE)
    - 4 integration tests:
        (a) parse args produces correct TrainArgs
        (b) dry-run with batches=1 batch-size=8 (small batch for faster CI) succeeds when stub-mode is invoked  via a `--stub-device` test-only flag that constructs an in-memory predictor without cuda; gated by `#[cfg(test)]` only
        (c) without --dry-run flag: exit code 64 (EX_USAGE)
        (d) dry-run with non-existent --corpus: logs warning + falls back to synthetic + still completes
  </in_scope>
  <out_of_scope>
    - Backward + optimizer.step (Phase 3 mejepa-train crate)
    - Per-step training cert emission (Phase 3)
    - Oracle head softmax invocation (TASK-PRED-008 already implements; not wired into the dry-run path here since DoD focuses on predictor forward + VICReg only)
    - Adversarial cases (those live in TASK-PRED-011 examples/predictor_fsv.rs)
    - MCP tool registration (Phase 7; constitution mcp_tool_namespace_lock prohibits new tools here)
    - Conformal calibration (Phase 4)
    - Multi-GPU / distributed (single RTX 5090 only)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/bin/mejepa.rs">
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "mejepa", about = "ME-JEPA-Code Phase 2 DoD harness")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Train(TrainArgs),
}

#[derive(Debug, Parser)]
struct TrainArgs {
    #[arg(long)]
    dry_run: bool,

    #[arg(long, default_value_t = 1)]
    batches: usize,

    #[arg(long, default_value_t = 64)]
    batch_size: usize,

    #[arg(long)]
    corpus: Option&lt;PathBuf&gt;,

    #[arg(long)]
    output: Option&lt;PathBuf&gt;,
}

fn main() -&gt; std::process::ExitCode;

fn run_dry_run(args: &amp;TrainArgs) -&gt; Result&lt;DryRunReport, MejepaCliError&gt;;

#[derive(Debug)]
struct DryRunReport {
    forward_latency_ms_p50: f32,
    forward_latency_ms_p99: f32,
    vram_resident_bytes: u64,
    loss: context_graph_mejepa::LossOutputs,
    dod_pass: bool,
    corpus_path: Option&lt;String&gt;,
    fallback_used: bool,
    failing_metric: Option&lt;&amp;'static str&gt;,
}

#[derive(Debug, thiserror::Error)]
enum MejepaCliError {
    #[error("MEJEPA_CLI_USAGE: {0}")]
    Usage(String),
    #[error("MEJEPA_CLI_PREDICTOR: {0}")]
    Predictor(#[from] context_graph_mejepa::PredictorError),
    #[error("MEJEPA_CLI_LOSS: {0}")]
    Loss(#[from] context_graph_mejepa::LossError),
    #[error("MEJEPA_CLI_IO: {0}")]
    Io(#[from] std::io::Error),
    #[error("MEJEPA_CLI_ASSERTION: {0}")]
    Assertion(String),
}

impl MejepaCliError { fn code(&amp;self) -&gt; &amp;'static str; }
    </signature>
  </signatures>
  <constraints>
    - Binary name: `mejepa` (per TECH-PREDICTOR `bin_entrypoint name="mejepa"`)
    - Cargo.toml entry: `[[bin]] name = "mejepa" path = "src/bin/mejepa.rs"`
    - Subcommand: `train` (single subcommand for Phase 2; future phases will add `predict`, `verify`, etc.)
    - **Default args**: --batches=1, --batch-size=64 — matches the doc 09 §7 Phase 2 DoD invocation EXACTLY
    - **Without --dry-run**: print to stderr "training requires Phase 3 mejepa-train; Phase 2 only supports --dry-run" and `return ExitCode::from(64)` (EX_USAGE)
    - **DoD thresholds** (each is a hard exit-2 gate):
        1. forward_latency_ms_p50 &lt; 50.0
        2. l_total finite AND l_total ∈ [10.0, 1000.0]
        3. l_predict / l_variance / l_covariance / l_invariance all finite
        4. vram_resident_bytes &lt; VRAM_STEADY_STATE_TARGET_BYTES (8 GB)
        5. output_finite == true (no NaN/Inf in any forward output)
    - On FIRST failing metric: set DryRunReport.failing_metric = Some("forward_latency_ms_p50" | "l_total" | "vram" | "..."); print to stderr; exit 2
    - **Corpus path** (--corpus given but file/dir missing): log `tracing::warn!`, set `fallback_used = true`, proceed with synthetic random panels — this matches AC-PRED-06-01 spirit (FSV evidence still written even on metric failure)
    - **stdout format**: one `key=value` per line; final line `dod_pass=true|false`; no JSON to stdout (JSON to FSV files only)
    - **stderr**: only on failure paths; one line per failing metric
    - **Exit codes**:
        0 = dod_pass
        2 = nominal metric failed (e.g. p50 ≥ 50ms, or l_total NaN)
        3 = harness assertion failed (FSV write→re-read mismatch; should never happen in nominal flow)
        64 = EX_USAGE (e.g. --dry-run absent)
        65 = EX_DATAERR (e.g. --corpus content malformed AND fallback also failed)
        70 = EX_SOFTWARE (predictor instantiation failure or other PredictorError)
    - **No MCP tool registration** — constitution `<mcp_tool_namespace_lock>` permits only `mcp__cgreality__mejepa_*` and only in Phase 7. This binary registers nothing.
    - File length stays &lt; 500 lines (split argv parsing helpers if approaching)
    - No `unwrap()` outside test code
  </constraints>
  <verification>
    - cargo build --release -p context-graph-mejepa --bin mejepa --features cuda succeeds with zero warnings
    - cargo test -p context-graph-mejepa shows >= 69 tests passing (this task adds 4)
    - cargo clippy -p context-graph-mejepa -- -D warnings clean
    - On RTX 5090: `cargo run --release -p context-graph-mejepa --features cuda --bin mejepa -- train --dry-run --batches 1 --batch-size 64` exits 0 with `dod_pass=true` on stdout
    - FSV files at /tmp/contextgraph-mejepa-predictor-fsv/{forward-pass-evidence.json, vicreg-loss-evidence.json} exist (mode 0o600 unix), parseable, contain finite metrics
    - `cargo run --bin mejepa -- train` (no --dry-run) → exit 64 with "training requires Phase 3 mejepa-train" on stderr
    - `cargo run --bin mejepa -- train --dry-run --corpus /nonexistent.json` → fallback warning + Ok exit
    - stdout matches the structured contract: forward_latency_ms_p50=, _p99=, vram_resident_bytes=, l_predict=, l_variance=, l_covariance=, l_invariance=, l_total=, dod_pass=, corpus_path=
  </verification>
</definition_of_done>

<pseudo_code>
fn main() -&gt; ExitCode {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();
    match cli.command {
        Command::Train(args) =&gt; {
            if !args.dry_run {
                eprintln!("MEJEPA_CLI_USAGE: training requires Phase 3 mejepa-train; Phase 2 only supports --dry-run");
                return ExitCode::from(64);
            }
            match run_dry_run(&args) {
                Ok(report) =&gt; {
                    print_report(&report);
                    if report.dod_pass { ExitCode::SUCCESS }
                    else {
                        if let Some(metric) = &report.failing_metric {
                            eprintln!("MEJEPA_CLI_DOD_FAILED: failing_metric={}", metric);
                        }
                        ExitCode::from(2)
                    }
                }
                Err(e) =&gt; {
                    eprintln!("{}: {}", e.code(), e);
                    ExitCode::from(match e.code() {
                        "MEJEPA_CLI_USAGE" =&gt; 64,
                        "MEJEPA_CLI_IO" =&gt; 65,
                        "MEJEPA_CLI_ASSERTION" =&gt; 3,
                        _ =&gt; 70,
                    })
                }
            }
        }
    }
}

fn run_dry_run(args: &TrainArgs) -&gt; Result&lt;DryRunReport, MejepaCliError&gt; {
    let fsv_dir = args.output.clone().unwrap_or_else(|| PathBuf::from(FSV_DIR));
    fs::create_dir_all(&fsv_dir)?;
    clear_json_in_dir(&fsv_dir)?;

    let device = Device::cuda_if_available(0).map_err(|e| PredictorError::DeviceUnavailable { detail: format!("{e}") })?;
    let cfg = PredictorConfig::default();
    let adapter = FrozenTargetAdapter::empty_for_test();
    let predictor = MeJepaPredictor::new(cfg, adapter, device.clone(), 30)?;

    // 1. Resolve panels
    let (panel_t0, panel_t1, panel_t2_target_bytes, corpus_path, fallback_used) = match &args.corpus {
        Some(p) if p.exists() =&gt; load_corpus_first_n(p, args.batches * args.batch_size).unwrap_or_else(|e| {
            tracing::warn!(target: "mejepa::cli", "corpus load failed: {e}; falling back to synthetic");
            (synthetic_panels(args.batch_size, &device)?, true)
        }),
        _ =&gt; {
            if args.corpus.is_some() { tracing::warn!("--corpus path missing; synthetic fallback"); }
            (synthetic_panels(args.batch_size, &device)?, args.corpus.is_some())
        }
    };
    // (synthetic_panels returns (panel_t0, panel_t1, target_panel_bytes); details elided)

    // 2. Warmup + timing
    let mut latencies = Vec::with_capacity(100 * args.batches);
    for _ in 0..10 { let _ = predictor.forward(&panel_t0, &panel_t1)?; }
    for _ in 0..(100 * args.batches) {
        let t = Instant::now();
        let _ = predictor.forward(&panel_t0, &panel_t1)?;
        latencies.push(t.elapsed().as_secs_f32() * 1000.0);
    }
    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p50 = latencies[latencies.len() / 2];
    let p99 = latencies[(latencies.len() as f32 * 0.99) as usize];

    // 3. VICReg
    let last_predicted = predictor.forward(&panel_t0, &panel_t1)?;
    let target = ...synthesize_target_from_bytes(panel_t2_target_bytes, &device)...;
    let lambdas = VicregLambdas::default();
    let loss = vicreg_loss(&last_predicted.data, &target, lambdas)?;

    // 4. VRAM
    let vram = match check_vram_steady_state(&device) {
        Ok(v) =&gt; v,
        Err(e) =&gt; {
            tracing::warn!(target: "mejepa::cli", "vram probe failed: {e}; recording zeros");
            VramReport { ..Default::default() }
        }
    };

    // 5. Build evidence + dod_pass
    let l_total_in_range = (10.0..=1000.0).contains(&loss.l_total) && loss.l_total.is_finite();
    let p50_pass = p50 &lt; 50.0;
    let vram_pass = vram.dod_pass_8gb;
    let formula_check = (loss.l_total - (loss.l_predict + 25.0*loss.l_variance + loss.l_covariance + 25.0*loss.l_invariance)).abs() &lt; 1e-3;
    let dod_pass = p50_pass && l_total_in_range && vram_pass && loss.is_finite_decomposition() && formula_check;
    let failing_metric = if !p50_pass { Some("forward_latency_ms_p50") }
        else if !l_total_in_range { Some("l_total") }
        else if !vram_pass { Some("vram_resident_bytes") }
        else if !loss.is_finite_decomposition() { Some("loss_components_finite") }
        else if !formula_check { Some("formula_check") }
        else { None };

    let fwd_evidence = ForwardPassEvidence { ... };
    let vicreg_evidence = VicregLossEvidence { ... };
    write_evidence_file(&fsv_dir.join("forward-pass-evidence.json"), &fwd_evidence)?;
    write_evidence_file(&fsv_dir.join("vicreg-loss-evidence.json"), &vicreg_evidence)?;

    // 6. Re-read + assert
    let re_fwd: ForwardPassEvidence = read_evidence_file(&fsv_dir.join("forward-pass-evidence.json"))?;
    if re_fwd.panel_dim != PANEL_DIM {
        return Err(MejepaCliError::Assertion("forward-pass-evidence.json round-trip altered panel_dim".to_string()));
    }
    let re_vic: VicregLossEvidence = read_evidence_file(&fsv_dir.join("vicreg-loss-evidence.json"))?;
    if (re_vic.l_total - vicreg_evidence.l_total).abs() &gt; 1e-3 {
        return Err(MejepaCliError::Assertion("vicreg-loss-evidence.json round-trip altered l_total".to_string()));
    }

    Ok(DryRunReport {
        forward_latency_ms_p50: p50,
        forward_latency_ms_p99: p99,
        vram_resident_bytes: vram.resident_bytes,
        loss,
        dod_pass,
        corpus_path: corpus_path.map(|p| p.display().to_string()),
        fallback_used,
        failing_metric,
    })
}

fn print_report(r: &DryRunReport) {
    println!("forward_latency_ms_p50={:.3}", r.forward_latency_ms_p50);
    println!("forward_latency_ms_p99={:.3}", r.forward_latency_ms_p99);
    println!("vram_resident_bytes={}", r.vram_resident_bytes);
    println!("l_predict={}", r.loss.l_predict);
    println!("l_variance={}", r.loss.l_variance);
    println!("l_covariance={}", r.loss.l_covariance);
    println!("l_invariance={}", r.loss.l_invariance);
    println!("l_total={}", r.loss.l_total);
    println!("dod_pass={}", r.dod_pass);
    println!("corpus_path={}", r.corpus_path.as_deref().unwrap_or("synthetic"));
    println!("fallback_used={}", r.fallback_used);
}

tests:
    test_parse_args_train_dry_run_defaults_match_dod
    test_main_without_dry_run_returns_64
    test_run_dry_run_with_nonexistent_corpus_falls_back_synthetic
    test_run_dry_run_synthetic_succeeds_when_thresholds_met (cuda-gated; mirrors example FSV but only nominal path)
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/bin/mejepa.rs">CLI binary: clap argv + train --dry-run subcommand + DryRunReport + write/re-read/assert + structured stdout + 4 tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/Cargo.toml">Add `[[bin]] name = "mejepa" path = "src/bin/mejepa.rs"`; add `clap = { workspace = true, features = ["derive"] }` and `tracing-subscriber = { workspace = true }` if not already in workspace deps</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa --bin mejepa --features cuda succeeds with zero warnings</criterion>
  <criterion>cargo build --release -p context-graph-mejepa --bin mejepa (no cuda) succeeds (CPU-host build path)</criterion>
  <criterion>cargo test -p context-graph-mejepa shows >= 69 tests passing</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings is clean</criterion>
  <criterion>On RTX 5090: `cargo run --release -p context-graph-mejepa --features cuda --bin mejepa -- train --dry-run --batches 1 --batch-size 64` exits 0 (REQ-PRED-15, AC-PRED-06-01)</criterion>
  <criterion>stdout contains all 11 keys: forward_latency_ms_p50, _p99, vram_resident_bytes, l_predict, l_variance, l_covariance, l_invariance, l_total, dod_pass, corpus_path, fallback_used</criterion>
  <criterion>FSV files at /tmp/contextgraph-mejepa-predictor-fsv/ are written + re-read + asserted before exit</criterion>
  <criterion>Without --dry-run flag: exit 64 (EX_USAGE) with stderr "training requires Phase 3 mejepa-train"</criterion>
  <criterion>--corpus pointing at nonexistent file: warns + falls back to synthetic + exits Ok</criterion>
  <criterion>DoD thresholds: forward_latency_ms_p50 &lt; 50, l_total ∈ [10, 1000], vram_resident_bytes &lt; 8 GB, all loss components finite</criterion>
  <criterion>On any failing threshold: exit 2 with first-failing-metric on stderr; FSV files still written so operator can inspect (AC-PRED-06-02)</criterion>
  <criterion>No MCP tool registered (constitution mcp_tool_namespace_lock honored)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa --bin mejepa</command>
  <command>cargo build --release -p context-graph-mejepa --bin mejepa --features cuda</command>
  <command>cargo test -p context-graph-mejepa</command>
  <command>CUBLAS_WORKSPACE_CONFIG=:4096:8 cargo run --release -p context-graph-mejepa --features cuda --bin mejepa -- train --dry-run --batches 1 --batch-size 64</command>
  <command>cargo run --release -p context-graph-mejepa --bin mejepa -- train</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```

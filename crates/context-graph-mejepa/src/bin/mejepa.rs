use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use candle_core::{DType, Device};
use clap::{Parser, Subcommand};
use context_graph_mejepa::fsv_writer;
use context_graph_mejepa::heal::{run_heal_drill, HealDrillArgs, InjectDrift, FSV_ROOT};
use context_graph_mejepa::panel_source::{self, PanelBatchView};
use context_graph_mejepa::{
    build_deterministic_compiler, open_infer_rocksdb, CalibrationStore, ForwardPassEvidence,
    FrozenTargetAdapter, InferTestArgs, LossOutputs, MeJepaPredictor, PredictorConfig,
    RocksDbInferStore, TargetProvenance, VicregLambdas, VicregLossEvidence, PANEL_DIM,
    VRAM_STEADY_STATE_TARGET_BYTES,
};
use thiserror::Error;

#[derive(Debug, Parser)]
#[command(name = "mejepa", about = "ME-JEPA-Code Phase 2 DoD harness")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Train(TrainArgs),
    InferTest(InferTestArgs),
    HealDrill(HealDrillCliArgs),
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
    corpus: Option<PathBuf>,
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Debug, Parser)]
struct HealDrillCliArgs {
    #[arg(long, value_enum, default_value = "hard")]
    inject_drift: InjectDrift,
    #[arg(long, default_value = FSV_ROOT)]
    output_fsv: PathBuf,
    #[arg(long, default_value_t = 5000)]
    max_observations: u64,
    #[arg(long, default_value_t = 42)]
    seed: u64,
    #[arg(long, default_value_t = 60)]
    rtx_5090_budget_min: u64,
}

#[derive(Debug)]
struct DryRunReport {
    forward_latency_ms_p50: f32,
    forward_latency_ms_p99: f32,
    vram_resident_bytes: u64,
    loss: LossOutputs,
    dod_pass: bool,
    corpus_path: String,
    failing_metric: Option<&'static str>,
}

#[derive(Debug, Error)]
enum MejepaCliError {
    #[error("MEJEPA_CLI_USAGE: {0}")]
    Usage(String),
    #[error("MEJEPA_CLI_PREDICTOR: {0}")]
    Predictor(#[from] context_graph_mejepa::PredictorError),
    #[error("MEJEPA_CLI_LOSS: {0}")]
    Loss(#[from] context_graph_mejepa::LossError),
    #[error("MEJEPA_CLI_CANDLE: {0}")]
    Candle(#[from] candle_core::Error),
    #[error("MEJEPA_CLI_IO: {0}")]
    Io(#[from] std::io::Error),
    #[error("MEJEPA_CLI_ASSERTION: {0}")]
    Assertion(String),
}

fn main() -> ExitCode {
    let _ = tracing_subscriber::fmt::try_init();
    let cli = Cli::parse();
    match cli.command {
        Command::Train(args) if !args.dry_run => {
            eprintln!("training requires Phase 3 mejepa-train; Phase 2 only supports --dry-run");
            ExitCode::from(64)
        }
        Command::InferTest(args) => match run_infer_test_command(args) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("{err}");
                ExitCode::from(4)
            }
        },
        Command::HealDrill(args) => match run_heal_drill(HealDrillArgs {
            inject_drift: args.inject_drift,
            output_fsv: args.output_fsv,
            max_observations: args.max_observations,
            seed: args.seed,
            rtx_5090_budget_min: args.rtx_5090_budget_min,
        }) {
            Ok(summary) => {
                println!("{}", serde_json::to_string_pretty(&summary).unwrap());
                ExitCode::from(summary.exit_code as u8)
            }
            Err(err) => {
                err.log_context(file!());
                eprintln!("{}: {err}", err.code());
                ExitCode::from(1)
            }
        },
        Command::Train(args) => match run_dry_run(&args) {
            Ok(report) => {
                println!("forward_latency_ms_p50={}", report.forward_latency_ms_p50);
                println!("forward_latency_ms_p99={}", report.forward_latency_ms_p99);
                println!("vram_resident_bytes={}", report.vram_resident_bytes);
                println!("l_predict={}", report.loss.l_predict);
                println!("l_variance={}", report.loss.l_variance);
                println!("l_covariance={}", report.loss.l_covariance);
                println!("l_invariance={}", report.loss.l_invariance);
                println!("l_total={}", report.loss.l_total);
                println!("corpus_path={}", report.corpus_path);
                println!("dod_pass={}", report.dod_pass);
                if let Some(metric) = report.failing_metric {
                    eprintln!("first_failing_metric={metric}");
                    ExitCode::from(2)
                } else {
                    ExitCode::SUCCESS
                }
            }
            Err(MejepaCliError::Usage(err)) => {
                eprintln!("{err}");
                ExitCode::from(64)
            }
            Err(MejepaCliError::Assertion(err)) => {
                eprintln!("{err}");
                ExitCode::from(3)
            }
            Err(err) => {
                eprintln!("{err}");
                ExitCode::from(70)
            }
        },
    }
}

fn run_infer_test_command(args: InferTestArgs) -> Result<ExitCode, MejepaCliError> {
    let db_path = args.output_fsv.join("infer-rocksdb");
    let repo_root = args.output_fsv.join("infer-test-repo");
    let db = open_infer_rocksdb(&db_path).map_err(|err| {
        MejepaCliError::Assertion(format!("failed to open inference RocksDB: {err}"))
    })?;
    let calibration = CalibrationStore::new(db.clone(), 30).map_err(|err| {
        MejepaCliError::Assertion(format!("failed to initialize calibration store: {err}"))
    })?;
    let store = std::sync::Arc::new(RocksDbInferStore::new(db));
    let compiler = build_deterministic_compiler(
        repo_root,
        store,
        calibration.clone(),
        context_graph_mejepa::MeJepaInferConfig::default(),
    )
    .map_err(|err| MejepaCliError::Assertion(format!("failed to initialize compiler: {err}")))?;
    let report = match context_graph_mejepa::run_infer_test(args, &compiler, &calibration) {
        Ok(report) => report,
        Err(context_graph_mejepa::MejepaInferError::CalibrationStale { .. }) => {
            eprintln!("MEJEPA_INFER_CALIBRATION_STALE");
            return Ok(ExitCode::from(2));
        }
        Err(context_graph_mejepa::MejepaInferError::ConformalInsufficientSamples { .. }) => {
            eprintln!("MEJEPA_INFER_CONFORMAL_INSUFFICIENT_SAMPLES");
            return Ok(ExitCode::from(3));
        }
        Err(err) => {
            return Err(MejepaCliError::Assertion(format!(
                "infer-test failed: {err}"
            )))
        }
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&report)
            .map_err(context_graph_mejepa::PredictorError::from)?
    );
    if context_graph_mejepa::dod_satisfied(&report) {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::from(1))
    }
}

fn run_dry_run(args: &TrainArgs) -> Result<DryRunReport, MejepaCliError> {
    if args.batches == 0 || args.batch_size == 0 {
        return Err(MejepaCliError::Usage(
            "batches and batch-size must both be >= 1".to_string(),
        ));
    }
    let output = args
        .output
        .clone()
        .unwrap_or_else(|| PathBuf::from(fsv_writer::FSV_DIR));
    clear_nominal_evidence_files(&output)?;
    let panel_path = args
        .corpus
        .clone()
        .unwrap_or_else(|| PathBuf::from(panel_source::DEFAULT_PHASE1_PANEL_PATH));
    let panel = panel_source::read_phase1_panel(&panel_path)?;
    let device = Device::new_cuda(0)?;
    let adapter = FrozenTargetAdapter::new(TargetProvenance::new(
        "phase2-cli-real-panel",
        BTreeMap::from([(
            "phase1-instruments".to_string(),
            "real-panel-json".to_string(),
        )]),
        0,
        Some(context_graph_mejepa_instruments::hash_f32s(panel.data())),
    ));
    let predictor = MeJepaPredictor::new(PredictorConfig::default(), adapter, device.clone(), 30)?;
    let panel_t0 = panel_source::panel_batch_tensor(
        &panel,
        args.batch_size,
        PanelBatchView::T0,
        &device,
        DType::BF16,
    )?;
    let panel_t1 = panel_source::panel_batch_tensor(
        &panel,
        args.batch_size,
        PanelBatchView::T1,
        &device,
        DType::BF16,
    )?;
    let panel_t2 = panel_source::panel_batch_tensor(
        &panel,
        args.batch_size,
        PanelBatchView::T2,
        &device,
        DType::BF16,
    )?;

    let dryrun = predictor.forward_dryrun(&panel_t0, &panel_t1)?;
    let output_sha256 = panel_source::tensor_sha256_f32(&dryrun.tensor)?;
    let mut latencies = Vec::with_capacity(100 * args.batches);
    for _ in 0..10 {
        let _ = predictor.forward(&panel_t0, &panel_t1)?;
        predictor.device().synchronize()?;
    }
    for _ in 0..(100 * args.batches) {
        let started = Instant::now();
        let _ = predictor.forward(&panel_t0, &panel_t1)?;
        predictor.device().synchronize()?;
        latencies.push(started.elapsed().as_secs_f64() as f32 * 1_000.0);
    }
    latencies.sort_by(|a, b| a.total_cmp(b));
    let p50 = latencies[latencies.len() / 2];
    let p99 = latencies[latencies.len().saturating_sub(2)];
    let loss =
        context_graph_mejepa::vicreg_loss(&dryrun.tensor, &panel_t2, VicregLambdas::default())?;
    let vram_resident_bytes = context_graph_mejepa::vram_resident_bytes(predictor.device())?;

    let forward = ForwardPassEvidence {
        source_of_truth: output
            .join("forward-pass-evidence.json")
            .display()
            .to_string(),
        input_panel_path: panel_path.display().to_string(),
        panel_dim: PANEL_DIM,
        batch_size: args.batch_size,
        warmup_calls: 10,
        measured_calls: 100 * args.batches,
        forward_latency_ms_p50: p50,
        forward_latency_ms_p99: p99,
        output_sha256_f32: output_sha256,
        output_finite: true,
        output_dtype: dryrun.dtype,
        vram_resident_bytes,
        dod_pass: p50 < 50.0 && vram_resident_bytes < VRAM_STEADY_STATE_TARGET_BYTES,
    };
    let total_in_dod_band = loss.l_total >= 10.0 && loss.l_total <= 1_000.0;
    let loss_evidence = VicregLossEvidence {
        source_of_truth: output
            .join("vicreg-loss-evidence.json")
            .display()
            .to_string(),
        input_panel_path: panel_path.display().to_string(),
        lambdas: VicregLambdas::default(),
        finite: loss.finite(),
        total_in_dod_band,
        dod_pass: loss.finite() && loss.formula_check && total_in_dod_band,
        outputs: loss.clone(),
    };
    let forward_readback =
        fsv_writer::write_readback_assert(&output.join("forward-pass-evidence.json"), &forward)?;
    let loss_readback = fsv_writer::write_readback_assert(
        &output.join("vicreg-loss-evidence.json"),
        &loss_evidence,
    )?;
    if forward_readback.panel_dim != PANEL_DIM || loss_readback.outputs != loss {
        return Err(MejepaCliError::Assertion(
            "FSV evidence readback did not match in-memory source state".to_string(),
        ));
    }
    let failing_metric = first_failing_metric(&forward_readback, &loss_readback);
    Ok(DryRunReport {
        forward_latency_ms_p50: p50,
        forward_latency_ms_p99: p99,
        vram_resident_bytes,
        loss,
        dod_pass: failing_metric.is_none(),
        corpus_path: panel_path.display().to_string(),
        failing_metric,
    })
}

fn first_failing_metric(
    forward: &ForwardPassEvidence,
    loss: &VicregLossEvidence,
) -> Option<&'static str> {
    if forward.forward_latency_ms_p50 >= 50.0 {
        return Some("forward_latency_ms_p50");
    }
    if !forward.output_finite {
        return Some("output_finite");
    }
    if forward.vram_resident_bytes >= VRAM_STEADY_STATE_TARGET_BYTES {
        return Some("vram_resident_bytes");
    }
    if !loss.finite {
        return Some("loss_finite");
    }
    if !loss.outputs.formula_check {
        return Some("vicreg_formula_check");
    }
    if !loss.total_in_dod_band {
        return Some("l_total");
    }
    None
}

fn clear_nominal_evidence_files(path: &std::path::Path) -> Result<(), std::io::Error> {
    fs::create_dir_all(path)?;
    for file_name in ["forward-pass-evidence.json", "vicreg-loss-evidence.json"] {
        let candidate = path.join(file_name);
        if candidate.exists() {
            fs::remove_file(candidate)?;
        }
    }
    Ok(())
}

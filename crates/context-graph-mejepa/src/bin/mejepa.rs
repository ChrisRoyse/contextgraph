use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Instant, SystemTime};

use candle_core::{DType, Device};
use clap::{Parser, Subcommand};
use context_graph_mejepa::eval::{
    build_eval_compiler, build_patch_similarity_graph, corpus_sha_from_holdout, synthetic_holdout,
    synthetic_patch_embeddings, write_weekly_report, EvalConfig, EvalProvenance, EvalRunner,
};
use context_graph_mejepa::fsv_writer;
use context_graph_mejepa::heal::{run_heal_drill, HealDrillArgs, InjectDrift, FSV_ROOT};
use context_graph_mejepa::panel_source::{self, PanelBatchView};
use context_graph_mejepa::{
    build_fixture_deterministic_compiler, open_infer_rocksdb, CalibrationStore,
    ForwardPassEvidence, FrozenTargetAdapter, InferTestArgs, LossOutputs, MeJepaPredictor,
    PredictorConfig, RocksDbInferStore, TargetProvenance, VicregLambdas, VicregLossEvidence,
    PANEL_DIM, VRAM_STEADY_STATE_TARGET_BYTES,
};
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use std::sync::Arc;
use thiserror::Error;

#[path = "mejepa/stress_cli.rs"]
mod stress_cli;

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
    Verify(VerifyArgs),
    Eval(EvalArgs),
    EvalRun(EvalRunArgs),
    EvalBuildGraph(EvalBuildGraphArgs),
    Constellation(ConstellationArgs),
    HealDrill(HealDrillCliArgs),
    Stress(StressArgs),
    StressTest(StressTestArgs),
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

#[derive(Debug, Parser)]
struct VerifyArgs {
    #[arg(long)]
    task: String,
    #[arg(long)]
    patch: PathBuf,
    #[arg(long, default_value = "./data/mejepa-corpus/")]
    corpus_root: PathBuf,
    #[arg(long, default_value = "/tmp/contextgraph-mejepa-shift-subscriber-fsv/")]
    evidence_dir: PathBuf,
    #[arg(long, default_value_t = 0.05)]
    holdout_sample_pct: f32,
}

#[derive(Debug, Parser)]
struct EvalArgs {
    #[command(subcommand)]
    command: EvalCommand,
}

#[derive(Debug, Subcommand)]
enum EvalCommand {
    SyntheticStress(SyntheticStressCliArgs),
}

#[derive(Debug, Parser)]
struct SyntheticStressCliArgs {
    #[arg(long, default_value = context_graph_mejepa::SYNTHETIC_STRESS_ROOT)]
    corpus_root: PathBuf,
    #[arg(long)]
    results_db_path: Option<PathBuf>,
    #[arg(long)]
    project_id_prefix: Option<String>,
    #[arg(long)]
    overwrite_corpus: bool,
    #[arg(long)]
    no_materialize_corpus: bool,
}

#[derive(Debug, Parser)]
struct EvalRunArgs {
    #[arg(long, default_value = "/tmp/contextgraph-mejepa-phase8-eval/rocksdb")]
    db_path: PathBuf,
    #[arg(long, default_value = "/tmp/contextgraph-mejepa-phase8-eval/repo")]
    repo_root: PathBuf,
    #[arg(long, default_value = "/tmp/contextgraph-mejepa-phase8-eval")]
    output_fsv: PathBuf,
    #[arg(long, default_value = "2026-05-11")]
    report_date: String,
}

#[derive(Debug, Parser)]
struct EvalBuildGraphArgs {
    #[arg(long, default_value = "/tmp/contextgraph-mejepa-phase8-eval/rocksdb")]
    db_path: PathBuf,
    #[arg(long, default_value = "/tmp/contextgraph-mejepa-phase8-eval")]
    output_fsv: PathBuf,
    #[arg(long, default_value_t = 0.85)]
    threshold: f32,
    #[arg(long, default_value_t = 3)]
    top_k: usize,
}

#[derive(Debug, Parser)]
struct ConstellationArgs {
    #[command(subcommand)]
    command: ConstellationCommand,
}

#[derive(Debug, Subcommand)]
enum ConstellationCommand {
    FreshnessAudit(ConstellationFreshnessAuditArgs),
}

#[derive(Debug, Parser)]
struct ConstellationFreshnessAuditArgs {
    #[arg(long)]
    db_path: PathBuf,
    #[arg(
        long,
        default_value = "/zfs/archive/contextgraph/exports/constellation-freshness-audit"
    )]
    output_root: PathBuf,
    #[arg(long, default_value_t = 10_000)]
    refresh_log_limit: usize,
}

#[derive(Debug, Parser)]
struct StressArgs {
    #[arg(long, default_value = "/tmp/contextgraph-mejepa-hygiene-stress-fsv")]
    output_fsv: PathBuf,
    #[arg(long, default_value_t = 10_000)]
    entries: u64,
    #[arg(long, default_value_t = 7)]
    gc_passes: u32,
}

#[derive(Debug, Parser)]
struct StressTestArgs {
    #[arg(long, default_value = "tiny")]
    size: String,
    #[arg(long)]
    run_id: Option<String>,
    #[arg(long, default_value_t = 72)]
    seed: u64,
    #[arg(long, default_value = context_graph_mejepa::STRESS_TRACE_ROOT)]
    trace_root: PathBuf,
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
    #[error("MEJEPA_CLI_INFER: {0}")]
    Infer(#[from] context_graph_mejepa::MejepaInferError),
    #[error("MEJEPA_CLI_TCT: {0}")]
    Tct(#[from] context_graph_mejepa_tct::TctError),
    #[error("MEJEPA_CLI_JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("MEJEPA_CLI_ROCKSDB: {0}")]
    RocksDb(#[from] rocksdb::Error),
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
        Command::Verify(args) => context_graph_mejepa::verify_cli::run_verify(
            &args.task,
            &args.patch,
            &args.corpus_root,
            &args.evidence_dir,
            args.holdout_sample_pct,
        ),
        Command::Eval(args) => match run_eval_command(args) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("{}: {err}", err.code());
                ExitCode::from(2)
            }
        },
        Command::EvalRun(args) => match run_eval_run(args) {
            Ok(code) => code,
            Err(err) => {
                err.log_context(file!());
                eprintln!("{}: {err}", err.code());
                ExitCode::from(2)
            }
        },
        Command::EvalBuildGraph(args) => match run_eval_build_graph(args) {
            Ok(code) => code,
            Err(err) => {
                err.log_context(file!());
                eprintln!("{}: {err}", err.code());
                ExitCode::from(2)
            }
        },
        Command::Constellation(args) => match run_constellation_command(args) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("{err}");
                ExitCode::from(2)
            }
        },
        Command::HealDrill(args) => match run_heal_drill(HealDrillArgs {
            inject_drift: args.inject_drift,
            output_fsv: args.output_fsv,
            max_observations: args.max_observations,
            seed: args.seed,
            rtx_5090_budget_min: args.rtx_5090_budget_min,
        }) {
            Ok(summary) => match serde_json::to_string_pretty(&summary) {
                Ok(json) => {
                    println!("{json}");
                    ExitCode::from(summary.exit_code as u8)
                }
                Err(err) => {
                    eprintln!("MEJEPA_HEAL_DRILL_JSON: {err}");
                    ExitCode::from(1)
                }
            },
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
        Command::Stress(args) => match stress_cli::run_stress(args) {
            Ok(value) => match serde_json::to_string_pretty(&value) {
                Ok(json) => {
                    println!("{json}");
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("MEJEPA_HYGIENE_STRESS_JSON: {err}");
                    ExitCode::from(1)
                }
            },
            Err(err) => {
                err.log_context(file!());
                eprintln!("{}: {err}", err.code);
                ExitCode::from(1)
            }
        },
        Command::StressTest(args) => match run_stress_test_command(args) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("{}: {err}", err.code());
                ExitCode::from(1)
            }
        },
    }
}

fn run_eval_command(
    args: EvalArgs,
) -> Result<ExitCode, context_graph_mejepa::SyntheticStressError> {
    match args.command {
        EvalCommand::SyntheticStress(args) => run_synthetic_stress_command(args),
    }
}

fn run_synthetic_stress_command(
    args: SyntheticStressCliArgs,
) -> Result<ExitCode, context_graph_mejepa::SyntheticStressError> {
    let results_db_path = args
        .results_db_path
        .unwrap_or_else(|| PathBuf::from(context_graph_mejepa::SYNTHETIC_STRESS_RESULTS_DB));
    let project_id_prefix = args
        .project_id_prefix
        .unwrap_or_else(|| format!("task-py-g-071-{}", chrono::Utc::now().timestamp_millis()));
    let report = context_graph_mejepa::run_synthetic_stress_eval(
        context_graph_mejepa::SyntheticStressEvalRequest {
            corpus_root: args.corpus_root,
            results_db_path,
            project_id_prefix,
            materialize_corpus: !args.no_materialize_corpus,
            overwrite_corpus: args.overwrite_corpus,
        },
    )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|source| {
            context_graph_mejepa::SyntheticStressError::Json {
                path: "stdout".to_string(),
                source,
            }
        })?
    );
    Ok(if report.ship_gate_passed {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn run_stress_test_command(
    args: StressTestArgs,
) -> Result<ExitCode, context_graph_mejepa::ProjectStressError> {
    let size = context_graph_mejepa::ProjectStressSize::parse(&args.size)?;
    let run =
        context_graph_mejepa::run_project_stress(context_graph_mejepa::ProjectStressRequest {
            size,
            run_id: args.run_id,
            seed: args.seed,
            trace_root: args.trace_root,
            fault: context_graph_mejepa::StressFaultInjection::None,
        })?;
    let json = serde_json::to_string_pretty(&run).map_err(|source| {
        context_graph_mejepa::ProjectStressError::Json {
            path: "stdout".to_string(),
            source,
        }
    })?;
    println!("{json}");
    Ok(if run.acceptance_passed {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn run_constellation_command(args: ConstellationArgs) -> Result<ExitCode, MejepaCliError> {
    match args.command {
        ConstellationCommand::FreshnessAudit(args) => run_constellation_freshness_audit(args),
    }
}

fn run_constellation_freshness_audit(
    args: ConstellationFreshnessAuditArgs,
) -> Result<ExitCode, MejepaCliError> {
    if args.refresh_log_limit == 0 {
        return Err(MejepaCliError::Usage(
            "--refresh-log-limit must be greater than zero".to_string(),
        ));
    }
    let db = open_runtime_rocksdb(&args.db_path)?;
    let store = context_graph_mejepa_tct::ConstellationStore::new(db)?;
    let latest_version = store.latest_version()?;
    let constellation = store.load_without_runtime_checks(latest_version)?;
    let log_entries = store.load_refresh_log_entries(args.refresh_log_limit)?;
    let overrides =
        context_graph_mejepa_tct::overrides_from_refresh_log(latest_version, &log_entries);
    let cells =
        context_graph_mejepa_tct::materialize_constellation_cells(&constellation, &overrides)?;
    let (max_age_days, _allow_stale) = context_graph_mejepa_tct::read_freshness_config()?;
    let report = context_graph_mejepa_tct::build_freshness_audit(
        latest_version,
        &cells,
        SystemTime::now(),
        context_graph_mejepa_tct::RefreshPolicyConfig {
            max_age_days,
            ..context_graph_mejepa_tct::RefreshPolicyConfig::default()
        },
    )?;
    let run_dir = args
        .output_root
        .join(format!("run-{}", chrono::Utc::now().timestamp_millis()));
    fs::create_dir_all(&run_dir)?;
    let report_path = run_dir.join("constellation_freshness_audit.json");
    write_json_0600(&report_path, &report)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "dbPath": args.db_path,
            "reportPath": report_path,
            "constellationVersionHex": hex::encode(latest_version),
            "totalCells": report.total_cells,
            "refitRequiredCount": report.refit_required_count,
            "skipCount": report.skip_count,
            "failedCellCount": report.failed_cell_count,
            "histogram": report.histogram,
            "sourceOfTruth": {
                "constellationCf": context_graph_mejepa_cf::CF_MEJEPA_CONSTELLATION,
                "refreshLogCf": context_graph_mejepa_cf::CF_MEJEPA_CONSTELLATION_REFRESH_LOG,
            }
        }))?
    );
    Ok(ExitCode::SUCCESS)
}

fn open_runtime_rocksdb(path: &std::path::Path) -> Result<Arc<DB>, MejepaCliError> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    opts.set_paranoid_checks(true);
    let descriptors = context_graph_mejepa_cf::all_hygiene_referenced_cfs()
        .into_iter()
        .map(|cf| ColumnFamilyDescriptor::new(cf, Options::default()))
        .collect::<Vec<_>>();
    Ok(Arc::new(DB::open_cf_descriptors(&opts, path, descriptors)?))
}

fn write_json_0600<T: serde::Serialize>(
    path: &std::path::Path,
    value: &T,
) -> Result<(), MejepaCliError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(&bytes)?;
    file.write_all(b"\n")?;
    Ok(())
}

fn run_eval_run(args: EvalRunArgs) -> Result<ExitCode, context_graph_mejepa::EvalError> {
    let (_db, compiler, eval_store, calibration) =
        build_eval_compiler(&args.db_path, &args.repo_root)?;
    let holdout = synthetic_holdout(&args.repo_root)?;
    let provenance = EvalProvenance {
        corpus_sha: corpus_sha_from_holdout(&holdout),
        eval_code_version: env!("CARGO_PKG_VERSION").to_string(),
        calibration_version: calibration.load_active()?.version.clone(),
        generated_by: "mejepa eval-run".to_string(),
    };
    let runner = EvalRunner::new(compiler, eval_store.clone(), EvalConfig::default())?;
    let report = runner.run_holdout_eval(&holdout, &[], args.report_date, provenance)?;
    let report_path = args.output_fsv.join("weekly-eval-report.json");
    write_weekly_report(&report_path, &report)?;
    let latest = eval_store.load_latest_report()?.ok_or_else(|| {
        context_graph_mejepa::EvalError::new(
            context_graph_mejepa::EvalErrorCode::ReportPersistFail,
            "latest eval report missing after eval-run",
        )
    })?;
    if latest.determinism_hash()? != report.determinism_hash()? {
        return Err(context_graph_mejepa::EvalError::new(
            context_graph_mejepa::EvalErrorCode::ReadbackMismatch,
            "latest eval report hash does not match current report",
        ));
    }
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(if report.ship_gate_passed {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn run_eval_build_graph(
    args: EvalBuildGraphArgs,
) -> Result<ExitCode, context_graph_mejepa::EvalError> {
    let db = context_graph_mejepa::open_infer_rocksdb(&args.db_path)?;
    let eval_store = context_graph_mejepa::RocksDbEvalStore::new(db)?;
    let graph =
        build_patch_similarity_graph(&synthetic_patch_embeddings(), args.threshold, args.top_k)?;
    eval_store.persist_graph(&graph)?;
    let readback = eval_store.load_graph()?.ok_or_else(|| {
        context_graph_mejepa::EvalError::new(
            context_graph_mejepa::EvalErrorCode::ReadbackMismatch,
            "patch similarity graph missing after persist",
        )
    })?;
    if readback.edge_count != graph.edge_count || readback.node_count != graph.node_count {
        return Err(context_graph_mejepa::EvalError::new(
            context_graph_mejepa::EvalErrorCode::ReadbackMismatch,
            "patch similarity graph readback differs",
        ));
    }
    let path = args.output_fsv.join("patch-similarity-graph.json");
    context_graph_mejepa::eval::report::write_json_0600(&path, &graph)?;
    println!("{}", serde_json::to_string_pretty(&graph)?);
    Ok(ExitCode::SUCCESS)
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
    let compiler = build_fixture_deterministic_compiler(
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

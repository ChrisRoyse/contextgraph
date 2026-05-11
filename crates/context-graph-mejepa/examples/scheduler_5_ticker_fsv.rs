use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{ensure, Context, Result};
use clap::Parser;
use context_graph_mejepa::eval::{ActiveLearningQueueEntry, ActiveLearningQueueState, RocksDbEvalStore};
use context_graph_mejepa::heal::{
    encode_value, read_status_snapshot, run_self_optimization_scheduler,
    write_json_atomic_readback, DriftSample, DriftStore, HealRocksStore, SchedulerState,
    SelfOptimConfig, TickName, CF_MEJEPA_TRAIN_CERTS,
};
use context_graph_mejepa::{OracleOutcome, TaskId, TrainCertSummary};
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, DB};
use serde_json::{json, Value};
use tokio::sync::watch;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value = "/mnt/d/contextgraph/fsv/mejepa-heal-fsv")]
    output_root: PathBuf,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> ExitCode {
    let _ = tracing_subscriber::fmt::try_init();
    let args = Args::parse();
    match run(args).await {
        Ok(report) => {
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
            ExitCode::SUCCESS
        }
        Err(err) => {
            tracing::error!(error = %err, "scheduler FSV failed");
            eprintln!("{err:#}");
            ExitCode::from(1)
        }
    }
}

async fn run(args: Args) -> Result<Value> {
    if args.output_root.exists() {
        ensure!(
            args.output_root.starts_with("/mnt/d/contextgraph/fsv/"),
            "refusing to remove FSV root outside /mnt/d/contextgraph/fsv"
        );
        fs::remove_dir_all(&args.output_root)
            .with_context(|| format!("remove {}", args.output_root.display()))?;
    }
    fs::create_dir_all(&args.output_root)
        .with_context(|| format!("create {}", args.output_root.display()))?;

    let happy = run_happy_path(&args.output_root.join("happy-path")).await?;
    let edge_shutdown = run_edge_shutdown(&args.output_root.join("edge-shutdown")).await?;
    let edge_invalid_config = run_edge_invalid_config(&args.output_root.join("edge-invalid-config")).await?;
    let edge_corrupt_drift = run_edge_corrupt_drift(&args.output_root.join("edge-corrupt-drift"))?;

    let report = json!({
        "sourceOfTruth": {
            "kind": "scheduler_status_file_plus_rocksdb",
            "schedulerStatusPath": args.output_root.join("happy-path/self_optimization_status.json"),
            "rocksDbPath": args.output_root.join("happy-path/rocksdb"),
            "finalArtifact": args.output_root.join("scheduler_ticks.json"),
            "schedulerTickers": TickName::all().map(|tick| tick.as_str()).to_vec(),
        },
        "happyPath": happy,
        "edgeCases": [edge_shutdown, edge_invalid_config, edge_corrupt_drift],
    });
    let artifact = args.output_root.join("scheduler_ticks.json");
    write_json_atomic_readback(&artifact, &report)?;
    let readback: Value = serde_json::from_slice(
        &fs::read(&artifact).with_context(|| format!("read {}", artifact.display()))?,
    )?;
    ensure!(readback == report, "scheduler FSV artifact readback mismatch");
    Ok(json!({
        "evidencePath": artifact,
        "report": report
    }))
}

async fn run_happy_path(root: &Path) -> Result<Value> {
    fs::create_dir_all(root).with_context(|| format!("create {}", root.display()))?;
    let db = open_db(&root.join("rocksdb"))?;
    seed_happy_path_state(db.clone())?;
    let status_path = root.join("self_optimization_status.json");
    let config = config_for(root, Duration::from_secs(1));
    let before = physical_state(db.as_ref(), Some(&status_path))?;
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let scheduler = tokio::spawn(run_self_optimization_scheduler(db.clone(), config, shutdown_rx));
    tokio::time::sleep(Duration::from_secs(10)).await;
    shutdown_tx.send(true).context("send shutdown")?;
    tokio::time::timeout(Duration::from_secs(5), scheduler)
        .await
        .context("scheduler shutdown timeout")?
        .context("scheduler join")?
        .context("scheduler result")?;
    let after = physical_state(db.as_ref(), Some(&status_path))?;
    let snapshot = read_status_snapshot(&status_path)?;
    let mut expected = BTreeMap::new();
    for tick in TickName::all() {
        let count = snapshot
            .ticker_counts
            .get(tick.as_str())
            .copied()
            .unwrap_or_default();
        expected.insert(tick.as_str().to_string(), json!({
            "observedCount": count,
            "expectedMinimum": 9,
            "expectedMaximum": 20,
        }));
        ensure!(
            (9..=20).contains(&count),
            "{} expected 9..=20 ticks from 10s one-second run, got {count}",
            tick.as_str()
        );
    }
    ensure!(snapshot.status == "stopped", "scheduler did not persist stopped status");
    ensure!(
        after["cfCounts"][context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_LEARNING_QUEUE].as_u64()
            == Some(1),
        "active learning queue row missing from RocksDB source of truth"
    );
    Ok(json!({
        "case": "happy_path_one_second_periods_for_ten_seconds",
        "trigger": "run_self_optimization_scheduler with all seven periods set to one second",
        "expectedOutcome": "all seven tickers fire and status file plus RocksDB source-of-truth state are readable after shutdown",
        "before": before,
        "after": after,
        "statusSnapshot": snapshot,
        "tickExpectations": expected,
    }))
}

async fn run_edge_shutdown(root: &Path) -> Result<Value> {
    fs::create_dir_all(root).with_context(|| format!("create {}", root.display()))?;
    let db = open_db(&root.join("rocksdb"))?;
    seed_happy_path_state(db.clone())?;
    let status_path = root.join("self_optimization_status.json");
    let config = config_for(root, Duration::from_millis(25));
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let scheduler = tokio::spawn(run_self_optimization_scheduler(db.clone(), config, shutdown_rx));
    tokio::time::sleep(Duration::from_millis(75)).await;
    let before = physical_state(db.as_ref(), Some(&status_path))?;
    shutdown_tx.send(true).context("send shutdown")?;
    tokio::time::timeout(Duration::from_secs(5), scheduler)
        .await
        .context("scheduler shutdown timeout")?
        .context("scheduler join")?
        .context("scheduler result")?;
    let after = physical_state(db.as_ref(), Some(&status_path))?;
    let snapshot = read_status_snapshot(&status_path)?;
    ensure!(snapshot.status == "stopped");
    ensure!(
        after["statusFile"]["tickerTotal"].as_u64() >= before["statusFile"]["tickerTotal"].as_u64(),
        "shutdown lost previously persisted ticker state"
    );
    Ok(json!({
        "case": "edge_shutdown_during_active_loop",
        "trigger": "send shutdown after ticks have started",
        "expectedOutcome": "current persisted scheduler state remains readable and final status is stopped",
        "before": before,
        "after": after,
        "statusSnapshot": snapshot,
    }))
}

async fn run_edge_invalid_config(root: &Path) -> Result<Value> {
    fs::create_dir_all(root).with_context(|| format!("create {}", root.display()))?;
    let db = open_db(&root.join("rocksdb"))?;
    seed_happy_path_state(db.clone())?;
    let before = physical_state(db.as_ref(), None)?;
    let mut config = config_for(root, Duration::from_millis(50));
    config.status_path = PathBuf::new();
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    let err = run_self_optimization_scheduler(db.clone(), config, shutdown_rx)
        .await
        .expect_err("empty status path must fail closed");
    let after = physical_state(db.as_ref(), None)?;
    ensure!(err.code() == "MEJEPA_HEAL_INVALID_STATE");
    ensure!(before == after, "invalid config changed physical DB state");
    Ok(json!({
        "case": "edge_empty_status_path",
        "trigger": "run scheduler with empty status_path",
        "expectedOutcome": "MEJEPA_HEAL_INVALID_STATE before any scheduler mutation",
        "before": before,
        "errorCode": err.code(),
        "error": err.to_string(),
        "after": after,
    }))
}

fn run_edge_corrupt_drift(root: &Path) -> Result<Value> {
    fs::create_dir_all(root).with_context(|| format!("create {}", root.display()))?;
    let db = open_db(&root.join("rocksdb"))?;
    seed_happy_path_state(db.clone())?;
    put_raw_cf(
        db.as_ref(),
        context_graph_mejepa_cf::CF_MEJEPA_DRIFT_WINDOW,
        b"corrupt-drift-sample",
        b"not-bincode",
    )?;
    let status_path = root.join("self_optimization_status.json");
    let before = physical_state(db.as_ref(), Some(&status_path))?;
    let mut state = SchedulerState::open(db.clone(), config_for(root, Duration::from_secs(1)))?;
    state.run_tick(TickName::DriftCheck)?;
    let after = physical_state(db.as_ref(), Some(&status_path))?;
    let snapshot = read_status_snapshot(&status_path)?;
    let drift_error = snapshot
        .last_error
        .get(TickName::DriftCheck.as_str())
        .context("drift error missing from scheduler status")?;
    ensure!(drift_error.error_code.as_deref() == Some("MEJEPA_HEAL_BINCODE"));
    ensure!(
        before["cfCounts"][context_graph_mejepa_cf::CF_MEJEPA_DRIFT_WINDOW]
            == after["cfCounts"][context_graph_mejepa_cf::CF_MEJEPA_DRIFT_WINDOW],
        "corrupt drift read should not hide or delete the corrupt source row"
    );
    Ok(json!({
        "case": "edge_corrupt_drift_storage",
        "trigger": "insert non-bincode bytes into CF_MEJEPA_DRIFT_WINDOW then run drift_check",
        "expectedOutcome": "tick failure is logged into scheduler status and corrupt RocksDB row remains available for diagnosis",
        "before": before,
        "after": after,
        "statusSnapshot": snapshot,
    }))
}

fn config_for(root: &Path, period: Duration) -> SelfOptimConfig {
    SelfOptimConfig {
        status_path: root.join("self_optimization_status.json"),
        hygiene_archive_root: root.join("archive"),
        witness_chain_path: root.join("witness-chain.bin"),
        ..SelfOptimConfig::default().with_all_periods(period)
    }
}

fn open_db(path: &Path) -> Result<Arc<DB>> {
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

fn seed_happy_path_state(db: Arc<DB>) -> Result<()> {
    let eval_store = RocksDbEvalStore::new(db.clone())?;
    let mut queue = ActiveLearningQueueState::new(4)?;
    for (idx, score) in [0.91f32, 0.73, 0.62].into_iter().enumerate() {
        let task_id = TaskId(format!("scheduler-fsv-task-{idx}"));
        queue.entries.insert(
            task_id.clone(),
            ActiveLearningQueueEntry {
                task_id,
                score,
                outcome_set_len: 2 + idx,
                ood_score: 0.10 + idx as f32 * 0.05,
                reason: "wide_conformal_set".to_string(),
            },
        );
    }
    eval_store.persist_queue(&queue)?;

    let store = HealRocksStore::from_db(db.clone())?;
    for offset in 0..context_graph_mejepa::heal::drift::DEFAULT_DRIFT_MIN_DETECTION_SAMPLES as u64 {
        let sample = DriftSample::try_new(
            vec![OracleOutcome::Pass],
            OracleOutcome::Pass,
            offset,
            0.05,
            1.0,
        )?;
        store.persist_drift_sample(offset, &sample)?;
    }
    for step in 0..3u64 {
        let summary = TrainCertSummary {
            step,
            delta_omega: 0.50,
            delta_xi: 0.50,
            witness_offset: step,
        };
        store.put_cf_readback(
            CF_MEJEPA_TRAIN_CERTS,
            &step.to_be_bytes(),
            &encode_value(&summary)?,
        )?;
    }
    Ok(())
}

fn physical_state(db: &DB, status_path: Option<&Path>) -> Result<Value> {
    let mut counts = serde_json::Map::new();
    for cf_name in context_graph_mejepa_cf::all_hygiene_referenced_cfs() {
        counts.insert(cf_name.to_string(), json!(cf_count(db, cf_name)?));
    }
    let status_file = match status_path {
        Some(path) if path.exists() => {
            let snapshot = read_status_snapshot(path)?;
            json!({
                "exists": true,
                "path": path,
                "lenBytes": fs::metadata(path)?.len(),
                "status": snapshot.status,
                "tickerCounts": snapshot.ticker_counts,
                "tickerTotal": snapshot.ticker_counts.values().sum::<u64>(),
                "lastTicker": snapshot.last_ticker,
            })
        }
        Some(path) => json!({
            "exists": false,
            "path": path,
            "tickerTotal": 0,
        }),
        None => json!({
            "exists": false,
            "path": null,
            "tickerTotal": 0,
        }),
    };
    Ok(json!({
        "cfCounts": Value::Object(counts),
        "statusFile": status_file,
    }))
}

fn cf_count(db: &DB, cf_name: &str) -> Result<u64> {
    let cf = db
        .cf_handle(cf_name)
        .with_context(|| format!("missing column family {cf_name}"))?;
    let mut count = 0u64;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let _ = item?;
        count += 1;
    }
    Ok(count)
}

fn put_raw_cf(db: &DB, cf_name: &str, key: &[u8], value: &[u8]) -> Result<()> {
    let cf = db
        .cf_handle(cf_name)
        .with_context(|| format!("missing column family {cf_name}"))?;
    db.put_cf(cf, key, value)?;
    let readback = db
        .get_cf(cf, key)?
        .with_context(|| format!("readback missing for {cf_name}/{}", hex::encode(key)))?;
    ensure!(readback == value, "raw put readback mismatch");
    Ok(())
}

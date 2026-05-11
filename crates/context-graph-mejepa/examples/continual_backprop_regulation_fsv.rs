use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{ensure, Context, Result};
use clap::Parser;
use context_graph_mejepa::heal::{
    bootstrap_pipeline_for_path, encode_value, regulate_substrate, ComponentBreakdown,
    SelfHealingPipeline, StatusChange, CF_MEJEPA_TRAIN_CERTS, CF_MEJEPA_WEIGHT_BLOBS,
};
use context_graph_mejepa::{OracleOutcome, TrainCertSummary};
use context_graph_mejepa_instruments::{Panel, PANEL_DIM};
use serde_json::{json, Value};

#[derive(Parser, Debug)]
struct Args {
    #[arg(
        long,
        default_value = "/mnt/d/contextgraph/fsv/contextgraph-mejepa-continual-backprop-fsv"
    )]
    output_root: PathBuf,
}

fn main() -> ExitCode {
    let _ = tracing_subscriber::fmt::try_init();
    let args = Args::parse();
    match run(args) {
        Ok(report) => {
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
            ExitCode::SUCCESS
        }
        Err(err) => {
            tracing::error!(error = %err, "continual-backprop FSV failed");
            eprintln!("{err:#}");
            ExitCode::from(1)
        }
    }
}

fn run(args: Args) -> Result<Value> {
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

    let happy = run_observe_happy_path(&args.output_root.join("happy-path"))?;
    let edge_empty = run_edge_empty_activation_window(&args.output_root.join("edge-empty-window"))?;
    let edge_missing =
        run_edge_missing_embedder(&args.output_root.join("edge-missing-embedder"))?;
    let edge_stability =
        run_edge_stability_zero(&args.output_root.join("edge-stability-zero"))?;

    let report = json!({
        "sourceOfTruth": {
            "kind": "rocksdb_column_family",
            "columnFamily": CF_MEJEPA_WEIGHT_BLOBS,
            "happyPathDb": args.output_root.join("happy-path/heal-rocksdb"),
            "happyPathExpectedKey": "continual_backprop/7/0"
        },
        "happyPath": happy,
        "edgeCases": [edge_empty, edge_missing, edge_stability],
    });
    let evidence_path = args
        .output_root
        .join("continual_backprop_regulation_fsv_evidence.json");
    write_json_readback(&evidence_path, &report)?;
    Ok(json!({
        "evidencePath": evidence_path,
        "report": report
    }))
}

fn run_observe_happy_path(root: &Path) -> Result<Value> {
    let mut pipeline = bootstrap_pipeline_for_path(root)
        .with_context(|| format!("bootstrap pipeline at {}", root.display()))?;
    seed_low_plasticity_train_certs(&mut pipeline)?;
    let before = physical_state(&pipeline)?;
    let before_weights = pipeline.active_embedders[0].weights.clone();
    let panel = dormant_panel()?;
    let output = pipeline
        .observe(&panel, &OracleOutcome::Pass, 1.0, 0, "cbp-fsv-session")
        .context("observe should trigger low-plasticity regulation")?;
    let expected_key = "continual_backprop/7/0";
    let readback = pipeline
        .storage
        .get_cf(CF_MEJEPA_WEIGHT_BLOBS, expected_key.as_bytes())
        .context("read continual-backprop persisted key")?;
    let after = physical_state(&pipeline)?;
    ensure!(
        readback.is_some(),
        "expected RocksDB row {expected_key} in {CF_MEJEPA_WEIGHT_BLOBS}"
    );
    ensure!(
        before_weights != pipeline.active_embedders[0].weights,
        "active embedder weights did not change"
    );
    ensure!(
        after["weightBlobCount"].as_u64() > before["weightBlobCount"].as_u64(),
        "weight blob count did not increase"
    );
    Ok(json!({
        "case": "happy_observe_low_plasticity_triggers_dormant_reinit",
        "trigger": "observe(panel with dormant units 0 and 2) plus low-plasticity train cert window",
        "expectedOutcome": "RocksDB CF_MEJEPA_WEIGHT_BLOBS contains continual_backprop/7/0 and active weights changed",
        "before": before,
        "observeOutput": output,
        "after": after,
        "readback": {
            "columnFamily": CF_MEJEPA_WEIGHT_BLOBS,
            "key": expected_key,
            "bytes": readback.as_ref().map(Vec::len).unwrap_or(0)
        },
        "assertions": {
            "persistedKeyExists": readback.is_some(),
            "activeWeightsChanged": before_weights != pipeline.active_embedders[0].weights
        }
    }))
}

fn run_edge_empty_activation_window(root: &Path) -> Result<Value> {
    let mut pipeline = bootstrap_pipeline_for_path(root)
        .with_context(|| format!("bootstrap pipeline at {}", root.display()))?;
    let before = physical_state(&pipeline)?;
    let err = regulate_substrate(&mut pipeline, &low_plasticity_breakdown(Some(7), 1.0))
        .expect_err("empty activation window must fail closed");
    let after = physical_state(&pipeline)?;
    ensure!(err.code() == "MEJEPA_HEAL_INVALID_STATE");
    ensure!(before == after);
    Ok(json!({
        "case": "edge_empty_activation_window",
        "trigger": "regulate_substrate without captured activations",
        "expectedOutcome": "MEJEPA_HEAL_INVALID_STATE and no RocksDB CF change",
        "before": before,
        "errorCode": err.code(),
        "error": err.to_string(),
        "after": after
    }))
}

fn run_edge_missing_embedder(root: &Path) -> Result<Value> {
    let mut pipeline = bootstrap_pipeline_for_path(root)
        .with_context(|| format!("bootstrap pipeline at {}", root.display()))?;
    pipeline.dormant_activation_window.push(vec![0.0; 16]);
    let before = physical_state(&pipeline)?;
    let err = regulate_substrate(&mut pipeline, &low_plasticity_breakdown(Some(404), 1.0))
        .expect_err("missing embedder must fail closed");
    let after = physical_state(&pipeline)?;
    ensure!(err.code() == "MEJEPA_HEAL_INVALID_STATE");
    ensure!(before == after);
    Ok(json!({
        "case": "edge_missing_embedder",
        "trigger": "regulate_substrate points at embedder_id 404",
        "expectedOutcome": "MEJEPA_HEAL_INVALID_STATE and no RocksDB CF change",
        "before": before,
        "errorCode": err.code(),
        "error": err.to_string(),
        "after": after
    }))
}

fn run_edge_stability_zero(root: &Path) -> Result<Value> {
    let mut pipeline = bootstrap_pipeline_for_path(root)
        .with_context(|| format!("bootstrap pipeline at {}", root.display()))?;
    pipeline.dormant_activation_window.push(vec![0.0; 16]);
    let before = physical_state(&pipeline)?;
    let err = regulate_substrate(&mut pipeline, &low_plasticity_breakdown(Some(7), 0.0))
        .expect_err("zero stability floor must halt before mutation");
    let after = physical_state(&pipeline)?;
    ensure!(err.code() == "MEJEPA_HEAL_CRITICAL_BUG");
    ensure!(after["statusChange"] == json!(StatusChange::Paused));
    ensure!(before["weightBlobCount"] == after["weightBlobCount"]);
    Ok(json!({
        "case": "edge_stability_floor_zero",
        "trigger": "regulate_substrate with stability_floor = 0",
        "expectedOutcome": "MEJEPA_HEAL_CRITICAL_BUG, pipeline paused, no weight blob write",
        "before": before,
        "errorCode": err.code(),
        "error": err.to_string(),
        "after": after
    }))
}

fn seed_low_plasticity_train_certs(pipeline: &mut SelfHealingPipeline) -> Result<()> {
    for step in 0..3u64 {
        let summary = TrainCertSummary {
            step,
            delta_omega: 0.10,
            delta_xi: 0.90,
            witness_offset: step,
        };
        pipeline
            .storage
            .put_cf_readback(
                CF_MEJEPA_TRAIN_CERTS,
                &step.to_be_bytes(),
                &encode_value(&summary)?,
            )
            .with_context(|| format!("persist low-plasticity train cert {step}"))?;
    }
    Ok(())
}

fn dormant_panel() -> Result<Panel> {
    let mut data = vec![0.2; PANEL_DIM];
    data[0] = 0.0;
    data[2] = 0.0;
    Panel::try_new(data, (1u16 << 15) - 1).context("construct dormant-unit panel")
}

fn low_plasticity_breakdown(
    embedder_id_with_highest_dk_contribution: Option<u32>,
    stability_floor: f32,
) -> ComponentBreakdown {
    ComponentBreakdown {
        effective_plasticity: 0.10,
        landscape_health: 0.80,
        stability_floor,
        predictor_redundancy: 0.10,
        constellation_violation_rate: 0.0,
        target_collapse: 0.0,
        embedder_id_with_highest_dk_contribution,
    }
}

fn physical_state(pipeline: &SelfHealingPipeline) -> Result<Value> {
    let status = pipeline.status.lock().unwrap().clone();
    let embedder = pipeline
        .active_embedders
        .first()
        .context("active embedder missing")?;
    Ok(json!({
        "weightBlobCount": pipeline.storage.count_cf(CF_MEJEPA_WEIGHT_BLOBS)?,
        "trainCertCount": pipeline.storage.count_cf(CF_MEJEPA_TRAIN_CERTS)?,
        "observationCounter": status.observation_counter,
        "statusChange": status.status_change,
        "dormantActivationRows": pipeline.dormant_activation_window.len(),
        "activeEmbedderId": embedder.embedder_id,
        "activeEmbedderWeightLen": embedder.weights.len(),
        "activeEmbedderFirstFourWeights": embedder.weights.iter().take(4).copied().collect::<Vec<_>>()
    }))
}

fn write_json_readback(path: &Path, value: &Value) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, &bytes).with_context(|| format!("write {}", path.display()))?;
    #[cfg(unix)]
    fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o600))
        .with_context(|| format!("chmod 0600 {}", path.display()))?;
    let readback = fs::read(path).with_context(|| format!("read back {}", path.display()))?;
    ensure!(readback == bytes, "evidence file readback mismatch");
    Ok(())
}

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use context_graph_mejepa::calibration::{count_cf, CF_MEJEPA_LIVE_PREDICTIONS};
use context_graph_mejepa::conformal::{non_conformity_score, CalibrationExample};
use context_graph_mejepa::heal::store::HealRocksStore;
use context_graph_mejepa::system_cost::{
    attach_system_cost_to_weekly_report, HealTickerTelemetrySnapshot, SystemCostBreakdown,
    SystemCostCounters, SystemCostError, SystemCostSnapshot,
};
use context_graph_mejepa::{
    build_deterministic_compiler, fixture_patch_context, CalibrationStore, Language,
    MeJepaInferConfig, RocksDbInferStore, VerifyVerdict,
};
use context_graph_mejepa_cf::CF_MEJEPA_WEIGHT_BLOBS;
use context_graph_mejepa_hygiene::open_hygiene_rocksdb;
use rocksdb::{IteratorMode, DB};
use serde_json::json;
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/phase-f-system-cost-fsv";
const REPORT_FILE: &str = "system_cost_fsv.json";
const VERIFY_COUNT: u64 = 1_000;
const HEAL_WRITE_COUNT: u64 = 200;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let started_at_unix_ms = chrono::Utc::now().timestamp_millis();
    let fsv_root = PathBuf::from(FSV_ROOT);
    fs::create_dir_all(&fsv_root)?;
    let run_root = fsv_root.join(format!("run-{}-{}", started_at_unix_ms, std::process::id()));
    fs::create_dir_all(&run_root)?;

    let counters = SystemCostCounters::new();
    let before = counters.snapshot();
    counters.record_cuda_microseconds(12_000);
    counters.record_agent_feedback_tokens(450);
    counters.record_rocksdb_write(2_048);
    counters.record_rocksdb_write(512);
    counters.record_heal_ticker_run("observe", 7)?;
    counters.record_heal_ticker_run("drift", 11)?;
    counters.record_heal_scheduler_restart();
    counters.record_operator_override_sampler_applied(3);
    let after = counters.snapshot();
    let breakdown = SystemCostBreakdown::from_snapshots(&before, &after);
    let weekly_report = json!({
        "reportDate": "2026-05-13",
        "shipGatePassed": false,
        "predictionOracleCorrelation": 0.866667
    });
    let extended_report = attach_system_cost_to_weekly_report(weekly_report, &breakdown)?;
    let weekly_report_path = run_root.join("weekly-report-with-system-cost.json");
    write_json_readback(&weekly_report_path, &extended_report)?;
    let weekly_readback: serde_json::Value =
        serde_json::from_slice(&fs::read(&weekly_report_path)?)?;
    let producer_evidence = producer_wiring_evidence(&run_root)?;
    let happy_pass = weekly_readback["systemCost"]["windowDelta"]["cudaKernelMicrosecondsTotal"]
        == json!(12_000)
        && weekly_readback["systemCost"]["windowDelta"]["agentFeedbackTokensTotal"] == json!(450)
        && weekly_readback["systemCost"]["windowDelta"]["rocksdbBytesWrittenTotal"] == json!(2_560)
        && weekly_readback["systemCost"]["windowDelta"]["rocksdbWritesTotal"] == json!(2)
        && weekly_readback["systemCost"]["windowDelta"]["operatorOverrideSamplerAppliedCountTotal"]
            == json!(3)
        && weekly_readback["systemCost"]["windowDelta"]["healTickerTelemetryTotal"]
            ["schedulerRestartCountTotal"]
            == json!(1)
        && weekly_readback["heal_tickers_per_window"]["tickers"]
            .as_array()
            .map(|rows| rows.len())
            == Some(2)
        && weekly_readback["systemCost"]["noSpendCaps"] == json!(true)
        && !contains_forbidden(&weekly_readback);

    let boundary_cases = vec![
        restart_delta_clamps_to_zero()?,
        forbidden_cost_cap_field_rejected()?,
        forbidden_nested_spend_field_rejected()?,
        non_object_weekly_report_rejected()?,
        invalid_heal_ticker_rejected()?,
    ];
    let all_boundaries_pass = boundary_cases
        .iter()
        .all(|case| case["pass"].as_bool().unwrap_or(false));

    let evidence_path = fsv_root.join(REPORT_FILE);
    let evidence = json!({
        "fsv_root": FSV_ROOT,
        "task_id": "TASK-OBS-013",
        "started_at_unix_ms": started_at_unix_ms,
        "build_release_sha": git_head_short()?,
        "run_root": run_root.display().to_string(),
        "happy_path": [
            {
                "case": "weekly_report_includes_resource_cost_breakdown",
                "sot": weekly_report_path.display().to_string(),
                "before": before,
                "trigger": "cargo run --release -p context-graph-mejepa --example phase_f_system_cost_fsv",
                "after": weekly_readback,
                "expected": {
                    "cuda_kernel_microseconds_total": 12000,
                    "agent_feedback_tokens_total": 450,
                    "rocksdb_bytes_written_total": 2560,
                    "rocksdb_writes_total": 2,
                    "heal_ticker_scheduler_restart_count_total": 1,
                    "heal_ticker_rows": 2,
                    "operator_override_sampler_applied_count_total": 3,
                    "no_spend_caps": true,
                    "forbidden_fields_absent": true
                },
                "actual": breakdown,
                "pass": happy_pass,
                "evidence_path": weekly_report_path.display().to_string()
            },
            producer_evidence
        ],
        "boundary_cases": boundary_cases,
        "all_passed": happy_pass
            && producer_evidence["pass"].as_bool().unwrap_or(false)
            && all_boundaries_pass,
        "cf_counts_before": {"weeklyReportFiles": 0},
        "cf_counts_after": {"weeklyReportFiles": 1},
        "readback_equal": happy_pass && producer_evidence["readback_equal"].as_bool().unwrap_or(false),
        "physical_artifacts": {
            "weekly_report": artifact_metadata(&weekly_report_path)?,
            "fsv_artifact": {
                "path": evidence_path.display().to_string(),
                "will_exist_after_write": true
            }
        }
    });
    if !evidence["all_passed"].as_bool().unwrap_or(false) {
        return Err("TASK-OBS-013 FSV evidence did not pass".into());
    }
    write_json_readback(&evidence_path, &evidence)?;
    let final_readback: serde_json::Value = serde_json::from_slice(&fs::read(&evidence_path)?)?;
    if final_readback["all_passed"] != json!(true)
        || final_readback["readback_equal"] != json!(true)
    {
        return Err("system cost FSV artifact readback failed".into());
    }
    println!("{}", serde_json::to_string_pretty(&final_readback)?);
    Ok(())
}

fn producer_wiring_evidence(
    run_root: &Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let producer_root = run_root.join("producer-wiring");
    let db_path = producer_root.join("infer-db");
    let repo_root = producer_root.join("repo");
    fs::create_dir_all(&repo_root)?;
    let db = open_hygiene_rocksdb(&db_path)?;
    let counters = Arc::new(SystemCostCounters::new());
    let before = counters.snapshot();
    let calibration_store = CalibrationStore::new(db.clone(), 30)?;
    let calibration_examples = calibration_examples();
    let norm_sq = calibration_examples
        .iter()
        .map(|example| {
            let score =
                non_conformity_score(&example.predicted_test_pass, &example.actual_test_pass)?;
            Ok(score * score)
        })
        .collect::<Result<Vec<_>, context_graph_mejepa::MejepaInferError>>()?;
    calibration_store.calibrate(
        &calibration_examples,
        &norm_sq,
        0.10,
        30,
        0.30,
        [8; 32],
        BTreeMap::new(),
    )?;
    let store = Arc::new(RocksDbInferStore::new_with_system_cost_counters(
        db.clone(),
        Arc::clone(&counters),
    ));
    let compiler = build_deterministic_compiler(
        repo_root.clone(),
        store.clone(),
        calibration_store,
        MeJepaInferConfig {
            outcome_set_max: 1,
            pause_state_path: None,
            ..MeJepaInferConfig::default()
        },
    )?;

    for idx in 0..VERIFY_COUNT {
        let scenario = format!("system-cost-verify-{idx:04}");
        let (patch, mut context) = fixture_patch_context(&repo_root, &scenario)?;
        context.session_id = [0x84; 16];
        let verdict = compiler.verify(&patch, &context)?;
        match verdict {
            VerifyVerdict::Approve { .. } => {}
            other => return Err(format!("verify producer case did not approve: {other:?}").into()),
        }
    }
    let live_prediction_count = count_cf(db.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?;
    let live_prediction_bytes = cf_value_bytes(db.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?;

    let heal_store =
        HealRocksStore::from_db_with_system_cost_counters(db.clone(), Arc::clone(&counters))?;
    let mut heal_value_bytes = 0_u64;
    for idx in 0..HEAL_WRITE_COUNT {
        let key = format!("phase-f-system-cost-heal-{idx:04}");
        let value = vec![idx as u8; 64 + (idx as usize % 17)];
        heal_value_bytes += value.len() as u64;
        heal_store.put_cf_readback(CF_MEJEPA_WEIGHT_BLOBS, key.as_bytes(), &value)?;
    }
    let after = counters.snapshot();
    let expected_rocksdb_bytes = live_prediction_bytes + heal_value_bytes;
    let expected_rocksdb_writes = VERIFY_COUNT + HEAL_WRITE_COUNT;

    flush_cf(db.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?;
    flush_cf(db.as_ref(), CF_MEJEPA_WEIGHT_BLOBS)?;
    drop(compiler);
    drop(store);
    drop(heal_store);
    drop(db);

    let reopened = open_hygiene_rocksdb(&db_path)?;
    let reopened_live_count = count_cf(reopened.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?;
    let reopened_live_bytes = cf_value_bytes(reopened.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?;
    let reopened_heal_count = count_cf(reopened.as_ref(), CF_MEJEPA_WEIGHT_BLOBS)?;
    drop(reopened);

    let pass = live_prediction_count == VERIFY_COUNT
        && reopened_live_count == VERIFY_COUNT
        && reopened_heal_count == HEAL_WRITE_COUNT
        && reopened_live_bytes == live_prediction_bytes
        && after.cuda_kernel_microseconds_total > 0
        && after.rocksdb_writes_total == expected_rocksdb_writes
        && after.rocksdb_bytes_written_total == expected_rocksdb_bytes;
    Ok(json!({
        "case": "producer_paths_increment_system_cost_counters",
        "sot": format!("{CF_MEJEPA_LIVE_PREDICTIONS} + {CF_MEJEPA_WEIGHT_BLOBS} + SystemCostCounters"),
        "before": before,
        "trigger": format!(
            "{VERIFY_COUNT} deterministic verify calls plus {HEAL_WRITE_COUNT} HealRocksStore::put_cf_readback writes"
        ),
        "after": after,
        "expected": {
            "live_prediction_rows": VERIFY_COUNT,
            "heal_weight_blob_rows": HEAL_WRITE_COUNT,
            "rocksdb_writes_total": expected_rocksdb_writes,
            "rocksdb_bytes_written_total": expected_rocksdb_bytes,
            "cuda_kernel_microseconds_total": "> 0"
        },
        "actual": {
            "live_prediction_rows": live_prediction_count,
            "reopened_live_prediction_rows": reopened_live_count,
            "heal_weight_blob_rows": reopened_heal_count,
            "live_prediction_bytes": live_prediction_bytes,
            "reopened_live_prediction_bytes": reopened_live_bytes,
            "heal_value_bytes": heal_value_bytes,
            "system_cost_snapshot": after
        },
        "pass": pass,
        "readback_equal": reopened_live_count == live_prediction_count
            && reopened_live_bytes == live_prediction_bytes
            && reopened_heal_count == HEAL_WRITE_COUNT,
        "evidence_path": producer_root.display().to_string()
    }))
}

fn restart_delta_clamps_to_zero() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let earlier = SystemCostSnapshot {
        cuda_kernel_microseconds_total: 10_000,
        agent_feedback_tokens_total: 1_000,
        rocksdb_bytes_written_total: 5_000,
        rocksdb_writes_total: 20,
        heal_ticker_telemetry_total: HealTickerTelemetrySnapshot::default(),
        operator_override_sampler_applied_count_total: 12,
        online_reward_signals_applied_count_total: 0,
        ewc_violations_total: 3,
        dormant_units_reinit_total: 0,
    };
    let current = SystemCostSnapshot::default();
    let breakdown = SystemCostBreakdown::from_snapshots(&earlier, &current);
    Ok(json!({
        "case": "daemon_restart_counter_reset",
        "sot": "SystemCostSnapshot::delta",
        "expected": 0,
        "actual": breakdown.window_delta.cuda_kernel_microseconds_total,
        "pass": breakdown.window_delta == SystemCostSnapshot::default()
    }))
}

fn forbidden_cost_cap_field_rejected() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let report = json!({
        "reportDate": "2026-05-13",
        "costCapReached": false
    });
    let err = attach_system_cost_to_weekly_report(report, &empty_breakdown()).unwrap_err();
    Ok(json!({
        "case": "forbidden_cost_cap_reached_field",
        "sot": "weekly report JSON",
        "expected": "MEJEPA_SYSTEM_COST_FORBIDDEN_FIELD",
        "actual": error_code(&err),
        "pass": matches!(err, SystemCostError::ForbiddenField("costCapReached"))
    }))
}

fn forbidden_nested_spend_field_rejected() -> Result<serde_json::Value, Box<dyn std::error::Error>>
{
    let report = json!({
        "reportDate": "2026-05-13",
        "nested": {"costEstimateUsd": 1.25}
    });
    let err = attach_system_cost_to_weekly_report(report, &empty_breakdown()).unwrap_err();
    Ok(json!({
        "case": "forbidden_nested_spend_field",
        "sot": "weekly report JSON",
        "expected": "MEJEPA_SYSTEM_COST_FORBIDDEN_FIELD",
        "actual": error_code(&err),
        "pass": matches!(err, SystemCostError::ForbiddenField("costEstimateUsd"))
    }))
}

fn non_object_weekly_report_rejected() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let err = attach_system_cost_to_weekly_report(json!(["not", "object"]), &empty_breakdown())
        .unwrap_err();
    Ok(json!({
        "case": "non_object_weekly_report",
        "sot": "weekly report JSON",
        "expected": "MEJEPA_SYSTEM_COST_WEEKLY_REPORT_NOT_OBJECT",
        "actual": error_code(&err),
        "pass": matches!(err, SystemCostError::WeeklyReportNotObject)
    }))
}

fn invalid_heal_ticker_rejected() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let counters = SystemCostCounters::new();
    let before = counters.snapshot();
    let err = counters
        .record_heal_ticker_run("DriftCheck", 10)
        .unwrap_err();
    let after = counters.snapshot();
    Ok(json!({
        "case": "invalid_heal_ticker_name_rejected",
        "sot": "SystemCostCounters::snapshot",
        "expected": "MEJEPA_SYSTEM_COST_INVALID_TICKER",
        "actual": error_code(&err),
        "pass": matches!(err, SystemCostError::InvalidTicker(_)) && before == after
    }))
}

fn calibration_examples() -> Vec<CalibrationExample> {
    (0..100)
        .map(|idx| {
            let low = matches!(idx, 9 | 20 | 31 | 42 | 53 | 64 | 75 | 86 | 97);
            CalibrationExample {
                language: Language::Python,
                predicted_test_pass: vec![if low { 0.10 } else { 0.95 }],
                actual_test_pass: vec![1.0],
            }
        })
        .collect()
}

fn cf_value_bytes(db: &DB, cf_name: &str) -> Result<u64, Box<dyn std::error::Error>> {
    let cf = db
        .cf_handle(cf_name)
        .ok_or_else(|| format!("missing column family {cf_name}"))?;
    let mut bytes = 0_u64;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        bytes += value.len() as u64;
    }
    Ok(bytes)
}

fn flush_cf(db: &DB, cf_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let cf = db
        .cf_handle(cf_name)
        .ok_or_else(|| format!("missing column family {cf_name}"))?;
    db.flush_cf(cf)?;
    Ok(())
}

fn empty_breakdown() -> SystemCostBreakdown {
    SystemCostBreakdown::from_snapshots(
        &SystemCostSnapshot::default(),
        &SystemCostSnapshot::default(),
    )
}

fn contains_forbidden(value: &serde_json::Value) -> bool {
    let forbidden = [
        "costEstimateUsd",
        "cost_estimate_usd",
        "costCapReached",
        "cost_cap_reached",
        "spendCap",
        "spend_cap",
    ];
    match value {
        serde_json::Value::Object(map) => map
            .iter()
            .any(|(key, child)| forbidden.contains(&key.as_str()) || contains_forbidden(child)),
        serde_json::Value::Array(items) => items.iter().any(contains_forbidden),
        _ => false,
    }
}

fn error_code(err: &SystemCostError) -> &'static str {
    match err {
        SystemCostError::WeeklyReportNotObject => "MEJEPA_SYSTEM_COST_WEEKLY_REPORT_NOT_OBJECT",
        SystemCostError::ForbiddenField(_) => "MEJEPA_SYSTEM_COST_FORBIDDEN_FIELD",
        SystemCostError::InvalidTicker(_) => "MEJEPA_SYSTEM_COST_INVALID_TICKER",
        SystemCostError::Serialize(_) => "MEJEPA_SYSTEM_COST_SERIALIZE_FAILED",
    }
}

fn write_json_readback<T: serde::Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, &bytes)?;
    let readback = fs::read(path)?;
    if readback != bytes {
        return Err(format!("readback mismatch for {}", path.display()).into());
    }
    Ok(())
}

fn artifact_metadata(path: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    Ok(json!({
        "path": path,
        "bytes": bytes.len(),
        "sha256": hex::encode(Sha256::digest(&bytes))
    }))
}

fn git_head_short() -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()?;
    if !output.status.success() {
        return Err("git rev-parse --short HEAD failed".into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

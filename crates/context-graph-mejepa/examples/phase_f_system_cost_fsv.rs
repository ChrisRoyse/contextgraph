use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use context_graph_mejepa::system_cost::{
    attach_system_cost_to_weekly_report, HealTickerTelemetrySnapshot, SystemCostBreakdown,
    SystemCostCounters, SystemCostError, SystemCostSnapshot,
};
use serde_json::json;
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-f-system-cost-fsv";
const REPORT_FILE: &str = "system_cost_fsv.json";

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
        "run_root": run_root,
        "happy_path": [{
            "case": "weekly_report_includes_resource_cost_breakdown",
            "sot": weekly_report_path,
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
            "evidence_path": weekly_report_path
        }],
        "boundary_cases": boundary_cases,
        "all_passed": happy_pass && all_boundaries_pass,
        "cf_counts_before": {"weeklyReportFiles": 0},
        "cf_counts_after": {"weeklyReportFiles": 1},
        "readback_equal": happy_pass,
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

fn restart_delta_clamps_to_zero() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let earlier = SystemCostSnapshot {
        cuda_kernel_microseconds_total: 10_000,
        agent_feedback_tokens_total: 1_000,
        rocksdb_bytes_written_total: 5_000,
        rocksdb_writes_total: 20,
        heal_ticker_telemetry_total: HealTickerTelemetrySnapshot::default(),
        operator_override_sampler_applied_count_total: 12,
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

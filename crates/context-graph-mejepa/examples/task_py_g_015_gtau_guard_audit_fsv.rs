use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::cli::write_json_0600;
use context_graph_mejepa_tct::{
    run_gtau_guard_audit, GtauGuardAuditConfig, GtauGuardAuditPaths, GtauGuardAuditReport,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const DEFAULT_FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/task-py-g-015-gtau-guard-audit-fsv";
const TENSOR_ROOT: &str = "/zfs/archive/contextgraph/corpus/constellation-tensors-v1";
const CALIBRATION_ROOT: &str = "/zfs/archive/contextgraph/state/calibration/per-slot-v1";
const DEFAULT_POST_FIX_SHIP_GATE_REPORT: &str =
    "/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/ship-gate-fsv.json";
const REPORT_NAME: &str = "gtau_guard_audit_fsv.json";
const AUDIT_NAME: &str = "gtau_guard_audit_report.json";
const READBACK_NAME: &str = "independent_readback.json";
const INVALID_SYNTHETIC_BASELINE_CORRELATION: f64 = 0.866667;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EdgeCase {
    name: String,
    pass: bool,
    observed_error_code: Option<String>,
    evidence: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TaskPyG015FsvReport {
    task_id: String,
    issue: u32,
    status: String,
    all_passed: bool,
    fsv_root: PathBuf,
    run_root: PathBuf,
    source_of_truth: Value,
    post_fix_rebaseline_reference: Value,
    audit_report: GtauGuardAuditReport,
    edge_cases: Vec<EdgeCase>,
    independent_readback: Value,
}

fn main() -> Result<()> {
    let run_id = format!("run-{}", chrono::Utc::now().format("%Y%m%dT%H%M%SZ"));
    let fsv_root = PathBuf::from(
        std::env::var("CG_TASK_PY_G_015_FSV_ROOT").unwrap_or_else(|_| DEFAULT_FSV_ROOT.to_string()),
    );
    let run_root = fsv_root.join(run_id);
    fs::create_dir_all(&run_root).context("create TASK-PY-G-015 FSV run root")?;

    let paths =
        GtauGuardAuditPaths::new(PathBuf::from(TENSOR_ROOT), PathBuf::from(CALIBRATION_ROOT));
    let config = GtauGuardAuditConfig::default();
    let post_fix_rebaseline_reference =
        post_fix_rebaseline_reference().context("read #529 post-fix ship-gate baseline")?;
    let audit = run_gtau_guard_audit(&paths, &config).context("run Gtau guard audit")?;
    ensure!(audit.passes, "audit completeness gates must pass");
    ensure!(
        audit.summary.reported_cell_count == 8 && audit.cell_reports.len() == 8,
        "audit must report all 8 Python mutation cells"
    );
    ensure!(
        !audit.summary.flat_vector_concat_used && audit.summary.slot_identity_preserved,
        "audit must preserve slot identity and reject flat-vector evidence"
    );
    ensure!(
        !audit.summary.ship_gate_countable_after_audit,
        "current #389/#130 state should remain non-countable until low-pass/missing neighborhoods are remediated"
    );
    ensure!(
        audit
            .cell_reports
            .iter()
            .all(|cell| cell.guard_pass_rate.is_some() || !cell.remediation.is_empty()),
        "every cell must have guard counts or explicit remediation"
    );

    let audit_path = run_root.join(AUDIT_NAME);
    write_json_0600(&audit_path, &audit).context("write audit report")?;
    let audit_readback: GtauGuardAuditReport =
        serde_json::from_slice(&fs::read(&audit_path).context("read audit report")?)
            .context("decode audit report readback")?;
    ensure!(audit_readback == audit, "audit report readback mismatch");

    let edge_cases = run_edge_cases(&run_root, &paths).context("run edge cases")?;
    ensure!(
        edge_cases.iter().all(|case| case.pass),
        "one or more Gtau guard audit edge cases failed"
    );

    let independent_readback = independent_readback(&audit_path, &audit_readback)
        .context("independent audit report readback")?;
    let readback_path = run_root.join(READBACK_NAME);
    write_json_0600(&readback_path, &independent_readback).context("write independent readback")?;
    let readback_value: Value =
        serde_json::from_slice(&fs::read(&readback_path).context("read independent readback")?)?;
    ensure!(
        readback_value["allPassed"].as_bool() == Some(true),
        "independent readback must preserve pass state"
    );

    let all_passed = audit.passes
        && audit_readback == audit
        && independent_readback["allPassed"].as_bool() == Some(true)
        && edge_cases.iter().all(|case| case.pass);
    let report = TaskPyG015FsvReport {
        task_id: "TASK-PY-G-015".to_string(),
        issue: 130,
        status: "pass".to_string(),
        all_passed,
        fsv_root: fsv_root.clone(),
        run_root: run_root.clone(),
        source_of_truth: json!({
            "durableRoot": "/zfs/archive/contextgraph",
            "tensorRoot": TENSOR_ROOT,
            "calibrationRoot": CALIBRATION_ROOT,
            "outputRoot": run_root,
            "slotDistributionTensors": format!("{TENSOR_ROOT}/slot_distribution_tensors.jsonl"),
            "pairwiseCorrelationTensors": format!("{TENSOR_ROOT}/pairwise_correlation_tensors.jsonl"),
            "perSlotCalibration": format!("{CALIBRATION_ROOT}/per_slot_calibration.jsonl"),
            "perCellSlotCalibration": format!("{CALIBRATION_ROOT}/per_cell_slot_calibration.jsonl"),
            "oodGuardPolicy": format!("{CALIBRATION_ROOT}/ood_guard_policy.json"),
        }),
        post_fix_rebaseline_reference,
        audit_report: audit,
        edge_cases,
        independent_readback,
    };
    ensure!(report.all_passed, "TASK-PY-G-015 FSV did not pass");
    let report_path = run_root.join(REPORT_NAME);
    write_json_0600(&report_path, &report).context("write FSV report")?;
    let report_readback: TaskPyG015FsvReport =
        serde_json::from_slice(&fs::read(&report_path).context("read FSV report")?)
            .context("decode FSV report readback")?;
    ensure!(
        report_readback == report && report_readback.all_passed,
        "FSV report readback mismatch"
    );

    println!(
        "task-py-g-015 Gtau guard audit FSV allPassed=true report={} report_sha256={} audit={} audit_sha256={} readback={} readback_sha256={}",
        report_path.display(),
        sha256_file(&report_path)?,
        audit_path.display(),
        sha256_file(&audit_path)?,
        readback_path.display(),
        sha256_file(&readback_path)?,
    );
    Ok(())
}

fn run_edge_cases(run_root: &Path, paths: &GtauGuardAuditPaths) -> Result<Vec<EdgeCase>> {
    let mut out = Vec::new();
    let config = GtauGuardAuditConfig::default();

    let empty_tensor_root = run_root.join("edge-empty-tensors");
    fs::create_dir_all(&empty_tensor_root)?;
    fs::write(
        empty_tensor_root.join("slot_distribution_tensors.jsonl"),
        "",
    )?;
    fs::write(
        empty_tensor_root.join("pairwise_correlation_tensors.jsonl"),
        "",
    )?;
    let empty_report = run_gtau_guard_audit(
        &GtauGuardAuditPaths::new(empty_tensor_root, paths.calibration_root.clone()),
        &config,
    )
    .context("empty tensor input edge")?;
    out.push(EdgeCase {
        name: "empty_tensor_input_reports_all_cells_missing".to_string(),
        pass: empty_report.summary.missing_cell_count == 8
            && !empty_report.summary.ship_gate_countable_after_audit
            && empty_report
                .cell_reports
                .iter()
                .all(|cell| !cell.remediation.is_empty()),
        observed_error_code: None,
        evidence: json!({
            "before": {
                "slotDistributionTensorBytes": 0,
                "pairwiseCorrelationTensorBytes": 0,
                "expectedCellCount": 8
            },
            "after": {
                "missingCellCount": empty_report.summary.missing_cell_count,
                "reportedCellCount": empty_report.summary.reported_cell_count,
                "shipGateCountable": empty_report.summary.ship_gate_countable_after_audit,
                "allCellsHaveRemediation": empty_report
                    .cell_reports
                    .iter()
                    .all(|cell| !cell.remediation.is_empty())
            }
        }),
    });

    let missing_paths = GtauGuardAuditPaths::new(
        run_root.join("edge-missing-tensors"),
        paths.calibration_root.clone(),
    );
    let missing_error = run_gtau_guard_audit(&missing_paths, &config)
        .expect_err("missing tensor input must fail closed");
    out.push(EdgeCase {
        name: "missing_tensor_input_fails_closed".to_string(),
        pass: missing_error.code() == "MEJEPA_TCT_IO",
        observed_error_code: Some(missing_error.code().to_string()),
        evidence: json!({
            "before": {
                "tensorRoot": missing_paths.tensor_root,
                "tensorRootExists": false
            },
            "after": {
                "errorCode": missing_error.code(),
                "error": missing_error.to_string(),
                "artifactWritten": false
            }
        }),
    });

    let invalid_tensor_root = run_root.join("edge-invalid-metadata");
    fs::create_dir_all(&invalid_tensor_root)?;
    fs::write(
        invalid_tensor_root.join("slot_distribution_tensors.jsonl"),
        invalid_metadata_row(),
    )?;
    fs::write(
        invalid_tensor_root.join("pairwise_correlation_tensors.jsonl"),
        "",
    )?;
    let invalid_error = run_gtau_guard_audit(
        &GtauGuardAuditPaths::new(invalid_tensor_root, paths.calibration_root.clone()),
        &config,
    )
    .expect_err("invalid language metadata must fail closed");
    out.push(EdgeCase {
        name: "invalid_non_python_metadata_fails_closed".to_string(),
        pass: invalid_error.code() == "MEJEPA_TCT_INVALID_INPUT",
        observed_error_code: Some(invalid_error.code().to_string()),
        evidence: json!({
            "before": {
                "language": "rust",
                "cellKey": "rust:known_good",
                "mutationCategory": "known_good"
            },
            "after": {
                "errorCode": invalid_error.code(),
                "error": invalid_error.to_string(),
                "artifactWritten": false
            }
        }),
    });

    let over_limit_error = run_gtau_guard_audit(
        paths,
        &GtauGuardAuditConfig {
            max_slot_tensor_rows: 1,
            ..GtauGuardAuditConfig::default()
        },
    )
    .expect_err("over-limit slot tensor batch must fail closed");
    out.push(EdgeCase {
        name: "over_limit_slot_tensor_batch_fails_closed".to_string(),
        pass: over_limit_error.code() == "MEJEPA_TCT_INVALID_INPUT",
        observed_error_code: Some(over_limit_error.code().to_string()),
        evidence: json!({
            "before": {
                "maxSlotTensorRows": 1,
                "sourceTensorRoot": paths.tensor_root,
                "expectedRealTensorRows": "greater_than_1"
            },
            "after": {
                "errorCode": over_limit_error.code(),
                "error": over_limit_error.to_string(),
                "artifactWritten": false
            }
        }),
    });

    Ok(out)
}

fn post_fix_rebaseline_reference() -> Result<Value> {
    let path = PathBuf::from(
        std::env::var("CG_TASK_PY_G_015_POST_FIX_SHIP_GATE_REPORT")
            .unwrap_or_else(|_| DEFAULT_POST_FIX_SHIP_GATE_REPORT.to_string()),
    );
    let path_text = path.display().to_string();
    ensure!(
        path_text.starts_with("/zfs/archive/contextgraph/"),
        "post-fix ship-gate report must be aiwonder durable evidence under /zfs/archive/contextgraph, got {}",
        path_text
    );
    let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
    let contains_forbidden_baseline = bytes
        .windows(b"0.866667".len())
        .any(|window| window == b"0.866667")
        || bytes
            .windows(b"0.8666666746139526".len())
            .any(|window| window == b"0.8666666746139526");
    ensure!(
        !contains_forbidden_baseline,
        "post-fix ship-gate report still contains the invalid synthetic-vector baseline"
    );
    let decoded: Value =
        serde_json::from_slice(&bytes).context("decode post-fix ship-gate report")?;
    ensure!(
        decoded["all_passed"].as_bool() == Some(true),
        "post-fix ship-gate report must have all_passed=true"
    );
    let execute = decoded
        .get("execute_and_inspect")
        .and_then(Value::as_object)
        .context("post-fix ship-gate report missing execute_and_inspect")?;
    let final_metrics = execute
        .get("final_metrics")
        .and_then(Value::as_object)
        .context("post-fix ship-gate report missing final_metrics")?;
    let rank_safe_metrics = execute
        .get("rank_safe_final_metrics")
        .and_then(Value::as_object)
        .context("post-fix ship-gate report missing rank_safe_final_metrics")?;
    let raw_metrics = execute
        .get("raw_final_metrics")
        .and_then(Value::as_object)
        .context("post-fix ship-gate report missing raw_final_metrics")?;
    let raw_corr = required_metric(raw_metrics, "prediction_oracle_correlation")?;
    let rank_safe_corr = required_metric(rank_safe_metrics, "prediction_oracle_correlation")?;
    let final_corr = required_metric(final_metrics, "prediction_oracle_correlation")?;
    reject_invalid_synthetic_baseline("raw", raw_corr)?;
    reject_invalid_synthetic_baseline("rank-safe", rank_safe_corr)?;
    reject_invalid_synthetic_baseline("rank-lift", final_corr)?;

    Ok(json!({
        "artifactPath": path,
        "artifactSha256": format!("sha256:{}", hex::encode(Sha256::digest(&bytes))),
        "allPassed": true,
        "containsForbiddenSyntheticBaseline": false,
        "taskId": decoded["task_id"],
        "rawPredictionOracleCorrelation": raw_corr,
        "rankSafePredictionOracleCorrelation": rank_safe_corr,
        "rankLiftPredictionOracleCorrelation": final_corr,
        "rankLiftBrier": required_metric(final_metrics, "brier")?,
        "rankLiftEce": required_metric(final_metrics, "ece")?,
        "shipGatePassed": execute.get("ship_gate_passed").cloned().unwrap_or(Value::Null),
        "shipGateBlockers": execute.get("ship_gate_blockers").cloned().unwrap_or(Value::Null),
        "rollingWindowBlockerCount": execute
            .get("rolling_window_blockers")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
        "rollingWindowPowerBlockerCount": execute
            .get("rolling_window_statistical_power_blockers")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
    }))
}

fn required_metric(metrics: &serde_json::Map<String, Value>, key: &str) -> Result<f64> {
    metrics
        .get(key)
        .and_then(Value::as_f64)
        .with_context(|| format!("post-fix ship-gate metric {key} missing or non-numeric"))
}

fn reject_invalid_synthetic_baseline(metric_label: &str, value: f64) -> Result<()> {
    ensure!(
        (value - INVALID_SYNTHETIC_BASELINE_CORRELATION).abs() > 0.0001,
        "post-fix {metric_label} correlation is indistinguishable from the invalid synthetic baseline"
    );
    Ok(())
}

fn invalid_metadata_row() -> String {
    json!({
        "artifact_kind": "python_constellation_slot_distribution_tensor",
        "cell_key": "rust:known_good",
        "language": "rust",
        "mutation_category": "known_good",
        "slot_id": "e1",
        "embedder_id": "e1",
        "slot_identity_preserved": true,
        "flat_vector_concat_used": false,
        "valid": true,
        "distance_mean": 0.1,
        "distance_max": 0.2,
        "variance_mean": 0.001
    })
    .to_string()
        + "\n"
}

fn independent_readback(path: &Path, report: &GtauGuardAuditReport) -> Result<Value> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let decoded: GtauGuardAuditReport =
        serde_json::from_slice(&bytes).context("decode audit report from independent readback")?;
    let cell_keys = decoded
        .cell_reports
        .iter()
        .map(|cell| cell.cell_key.clone())
        .collect::<Vec<_>>();
    let all_passed = decoded == *report
        && decoded.passes
        && decoded.summary.reported_cell_count == 8
        && !decoded.summary.ship_gate_countable_after_audit
        && !decoded.summary.flat_vector_concat_used
        && decoded.summary.slot_identity_preserved;
    Ok(json!({
        "allPassed": all_passed,
        "artifactPath": path,
        "artifactSha256": format!("sha256:{}", hex::encode(Sha256::digest(&bytes))),
        "reportedCellCount": decoded.summary.reported_cell_count,
        "missingCellCount": decoded.summary.missing_cell_count,
        "lowPassCellCount": decoded.summary.low_pass_cell_count,
        "failClosedCellCount": decoded.summary.fail_closed_cell_count,
        "shipGateCountableAfterAudit": decoded.summary.ship_gate_countable_after_audit,
        "slotIdentityPreserved": decoded.summary.slot_identity_preserved,
        "flatVectorConcatUsed": decoded.summary.flat_vector_concat_used,
        "cellKeys": cell_keys,
        "missingNeighborhoodCount": decoded.missing_reward_signal_neighborhoods.len()
    }))
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
}

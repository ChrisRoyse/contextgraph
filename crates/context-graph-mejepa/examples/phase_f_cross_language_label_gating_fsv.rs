use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use context_graph_mejepa::cross_language_label_gating::{
    label_can_transfer, LabelTransferDecision, LabelTransferError, LabelTransferQuery,
    RejectReason, CORRELATION_THRESHOLD, SIMILARITY_THRESHOLD, TRANSFERRED_LABEL_WEIGHT,
};
use context_graph_mejepa::{Language, RootCauseClass};
use serde_json::json;
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/phase-f-cross-language-label-gating-fsv";
const REPORT_FILE: &str = "cross_language_label_gating_fsv.json";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let started_at_unix_ms = chrono::Utc::now().timestamp_millis();
    let fsv_root = PathBuf::from(FSV_ROOT);
    fs::create_dir_all(&fsv_root)?;
    let run_root = fsv_root.join(format!("run-{}-{}", started_at_unix_ms, std::process::id()));
    fs::create_dir_all(&run_root)?;

    let happy_query = query(false, true, 0.86, 0.90);
    let happy_decision = label_can_transfer(&happy_query)?;
    let decision_path = run_root.join("allowed-transfer-decision.json");
    write_json_readback(
        &decision_path,
        &json!({
            "query": happy_query,
            "decision": happy_decision,
            "thresholds": {
                "similarity_must_be_greater_than": SIMILARITY_THRESHOLD,
                "correlation_at_least": CORRELATION_THRESHOLD,
                "transfer_weight": TRANSFERRED_LABEL_WEIGHT
            }
        }),
    )?;
    let decision_readback: serde_json::Value = serde_json::from_slice(&fs::read(&decision_path)?)?;
    let happy_pass = happy_decision
        == (LabelTransferDecision::Transfer {
            weight: TRANSFERRED_LABEL_WEIGHT,
        })
        && decision_readback["decision"]["transfer"]["weight"] == json!(TRANSFERRED_LABEL_WEIGHT);

    let boundary_cases = vec![
        same_language_full_weight()?,
        exact_similarity_threshold_rejects()?,
        root_cause_mismatch_rejects()?,
        low_correlation_rejects()?,
        nan_similarity_fails_closed()?,
        out_of_range_correlation_fails_closed()?,
    ];
    let all_boundaries_pass = boundary_cases
        .iter()
        .all(|case| case["pass"].as_bool().unwrap_or(false));
    let evidence_path = fsv_root.join(REPORT_FILE);
    let evidence = json!({
        "fsv_root": FSV_ROOT,
        "task_id": "TASK-PREDICT-LABEL-011",
        "started_at_unix_ms": started_at_unix_ms,
        "build_release_sha": git_head_short()?,
        "run_root": run_root,
        "happy_path": [{
            "case": "python_to_rust_high_similarity_same_root_transfers",
            "sot": decision_path,
            "before": null,
            "trigger": "cargo run --release -p context-graph-mejepa --example phase_f_cross_language_label_gating_fsv",
            "after": decision_readback,
            "expected": {
                "decision": "transfer",
                "weight": TRANSFERRED_LABEL_WEIGHT,
                "similarity_gt": SIMILARITY_THRESHOLD,
                "correlation_gte": CORRELATION_THRESHOLD
            },
            "actual": happy_decision,
            "pass": happy_pass,
            "evidence_path": decision_path
        }],
        "boundary_cases": boundary_cases,
        "all_passed": happy_pass && all_boundaries_pass,
        "cf_counts_before": {"decisionEvidenceFiles": 0},
        "cf_counts_after": {"decisionEvidenceFiles": 1},
        "readback_equal": happy_pass,
        "physical_artifacts": {
            "decision_file": artifact_metadata(&decision_path)?,
            "fsv_artifact": {
                "path": evidence_path.display().to_string(),
                "will_exist_after_write": true
            }
        }
    });

    if !evidence["all_passed"].as_bool().unwrap_or(false) {
        return Err("TASK-PREDICT-LABEL-011 FSV evidence did not pass".into());
    }
    write_json_readback(&evidence_path, &evidence)?;
    let final_readback: serde_json::Value = serde_json::from_slice(&fs::read(&evidence_path)?)?;
    if final_readback["all_passed"] != json!(true)
        || final_readback["readback_equal"] != json!(true)
    {
        return Err("cross-language label gating FSV artifact readback failed".into());
    }
    println!("{}", serde_json::to_string_pretty(&final_readback)?);
    Ok(())
}

fn same_language_full_weight() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let decision = label_can_transfer(&query(true, true, 0.10, -0.10))?;
    Ok(json!({
        "case": "same_language_label_not_transferred",
        "sot": "label_can_transfer",
        "expected": {"decision": "same_language", "weight": 1.0},
        "actual": decision,
        "pass": decision == (LabelTransferDecision::SameLanguage { weight: 1.0 })
    }))
}

fn exact_similarity_threshold_rejects() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let decision = label_can_transfer(&query(false, true, SIMILARITY_THRESHOLD, 0.90))?;
    Ok(json!({
        "case": "exact_similarity_threshold_rejects",
        "sot": "label_can_transfer",
        "expected": "similarity must be strictly greater than threshold",
        "actual": decision,
        "pass": decision == (LabelTransferDecision::Reject {
            reason: RejectReason::SimilarityBelowThreshold,
        })
    }))
}

fn root_cause_mismatch_rejects() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let decision = label_can_transfer(&query(false, false, 0.95, 0.90))?;
    Ok(json!({
        "case": "root_cause_mismatch",
        "sot": "label_can_transfer",
        "expected": "root_cause_mismatch",
        "actual": decision,
        "pass": decision == (LabelTransferDecision::Reject {
            reason: RejectReason::RootCauseMismatch,
        })
    }))
}

fn low_correlation_rejects() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let decision = label_can_transfer(&query(false, true, 0.95, CORRELATION_THRESHOLD - 0.01))?;
    Ok(json!({
        "case": "correlation_below_threshold",
        "sot": "label_can_transfer",
        "expected": "correlation_below_threshold",
        "actual": decision,
        "pass": decision == (LabelTransferDecision::Reject {
            reason: RejectReason::CorrelationBelowThreshold,
        })
    }))
}

fn nan_similarity_fails_closed() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let err = label_can_transfer(&query(false, true, f32::NAN, 0.90)).unwrap_err();
    Ok(json!({
        "case": "nan_similarity",
        "sot": "LabelTransferError",
        "expected": "MEJEPA_LABEL_TRANSFER_SIMILARITY_NOT_FINITE",
        "actual": error_code(&err),
        "pass": matches!(err, LabelTransferError::SimilarityNotFinite)
    }))
}

fn out_of_range_correlation_fails_closed() -> Result<serde_json::Value, Box<dyn std::error::Error>>
{
    let err = label_can_transfer(&query(false, true, 0.95, -1.01)).unwrap_err();
    Ok(json!({
        "case": "correlation_out_of_range",
        "sot": "LabelTransferError",
        "expected": "MEJEPA_LABEL_TRANSFER_CORRELATION_OUT_OF_RANGE",
        "actual": error_code(&err),
        "pass": matches!(err, LabelTransferError::CorrelationOutOfRange(_))
    }))
}

fn query(
    same_language: bool,
    same_root_cause: bool,
    similarity: f32,
    correlation: f32,
) -> LabelTransferQuery {
    LabelTransferQuery {
        source_language: Language::Python,
        target_language: if same_language {
            Language::Python
        } else {
            Language::Rust
        },
        source_root_cause: RootCauseClass::LogicError,
        target_root_cause: if same_root_cause {
            RootCauseClass::LogicError
        } else {
            RootCauseClass::EnvironmentError
        },
        similarity,
        source_cross_language_correlation: correlation,
    }
}

fn error_code(err: &LabelTransferError) -> &'static str {
    match err {
        LabelTransferError::SimilarityNotFinite => "MEJEPA_LABEL_TRANSFER_SIMILARITY_NOT_FINITE",
        LabelTransferError::SimilarityOutOfRange(_) => {
            "MEJEPA_LABEL_TRANSFER_SIMILARITY_OUT_OF_RANGE"
        }
        LabelTransferError::CorrelationNotFinite => "MEJEPA_LABEL_TRANSFER_CORRELATION_NOT_FINITE",
        LabelTransferError::CorrelationOutOfRange(_) => {
            "MEJEPA_LABEL_TRANSFER_CORRELATION_OUT_OF_RANGE"
        }
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

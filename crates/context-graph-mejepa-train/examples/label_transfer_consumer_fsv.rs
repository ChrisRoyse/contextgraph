use context_graph_mejepa::cross_language_label_gating::{
    LabelTransferDecision, RejectReason, TRANSFERRED_LABEL_WEIGHT,
};
use context_graph_mejepa::{
    label_transfer_decision_key, load_label_transfer_decision, open_infer_rocksdb,
    summarize_label_transfer_decisions, ChunkId, FailureModeClass, Language, PredictionId,
    RootCauseClass, TestId,
};
use context_graph_mejepa_cf::CF_MEJEPA_LABEL_TRANSFER_DECISIONS;
use context_graph_mejepa_train::labels::{
    apply_phase_b_labels_with_transfer_gate, ExtractedPhaseBLabels, FailureModeLabel, LabelSource,
    LabelTransferApplicationContext,
};
use rocksdb::{IteratorMode, DB};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {
    let fsv_root = PathBuf::from("/zfs/archive/contextgraph/fsv/phase-f-label-transfer-gating-fsv");
    fs::create_dir_all(&fsv_root)?;
    let run_root = fsv_root.join(format!(
        "run-{}-{}",
        chrono::Utc::now().timestamp_millis(),
        std::process::id()
    ));
    fs::create_dir_all(&run_root)?;
    let db_path = run_root.join("infer-db");
    let db = open_infer_rocksdb(&db_path)?;

    let counts_before = cf_count(db.as_ref(), CF_MEJEPA_LABEL_TRANSFER_DECISIONS)?;
    let low_similarity = apply_case(
        db.as_ref(),
        "pytest-low-similarity",
        Language::Python,
        Language::Rust,
        RootCauseClass::LogicError,
        0.10,
        0.90,
        1,
    )?;
    let high_similarity = apply_case(
        db.as_ref(),
        "pytest-high-similarity",
        Language::Python,
        Language::Rust,
        RootCauseClass::LogicError,
        0.95,
        0.90,
        2,
    )?;
    let same_language = apply_case(
        db.as_ref(),
        "pytest-same-language",
        Language::Python,
        Language::Python,
        RootCauseClass::LogicError,
        0.10,
        -0.90,
        3,
    )?;
    let root_mismatch = apply_case(
        db.as_ref(),
        "pytest-root-mismatch",
        Language::Python,
        Language::Rust,
        RootCauseClass::EnvironmentError,
        0.95,
        0.90,
        4,
    )?;
    flush_cf(db.as_ref(), CF_MEJEPA_LABEL_TRANSFER_DECISIONS)?;
    let summary_after = summarize_label_transfer_decisions(db.as_ref())?;
    let records_after = [
        &low_similarity,
        &high_similarity,
        &same_language,
        &root_mismatch,
    ]
    .iter()
    .map(|applied| {
        applied
            .transfer_decisions
            .first()
            .cloned()
            .ok_or("MEJEPA_LABEL_TRANSFER_FSV_MISSING_DECISION")
    })
    .collect::<std::result::Result<Vec<_>, _>>()?;
    let keys = records_after
        .iter()
        .map(label_transfer_decision_key)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let counts_after = cf_count(db.as_ref(), CF_MEJEPA_LABEL_TRANSFER_DECISIONS)?;

    drop(db);
    let reopened = open_infer_rocksdb(&db_path)?;
    let reopened_summary = summarize_label_transfer_decisions(reopened.as_ref())?;
    let reopened_records = keys
        .iter()
        .map(|key| {
            load_label_transfer_decision(reopened.as_ref(), key)?
                .ok_or_else(|| "MEJEPA_LABEL_TRANSFER_FSV_REOPENED_ROW_MISSING".into())
        })
        .collect::<Result<Vec<_>>>()?;
    let reopened_count = cf_count(reopened.as_ref(), CF_MEJEPA_LABEL_TRANSFER_DECISIONS)?;
    let physical_artifacts = physical_artifacts(&run_root)?;
    let readback_equal = reopened_summary == summary_after
        && reopened_records == records_after
        && reopened_count == counts_after;

    let happy_path = vec![
        json!({
            "case": "high_similarity_python_to_rust_transfers_at_half_weight",
            "sot": CF_MEJEPA_LABEL_TRANSFER_DECISIONS,
            "trigger": "apply_phase_b_labels_with_transfer_gate",
            "expected": {"decision": "transfer", "weight": TRANSFERRED_LABEL_WEIGHT},
            "actual": {
                "accepted_failure_modes": high_similarity.accepted_failure_modes.len(),
                "rejected_failure_modes": high_similarity.rejected_failure_modes.len(),
                "decision": high_similarity.transfer_decisions[0].decision,
                "effective_weight": high_similarity.transfer_decisions[0].applied_weight
            },
            "pass": high_similarity.accepted_failure_modes.len() == 1
                && high_similarity.rejected_failure_modes.is_empty()
                && high_similarity.transfer_decisions[0].decision == (LabelTransferDecision::Transfer { weight: TRANSFERRED_LABEL_WEIGHT })
                && approx_eq(high_similarity.transfer_decisions[0].applied_weight, TRANSFERRED_LABEL_WEIGHT)
        }),
        json!({
            "case": "weekly_summary_renders_accept_reject_ratios_per_language_pair",
            "sot": CF_MEJEPA_LABEL_TRANSFER_DECISIONS,
            "trigger": "summarize_label_transfer_decisions",
            "expected": {"python_to_rust_total": 3, "accepted": 1, "rejected": 2},
            "actual": summary_after,
            "pass": summary_after.total_rows == 4
                && summary_after.transfer_count == 1
                && summary_after.reject_count == 2
                && summary_after.same_language_count == 1
                && summary_after.pair_summaries.iter().any(|pair| {
                    pair.source_language == Language::Python
                        && pair.target_language == Language::Rust
                        && pair.total_count == 3
                        && pair.accepted_count == 1
                        && pair.reject_count == 2
                        && approx_eq(pair.accept_ratio, 1.0 / 3.0)
                        && approx_eq(pair.reject_ratio, 2.0 / 3.0)
                })
        }),
    ];
    let boundary_cases = vec![
        json!({
            "case": "low_similarity_python_to_rust_rejected_and_recorded",
            "sot": CF_MEJEPA_LABEL_TRANSFER_DECISIONS,
            "expected": "reject similarity_below_threshold",
            "actual": {
                "accepted_failure_modes": low_similarity.accepted_failure_modes.len(),
                "rejected_failure_modes": low_similarity.rejected_failure_modes.len(),
                "decision": low_similarity.transfer_decisions[0].decision,
                "effective_weight": low_similarity.transfer_decisions[0].applied_weight
            },
            "pass": low_similarity.accepted_failure_modes.is_empty()
                && low_similarity.rejected_failure_modes.len() == 1
                && low_similarity.transfer_decisions[0].decision == (LabelTransferDecision::Reject { reason: RejectReason::SimilarityBelowThreshold })
                && low_similarity.transfer_decisions[0].applied_weight == 0.0
        }),
        json!({
            "case": "same_language_label_accepted_at_full_weight",
            "sot": CF_MEJEPA_LABEL_TRANSFER_DECISIONS,
            "expected": "same_language weight 1.0",
            "actual": {
                "accepted_failure_modes": same_language.accepted_failure_modes.len(),
                "decision": same_language.transfer_decisions[0].decision,
                "effective_weight": same_language.transfer_decisions[0].applied_weight
            },
            "pass": same_language.accepted_failure_modes.len() == 1
                && same_language.transfer_decisions[0].decision == (LabelTransferDecision::SameLanguage { weight: 1.0 })
                && same_language.transfer_decisions[0].applied_weight == 1.0
        }),
        json!({
            "case": "root_cause_mismatch_rejected_and_recorded",
            "sot": CF_MEJEPA_LABEL_TRANSFER_DECISIONS,
            "expected": "reject root_cause_mismatch",
            "actual": {
                "accepted_failure_modes": root_mismatch.accepted_failure_modes.len(),
                "rejected_failure_modes": root_mismatch.rejected_failure_modes.len(),
                "decision": root_mismatch.transfer_decisions[0].decision
            },
            "pass": root_mismatch.accepted_failure_modes.is_empty()
                && root_mismatch.rejected_failure_modes.len() == 1
                && root_mismatch.transfer_decisions[0].decision == (LabelTransferDecision::Reject { reason: RejectReason::RootCauseMismatch })
        }),
        json!({
            "case": "rocksdb_reopen_readback_equal",
            "sot": CF_MEJEPA_LABEL_TRANSFER_DECISIONS,
            "expected": {"rows": 4, "record_readback_equal": true},
            "actual": {
                "counts_before": counts_before,
                "counts_after": counts_after,
                "reopened_count": reopened_count,
                "readback_equal": readback_equal
            },
            "pass": counts_before == 0 && counts_after == 4 && reopened_count == 4 && readback_equal
        }),
    ];
    let cases_passed = happy_path
        .iter()
        .chain(boundary_cases.iter())
        .all(|case| case["pass"].as_bool() == Some(true));
    let all_passed = cases_passed && !physical_artifacts.is_empty();
    let report = json!({
        "fsv_root": fsv_root.display().to_string(),
        "task_id": "TASK-PREDICT-LABEL-012",
        "build_release_sha": git_head_short(),
        "source_of_truth": {
            "db_path": db_path.display().to_string(),
            "decision_cf": CF_MEJEPA_LABEL_TRANSFER_DECISIONS
        },
        "happy_path": happy_path,
        "boundary_cases": boundary_cases,
        "counts_before": counts_before,
        "counts_after": counts_after,
        "reopened_count": reopened_count,
        "reopened_summary": reopened_summary,
        "physical_artifacts": physical_artifacts,
        "readback_equal": readback_equal,
        "all_passed": all_passed
    });
    let artifact_path = fsv_root.join("label_transfer_consumer_fsv.json");
    write_report_readback(&artifact_path, &report)?;
    let readback: serde_json::Value = serde_json::from_slice(&fs::read(&artifact_path)?)?;
    if readback != report {
        return Err("MEJEPA_LABEL_TRANSFER_FSV_REPORT_READBACK_MISMATCH".into());
    }
    if !all_passed {
        return Err("MEJEPA_LABEL_TRANSFER_FSV_FAILED".into());
    }
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn apply_case(
    db: &DB,
    label_source_id: &str,
    source_language: Language,
    target_language: Language,
    target_root_cause: RootCauseClass,
    similarity: f32,
    correlation: f32,
    seed: u8,
) -> Result<context_graph_mejepa_train::labels::AppliedPhaseBLabels> {
    let labels = ExtractedPhaseBLabels {
        source: LabelSource::Pytest,
        failure_modes: vec![FailureModeLabel {
            test_id: Some(TestId(format!("tests/test_{seed}.py::test_case"))),
            failure_class: FailureModeClass::AssertionMismatch,
            root_cause_class: RootCauseClass::LogicError,
            evidence_line: format!("FAILED tests/test_{seed}.py::test_case - AssertionError"),
        }],
        failed_tests: Vec::new(),
        edge_cases: Vec::new(),
        latent_bugs: Vec::new(),
        security_concerns: Vec::new(),
        reasoning_class: context_graph_mejepa::ReasoningClass::PlausibleButWrong,
        touched_files: vec![PathBuf::from(format!("tests/test_{seed}.py"))],
    };
    Ok(apply_phase_b_labels_with_transfer_gate(
        db,
        &labels,
        &LabelTransferApplicationContext {
            prediction_id: PredictionId([seed; 16]),
            label_source_id: label_source_id.to_string(),
            source_language,
            target_language,
            source_chunk_id: ChunkId(format!("python/source_{seed}.py#fn")),
            target_chunk_id: ChunkId(format!("rust/target_{seed}.rs#fn")),
            target_root_cause,
            similarity,
            source_cross_language_correlation: correlation,
            created_at_unix_ms: 1_778_700_000_000 + i64::from(seed),
        },
    )?)
}

fn cf_count(db: &DB, cf_name: &str) -> Result<usize> {
    let cf = db
        .cf_handle(cf_name)
        .ok_or("MEJEPA_LABEL_TRANSFER_FSV_CF_MISSING")?;
    let mut count = 0_usize;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        item?;
        count += 1;
    }
    Ok(count)
}

fn flush_cf(db: &DB, cf_name: &str) -> Result<()> {
    let cf = db
        .cf_handle(cf_name)
        .ok_or("MEJEPA_LABEL_TRANSFER_FSV_CF_MISSING")?;
    db.flush_cf(cf)?;
    Ok(())
}

fn physical_artifacts(root: &Path) -> Result<Vec<serde_json::Value>> {
    let mut out = Vec::new();
    collect_artifacts(root, &mut out)?;
    Ok(out)
}

fn collect_artifacts(path: &Path, out: &mut Vec<serde_json::Value>) -> Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_artifacts(&path, out)?;
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) == Some("sst")
            || path.file_name().and_then(|name| name.to_str()) == Some("CURRENT")
        {
            let bytes = fs::read(&path)?;
            out.push(json!({
                "path": path,
                "bytes": bytes.len(),
                "sha256": hex::encode(Sha256::digest(&bytes)),
            }));
        }
    }
    Ok(())
}

fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.0001
}

fn write_report_readback(path: &Path, report: &serde_json::Value) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(report)?;
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(())
}

fn git_head_short() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

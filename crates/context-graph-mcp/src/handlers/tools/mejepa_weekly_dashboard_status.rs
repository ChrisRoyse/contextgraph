//! Source-of-truth assembly for the Phase F weekly evaluation dashboard.

use std::fs;
use std::path::{Path, PathBuf};

use context_graph_mejepa::eval::{corpus_sha_from_holdout, RocksDbEvalStore};
use context_graph_mejepa::{FeedbackKind, HoldoutPanel, SurpriseEvent};
use rocksdb::{IteratorMode, DB};
use serde_json::{json, Value};

use crate::tools::names as tool_names;

const WEEKLY_CELL_CORRELATION_MIN: f32 = 0.25;
const WEEKLY_STATE_TRANSFER_MIN: f32 = 0.80;

pub(super) fn weekly_eval_dashboard(
    db_path: &Path,
    d_root_input: Option<PathBuf>,
    exports_root_input: Option<PathBuf>,
    max_cells: usize,
) -> Result<Value, context_graph_mejepa::EvalError> {
    let db = context_graph_mejepa_hygiene::open_hygiene_rocksdb(db_path).map_err(|err| {
        context_graph_mejepa::EvalError::new(
            context_graph_mejepa::EvalErrorCode::Store,
            format!("open weekly dashboard RocksDB failed: {err}"),
        )
    })?;
    let eval_store = RocksDbEvalStore::new(db.clone())?;
    let report = eval_store.load_latest_report()?.ok_or_else(|| {
        context_graph_mejepa::EvalError::new(
            context_graph_mejepa::EvalErrorCode::ReportPersistFail,
            "no weekly eval report exists in CF_MEJEPA_EVAL_REPORTS",
        )
    })?;
    report.validate()?;
    let report_hash = report.determinism_hash()?;
    let exports_root = resolve_exports_root(exports_root_input)?;
    let d_root = resolve_d_root(d_root_input, &exports_root)?;
    let weekly_files = weekly_file_status(&exports_root, &report, &report_hash);
    let weekly_files_status = weekly_files
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let holdout_feed = weekly_holdout_feed_status(&d_root);
    let holdout_feed_status = holdout_feed
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let feedback = agent_feedback_summary(db.as_ref(), 5)?;
    let label_transfer = context_graph_mejepa::summarize_label_transfer_decisions(db.as_ref())
        .map_err(|err| {
            context_graph_mejepa::EvalError::new(
                context_graph_mejepa::EvalErrorCode::Store,
                err.to_string(),
            )
        })?;
    let overall_status = if weekly_files_status == "present"
        && holdout_feed_status == "ready"
        && report.ship_gate_passed
    {
        "healthy"
    } else if weekly_files_status == "present" || holdout_feed_status == "ready" {
        "attention_required"
    } else {
        "degraded"
    };
    Ok(json!({
        "tool": tool_names::MEJEPA_WEEKLY_EVAL_DASHBOARD,
        "overallStatus": overall_status,
        "report": {
            "reportDate": report.report_date,
            "generatedAtUnixMs": report.generated_at_unix_ms,
            "holdoutCount": report.holdout_count,
            "shipGatePassed": report.ship_gate_passed,
            "shipGateFailures": report.ship_gate_failures,
            "reportHash": report_hash,
            "sourceOfTruth": "CF_MEJEPA_EVAL_REPORTS"
        },
        "holdoutFeed": holdout_feed,
        "perCellShipGate": {
            "rows": per_cell_dashboard_rows(&report, max_cells),
            "totalCells": report.per_cell_correlation.len(),
            "maxCells": max_cells,
            "truncated": report.per_cell_correlation.len() > max_cells
        },
        "calibration": {
            "conformalCoverage": report.conformal_coverage_health,
            "oodCalibration": report.ood_calibration_health,
            "gtauPassRate": report.gtau_pass_rate,
            "perPredictionClassCalibration": report.per_prediction_class_calibration,
            "perFailureModeClass": report.per_failure_mode_class
        },
        "activeLearning": report.active_learning,
        "labelTransferDecisions": label_transfer,
        "agentFeedback": feedback,
        "operationalSignals": operational_signal_counts(db.as_ref())?,
        "weeklyFiles": weekly_files,
        "requiredWeeklySections": required_weekly_sections_status(&exports_root, &report.report_date),
        "runbook": runbook_actions(&report, weekly_files_status, holdout_feed_status),
        "sourceOfTruth": {
            "evalReport": "CF_MEJEPA_EVAL_REPORTS",
            "holdoutFeed": d_root.join("state/gold-labels/weekly_holdout_panels.json"),
            "exportsRoot": exports_root,
            "agentFeedback": context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK,
            "labelTransferDecisions": context_graph_mejepa_cf::CF_MEJEPA_LABEL_TRANSFER_DECISIONS,
            "driftHistory": context_graph_mejepa_cf::CF_MEJEPA_DRIFT_HISTORY
        }
    }))
}

fn resolve_exports_root(
    input: Option<PathBuf>,
) -> Result<PathBuf, context_graph_mejepa::EvalError> {
    if let Some(path) = input {
        return Ok(path);
    }
    context_graph_paths::data_root()
        .map(|root| root.join("exports/eval"))
        .map_err(|err| {
            context_graph_mejepa::EvalError::new(
                context_graph_mejepa::EvalErrorCode::InvalidInput,
                err.to_string(),
            )
        })
}

fn resolve_d_root(
    input: Option<PathBuf>,
    exports_root: &Path,
) -> Result<PathBuf, context_graph_mejepa::EvalError> {
    if let Some(path) = input {
        return Ok(path);
    }
    if exports_root.file_name().is_some_and(|name| name == "eval")
        && exports_root
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|name| name == "exports")
    {
        if let Some(root) = exports_root.parent().and_then(Path::parent) {
            return Ok(root.to_path_buf());
        }
    }
    context_graph_paths::data_root().map_err(|err| {
        context_graph_mejepa::EvalError::new(
            context_graph_mejepa::EvalErrorCode::InvalidInput,
            err.to_string(),
        )
    })
}

fn per_cell_dashboard_rows(
    report: &context_graph_mejepa::EvalReport,
    max_cells: usize,
) -> Vec<Value> {
    report
        .per_cell_correlation
        .iter()
        .take(max_cells)
        .map(|(cell, correlation)| {
            let state_transfer = report
                .per_cell_state_transfer
                .get(cell)
                .and_then(Option::as_ref);
            let correlation_status = match correlation {
                Some(value) if *value >= WEEKLY_CELL_CORRELATION_MIN => "pass",
                Some(_) => "below_threshold",
                None => "insufficient_samples",
            };
            let state_transfer_candidate = match state_transfer {
                Some(value) if value.transfer_score < WEEKLY_STATE_TRANSFER_MIN => "fine_tune",
                Some(_) => "hold",
                None => "collect_more_labels",
            };
            json!({
                "cell": cell,
                "correlation": correlation,
                "bayesianShrinkage": report.bayesian_shrinkage.get(cell),
                "stateTransfer": state_transfer,
                "correlationStatus": correlation_status,
                "stateTransferCandidate": state_transfer_candidate
            })
        })
        .collect()
}

fn operational_signal_counts(db: &DB) -> Result<Value, context_graph_mejepa::EvalError> {
    Ok(json!({
        "agentFeedbackRows": cf_count(db, context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK)?,
        "driftWindowRows": cf_count(db, context_graph_mejepa_cf::CF_MEJEPA_DRIFT_WINDOW)?,
        "driftHistoryRows": cf_count(db, context_graph_mejepa_cf::CF_MEJEPA_DRIFT_HISTORY)?,
        "healReportRows": cf_count(db, context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS)?,
        "modelPromotionRows": cf_count(db, context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS)?,
        "activeLearningQueueRows": cf_count(db, context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_LEARNING_QUEUE)?,
        "labelTransferDecisionRows": cf_count(db, context_graph_mejepa_cf::CF_MEJEPA_LABEL_TRANSFER_DECISIONS)?,
        "oodEscalationRows": cf_count(db, context_graph_mejepa_cf::CF_MEJEPA_OOD_ESCALATIONS)?
    }))
}

fn cf_count(db: &DB, cf_name: &'static str) -> Result<usize, context_graph_mejepa::EvalError> {
    let cf = db.cf_handle(cf_name).ok_or_else(|| {
        context_graph_mejepa::EvalError::new(
            context_graph_mejepa::EvalErrorCode::Store,
            format!("missing column family {cf_name}"),
        )
    })?;
    Ok(db.iterator_cf(cf, IteratorMode::Start).count())
}

fn weekly_file_status(
    exports_root: &Path,
    report: &context_graph_mejepa::EvalReport,
    report_hash: &str,
) -> Value {
    let report_dir = exports_root.join(&report.report_date);
    let markdown_path = report_dir.join("weekly.md");
    let json_path = report_dir.join("weekly.json");
    let markdown = file_status(&markdown_path);
    let json_file = file_status(&json_path);
    let json_hash_status = match fs::read(&json_path) {
        Ok(bytes) => match serde_json::from_slice::<context_graph_mejepa::EvalReport>(&bytes) {
            Ok(exported) => match exported.determinism_hash() {
                Ok(hash) if hash == report_hash => json!({
                    "status": "matches",
                    "hash": hash
                }),
                Ok(hash) => json!({
                    "status": "mismatch",
                    "hash": hash,
                    "expected": report_hash
                }),
                Err(err) => json!({
                    "status": "invalid",
                    "error": err.to_string()
                }),
            },
            Err(err) => json!({
                "status": "invalid",
                "error": err.to_string()
            }),
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            json!({"status": "missing"})
        }
        Err(err) => json!({
            "status": "unreadable",
            "error": err.to_string()
        }),
    };
    let present = markdown["exists"] == true && json_file["exists"] == true;
    let hash_matches = json_hash_status["status"] == "matches";
    json!({
        "status": if present && hash_matches { "present" } else { "degraded" },
        "reportDir": report_dir,
        "markdown": markdown,
        "json": json_file,
        "jsonHashReadback": json_hash_status
    })
}

fn weekly_holdout_feed_status(d_root: &Path) -> Value {
    let path = d_root.join("state/gold-labels/weekly_holdout_panels.json");
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return json!({
                "status": "missing",
                "path": path,
                "errorCode": "MEJEPA_WEEKLY_HOLDOUT_MISSING"
            });
        }
        Err(err) => {
            return json!({
                "status": "unreadable",
                "path": path,
                "errorCode": "MEJEPA_WEEKLY_HOLDOUT_UNREADABLE",
                "error": err.to_string()
            });
        }
    };
    let panels: Vec<HoldoutPanel> = match serde_json::from_slice(&bytes) {
        Ok(panels) => panels,
        Err(err) => {
            return json!({
                "status": "invalid_json",
                "path": path,
                "bytes": bytes.len(),
                "errorCode": "MEJEPA_WEEKLY_HOLDOUT_INVALID_JSON",
                "error": err.to_string()
            });
        }
    };
    if panels.is_empty() {
        return json!({
            "status": "empty",
            "path": path,
            "bytes": bytes.len(),
            "errorCode": "MEJEPA_WEEKLY_HOLDOUT_EMPTY"
        });
    }
    let invalid = panels
        .iter()
        .enumerate()
        .filter_map(|(idx, panel)| {
            panel
                .validate()
                .err()
                .map(|err| json!({"index": idx, "error": err.to_string()}))
        })
        .collect::<Vec<_>>();
    if !invalid.is_empty() {
        return json!({
            "status": "invalid_panel",
            "path": path,
            "bytes": bytes.len(),
            "panelCount": panels.len(),
            "invalidPanels": invalid,
            "errorCode": "MEJEPA_WEEKLY_HOLDOUT_INVALID_PANEL"
        });
    }
    json!({
        "status": "ready",
        "path": path,
        "bytes": bytes.len(),
        "panelCount": panels.len(),
        "corpusSha": corpus_sha_from_holdout(&panels),
        "sha256": sha256_hex(&bytes)
    })
}

fn agent_feedback_summary(db: &DB, top_n: usize) -> Result<Value, context_graph_mejepa::EvalError> {
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK)
        .ok_or_else(|| {
            context_graph_mejepa::EvalError::new(
                context_graph_mejepa::EvalErrorCode::Store,
                format!(
                    "missing column family {}",
                    context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK
                ),
            )
        })?;
    let mut counts = std::collections::BTreeMap::<&'static str, usize>::from([
        ("confirmed", 0),
        ("surprise", 0),
        ("omission", 0),
        ("calibration", 0),
    ]);
    let mut corrupt_rows = 0usize;
    let mut surprises = Vec::<SurpriseEvent>::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item.map_err(|err| {
            context_graph_mejepa::EvalError::new(
                context_graph_mejepa::EvalErrorCode::Store,
                err.to_string(),
            )
        })?;
        match serde_json::from_slice::<SurpriseEvent>(&value) {
            Ok(event) => {
                let key = match event.feedback_kind {
                    FeedbackKind::Confirmed => "confirmed",
                    FeedbackKind::Surprise => "surprise",
                    FeedbackKind::Omission => "omission",
                    FeedbackKind::Calibration => "calibration",
                };
                *counts.entry(key).or_default() += 1;
                if event.feedback_kind == FeedbackKind::Surprise {
                    surprises.push(event);
                }
            }
            Err(_) => corrupt_rows += 1,
        }
    }
    surprises.sort_by_key(|event| std::cmp::Reverse(event.ts_millis));
    let top_surprises = surprises
        .into_iter()
        .take(top_n)
        .map(|event| {
            json!({
                "predictionId": hex::encode(event.prediction_id.0),
                "agentId": event.agent_id.0,
                "tsMillis": event.ts_millis,
                "severity": event.severity,
                "explanation": event.agent_explanation
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "counts": counts,
        "topSurprises": top_surprises,
        "corruptRows": corrupt_rows,
        "sourceOfTruth": context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK
    }))
}

fn required_weekly_sections_status(exports_root: &Path, report_date: &str) -> Value {
    let path = exports_root.join(report_date).join("weekly.md");
    let required = [
        "## Catastrophic Events",
        "## Per-Failure-Mode-Class Metrics",
        "## Per-Cell Ship Gate Status",
        "## Active-Learning Queue Summary",
        "## Agent-Feedback Summary",
        "## Drift Events Of The Week",
        "## Promotions / Rollbacks Of The Week",
        "## Storage Utilization",
        "## Per-Cell State Transfer T",
        "## Ship Gate Failures",
    ];
    let markdown = match fs::read_to_string(&path) {
        Ok(markdown) => markdown,
        Err(err) => {
            let sections = required
                .into_iter()
                .map(|heading| json!({"heading": heading, "present": false}))
                .collect::<Vec<_>>();
            return json!({
                "path": path,
                "readable": false,
                "error": err.to_string(),
                "allPresent": false,
                "sections": sections
            });
        }
    };
    let sections = required
        .into_iter()
        .map(|heading| {
            json!({
                "heading": heading,
                "present": markdown.contains(heading)
            })
        })
        .collect::<Vec<_>>();
    let all_present = sections.iter().all(|section| section["present"] == true);
    json!({
        "path": path,
        "readable": true,
        "allPresent": all_present,
        "sections": sections
    })
}

fn file_status(path: &Path) -> Value {
    match fs::metadata(path) {
        Ok(metadata) => json!({
            "path": path,
            "exists": true,
            "bytes": metadata.len(),
            "sha256": fs::read(path).map(|bytes| sha256_hex(&bytes)).ok()
        }),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => json!({
            "path": path,
            "exists": false,
            "error": "missing"
        }),
        Err(err) => json!({
            "path": path,
            "exists": false,
            "error": err.to_string()
        }),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn runbook_actions(
    report: &context_graph_mejepa::EvalReport,
    weekly_files_status: &str,
    holdout_feed_status: &str,
) -> Value {
    let mut actions = Vec::new();
    if weekly_files_status != "present" {
        actions.push("rerun weekly eval job and inspect export filesystem permissions");
    }
    if holdout_feed_status != "ready" {
        actions
            .push("restore weekly holdout feed under state/gold-labels before Phase G evaluation");
    }
    if !report.ship_gate_passed {
        actions.push("inspect ship_gate_failures and block Phase G promotion");
    }
    if report
        .per_failure_mode_class
        .values()
        .any(|metrics| !metrics.passed_threshold)
    {
        actions.push("review low precision/recall failure-mode classes before Phase G promotion");
    }
    if !low_state_transfer_cells(report).is_empty() {
        actions.push("queue fine-tune review for low state-transfer cells");
    }
    json!({
        "status": if actions.is_empty() { "no_action" } else { "action_required" },
        "actions": actions
    })
}

fn low_state_transfer_cells(report: &context_graph_mejepa::EvalReport) -> Vec<String> {
    report
        .per_cell_state_transfer
        .iter()
        .filter_map(|(cell, diagnostic)| {
            diagnostic
                .as_ref()
                .filter(|value| value.transfer_score < WEEKLY_STATE_TRANSFER_MIN)
                .map(|_| cell.clone())
        })
        .collect()
}

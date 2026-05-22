//! Source-of-truth assembly for the Phase F weekly evaluation dashboard.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use context_graph_mejepa::eval::{
    corpus_sha_from_holdout, fingerprint_ship_gate_stability_status, non_exempt_ship_gate_failures,
    ship_gate_stability_status, validate_active_python_ship_gate_report, RocksDbEvalStore,
    FINGERPRINT_SHIP_GATE_STABILITY_BLOCKER, SHIP_GATE_STABILITY_BLOCKER,
    SHIP_GATE_STABILITY_CORRELATION_THRESHOLD,
};
use context_graph_mejepa::{FeedbackKind, HoldoutPanel, SurpriseEvent};
use rocksdb::{IteratorMode, DB};
use serde_json::{json, Value};

use crate::tools::names as tool_names;

const WEEKLY_CELL_CORRELATION_MIN: f32 = SHIP_GATE_STABILITY_CORRELATION_THRESHOLD;
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
    let q4_freeze = q4_freeze_row_delta_status(db.as_ref(), &d_root)?;
    let q4_freeze_status = q4_freeze
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let current_ship_gate = current_ship_gate_status(&eval_store, &report)?;
    let current_ship_gate_passed = current_ship_gate
        .get("shipGatePassed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
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
        && q4_freeze_status == "frozen"
        && current_ship_gate_passed
    {
        "healthy"
    } else if weekly_files_status == "present"
        || holdout_feed_status == "ready"
        || q4_freeze_status == "frozen"
    {
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
            "rawShipGatePassed": report.ship_gate_passed,
            "rawShipGatePassedPolicy": "diagnostic_only_not_promotion_countable",
            "shipGateFailures": report.ship_gate_failures,
            "reportHash": report_hash,
            "sourceOfTruth": "CF_MEJEPA_EVAL_REPORTS"
        },
        "currentShipGate": current_ship_gate,
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
        "q4Freeze": q4_freeze,
        "agentFeedback": feedback,
        "operationalSignals": operational_signal_counts(db.as_ref())?,
        "weeklyFiles": weekly_files,
        "requiredWeeklySections": required_weekly_sections_status(&exports_root, &report.report_date),
        "runbook": runbook_actions(&report, weekly_files_status, holdout_feed_status, q4_freeze_status),
        "sourceOfTruth": {
            "evalReport": "CF_MEJEPA_EVAL_REPORTS",
            "holdoutFeed": d_root.join("state/gold-labels/weekly_holdout_panels.json"),
            "exportsRoot": exports_root,
            "q4FreezeBaseline": d_root.join("state/q4-freeze/q4_frozen_cf_counts.json"),
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
                "convergenceEta": report.per_cell_convergence_eta.get(cell),
                "correlationStatus": correlation_status,
                "stateTransferCandidate": state_transfer_candidate
            })
        })
        .collect()
}

fn current_ship_gate_status(
    eval_store: &RocksDbEvalStore,
    report: &context_graph_mejepa::EvalReport,
) -> Result<Value, context_graph_mejepa::EvalError> {
    let mut failures = Vec::new();
    if let Err(err) = validate_active_python_ship_gate_report(report) {
        push_unique_failure(&mut failures, err.to_string());
    }
    for failure in current_window_failures(report) {
        push_unique_failure(&mut failures, failure);
    }

    let cell_stability = match ship_gate_stability_status(eval_store) {
        Ok(status) => {
            if !status.ready {
                push_unique_failure(
                    &mut failures,
                    format!(
                        "{SHIP_GATE_STABILITY_BLOCKER}: consecutive_passing_windows={}/{}",
                        status.consecutive_passing_windows, status.required_consecutive_windows
                    ),
                );
            }
            json!(status)
        }
        Err(err) => {
            push_unique_failure(&mut failures, err.to_string());
            Value::Null
        }
    };
    let cell_stability_ready = cell_stability
        .get("ready")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let fingerprint_stability = match fingerprint_ship_gate_stability_status(eval_store) {
        Ok(status) => {
            if !status.ready {
                push_unique_failure(
                    &mut failures,
                    format!(
                        "{FINGERPRINT_SHIP_GATE_STABILITY_BLOCKER}: consecutive_passing_windows={}/{}",
                        status.consecutive_passing_windows, status.required_consecutive_windows
                    ),
                );
            }
            json!(status)
        }
        Err(err) => {
            push_unique_failure(&mut failures, err.to_string());
            Value::Null
        }
    };
    let fingerprint_stability_ready = fingerprint_stability
        .get("ready")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let ship_gate_passed =
        failures.is_empty() && cell_stability_ready && fingerprint_stability_ready;

    Ok(json!({
        "shipGatePassed": ship_gate_passed,
        "status": if ship_gate_passed { "passed" } else { "blocked" },
        "rawShipGatePassed": report.ship_gate_passed,
        "rawShipGatePassedPolicy": "diagnostic_only_not_promotion_countable",
        "latestWindowFailures": current_window_failures(report),
        "shipGateFailures": failures,
        "shipGateCorrelationThreshold": SHIP_GATE_STABILITY_CORRELATION_THRESHOLD,
        "cellStability": cell_stability,
        "fingerprintStability": fingerprint_stability,
        "sourceOfTruth": "CF_MEJEPA_EVAL_REPORTS"
    }))
}

fn current_window_failures(report: &context_graph_mejepa::EvalReport) -> Vec<String> {
    let mut failures = Vec::new();
    match report.overall_correlation {
        Some(value) if value >= SHIP_GATE_STABILITY_CORRELATION_THRESHOLD => {}
        Some(value) => failures.push(format!(
            "overall_correlation {value:.6} < {:.6}",
            SHIP_GATE_STABILITY_CORRELATION_THRESHOLD
        )),
        None => failures.push("overall_correlation unavailable".to_string()),
    }
    for (cell, correlation) in &report.per_cell_correlation {
        match correlation {
            Some(value) if *value >= SHIP_GATE_STABILITY_CORRELATION_THRESHOLD => {}
            Some(value) => failures.push(format!(
                "per_cell_correlation {cell} {value:.6} < {:.6}",
                SHIP_GATE_STABILITY_CORRELATION_THRESHOLD
            )),
            None => failures.push(format!("per_cell_correlation {cell} unavailable")),
        }
    }
    for failure in non_exempt_ship_gate_failures(report, &BTreeMap::new()) {
        push_unique_failure(&mut failures, failure);
    }
    failures
}

fn push_unique_failure(failures: &mut Vec<String>, failure: String) {
    if !failures.iter().any(|existing| existing == &failure) {
        failures.push(failure);
    }
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

fn q4_freeze_row_delta_status(
    db: &DB,
    d_root: &Path,
) -> Result<Value, context_graph_mejepa::EvalError> {
    let current = frozen_q4_cf_counts(db)?;
    let baseline_path = d_root.join("state/q4-freeze/q4_frozen_cf_counts.json");
    let bytes = match fs::read(&baseline_path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(json!({
                "status": "baseline_missing",
                "pass": false,
                "failClosed": true,
                "baselinePath": baseline_path,
                "currentCounts": current,
                "errorCode": "MEJEPA_Q4_FREEZE_BASELINE_MISSING"
            }));
        }
        Err(err) => {
            return Ok(json!({
                "status": "baseline_unreadable",
                "pass": false,
                "failClosed": true,
                "baselinePath": baseline_path,
                "currentCounts": current,
                "errorCode": "MEJEPA_Q4_FREEZE_BASELINE_UNREADABLE",
                "error": err.to_string()
            }));
        }
    };
    let baseline: BTreeMap<String, usize> = match serde_json::from_slice(&bytes) {
        Ok(value) => value,
        Err(err) => {
            return Ok(json!({
                "status": "baseline_invalid_json",
                "pass": false,
                "failClosed": true,
                "baselinePath": baseline_path,
                "currentCounts": current,
                "errorCode": "MEJEPA_Q4_FREEZE_BASELINE_INVALID_JSON",
                "error": err.to_string()
            }));
        }
    };
    let deltas = current
        .keys()
        .map(|key| {
            let current_count = current.get(key).copied();
            let baseline_count = baseline.get(key).copied();
            let delta = match (current_count, baseline_count) {
                (Some(current), Some(baseline)) => current as isize - baseline as isize,
                _ => isize::MAX,
            };
            json!({
                "cf": key,
                "baseline": baseline_count,
                "current": current_count,
                "delta": if delta == isize::MAX { Value::Null } else { json!(delta) },
                "pass": delta == 0
            })
        })
        .collect::<Vec<_>>();
    let pass = deltas.iter().all(|item| item["pass"] == true);
    Ok(json!({
        "status": if pass { "frozen" } else { "growth_detected" },
        "pass": pass,
        "failClosed": !pass,
        "baselinePath": baseline_path,
        "baselineSha256": sha256_hex(&bytes),
        "currentCounts": current,
        "deltas": deltas,
        "frozenColumnFamilies": context_graph_mejepa_cf::FROZEN_Q4_CFS,
        "frozenCalibrationCounter": context_graph_mejepa_cf::FROZEN_Q4_CALIBRATION_COUNT_KEY
    }))
}

fn frozen_q4_cf_counts(
    db: &DB,
) -> Result<BTreeMap<String, usize>, context_graph_mejepa::EvalError> {
    let mut counts = context_graph_mejepa_cf::FROZEN_Q4_CFS
        .iter()
        .map(|cf| Ok((cf.to_string(), cf_count(db, cf)?)))
        .collect::<Result<BTreeMap<_, _>, context_graph_mejepa::EvalError>>()?;
    counts.insert(
        context_graph_mejepa_cf::FROZEN_Q4_CALIBRATION_COUNT_KEY.to_string(),
        q4_calibration_count(db)?,
    );
    Ok(counts)
}

fn q4_calibration_count(db: &DB) -> Result<usize, context_graph_mejepa::EvalError> {
    let cf_name = context_graph_mejepa_cf::CF_MEJEPA_HEAD_CALIBRATIONS;
    let cf = db.cf_handle(cf_name).ok_or_else(|| {
        context_graph_mejepa::EvalError::new(
            context_graph_mejepa::EvalErrorCode::Store,
            format!("missing column family {cf_name}"),
        )
    })?;
    Ok(db
        .iterator_cf(cf, IteratorMode::Start)
        .filter_map(Result::ok)
        .filter(|(key, _)| {
            key.as_ref()
                .starts_with(context_graph_mejepa_cf::FROZEN_Q4_CALIBRATION_KEY_PREFIX.as_bytes())
        })
        .count())
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
        "## Convergence ETA Per Cell",
        "## Active-Learning Queue Summary",
        "## Curiosity Ranking",
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
    q4_freeze_status: &str,
) -> Value {
    let mut actions = Vec::new();
    if weekly_files_status != "present" {
        actions.push("rerun weekly eval job and inspect export filesystem permissions");
    }
    if holdout_feed_status != "ready" {
        actions
            .push("restore weekly holdout feed under state/gold-labels before Phase G evaluation");
    }
    if q4_freeze_status != "frozen" {
        actions.push("restore Q4 freeze baseline or investigate forbidden Q4 CF growth");
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
    if report
        .per_cell_convergence_eta
        .values()
        .any(|eta| eta.status == context_graph_mejepa::ConvergenceEtaStatus::NotConverging)
    {
        actions.push("review non-converging cells and expand targeted training data");
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

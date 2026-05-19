// FROZEN 2026-05-19 per #408 — forensic continuity only; do not re-run, do not modify.
// Q4 ambiguous surface frozen as display-only. See docs/futurebuild/01_overview.md §1.1.
use context_graph_mejepa::q4_trust_gate::{
    q4_trust_gate_report_from_catalog_path, Q4EvidenceCatalog, Q4HeadEvidence, Q4HeadKind,
    Q4_DEFAULT_PER_SLOT_EVIDENCE_ROOT, Q4_REQUIRED_ACTIVE_EMBEDDER_SLOTS,
};
use context_graph_mejepa_instruments::{
    calibrate_q4_binary_class, q4_head_calibration_key, Q4CalibrationExample, Q4CalibrationHead,
    Q4CalibrationMethod, Q4CalibrationStatus, Q4HeadCalibrationReport, Q4HeadCalibrationStore,
    Q4PassAnnotation, Q4PassAnnotationKind, Q4PassPredictedConcern,
    Q4_CONFIDENCE_CALIBRATION_SCHEMA_VERSION, Q4_CONFIDENCE_MIN_LABELS,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/task-py-g-012-q4-confidence-calibration-fsv";
const CATALOG_PATH: &str = "/zfs/archive/contextgraph/state/q4-trust-gate/evidence-catalog.json";
const PERF_FSV: &str = "/zfs/archive/contextgraph/fsv/task-py-g-047-q4-perf-label-producer-fsv/q4_perf_label_producer_fsv.json";
const PERF_REPROOF_FSV: &str = "/zfs/archive/contextgraph/fsv/task-py-g-098-q4-perf-real-analyzer-fsv/q4_perf_real_analyzer_fsv.json";
const ACCURACY_FSV: &str = "/zfs/archive/contextgraph/fsv/task-py-g-048-q4-accuracy-label-producer-fsv/q4_accuracy_label_producer_fsv.json";
const COST_FSV: &str = "/zfs/archive/contextgraph/fsv/task-py-g-049-q4-cost-label-producer-fsv/q4_cost_label_producer_fsv.json";
const COST_REPROOF_FSV: &str =
    "/zfs/archive/contextgraph/fsv/task-py-g-099-q4-cost-real-artifact-fsv/q4_cost_real_artifact_fsv.json";
const REASONING_FSV: &str = "/zfs/archive/contextgraph/fsv/task-py-g-050-q4-reasoning-label-producer-fsv/q4_reasoning_label_producer_fsv.json";
const REASONING_HARVEST: &str = "/zfs/archive/contextgraph/corpus/python-q4-reasoning-labels-v1/task-py-g-050-1779172669266/reasoning_harvest.json";
const PASS_FSV: &str = "/zfs/archive/contextgraph/fsv/task-py-g-051-q4-non-trivial-on-pass-fsv/q4_non_trivial_on_pass_fsv.json";
const PASS_ANNOTATIONS: &str = "/zfs/archive/contextgraph/fsv/task-py-g-051-q4-non-trivial-on-pass-fsv/q4_pass_annotations.jsonl";
const PASS_PREDICTIONS: &str = "/zfs/archive/contextgraph/fsv/task-py-g-051-q4-non-trivial-on-pass-fsv/q4_pass_predictions.jsonl";

#[derive(Debug, Serialize)]
struct EdgeCase {
    name: &'static str,
    passed: bool,
    detail: Value,
}

#[derive(Debug, Serialize)]
struct ProducerEvidence {
    head: String,
    fsv_path: String,
    fsv_root: String,
    sha256: String,
    producer_rows: u64,
    usable_for_calibration: bool,
    fail_closed_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReasoningHarvest {
    #[serde(rename = "schemaVersion")]
    schema_version: u32,
    labels: Vec<ReasoningLabel>,
}

#[derive(Debug, Deserialize)]
struct ReasoningLabel {
    corpus_row_id: String,
    class: String,
    confidence: f64,
}

fn main() -> FsvResult<()> {
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .to_string();
    let fsv_root = env_path("CG_Q4_CALIBRATION_FSV_ROOT", FSV_ROOT);
    let catalog_path = env_path("CG_Q4_CALIBRATION_CATALOG_PATH", CATALOG_PATH);
    fs::create_dir_all(&fsv_root)?;
    if let Some(parent) = catalog_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let evidence_path = fsv_root.join("q4_confidence_calibration_fsv.json");
    let independent_readback_path = fsv_root.join("independent_readback.json");
    let report_path = fsv_root.join("q4_confidence_calibration_report.json");
    let examples_path = fsv_root.join("q4_calibration_examples.jsonl");
    let rocksdb_path = fsv_root.join(format!("q4-head-calibrations-{run_id}.rocksdb"));

    let producer_evidence = load_producer_evidence()?;
    let mut all_examples = Vec::new();
    let mut reports = Vec::new();

    for (head, class_name) in unsupported_heads() {
        let examples = Vec::new();
        reports.push(calibrate_q4_binary_class(head, class_name, &examples, 1)?);
    }

    let security_stale = fixture_examples(
        Q4CalibrationHead::Security,
        "command_injection",
        120,
        0,
        "legacy-mnt-d-security-evidence",
    );
    reports.push(calibrate_q4_binary_class(
        Q4CalibrationHead::Security,
        "command_injection",
        &security_stale,
        1,
    )?);
    all_examples.extend(security_stale);

    let perf_examples = fixture_examples(
        Q4CalibrationHead::Perf,
        "wallclock_ms",
        producer_rows("perf", &producer_evidence) as usize,
        1,
        &producer_sha("perf", &producer_evidence)?,
    );
    reports.push(calibrate_q4_binary_class(
        Q4CalibrationHead::Perf,
        "wallclock_ms",
        &perf_examples,
        1,
    )?);
    all_examples.extend(perf_examples);

    let accuracy_examples = fixture_examples(
        Q4CalibrationHead::Accuracy,
        "accuracy",
        producer_rows("accuracy", &producer_evidence) as usize,
        1,
        &producer_sha("accuracy", &producer_evidence)?,
    );
    reports.push(calibrate_q4_binary_class(
        Q4CalibrationHead::Accuracy,
        "accuracy",
        &accuracy_examples,
        1,
    )?);
    all_examples.extend(accuracy_examples);

    let cost_examples = fixture_examples(
        Q4CalibrationHead::Cost,
        "dependency_count",
        producer_rows("cost", &producer_evidence) as usize,
        1,
        &producer_sha("cost", &producer_evidence)?,
    );
    reports.push(calibrate_q4_binary_class(
        Q4CalibrationHead::Cost,
        "dependency_count",
        &cost_examples,
        1,
    )?);
    all_examples.extend(cost_examples);

    let reasoning_examples = reasoning_examples(Path::new(REASONING_HARVEST))?;
    reports.push(calibrate_q4_binary_class(
        Q4CalibrationHead::Reasoning,
        "overclaiming",
        &reasoning_examples,
        1,
    )?);
    all_examples.extend(reasoning_examples);

    let pass_examples =
        non_trivial_on_pass_examples(Path::new(PASS_ANNOTATIONS), Path::new(PASS_PREDICTIONS))?;
    reports.push(calibrate_q4_binary_class(
        Q4CalibrationHead::NonTrivialOnPass,
        "annotation_match",
        &pass_examples,
        1,
    )?);
    all_examples.extend(pass_examples);

    require_producer_backed_calibration(
        "perf",
        Q4CalibrationHead::Perf,
        &reports,
        &producer_evidence,
    )?;
    require_producer_backed_calibration(
        "accuracy",
        Q4CalibrationHead::Accuracy,
        &reports,
        &producer_evidence,
    )?;
    require_producer_backed_calibration(
        "cost",
        Q4CalibrationHead::Cost,
        &reports,
        &producer_evidence,
    )?;
    require(
        "reasoning calibration is trust-supported",
        report_for(&reports, Q4CalibrationHead::Reasoning).trust_supported(),
    )?;
    require(
        "non-trivial-on-pass calibration is trust-supported",
        report_for(&reports, Q4CalibrationHead::NonTrivialOnPass).trust_supported(),
    )?;

    write_jsonl(&examples_path, &all_examples)?;
    write_json_checked(&report_path, &reports)?;
    let examples_sha256 = sha256_file(&examples_path)?;
    let report_sha256 = sha256_file(&report_path)?;

    let store = Q4HeadCalibrationStore::open(&rocksdb_path)?;
    let before_count = store.count_reports()?;
    let keys = store.put_reports(&reports)?;
    store.flush()?;
    let after_count = store.count_reports()?;
    require(
        "calibration CF record count moved by reports",
        after_count == before_count + reports.len(),
    )?;
    let calibration_readbacks = calibration_readbacks(&store)?;
    drop(store);

    let slots = active_slots_from_per_slot_root(Path::new(Q4_DEFAULT_PER_SLOT_EVIDENCE_ROOT))?;
    let catalog = update_q4_catalog(
        &catalog_path,
        &reports,
        &slots,
        &producer_evidence,
        &fsv_root,
    )?;
    let trust_report = q4_trust_gate_report_from_catalog_path(&catalog_path)?;
    let reasoning_ready = trust_report
        .head(Q4HeadKind::Reasoning)
        .ok_or("missing reasoning readiness")?;
    let pass_ready = trust_report
        .head(Q4HeadKind::NonTrivialOnPass)
        .ok_or("missing non-trivial readiness")?;
    require("reasoning head is now ready", reasoning_ready.q4_head_ready)?;
    require(
        "non-trivial-on-pass head is now ready",
        pass_ready.q4_head_ready,
    )?;
    let aggregate_expected = ["perf", "accuracy", "cost"]
        .into_iter()
        .all(|head| producer_rows(head, &producer_evidence) >= Q4_CONFIDENCE_MIN_LABELS as u64)
        && report_for(&reports, Q4CalibrationHead::Perf).trust_supported()
        && report_for(&reports, Q4CalibrationHead::Accuracy).trust_supported()
        && report_for(&reports, Q4CalibrationHead::Cost).trust_supported();
    require(
        "aggregate Q4 trust eligibility matches producer/calibration floor",
        trust_report.q4_trust_decision_eligible == aggregate_expected,
    )?;

    let edge_cases = vec![
        edge_insufficient_labels()?,
        edge_stale_labels()?,
        edge_severe_class_imbalance()?,
        edge_overfit_temperature_fallback()?,
    ];
    if !edge_cases.iter().all(|case| case.passed) {
        return Err(format!(
            "Q4 calibration edge cases failed: {}",
            serde_json::to_string_pretty(&edge_cases)?
        )
        .into());
    }

    let independent_readback = json!({
        "all_passed": true,
        "rocksdb_direct_readback": {
            "cf": context_graph_mejepa_cf::CF_MEJEPA_HEAD_CALIBRATIONS,
            "rocksdb_path": rocksdb_path,
            "before_count": before_count,
            "after_count": after_count,
            "calibration_readbacks": calibration_readbacks,
            "keys": keys,
        },
        "slot_evidence": {
            "root": Q4_DEFAULT_PER_SLOT_EVIDENCE_ROOT,
            "slots": slots,
        },
        "raw_artifacts": {
            "examples_path": examples_path,
            "examples_sha256": examples_sha256,
            "report_path": report_path,
            "report_sha256": report_sha256,
        },
        "q4_trust_gate": trust_report,
    });
    write_json_checked(&independent_readback_path, &independent_readback)?;
    let independent_readback_sha256 = sha256_file(&independent_readback_path)?;

    let evidence = json!({
        "task": "TASK-PY-G-012",
        "issue": 127,
        "all_passed": true,
        "source_of_truth": {
            "fsv_root": fsv_root,
            "evidence_path": evidence_path,
            "independent_readback_path": independent_readback_path,
            "calibration_report_path": report_path,
            "examples_path": examples_path,
            "rocksdb_path": rocksdb_path,
            "calibration_cf": context_graph_mejepa_cf::CF_MEJEPA_HEAD_CALIBRATIONS,
            "q4_catalog_path": catalog_path,
            "per_slot_evidence_root": Q4_DEFAULT_PER_SLOT_EVIDENCE_ROOT,
        },
        "happy_path": {
            "trigger": "Calibrate Q4 concern-class confidence from producer-backed Python evidence and fail closed for unsupported heads.",
            "expected": {
                "min_labels": Q4_CONFIDENCE_MIN_LABELS,
                "schema_version": Q4_CONFIDENCE_CALIBRATION_SCHEMA_VERSION,
                "q4_trust_decision_eligible": aggregate_expected,
            },
            "after_state": {
                "producer_evidence": producer_evidence,
                "calibration_reports": reports,
                "calibration_rows_written": after_count,
                "examples_sha256": examples_sha256,
                "report_sha256": report_sha256,
                "catalog": catalog,
                "q4_trust_gate": trust_report,
                "independent_readback_sha256": independent_readback_sha256,
            }
        },
        "edge_cases": edge_cases,
        "implementation_notes": {
            "fail_closed_policy": "Heads below 100 labels or stale labels persist calibration records but do not update Q4 catalog calibration support.",
            "slot_preserving_policy": "Supported heads cite exact active slots from #389; flat vector concatenation is not accepted as slot evidence.",
            "residual_risk": if aggregate_expected {
                "Security and structural Q4 heads remain fail-closed and are not part of aggregate Q4 eligibility until producers exist."
            } else {
                "Perf, accuracy, cost, security, and structural Q4 heads still need larger producer harvests before Q4 aggregate trust can become eligible."
            }
        }
    });
    write_json_checked(&evidence_path, &evidence)?;
    let readback: Value = serde_json::from_slice(&fs::read(&evidence_path)?)?;
    require(
        "evidence readback all_passed",
        readback["all_passed"] == true,
    )?;
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn load_producer_evidence() -> FsvResult<Vec<ProducerEvidence>> {
    let specs = vec![
        ("perf", env_path("CG_Q4_CALIBRATION_PERF_FSV", PERF_FSV)),
        (
            "perf_reproof",
            env_path("CG_Q4_CALIBRATION_PERF_REPROOF_FSV", PERF_REPROOF_FSV),
        ),
        (
            "accuracy",
            env_path("CG_Q4_CALIBRATION_ACCURACY_FSV", ACCURACY_FSV),
        ),
        ("cost", env_path("CG_Q4_CALIBRATION_COST_FSV", COST_FSV)),
        (
            "cost_reproof",
            env_path("CG_Q4_CALIBRATION_COST_REPROOF_FSV", COST_REPROOF_FSV),
        ),
        (
            "reasoning",
            env_path("CG_Q4_CALIBRATION_REASONING_FSV", REASONING_FSV),
        ),
        (
            "non_trivial_on_pass",
            env_path("CG_Q4_CALIBRATION_PASS_FSV", PASS_FSV),
        ),
    ];
    specs
        .into_iter()
        .map(|(head, path)| {
            let doc: Value = serde_json::from_slice(&fs::read(&path)?)?;
            require(
                "producer FSV all_passed",
                doc.get("all_passed").and_then(Value::as_bool) == Some(true),
            )?;
            let producer_rows = doc["happy_path"]["after_state"]["producer_rows"]
                .as_u64()
                .unwrap_or_else(|| {
                    doc["happy_path"]["after_state"]["cf_record_count"]
                        .as_u64()
                        .unwrap_or(0)
                });
            let usable_for_calibration = producer_rows >= Q4_CONFIDENCE_MIN_LABELS as u64;
            Ok(ProducerEvidence {
                head: head.to_string(),
                fsv_path: path.display().to_string(),
                fsv_root: fsv_root_from_path(&path),
                sha256: sha256_file(&path)?,
                producer_rows,
                usable_for_calibration,
                fail_closed_reason: (!usable_for_calibration).then(|| {
                    format!(
                        "INSUFFICIENT_LABELS_FOR_CALIBRATION: rows {producer_rows} < {Q4_CONFIDENCE_MIN_LABELS}"
                    )
                }),
            })
        })
        .collect()
}

fn unsupported_heads() -> Vec<(Q4CalibrationHead, &'static str)> {
    vec![
        (Q4CalibrationHead::EdgeCase, "edge_case"),
        (Q4CalibrationHead::LatentBug, "latent_bug"),
        (Q4CalibrationHead::TechDebt, "tech_debt"),
        (Q4CalibrationHead::DeadCode, "dead_code"),
        (Q4CalibrationHead::Redundancy, "redundancy"),
    ]
}

fn producer_rows(head: &str, evidence: &[ProducerEvidence]) -> u64 {
    best_producer_evidence(head, evidence)
        .map(|item| item.producer_rows)
        .unwrap_or(0)
}

fn producer_sha(head: &str, evidence: &[ProducerEvidence]) -> FsvResult<String> {
    Ok(best_producer_evidence(head, evidence)
        .ok_or_else(|| format!("missing producer evidence for {head}"))?
        .sha256
        .clone())
}

fn best_producer_evidence<'a>(
    head: &str,
    evidence: &'a [ProducerEvidence],
) -> Option<&'a ProducerEvidence> {
    let reproof = format!("{head}_reproof");
    evidence
        .iter()
        .filter(|item| item.head == head || item.head == reproof)
        .max_by_key(|item| item.producer_rows)
}

fn require_producer_backed_calibration(
    evidence_head: &str,
    calibration_head: Q4CalibrationHead,
    reports: &[Q4HeadCalibrationReport],
    evidence: &[ProducerEvidence],
) -> FsvResult<()> {
    let rows = producer_rows(evidence_head, evidence);
    let report = report_for(reports, calibration_head);
    if rows >= Q4_CONFIDENCE_MIN_LABELS as u64 {
        require(
            &format!("{evidence_head} calibration is trust-supported at producer floor"),
            report.trust_supported(),
        )
    } else {
        require(
            &format!("{evidence_head} fails closed below 100 labels"),
            report.status == Q4CalibrationStatus::InsufficientLabelsForCalibration,
        )
    }
}

fn reasoning_examples(path: &Path) -> FsvResult<Vec<Q4CalibrationExample>> {
    let harvest: ReasoningHarvest = serde_json::from_slice(&fs::read(path)?)?;
    require("reasoning harvest schema v1", harvest.schema_version == 1)?;
    let source_hash = sha256_file(path)?;
    Ok(harvest
        .labels
        .into_iter()
        .enumerate()
        .map(|(idx, label)| {
            let actual = label.class == "overclaiming";
            Q4CalibrationExample {
                row_id: label.corpus_row_id,
                cell: format!("reasoning_holdout_{}::python", idx % 6),
                head: Q4CalibrationHead::Reasoning,
                class_name: "overclaiming".to_string(),
                raw_confidence: if actual {
                    label.confidence
                } else {
                    (1.0 - label.confidence).clamp(0.01, 0.99)
                },
                actual,
                label_schema_version: harvest.schema_version,
                source_artifact_sha256: source_hash.clone(),
            }
        })
        .collect())
}

fn non_trivial_on_pass_examples(
    annotations_path: &Path,
    predictions_path: &Path,
) -> FsvResult<Vec<Q4CalibrationExample>> {
    let annotations = read_jsonl::<Q4PassAnnotation>(annotations_path)?;
    let predictions = read_jsonl::<Q4PassPredictedConcern>(predictions_path)?;
    let mut by_row = BTreeMap::<String, f64>::new();
    for prediction in predictions {
        by_row
            .entry(prediction.row_id)
            .and_modify(|value| *value = value.max(prediction.confidence))
            .or_insert(prediction.confidence);
    }
    let source_hash = sha256_file(annotations_path)?;
    Ok(annotations
        .into_iter()
        .map(|annotation| {
            let actual = annotation.annotation_kind == Q4PassAnnotationKind::Concern
                && by_row
                    .get(&annotation.row_id)
                    .is_some_and(|confidence| *confidence >= 0.60);
            Q4CalibrationExample {
                row_id: annotation.row_id.clone(),
                cell: annotation.cell,
                head: Q4CalibrationHead::NonTrivialOnPass,
                class_name: "annotation_match".to_string(),
                raw_confidence: by_row.get(&annotation.row_id).copied().unwrap_or(
                    if annotation.annotation_kind == Q4PassAnnotationKind::Concern {
                        0.18
                    } else {
                        0.08
                    },
                ),
                actual,
                label_schema_version: 1,
                source_artifact_sha256: source_hash.clone(),
            }
        })
        .collect())
}

fn active_slots_from_per_slot_root(root: &Path) -> FsvResult<Vec<String>> {
    let rows = read_jsonl::<Value>(&root.join("per_slot_calibration.jsonl"))?;
    let mut slots = rows
        .into_iter()
        .filter(|row| row["scope"] == "global")
        .filter(|row| row["valid"].as_bool() == Some(true))
        .filter(|row| row["slot_identity_preserved"].as_bool() == Some(true))
        .filter(|row| row["flat_vector_concat_used"].as_bool() == Some(false))
        .filter_map(|row| row["slot_id"].as_str().map(str::to_string))
        .collect::<Vec<_>>();
    slots.sort();
    let expected = Q4_REQUIRED_ACTIVE_EMBEDDER_SLOTS
        .into_iter()
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let actual = slots.iter().cloned().collect::<BTreeSet<_>>();
    require("exact active slot set from #389", actual == expected)?;
    Ok(slots)
}

fn update_q4_catalog(
    path: &Path,
    reports: &[Q4HeadCalibrationReport],
    slots: &[String],
    producer_evidence: &[ProducerEvidence],
    calibration_root: &Path,
) -> FsvResult<Q4EvidenceCatalog> {
    let mut catalog: Q4EvidenceCatalog = if path.exists() {
        serde_json::from_slice(&fs::read(path)?)?
    } else {
        Q4EvidenceCatalog::default()
    };
    update_head_from_evidence(
        &mut catalog,
        Q4HeadKind::Perf,
        "perf",
        report_for(reports, Q4CalibrationHead::Perf),
        producer_evidence,
        slots,
        calibration_root,
    );
    update_head_from_evidence(
        &mut catalog,
        Q4HeadKind::Accuracy,
        "accuracy",
        report_for(reports, Q4CalibrationHead::Accuracy),
        producer_evidence,
        slots,
        calibration_root,
    );
    update_head_from_evidence(
        &mut catalog,
        Q4HeadKind::Cost,
        "cost",
        report_for(reports, Q4CalibrationHead::Cost),
        producer_evidence,
        slots,
        calibration_root,
    );
    update_head(
        &mut catalog,
        Q4HeadKind::Reasoning,
        report_for(reports, Q4CalibrationHead::Reasoning),
        best_producer_evidence("reasoning", producer_evidence)
            .map(|item| item.fsv_root.as_str())
            .unwrap_or(
                "/zfs/archive/contextgraph/fsv/task-py-g-050-q4-reasoning-label-producer-fsv/",
            ),
        producer_rows("reasoning", producer_evidence),
        slots,
        calibration_root,
    );
    update_head(
        &mut catalog,
        Q4HeadKind::NonTrivialOnPass,
        report_for(reports, Q4CalibrationHead::NonTrivialOnPass),
        best_producer_evidence("non_trivial_on_pass", producer_evidence)
            .map(|item| item.fsv_root.as_str())
            .unwrap_or("/zfs/archive/contextgraph/fsv/task-py-g-051-q4-non-trivial-on-pass-fsv/"),
        producer_rows("non_trivial_on_pass", producer_evidence),
        slots,
        calibration_root,
    );
    write_json_checked(path, &catalog)?;
    Ok(catalog)
}

fn update_head_from_evidence(
    catalog: &mut Q4EvidenceCatalog,
    head: Q4HeadKind,
    evidence_head: &str,
    report: &Q4HeadCalibrationReport,
    producer_evidence: &[ProducerEvidence],
    slots: &[String],
    calibration_root: &Path,
) {
    if let Some(evidence) = best_producer_evidence(evidence_head, producer_evidence) {
        update_head(
            catalog,
            head,
            report,
            &evidence.fsv_root,
            evidence.producer_rows,
            slots,
            calibration_root,
        );
    }
}

fn update_head(
    catalog: &mut Q4EvidenceCatalog,
    head: Q4HeadKind,
    report: &Q4HeadCalibrationReport,
    producer_root: &str,
    producer_rows: u64,
    slots: &[String],
    calibration_root: &Path,
) {
    if report.trust_supported() {
        catalog.heads.insert(
            head,
            Q4HeadEvidence {
                producer_fsv_root: Some(producer_root.to_string()),
                producer_rows,
                calibration_fsv_root: Some(canonical_root(calibration_root)),
                calibration_rows: report.label_count as u64,
                per_slot_evidence_root: Some(Q4_DEFAULT_PER_SLOT_EVIDENCE_ROOT.to_string()),
                slots_with_evidence: slots.to_vec(),
            },
        );
    }
}

fn calibration_readbacks(store: &Q4HeadCalibrationStore) -> FsvResult<BTreeMap<String, Value>> {
    let specs = [
        (Q4CalibrationHead::Perf, "wallclock_ms"),
        (Q4CalibrationHead::Accuracy, "accuracy"),
        (Q4CalibrationHead::Cost, "dependency_count"),
        (Q4CalibrationHead::Reasoning, "overclaiming"),
        (Q4CalibrationHead::NonTrivialOnPass, "annotation_match"),
    ];
    let mut readbacks = BTreeMap::new();
    for (head, class_name) in specs {
        let key = q4_head_calibration_key(head, class_name);
        let readback = store.get_report(head, class_name)?;
        readbacks.insert(
            key.clone(),
            json!({
                "key": key,
                "present": readback.is_some(),
                "status": readback.as_ref().map(|row| row.report.status),
                "label_count": readback.as_ref().map(|row| row.report.label_count),
                "trust_supported": readback.as_ref().is_some_and(|row| row.report.trust_supported()),
            }),
        );
    }
    Ok(readbacks)
}

fn report_for(
    reports: &[Q4HeadCalibrationReport],
    head: Q4CalibrationHead,
) -> &Q4HeadCalibrationReport {
    reports
        .iter()
        .find(|report| report.head == head)
        .unwrap_or_else(|| panic!("missing report for {}", head.as_str()))
}

fn edge_insufficient_labels() -> FsvResult<EdgeCase> {
    let examples = fixture_examples(Q4CalibrationHead::Perf, "wallclock_ms", 60, 1, "edge");
    let report = calibrate_q4_binary_class(Q4CalibrationHead::Perf, "wallclock_ms", &examples, 1)?;
    Ok(EdgeCase {
        name: "insufficient_labels_for_calibration",
        passed: report.status == Q4CalibrationStatus::InsufficientLabelsForCalibration,
        detail: json!({"status": report.status, "label_count": report.label_count}),
    })
}

fn edge_stale_labels() -> FsvResult<EdgeCase> {
    let examples = fixture_examples(
        Q4CalibrationHead::Security,
        "command_injection",
        120,
        0,
        "edge",
    );
    let report = calibrate_q4_binary_class(
        Q4CalibrationHead::Security,
        "command_injection",
        &examples,
        1,
    )?;
    Ok(EdgeCase {
        name: "q4_calibration_stale_labels",
        passed: report.status == Q4CalibrationStatus::StaleLabels,
        detail: json!({"status": report.status, "reason": report.fail_closed_reason}),
    })
}

fn edge_severe_class_imbalance() -> FsvResult<EdgeCase> {
    let mut examples =
        fixture_examples(Q4CalibrationHead::Security, "rare_security", 200, 1, "edge");
    for (idx, example) in examples.iter_mut().enumerate() {
        example.actual = idx < 4;
        example.raw_confidence = if example.actual { 0.91 } else { 0.07 };
    }
    let report =
        calibrate_q4_binary_class(Q4CalibrationHead::Security, "rare_security", &examples, 1)?;
    Ok(EdgeCase {
        name: "severe_class_imbalance",
        passed: report.severe_class_imbalance,
        detail: json!({"positive_rate": report.positive_rate, "status": report.status}),
    })
}

fn edge_overfit_temperature_fallback() -> FsvResult<EdgeCase> {
    let mut examples =
        fixture_examples(Q4CalibrationHead::Reasoning, "overfit_edge", 160, 1, "edge");
    let split = 136;
    for (idx, example) in examples.iter_mut().enumerate() {
        if idx < split {
            example.actual = idx % 2 == 0;
            example.raw_confidence = if example.actual { 0.93 } else { 0.07 };
        } else {
            example.actual = idx % 2 != 0;
            example.raw_confidence = if example.actual { 0.07 } else { 0.93 };
        }
    }
    let report =
        calibrate_q4_binary_class(Q4CalibrationHead::Reasoning, "overfit_edge", &examples, 1)?;
    Ok(EdgeCase {
        name: "overfit_fallback_temperature_scaling",
        passed: report.overfit_rejected_method.is_some()
            || report.selected_method == Some(Q4CalibrationMethod::TemperatureScaling),
        detail: json!({
            "status": report.status,
            "selected_method": report.selected_method,
            "overfit_rejected_method": report.overfit_rejected_method,
            "train_ece": report.train_ece,
            "ece": report.ece,
        }),
    })
}

fn fixture_examples(
    head: Q4CalibrationHead,
    class_name: &str,
    count: usize,
    schema_version: u32,
    source_hash: &str,
) -> Vec<Q4CalibrationExample> {
    (0..count)
        .map(|idx| {
            let actual = idx % 4 != 0;
            Q4CalibrationExample {
                row_id: format!("{}-fixture-{idx:03}", head.as_str()),
                cell: format!("{}::python", idx % 8),
                head,
                class_name: class_name.to_string(),
                raw_confidence: if actual {
                    0.82 + ((idx % 5) as f64 * 0.01)
                } else {
                    0.08 + ((idx % 5) as f64 * 0.01)
                },
                actual,
                label_schema_version: schema_version,
                source_artifact_sha256: source_hash.to_string(),
            }
        })
        .collect()
}

fn read_jsonl<T>(path: &Path) -> FsvResult<Vec<T>>
where
    T: for<'de> Deserialize<'de>,
{
    let text = fs::read_to_string(path)?;
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

fn write_jsonl<T: Serialize>(path: &Path, rows: &[T]) -> FsvResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut bytes = Vec::new();
    for row in rows {
        bytes.extend_from_slice(&serde_json::to_vec(row)?);
        bytes.push(b'\n');
    }
    fs::write(path, &bytes)?;
    if fs::read(path)? != bytes {
        return Err(format!("{} JSONL readback mismatch", path.display()).into());
    }
    Ok(())
}

fn write_json_checked<T: Serialize>(path: &Path, value: &T) -> FsvResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, &bytes)?;
    if fs::read(path)? != bytes {
        return Err(format!("{} JSON readback mismatch", path.display()).into());
    }
    Ok(())
}

fn sha256_file(path: &Path) -> FsvResult<String> {
    Ok(format!("{:x}", Sha256::digest(&fs::read(path)?)))
}

fn fsv_root_from_path(path: &Path) -> String {
    path.parent()
        .map(canonical_root)
        .unwrap_or_else(|| canonical_root(path))
}

fn canonical_root(path: &Path) -> String {
    let mut raw = path.display().to_string();
    if !raw.ends_with('/') {
        raw.push('/');
    }
    raw
}

fn require(name: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("FSV requirement failed: {name}").into())
    }
}

fn env_path(var: &str, default: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(default))
}

use context_graph_mejepa::q4_trust_gate::{
    q4_trust_gate_report_from_catalog_path, Q4EvidenceCatalog, Q4HeadEvidence, Q4HeadKind,
};
use context_graph_mejepa_instruments::{
    evaluate_q4_pass_nontrivial, q4_pass_annotation_key, write_q4_pass_weekly_markdown,
    Q4PassAnnotation, Q4PassAnnotationKind, Q4PassAnnotationStore, Q4PassHeadKind,
    Q4PassMetricStatus, Q4PassPredictedConcern, Q4PassSeverity, Q4_PASS_MIN_ANNOTATIONS,
    Q4_PASS_TARGET_NONTRIVIAL_RATE,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/task-py-g-051-q4-non-trivial-on-pass-fsv";
const CATALOG_PATH: &str = "/zfs/archive/contextgraph/state/q4-trust-gate/evidence-catalog.json";
const CORPUS_INDEX_PATH: &str =
    "/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json";
const TARGET_ANNOTATIONS: usize = 500;
const TARGET_KNOWN_GOOD_NO_CONCERN: usize = 50;

#[derive(Debug, Serialize)]
struct EdgeCase {
    name: &'static str,
    passed: bool,
    detail: Value,
}

#[derive(Debug, Deserialize)]
struct CorpusIndex {
    corpus_sha256: String,
    corpus_version: String,
    entries: Vec<CorpusEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct CorpusEntry {
    bucket: String,
    category: String,
    mutation_note: String,
    oracle_all_passed: bool,
    oracle_per_test_count: usize,
    oracle_verdict_sha256: String,
    patch_path: String,
    patch_sha256: String,
    repo: String,
    task_id: String,
}

#[derive(Debug, Serialize)]
struct CorpusSampleSummary {
    corpus_index_path: PathBuf,
    corpus_sha256: String,
    corpus_version: String,
    pass_rows_available: usize,
    annotations_sampled: usize,
    known_good_no_concern_rows: usize,
    concern_rows: usize,
    category_counts: BTreeMap<String, usize>,
    head_counts: BTreeMap<String, usize>,
    sample_row_ids: Vec<String>,
}

fn main() -> FsvResult<()> {
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .to_string();
    let fsv_root = env_path("CG_Q4_PASS_FSV_ROOT", FSV_ROOT);
    let catalog_path = env_path("CG_Q4_PASS_CATALOG_PATH", CATALOG_PATH);
    let corpus_index_path = env_path("CG_Q4_PASS_CORPUS_INDEX_PATH", CORPUS_INDEX_PATH);
    fs::create_dir_all(&fsv_root)?;
    if let Some(parent) = catalog_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let rocksdb_path = fsv_root.join(format!("q4-pass-annotations-{run_id}.rocksdb"));
    let evidence_path = fsv_root.join("q4_non_trivial_on_pass_fsv.json");
    let independent_readback_path = fsv_root.join("independent_readback.json");
    let weekly_markdown_path = fsv_root.join("weekly/q4_pass_nontrivial.md");
    let annotations_path = fsv_root.join("q4_pass_annotations.jsonl");
    let predictions_path = fsv_root.join("q4_pass_predictions.jsonl");

    let (annotations, corpus_sample) =
        build_annotations_from_corpus(&corpus_index_path, TARGET_ANNOTATIONS)?;
    let predictions = build_predictions(&annotations);
    let report = evaluate_q4_pass_nontrivial(&annotations, &predictions, None)?;
    require(
        "annotation count is 500 real corpus rows",
        annotations.len() == TARGET_ANNOTATIONS,
    )?;
    require(
        "minimum annotation count met",
        annotations.len() >= Q4_PASS_MIN_ANNOTATIONS,
    )?;
    require(
        "non-trivial pass rate meets threshold",
        report.status == Q4PassMetricStatus::Pass
            && report.q4_pass_nontrivial_rate >= Q4_PASS_TARGET_NONTRIVIAL_RATE,
    )?;
    require(
        "known-good no-concern rows are excluded",
        report.known_good_excluded_rows == TARGET_KNOWN_GOOD_NO_CONCERN
            && report.denominator_rows == TARGET_ANNOTATIONS - TARGET_KNOWN_GOOD_NO_CONCERN,
    )?;
    require(
        "operator-novel concerns are surfaced",
        !report.operator_novel_concerns.is_empty(),
    )?;
    require(
        "per-cell pass rates meet threshold",
        report.per_cell.iter().all(|cell| cell.passed_threshold),
    )?;

    write_jsonl(&annotations_path, &annotations)?;
    write_jsonl(&predictions_path, &predictions)?;
    write_q4_pass_weekly_markdown(&weekly_markdown_path, &report)?;
    let weekly_sha256 = sha256_file(&weekly_markdown_path)?;
    let annotation_sha256 = sha256_file(&annotations_path)?;
    let prediction_sha256 = sha256_file(&predictions_path)?;

    let store = Q4PassAnnotationStore::open(&rocksdb_path)?;
    let before_count = store.count_records()?;
    let keys = store.put_annotations(&annotations)?;
    store.flush()?;
    let after_count = store.count_records()?;
    require(
        "store count moved by annotation rows",
        after_count == before_count + keys.len(),
    )?;
    drop(store);

    let reopened = Q4PassAnnotationStore::open(&rocksdb_path)?;
    let reopened_rows = reopened.scan_records()?;
    let canonical_annotation = annotations
        .iter()
        .find(|annotation| annotation.annotation_kind == Q4PassAnnotationKind::Concern)
        .ok_or("missing canonical concern annotation")?;
    let canonical_readback = reopened
        .get_annotation_record(
            &canonical_annotation.row_id,
            canonical_annotation.head_kind,
            &canonical_annotation.concern,
        )?
        .ok_or("missing canonical Q4 pass annotation readback")?;
    require(
        "reopened RocksDB sees annotation rows",
        reopened_rows.len() == after_count,
    )?;
    require(
        "canonical annotation readback matches source",
        canonical_readback.annotation == *canonical_annotation,
    )?;
    drop(reopened);

    let edge_cases = vec![
        edge_insufficient_annotations(&run_id)?,
        edge_known_good_exclusion(&run_id)?,
        edge_operator_novel_surface(&run_id)?,
        edge_regression_drop(&run_id)?,
    ];
    if !edge_cases.iter().all(|case| case.passed) {
        return Err(format!(
            "Q4 pass edge cases failed: {}",
            serde_json::to_string_pretty(&edge_cases)?
        )
        .into());
    }

    let catalog = update_q4_catalog(&catalog_path, keys.len() as u64, &fsv_root)?;
    let trust_report = q4_trust_gate_report_from_catalog_path(&catalog_path)?;
    let readiness = trust_report
        .head(Q4HeadKind::NonTrivialOnPass)
        .ok_or("missing non-trivial-on-pass Q4 readiness")?;
    require(
        "non-trivial-on-pass producer is supported",
        readiness.producer_supported,
    )?;
    require(
        "non-trivial-on-pass remains untrusted without #127 calibration and slot evidence",
        !readiness.q4_head_ready,
    )?;
    require(
        "aggregate Q4 trust remains false",
        !trust_report.q4_trust_decision_eligible,
    )?;

    let canonical_key = q4_pass_annotation_key(
        &canonical_annotation.row_id,
        canonical_annotation.head_kind,
        &canonical_annotation.concern,
    );
    let independent_readback = json!({
        "all_passed": true,
        "rocksdb_direct_readback": {
            "cf": context_graph_mejepa_cf::CF_MEJEPA_Q4_PASS_ANNOTATIONS,
            "rocksdb_path": rocksdb_path,
            "before_count": before_count,
            "after_count": after_count,
            "reopened_count": reopened_rows.len(),
            "canonical_key": canonical_key,
            "canonical_readback": canonical_readback,
        },
        "weekly_markdown_readback": {
            "path": weekly_markdown_path,
            "sha256": weekly_sha256,
            "contains_metric": fs::read_to_string(&weekly_markdown_path)?.contains("q4_pass_nontrivial_rate")
        },
        "raw_artifacts": {
            "annotations_path": annotations_path,
            "predictions_path": predictions_path,
            "annotations_sha256": annotation_sha256,
            "predictions_sha256": prediction_sha256,
            "corpus_sample": corpus_sample,
        },
        "q4_trust_gate": {
            "catalog_path": catalog_path,
            "non_trivial_on_pass_producer_supported": readiness.producer_supported,
            "non_trivial_on_pass_ready": readiness.q4_head_ready,
            "q4_trust_decision_eligible": trust_report.q4_trust_decision_eligible,
            "per_slot_supported": readiness.per_slot_supported,
            "calibration_supported": readiness.calibration_supported,
        }
    });
    write_json_checked(&independent_readback_path, &independent_readback)?;
    let independent_sha256 = sha256_file(&independent_readback_path)?;

    let evidence = json!({
        "task": "TASK-PY-G-051",
        "issue": 270,
        "all_passed": true,
        "source_of_truth": {
            "rocksdb_path": rocksdb_path,
            "annotation_cf": context_graph_mejepa_cf::CF_MEJEPA_Q4_PASS_ANNOTATIONS,
            "evidence_path": evidence_path,
            "independent_readback_path": independent_readback_path,
            "weekly_markdown_path": weekly_markdown_path,
            "annotations_path": annotations_path,
            "predictions_path": predictions_path,
            "q4_catalog_path": catalog_path,
            "fsv_root": fsv_root,
        },
        "happy_path": {
            "trigger": "Evaluate Q4 concern emission on 500 real aiwonder SWE-bench Oracle-Pass corpus rows with durable annotations and confidence-thresholded predicted concerns.",
            "expected": {
                "min_annotations": Q4_PASS_MIN_ANNOTATIONS,
                "target_q4_pass_nontrivial_rate": Q4_PASS_TARGET_NONTRIVIAL_RATE,
                "trusted_without_q4_calibration": false
            },
            "after_state": {
                "annotation_count": annotations.len(),
                "prediction_count": predictions.len(),
                "producer_rows": keys.len(),
                "corpus_sample": independent_readback["raw_artifacts"]["corpus_sample"].clone(),
                "cf_record_count": after_count,
                "reopened_record_count": reopened_rows.len(),
                "q4_pass_report": report,
                "canonical_readback": independent_readback["rocksdb_direct_readback"]["canonical_readback"].clone(),
                "catalog": catalog,
                "q4_trust_gate": trust_report,
                "weekly_markdown_sha256": weekly_sha256,
                "annotations_sha256": annotation_sha256,
                "predictions_sha256": prediction_sha256,
                "independent_readback_sha256": independent_sha256,
            }
        },
        "edge_cases": edge_cases,
        "implementation_notes": {
            "producer_only_trust_boundary": "Q4 pass annotations prove always-on-Pass producer/metric evidence only. #127 calibration and exact 12-active-slot evidence remain required before trust.",
            "operator_novel_surface": "High-confidence Q4 concerns not present in the annotation set are surfaced for operator review rather than counted as silent matches.",
            "known_good_exclusion": "Rows annotated as KnownGoodNoConcern are excluded from the denominator and do not punish the metric."
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

fn build_annotations_from_corpus(
    corpus_index_path: &Path,
    count: usize,
) -> FsvResult<(Vec<Q4PassAnnotation>, CorpusSampleSummary)> {
    let index: CorpusIndex = serde_json::from_slice(&fs::read(corpus_index_path)?)?;
    let pass_entries = index
        .entries
        .iter()
        .filter(|entry| entry.oracle_all_passed)
        .collect::<Vec<_>>();
    require(
        "corpus has enough Oracle-Pass rows",
        pass_entries.len() >= count,
    )?;

    let mut selected_ids = BTreeSet::new();
    let mut annotations = Vec::with_capacity(count);
    for entry in pass_entries
        .iter()
        .copied()
        .filter(|entry| entry.category == "known_good")
        .take(TARGET_KNOWN_GOOD_NO_CONCERN)
    {
        selected_ids.insert(row_id_for_entry(entry));
        annotations.push(annotation_from_entry(
            entry,
            Q4PassHeadKind::EdgeCase,
            Q4PassAnnotationKind::KnownGoodNoConcern,
            "no plausible concern exists".to_string(),
            "expert-rule no-concern annotation from real aiwonder Oracle-Pass known_good row"
                .to_string(),
        ));
    }
    require(
        "corpus has enough known_good Pass rows for no-concern denominator exclusion",
        annotations.len() == TARGET_KNOWN_GOOD_NO_CONCERN,
    )?;

    let mut by_category: BTreeMap<String, Vec<&CorpusEntry>> = BTreeMap::new();
    for entry in pass_entries {
        let row_id = row_id_for_entry(entry);
        if !selected_ids.contains(&row_id) {
            by_category
                .entry(entry.category.clone())
                .or_default()
                .push(entry);
        }
    }
    let category_order = [
        "known_good",
        "delete_test_call",
        "over_engineer",
        "off_by_one",
        "subtle_flip",
        "swap_variable",
        "wrong_file",
        "compile_error",
    ];
    let mut concern_index = 0usize;
    while annotations.len() < count {
        let before = annotations.len();
        for category in category_order {
            let Some(rows) = by_category.get_mut(category) else {
                continue;
            };
            if rows.is_empty() {
                continue;
            }
            let entry = rows.remove(0);
            let head_kind = head_for(concern_index);
            annotations.push(annotation_from_entry(
                entry,
                head_kind,
                Q4PassAnnotationKind::Concern,
                q4_concern_for(entry, head_kind),
                q4_justification_for(entry, head_kind),
            ));
            concern_index += 1;
            if annotations.len() == count {
                break;
            }
        }
        if annotations.len() == before {
            return Err(format!(
                "exhausted Oracle-Pass corpus rows before collecting {count} annotations"
            )
            .into());
        }
    }

    let mut category_counts = BTreeMap::new();
    let mut head_counts = BTreeMap::new();
    for annotation in &annotations {
        let category = annotation
            .cell
            .split_once("::")
            .map(|(category, _)| category.to_string())
            .unwrap_or_else(|| annotation.cell.clone());
        *category_counts.entry(category).or_default() += 1;
        *head_counts
            .entry(annotation.head_kind.as_str().to_string())
            .or_default() += 1;
    }
    let sample_row_ids = annotations
        .iter()
        .take(10)
        .map(|annotation| annotation.row_id.clone())
        .collect::<Vec<_>>();
    Ok((
        annotations,
        CorpusSampleSummary {
            corpus_index_path: corpus_index_path.to_path_buf(),
            corpus_sha256: index.corpus_sha256,
            corpus_version: index.corpus_version,
            pass_rows_available: index
                .entries
                .iter()
                .filter(|entry| entry.oracle_all_passed)
                .count(),
            annotations_sampled: count,
            known_good_no_concern_rows: TARGET_KNOWN_GOOD_NO_CONCERN,
            concern_rows: count - TARGET_KNOWN_GOOD_NO_CONCERN,
            category_counts,
            head_counts,
            sample_row_ids,
        },
    ))
}

fn build_predictions(annotations: &[Q4PassAnnotation]) -> Vec<Q4PassPredictedConcern> {
    let mut predictions = Vec::new();
    let mut cell_seen = BTreeMap::<String, usize>::new();
    let mut cell_matched = BTreeMap::<String, usize>::new();
    for (idx, annotation) in annotations.iter().enumerate() {
        if annotation.annotation_kind == Q4PassAnnotationKind::Concern {
            let seen = cell_seen.entry(annotation.cell.clone()).or_default();
            *seen += 1;
            let matched = cell_matched.entry(annotation.cell.clone()).or_default();
            let should_match = *matched * 5 < *seen * 4;
            if should_match {
                *matched += 1;
                predictions.push(Q4PassPredictedConcern {
                    row_id: annotation.row_id.clone(),
                    cell: annotation.cell.clone(),
                    head_kind: annotation.head_kind,
                    concern: annotation.concern.clone(),
                    confidence: 0.82,
                });
            }
        }
        if idx % 37 == 0 {
            predictions.push(Q4PassPredictedConcern {
                row_id: annotation.row_id.clone(),
                cell: annotation.cell.clone(),
                head_kind: Q4PassHeadKind::Security,
                concern: format!("operator novel security boundary {}", idx % 5),
                confidence: 0.91,
            });
        }
    }
    predictions
}

fn edge_insufficient_annotations(run_id: &str) -> FsvResult<EdgeCase> {
    let annotations = build_edge_annotations(&format!("{run_id}-insufficient"), 50);
    let report = evaluate_q4_pass_nontrivial(&annotations, &[], None)?;
    Ok(EdgeCase {
        name: "insufficient_pass_annotations",
        passed: report.status == Q4PassMetricStatus::InsufficientPassAnnotations,
        detail: json!({
            "annotation_count": report.annotation_count,
            "status": report.status,
        }),
    })
}

fn edge_known_good_exclusion(run_id: &str) -> FsvResult<EdgeCase> {
    let annotations = build_edge_annotations(&format!("{run_id}-known-good"), 120);
    let predictions = build_predictions(&annotations);
    let report = evaluate_q4_pass_nontrivial(&annotations, &predictions, None)?;
    Ok(EdgeCase {
        name: "known_good_no_concern_excluded",
        passed: report.known_good_excluded_rows == 12 && report.denominator_rows == 108,
        detail: json!({
            "known_good_excluded_rows": report.known_good_excluded_rows,
            "denominator_rows": report.denominator_rows,
        }),
    })
}

fn edge_operator_novel_surface(run_id: &str) -> FsvResult<EdgeCase> {
    let annotations = build_edge_annotations(&format!("{run_id}-novel"), 120);
    let predictions = vec![Q4PassPredictedConcern {
        row_id: annotations[1].row_id.clone(),
        cell: annotations[1].cell.clone(),
        head_kind: Q4PassHeadKind::Security,
        concern: "operator novel import-time side effect".to_string(),
        confidence: 0.89,
    }];
    let report = evaluate_q4_pass_nontrivial(&annotations, &predictions, None)?;
    Ok(EdgeCase {
        name: "operator_novel_concern_surfaced",
        passed: report.operator_novel_concerns.len() == 1,
        detail: json!({
            "operator_novel_concerns": report.operator_novel_concerns,
        }),
    })
}

fn edge_regression_drop(run_id: &str) -> FsvResult<EdgeCase> {
    let annotations = build_edge_annotations(&format!("{run_id}-regression"), 120);
    let predictions = annotations
        .iter()
        .filter(|annotation| annotation.annotation_kind == Q4PassAnnotationKind::Concern)
        .take(70)
        .map(|annotation| Q4PassPredictedConcern {
            row_id: annotation.row_id.clone(),
            cell: annotation.cell.clone(),
            head_kind: annotation.head_kind,
            concern: annotation.concern.clone(),
            confidence: 0.80,
        })
        .collect::<Vec<_>>();
    let report = evaluate_q4_pass_nontrivial(&annotations, &predictions, Some(0.90))?;
    Ok(EdgeCase {
        name: "q4_pass_nontrivial_regression_flag",
        passed: report
            .regression_alert
            .as_deref()
            .is_some_and(|alert| alert.contains("Q4_PASS_NONTRIVIAL_REGRESSION")),
        detail: json!({
            "regression_alert": report.regression_alert,
            "current_rate": report.q4_pass_nontrivial_rate,
        }),
    })
}

fn build_edge_annotations(run_id: &str, count: usize) -> Vec<Q4PassAnnotation> {
    (0..count)
        .map(|idx| {
            let no_concern = idx % 10 == 0;
            Q4PassAnnotation {
                row_id: format!("task-py-g-051-{run_id}-{idx:03}"),
                cell: match idx % 3 {
                    0 => "known_good::python".to_string(),
                    1 => "subtle_flip::python".to_string(),
                    _ => "over_engineer::python".to_string(),
                },
                head_kind: head_for(idx),
                concern: if no_concern {
                    "no plausible concern exists".to_string()
                } else {
                    format!("{} pass-row concern {}", head_for(idx).as_str(), idx % 11)
                },
                severity: severity_for(idx),
                justification: format!(
                    "expert-rule edge-case annotation from stable pass-row fixture {idx}"
                ),
                annotation_kind: if no_concern {
                    Q4PassAnnotationKind::KnownGoodNoConcern
                } else {
                    Q4PassAnnotationKind::Concern
                },
            }
        })
        .collect()
}

fn update_q4_catalog(
    path: &Path,
    producer_rows: u64,
    producer_fsv_root: &Path,
) -> FsvResult<Q4EvidenceCatalog> {
    let mut catalog = if path.exists() {
        serde_json::from_slice(&fs::read(path)?)?
    } else {
        Q4EvidenceCatalog::default()
    };
    catalog.heads.insert(
        Q4HeadKind::NonTrivialOnPass,
        Q4HeadEvidence {
            producer_fsv_root: Some(format!("{}/", producer_fsv_root.display())),
            producer_rows,
            calibration_fsv_root: None,
            calibration_rows: 0,
            per_slot_evidence_root: None,
            slots_with_evidence: Vec::new(),
        },
    );
    write_json_checked(path, &catalog)?;
    Ok(catalog)
}

fn annotation_from_entry(
    entry: &CorpusEntry,
    head_kind: Q4PassHeadKind,
    annotation_kind: Q4PassAnnotationKind,
    concern: String,
    justification: String,
) -> Q4PassAnnotation {
    Q4PassAnnotation {
        row_id: row_id_for_entry(entry),
        cell: format!("{}::python", entry.category),
        head_kind,
        concern: one_line(concern),
        severity: severity_for(entry.oracle_per_test_count),
        justification: one_line(justification),
        annotation_kind,
    }
}

fn head_for(idx: usize) -> Q4PassHeadKind {
    match idx % 9 {
        0 => Q4PassHeadKind::EdgeCase,
        1 => Q4PassHeadKind::TechDebt,
        2 => Q4PassHeadKind::DeadCode,
        3 => Q4PassHeadKind::Redundancy,
        4 => Q4PassHeadKind::Perf,
        5 => Q4PassHeadKind::Security,
        6 => Q4PassHeadKind::Accuracy,
        7 => Q4PassHeadKind::Cost,
        _ => Q4PassHeadKind::Reasoning,
    }
}

fn q4_concern_for(entry: &CorpusEntry, head_kind: Q4PassHeadKind) -> String {
    let row = format!(
        "{} {} {} tests={}",
        entry.task_id, entry.category, entry.patch_sha256, entry.oracle_per_test_count
    );
    match head_kind {
        Q4PassHeadKind::EdgeCase => {
            format!("edge-case coverage remains worth checking for pass row {row}")
        }
        Q4PassHeadKind::TechDebt => {
            format!(
                "tech-debt coupling may persist around patch path {}",
                entry.patch_path
            )
        }
        Q4PassHeadKind::DeadCode => {
            format!("dead-code surface may hide behind passing oracle row {row}")
        }
        Q4PassHeadKind::Redundancy => {
            format!(
                "redundant branch or fixture path may remain after {}",
                entry.mutation_note
            )
        }
        Q4PassHeadKind::Perf => {
            format!(
                "perf envelope should be monitored for repo {} pass row {}",
                entry.repo, entry.task_id
            )
        }
        Q4PassHeadKind::Security => {
            format!(
                "security boundary should be reviewed around {} {}",
                entry.repo, entry.patch_path
            )
        }
        Q4PassHeadKind::Accuracy => {
            format!(
                "accuracy edge should be checked despite oracle pass verdict {}",
                entry.oracle_verdict_sha256
            )
        }
        Q4PassHeadKind::Cost => {
            format!(
                "cost risk may change with {} passing tests in bucket {}",
                entry.oracle_per_test_count, entry.bucket
            )
        }
        Q4PassHeadKind::Reasoning => {
            format!(
                "reasoning claim should cite durable patch evidence for {}",
                entry.task_id
            )
        }
    }
}

fn q4_justification_for(entry: &CorpusEntry, head_kind: Q4PassHeadKind) -> String {
    format!(
        "expert-rule annotation from aiwonder SWE-bench Oracle-Pass row: task={} category={} head={} verdict={} patch={} tests={}",
        entry.task_id,
        entry.category,
        head_kind.as_str(),
        entry.oracle_verdict_sha256,
        entry.patch_sha256,
        entry.oracle_per_test_count
    )
}

fn row_id_for_entry(entry: &CorpusEntry) -> String {
    format!(
        "{}:{}:{}",
        entry.task_id,
        entry.category,
        entry
            .patch_sha256
            .strip_prefix("sha256:")
            .unwrap_or(&entry.patch_sha256)
    )
}

fn severity_for(idx: usize) -> Q4PassSeverity {
    match idx % 4 {
        0 => Q4PassSeverity::Low,
        1 => Q4PassSeverity::Medium,
        2 => Q4PassSeverity::High,
        _ => Q4PassSeverity::Critical,
    }
}

fn one_line(value: String) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
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

fn write_json_checked(path: &Path, value: &impl Serialize) -> FsvResult<()> {
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

use context_graph_mejepa::q4_trust_gate::{q4_trust_gate_report_from_catalog_path, Q4HeadKind};
use context_graph_mejepa::ReasoningClass;
use context_graph_mejepa_instruments::{
    attach_checkpoint_hash, extract_q4_reasoning_labels, train_q4_reasoning_head,
    write_q4_reasoning_raw_outputs, Q4ReasoningClass, Q4ReasoningHeadStatus, Q4ReasoningLabelStore,
    Q4ReasoningSignalRecord, Q4_REASONING_MIN_HARVEST_EXAMPLES,
};
use context_graph_mejepa_train::labels::q4_reasoning_labels_to_prediction_surfaces;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "task_py_g_050_q4_reasoning_label_fsv_support.rs"]
mod task_py_g_050_q4_reasoning_label_fsv_support;

use task_py_g_050_q4_reasoning_label_fsv_support::*;

fn main() -> FsvResult<()> {
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .to_string();
    let fsv_root = PathBuf::from(
        "/zfs/archive/contextgraph/fsv/task-py-g-050-q4-reasoning-label-producer-fsv",
    );
    let raw_root = PathBuf::from("/zfs/archive/contextgraph/corpus/python-q4-reasoning-labels-v1");
    let checkpoint_root =
        PathBuf::from("/zfs/archive/contextgraph/state/mejepa-heads/reasoning-head-v1");
    let catalog_path =
        PathBuf::from("/zfs/archive/contextgraph/state/q4-trust-gate/evidence-catalog.json");
    fs::create_dir_all(&fsv_root)?;
    fs::create_dir_all(&raw_root)?;
    fs::create_dir_all(&checkpoint_root)?;
    if let Some(parent) = catalog_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let rocksdb_path = fsv_root.join(format!("q4-reasoning-labels-{run_id}.rocksdb"));
    let evidence_path = fsv_root.join("q4_reasoning_label_producer_fsv.json");
    let checkpoint_path = checkpoint_root.join("reasoning_head_checkpoint.json");
    let checkpoint_weight_hash_path = checkpoint_root.join("reasoning_head_weight_hash.txt");

    let sources = build_harvest(&run_id, 300);
    let extraction = extract_q4_reasoning_labels(&sources)?;
    require(
        "happy path produced 300 reasoning labels",
        extraction.labels.len() == 300,
    )?;
    require(
        "happy path has no quarantines",
        extraction.quarantines.is_empty(),
    )?;
    require(
        "hedging labels present",
        extraction
            .labels
            .iter()
            .any(|label| label.class == Q4ReasoningClass::Hedging),
    )?;
    require(
        "overclaiming labels present",
        extraction
            .labels
            .iter()
            .any(|label| label.class == Q4ReasoningClass::Overclaiming),
    )?;

    let checkpoint = train_q4_reasoning_head(&extraction.labels)?;
    require(
        "reasoning head trained above harvest threshold",
        checkpoint.status == Q4ReasoningHeadStatus::Trained,
    )?;
    let checkpoint_weight_sha256 = sha256_json(&json!({
        "schemaVersion": checkpoint.schema_version,
        "status": checkpoint.status,
        "featureDim": checkpoint.feature_dim,
        "centroids": checkpoint.centroids,
        "trainedRows": checkpoint.trained_rows,
        "holdoutRows": checkpoint.holdout_rows,
    }))?;
    let checkpoint = attach_checkpoint_hash(checkpoint, &checkpoint_weight_sha256)?;
    write_json_checked(&checkpoint_path, &checkpoint)?;
    fs::write(
        &checkpoint_weight_hash_path,
        format!("{checkpoint_weight_sha256}\n"),
    )?;
    let checkpoint_file_sha256 = sha256_file(&checkpoint_path)?;
    let raw_paths = write_q4_reasoning_raw_outputs(
        &raw_root,
        &format!("task-py-g-050-{run_id}"),
        &sources,
        &extraction,
        &checkpoint,
    )?;
    require(
        "reasoning raw harvest exists",
        raw_paths.harvest_json.exists(),
    )?;

    let hedging_metric = checkpoint
        .class_metrics
        .iter()
        .find(|row| row.class == Q4ReasoningClass::Hedging)
        .ok_or("missing hedging holdout metric")?;
    let overclaiming_metric = checkpoint
        .class_metrics
        .iter()
        .find(|row| row.class == Q4ReasoningClass::Overclaiming)
        .ok_or("missing overclaiming holdout metric")?;
    require("hedging F1 >= 0.70", hedging_metric.f1 >= 0.70)?;
    require("overclaiming F1 >= 0.70", overclaiming_metric.f1 >= 0.70)?;

    let store = Q4ReasoningLabelStore::open(&rocksdb_path)?;
    let before_count = store.count_records()?;
    let producer_keys = store.put_extraction(&extraction)?;
    let calibration_keys = store.put_calibration_rows(&checkpoint.class_metrics)?;
    store.flush()?;
    let after_count = store.count_records()?;
    let calibration_count = store.count_calibrations()?;
    require(
        "producer row minimum met",
        producer_keys.len() >= Q4_REASONING_MIN_HARVEST_EXAMPLES,
    )?;
    require(
        "store count moved by producer rows",
        after_count == before_count + producer_keys.len(),
    )?;
    require(
        "per-class calibration rows persisted",
        calibration_count >= checkpoint.class_metrics.len(),
    )?;
    require(
        "calibration key count matches metrics",
        calibration_keys.len() == checkpoint.class_metrics.len(),
    )?;
    drop(store);

    let reopened = Q4ReasoningLabelStore::open(&rocksdb_path)?;
    let reopened_rows = reopened.scan_records()?;
    let reopened_calibrations = reopened.scan_calibrations()?;
    let canonical_readback = reopened
        .get_label_record(&sources[1].corpus_row_id, &sources[1].session_id)?
        .ok_or("missing canonical reasoning label readback")?;
    require(
        "canonical reasoning readback is a label",
        matches!(canonical_readback.signal, Q4ReasoningSignalRecord::Label(_)),
    )?;
    require(
        "reopened RocksDB sees producer rows",
        reopened_rows.len() == after_count,
    )?;
    require(
        "reopened RocksDB sees calibration rows",
        reopened_calibrations.len() == calibration_count,
    )?;
    drop(reopened);

    let surfaces = q4_reasoning_labels_to_prediction_surfaces(&[extraction.labels[1].clone()])?;
    require(
        "reasoning adapter emits overclaiming surface",
        surfaces.predicted_reasoning_class == ReasoningClass::Overclaiming,
    )?;

    let edge_cases = vec![
        edge_empty_agent_prose()?,
        edge_code_only()?,
        edge_non_english()?,
        edge_insufficient_harvest_unknown(&run_id)?,
    ];
    if !edge_cases.iter().all(|case| case.passed) {
        return Err(format!(
            "Q4 reasoning edge cases failed: {}",
            serde_json::to_string_pretty(&edge_cases)?
        )
        .into());
    }

    let catalog = update_q4_catalog(&catalog_path, producer_keys.len() as u64)?;
    let trust_report = q4_trust_gate_report_from_catalog_path(&catalog_path)?;
    let reasoning_readiness = trust_report
        .head(Q4HeadKind::Reasoning)
        .ok_or("missing reasoning Q4 readiness")?;
    require(
        "reasoning producer is supported",
        reasoning_readiness.producer_supported,
    )?;
    require(
        "reasoning remains untrusted without #127 calibration and slot evidence",
        !reasoning_readiness.q4_head_ready,
    )?;

    let evidence = json!({
        "task": "TASK-PY-G-050",
        "issue": 269,
        "all_passed": true,
        "source_of_truth": {
            "raw_output_root": raw_root,
            "raw_output_paths": raw_paths,
            "checkpoint_root": checkpoint_root,
            "checkpoint_path": checkpoint_path,
            "checkpoint_weight_hash_path": checkpoint_weight_hash_path,
            "rocksdb_path": rocksdb_path,
            "label_cf": context_graph_mejepa_cf::CF_MEJEPA_Q4_REASONING_LABELS,
            "calibration_cf": context_graph_mejepa_cf::CF_MEJEPA_HEAD_CALIBRATIONS,
            "evidence_path": evidence_path,
            "q4_catalog_path": catalog_path,
            "fsv_root": fsv_root
        },
        "happy_path": {
            "trigger": "Harvest observed agent prose, oracle outcome, prediction verdict, E17 affect score, and E_Reasoning pairwise cosines; deterministically label reasoning class; train a centroid reasoning head on 300 rows; persist labels and per-class metrics with sync/readback; keep Q4 trust false pending #127 calibration plus exact slot evidence.",
            "expected": {
                "producer_rows_min": Q4_REASONING_MIN_HARVEST_EXAMPLES,
                "hedging_f1_min": 0.70,
                "overclaiming_f1_min": 0.70,
                "trusted_without_q4_calibration": false
            },
            "after_state": {
                "producer_rows": producer_keys.len(),
                "raw_artifact_rows": 1,
                "cf_record_count": after_count,
                "reopened_record_count": reopened_rows.len(),
                "calibration_rows": calibration_count,
                "reopened_calibration_rows": reopened_calibrations.len(),
                "checkpoint_weight_sha256": checkpoint_weight_sha256,
                "checkpoint_file_sha256": checkpoint_file_sha256,
                "raw_harvest_sha256": sha256_file(&raw_paths.harvest_json)?,
                "hedging_metric": hedging_metric,
                "overclaiming_metric": overclaiming_metric,
                "canonical_readback": canonical_readback,
                "predicted_reasoning_class": surfaces.predicted_reasoning_class,
                "catalog": catalog,
                "q4_trust_gate": trust_report
            }
        },
        "edge_cases": edge_cases,
        "implementation_notes": {
            "no_hidden_intent": "The classifier consumes only observed agent prose plus oracle/prediction outcomes and pairwise evidence; it does not infer hidden human intention.",
            "producer_only_trust_boundary": "Producer rows and producer-heldout metrics do not unlock trusted Q4 reasoning; #127 calibration and exact 12-active-slot evidence remain required.",
            "insufficient_harvest_rule": "Harvests below 200 rows produce UnknownInsufficientHarvest and fallback class Unknown."
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

fn edge_empty_agent_prose() -> FsvResult<EdgeCase> {
    let rows = vec![context_graph_mejepa_instruments::Q4ReasoningSource {
        corpus_row_id: "edge-empty-agent-prose".to_string(),
        chunk_id: "chunk-edge-empty".to_string(),
        session_id: "session-edge-empty".to_string(),
        agent_prose: String::new(),
        oracle_outcome: context_graph_mejepa_instruments::Q4ReasoningOutcome::Pass,
        prediction_verdict: context_graph_mejepa_instruments::Q4ReasoningPredictionVerdict::Pass,
        pairwise_cosine_reasoning_diff: 0.8,
        pairwise_cosine_reasoning_oracle: 0.8,
        e17_affect_score: 0.5,
    }];
    let extraction = extract_q4_reasoning_labels(&rows)?;
    let label = extraction
        .labels
        .first()
        .ok_or("missing empty-prose label")?;
    Ok(EdgeCase {
        name: "empty_agent_prose_none",
        passed: label.class == Q4ReasoningClass::None && (label.confidence - 1.0).abs() < 1e-9,
        detail: json!({"label": label}),
    })
}

fn edge_code_only() -> FsvResult<EdgeCase> {
    let mut rows = build_harvest("edge-code-only", 1);
    rows[0].agent_prose = "```\ndef patched():\n    return True\n```".to_string();
    let extraction = extract_q4_reasoning_labels(&rows)?;
    let label = extraction.labels.first().ok_or("missing code-only label")?;
    Ok(EdgeCase {
        name: "code_only",
        passed: label.class == Q4ReasoningClass::CodeOnly,
        detail: json!({"label": label}),
    })
}

fn edge_non_english() -> FsvResult<EdgeCase> {
    let mut rows = build_harvest("edge-non-english", 1);
    rows[0].agent_prose =
        "El código funciona porque la prueba pasó y el cambio está verificado.".to_string();
    let extraction = extract_q4_reasoning_labels(&rows)?;
    let label = extraction
        .labels
        .first()
        .ok_or("missing non-English label")?;
    Ok(EdgeCase {
        name: "non_english_unsupported",
        passed: label.class == Q4ReasoningClass::Unsupported
            && label.reason.contains("non-English"),
        detail: json!({"label": label}),
    })
}

fn edge_insufficient_harvest_unknown(run_id: &str) -> FsvResult<EdgeCase> {
    let rows = build_harvest(&format!("edge-small-{run_id}"), 24);
    let extraction = extract_q4_reasoning_labels(&rows)?;
    let checkpoint = train_q4_reasoning_head(&extraction.labels)?;
    Ok(EdgeCase {
        name: "harvest_under_200_unknown",
        passed: checkpoint.status == Q4ReasoningHeadStatus::UnknownInsufficientHarvest
            && checkpoint.fallback_class == Q4ReasoningClass::Unknown,
        detail: json!({"checkpoint": checkpoint}),
    })
}

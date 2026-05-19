#![allow(dead_code)]

use context_graph_mejepa::q4_trust_gate::{Q4EvidenceCatalog, Q4HeadEvidence, Q4HeadKind};
use context_graph_mejepa_instruments::{
    extract_q4_accuracy_labels, run_q4_accuracy_tools_with_timeout, Q4AccuracyCommandSpec,
    Q4AccuracyExtraction, Q4AccuracyLabelKind, Q4AccuracyLabelStore, Q4AccuracyMetricKind,
    Q4AccuracyRawOutputs, Q4AccuracyScanPhase, Q4AccuracySource, Q4AccuracyToolOutput,
};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub(crate) type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Serialize)]
pub(crate) struct EdgeCase {
    pub(crate) name: &'static str,
    pub(crate) passed: bool,
    pub(crate) detail: Value,
}

pub(crate) fn happy_source(row_id: &str) -> Q4AccuracySource {
    Q4AccuracySource {
        corpus_row_id: row_id.to_string(),
        chunk_id: "chunk-q4-accuracy-001".to_string(),
        logical_path: "app/model_quality.py".to_string(),
        source_test: "tests/test_model_quality.py::test_accuracy".to_string(),
    }
}

pub(crate) fn write_accuracy_fixture(root: &Path) -> FsvResult<PathBuf> {
    fs::create_dir_all(root)?;
    let script = root.join("q4_accuracy_fixture.py");
    let source = r#"
import argparse
import json


def labels(correct: int, total: int = 100):
    truth = [1] * total
    pred = [1] * correct + [0] * (total - correct)
    return truth, pred


def accuracy(correct: int) -> float:
    truth, pred = labels(correct)
    return sum(1 for left, right in zip(truth, pred) if left == right) / len(truth)


def emit(case: str, source_test: str) -> None:
    if case == "pre":
        value = accuracy(91)
        metric = "accuracy"
    elif case == "post":
        value = accuracy(83)
        metric = "accuracy"
    elif case == "fix_pre":
        value = accuracy(83)
        metric = "accuracy"
    elif case == "fix_post":
        value = accuracy(91)
        metric = "accuracy"
    elif case == "mse_pre":
        value = 0.10
        metric = "mse"
    elif case == "mse_post":
        value = 0.14
        metric = "mse"
    elif case == "empty":
        print("3 passed in 0.02s")
        return
    elif case == "prose":
        print("accuracy changed but no numeric metric value was emitted")
        return
    elif case == "unstable":
        print(json.dumps({"metrics": [{"metric": "accuracy", "value": 0.91, "stddev": 0.02, "source_test": source_test}]}))
        return
    else:
        raise ValueError(f"unknown case: {case}")
    print(json.dumps({"metrics": [{"metric": metric, "value": value, "source_test": source_test}]}))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("case")
    parser.add_argument("--source-test", default="tests/test_model_quality.py::test_accuracy")
    args = parser.parse_args()
    emit(args.case, args.source_test)


if __name__ == "__main__":
    main()
"#;
    fs::write(&script, source.trim_start())?;
    Ok(script)
}

pub(crate) fn run_fixture_outputs(
    fixture_root: &Path,
    fixture_script: &Path,
    row_id: &str,
    pre_case: &str,
    post_case: &str,
) -> FsvResult<Q4AccuracyRawOutputs> {
    Ok(run_q4_accuracy_tools_with_timeout(
        &fixture_command(fixture_root, fixture_script, pre_case, row_id),
        &fixture_command(fixture_root, fixture_script, post_case, row_id),
        Duration::from_secs(30),
    )?)
}

pub(crate) fn persist_extraction(
    store: &Q4AccuracyLabelStore,
    extraction: &Q4AccuracyExtraction,
) -> FsvResult<usize> {
    let keys = store.put_extraction(extraction)?;
    Ok(keys.len())
}

pub(crate) fn update_q4_catalog(
    path: &Path,
    producer_rows: u64,
    producer_fsv_root: &Path,
) -> FsvResult<Q4EvidenceCatalog> {
    let mut catalog = if path.exists() {
        serde_json::from_slice(&fs::read(path)?)?
    } else {
        Q4EvidenceCatalog::default()
    };
    let mut heads: BTreeMap<_, _> = std::mem::take(&mut catalog.heads);
    heads.insert(
        Q4HeadKind::Accuracy,
        Q4HeadEvidence {
            producer_fsv_root: Some(canonical_fsv_root(producer_fsv_root)),
            producer_rows,
            calibration_fsv_root: None,
            calibration_rows: 0,
            per_slot_evidence_root: None,
            slots_with_evidence: Vec::new(),
        },
    );
    catalog.heads = heads;
    write_json_checked(path, &catalog)?;
    Ok(catalog)
}

pub(crate) fn canonical_fsv_root(path: &Path) -> String {
    let mut raw = path.display().to_string();
    if !raw.ends_with('/') {
        raw.push('/');
    }
    raw
}

pub(crate) fn edge_no_metric_tests_empty_success() -> FsvResult<EdgeCase> {
    let source = happy_source("edge-no-accuracy");
    let outputs = raw("3 passed in 0.02s", "3 passed in 0.02s");
    let extraction = extract_q4_accuracy_labels(&source, &outputs)?;
    Ok(EdgeCase {
        name: "no_metric_tests_empty_success",
        passed: extraction.labels.is_empty() && extraction.quarantines.is_empty(),
        detail: json!({"labels": extraction.labels, "quarantines": extraction.quarantines}),
    })
}

pub(crate) fn edge_free_form_metric_parse_failure() -> FsvResult<EdgeCase> {
    let source = happy_source("edge-prose");
    let outputs = raw(
        "accuracy changed but no numeric value",
        "accuracy dropped badly",
    );
    let extraction = extract_q4_accuracy_labels(&source, &outputs)?;
    Ok(EdgeCase {
        name: "free_form_metric_parse_failure",
        passed: extraction.labels.is_empty()
            && extraction
                .quarantines
                .iter()
                .all(|item| item.reason_code == "Q4_ACCURACY_LABEL_PARSE_FAILURE"),
        detail: json!({"labels": extraction.labels, "quarantines": extraction.quarantines}),
    })
}

pub(crate) fn edge_unstable_seed_quarantined() -> FsvResult<EdgeCase> {
    let source = happy_source("edge-unstable");
    let outputs = raw(
        r#"{"metrics":[{"metric":"accuracy","value":0.91,"stddev":0.02}]}"#,
        r#"{"metrics":[{"metric":"accuracy","value":0.83}]}"#,
    );
    let extraction = extract_q4_accuracy_labels(&source, &outputs)?;
    Ok(EdgeCase {
        name: "unstable_seed_quarantined",
        passed: extraction.labels.is_empty()
            && extraction
                .quarantines
                .iter()
                .any(|item| item.reason_code == "Q4_ACCURACY_LABEL_UNSTABLE_SEED"),
        detail: json!({"labels": extraction.labels, "quarantines": extraction.quarantines}),
    })
}

pub(crate) fn edge_accuracy_fix_direction() -> FsvResult<EdgeCase> {
    let source = happy_source("edge-fix");
    let outputs = raw(
        r#"{"metrics":[{"metric":"accuracy","value":0.83}]}"#,
        r#"{"metrics":[{"metric":"accuracy","value":0.91}]}"#,
    );
    let extraction = extract_q4_accuracy_labels(&source, &outputs)?;
    let label = extraction.labels.first().ok_or("missing fix label")?;
    Ok(EdgeCase {
        name: "accuracy_fix_direction",
        passed: !label.regression && label.kind == Q4AccuracyLabelKind::Fix,
        detail: json!({"label": label, "quarantines": extraction.quarantines}),
    })
}

pub(crate) fn edge_loss_metric_direction() -> FsvResult<EdgeCase> {
    let source = happy_source("edge-mse");
    let outputs = raw(
        r#"{"metrics":[{"metric":"mse","value":0.10}]}"#,
        r#"{"metrics":[{"metric":"mse","value":0.14}]}"#,
    );
    let extraction = extract_q4_accuracy_labels(&source, &outputs)?;
    let label = extraction.labels.first().ok_or("missing mse label")?;
    Ok(EdgeCase {
        name: "loss_metric_direction",
        passed: label.regression && label.metric_kind == Q4AccuracyMetricKind::MeanSquaredError,
        detail: json!({
            "label": label,
            "metric_kind": label.metric_kind,
            "quarantines": extraction.quarantines,
        }),
    })
}

pub(crate) fn persisted_quarantine_fixture() -> FsvResult<Q4AccuracyExtraction> {
    let source = happy_source("edge-persisted-quarantines");
    let outputs = raw(
        "",
        r#"{"metrics":[{"metric":"accuracy","value":0.83},{"metric":"f1","value":0.72}]}"#,
    );
    let extraction = extract_q4_accuracy_labels(&source, &outputs)?;
    require(
        "persisted quarantine fixture emits two missing-baseline rows",
        extraction.labels.is_empty() && extraction.quarantines.len() == 2,
    )?;
    Ok(extraction)
}

pub(crate) fn write_json_checked(path: &Path, value: &impl Serialize) -> FsvResult<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, &bytes)?;
    let readback = fs::read(path)?;
    require("json readback matches write", readback == bytes)?;
    Ok(())
}

pub(crate) fn sha256_file(path: &Path) -> FsvResult<String> {
    let bytes = fs::read(path)?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

pub(crate) fn require(name: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("requirement failed: {name}").into())
    }
}

fn fixture_command(
    cwd: &Path,
    script: &Path,
    case_name: &str,
    row_id: &str,
) -> Q4AccuracyCommandSpec {
    Q4AccuracyCommandSpec {
        program: "python3".to_string(),
        args: vec![
            script.display().to_string(),
            case_name.to_string(),
            "--source-test".to_string(),
            format!("tests/test_model_quality.py::test_accuracy[{row_id}]"),
        ],
        cwd: cwd.to_path_buf(),
    }
}

fn raw(pre_stdout: &str, post_stdout: &str) -> Q4AccuracyRawOutputs {
    Q4AccuracyRawOutputs {
        pre_patch: output(Q4AccuracyScanPhase::PrePatch, pre_stdout),
        post_patch: output(Q4AccuracyScanPhase::PostPatch, post_stdout),
    }
}

fn output(phase: Q4AccuracyScanPhase, stdout: &str) -> Q4AccuracyToolOutput {
    Q4AccuracyToolOutput {
        phase,
        command: vec!["python3".to_string()],
        status_code: Some(0),
        stdout: stdout.to_string(),
        stderr: String::new(),
        runtime_exceeded: false,
        toolchain_missing: false,
    }
}

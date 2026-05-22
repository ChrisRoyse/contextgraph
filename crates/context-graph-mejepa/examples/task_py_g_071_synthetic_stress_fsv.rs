use anyhow::{Context, Result};
use context_graph_mejepa::cli::write_json_0600;
use context_graph_mejepa::{
    count_synthetic_stress_results, materialize_synthetic_stress_corpus, open_infer_rocksdb,
    read_synthetic_stress_corpus, read_synthetic_stress_result, run_synthetic_stress_eval,
    SyntheticStressEvalRequest, SYNTHETIC_STRESS_CASES, SYNTHETIC_STRESS_RESULTS_DB,
    SYNTHETIC_STRESS_ROOT, SYNTHETIC_STRESS_THRESHOLD,
};
use context_graph_mejepa_cf::{CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_SYNTHETIC_STRESS_RESULTS};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/task-py-g-071-synthetic-corpus-fsv";

fn main() -> Result<()> {
    let run_dir = PathBuf::from(FSV_ROOT).join(format!(
        "run-{}",
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ")
    ));
    fs::create_dir_all(&run_dir).context("create fsv run dir")?;
    let corpus_root = PathBuf::from(SYNTHETIC_STRESS_ROOT);
    let results_db_path = PathBuf::from(SYNTHETIC_STRESS_RESULTS_DB);
    if corpus_root.exists() {
        fs::remove_dir_all(&corpus_root).context("reset synthetic stress corpus")?;
    }
    if results_db_path.exists() {
        fs::remove_dir_all(&results_db_path).context("reset synthetic stress results db")?;
    }
    let before = corpus_state("before_happy", &corpus_root, &results_db_path)?;

    let happy = run_synthetic_stress_eval(SyntheticStressEvalRequest {
        corpus_root: corpus_root.clone(),
        results_db_path: results_db_path.clone(),
        project_id_prefix: format!(
            "task-py-g-071-fsv-{}",
            chrono::Utc::now().timestamp_millis()
        ),
        materialize_corpus: true,
        overwrite_corpus: true,
    })?;
    let after = corpus_state("after_happy", &corpus_root, &results_db_path)?;
    let result_readback = read_all_results(&results_db_path, &happy.results)?;

    let malformed = malformed_expectation_edge(&run_dir)?;
    let empty = empty_corpus_edge(&run_dir)?;
    let replay = replay_edge(&corpus_root, &results_db_path)?;
    let all_passed = happy.synthetic_stress_passed
        && happy.cases_total == SYNTHETIC_STRESS_CASES
        && happy.cases_passed == SYNTHETIC_STRESS_CASES
        && result_readback["allRowsReadBack"] == true
        && malformed["pass"] == true
        && empty["pass"] == true
        && replay["pass"] == true;

    let report = json!({
        "schemaVersion": 1,
        "taskId": "TASK-PY-G-071",
        "issue": 290,
        "allPassed": all_passed,
        "sourceOfTruth": {
            "corpusRoot": corpus_root,
            "resultsDbPath": results_db_path,
            "resultCf": CF_MEJEPA_SYNTHETIC_STRESS_RESULTS,
            "livePredictionCf": CF_MEJEPA_LIVE_PREDICTIONS
        },
        "threshold": SYNTHETIC_STRESS_THRESHOLD,
        "before": before,
        "happyPath": {
            "casesTotal": happy.cases_total,
            "casesPassed": happy.cases_passed,
            "syntheticStressCorrectness": happy.synthetic_stress_correctness,
            "syntheticStressPassed": happy.synthetic_stress_passed,
            "shipGateCountable": happy.ship_gate_countable,
            "gatePredicate": happy.gate_predicate,
            "source": happy.source,
            "caseCountsByKind": happy.case_counts_by_kind,
            "resultRowsBefore": happy.result_rows_before,
            "resultRowsAfter": happy.result_rows_after,
            "livePredictionRows": happy.results.iter().map(|row| json!({
                "caseId": row.case_id,
                "matched": row.matched,
                "livePredictionRowsBefore": row.live_prediction_rows_before,
                "livePredictionRowsAfter": row.live_prediction_rows_after,
                "predictionDbPath": row.predictions_db_path,
                "predictionIdHex": row.actual.prediction_id_hex,
                "livePredictionKeyHex": row.actual.live_prediction_key_hex,
                "actualTopFailureMode": row.actual.top_failure_mode,
                "mismatchReasons": row.mismatch_reasons,
            })).collect::<Vec<_>>()
        },
        "after": after,
        "resultReadback": result_readback,
        "edgeCases": [malformed, empty, replay],
    });
    let report_path = run_dir.join("synthetic_stress_fsv.json");
    write_json_0600(&report_path, &report).context("write fsv report")?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    if all_passed {
        Ok(())
    } else {
        anyhow::bail!("TASK-PY-G-071 FSV failed")
    }
}

fn corpus_state(label: &str, root: &Path, results_db_path: &Path) -> Result<Value> {
    let case_count = read_synthetic_stress_corpus(root)
        .map(|cases| cases.len())
        .unwrap_or(0);
    let result_count = count_results(results_db_path)?;
    Ok(json!({
        "label": label,
        "corpusRootExists": root.exists(),
        "caseDirectoryCount": count_case_dirs(root)?,
        "readableCaseCount": case_count,
        "resultRows": result_count,
    }))
}

fn malformed_expectation_edge(run_dir: &Path) -> Result<Value> {
    let root = run_dir.join("malformed-expectation-corpus");
    let db_path = run_dir.join("malformed-results.rocksdb");
    materialize_synthetic_stress_corpus(&root, true)?;
    let expected_path = root.join("broken_obvious_00/expected_verdict.json");
    let before = json!({
        "caseDirectoryCount": count_case_dirs(&root)?,
        "resultRows": count_results(&db_path)?,
        "expectedPath": expected_path,
    });
    fs::write(&expected_path, b"{ not valid json").context("write corrupt expected")?;
    let err = run_synthetic_stress_eval(SyntheticStressEvalRequest {
        corpus_root: root.clone(),
        results_db_path: db_path.clone(),
        project_id_prefix: "task-py-g-071-malformed".to_string(),
        materialize_corpus: false,
        overwrite_corpus: false,
    })
    .expect_err("malformed expectation should fail closed");
    let after = json!({
        "caseDirectoryCount": count_case_dirs(&root)?,
        "resultRows": count_results(&db_path)?,
        "errorCode": err.code(),
    });
    Ok(json!({
        "case": "malformed_expected_verdict_fails_closed",
        "pass": err.code() == "SYNTHETIC_STRESS_MALFORMED_EXPECTATION",
        "before": before,
        "after": after,
    }))
}

fn empty_corpus_edge(run_dir: &Path) -> Result<Value> {
    let root = run_dir.join("empty-corpus");
    let db_path = run_dir.join("empty-results.rocksdb");
    fs::create_dir_all(&root).context("create empty corpus")?;
    let before = json!({
        "caseDirectoryCount": count_case_dirs(&root)?,
        "resultRows": count_results(&db_path)?,
    });
    let err = run_synthetic_stress_eval(SyntheticStressEvalRequest {
        corpus_root: root.clone(),
        results_db_path: db_path.clone(),
        project_id_prefix: "task-py-g-071-empty".to_string(),
        materialize_corpus: false,
        overwrite_corpus: false,
    })
    .expect_err("empty corpus should fail closed");
    let after = json!({
        "caseDirectoryCount": count_case_dirs(&root)?,
        "resultRows": count_results(&db_path)?,
        "errorCode": err.code(),
    });
    Ok(json!({
        "case": "empty_corpus_fails_closed",
        "pass": err.code() == "SYNTHETIC_STRESS_EMPTY_CORPUS",
        "before": before,
        "after": after,
    }))
}

fn replay_edge(corpus_root: &Path, results_db_path: &Path) -> Result<Value> {
    let before = json!({
        "caseDirectoryCount": count_case_dirs(corpus_root)?,
        "resultRows": count_results(results_db_path)?,
    });
    let replay = run_synthetic_stress_eval(SyntheticStressEvalRequest {
        corpus_root: corpus_root.to_path_buf(),
        results_db_path: results_db_path.to_path_buf(),
        project_id_prefix: "task-py-g-071-replay".to_string(),
        materialize_corpus: false,
        overwrite_corpus: false,
    })?;
    let after = json!({
        "caseDirectoryCount": count_case_dirs(corpus_root)?,
        "resultRows": count_results(results_db_path)?,
        "casesPassed": replay.cases_passed,
        "correctness": replay.synthetic_stress_correctness,
    });
    Ok(json!({
        "case": "duplicate_replay_overwrites_result_rows_without_growth",
        "pass": before["resultRows"] == 30 && after["resultRows"] == 30 && replay.synthetic_stress_passed,
        "before": before,
        "after": after,
    }))
}

fn read_all_results(
    results_db_path: &Path,
    results: &[context_graph_mejepa::SyntheticStressResult],
) -> Result<Value> {
    let db = open_infer_rocksdb(results_db_path)?;
    let mut rows = Vec::new();
    for result in results {
        let readback = read_synthetic_stress_result(db.as_ref(), &result.case_id)?
            .context("missing result readback")?;
        rows.push(json!({
            "caseId": result.case_id,
            "matched": readback.matched,
            "resultKey": readback.result_key,
            "predictionIdHex": readback.actual.prediction_id_hex,
            "livePredictionKeyHex": readback.actual.live_prediction_key_hex,
        }));
    }
    Ok(json!({
        "allRowsReadBack": rows.len() == results.len(),
        "rowsReadBack": rows.len(),
        "sampleRows": rows.into_iter().take(5).collect::<Vec<_>>(),
    }))
}

fn count_results(path: &Path) -> Result<usize> {
    let db = open_infer_rocksdb(path)?;
    Ok(count_synthetic_stress_results(db.as_ref())?)
}

fn count_case_dirs(path: &Path) -> Result<usize> {
    if !path.exists() {
        return Ok(0);
    }
    let mut count = 0usize;
    for entry in fs::read_dir(path).context("read case dir")? {
        let entry = entry.context("case dir entry")?;
        if entry.file_type().context("case dir file type")?.is_dir() {
            count += 1;
        }
    }
    Ok(count)
}

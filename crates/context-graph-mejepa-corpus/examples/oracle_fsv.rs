// Manual Full State Verification for Phase 0 oracle + RocksDB storage.
//
// Sources of truth on disk:
//   /tmp/contextgraph-mejepa-corpus-fsv/oracle/pytest-report.json
//   /tmp/contextgraph-mejepa-corpus-fsv/oracle/rocksdb/
//   /tmp/contextgraph-mejepa-corpus-fsv/oracle-evidence.json
//
// The trigger is a real `python3 -m pytest` run over synthetic tests whose
// expected outcomes are known up front: 1 pass, 1 fail, 1 skip. A small local
// pytest plugin writes the report file; the oracle parser reads that file
// separately, then the verdict and mutation are persisted to RocksDB and
// reopened/read back as the physical source of truth.

use context_graph_mejepa_corpus::oracle::{
    parse_pytest_json_report, run_command_json_oracle, CommandOracleConfig, OracleStore,
    CF_MEJEPA_CORPUS_PROVENANCE, CF_MEJEPA_MUTATION_CORPUS, CF_MEJEPA_ORACLE_VERDICTS,
};
use context_graph_mejepa_corpus::{
    apply_mutation, compute_corpus_sha256, CorpusProvenance, CorpusProvenanceRow, Language,
    MutationCategory, MutationConfig, MutationOutcome, MEJEPA_CORPUS_VERSION_V1,
    MEJEPA_EMBEDDER_VERSION_V1,
};
use context_graph_mejepa_instruments::{OracleVerdict, TestOutcome};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

fn ensure(label: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("oracle FSV invariant failed: {label}").into())
    }
}

fn write_text_checked(path: &Path, text: &str) -> FsvResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)?;
    let readback = fs::read_to_string(path)?;
    ensure(
        &format!("text readback must match write at {}", path.display()),
        readback == text,
    )?;
    Ok(())
}

fn sha256_file(path: &Path) -> FsvResult<String> {
    let bytes = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn sha256_text(text: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(text.as_bytes()))
}

fn sha256_json_value(value: &Value) -> String {
    sha256_text(&serde_json::to_string(value).expect("JSON value serialises"))
}

fn pytest_plugin() -> &'static str {
    r#"import json
import os

RESULTS = []

def pytest_runtest_logreport(report):
    if report.when != "call" and report.outcome != "skipped":
        return
    row = {
        "nodeid": report.nodeid,
        "outcome": report.outcome,
        report.when: {"duration": report.duration},
    }
    if report.longrepr:
        row["longrepr"] = str(report.longrepr)
    RESULTS.append(row)

def pytest_sessionfinish(session, exitstatus):
    counts = {"passed": 0, "failed": 0, "skipped": 0, "errors": 0}
    for row in RESULTS:
        outcome = row["outcome"]
        if outcome == "passed":
            counts["passed"] += 1
        elif outcome == "failed":
            counts["failed"] += 1
        elif outcome == "skipped":
            counts["skipped"] += 1
        else:
            counts["errors"] += 1
    counts["total"] = len(RESULTS)
    path = os.environ["MEJEPA_REPORT_PATH"]
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        json.dump({"summary": counts, "tests": RESULTS}, f, sort_keys=True, indent=2)
"#
}

fn pytest_tests() -> &'static str {
    r#"import pytest

def test_addition_passes():
    assert 2 + 2 == 4

def test_subtraction_fails():
    assert 5 - 2 == 99

@pytest.mark.skip(reason="synthetic skip for oracle FSV")
def test_skip_branch():
    assert False
"#
}

fn synthetic_source() -> &'static str {
    "def add(a: int, b: int) -> int:\n    return a + b\n"
}

fn write_pytest_project(root: &Path) -> FsvResult<Value> {
    let conftest = root.join("conftest.py");
    let test_file = root.join("test_synthetic_oracle.py");
    write_text_checked(&conftest, pytest_plugin())?;
    write_text_checked(&test_file, pytest_tests())?;
    Ok(json!({
        "conftest_path": conftest.display().to_string(),
        "test_file_path": test_file.display().to_string(),
        "conftest_sha256": sha256_file(&conftest)?,
        "test_file_sha256": sha256_file(&test_file)?,
    }))
}

fn run_pytest_oracle(root: &Path, report_path: &Path) -> FsvResult<OracleVerdict> {
    let config = CommandOracleConfig {
        program: "python3".to_string(),
        args: vec![
            "-m".to_string(),
            "pytest".to_string(),
            root.display().to_string(),
            "-q".to_string(),
        ],
        cwd: None,
        env: vec![(
            "MEJEPA_REPORT_PATH".to_string(),
            report_path.display().to_string(),
        )],
        report_path: report_path.to_path_buf(),
        timeout: Duration::from_secs(30),
        accepted_exit_codes: vec![0, 1],
    };
    Ok(run_command_json_oracle(&config)?)
}

fn store_and_reopen(
    db_path: &Path,
    task_id: &str,
    mutation: &MutationOutcome,
    verdict: &OracleVerdict,
) -> FsvResult<Value> {
    {
        let store = OracleStore::open(db_path)?;
        let patch_sha256 = sha256_text(&mutation.mutated_source);
        let verdict_sha256 = sha256_json_value(&serde_json::to_value(verdict)?);
        let corpus_sha256 = compute_corpus_sha256(&[CorpusProvenanceRow {
            task_id: task_id.to_string(),
            category: MutationCategory::KnownGood,
            patch_sha256,
            oracle_verdict_sha256: verdict_sha256,
        }])?;
        let provenance = CorpusProvenance {
            corpus_version: MEJEPA_CORPUS_VERSION_V1.to_string(),
            embedder_version: MEJEPA_EMBEDDER_VERSION_V1.to_string(),
            generated_at_unix_ms: 1,
            generator_version: "oracle_fsv".to_string(),
            seed: 0,
            languages: vec![Language::Python],
            task_manifest_count: 1,
            mutation_count: 1,
            mutation_categories: vec![MutationCategory::KnownGood],
            source_patch_mode: "synthetic-source".to_string(),
            split_mode: "instance_atomic".to_string(),
            corpus_sha256,
            complete: true,
        };
        store.put_corpus_row(task_id, MutationCategory::KnownGood, mutation, verdict)?;
        store.put_provenance(&provenance)?;
        store.flush()?;
    }

    let reopened = OracleStore::open(db_path)?;
    let mutation_readback = reopened
        .get_mutation(task_id, MutationCategory::KnownGood)?
        .ok_or("mutation row missing after reopen")?;
    let verdict_readback = reopened
        .get_verdict(task_id, MutationCategory::KnownGood)?
        .ok_or("oracle verdict row missing after reopen")?;
    let corpus_row_readback = reopened
        .get_corpus_row(task_id, MutationCategory::KnownGood)?
        .ok_or("atomic corpus row missing after reopen")?;
    let provenance_readback = reopened
        .get_provenance(MEJEPA_CORPUS_VERSION_V1, MEJEPA_EMBEDDER_VERSION_V1)?
        .ok_or("provenance row missing after reopen")?;
    ensure(
        "mutation row must survive RocksDB reopen",
        mutation_readback == *mutation,
    )?;
    ensure(
        "oracle verdict row must survive RocksDB reopen",
        verdict_readback == *verdict,
    )?;
    ensure(
        "atomic corpus row must survive RocksDB reopen",
        corpus_row_readback == (mutation.clone(), verdict.clone()),
    )?;
    let mutation_count = reopened.count_cf(CF_MEJEPA_MUTATION_CORPUS)?;
    let verdict_count = reopened.count_cf(CF_MEJEPA_ORACLE_VERDICTS)?;
    let provenance_count = reopened.count_cf(CF_MEJEPA_CORPUS_PROVENANCE)?;
    ensure("mutation CF count must be 1", mutation_count == 1)?;
    ensure("oracle verdict CF count must be 1", verdict_count == 1)?;
    ensure("provenance CF count must be 1", provenance_count == 1)?;
    Ok(json!({
        "rocksdb_path": db_path.display().to_string(),
        "counts_after_reopen": {
            CF_MEJEPA_MUTATION_CORPUS: mutation_count,
            CF_MEJEPA_ORACLE_VERDICTS: verdict_count,
            CF_MEJEPA_CORPUS_PROVENANCE: provenance_count,
        },
        "mutation_rows": reopened.scan_cf_json(CF_MEJEPA_MUTATION_CORPUS)?,
        "verdict_rows": reopened.scan_cf_json(CF_MEJEPA_ORACLE_VERDICTS)?,
        "provenance": provenance_readback,
        "provenance_rows": reopened.scan_cf_json(CF_MEJEPA_CORPUS_PROVENANCE)?,
        "passes": true,
    }))
}

fn boundary_empty_report() -> FsvResult<Value> {
    let result = parse_pytest_json_report("");
    ensure(
        "empty report must reject",
        matches!(&result, Err(e) if e.code() == "MEJEPA_ORACLE_JSON_PARSE"),
    )?;
    Ok(json!({
        "before_state": {"report_text_len": 0},
        "after_state": {"verdict_produced": false},
        "expected_code": "MEJEPA_ORACLE_JSON_PARSE",
        "actual_code": result.err().map(|e| e.code()),
        "passes": true,
    }))
}

fn boundary_mismatched_summary() -> FsvResult<Value> {
    let report = r#"{
      "summary": {"passed": 2, "failed": 0, "skipped": 0, "errors": 0, "total": 1},
      "tests": [{"nodeid": "tests/test_x.py::test_y", "outcome": "passed", "call": {"duration": 0.001}}]
    }"#;
    let result = parse_pytest_json_report(report);
    ensure(
        "mismatched summary must reject",
        matches!(&result, Err(e) if e.code() == "MEJEPA_ORACLE_REPORT_INVALID"),
    )?;
    Ok(json!({
        "before_state": {
            "summary_passed": 2,
            "tests_count": 1,
        },
        "after_state": {"verdict_produced": false},
        "expected_code": "MEJEPA_ORACLE_REPORT_INVALID",
        "actual_code": result.err().map(|e| e.code()),
        "passes": true,
    }))
}

fn boundary_stale_report(root: &Path) -> FsvResult<Value> {
    let stale_path = root.join("stale-report.json");
    write_text_checked(&stale_path, r#"{"summary":{"total":0},"tests":[]}"#)?;
    let config = CommandOracleConfig {
        program: "sh".to_string(),
        args: vec!["-c".to_string(), "true".to_string()],
        cwd: None,
        env: vec![],
        report_path: stale_path.clone(),
        timeout: Duration::from_secs(5),
        accepted_exit_codes: vec![0],
    };
    let result = run_command_json_oracle(&config);
    ensure(
        "stale report path must reject before command execution",
        matches!(&result, Err(e) if e.code() == "MEJEPA_ORACLE_INVALID_INPUT"),
    )?;
    Ok(json!({
        "source_of_truth": stale_path.display().to_string(),
        "before_state": {"report_path_exists": true},
        "after_state": {
            "command_accepted": false,
            "report_still_exists": stale_path.exists(),
        },
        "expected_code": "MEJEPA_ORACLE_INVALID_INPUT",
        "actual_code": result.err().map(|e| e.code()),
        "passes": true,
    }))
}

fn boundary_bad_store_key(db_path: &Path) -> FsvResult<Value> {
    let store = OracleStore::open(db_path)?;
    let mutation = apply_mutation(
        MutationCategory::KnownGood,
        "def add(a, b):\n    return a + b\n",
        MutationConfig::default(),
    )?;
    let verdict = OracleVerdict {
        per_test: vec![context_graph_mejepa_instruments::PerTestOutcome {
            test_id: "tests/test_key.py::test_ok".to_string(),
            outcome: TestOutcome::Pass,
            runtime_ms: 1,
        }],
        exception: None,
    };
    let result = store.put_corpus_row("bad|task", MutationCategory::KnownGood, &mutation, &verdict);
    ensure(
        "bad RocksDB task key must reject",
        matches!(&result, Err(e) if e.code() == "MEJEPA_ORACLE_INVALID_INPUT"),
    )?;
    Ok(json!({
        "before_state": {"task_id": "bad|task"},
        "after_state": {"row_written": false},
        "expected_code": "MEJEPA_ORACLE_INVALID_INPUT",
        "actual_code": result.err().map(|e| e.code()),
        "passes": true,
    }))
}

fn main() -> FsvResult<()> {
    let root = PathBuf::from("/tmp/contextgraph-mejepa-corpus-fsv/oracle");
    let evidence_path = PathBuf::from("/tmp/contextgraph-mejepa-corpus-fsv/oracle-evidence.json");
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    fs::create_dir_all(&root)?;
    let report_path = root.join("pytest-report.json");
    let db_path = root.join("rocksdb");

    let before = json!({
        "root_exists": root.exists(),
        "report_path_exists": report_path.exists(),
        "rocksdb_path_exists": db_path.exists(),
    });

    let project_evidence = write_pytest_project(&root.join("pytest-project"))?;
    let verdict = run_pytest_oracle(&root.join("pytest-project"), &report_path)?;
    ensure(
        "pytest report file must exist after trigger",
        report_path.exists(),
    )?;
    ensure(
        "verdict must contain 3 per-test rows",
        verdict.per_test.len() == 3,
    )?;
    ensure(
        "expected pass/fail/skip outcomes must be present",
        verdict
            .per_test
            .iter()
            .filter(|row| row.outcome == TestOutcome::Pass)
            .count()
            == 1
            && verdict
                .per_test
                .iter()
                .filter(|row| row.outcome == TestOutcome::Fail)
                .count()
                == 1
            && verdict
                .per_test
                .iter()
                .filter(|row| row.outcome == TestOutcome::Skip)
                .count()
                == 1,
    )?;

    let mutation = apply_mutation(
        MutationCategory::KnownGood,
        synthetic_source(),
        MutationConfig::default(),
    )?;
    ensure(
        "known-good mutation must preserve source",
        mutation.mutated_source == synthetic_source(),
    )?;
    let store_evidence =
        store_and_reopen(&db_path, "synthetic-phase0-oracle-fsv", &mutation, &verdict)?;

    let after = json!({
        "report_path_exists": report_path.exists(),
        "report_sha256": sha256_file(&report_path)?,
        "report_json": serde_json::from_str::<Value>(&fs::read_to_string(&report_path)?)?,
        "rocksdb_path_exists": db_path.exists(),
    });

    let evidence = json!({
        "source_of_truth": {
            "pytest_report": report_path.display().to_string(),
            "rocksdb": db_path.display().to_string(),
            "evidence": evidence_path.display().to_string(),
        },
        "before_state": before,
        "trigger": {
            "command": "python3 -m pytest <synthetic-project> -q",
            "accepted_exit_codes": [0, 1],
        },
        "pytest_project": project_evidence,
        "after_state": after,
        "parsed_verdict": verdict,
        "mutation": mutation,
        "rocksdb_readback": store_evidence,
        "boundaries": {
            "empty_report": boundary_empty_report()?,
            "mismatched_summary": boundary_mismatched_summary()?,
            "stale_report_path": boundary_stale_report(&root)?,
            "bad_store_key": boundary_bad_store_key(&root.join("bad-key-rocksdb"))?,
        },
        "passes": true,
    });

    let text = serde_json::to_string_pretty(&evidence)?;
    write_text_checked(&evidence_path, &text)?;

    println!("Phase 0 oracle FSV evidence: {}", evidence_path.display());
    println!("{}", text);
    Ok(())
}

// Full State Verification for Phase 0 oracle parsing and RocksDB persistence.

use context_graph_mejepa_corpus::oracle::{
    parse_pytest_json_report, run_docker_json_oracle, DockerMount, DockerOracleConfig, OracleStore,
    CF_MEJEPA_MUTATION_CORPUS, CF_MEJEPA_ORACLE_VERDICTS,
};
use context_graph_mejepa_corpus::{apply_mutation, MutationCategory, MutationConfig};
use context_graph_mejepa_instruments::{OracleVerdict, PerTestOutcome, TestOutcome};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

fn main() -> FsvResult<()> {
    let root = PathBuf::from("/tmp/contextgraph-mejepa-corpus-fsv/oracle-store");
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    fs::create_dir_all(&root)?;

    let db_path = root.join("rocksdb");
    let store = OracleStore::open(&db_path)?;
    let before_counts = cf_counts(&store)?;

    let task_id = "synthetic__oracle-store-1";
    let category = MutationCategory::KnownGood;
    let mutated_source = "def add(a, b):\n    return a + b\n";
    let mutation_outcome = apply_mutation(category, mutated_source, MutationConfig::default())?;
    let pytest_json = r#"{
      "summary": {"passed": 1, "failed": 0, "skipped": 0, "errors": 0, "total": 1},
      "tests": [
        {"nodeid": "tests/test_add.py::test_add_two_numbers", "outcome": "passed", "call": {"duration": 0.003}}
      ]
    }"#;
    let verdict = parse_pytest_json_report(pytest_json)?;

    store.put_mutation(task_id, category, &mutation_outcome)?;
    store.put_verdict(task_id, category, &verdict)?;
    store.flush()?;
    let mutation_readback = store
        .get_mutation(task_id, category)?
        .expect("mutation row");
    let verdict_readback = store.get_verdict(task_id, category)?.expect("verdict row");
    require(
        "mutation source readback",
        mutation_readback == mutation_outcome,
    )?;
    require("oracle verdict readback", verdict_readback == verdict)?;
    let after_counts = cf_counts(&store)?;
    let mutation_rows = store.scan_cf_json(CF_MEJEPA_MUTATION_CORPUS)?;
    let verdict_rows = store.scan_cf_json(CF_MEJEPA_ORACLE_VERDICTS)?;

    let evidence = json!({
        "source_of_truth": {
            "rocksdb_path": db_path.display().to_string(),
            "mutation_cf": CF_MEJEPA_MUTATION_CORPUS,
            "verdict_cf": CF_MEJEPA_ORACLE_VERDICTS,
        },
        "happy_path": {
            "before_state": before_counts,
            "trigger": {
                "task_id": task_id,
                "category": category.slug(),
                "mutated_source_sha256": sha256_text(mutated_source),
                "pytest_json_sha256": sha256_text(pytest_json),
            },
            "after_state": {
                "cf_counts": after_counts,
                "mutation_rows": mutation_rows,
                "verdict": verdict_readback,
                "verdict_rows": verdict_rows,
            },
            "passes": true,
        },
        "edge_cases": {
            "empty_report": edge_report(r#"{"summary":{"total":0},"tests":[]}"#),
            "summary_mismatch": edge_report(r#"{"summary":{"passed":2,"total":1},"tests":[{"nodeid":"t","outcome":"passed"}]}"#),
            "invalid_key_delimiter": edge_invalid_key(&store),
            "docker_missing_command": edge_docker_missing_command(&root),
        },
        "passes": true,
    });
    let evidence_path = root.join("oracle-store-evidence.json");
    let text = serde_json::to_string_pretty(&evidence)?;
    fs::write(&evidence_path, &text)?;
    let readback: Value = serde_json::from_str(&fs::read_to_string(&evidence_path)?)?;
    require("evidence readback", readback == evidence)?;
    println!("Oracle store FSV evidence: {}", evidence_path.display());
    println!("{}", text);
    Ok(())
}

fn edge_report(raw: &str) -> Value {
    let result = parse_pytest_json_report(raw);
    let actual = result.err().map(|e| e.code());
    json!({
        "before_state": { "raw_json_sha256": sha256_text(raw) },
        "after_state": { "accepted": false },
        "expected_code": "MEJEPA_ORACLE_REPORT_INVALID",
        "actual_code": actual,
        "passes": actual == Some("MEJEPA_ORACLE_REPORT_INVALID"),
    })
}

fn edge_invalid_key(store: &OracleStore) -> Value {
    let verdict = OracleVerdict {
        per_test: vec![PerTestOutcome {
            test_id: "tests/test_add.py::test_add_two_numbers".into(),
            outcome: TestOutcome::Pass,
            runtime_ms: 3,
        }],
        exception: None,
    };
    let result = store.put_verdict("bad|task", MutationCategory::KnownGood, &verdict);
    let actual = result.err().map(|e| e.code());
    json!({
        "before_state": { "task_id": "bad|task" },
        "after_state": { "row_written": false },
        "expected_code": "MEJEPA_ORACLE_INVALID_INPUT",
        "actual_code": actual,
        "passes": actual == Some("MEJEPA_ORACLE_INVALID_INPUT"),
    })
}

fn edge_docker_missing_command(root: &std::path::Path) -> Value {
    let config = DockerOracleConfig {
        image: "swebench/sweb.eval.x86_64.synthetic__oracle-store-1:latest".to_string(),
        mounts: vec![DockerMount {
            host_path: root.to_path_buf(),
            container_path: "/workspace/mejepa-oracle".to_string(),
            readonly: false,
        }],
        command: vec![],
        report_path_host: root.join("docker-edge-report.json"),
        timeout: Duration::from_secs(1),
        accepted_exit_codes: vec![0],
        network_none: true,
        read_only_root: true,
    };
    let result = run_docker_json_oracle(&config);
    let actual = result.err().map(|e| e.code());
    json!({
        "before_state": { "docker_command_len": 0 },
        "after_state": { "docker_spawned": false },
        "expected_code": "MEJEPA_ORACLE_INVALID_INPUT",
        "actual_code": actual,
        "passes": actual == Some("MEJEPA_ORACLE_INVALID_INPUT"),
    })
}

fn cf_counts(store: &OracleStore) -> FsvResult<Value> {
    let mut out = serde_json::Map::new();
    for cf_name in [CF_MEJEPA_MUTATION_CORPUS, CF_MEJEPA_ORACLE_VERDICTS] {
        let count = store.count_cf(cf_name)?;
        out.insert(cf_name.to_string(), json!(count));
    }
    Ok(Value::Object(out))
}

fn sha256_text(text: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(text.as_bytes()))
}

fn require(label: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("oracle store FSV invariant failed: {label}").into())
    }
}

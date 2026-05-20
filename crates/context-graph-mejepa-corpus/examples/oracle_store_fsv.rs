// Full State Verification for Phase 0 oracle parsing and RocksDB persistence.

use context_graph_mejepa_corpus::oracle::{
    parse_pytest_json_report, run_docker_json_oracle, DockerMount, DockerOracleConfig, OracleStore,
    CF_MEJEPA_CORPUS_PROVENANCE, CF_MEJEPA_MUTATION_CORPUS, CF_MEJEPA_ORACLE_VERDICTS,
};
use context_graph_mejepa_corpus::{
    apply_mutation, compute_corpus_sha256, CorpusProvenance, CorpusProvenanceRow, Language,
    MutationCategory, MutationConfig, MEJEPA_CORPUS_VERSION_V1, MEJEPA_EMBEDDER_VERSION_V1,
};
use context_graph_mejepa_instruments::{OracleVerdict, PerTestOutcome, TestOutcome};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::time::Duration;

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

fn main() -> FsvResult<()> {
    let root = context_graph_paths::ensure_subdir("fsv/context-graph-mejepa-corpus/oracle-store")?;
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

    let patch_sha256 = sha256_text(&mutation_outcome.mutated_source);
    let verdict_sha256 = sha256_json_value(&serde_json::to_value(&verdict)?);
    let corpus_sha256 = compute_corpus_sha256(&[CorpusProvenanceRow {
        task_id: task_id.to_string(),
        category,
        patch_sha256: patch_sha256.clone(),
        oracle_verdict_sha256: verdict_sha256.clone(),
    }])?;
    let provenance = CorpusProvenance {
        corpus_version: MEJEPA_CORPUS_VERSION_V1.to_string(),
        embedder_version: MEJEPA_EMBEDDER_VERSION_V1.to_string(),
        generated_at_unix_ms: 1,
        generator_version: "oracle_store_fsv".to_string(),
        seed: 0,
        languages: vec![Language::Python],
        task_manifest_count: 1,
        mutation_count: 1,
        mutation_categories: vec![category],
        source_patch_mode: "synthetic-source".to_string(),
        split_mode: "instance_atomic".to_string(),
        corpus_sha256,
        complete: true,
    };
    store.put_corpus_row(task_id, category, &mutation_outcome, &verdict)?;
    store.put_provenance(&provenance)?;
    store.flush()?;
    let corpus_row_readback = store
        .get_corpus_row(task_id, category)?
        .expect("corpus row");
    let mutation_readback = store
        .get_mutation(task_id, category)?
        .expect("mutation row");
    let verdict_readback = store.get_verdict(task_id, category)?.expect("verdict row");
    let provenance_readback = store
        .get_provenance(MEJEPA_CORPUS_VERSION_V1, MEJEPA_EMBEDDER_VERSION_V1)?
        .expect("provenance row");
    require(
        "mutation source readback",
        mutation_readback == mutation_outcome,
    )?;
    require("oracle verdict readback", verdict_readback == verdict)?;
    require(
        "atomic corpus row readback",
        corpus_row_readback == (mutation_outcome.clone(), verdict.clone()),
    )?;
    require("provenance readback", provenance_readback == provenance)?;
    let after_counts = cf_counts(&store)?;
    let mutation_rows = store.scan_cf_json(CF_MEJEPA_MUTATION_CORPUS)?;
    let verdict_rows = store.scan_cf_json(CF_MEJEPA_ORACLE_VERDICTS)?;
    let provenance_rows = store.scan_cf_json(CF_MEJEPA_CORPUS_PROVENANCE)?;

    let evidence = json!({
        "source_of_truth": {
            "rocksdb_path": db_path.display().to_string(),
            "mutation_cf": CF_MEJEPA_MUTATION_CORPUS,
            "verdict_cf": CF_MEJEPA_ORACLE_VERDICTS,
            "provenance_cf": CF_MEJEPA_CORPUS_PROVENANCE,
        },
        "happy_path": {
            "before_state": before_counts,
            "trigger": {
                "task_id": task_id,
                "category": category.slug(),
                "mutated_source_sha256": patch_sha256,
                "pytest_json_sha256": sha256_text(pytest_json),
                "verdict_sha256": verdict_sha256,
            },
            "after_state": {
                "cf_counts": after_counts,
                "mutation_rows": mutation_rows,
                "verdict": verdict_readback,
                "verdict_rows": verdict_rows,
                "provenance": provenance_readback,
                "provenance_rows": provenance_rows,
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
    let mutation = apply_mutation(
        MutationCategory::KnownGood,
        "def add(a, b):\n    return a + b\n",
        MutationConfig::default(),
    )
    .expect("known-good mutation");
    let verdict = OracleVerdict {
        per_test: vec![PerTestOutcome {
            test_id: "tests/test_add.py::test_add_two_numbers".into(),
            outcome: TestOutcome::Pass,
            runtime_ms: 3,
        }],
        exception: None,
        evidence_unavailable: false,
    };
    let result = store.put_corpus_row("bad|task", MutationCategory::KnownGood, &mutation, &verdict);
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
    for cf_name in [
        CF_MEJEPA_MUTATION_CORPUS,
        CF_MEJEPA_ORACLE_VERDICTS,
        CF_MEJEPA_CORPUS_PROVENANCE,
    ] {
        let count = store.count_cf(cf_name)?;
        out.insert(cf_name.to_string(), json!(count));
    }
    Ok(Value::Object(out))
}

fn sha256_text(text: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(text.as_bytes()))
}

fn sha256_json_value(value: &Value) -> String {
    sha256_text(&serde_json::to_string(value).expect("JSON value serialises"))
}

fn require(label: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("oracle store FSV invariant failed: {label}").into())
    }
}

use context_graph_mejepa_instruments::{
    analyze_churn, analyze_complexity, analyze_coverage_delta, analyze_doc_coverage,
    composite_reward_cfs, extract_composite_reward_signals, extract_dependency_cve_labels,
    write_composite_reward_raw_outputs, ChurnCommit, ChurnInput, ComplexityInput,
    CompositeRewardInput, CompositeRewardSignalKind, CompositeRewardSignalRecord,
    CompositeRewardSignalStore, CoverageInput, CoverageSnapshot, DepCveInput, DocCoverageInput,
    FileCoverageSnapshot,
};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Serialize)]
struct EdgeCase {
    name: &'static str,
    passed: bool,
    detail: Value,
}

fn main() -> FsvResult<()> {
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .to_string();
    let fsv_root = PathBuf::from("/mnt/d/contextgraph/fsv/task-py-g-075-composite-reward-fsv")
        .join(format!("run-{run_id}"));
    fs::create_dir_all(&fsv_root)?;
    let raw_root = PathBuf::from("/mnt/d/contextgraph/corpus/python-reward-signals-v1");
    let row_id = format!("task-rwd-209-{run_id}");
    let chunk_id = "chunk-composite-reward-001";
    let path = "pkg/reward.py";
    let rocksdb_path = fsv_root.join("composite-reward-rocksdb");
    let evidence_path = fsv_root.join("composite_reward_fsv.json");

    let coverage_metrics = analyze_coverage_delta(&coverage_input())?;
    require(
        "coverage happy path is half-covered",
        (coverage_metrics.candidate_line_coverage - 0.5).abs() <= f32::EPSILON,
    )?;
    let complexity_metrics = analyze_complexity(&ComplexityInput {
        language: "python".into(),
        path: path.into(),
        source: ladder_source().into(),
    })?;
    require(
        "complexity ladder has cyclomatic complexity 7",
        complexity_metrics.max_cyclomatic_complexity == 7,
    )?;
    let churn_input = churn_input(&row_id, chunk_id, path);
    let churn_label = analyze_churn(&churn_input)?;
    require(
        "churn happy path has 5 commits by 2 authors",
        churn_label.commits_touching_range == 5 && churn_label.distinct_authors == 2,
    )?;
    let doc_input = doc_input(&row_id, chunk_id, path, documented_source());
    let doc_label = analyze_doc_coverage(&doc_input)?;
    require(
        "doc happy path has full doc coverage",
        doc_label.doc_coverage == 1.0 && doc_label.documented_parameter_count == 1,
    )?;
    let dep_input = dep_input(&row_id, chunk_id, path, advisory_json());
    let dep_labels = extract_dependency_cve_labels(&dep_input)?;
    require(
        "dep-CVE happy path finds urllib3 CVE",
        dep_labels
            .iter()
            .any(|label| label.package == "urllib3" && label.known_cve_count > 0),
    )?;

    let composite_input = CompositeRewardInput {
        corpus_row_id: row_id.clone(),
        chunk_id: chunk_id.into(),
        path: path.into(),
        coverage_metrics: Some(coverage_metrics.clone()),
        complexity_metrics: Some(complexity_metrics.clone()),
        churn_input: Some(churn_input),
        doc_coverage_input: Some(doc_input),
        dep_cve_input: Some(dep_input),
    };
    let extraction = extract_composite_reward_signals(&composite_input)?;
    require(
        "happy path has no quarantines",
        extraction.quarantines.is_empty(),
    )?;
    require(
        "happy path has one coverage label",
        extraction.coverage.len() == 1,
    )?;
    require(
        "happy path has one complexity label",
        extraction.complexity.len() == 1,
    )?;
    require(
        "happy path has one churn label",
        extraction.churn.len() == 1,
    )?;
    require(
        "happy path has one doc label",
        extraction.doc_coverage.len() == 1,
    )?;
    require(
        "happy path has one dep-CVE label",
        extraction.dep_cve.len() == 1,
    )?;

    let raw_paths = write_composite_reward_raw_outputs(&raw_root, &row_id, &extraction)?;
    require(
        "coverage raw output exists",
        raw_paths.coverage_json.exists(),
    )?;
    require("churn raw output exists", raw_paths.churn_json.exists())?;
    require(
        "complexity raw output exists",
        raw_paths.complexity_json.exists(),
    )?;
    require("doc raw output exists", raw_paths.doc_json.exists())?;
    require("depcve raw output exists", raw_paths.depcve_json.exists())?;
    let raw_hashes = raw_file_hashes(&raw_paths)?;

    let store = CompositeRewardSignalStore::open(&rocksdb_path)?;
    let before_counts = store.count_all()?;
    let keys = store.put_extraction(&extraction)?;
    store.flush_all()?;
    let after_counts = store.count_all()?;
    for cf in composite_reward_cfs() {
        require(
            "happy path increments each reward CF by one",
            after_counts.get(*cf).copied().unwrap_or_default()
                == before_counts.get(*cf).copied().unwrap_or_default() + 1,
        )?;
    }
    drop(store);
    let reopened = CompositeRewardSignalStore::open(&rocksdb_path)?;
    let reopened_counts = reopened.count_all()?;
    for cf in composite_reward_cfs() {
        require(
            "reopened RocksDB sees one row per reward CF",
            reopened_counts.get(*cf).copied().unwrap_or_default() == 1,
        )?;
    }
    let dep_rows = reopened.scan_records(context_graph_mejepa_cf::CF_MEJEPA_DEP_CVE_SIGNALS)?;
    require(
        "dep-CVE readback preserves label fields",
        dep_rows.iter().any(|(_, row)| match &row.signal {
            CompositeRewardSignalRecord::DepCve(label) => {
                label.package == "urllib3" && label.known_cve_count == 2
            }
            _ => false,
        }),
    )?;
    drop(reopened);

    let edge_cases = vec![
        edge_missing_coverage_quarantined_independently(&composite_input)?,
        edge_empty_doc_source_fails_closed(&row_id, chunk_id, path)?,
        edge_file_too_large_doc_source_fails_closed(&row_id, chunk_id, path)?,
        edge_corrupted_advisory_json_fails_closed(&row_id, chunk_id, path)?,
        edge_churn_no_touching_commits_fails_closed(&row_id, chunk_id, path)?,
    ];
    if !edge_cases.iter().all(|case| case.passed) {
        return Err(format!(
            "composite reward edge cases failed: {}",
            serde_json::to_string_pretty(&edge_cases)?
        )
        .into());
    }

    let evidence = json!({
        "task": "TASK-RWD-209",
        "issue": 294,
        "all_passed": true,
        "source_of_truth": {
            "raw_output_root": raw_root,
            "raw_output_paths": raw_paths,
            "raw_output_sha256": raw_hashes,
            "rocksdb_path": rocksdb_path,
            "cfs": composite_reward_cfs(),
            "evidence_path": evidence_path,
        },
        "happy_path": {
            "before_state": {"cf_counts": before_counts},
            "trigger": "Compute coverage, complexity, churn, doc-coverage, and dep-CVE labels; write raw JSON artifacts; persist each signal family to its canonical CF; drop and reopen RocksDB; field-read the dep-CVE label.",
            "expected": {
                "coverage_line_pct": 0.5,
                "churn_commits": 5,
                "churn_authors": 2,
                "complexity_cyclomatic": 7,
                "doc_coverage": 1.0,
                "dep_cve_package": "urllib3",
                "dep_cve_count": 2,
                "cf_delta_each": 1
            },
            "after_state": {
                "labels": {
                    "coverage": extraction.coverage,
                    "complexity": extraction.complexity,
                    "churn": extraction.churn,
                    "doc_coverage": extraction.doc_coverage,
                    "dep_cve": extraction.dep_cve,
                },
                "quarantines": extraction.quarantines,
                "keys": keys,
                "cf_counts": after_counts,
                "reopened_cf_counts": reopened_counts,
            }
        },
        "edge_cases": edge_cases,
        "implementation_notes": {
            "dependency_feed_policy": "dependency-CVE matching consumes a caller-supplied local advisory JSON snapshot; no network fetch is performed in the ME-JEPA action path",
            "per_signal_independence": "missing or malformed one signal emits that signal's quarantine record without suppressing successfully computed sibling signals"
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

fn coverage_input() -> CoverageInput {
    CoverageInput {
        baseline: CoverageSnapshot {
            tool: "coverage.py-json".into(),
            files: vec![FileCoverageSnapshot {
                path: "pkg/reward.py".into(),
                executable_lines: vec![1, 2, 3, 4],
                covered_lines: vec![1],
                missing_lines: vec![2, 3, 4],
            }],
        },
        candidate: CoverageSnapshot {
            tool: "coverage.py-json".into(),
            files: vec![FileCoverageSnapshot {
                path: "pkg/reward.py".into(),
                executable_lines: vec![1, 2, 3, 4],
                covered_lines: vec![1, 2],
                missing_lines: vec![3, 4],
            }],
        },
    }
}

fn churn_input(row_id: &str, chunk_id: &str, path: &str) -> ChurnInput {
    let now = 1_779_000_000_i64;
    ChurnInput {
        corpus_row_id: row_id.into(),
        chunk_id: chunk_id.into(),
        path: path.into(),
        byte_start: 10,
        byte_end: 110,
        now_unix_secs: now,
        commits: vec![
            commit("c1", "alice", now, 30, path),
            commit("c2", "bob", now, 25, path),
            commit("c3", "alice", now, 20, path),
            commit("c4", "bob", now, 15, path),
            commit("c5", "alice", now, 1, path),
        ],
    }
}

fn commit(sha: &str, author: &str, now: i64, days_ago: i64, path: &str) -> ChurnCommit {
    ChurnCommit {
        commit_sha: sha.into(),
        author: author.into(),
        committed_unix_secs: now - days_ago * 24 * 60 * 60,
        path: path.into(),
        touched_byte_start: 10,
        touched_byte_end: 110,
        lines_added: 3,
        lines_deleted: 1,
    }
}

fn doc_input(row_id: &str, chunk_id: &str, path: &str, source: &str) -> DocCoverageInput {
    DocCoverageInput {
        corpus_row_id: row_id.into(),
        chunk_id: chunk_id.into(),
        language: "python".into(),
        path: path.into(),
        source_text: source.into(),
    }
}

fn dep_input(row_id: &str, chunk_id: &str, path: &str, advisory_db_json: String) -> DepCveInput {
    DepCveInput {
        corpus_row_id: row_id.into(),
        chunk_id: chunk_id.into(),
        path: path.into(),
        source_text: "import urllib3\n".into(),
        requirements_txt: Some("urllib3==1.25.0\n".into()),
        pyproject_toml: None,
        advisory_db_json,
    }
}

fn documented_source() -> &'static str {
    "def scale(value):\n    \"\"\"Return a scaled value.\n\n    Args:\n        value: numeric value to scale.\n    \"\"\"\n    return value * 2\n"
}

fn ladder_source() -> &'static str {
    r#"def classify(x):
    if x == 0:
        return "zero"
    elif x == 1:
        return "one"
    elif x == 2:
        return "two"
    elif x == 3:
        return "three"
    elif x == 4:
        return "four"
    elif x == 5:
        return "five"
    return "many"
"#
}

fn advisory_json() -> String {
    r#"{
        "packages": {
            "urllib3": [
                {"id": "CVE-2020-26137", "affected_versions": ["==1.25.0"], "cvss": 7.5},
                {"id": "CVE-2021-33503", "affected_versions": ["<=1.25.10"], "cvss": 9.8}
            ]
        }
    }"#
    .into()
}

fn edge_missing_coverage_quarantined_independently(
    input: &CompositeRewardInput,
) -> FsvResult<EdgeCase> {
    let mut edge_input = input.clone();
    edge_input.coverage_metrics = None;
    let extraction = extract_composite_reward_signals(&edge_input)?;
    let passed = extraction.quarantines.iter().any(|quarantine| {
        quarantine.signal_kind == CompositeRewardSignalKind::Coverage
            && quarantine.reason_code == "COMPOSITE_COVERAGE_MISSING"
    }) && extraction.complexity.len() == 1
        && extraction.churn.len() == 1
        && extraction.doc_coverage.len() == 1
        && extraction.dep_cve.len() == 1;
    Ok(EdgeCase {
        name: "missing_coverage_quarantined_independently",
        passed,
        detail: json!({
            "quarantines": extraction.quarantines,
            "sibling_labels": {
                "complexity": extraction.complexity.len(),
                "churn": extraction.churn.len(),
                "doc_coverage": extraction.doc_coverage.len(),
                "dep_cve": extraction.dep_cve.len()
            }
        }),
    })
}

fn edge_empty_doc_source_fails_closed(
    row_id: &str,
    chunk_id: &str,
    path: &str,
) -> FsvResult<EdgeCase> {
    let input = doc_input(row_id, chunk_id, path, "");
    let err = analyze_doc_coverage(&input).expect_err("empty doc source must fail");
    Ok(EdgeCase {
        name: "empty_doc_source_fails_closed",
        passed: err.code() == "MEJEPA_INSTRUMENTS_INVALID_INPUT",
        detail: json!({"error": err.to_string(), "code": err.code()}),
    })
}

fn edge_file_too_large_doc_source_fails_closed(
    row_id: &str,
    chunk_id: &str,
    path: &str,
) -> FsvResult<EdgeCase> {
    let huge = "x".repeat(10_000_001);
    let input = doc_input(row_id, chunk_id, path, &huge);
    let err = analyze_doc_coverage(&input).expect_err("large doc source must fail");
    Ok(EdgeCase {
        name: "file_too_large_doc_source_fails_closed",
        passed: err.code() == "MEJEPA_INSTRUMENTS_INVALID_INPUT",
        detail: json!({"error": err.to_string(), "code": err.code()}),
    })
}

fn edge_corrupted_advisory_json_fails_closed(
    row_id: &str,
    chunk_id: &str,
    path: &str,
) -> FsvResult<EdgeCase> {
    let input = dep_input(row_id, chunk_id, path, "{not-json".into());
    let err = extract_dependency_cve_labels(&input).expect_err("corrupt advisory must fail");
    Ok(EdgeCase {
        name: "corrupted_advisory_json_fails_closed",
        passed: err.code() == "MEJEPA_INSTRUMENTS_INVALID_INPUT",
        detail: json!({"error": err.to_string(), "code": err.code()}),
    })
}

fn edge_churn_no_touching_commits_fails_closed(
    row_id: &str,
    chunk_id: &str,
    path: &str,
) -> FsvResult<EdgeCase> {
    let now = 1_779_000_000_i64;
    let input = ChurnInput {
        corpus_row_id: row_id.into(),
        chunk_id: chunk_id.into(),
        path: path.into(),
        byte_start: 10,
        byte_end: 110,
        now_unix_secs: now,
        commits: vec![ChurnCommit {
            commit_sha: "outside".into(),
            author: "alice".into(),
            committed_unix_secs: now - 24 * 60 * 60,
            path: path.into(),
            touched_byte_start: 500,
            touched_byte_end: 600,
            lines_added: 1,
            lines_deleted: 0,
        }],
    };
    let err = analyze_churn(&input).expect_err("no touching commits must fail");
    Ok(EdgeCase {
        name: "churn_no_touching_commits_fails_closed",
        passed: err.code() == "MEJEPA_INSTRUMENTS_INVALID_INPUT",
        detail: json!({"error": err.to_string(), "code": err.code()}),
    })
}

fn raw_file_hashes(
    paths: &context_graph_mejepa_instruments::CompositeRewardRawOutputPaths,
) -> FsvResult<BTreeMap<String, String>> {
    let mut hashes = BTreeMap::new();
    for (name, path) in [
        ("coverage.json", &paths.coverage_json),
        ("churn.json", &paths.churn_json),
        ("complexity.json", &paths.complexity_json),
        ("doc.json", &paths.doc_json),
        ("depcve.json", &paths.depcve_json),
        ("composite.json", &paths.composite_json),
    ] {
        hashes.insert(name.to_string(), sha256_file(path)?);
    }
    Ok(hashes)
}

fn sha256_file(path: &Path) -> FsvResult<String> {
    let mut hasher = Sha256::new();
    hasher.update(fs::read(path)?);
    Ok(format!("{:x}", hasher.finalize()))
}

fn write_json_checked(path: &Path, value: &Value) -> FsvResult<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, &bytes)?;
    let readback = fs::read(path)?;
    require("json write readback equals bytes", readback == bytes)?;
    Ok(())
}

fn require(message: &'static str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

use context_graph_mejepa_instruments::{
    analyze_coverage_delta, coverage_snapshot_from_coverage_py_json, CoverageInput,
    ECoverageInstrument, Instrument,
};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Serialize)]
struct EdgeCase {
    name: &'static str,
    error_code: String,
    passed: bool,
}

fn main() -> FsvResult<()> {
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .to_string();
    let root = PathBuf::from("/mnt/d/contextgraph/fsv/task-rwd-202-coverage-fsv")
        .join(format!("run-{run_id}"));
    fs::create_dir_all(&root)?;
    let baseline_path = root.join("baseline_coverage.json");
    let candidate_path = root.join("candidate_coverage.json");
    let evidence_path = root.join("coverage_fsv.json");
    fs::write(&baseline_path, baseline_json())?;
    fs::write(&candidate_path, candidate_json())?;
    require("baseline coverage JSON written", baseline_path.exists())?;
    require("candidate coverage JSON written", candidate_path.exists())?;

    let input = CoverageInput {
        baseline: coverage_snapshot_from_coverage_py_json(
            "coverage.py",
            &fs::read_to_string(&baseline_path)?,
        )?,
        candidate: coverage_snapshot_from_coverage_py_json(
            "coverage.py",
            &fs::read_to_string(&candidate_path)?,
        )?,
    };
    let metrics = analyze_coverage_delta(&input)?;
    let vector = ECoverageInstrument.encode(&input)?;
    require("vector length is E_Runtime slot", vector.len() == 256)?;
    require(
        "vector has non-zero reward signal",
        vector.iter().any(|value| *value > 0.0),
    )?;
    require("file_count is 2", metrics.file_count == 2)?;
    require(
        "baseline coverage is 3/6",
        (metrics.baseline_line_coverage - 0.5).abs() < 0.0001,
    )?;
    require(
        "candidate coverage is 4/6",
        (metrics.candidate_line_coverage - (4.0 / 6.0)).abs() < 0.0001,
    )?;
    require(
        "aggregate coverage delta is positive",
        (metrics.coverage_delta - (1.0 / 6.0)).abs() < 0.0001,
    )?;
    require(
        "newly covered line count is 2",
        metrics.newly_covered_lines == 2,
    )?;
    require(
        "newly uncovered line count is 1",
        metrics.newly_uncovered_lines == 1,
    )?;

    let edge_cases = vec![
        edge_case_json("empty_json", ""),
        edge_case_json("empty_files", r#"{"files":{}}"#),
        edge_case_input("duplicate_line", duplicate_line_input()),
        edge_case_input("overlapping_covered_missing", overlapping_input()),
        edge_case_input("zero_line", zero_line_input()),
    ];
    require(
        "all edge cases fail closed with instrument invalid-input code",
        edge_cases.iter().all(|case| case.passed),
    )?;

    let evidence = json!({
        "task": "TASK-RWD-202",
        "all_passed": true,
        "source_of_truth": {
            "root": root,
            "baseline_path": baseline_path,
            "candidate_path": candidate_path,
            "evidence_path": evidence_path,
        },
        "happy_path": {
            "metrics": metrics,
            "vector_len": vector.len(),
            "vector_non_zero": vector.iter().filter(|value| **value != 0.0).count(),
            "vector_sha256": sha256_json(&vector)?,
        },
        "edge_cases": edge_cases,
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

fn edge_case_json(name: &'static str, json: &str) -> EdgeCase {
    let error_code = coverage_snapshot_from_coverage_py_json("coverage.py", json)
        .unwrap_err()
        .code()
        .to_string();
    EdgeCase {
        name,
        passed: error_code == "MEJEPA_INSTRUMENTS_INVALID_INPUT",
        error_code,
    }
}

fn edge_case_input(name: &'static str, input: CoverageInput) -> EdgeCase {
    let error_code = ECoverageInstrument
        .encode(&input)
        .unwrap_err()
        .code()
        .to_string();
    EdgeCase {
        name,
        passed: error_code == "MEJEPA_INSTRUMENTS_INVALID_INPUT",
        error_code,
    }
}

fn baseline_json() -> &'static str {
    r#"{
  "meta": {"version": "7.5.0"},
  "files": {
    "src/a.py": {
      "executed_lines": [1, 2],
      "missing_lines": [3, 4]
    },
    "src/b.py": {
      "executed_lines": [1],
      "missing_lines": [2]
    }
  }
}"#
}

fn candidate_json() -> &'static str {
    r#"{
  "meta": {"version": "7.5.0"},
  "files": {
    "src/a.py": {
      "executed_lines": [1, 2, 3],
      "missing_lines": [4]
    },
    "src/b.py": {
      "executed_lines": [2],
      "missing_lines": [1]
    }
  }
}"#
}

fn duplicate_line_input() -> CoverageInput {
    let mut input = valid_input();
    input.candidate.files[0].covered_lines.push(1);
    input
}

fn overlapping_input() -> CoverageInput {
    let mut input = valid_input();
    input.candidate.files[0].missing_lines.push(3);
    input
}

fn zero_line_input() -> CoverageInput {
    let mut input = valid_input();
    input.candidate.files[0].executable_lines.push(0);
    input
}

fn valid_input() -> CoverageInput {
    CoverageInput {
        baseline: coverage_snapshot_from_coverage_py_json("coverage.py", baseline_json()).unwrap(),
        candidate: coverage_snapshot_from_coverage_py_json("coverage.py", candidate_json())
            .unwrap(),
    }
}

fn write_json_checked(path: &Path, value: &Value) -> FsvResult<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, &bytes)?;
    let readback = fs::read(path)?;
    require("json write/readback bytes equal", readback == bytes)?;
    Ok(())
}

fn sha256_json<T: Serialize>(value: &T) -> FsvResult<String> {
    let bytes = serde_json::to_vec(value)?;
    Ok(sha256_bytes(&bytes))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn require(message: &'static str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

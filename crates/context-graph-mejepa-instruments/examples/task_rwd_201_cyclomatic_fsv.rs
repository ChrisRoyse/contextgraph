use context_graph_mejepa_instruments::{
    analyze_complexity, ComplexityInput, ECyclomaticInstrument, Instrument,
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
    let root = PathBuf::from("/mnt/d/contextgraph/fsv/task-rwd-201-cyclomatic-fsv")
        .join(format!("run-{run_id}"));
    fs::create_dir_all(&root)?;
    let source_path = root.join("ladder.py");
    let evidence_path = root.join("cyclomatic_fsv.json");
    fs::write(&source_path, ladder_source())?;
    require("ladder source physically written", source_path.exists())?;

    let input = ComplexityInput {
        language: "python".to_string(),
        path: source_path.to_string_lossy().to_string(),
        source: fs::read_to_string(&source_path)?,
    };
    let metrics = analyze_complexity(&input)?;
    let vector = ECyclomaticInstrument.encode(&input)?;
    require(
        "vector length is E_StaticAnalysis slot",
        vector.len() == 256,
    )?;
    require(
        "vector has non-zero reward signal",
        vector.iter().any(|value| *value > 0.0),
    )?;
    require(
        "ladder cyclomatic complexity is 7",
        metrics.max_cyclomatic_complexity == 7,
    )?;
    require(
        "Halstead volume is positive",
        metrics.halstead_volume.is_finite() && metrics.halstead_volume > 0.0,
    )?;

    let nested_input = ComplexityInput {
        language: "python".to_string(),
        path: "nested.py".to_string(),
        source: nested_source().to_string(),
    };
    let nested_metrics = analyze_complexity(&nested_input)?;
    require(
        "nested source records depth",
        nested_metrics.max_nesting_depth >= 4,
    )?;
    require(
        "nested cognitive exceeds ladder baseline",
        nested_metrics.max_cognitive_complexity > metrics.max_cognitive_complexity,
    )?;

    let edge_cases = vec![
        edge_case(
            "empty_source",
            ComplexityInput {
                language: "python".to_string(),
                path: "empty.py".to_string(),
                source: String::new(),
            },
        ),
        edge_case(
            "syntax_error",
            ComplexityInput {
                language: "python".to_string(),
                path: "broken.py".to_string(),
                source: "def broken(:\n".to_string(),
            },
        ),
        edge_case(
            "non_python",
            ComplexityInput {
                language: "javascript".to_string(),
                path: "sample.js".to_string(),
                source: "function f() { return 1; }\n".to_string(),
            },
        ),
        edge_case(
            "nul_source",
            ComplexityInput {
                language: "python".to_string(),
                path: "nul.py".to_string(),
                source: "x = '\\0'\0\n".to_string(),
            },
        ),
    ];
    require(
        "all edge cases fail closed with instrument invalid-input code",
        edge_cases.iter().all(|case| case.passed),
    )?;

    let evidence = json!({
        "task": "TASK-RWD-201",
        "all_passed": true,
        "source_of_truth": {
            "root": root,
            "source_path": source_path,
            "evidence_path": evidence_path,
        },
        "happy_path": {
            "metrics": metrics,
            "vector_len": vector.len(),
            "vector_non_zero": vector.iter().filter(|value| **value != 0.0).count(),
            "vector_sha256": sha256_json(&vector)?,
        },
        "depth_case": {
            "metrics": nested_metrics,
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

fn edge_case(name: &'static str, input: ComplexityInput) -> EdgeCase {
    let error_code = ECyclomaticInstrument
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
    else:
        return "many"
"#
}

fn nested_source() -> &'static str {
    r#"def nested(values):
    total = 0
    for row in values:
        if row:
            while total < 10:
                if row[0] > total and row[0] < 99:
                    total += row[0]
    return total
"#
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

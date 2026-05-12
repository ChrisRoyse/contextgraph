use context_graph_mejepa_train::labels::{
    run_phase_b_label_analyzer, AnalyzerOutputMode, LabelInputLanguage, LabelSource,
    PhaseBLabelAnalyzerConfig,
};
use serde_json::{json, Value};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

fn main() -> FsvResult<()> {
    let root = PathBuf::from("/mnt/d/contextgraph/fsv/phase-b-label-analyzer-fsv");
    reset_secure_dir(&root)?;
    let python = std::env::var("PYTHON").unwrap_or_else(|_| "python3".to_string());
    write_fixture_files(&root)?;

    let positive = run_happy_positive(&root, &python)?;
    let negative = run_happy_negative(&root, &python)?;
    let edges = json!({
        "missing_analyzer_binary": run_edge_missing_binary(&root)?,
        "malformed_analyzer_output": run_edge_malformed_json(&root, &python)?,
        "unsupported_language": run_edge_unsupported_language(&root, &python)?,
    });
    let evidence_path = root.join("phase-b-label-analyzer-fsv.json");
    let evidence = json!({
        "source_of_truth": {
            "analyzer_output_files": root.display().to_string(),
            "label_rows": "secure JSON label rows written beside analyzer outputs"
        },
        "happy_path": {
            "known_positive": positive,
            "known_negative": negative
        },
        "edge_cases": edges
    });
    write_secure_json(&evidence_path, &evidence)?;
    let readback: Value = serde_json::from_str(&fs::read_to_string(&evidence_path)?)?;
    require(
        "positive labels persisted",
        readback["happy_path"]["known_positive"]["passes"] == true,
    )?;
    require(
        "negative labels persisted",
        readback["happy_path"]["known_negative"]["passes"] == true,
    )?;
    require(
        "edge cases fail closed",
        readback["edge_cases"]
            .as_object()
            .ok_or("edge_cases not object")?
            .values()
            .all(|edge| edge["passes"] == true),
    )?;
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn run_happy_positive(root: &Path, python: &str) -> FsvResult<Value> {
    let case = "known_positive_pytest_failure";
    let labels_path = root.join(format!("{case}-labels.json"));
    let before = json!({
        "labels_file_exists": labels_path.exists(),
        "analyzer": "python3 -m pytest -q tests/test_label_positive.py",
    });
    println!("before {case}: {}", serde_json::to_string(&before)?);
    let labels = run_phase_b_label_analyzer(&pytest_config(
        root,
        python,
        "tests/test_label_positive.py",
        true,
    ))?;
    write_secure_json(&labels_path, &serde_json::to_value(&labels)?)?;
    let readback: Value = serde_json::from_str(&fs::read_to_string(&labels_path)?)?;
    let after = json!({
        "labels_file_exists": labels_path.exists(),
        "failed_tests": readback["failed_tests"].as_array().map(Vec::len).unwrap_or(0),
        "failure_modes": readback["failure_modes"].as_array().map(Vec::len).unwrap_or(0),
        "edge_cases": readback["edge_cases"].as_array().map(Vec::len).unwrap_or(0),
        "readback": readback,
    });
    println!("after {case}: {}", serde_json::to_string(&after)?);
    let passes =
        after["failed_tests"] == 1 && after["failure_modes"] == 1 && after["edge_cases"] == 1;
    Ok(json!({ "before_state": before, "after_state": after, "passes": passes }))
}

fn run_happy_negative(root: &Path, python: &str) -> FsvResult<Value> {
    let case = "known_negative_pytest_pass";
    let labels_path = root.join(format!("{case}-labels.json"));
    let before = json!({
        "labels_file_exists": labels_path.exists(),
        "analyzer": "python3 -m pytest -q tests/test_label_negative.py",
    });
    println!("before {case}: {}", serde_json::to_string(&before)?);
    let labels = run_phase_b_label_analyzer(&pytest_config(
        root,
        python,
        "tests/test_label_negative.py",
        false,
    ))?;
    write_secure_json(&labels_path, &serde_json::to_value(&labels)?)?;
    let readback: Value = serde_json::from_str(&fs::read_to_string(&labels_path)?)?;
    let label_total = [
        "failed_tests",
        "failure_modes",
        "edge_cases",
        "latent_bugs",
        "security_concerns",
    ]
    .iter()
    .map(|field| {
        readback[*field]
            .as_array()
            .map(Vec::len)
            .unwrap_or(usize::MAX)
    })
    .sum::<usize>();
    let after = json!({
        "labels_file_exists": labels_path.exists(),
        "label_total": label_total,
        "reasoning_class": readback["reasoning_class"],
        "readback": readback,
    });
    println!("after {case}: {}", serde_json::to_string(&after)?);
    Ok(json!({ "before_state": before, "after_state": after, "passes": label_total == 0 }))
}

fn run_edge_missing_binary(root: &Path) -> FsvResult<Value> {
    let case = "missing_analyzer_binary";
    let before = json!({ "command": root.join("does-not-exist").display().to_string() });
    println!("before {case}: {}", serde_json::to_string(&before)?);
    let err = run_phase_b_label_analyzer(&PhaseBLabelAnalyzerConfig {
        source: LabelSource::Pytest,
        language: LabelInputLanguage::Python,
        program: root.join("does-not-exist"),
        args: vec!["-m".to_string(), "pytest".to_string()],
        cwd: root.to_path_buf(),
        output_mode: AnalyzerOutputMode::CommandLog,
        require_labels: true,
        max_output_bytes: 65_536,
    })
    .unwrap_err();
    let after = json!({ "accepted": false, "error_code": err.code(), "error": err.to_string() });
    println!("after {case}: {}", serde_json::to_string(&after)?);
    Ok(
        json!({ "before_state": before, "after_state": after, "expected_code": "UTML_IO", "passes": err.code() == "UTML_IO" }),
    )
}

fn run_edge_malformed_json(root: &Path, python: &str) -> FsvResult<Value> {
    let case = "malformed_analyzer_output";
    let before = json!({ "command": "python3 -c print('{not-json')" });
    println!("before {case}: {}", serde_json::to_string(&before)?);
    let err = run_phase_b_label_analyzer(&PhaseBLabelAnalyzerConfig {
        source: LabelSource::Pytest,
        language: LabelInputLanguage::Python,
        program: PathBuf::from(python),
        args: vec!["-c".to_string(), "print('{not-json')".to_string()],
        cwd: root.to_path_buf(),
        output_mode: AnalyzerOutputMode::StrictJson,
        require_labels: true,
        max_output_bytes: 65_536,
    })
    .unwrap_err();
    let after = json!({ "accepted": false, "error_code": err.code(), "error": err.to_string() });
    println!("after {case}: {}", serde_json::to_string(&after)?);
    Ok(
        json!({ "before_state": before, "after_state": after, "expected_code": "UTML_INVALID_SIGNAL", "passes": err.code() == "UTML_INVALID_SIGNAL" }),
    )
}

fn run_edge_unsupported_language(root: &Path, python: &str) -> FsvResult<Value> {
    let case = "unsupported_language";
    let before = json!({ "source": "pytest", "language": "rust" });
    println!("before {case}: {}", serde_json::to_string(&before)?);
    let err = run_phase_b_label_analyzer(&PhaseBLabelAnalyzerConfig {
        source: LabelSource::Pytest,
        language: LabelInputLanguage::Rust,
        program: PathBuf::from(python),
        args: vec!["-m".to_string(), "pytest".to_string()],
        cwd: root.to_path_buf(),
        output_mode: AnalyzerOutputMode::CommandLog,
        require_labels: true,
        max_output_bytes: 65_536,
    })
    .unwrap_err();
    let after = json!({ "accepted": false, "error_code": err.code(), "error": err.to_string() });
    println!("after {case}: {}", serde_json::to_string(&after)?);
    Ok(
        json!({ "before_state": before, "after_state": after, "expected_code": "UTML_INVALID_SIGNAL", "passes": err.code() == "UTML_INVALID_SIGNAL" }),
    )
}

fn pytest_config(
    root: &Path,
    python: &str,
    test_file: &str,
    require_labels: bool,
) -> PhaseBLabelAnalyzerConfig {
    PhaseBLabelAnalyzerConfig {
        source: LabelSource::Pytest,
        language: LabelInputLanguage::Python,
        program: PathBuf::from(python),
        args: vec![
            "-m".to_string(),
            "pytest".to_string(),
            "-q".to_string(),
            test_file.to_string(),
        ],
        cwd: root.to_path_buf(),
        output_mode: AnalyzerOutputMode::CommandLog,
        require_labels,
        max_output_bytes: 65_536,
    }
}

fn write_fixture_files(root: &Path) -> FsvResult<()> {
    fs::create_dir_all(root.join("tests"))?;
    write_checked(
        &root.join("tests/test_label_positive.py"),
        b"def parse_items(items):\n    assert items, 'empty input rejected'\n    return len(items)\n\n\ndef test_empty_input():\n    parse_items([])\n",
    )?;
    write_checked(
        &root.join("tests/test_label_negative.py"),
        b"def test_non_empty_input():\n    assert sum([1, 2, 3]) == 6\n",
    )?;
    Ok(())
}

fn reset_secure_dir(path: &Path) -> FsvResult<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

fn write_checked(path: &Path, bytes: &[u8]) -> FsvResult<()> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    let readback = fs::read(path)?;
    require(&format!("readback {}", path.display()), readback == bytes)?;
    Ok(())
}

fn write_secure_json(path: &Path, value: &Value) -> FsvResult<()> {
    let mut file = File::create(path)?;
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}

fn require(label: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("FSV assertion failed: {label}").into())
    }
}

use context_graph_mejepa_instruments::stdlib_registry::{
    default_registry_dir, detect_reinvented_wheels, load_registry_dir, EXPECTED_STDLIB_LANGUAGES,
};
use serde::Serialize;
use serde_json::json;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-d-fsv";

#[derive(Debug, Serialize)]
struct RegistryState {
    label: String,
    registry_file_count: usize,
    languages: Vec<String>,
}

#[derive(Debug, Serialize)]
struct EdgeCaseReport {
    case: String,
    before: RegistryState,
    error_code: Option<String>,
    error_message: Option<String>,
    after: RegistryState,
    detection_count: Option<usize>,
}

#[derive(Debug, Serialize)]
struct FsvReport {
    source_of_truth: serde_json::Value,
    before: RegistryState,
    after_load: RegistryState,
    python_detection: serde_json::Value,
    edge_cases: Vec<EdgeCaseReport>,
    evidence_path: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(FSV_ROOT)?;
    let registry_dir = default_registry_dir();
    let evidence_path = PathBuf::from(FSV_ROOT).join("stdlib-registry-fsv.json");
    let before = state("before_load", &registry_dir, &[])?;
    let registries = load_registry_dir(&registry_dir)?;
    let after_load = state(
        "after_load",
        &registry_dir,
        &registries.keys().cloned().collect::<Vec<_>>(),
    )?;
    if after_load.registry_file_count != EXPECTED_STDLIB_LANGUAGES.len() {
        return Err("registry file count did not match required language count".into());
    }

    let python_source = r#"
import time

for attempt in range(5):
    try:
        call_remote()
        break
    except Exception:
        time.sleep(2 ** attempt)
"#;
    let python_detection = detect_reinvented_wheels(&registries, "python", python_source)?;
    if !python_detection
        .iter()
        .any(|detection| detection.pattern_id == "python_retry_backoff")
    {
        return Err("python retry backoff pattern was not detected".into());
    }

    let empty_source_before = state(
        "empty_source_before",
        &registry_dir,
        &registries.keys().cloned().collect::<Vec<_>>(),
    )?;
    let empty_detections = detect_reinvented_wheels(&registries, "python", "")?;
    let empty_source_after = state(
        "empty_source_after",
        &registry_dir,
        &registries.keys().cloned().collect::<Vec<_>>(),
    )?;
    if !empty_detections.is_empty() {
        return Err("empty source unexpectedly produced stdlib detections".into());
    }

    let unknown_before = state(
        "unknown_language_before",
        &registry_dir,
        &registries.keys().cloned().collect::<Vec<_>>(),
    )?;
    let unknown_err = detect_reinvented_wheels(&registries, "unknown", "").unwrap_err();
    let unknown_after = state(
        "unknown_language_after",
        &registry_dir,
        &registries.keys().cloned().collect::<Vec<_>>(),
    )?;

    let missing_before = state(
        "missing_registry_before",
        &registry_dir,
        &registries.keys().cloned().collect::<Vec<_>>(),
    )?;
    let temp = tempfile::tempdir()?;
    fs::write(
        temp.path().join("python.yml"),
        "schema_version: 1\nlanguage: python\npatterns: []\n",
    )?;
    let missing_err = load_registry_dir(temp.path()).unwrap_err();
    let missing_after = state(
        "missing_registry_after",
        &registry_dir,
        &registries.keys().cloned().collect::<Vec<_>>(),
    )?;

    let edge_cases = vec![
        EdgeCaseReport {
            case: "empty_source_no_detection".to_string(),
            before: empty_source_before,
            error_code: None,
            error_message: None,
            after: empty_source_after,
            detection_count: Some(empty_detections.len()),
        },
        EdgeCaseReport {
            case: "unknown_language_fails_closed".to_string(),
            before: unknown_before,
            error_code: Some(unknown_err.code().to_string()),
            error_message: Some(unknown_err.to_string()),
            after: unknown_after,
            detection_count: None,
        },
        EdgeCaseReport {
            case: "missing_language_file_fails_closed".to_string(),
            before: missing_before,
            error_code: Some(missing_err.code().to_string()),
            error_message: Some(missing_err.to_string()),
            after: missing_after,
            detection_count: None,
        },
    ];

    let report = FsvReport {
        source_of_truth: json!({
            "type": "checked_in_yaml_registry_directory",
            "registry_dir": registry_dir,
            "required_languages": EXPECTED_STDLIB_LANGUAGES,
            "verification": "load every YAML file from disk, validate schema, then run detector against source text"
        }),
        before,
        after_load,
        python_detection: serde_json::to_value(&python_detection)?,
        edge_cases,
        evidence_path: evidence_path.display().to_string(),
    };
    fs::write(&evidence_path, serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn state(
    label: &str,
    registry_dir: &Path,
    languages: &[String],
) -> Result<RegistryState, Box<dyn Error>> {
    let registry_file_count = fs::read_dir(registry_dir)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "yml"))
        .count();
    let mut languages = languages.to_vec();
    languages.sort();
    Ok(RegistryState {
        label: label.to_string(),
        registry_file_count,
        languages,
    })
}

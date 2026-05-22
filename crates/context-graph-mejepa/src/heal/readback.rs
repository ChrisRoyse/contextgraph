use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::heal::drift::DriftSeverity;
use crate::heal::errors::HealError;

pub const READBACK_ROOT: &str = "/tmp/contextgraph-mejepa-self-healing-readback";
pub const READBACK_FILES: &[&str] = &[
    "drift-drill-evidence.json",
    "abc-promotion-evidence.json",
    "lora-refresh-evidence.json",
    "witness-integrity-evidence.json",
    "ewc-fisher-evidence.json",
];

pub fn write_readback_evidence_canonical<T: Serialize>(
    file_name: &str,
    payload: &T,
    root: Option<&Path>,
) -> Result<PathBuf, HealError> {
    if !READBACK_FILES.contains(&file_name) && file_name != "_master_index.json" {
        return Err(HealError::invalid(
            "readback.file_name",
            format!("unexpected readback file {file_name}"),
        ));
    }
    let root = root
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(READBACK_ROOT));
    fs::create_dir_all(&root).map_err(|err| HealError::io("create_dir_all", &root, err))?;
    let path = root.join(file_name);
    let mut bytes = serde_json::to_vec_pretty(payload)?;
    bytes.push(b'\n');
    fs::write(&path, bytes).map_err(|err| HealError::io("write", &path, err))?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
        .map_err(|err| HealError::io("chmod", &path, err))?;
    Ok(path)
}

pub fn assert_readback_directory_complete(root: Option<&Path>) -> Result<(), HealError> {
    let root = root
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(READBACK_ROOT));
    let mut missing = Vec::new();
    let mut wrong_mode = Vec::new();
    let mut parse_failures = Vec::new();
    for file in READBACK_FILES {
        let path = root.join(file);
        if !path.exists() {
            missing.push((*file).to_string());
            continue;
        }
        let meta = fs::metadata(&path).map_err(|err| HealError::io("metadata", &path, err))?;
        let mode = meta.permissions().mode() & 0o777;
        if mode != 0o600 {
            wrong_mode.push(((*file).to_string(), mode));
        }
        let bytes = fs::read(&path).map_err(|err| HealError::io("read", &path, err))?;
        if let Err(err) = serde_json::from_slice::<serde_json::Value>(&bytes) {
            parse_failures.push(format!("{file}: {err}"));
        }
    }
    if !missing.is_empty() || !wrong_mode.is_empty() || !parse_failures.is_empty() {
        return Err(HealError::ReadbackIncomplete {
            missing,
            wrong_mode,
            parse_failures,
        });
    }
    Ok(())
}

pub fn readback_directory_summary(
    root: Option<&Path>,
) -> Result<ReadbackDirectorySummary, HealError> {
    let root = root
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(READBACK_ROOT));
    let root_absolute = root.canonicalize().unwrap_or(root.clone());
    let mut file_paths_present = Vec::new();
    let mut file_modes = Vec::new();
    let mut file_sizes_bytes = Vec::new();
    let mut total_bytes = 0u64;
    for file in READBACK_FILES {
        let path = root.join(file);
        if path.exists() {
            let meta = fs::metadata(&path).map_err(|err| HealError::io("metadata", &path, err))?;
            file_paths_present.push(path);
            file_modes.push(meta.permissions().mode() & 0o777);
            file_sizes_bytes.push(meta.len());
            total_bytes += meta.len();
        }
    }
    let all_complete = file_paths_present.len() == READBACK_FILES.len()
        && file_modes.iter().all(|mode| *mode == 0o600);
    Ok(ReadbackDirectorySummary {
        root_absolute,
        file_paths_present,
        file_modes,
        file_sizes_bytes,
        total_bytes,
        all_complete,
    })
}

pub fn append_readback_master_index(root: Option<&Path>) -> Result<(), HealError> {
    let root = root
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(READBACK_ROOT));
    let summary = readback_directory_summary(Some(&root))?;
    let index = serde_json::json!({
        "phase": "phase5-self-healing",
        "commit_sha": option_env!("GIT_COMMIT").unwrap_or("unknown"),
        "generated_at": chrono::Utc::now().to_rfc3339(),
        "file_paths": summary.file_paths_present,
        "all_complete": summary.all_complete
    });
    write_readback_evidence_canonical("_master_index.json", &index, Some(&root))?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReadbackDirectorySummary {
    pub root_absolute: PathBuf,
    pub file_paths_present: Vec<PathBuf>,
    pub file_modes: Vec<u32>,
    pub file_sizes_bytes: Vec<u64>,
    pub total_bytes: u64,
    pub all_complete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DriftDrillEvidence {
    pub inject_drift: String,
    pub observations_to_detect: u64,
    pub severity_history: Vec<(u64, DriftSeverity)>,
    pub empirical_coverage_trajectory: Vec<f32>,
    pub max_observations: u64,
    pub hit_max_without_detection: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReadbackHarnessReport {
    pub drill_summary: serde_json::Value,
    pub readback_directory_summary: ReadbackDirectorySummary,
    pub ship_gate_pass: bool,
    pub all_5_files_present: bool,
    pub all_modes_0o600: bool,
    pub all_files_parse_as_json: bool,
    pub total_evidence_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_readback_evidence_canonical_sets_mode_0o600() {
        let temp = tempfile::tempdir().unwrap();
        let path = write_readback_evidence_canonical(
            "drift-drill-evidence.json",
            &serde_json::json!({"ok": true}),
            Some(temp.path()),
        )
        .unwrap();
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn readback_files_const_has_exactly_5_entries() {
        assert_eq!(READBACK_FILES.len(), 5);
        assert!(READBACK_ROOT.starts_with("/tmp/contextgraph"));
    }
}

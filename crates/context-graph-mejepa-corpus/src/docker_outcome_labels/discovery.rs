use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::types::{DockerOutcomeLabelRow, DockerOutcomeResult};

#[derive(Debug, Clone)]
pub(crate) struct PredictionPatchEvidence {
    pub(crate) jsonl_path: PathBuf,
    pub(crate) model_patch_sha256: String,
}

#[derive(Debug, Deserialize)]
struct PredictionRow {
    instance_id: String,
    model_name_or_path: String,
    model_patch: String,
}

pub(crate) fn discover_reports(
    corpus_root: &Path,
) -> DockerOutcomeResult<BTreeMap<String, PathBuf>> {
    let root = corpus_root.join("swebench-runs/logs/run_evaluation");
    let mut reports = BTreeMap::<String, PathBuf>::new();
    for path in recursive_files(&root, "report.json")? {
        let parts = path
            .components()
            .map(|part| part.as_os_str().to_string_lossy().to_string())
            .collect::<Vec<_>>();
        let Some(report_idx) = parts.iter().rposition(|part| part == "report.json") else {
            continue;
        };
        if report_idx < 2 {
            continue;
        }
        let task_id = &parts[report_idx - 1];
        let model = &parts[report_idx - 2];
        let Some(category) = model.strip_prefix("mejepa-") else {
            continue;
        };
        let key = DockerOutcomeLabelRow::storage_key(task_id, category);
        reports
            .entry(key)
            .and_modify(|current| {
                if path > *current {
                    *current = path.clone();
                }
            })
            .or_insert(path);
    }
    Ok(reports)
}

pub(crate) fn discover_prediction_patches(
    corpus_root: &Path,
) -> DockerOutcomeResult<BTreeMap<String, PredictionPatchEvidence>> {
    let root = corpus_root.join("swebench-runs");
    let mut out = BTreeMap::new();
    for path in recursive_files(&root, ".jsonl")? {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.starts_with("predictions-mejepaPhase0Droot") {
            continue;
        }
        for line in fs::read_to_string(&path)?.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let row: PredictionRow = serde_json::from_str(line)?;
            let Some(category) = row.model_name_or_path.strip_prefix("mejepa-") else {
                continue;
            };
            let key = DockerOutcomeLabelRow::storage_key(&row.instance_id, category);
            let evidence = PredictionPatchEvidence {
                jsonl_path: path.clone(),
                model_patch_sha256: sha256_text(&row.model_patch),
            };
            out.entry(key)
                .and_modify(|current: &mut PredictionPatchEvidence| {
                    if path > current.jsonl_path {
                        *current = evidence.clone();
                    }
                })
                .or_insert(evidence);
        }
    }
    Ok(out)
}

fn recursive_files(root: &Path, suffix: &str) -> DockerOutcomeResult<Vec<PathBuf>> {
    let mut out = Vec::new();
    if !root.exists() {
        return Ok(out);
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(suffix))
            {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

pub(crate) fn relative_or_absolute(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

pub(crate) fn sha256_file(path: &Path) -> DockerOutcomeResult<String> {
    Ok(format!("sha256:{}", sha256_bytes(&fs::read(path)?)))
}

pub(crate) fn sha256_text(text: &str) -> String {
    format!("sha256:{}", sha256_bytes(text.as_bytes()))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

use crate::{InstrumentError, InstrumentResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const PANEL_B_ARTIFACT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PanelBSlot {
    Ast,
    Cfg,
    DataFlow,
    TypeGraph,
    CodeSemantic,
    Text,
}

impl PanelBSlot {
    pub fn slug(&self) -> &'static str {
        match self {
            Self::Ast => "ast",
            Self::Cfg => "cfg",
            Self::DataFlow => "data_flow",
            Self::TypeGraph => "type_graph",
            Self::CodeSemantic => "code_semantic",
            Self::Text => "text",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PanelBArtifactSpec {
    pub slot: PanelBSlot,
    pub model_id: String,
    pub source_uri: String,
    pub source_revision: String,
    pub scorer_kind: String,
    pub relative_dir: String,
    pub required_files: Vec<String>,
}

impl PanelBArtifactSpec {
    pub fn new(
        slot: PanelBSlot,
        model_id: impl Into<String>,
        source_uri: impl Into<String>,
        source_revision: impl Into<String>,
        scorer_kind: impl Into<String>,
        relative_dir: impl Into<String>,
        required_files: Vec<&'static str>,
    ) -> Self {
        Self {
            slot,
            model_id: model_id.into(),
            source_uri: source_uri.into(),
            source_revision: source_revision.into(),
            scorer_kind: scorer_kind.into(),
            relative_dir: relative_dir.into(),
            required_files: required_files.into_iter().map(str::to_string).collect(),
        }
    }

    pub fn validate(&self) -> InstrumentResult<()> {
        validate_single_line("PanelBArtifactSpec.model_id", &self.model_id)?;
        validate_single_line("PanelBArtifactSpec.source_uri", &self.source_uri)?;
        validate_revision("PanelBArtifactSpec.source_revision", &self.source_revision)?;
        validate_single_line("PanelBArtifactSpec.scorer_kind", &self.scorer_kind)?;
        validate_relative_path("PanelBArtifactSpec.relative_dir", &self.relative_dir)?;
        if self.required_files.is_empty() {
            return Err(InstrumentError::invalid(
                "PanelBArtifactSpec.required_files",
                "no required files declared",
                "declare the exact artifact files needed to instantiate the Panel B encoder",
            ));
        }
        for file in &self.required_files {
            validate_relative_path("PanelBArtifactSpec.required_files", file)?;
        }
        let lower = format!(
            "{}\n{}\n{}",
            self.model_id, self.source_uri, self.scorer_kind
        )
        .to_ascii_lowercase();
        for forbidden in ["fake", "synthetic", "placeholder", "dummy", "stub"] {
            if lower.contains(forbidden) {
                return Err(InstrumentError::invalid(
                    "PanelBArtifactSpec.source",
                    format!("Panel B source {:?} contains {forbidden}", self.model_id),
                    "use a real model/tool source and hash its files from aiwonder storage",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PanelBArtifact {
    pub slot: PanelBSlot,
    pub model_id: String,
    pub source_uri: String,
    pub source_revision: String,
    pub scorer_kind: String,
    pub root: String,
    pub artifact_sha256: String,
    pub file_sha256: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MissingPanelBArtifact {
    pub slot: PanelBSlot,
    pub model_id: String,
    pub source_uri: String,
    pub source_revision: String,
    pub scorer_kind: String,
    pub root: String,
    pub missing_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PanelBArtifactManifest {
    pub schema_version: u32,
    pub models_root: String,
    pub required_artifact_count: usize,
    pub available_artifact_count: usize,
    pub artifacts: Vec<PanelBArtifact>,
    pub missing: Vec<MissingPanelBArtifact>,
}

impl PanelBArtifactManifest {
    pub fn all_required_available(&self) -> bool {
        self.available_artifact_count == self.required_artifact_count && self.missing.is_empty()
    }

    pub fn artifact_shas_by_slot(&self) -> BTreeMap<String, String> {
        self.artifacts
            .iter()
            .map(|artifact| {
                (
                    artifact.slot.slug().to_string(),
                    artifact.artifact_sha256.clone(),
                )
            })
            .collect()
    }
}

pub fn default_panel_b_artifact_specs() -> Vec<PanelBArtifactSpec> {
    vec![
        PanelBArtifactSpec::new(
            PanelBSlot::Ast,
            "microsoft/codebert-base",
            "https://huggingface.co/microsoft/codebert-base",
            "3b0952feddeffad0063f274080e3c23d75e7eb39",
            "hf_transformers_feature_extraction",
            "panel-b/codebert-base",
            vec![
                "config.json",
                "merges.txt",
                "pytorch_model.bin",
                "vocab.json",
            ],
        ),
        PanelBArtifactSpec::new(
            PanelBSlot::Cfg,
            "microsoft/graphcodebert-base",
            "https://huggingface.co/microsoft/graphcodebert-base",
            "2b0488a7bb0eefc7041f1bb2cad1ab26b0da269d",
            "hf_transformers_feature_extraction",
            "panel-b/graphcodebert-base",
            vec![
                "config.json",
                "merges.txt",
                "pytorch_model.bin",
                "vocab.json",
            ],
        ),
        PanelBArtifactSpec::new(
            PanelBSlot::CodeSemantic,
            "microsoft/unixcoder-base",
            "https://huggingface.co/microsoft/unixcoder-base",
            "5604afdc964f6c53782a6813140ade5216b99006",
            "hf_transformers_feature_extraction",
            "panel-b/unixcoder-base",
            vec![
                "config.json",
                "merges.txt",
                "pytorch_model.bin",
                "vocab.json",
            ],
        ),
        PanelBArtifactSpec::new(
            PanelBSlot::Text,
            "BAAI/bge-large-en-v1.5",
            "https://huggingface.co/BAAI/bge-large-en-v1.5",
            "d4aa6901d3a41ba39fb536a557fa166f842b0e09",
            "hf_transformers_feature_extraction",
            "panel-b/bge-large-en-v1.5",
            vec!["config.json", "tokenizer.json", "model.safetensors"],
        ),
    ]
}

pub fn resolve_panel_b_artifact_manifest(
    models_root: impl AsRef<Path>,
    specs: &[PanelBArtifactSpec],
) -> InstrumentResult<PanelBArtifactManifest> {
    let models_root = validate_aiwonder_models_root(models_root.as_ref())?;
    if specs.is_empty() {
        return Err(InstrumentError::invalid(
            "panel_b.specs",
            "no Panel B artifact specs supplied",
            "use default_panel_b_artifact_specs or pass explicit real artifact specs",
        ));
    }
    let mut artifacts = Vec::new();
    let mut missing = Vec::new();
    for spec in specs {
        spec.validate()?;
        let root = models_root.join(&spec.relative_dir);
        let mut missing_files = Vec::new();
        let mut file_sha256 = BTreeMap::new();
        for required in &spec.required_files {
            let path = root.join(required);
            if !path.is_file() {
                missing_files.push(required.clone());
                continue;
            }
            file_sha256.insert(required.clone(), sha256_file(&path)?);
        }
        if missing_files.is_empty() {
            artifacts.push(PanelBArtifact {
                slot: spec.slot.clone(),
                model_id: spec.model_id.clone(),
                source_uri: spec.source_uri.clone(),
                source_revision: spec.source_revision.clone(),
                scorer_kind: spec.scorer_kind.clone(),
                root: root.display().to_string(),
                artifact_sha256: artifact_sha256(spec, &file_sha256),
                file_sha256,
            });
        } else {
            missing.push(MissingPanelBArtifact {
                slot: spec.slot.clone(),
                model_id: spec.model_id.clone(),
                source_uri: spec.source_uri.clone(),
                source_revision: spec.source_revision.clone(),
                scorer_kind: spec.scorer_kind.clone(),
                root: root.display().to_string(),
                missing_files,
            });
        }
    }
    Ok(PanelBArtifactManifest {
        schema_version: PANEL_B_ARTIFACT_SCHEMA_VERSION,
        models_root: models_root.display().to_string(),
        required_artifact_count: specs.len(),
        available_artifact_count: artifacts.len(),
        artifacts,
        missing,
    })
}

pub fn require_panel_b_artifacts(manifest: &PanelBArtifactManifest) -> InstrumentResult<()> {
    if manifest.all_required_available() {
        return Ok(());
    }
    Err(InstrumentError::invalid(
        "panel_b.artifacts",
        format!(
            "Panel B has {}/{} required artifacts available; missing {}",
            manifest.available_artifact_count,
            manifest.required_artifact_count,
            manifest.missing.len()
        ),
        "install the required non-overlapping Panel B artifacts on aiwonder before enabling the cross-panel ship gate",
    ))
}

pub fn validate_aiwonder_models_root(path: &Path) -> InstrumentResult<PathBuf> {
    if !path.is_absolute() {
        return Err(model_root_error(path));
    }
    for root in [
        Path::new("/zfs/hot/contextgraph"),
        Path::new("/zfs/archive/contextgraph"),
        Path::new("/home/croyse/.cache/contextgraph"),
    ] {
        if path.starts_with(root) {
            return Ok(path.to_path_buf());
        }
    }
    Err(model_root_error(path))
}

fn sha256_file(path: &Path) -> InstrumentResult<String> {
    let mut file = fs::File::open(path).map_err(|err| {
        InstrumentError::invalid(
            "panel_b.artifact_file",
            format!("failed to open {}: {err}", path.display()),
            "verify the aiwonder model artifact path and permissions",
        )
    })?;
    let mut hasher = Sha256::new();
    let mut buf = [0_u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|err| {
            InstrumentError::invalid(
                "panel_b.artifact_file",
                format!("failed to read {}: {err}", path.display()),
                "verify the aiwonder model artifact path and storage health",
            )
        })?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn artifact_sha256(spec: &PanelBArtifactSpec, file_sha256: &BTreeMap<String, String>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(spec.model_id.as_bytes());
    hasher.update([0]);
    hasher.update(spec.source_uri.as_bytes());
    hasher.update([0]);
    hasher.update(spec.source_revision.as_bytes());
    hasher.update([0]);
    hasher.update(spec.scorer_kind.as_bytes());
    hasher.update([0]);
    for (relative, sha) in file_sha256 {
        hasher.update(relative.as_bytes());
        hasher.update([0]);
        hasher.update(sha.as_bytes());
        hasher.update([0]);
    }
    format!("{:x}", hasher.finalize())
}

fn validate_single_line(field: &'static str, value: &str) -> InstrumentResult<()> {
    if value.trim().is_empty() || value.contains('\n') || value.contains('\r') {
        return Err(InstrumentError::invalid(
            field,
            format!("invalid single-line value {value:?}"),
            "write non-empty single-line identifiers",
        ));
    }
    Ok(())
}

fn validate_revision(field: &'static str, value: &str) -> InstrumentResult<()> {
    validate_single_line(field, value)?;
    let is_hex_revision =
        (7..=64).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_hexdigit());
    if !is_hex_revision {
        return Err(InstrumentError::invalid(
            field,
            format!("revision {value:?} is not an immutable hex commit/revision"),
            "pin Panel B sources to immutable revisions before using them for gate evidence",
        ));
    }
    Ok(())
}

fn validate_relative_path(field: &'static str, value: &str) -> InstrumentResult<()> {
    validate_single_line(field, value)?;
    let path = Path::new(value);
    if path.is_absolute()
        || value.contains("..")
        || value.contains('\\')
        || value.starts_with('/')
        || value.starts_with('~')
    {
        return Err(InstrumentError::invalid(
            field,
            format!("path {value:?} is not a safe relative artifact path"),
            "store artifact paths relative to the approved aiwonder models root",
        ));
    }
    Ok(())
}

fn model_root_error(path: &Path) -> InstrumentError {
    InstrumentError::invalid(
        "panel_b.models_root",
        format!("{} is not an approved aiwonder model root", path.display()),
        "use /zfs/hot/contextgraph, /zfs/archive/contextgraph, or explicit aiwonder cache storage",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_aiwonder_model_root() {
        let err = validate_aiwonder_models_root(Path::new("/tmp/contextgraph/models"))
            .expect_err("non-aiwonder root must be rejected");
        assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
    }

    #[test]
    fn rejects_synthetic_artifact_ids() {
        let spec = PanelBArtifactSpec::new(
            PanelBSlot::Ast,
            "synthetic-codebert",
            "https://huggingface.co/microsoft/codebert-base",
            "3b0952feddeffad0063f274080e3c23d75e7eb39",
            "hf_transformers_feature_extraction",
            "panel-b/codebert-base",
            vec!["config.json"],
        );
        assert_eq!(
            spec.validate().unwrap_err().code(),
            "MEJEPA_INSTRUMENTS_INVALID_INPUT"
        );
    }

    #[test]
    fn rejects_branch_revisions() {
        let spec = PanelBArtifactSpec::new(
            PanelBSlot::Ast,
            "microsoft/codebert-base",
            "https://huggingface.co/microsoft/codebert-base",
            "main",
            "hf_transformers_feature_extraction",
            "panel-b/codebert-base",
            vec!["config.json"],
        );
        assert_eq!(
            spec.validate().unwrap_err().code(),
            "MEJEPA_INSTRUMENTS_INVALID_INPUT"
        );
    }

    #[test]
    fn reports_missing_artifacts_without_inventing_shas() {
        let manifest = resolve_panel_b_artifact_manifest(
            Path::new("/zfs/hot/contextgraph/models"),
            &[PanelBArtifactSpec::new(
                PanelBSlot::Text,
                "BAAI/bge-large-en-v1.5",
                "https://huggingface.co/BAAI/bge-large-en-v1.5",
                "d4aa6901d3a41ba39fb536a557fa166f842b0e09",
                "hf_transformers_feature_extraction",
                "panel-b/not-installed",
                vec!["config.json"],
            )],
        )
        .unwrap();
        assert!(!manifest.all_required_available());
        assert!(manifest.artifacts.is_empty());
        assert_eq!(manifest.missing.len(), 1);
        assert_eq!(
            manifest.missing[0].source_revision,
            "d4aa6901d3a41ba39fb536a557fa166f842b0e09"
        );
        assert_eq!(
            manifest.missing[0].scorer_kind,
            "hf_transformers_feature_extraction"
        );
        assert_eq!(
            require_panel_b_artifacts(&manifest).unwrap_err().code(),
            "MEJEPA_INSTRUMENTS_INVALID_INPUT"
        );
    }

    #[test]
    fn artifact_hash_changes_with_source_revision() {
        let mut file_sha256 = BTreeMap::new();
        file_sha256.insert("config.json".to_string(), "a".repeat(64));
        let first = PanelBArtifactSpec::new(
            PanelBSlot::Text,
            "BAAI/bge-large-en-v1.5",
            "https://huggingface.co/BAAI/bge-large-en-v1.5",
            "d4aa6901d3a41ba39fb536a557fa166f842b0e09",
            "hf_transformers_feature_extraction",
            "panel-b/bge-large-en-v1.5",
            vec!["config.json"],
        );
        let second = PanelBArtifactSpec::new(
            PanelBSlot::Text,
            "BAAI/bge-large-en-v1.5",
            "https://huggingface.co/BAAI/bge-large-en-v1.5",
            "eeeeeee1d3a41ba39fb536a557fa166f842b0e09",
            "hf_transformers_feature_extraction",
            "panel-b/bge-large-en-v1.5",
            vec!["config.json"],
        );
        assert_ne!(
            artifact_sha256(&first, &file_sha256),
            artifact_sha256(&second, &file_sha256)
        );
    }
}

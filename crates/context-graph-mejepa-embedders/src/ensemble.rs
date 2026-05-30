use crate::calibration::verify_calibration_certificate;
use crate::config::ModelsConfig;
use crate::digest::{verify_registration_digest, FileDigest};
use crate::embedder_id::EmbedderId;
use crate::error::{EmbedError, EmbedResult};
use crate::vram::{query_vram_budget, VramBudget};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoadedEmbedder {
    pub embedder: EmbedderId,
    pub name: String,
    pub manifest_sha256: String,
    pub file_count: usize,
    pub byte_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnsembleStatus {
    pub loaded_count: usize,
    pub loaded: Vec<LoadedEmbedder>,
    pub config_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ensemble {
    pub status: EnsembleStatus,
}

impl Ensemble {
    pub fn load_content_set(config_path: impl AsRef<Path>) -> EmbedResult<Self> {
        let config_path = config_path.as_ref();
        let config = ModelsConfig::load(config_path)?;
        let _budget = query_vram_budget(VramBudget::content_set_rtx5090())?;
        let loaded = verify_content_registrations(&config)?;
        Ok(Self {
            status: EnsembleStatus {
                loaded_count: loaded.len(),
                loaded,
                config_path: config_path.display().to_string(),
            },
        })
    }

    pub fn load_learner_state(
        config_path: impl AsRef<Path>,
        embedder: EmbedderId,
        calibration_cert_path: Option<&Path>,
    ) -> EmbedResult<LoadedEmbedder> {
        if !EmbedderId::learner_state().contains(&embedder) {
            return Err(EmbedError::invalid(
                "Ensemble.load_learner_state.embedder",
                format!("{embedder} is not in E15-E21 learner-state namespace"),
                "call load_content_set for E1-E14 content embedders",
            ));
        }
        if embedder == EmbedderId::E17 {
            let cert_path = calibration_cert_path.ok_or_else(|| EmbedError::E17Uncalibrated {
                cert_path: Path::new("memory/decisions/e17-calibration-certificate.json")
                    .to_path_buf(),
                message: "no certificate path supplied".into(),
                remediation:
                    "supply the signed E17 calibration certificate path before scoring agent state",
            })?;
            verify_calibration_certificate(cert_path, EmbedderId::E17)?;
        }
        let config = ModelsConfig::load(config_path)?;
        let reg = config.registration(embedder)?;
        let files = verify_registration_digest(reg)?;
        Ok(loaded_from_files(
            embedder,
            &reg.name,
            &reg.manifest_sha256,
            &files,
        ))
    }
}

pub fn verify_content_registrations(config: &ModelsConfig) -> EmbedResult<Vec<LoadedEmbedder>> {
    let mut loaded = Vec::new();
    for embedder in EmbedderId::content() {
        let reg = config.registration(embedder)?;
        let files = verify_registration_digest(reg)?;
        loaded.push(loaded_from_files(
            embedder,
            &reg.name,
            &reg.manifest_sha256,
            &files,
        ));
    }
    Ok(loaded)
}

pub fn verify_all_registration_digests(
    config: &ModelsConfig,
) -> EmbedResult<BTreeMap<EmbedderId, LoadedEmbedder>> {
    verify_required_registration_digests(config)
}

pub fn verify_required_registration_digests(
    config: &ModelsConfig,
) -> EmbedResult<BTreeMap<EmbedderId, LoadedEmbedder>> {
    let mut out = BTreeMap::new();
    for embedder in EmbedderId::required_registrations() {
        let reg = config.registration(embedder)?;
        let files = verify_registration_digest(reg)?;
        out.insert(
            embedder,
            loaded_from_files(embedder, &reg.name, &reg.manifest_sha256, &files),
        );
    }
    Ok(out)
}

pub fn verify_declared_registration_digests(
    config: &ModelsConfig,
) -> EmbedResult<BTreeMap<EmbedderId, LoadedEmbedder>> {
    let mut out = BTreeMap::new();
    for reg in config.embedders.values() {
        if reg.embedder.is_retired() {
            continue;
        }
        let files = verify_registration_digest(reg)?;
        out.insert(
            reg.embedder,
            loaded_from_files(reg.embedder, &reg.name, &reg.manifest_sha256, &files),
        );
    }
    Ok(out)
}

fn loaded_from_files(
    embedder: EmbedderId,
    name: &str,
    manifest_sha256: &str,
    files: &[FileDigest],
) -> LoadedEmbedder {
    LoadedEmbedder {
        embedder,
        name: name.to_string(),
        manifest_sha256: manifest_sha256.to_string(),
        file_count: files.len(),
        byte_count: files.iter().map(|file| file.size_bytes).sum(),
    }
}

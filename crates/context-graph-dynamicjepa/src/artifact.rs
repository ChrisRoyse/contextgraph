use context_graph_core::dynamicjepa::{
    ArtifactFile, DynamicJepaError, DynamicJepaResult, ModelArtifactId, ModelArtifactRecord,
    Validate,
};
use context_graph_storage::dynamicjepa::get_model_artifact;
use rocksdb::DB;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, serde::Serialize)]
pub struct ArtifactHashVerification {
    pub relative_path: String,
    pub registry_sha256: String,
    pub recomputed_sha256: String,
    pub registry_size_bytes: u64,
    pub recomputed_size_bytes: u64,
    pub equal: bool,
}

#[derive(Debug, Clone)]
pub struct LoadedArtifact {
    pub registry: ModelArtifactRecord,
    pub model_file_sha256: [u8; 32],
}

pub fn compute_file_sha256(path: &Path) -> DynamicJepaResult<[u8; 32]> {
    let mut file = fs::File::open(path).map_err(|err| DynamicJepaError::Storage {
        operation: "compute_file_sha256.open".to_string(),
        cf: "artifact_file".to_string(),
        message: format!("failed to open {}: {err}", path.display()),
        remediation: "verify the artifact file exists and rerun training if it is missing"
            .to_string(),
    })?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buf)
            .map_err(|err| DynamicJepaError::Storage {
                operation: "compute_file_sha256.read".to_string(),
                cf: "artifact_file".to_string(),
                message: format!("failed to read {}: {err}", path.display()),
                remediation:
                    "verify filesystem health and rerun training if the artifact is corrupt"
                        .to_string(),
            })?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(hasher.finalize().into())
}

pub fn compute_artifact_hashes(root: &Path) -> DynamicJepaResult<Vec<ArtifactFile>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(root).map_err(|err| DynamicJepaError::Storage {
        operation: "compute_artifact_hashes.read_dir".to_string(),
        cf: "artifact_root".to_string(),
        message: format!("failed to read artifact root {}: {err}", root.display()),
        remediation: "verify artifact_root is a readable completed artifact directory".to_string(),
    })? {
        let entry = entry.map_err(|err| DynamicJepaError::Storage {
            operation: "compute_artifact_hashes.dir_entry".to_string(),
            cf: "artifact_root".to_string(),
            message: err.to_string(),
            remediation: "verify artifact_root directory entries are readable".to_string(),
        })?;
        let file_type = entry.file_type().map_err(|err| DynamicJepaError::Storage {
            operation: "compute_artifact_hashes.file_type".to_string(),
            cf: "artifact_root".to_string(),
            message: format!("failed to stat {}: {err}", entry.path().display()),
            remediation: "verify artifact files were not removed during verification".to_string(),
        })?;
        if !file_type.is_file() {
            return Err(DynamicJepaError::validation(
                "artifact_root",
                format!("unexpected non-file entry {}", entry.path().display()),
                "artifact directories may contain only files from the DynamicJEPA artifact contract",
            ));
        }
        let path = entry.path();
        let relative_path = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                DynamicJepaError::validation(
                    "artifact_file",
                    format!("artifact path {} has no UTF-8 file name", path.display()),
                    "write artifact files with stable ASCII names",
                )
            })?
            .to_string();
        let metadata = fs::metadata(&path).map_err(|err| DynamicJepaError::Storage {
            operation: "compute_artifact_hashes.metadata".to_string(),
            cf: "artifact_file".to_string(),
            message: format!("failed to stat {}: {err}", path.display()),
            remediation: "verify artifact file exists and is readable".to_string(),
        })?;
        let record = ArtifactFile {
            relative_path,
            sha256: compute_file_sha256(&path)?,
            size_bytes: metadata.len(),
        };
        record.validate()?;
        files.push(record);
    }
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(files)
}

pub fn verify_artifact_files(
    artifact: &ModelArtifactRecord,
) -> DynamicJepaResult<Vec<ArtifactHashVerification>> {
    let recomputed = compute_artifact_hashes(&artifact.artifact_root)?;
    if artifact.files.len() != recomputed.len() {
        return Err(DynamicJepaError::ArtifactHashMismatch {
            artifact_id: artifact.artifact_id.0,
            file: "artifact_root".to_string(),
            expected: format!("{} files", artifact.files.len()),
            actual: format!("{} files", recomputed.len()),
        });
    }
    artifact
        .files
        .iter()
        .zip(recomputed.iter())
        .map(|(registry, disk)| {
            if registry.relative_path != disk.relative_path {
                return Err(DynamicJepaError::ArtifactHashMismatch {
                    artifact_id: artifact.artifact_id.0,
                    file: registry.relative_path.clone(),
                    expected: registry.relative_path.clone(),
                    actual: disk.relative_path.clone(),
                });
            }
            Ok(ArtifactHashVerification {
                relative_path: registry.relative_path.clone(),
                registry_sha256: hex(&registry.sha256),
                recomputed_sha256: hex(&disk.sha256),
                registry_size_bytes: registry.size_bytes,
                recomputed_size_bytes: disk.size_bytes,
                equal: registry.sha256 == disk.sha256 && registry.size_bytes == disk.size_bytes,
            })
        })
        .collect()
}

pub fn load_artifact_for_inference(
    db: &DB,
    artifact_id: ModelArtifactId,
) -> DynamicJepaResult<LoadedArtifact> {
    let registry = get_model_artifact(db, artifact_id)?.ok_or_else(|| {
        DynamicJepaError::SourceOfTruthMissing {
            cf: context_graph_storage::dynamicjepa::column_families::CF_DJ_MODEL_ARTIFACTS
                .to_string(),
            key: artifact_id.into_bytes().to_vec(),
        }
    })?;
    let checks = verify_artifact_files(&registry)?;
    for check in &checks {
        if !check.equal {
            return Err(DynamicJepaError::ArtifactHashMismatch {
                artifact_id: registry.artifact_id.0,
                file: check.relative_path.clone(),
                expected: check.registry_sha256.clone(),
                actual: check.recomputed_sha256.clone(),
            });
        }
    }
    let model_file = registry.artifact_root.join("model.safetensors");
    let model_file_sha256 = compute_file_sha256(&model_file)?;
    let _weights =
        candle_core::safetensors::load(&model_file, &candle_core::Device::Cpu).map_err(|err| {
            DynamicJepaError::TrainingFailed {
            training_run_id: registry.training_run_id.0,
            message: format!("failed to load model.safetensors: {err}"),
            remediation:
                "rerun training; artifact load must not continue after safetensors decode failure"
                    .to_string(),
        }
        })?;
    Ok(LoadedArtifact {
        registry,
        model_file_sha256,
    })
}

pub fn ensure_clean_run_dir(
    base_root: &Path,
    domain: &str,
    run_id: uuid::Uuid,
) -> DynamicJepaResult<PathBuf> {
    let run_dir = base_root.join(domain).join(run_id.to_string());
    if run_dir.exists() {
        return Err(DynamicJepaError::validation(
            "artifact_root",
            format!(
                "artifact run directory already exists: {}",
                run_dir.display()
            ),
            "delete the pre-production artifact root or use a fresh training run id",
        ));
    }
    fs::create_dir_all(&run_dir).map_err(|err| DynamicJepaError::Storage {
        operation: "artifact.create_run_dir".to_string(),
        cf: "artifact_root".to_string(),
        message: format!("failed to create {}: {err}", run_dir.display()),
        remediation: "verify artifact_root is writable".to_string(),
    })?;
    Ok(run_dir)
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

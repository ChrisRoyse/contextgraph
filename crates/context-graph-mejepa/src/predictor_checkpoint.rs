use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use candle_core::safetensors::MmapedSafetensors;
use candle_core::DType;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::config::{validate_aiwonder_checkpoint_path, PredictorConfig};
use crate::error::PredictorError;
use crate::predictor::MeJepaPredictor;

pub const PREDICTOR_CHECKPOINT_MANIFEST_SCHEMA_VERSION: u16 = 1;
pub const PREDICTOR_CHECKPOINT_TRAINING_STATUS_TRAINED: &str = "trained";
pub const PREDICTOR_CHECKPOINT_STUB_MAGIC: &[u8] = b"MEJEPA_BEST_STUB_V1";
pub const PREDICTOR_CHECKPOINT_FILE_NAME: &str = "best.safetensors";
pub const PREDICTOR_CHECKPOINT_MANIFEST_FILE_NAME: &str = "best.manifest.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PredictorCheckpointManifest {
    pub schema_version: u16,
    pub checkpoint_file: String,
    pub checkpoint_sha256: String,
    pub checkpoint_bytes: u64,
    pub architecture_sha256: String,
    pub predictor_config_sha256: String,
    pub payload_step: u64,
    pub optimizer_steps: u64,
    pub training_mode: String,
    pub training_status: String,
    pub initial_weight_sha256: String,
    pub trained_weight_sha256: String,
    pub training_certificate_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_active_constellation_adapter: Option<NativeActiveConstellationAdapterEvidence>,
    pub corpus_sha256: String,
    pub config_sha256: String,
    pub code_version: String,
    pub created_at_unix_ms: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeActiveConstellationAdapterEvidence {
    pub manifest_sha256: String,
    pub checkpoint_sha256: String,
    pub training_certificate_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoadedPredictorCheckpoint {
    pub manifest_path: PathBuf,
    pub checkpoint_path: PathBuf,
    pub manifest_sha256: String,
    pub checkpoint_sha256: String,
    pub checkpoint_bytes: u64,
    pub architecture_sha256: String,
    pub predictor_config_sha256: String,
    pub payload_step: u64,
    pub optimizer_steps: u64,
    pub training_mode: String,
    pub initial_weight_sha256: String,
    pub trained_weight_sha256: String,
    pub training_certificate_sha256: String,
    pub native_active_constellation_adapter: Option<NativeActiveConstellationAdapterEvidence>,
    pub corpus_sha256: String,
    pub config_sha256: String,
    pub code_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PredictorCheckpointExportMetadata {
    pub payload_step: u64,
    pub optimizer_steps: u64,
    pub training_mode: String,
    pub initial_weight_sha256: String,
    pub training_certificate_sha256: String,
    pub native_active_constellation_adapter: Option<NativeActiveConstellationAdapterEvidence>,
    pub corpus_sha256: String,
    pub config_sha256: String,
    pub code_version: String,
    pub created_at_unix_ms: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportedPredictorCheckpoint {
    pub manifest_path: PathBuf,
    pub checkpoint_path: PathBuf,
    pub manifest_sha256: String,
    pub checkpoint_sha256: String,
    pub checkpoint_bytes: u64,
    pub architecture_sha256: String,
    pub predictor_config_sha256: String,
    pub payload_step: u64,
    pub optimizer_steps: u64,
    pub training_mode: String,
    pub initial_weight_sha256: String,
    pub trained_weight_sha256: String,
    pub training_certificate_sha256: String,
    pub native_active_constellation_adapter: Option<NativeActiveConstellationAdapterEvidence>,
    pub corpus_sha256: String,
    pub config_sha256: String,
    pub code_version: String,
}

pub fn export_trained_predictor_checkpoint(
    predictor: &MeJepaPredictor,
    checkpoint_dir: &Path,
    expected_config: &PredictorConfig,
    metadata: PredictorCheckpointExportMetadata,
) -> Result<ExportedPredictorCheckpoint, PredictorError> {
    let checkpoint_dir = checkpoint_dir.to_path_buf();
    validate_aiwonder_checkpoint_path("trained_checkpoint_export_dir", &checkpoint_dir)?;
    validate_export_metadata(&metadata)?;
    fs::create_dir_all(&checkpoint_dir)?;
    let trained_weight_sha256 = predictor_weight_content_sha256(predictor)?;
    if trained_weight_sha256 == metadata.initial_weight_sha256 {
        return Err(PredictorError::ConfigInvalid {
            detail: "trained_weight_sha256 equals initial_weight_sha256; refusing to export unchanged predictor weights as trained".to_string(),
        });
    }

    let tmp_checkpoint_path = checkpoint_dir.join(format!("{PREDICTOR_CHECKPOINT_FILE_NAME}.tmp"));
    let checkpoint_path = checkpoint_dir.join(PREDICTOR_CHECKPOINT_FILE_NAME);
    if tmp_checkpoint_path.exists() {
        fs::remove_file(&tmp_checkpoint_path)?;
    }
    predictor.varmap.save(&tmp_checkpoint_path)?;
    set_private_file_permissions(&tmp_checkpoint_path)?;
    sync_existing_file(&tmp_checkpoint_path)?;
    fs::rename(&tmp_checkpoint_path, &checkpoint_path)?;
    sync_existing_file(&checkpoint_path)?;

    let checkpoint_bytes = fs::read(&checkpoint_path)?;
    if checkpoint_bytes.is_empty() {
        return Err(PredictorError::ConfigInvalid {
            detail: format!("exported checkpoint {} is empty", checkpoint_path.display()),
        });
    }
    if checkpoint_bytes.starts_with(PREDICTOR_CHECKPOINT_STUB_MAGIC)
        || checkpoint_bytes
            .windows(PREDICTOR_CHECKPOINT_STUB_MAGIC.len())
            .any(|window| window == PREDICTOR_CHECKPOINT_STUB_MAGIC)
    {
        return Err(PredictorError::ConfigInvalid {
            detail: format!(
                "exported checkpoint {} contains MEJEPA_BEST_STUB_V1 text-stub magic",
                checkpoint_path.display()
            ),
        });
    }

    let checkpoint_sha256 = sha256_hex(&checkpoint_bytes);
    let predictor_config_sha256 = predictor_config_sha256(expected_config)?;
    let architecture_sha256 = predictor_architecture_sha256(predictor, expected_config)?;
    let manifest = PredictorCheckpointManifest {
        schema_version: PREDICTOR_CHECKPOINT_MANIFEST_SCHEMA_VERSION,
        checkpoint_file: PREDICTOR_CHECKPOINT_FILE_NAME.to_string(),
        checkpoint_sha256,
        checkpoint_bytes: checkpoint_bytes.len() as u64,
        architecture_sha256,
        predictor_config_sha256,
        payload_step: metadata.payload_step,
        optimizer_steps: metadata.optimizer_steps,
        training_mode: metadata.training_mode,
        training_status: PREDICTOR_CHECKPOINT_TRAINING_STATUS_TRAINED.to_string(),
        initial_weight_sha256: metadata.initial_weight_sha256,
        trained_weight_sha256,
        training_certificate_sha256: metadata.training_certificate_sha256,
        native_active_constellation_adapter: metadata.native_active_constellation_adapter,
        corpus_sha256: metadata.corpus_sha256,
        config_sha256: metadata.config_sha256,
        code_version: metadata.code_version,
        created_at_unix_ms: metadata.created_at_unix_ms,
    };
    validate_manifest(&manifest)?;

    let manifest_path = checkpoint_dir.join(PREDICTOR_CHECKPOINT_MANIFEST_FILE_NAME);
    let tmp_manifest_path =
        checkpoint_dir.join(format!("{PREDICTOR_CHECKPOINT_MANIFEST_FILE_NAME}.tmp"));
    write_bytes_atomic_0600(
        &manifest_path,
        &tmp_manifest_path,
        &serde_json::to_vec_pretty(&manifest)?,
    )?;
    let manifest_bytes = fs::read(&manifest_path)?;
    let manifest_sha256 = sha256_hex(&manifest_bytes);

    Ok(ExportedPredictorCheckpoint {
        manifest_path,
        checkpoint_path,
        manifest_sha256,
        checkpoint_sha256: manifest.checkpoint_sha256,
        checkpoint_bytes: manifest.checkpoint_bytes,
        architecture_sha256: manifest.architecture_sha256,
        predictor_config_sha256: manifest.predictor_config_sha256,
        payload_step: manifest.payload_step,
        optimizer_steps: manifest.optimizer_steps,
        training_mode: manifest.training_mode,
        initial_weight_sha256: manifest.initial_weight_sha256,
        trained_weight_sha256: manifest.trained_weight_sha256,
        training_certificate_sha256: manifest.training_certificate_sha256,
        native_active_constellation_adapter: manifest.native_active_constellation_adapter,
        corpus_sha256: manifest.corpus_sha256,
        config_sha256: manifest.config_sha256,
        code_version: manifest.code_version,
    })
}

pub fn load_verified_trained_predictor_checkpoint(
    predictor: &mut MeJepaPredictor,
    manifest_path: &Path,
    expected_config: &PredictorConfig,
) -> Result<LoadedPredictorCheckpoint, PredictorError> {
    let manifest_path = manifest_path.to_path_buf();
    validate_aiwonder_checkpoint_path("trained_checkpoint_manifest_path", &manifest_path)?;
    let manifest_bytes = fs::read(&manifest_path)?;
    let manifest_sha256 = sha256_hex(&manifest_bytes);
    let manifest: PredictorCheckpointManifest = serde_json::from_slice(&manifest_bytes)?;
    validate_manifest(&manifest)?;
    let root = manifest_path
        .parent()
        .ok_or_else(|| PredictorError::ConfigInvalid {
            detail: format!(
                "checkpoint manifest has no parent: {}",
                manifest_path.display()
            ),
        })?;
    let checkpoint_path = root.join(&manifest.checkpoint_file);
    validate_aiwonder_checkpoint_path("trained_checkpoint_file", &checkpoint_path)?;
    let checkpoint_bytes = fs::read(&checkpoint_path)?;
    if checkpoint_bytes.starts_with(PREDICTOR_CHECKPOINT_STUB_MAGIC)
        || checkpoint_bytes
            .windows(PREDICTOR_CHECKPOINT_STUB_MAGIC.len())
            .any(|window| window == PREDICTOR_CHECKPOINT_STUB_MAGIC)
    {
        return Err(PredictorError::ConfigInvalid {
            detail: format!(
                "checkpoint {} is a MEJEPA_BEST_STUB_V1 text artifact, not trained safetensors",
                checkpoint_path.display()
            ),
        });
    }
    let observed_sha256 = sha256_hex(&checkpoint_bytes);
    if observed_sha256 != manifest.checkpoint_sha256 {
        return Err(PredictorError::ConfigInvalid {
            detail: format!(
                "checkpoint SHA-256 mismatch for {} expected {} got {}",
                checkpoint_path.display(),
                manifest.checkpoint_sha256,
                observed_sha256
            ),
        });
    }
    if checkpoint_bytes.len() as u64 != manifest.checkpoint_bytes {
        return Err(PredictorError::ConfigInvalid {
            detail: format!(
                "checkpoint byte length mismatch for {} expected {} got {}",
                checkpoint_path.display(),
                manifest.checkpoint_bytes,
                checkpoint_bytes.len()
            ),
        });
    }

    let expected_config_sha = predictor_config_sha256(expected_config)?;
    if manifest.predictor_config_sha256 != expected_config_sha {
        return Err(PredictorError::ConfigInvalid {
            detail: format!(
                "predictor_config_sha256 mismatch expected {} got {}",
                expected_config_sha, manifest.predictor_config_sha256
            ),
        });
    }
    let expected_architecture_sha = predictor_architecture_sha256(predictor, expected_config)?;
    if manifest.architecture_sha256 != expected_architecture_sha {
        return Err(PredictorError::ConfigInvalid {
            detail: format!(
                "architecture_sha256 mismatch expected {} got {}",
                expected_architecture_sha, manifest.architecture_sha256
            ),
        });
    }

    load_safetensors_strict(predictor, &checkpoint_path)?;
    Ok(LoadedPredictorCheckpoint {
        manifest_path,
        checkpoint_path,
        manifest_sha256,
        checkpoint_sha256: manifest.checkpoint_sha256,
        checkpoint_bytes: manifest.checkpoint_bytes,
        architecture_sha256: manifest.architecture_sha256,
        predictor_config_sha256: manifest.predictor_config_sha256,
        payload_step: manifest.payload_step,
        optimizer_steps: manifest.optimizer_steps,
        training_mode: manifest.training_mode,
        initial_weight_sha256: manifest.initial_weight_sha256,
        trained_weight_sha256: manifest.trained_weight_sha256,
        training_certificate_sha256: manifest.training_certificate_sha256,
        native_active_constellation_adapter: manifest.native_active_constellation_adapter,
        corpus_sha256: manifest.corpus_sha256,
        config_sha256: manifest.config_sha256,
        code_version: manifest.code_version,
    })
}

pub fn predictor_config_sha256(config: &PredictorConfig) -> Result<String, PredictorError> {
    sha256_json(config)
}

pub fn predictor_architecture_sha256(
    predictor: &MeJepaPredictor,
    expected_config: &PredictorConfig,
) -> Result<String, PredictorError> {
    sha256_json(&json!({
        "predictor_config": expected_config,
        "architecture_summary": predictor.architecture_summary(),
        "weight_shapes": predictor_weight_shapes(predictor)?,
    }))
}

pub fn predictor_weight_content_sha256(
    predictor: &MeJepaPredictor,
) -> Result<String, PredictorError> {
    let data = predictor
        .varmap
        .data()
        .lock()
        .map_err(|_| PredictorError::ConfigInvalid {
            detail: "predictor VarMap mutex was poisoned while hashing checkpoint weights"
                .to_string(),
        })?;
    let mut names = data.keys().cloned().collect::<Vec<_>>();
    names.sort();
    let mut hasher = Sha256::new();
    for name in names {
        let var = data
            .get(&name)
            .ok_or_else(|| PredictorError::ConfigInvalid {
                detail: format!("predictor VarMap key {name} disappeared while hashing weights"),
            })?;
        let tensor = var.as_tensor();
        hasher.update(name.as_bytes());
        hasher.update([0]);
        hasher.update(format!("{:?}", tensor.dtype()).as_bytes());
        hasher.update([0]);
        for dim in tensor.dims() {
            hasher.update(dim.to_le_bytes());
        }
        hasher.update([0]);
        let values = tensor
            .to_dtype(DType::F32)?
            .flatten_all()?
            .to_vec1::<f32>()?;
        for value in values {
            hasher.update(value.to_le_bytes());
        }
    }
    Ok(hex::encode(hasher.finalize()))
}

fn validate_manifest(manifest: &PredictorCheckpointManifest) -> Result<(), PredictorError> {
    if manifest.schema_version != PREDICTOR_CHECKPOINT_MANIFEST_SCHEMA_VERSION {
        return Err(PredictorError::ConfigInvalid {
            detail: format!(
                "predictor checkpoint manifest schema_version {} != {}",
                manifest.schema_version, PREDICTOR_CHECKPOINT_MANIFEST_SCHEMA_VERSION
            ),
        });
    }
    if manifest.checkpoint_file.trim().is_empty()
        || manifest.checkpoint_file.contains('/')
        || manifest.checkpoint_file.contains('\\')
        || !manifest.checkpoint_file.ends_with(".safetensors")
    {
        return Err(PredictorError::ConfigInvalid {
            detail: "checkpoint_file must be a local .safetensors file name".to_string(),
        });
    }
    for (field, value) in [
        ("checkpoint_sha256", &manifest.checkpoint_sha256),
        ("architecture_sha256", &manifest.architecture_sha256),
        ("predictor_config_sha256", &manifest.predictor_config_sha256),
        ("initial_weight_sha256", &manifest.initial_weight_sha256),
        ("trained_weight_sha256", &manifest.trained_weight_sha256),
        (
            "training_certificate_sha256",
            &manifest.training_certificate_sha256,
        ),
        ("corpus_sha256", &manifest.corpus_sha256),
        ("config_sha256", &manifest.config_sha256),
    ] {
        validate_lower_sha256(field, value)?;
    }
    if manifest.checkpoint_bytes == 0 {
        return Err(PredictorError::ConfigInvalid {
            detail: "checkpoint_bytes must be > 0".to_string(),
        });
    }
    if manifest.payload_step == 0 {
        return Err(PredictorError::ConfigInvalid {
            detail: "payload_step must be > 0 for a trained predictor checkpoint".to_string(),
        });
    }
    if manifest.optimizer_steps == 0 {
        return Err(PredictorError::ConfigInvalid {
            detail: "optimizer_steps must be > 0 for a trained predictor checkpoint".to_string(),
        });
    }
    if manifest.initial_weight_sha256 == manifest.trained_weight_sha256 {
        return Err(PredictorError::ConfigInvalid {
            detail:
                "initial_weight_sha256 and trained_weight_sha256 must differ for trained checkpoints"
                    .to_string(),
        });
    }
    if manifest.training_mode.trim().is_empty() || manifest.code_version.trim().is_empty() {
        return Err(PredictorError::ConfigInvalid {
            detail: "training_mode and code_version must be non-empty".to_string(),
        });
    }
    if let Some(evidence) = &manifest.native_active_constellation_adapter {
        validate_native_active_constellation_adapter_evidence(evidence)?;
    }
    if manifest.training_status != PREDICTOR_CHECKPOINT_TRAINING_STATUS_TRAINED {
        return Err(PredictorError::ConfigInvalid {
            detail: format!(
                "training_status must be {:?}; got {:?}",
                PREDICTOR_CHECKPOINT_TRAINING_STATUS_TRAINED, manifest.training_status
            ),
        });
    }
    Ok(())
}

fn validate_export_metadata(
    metadata: &PredictorCheckpointExportMetadata,
) -> Result<(), PredictorError> {
    if metadata.payload_step == 0 || metadata.optimizer_steps == 0 {
        return Err(PredictorError::ConfigInvalid {
            detail: "payload_step and optimizer_steps must be > 0 for checkpoint export"
                .to_string(),
        });
    }
    if metadata.training_mode.trim().is_empty() || metadata.code_version.trim().is_empty() {
        return Err(PredictorError::ConfigInvalid {
            detail: "training_mode and code_version must be non-empty for checkpoint export"
                .to_string(),
        });
    }
    validate_lower_sha256("initial_weight_sha256", &metadata.initial_weight_sha256)?;
    validate_lower_sha256(
        "training_certificate_sha256",
        &metadata.training_certificate_sha256,
    )?;
    if let Some(evidence) = &metadata.native_active_constellation_adapter {
        validate_native_active_constellation_adapter_evidence(evidence)?;
    }
    validate_lower_sha256("corpus_sha256", &metadata.corpus_sha256)?;
    validate_lower_sha256("config_sha256", &metadata.config_sha256)?;
    Ok(())
}

fn validate_native_active_constellation_adapter_evidence(
    evidence: &NativeActiveConstellationAdapterEvidence,
) -> Result<(), PredictorError> {
    validate_lower_sha256(
        "native_active_constellation_adapter.manifest_sha256",
        &evidence.manifest_sha256,
    )?;
    validate_lower_sha256(
        "native_active_constellation_adapter.checkpoint_sha256",
        &evidence.checkpoint_sha256,
    )?;
    validate_lower_sha256(
        "native_active_constellation_adapter.training_certificate_sha256",
        &evidence.training_certificate_sha256,
    )?;
    Ok(())
}

fn load_safetensors_strict(
    predictor: &mut MeJepaPredictor,
    checkpoint_path: &Path,
) -> Result<(), PredictorError> {
    let expected = predictor_weight_shapes(predictor)?;
    let data = unsafe { MmapedSafetensors::new(checkpoint_path) }?;
    let mut observed = BTreeMap::new();
    for (name, view) in data.tensors() {
        observed.insert(name, view.shape().to_vec());
    }
    let expected_keys = expected.keys().cloned().collect::<BTreeSet<_>>();
    let observed_keys = observed.keys().cloned().collect::<BTreeSet<_>>();
    if expected_keys != observed_keys {
        let missing = expected_keys
            .difference(&observed_keys)
            .take(16)
            .cloned()
            .collect::<Vec<_>>();
        let unexpected = observed_keys
            .difference(&expected_keys)
            .take(16)
            .cloned()
            .collect::<Vec<_>>();
        return Err(PredictorError::ConfigInvalid {
            detail: format!(
                "checkpoint tensor key set mismatch missing={missing:?} unexpected={unexpected:?}"
            ),
        });
    }
    for (name, shape) in &expected {
        let observed_shape = observed
            .get(name)
            .ok_or_else(|| PredictorError::ConfigInvalid {
                detail: format!("checkpoint tensor {name} missing after key-set validation"),
            })?;
        if observed_shape != shape {
            return Err(PredictorError::ConfigInvalid {
                detail: format!(
                    "checkpoint tensor {name} shape mismatch expected {shape:?} got {observed_shape:?}"
                ),
            });
        }
    }
    let mut varmap = predictor.varmap.clone();
    varmap.load(checkpoint_path)?;
    Ok(())
}

fn predictor_weight_shapes(
    predictor: &MeJepaPredictor,
) -> Result<BTreeMap<String, Vec<usize>>, PredictorError> {
    let data = predictor
        .varmap
        .data()
        .lock()
        .map_err(|_| PredictorError::ConfigInvalid {
            detail: "predictor VarMap mutex was poisoned while inspecting checkpoint shapes"
                .to_string(),
        })?;
    Ok(data
        .iter()
        .map(|(name, var)| (name.clone(), var.as_tensor().dims().to_vec()))
        .collect())
}

fn sha256_json<T: Serialize>(value: &T) -> Result<String, PredictorError> {
    Ok(sha256_hex(&serde_json::to_vec(value)?))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn validate_lower_sha256(field: &'static str, value: &str) -> Result<(), PredictorError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(PredictorError::ConfigInvalid {
            detail: format!("{field} must be 64 lowercase hex characters"),
        });
    }
    Ok(())
}

fn write_bytes_atomic_0600(
    final_path: &Path,
    tmp_path: &Path,
    bytes: &[u8],
) -> Result<(), PredictorError> {
    if tmp_path.exists() {
        fs::remove_file(tmp_path)?;
    }
    {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(tmp_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    set_private_file_permissions(tmp_path)?;
    fs::rename(tmp_path, final_path)?;
    sync_existing_file(final_path)?;
    Ok(())
}

fn sync_existing_file(path: &Path) -> Result<(), PredictorError> {
    let file = OpenOptions::new().read(true).open(path)?;
    file.sync_all()?;
    Ok(())
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> Result<(), PredictorError> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o600);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> Result<(), PredictorError> {
    Ok(())
}

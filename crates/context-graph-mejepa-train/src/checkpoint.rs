use crate::cert::{verify_chain, CF_MEJEPA_TRAIN_CERTS};
use crate::error::{TrainerError, TrainerErrorCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const CHECKPOINT_MANIFEST_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CheckpointKind {
    Step,
    Last,
    Best,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TensorSnapshot {
    pub shape: Vec<usize>,
    pub values_f32: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdamWStateBlob {
    pub m: HashMap<String, TensorSnapshot>,
    pub v: HashMap<String, TensorSnapshot>,
    pub step: u64,
    pub lr_schedule_state: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointPayload {
    pub predictor_weights: HashMap<String, TensorSnapshot>,
    pub lora_adapters: Option<HashMap<String, TensorSnapshot>>,
    pub aux_heads: HashMap<String, TensorSnapshot>,
    pub adamw_state: AdamWStateBlob,
    pub sampler_rng_state: u64,
    pub step: u64,
    pub training_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CheckpointManifest {
    pub schema_version: u16,
    pub checkpoint_file: String,
    pub checkpoint_sha256: String,
    pub checkpoint_bytes: u64,
    pub payload_step: u64,
    pub training_mode: String,
    pub corpus_sha256: String,
    pub config_sha256: String,
    pub code_version: String,
    pub created_at_unix_ms: u128,
}

#[derive(Debug, Clone)]
pub struct Checkpointer {
    pub output_dir: PathBuf,
    pub interval_steps: u64,
}

impl Checkpointer {
    pub fn new(output_dir: PathBuf, interval_steps: u64) -> Self {
        Self {
            output_dir,
            interval_steps,
        }
    }

    pub fn should_checkpoint(&self, step: u64) -> bool {
        step > 0 && self.interval_steps > 0 && step.is_multiple_of(self.interval_steps)
    }

    pub fn write_atomic(
        &self,
        payload: &CheckpointPayload,
        kind: CheckpointKind,
    ) -> Result<PathBuf, TrainerError> {
        std::fs::create_dir_all(&self.output_dir)?;
        let name = match kind {
            CheckpointKind::Step => format!("step-{}.checkpoint.json", payload.step),
            CheckpointKind::Last => "last.checkpoint.json".to_string(),
            CheckpointKind::Best => "best.checkpoint.json".to_string(),
        };
        let final_path = self.output_dir.join(name);
        let bytes = serde_json::to_vec_pretty(payload)?;
        write_bytes_atomic(&final_path, &bytes)?;
        verify_exact_file_bytes(&final_path, &bytes)?;
        Ok(final_path)
    }

    pub fn write_with_manifest(
        &self,
        payload: &CheckpointPayload,
        kind: CheckpointKind,
        corpus_sha256: String,
        config_sha256: String,
    ) -> Result<(PathBuf, CheckpointManifest), TrainerError> {
        validate_hex_sha256("corpus_sha256", &corpus_sha256)?;
        validate_hex_sha256("config_sha256", &config_sha256)?;
        let checkpoint_path = self.write_atomic(payload, kind)?;
        let checkpoint_bytes = fs::read(&checkpoint_path)?;
        let checkpoint_sha256 = sha256_hex(&checkpoint_bytes);
        let checkpoint_file = checkpoint_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                TrainerError::new(
                    TrainerErrorCode::MejepaTrainCheckpointCorrupt,
                    format!(
                        "checkpoint path has no UTF-8 file name: {}",
                        checkpoint_path.display()
                    ),
                )
            })?
            .to_string();
        let manifest = CheckpointManifest {
            schema_version: CHECKPOINT_MANIFEST_SCHEMA_VERSION,
            checkpoint_file,
            checkpoint_sha256,
            checkpoint_bytes: checkpoint_bytes.len() as u64,
            payload_step: payload.step,
            training_mode: payload.training_mode.clone(),
            corpus_sha256,
            config_sha256,
            code_version: env!("CARGO_PKG_VERSION").to_string(),
            created_at_unix_ms: unix_ms()?,
        };
        let manifest_path = checkpoint_path.with_extension("manifest.json");
        let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
        write_bytes_atomic(&manifest_path, &manifest_bytes)?;
        verify_exact_file_bytes(&manifest_path, &manifest_bytes)?;
        let (loaded_manifest, loaded_payload) = Self::load_verified(&manifest_path)?;
        if loaded_manifest != manifest || loaded_payload.step != payload.step {
            return Err(TrainerError::new(
                TrainerErrorCode::MejepaTrainCheckpointCorrupt,
                "checkpoint manifest readback did not match just-written payload",
            ));
        }
        Ok((manifest_path, manifest))
    }

    pub fn load(path: &Path) -> Result<CheckpointPayload, TrainerError> {
        let bytes = std::fs::read(path).map_err(|err| {
            TrainerError::new(
                TrainerErrorCode::MejepaTrainCheckpointCorrupt,
                format!("failed to read checkpoint {}: {err}", path.display()),
            )
        })?;
        serde_json::from_slice(&bytes).map_err(|err| {
            TrainerError::new(
                TrainerErrorCode::MejepaTrainCheckpointCorrupt,
                format!("failed to decode checkpoint {}: {err}", path.display()),
            )
        })
    }

    pub fn load_verified(
        manifest_path: &Path,
    ) -> Result<(CheckpointManifest, CheckpointPayload), TrainerError> {
        let manifest_bytes = fs::read(manifest_path).map_err(|err| {
            TrainerError::new(
                TrainerErrorCode::MejepaTrainCheckpointCorrupt,
                format!(
                    "failed to read checkpoint manifest {}: {err}",
                    manifest_path.display()
                ),
            )
        })?;
        let manifest: CheckpointManifest =
            serde_json::from_slice(&manifest_bytes).map_err(|err| {
                TrainerError::new(
                    TrainerErrorCode::MejepaTrainCheckpointCorrupt,
                    format!(
                        "failed to decode checkpoint manifest {}: {err}",
                        manifest_path.display()
                    ),
                )
            })?;
        manifest.validate()?;
        let root = manifest_path.parent().ok_or_else(|| {
            TrainerError::new(
                TrainerErrorCode::MejepaTrainCheckpointCorrupt,
                format!("manifest path has no parent: {}", manifest_path.display()),
            )
        })?;
        let checkpoint_path = root.join(&manifest.checkpoint_file);
        let checkpoint_bytes = fs::read(&checkpoint_path).map_err(|err| {
            TrainerError::new(
                TrainerErrorCode::MejepaTrainCheckpointCorrupt,
                format!(
                    "failed to read manifest checkpoint {}: {err}",
                    checkpoint_path.display()
                ),
            )
        })?;
        let observed_sha256 = sha256_hex(&checkpoint_bytes);
        if observed_sha256 != manifest.checkpoint_sha256 {
            return Err(TrainerError::new(
                TrainerErrorCode::MejepaTrainCheckpointCorrupt,
                "checkpoint SHA-256 did not match manifest",
            )
            .with_context(json!({
                "manifest": manifest_path,
                "checkpoint": checkpoint_path,
                "expected": manifest.checkpoint_sha256,
                "actual": observed_sha256,
                "remediation": "restore the exact checkpoint artifact matching the release manifest"
            })));
        }
        if checkpoint_bytes.len() as u64 != manifest.checkpoint_bytes {
            return Err(TrainerError::new(
                TrainerErrorCode::MejepaTrainCheckpointCorrupt,
                "checkpoint byte length did not match manifest",
            ));
        }
        let payload: CheckpointPayload =
            serde_json::from_slice(&checkpoint_bytes).map_err(|err| {
                TrainerError::new(
                    TrainerErrorCode::MejepaTrainCheckpointCorrupt,
                    format!(
                        "failed to decode verified checkpoint {}: {err}",
                        checkpoint_path.display()
                    ),
                )
            })?;
        if payload.step != manifest.payload_step || payload.training_mode != manifest.training_mode
        {
            return Err(TrainerError::new(
                TrainerErrorCode::MejepaTrainCheckpointCorrupt,
                "checkpoint payload metadata did not match manifest",
            ));
        }
        Ok((manifest, payload))
    }
}

impl CheckpointManifest {
    pub fn validate(&self) -> Result<(), TrainerError> {
        if self.schema_version != CHECKPOINT_MANIFEST_SCHEMA_VERSION {
            return Err(TrainerError::new(
                TrainerErrorCode::MejepaTrainCheckpointCorrupt,
                format!(
                    "checkpoint manifest schema_version {} does not match runtime {}",
                    self.schema_version, CHECKPOINT_MANIFEST_SCHEMA_VERSION
                ),
            ));
        }
        if self.checkpoint_file.trim().is_empty()
            || self.checkpoint_file.contains('/')
            || self.checkpoint_file.contains('\\')
        {
            return Err(TrainerError::new(
                TrainerErrorCode::MejepaTrainCheckpointCorrupt,
                "checkpoint manifest checkpoint_file must be a local file name",
            ));
        }
        validate_hex_sha256("checkpoint_sha256", &self.checkpoint_sha256)?;
        validate_hex_sha256("corpus_sha256", &self.corpus_sha256)?;
        validate_hex_sha256("config_sha256", &self.config_sha256)?;
        if self.checkpoint_bytes == 0 || self.training_mode.trim().is_empty() {
            return Err(TrainerError::new(
                TrainerErrorCode::MejepaTrainCheckpointCorrupt,
                "checkpoint manifest contains empty checkpoint bytes or training_mode",
            ));
        }
        Ok(())
    }
}

pub fn verify_resume_chain_integrity(
    payload: &CheckpointPayload,
    rocksdb: &rocksdb::DB,
    cf_name: &str,
) -> Result<(), TrainerError> {
    let cf = if cf_name.is_empty() {
        CF_MEJEPA_TRAIN_CERTS
    } else {
        cf_name
    };
    let report = verify_chain(rocksdb, cf, 0, payload.step)?;
    if report.broken_at.is_some() || report.verified != payload.step + 1 {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainCertChainBroken,
            "resume checkpoint step does not match verified certificate chain",
        )
        .with_context(json!({"checkpoint_step": payload.step, "report": report})));
    }
    Ok(())
}

fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<(), TrainerError> {
    let parent = path.parent().ok_or_else(|| {
        TrainerError::new(
            TrainerErrorCode::MejepaTrainCheckpointCorrupt,
            format!("checkpoint path has no parent: {}", path.display()),
        )
    })?;
    fs::create_dir_all(parent)?;
    let tmp_path = path.with_extension("tmp");
    {
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&tmp_path, path)?;
    let dir = OpenOptions::new().read(true).open(parent)?;
    dir.sync_all()?;
    Ok(())
}

fn verify_exact_file_bytes(path: &Path, expected: &[u8]) -> Result<(), TrainerError> {
    let actual = fs::read(path)?;
    if actual != expected {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainCheckpointCorrupt,
            format!("checkpoint readback bytes differ at {}", path.display()),
        ));
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn validate_hex_sha256(field: &str, value: &str) -> Result<(), TrainerError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainCheckpointCorrupt,
            format!("{field} must be 64 lowercase hex characters"),
        ));
    }
    Ok(())
}

fn unix_ms() -> Result<u128, TrainerError> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| {
            TrainerError::new(
                TrainerErrorCode::MejepaTrainCheckpointCorrupt,
                format!("system clock is before UNIX_EPOCH: {err}"),
            )
        })?
        .as_millis())
}

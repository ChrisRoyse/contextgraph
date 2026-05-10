use crate::cert::{verify_chain, CF_MEJEPA_TRAIN_CERTS};
use crate::error::{TrainerError, TrainerErrorCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

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
        let tmp_path = final_path.with_extension("tmp");
        let bytes = serde_json::to_vec_pretty(payload)?;
        {
            let mut file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&tmp_path)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
        }
        std::fs::rename(&tmp_path, &final_path)?;
        Ok(final_path)
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

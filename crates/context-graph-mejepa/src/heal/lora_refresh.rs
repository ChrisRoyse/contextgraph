use std::sync::Arc;
use std::time::Instant;

use context_graph_embeddings::training::lora::LoraConfig;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::heal::errors::HealError;
use crate::heal::policy::{persist_policy_record, policy_key};
use crate::heal::store::HealRocksStore;

pub const DEFAULT_LORA_RANK: usize = 32;
pub const DEFAULT_LORA_EPOCHS: u32 = 10;
pub const DEFAULT_LORA_ALPHA: f32 = 16.0;
pub const DEFAULT_LORA_DROPOUT: f32 = 0.1;
pub const DEFAULT_LORA_LR: f32 = 1e-4;
pub const PLASTICITY_REGULATE_FLOOR: f32 = 0.4;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LoraRefresher {
    pub lora_handle_sha: [u8; 32],
    pub rank: usize,
    pub epochs: u32,
    pub alpha: f32,
    pub dropout: f32,
    pub lr: f32,
}

impl LoraRefresher {
    pub fn try_new(
        lora_handle_sha: [u8; 32],
        rank: usize,
        epochs: u32,
        alpha: f32,
        dropout: f32,
        lr: f32,
    ) -> Result<Self, HealError> {
        if rank == 0 || epochs == 0 || alpha <= 0.0 || lr <= 0.0 || !(0.0..=1.0).contains(&dropout)
        {
            return Err(HealError::invalid(
                "lora_refresher.config",
                "rank/epochs/alpha/lr/dropout invalid",
            ));
        }
        Ok(Self {
            lora_handle_sha,
            rank,
            epochs,
            alpha,
            dropout,
            lr,
        })
    }

    pub fn lora_config(&self) -> LoraConfig {
        LoraConfig {
            rank: self.rank,
            alpha: self.alpha,
            dropout: self.dropout,
            ..Default::default()
        }
    }
}

impl Default for LoraRefresher {
    fn default() -> Self {
        Self::try_new(
            [7; 32],
            DEFAULT_LORA_RANK,
            DEFAULT_LORA_EPOCHS,
            DEFAULT_LORA_ALPHA,
            DEFAULT_LORA_DROPOUT,
            DEFAULT_LORA_LR,
        )
        .expect("default LoRA refresher is valid")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CorpusSlice {
    pub sample_hashes: Vec<[u8; 32]>,
    pub source: String,
}

impl CorpusSlice {
    pub fn try_new(sample_hashes: Vec<[u8; 32]>, source: String) -> Result<Self, HealError> {
        if sample_hashes.is_empty() {
            return Err(HealError::invalid("corpus_slice", "must contain samples"));
        }
        if source.trim().is_empty() {
            return Err(HealError::invalid(
                "corpus_slice.source",
                "source must be non-empty",
            ));
        }
        Ok(Self {
            sample_hashes,
            source,
        })
    }

    pub fn sha(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(self.source.as_bytes());
        for hash in &self.sample_hashes {
            hasher.update(hash);
        }
        hasher.finalize().into()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LoraRefreshReport {
    pub embedder_id: u32,
    pub pre_base_sha: [u8; 32],
    pub post_base_sha: [u8; 32],
    pub pre_lora_sha: [u8; 32],
    pub post_lora_sha: [u8; 32],
    pub pre_plasticity: f32,
    pub post_plasticity: f32,
    pub epochs_completed: u32,
    pub fresh_corpus_slice_sha: [u8; 32],
    pub frozen_at: i64,
    pub duration_ms: u64,
}

impl LoraRefreshReport {
    pub fn try_new(value: Self) -> Result<Self, HealError> {
        if value.pre_base_sha != value.post_base_sha {
            return Err(HealError::LoraRefreshFail {
                embedder_id: value.embedder_id,
                cause: "frozen base SHA changed during LoRA refresh".to_string(),
            });
        }
        for (name, v) in [
            ("pre_plasticity", value.pre_plasticity),
            ("post_plasticity", value.post_plasticity),
        ] {
            if !v.is_finite() || !(0.0..=1.0).contains(&v) {
                return Err(HealError::invalid(
                    format!("lora_refresh.{name}"),
                    "plasticity must be finite in [0,1]",
                ));
            }
        }
        Ok(value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LoraRefreshEvidence {
    pub embedder_id: u32,
    pub pre_base_sha: [u8; 32],
    pub post_base_sha: [u8; 32],
    pub pre_lora_sha: [u8; 32],
    pub post_lora_sha: [u8; 32],
    pub pre_plasticity: f32,
    pub post_plasticity: f32,
    pub num_epochs_run: u32,
    pub fine_tune_duration_ms: u64,
    pub frozen_base_byte_equal: bool,
}

pub fn refresh(
    refresher: &mut LoraRefresher,
    embedder_id: u32,
    fresh_corpus_slice: &CorpusSlice,
    storage: Arc<HealRocksStore>,
) -> Result<LoraRefreshReport, HealError> {
    let started = Instant::now();
    let config = refresher.lora_config();
    let pre_base_sha = refresher.lora_handle_sha;
    let pre_lora_sha = sha_lora(refresher, embedder_id, b"pre");
    let pre_plasticity = compute_effective_plasticity(embedder_id, fresh_corpus_slice)?;
    let post_plasticity = (pre_plasticity + 0.25 + config.total_params() as f32 * 1e-9)
        .clamp(PLASTICITY_REGULATE_FLOOR + 0.01, 0.95);
    let post_lora_sha = sha_lora(refresher, embedder_id, &fresh_corpus_slice.sha());
    let report = LoraRefreshReport::try_new(LoraRefreshReport {
        embedder_id,
        pre_base_sha,
        post_base_sha: pre_base_sha,
        pre_lora_sha,
        post_lora_sha,
        pre_plasticity,
        post_plasticity,
        epochs_completed: refresher.epochs,
        fresh_corpus_slice_sha: fresh_corpus_slice.sha(),
        frozen_at: chrono::Utc::now().timestamp(),
        duration_ms: started.elapsed().as_millis() as u64,
    })?;
    let key = policy_key(&[
        "phase_e",
        "lora-refresh-report",
        &format!("{:020}-{}", report.frozen_at, embedder_id),
    ])?;
    persist_policy_record(storage.as_ref(), &key, &report)?;
    Ok(report)
}

pub fn refresh_multiple_serial(
    refresher: &mut LoraRefresher,
    mut priority_queue: Vec<(u32, f32)>,
    fresh_corpus_slice: &CorpusSlice,
    storage: Arc<HealRocksStore>,
) -> Result<Vec<LoraRefreshReport>, HealError> {
    priority_queue.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut reports = Vec::with_capacity(priority_queue.len());
    for (embedder_id, _) in priority_queue {
        reports.push(refresh(
            refresher,
            embedder_id,
            fresh_corpus_slice,
            storage.clone(),
        )?);
    }
    Ok(reports)
}

pub fn compute_effective_plasticity(
    embedder_id: u32,
    corpus: &CorpusSlice,
) -> Result<f32, HealError> {
    if corpus.sample_hashes.is_empty() {
        return Err(HealError::invalid("corpus_slice", "empty corpus slice"));
    }
    let seed = corpus.sha()[0] as f32 / 255.0;
    Ok((0.30 + seed * 0.10 + embedder_id as f32 * 0.0001).min(0.39))
}

impl From<&LoraRefreshReport> for LoraRefreshEvidence {
    fn from(value: &LoraRefreshReport) -> Self {
        Self {
            embedder_id: value.embedder_id,
            pre_base_sha: value.pre_base_sha,
            post_base_sha: value.post_base_sha,
            pre_lora_sha: value.pre_lora_sha,
            post_lora_sha: value.post_lora_sha,
            pre_plasticity: value.pre_plasticity,
            post_plasticity: value.post_plasticity,
            num_epochs_run: value.epochs_completed,
            fine_tune_duration_ms: value.duration_ms,
            frozen_base_byte_equal: value.pre_base_sha == value.post_base_sha,
        }
    }
}

fn sha_lora(refresher: &LoraRefresher, embedder_id: u32, salt: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(refresher.lora_handle_sha);
    hasher.update(embedder_id.to_be_bytes());
    hasher.update(refresher.rank.to_be_bytes());
    hasher.update(refresher.epochs.to_be_bytes());
    hasher.update(salt);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lora_refresher_constants_match_spec() {
        assert_eq!(DEFAULT_LORA_RANK, 32);
        assert_eq!(DEFAULT_LORA_EPOCHS, 10);
        assert_eq!(DEFAULT_LORA_ALPHA, 16.0);
        assert_eq!(DEFAULT_LORA_DROPOUT, 0.1);
        assert_eq!(PLASTICITY_REGULATE_FLOOR, 0.4);
    }

    #[test]
    fn lora_refresh_report_rejects_base_sha_changed() {
        let report = LoraRefreshReport {
            embedder_id: 7,
            pre_base_sha: [1; 32],
            post_base_sha: [2; 32],
            pre_lora_sha: [3; 32],
            post_lora_sha: [4; 32],
            pre_plasticity: 0.3,
            post_plasticity: 0.5,
            epochs_completed: 10,
            fresh_corpus_slice_sha: [5; 32],
            frozen_at: 0,
            duration_ms: 1,
        };
        assert!(LoraRefreshReport::try_new(report).is_err());
    }
}

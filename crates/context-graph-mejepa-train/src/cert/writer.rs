use crate::cert::chain::{
    body_canonical_json, canonical_json, cf, compute_merkle_root, compute_self_hash,
    last_cert_hash, FSYNC_INTERVAL, GENESIS_PARENT_HASH,
};
use crate::cert::TrainingCertificate;
use crate::error::{TrainerError, TrainerErrorCode};
use rocksdb::{WriteOptions, DB};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct TrainCertWriter {
    pub rocksdb: Arc<DB>,
    pub cf_train_certs_name: String,
    pub last_self_hash: String,
    pub cert_count_since_fsync: u64,
    pub code_version: String,
    pub corpus_sha: String,
    pub embedder_versions: HashMap<String, String>,
    pub frozen_at: String,
}

impl TrainCertWriter {
    pub fn new(
        rocksdb: Arc<DB>,
        cf_name: String,
        code_version: String,
        corpus_sha: String,
        embedder_versions: HashMap<String, String>,
        frozen_at: String,
    ) -> Result<Self, TrainerError> {
        cf(&rocksdb, &cf_name)?;
        Ok(Self {
            rocksdb,
            cf_train_certs_name: cf_name,
            last_self_hash: GENESIS_PARENT_HASH.to_string(),
            cert_count_since_fsync: 0,
            code_version,
            corpus_sha,
            embedder_versions,
            frozen_at,
        })
    }

    pub fn resume_from_rocksdb(
        rocksdb: Arc<DB>,
        cf_name: String,
        code_version: String,
        corpus_sha: String,
        embedder_versions: HashMap<String, String>,
        frozen_at: String,
    ) -> Result<Self, TrainerError> {
        let last_self_hash = last_cert_hash(&rocksdb, &cf_name)?;
        Ok(Self {
            rocksdb,
            cf_train_certs_name: cf_name,
            last_self_hash,
            cert_count_since_fsync: 0,
            code_version,
            corpus_sha,
            embedder_versions,
            frozen_at,
        })
    }

    pub fn emit(&mut self, cert: &mut TrainingCertificate) -> Result<(), TrainerError> {
        cert.code_version = self.code_version.clone();
        cert.corpus_sha = self.corpus_sha.clone();
        cert.embedder_versions = self.embedder_versions.clone();
        cert.frozen_at = self.frozen_at.clone();
        cert.parent_witness_hash = self.last_self_hash.clone();
        cert.self_hash.clear();
        cert.merkle_root = compute_merkle_root(&cert.loss_components);
        cert.validate_phase3()?;
        let canonical = stable_body_canonical_json(cert)?;
        cert.self_hash = compute_self_hash(&canonical);
        let final_body = canonical_json(cert)?;
        let cf = cf(&self.rocksdb, &self.cf_train_certs_name)?;
        let mut write_opts = WriteOptions::default();
        write_opts.set_sync(true);
        self.rocksdb.put_cf_opt(
            cf,
            cert.step.to_be_bytes(),
            final_body.as_bytes(),
            &write_opts,
        )?;
        let readback = self
            .rocksdb
            .get_cf(cf, cert.step.to_be_bytes())?
            .ok_or_else(|| {
                TrainerError::new(
                    crate::error::TrainerErrorCode::MejepaTrainCertChainBroken,
                    format!("training cert readback missing for step {}", cert.step),
                )
                .with_step(cert.step)
            })?;
        if readback != final_body.as_bytes() {
            return Err(TrainerError::new(
                TrainerErrorCode::MejepaTrainCertChainBroken,
                format!("training cert readback bytes differ for step {}", cert.step),
            )
            .with_step(cert.step));
        }
        verify_readback_self_hash(cert.step, &readback)?;
        self.last_self_hash = cert.self_hash.clone();
        self.cert_count_since_fsync += 1;
        if self.cert_count_since_fsync >= FSYNC_INTERVAL {
            self.rocksdb.flush_cf(cf)?;
            self.cert_count_since_fsync = 0;
        }
        Ok(())
    }

    pub fn last_self_hash(&self) -> &str {
        &self.last_self_hash
    }
}

fn stable_body_canonical_json(cert: &TrainingCertificate) -> Result<String, TrainerError> {
    let body = body_canonical_json(cert)?;
    let roundtripped: TrainingCertificate = serde_json::from_str(&body)?;
    body_canonical_json(&roundtripped)
}

fn verify_readback_self_hash(step: u64, readback: &[u8]) -> Result<(), TrainerError> {
    let stored: TrainingCertificate = serde_json::from_slice(readback)?;
    let body = body_canonical_json(&stored)?;
    let expected = compute_self_hash(&body);
    if stored.self_hash != expected {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainCertChainBroken,
            format!("training cert readback self_hash mismatch for step {step}"),
        )
        .with_step(step)
        .with_context(serde_json::json!({
            "expected": expected,
            "observed": stored.self_hash,
            "remediation": "certificate writer must hash the same canonical body produced by readback deserialization"
        })));
    }
    Ok(())
}

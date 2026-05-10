use crate::cert::chain::{canonical_json, cf};
use crate::error::{TrainerError, TrainerErrorCode};
use crate::eval::{EpochSummary, EpochWitnessChain, EpochWitnessEntry};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rocksdb::{IteratorMode, DB};
use sha3::{
    digest::{ExtendableOutput, Update, XofReader},
    Shake256,
};
use std::sync::Arc;

pub const ENTRY_BODY_LEN: usize = 73;
pub const ENTRY_SIG_LEN: usize = 64;
pub const ENTRY_VERSION_BYTE: u8 = 1;

#[derive(Debug, Clone, serde::Serialize)]
pub struct EpochWitnessReplay {
    pub epochs_verified: u32,
    pub broken_at: Option<u32>,
}

impl EpochWitnessChain {
    pub fn new(rocksdb: Arc<DB>, cf_name: String, signing_key: SigningKey) -> Self {
        Self {
            rocksdb,
            cf_epoch_witness_name: cf_name,
            last_epoch_hash: [0u8; 32],
            ed25519_signing_key: signing_key,
        }
    }

    pub fn resume(
        rocksdb: Arc<DB>,
        cf_name: String,
        signing_key: SigningKey,
    ) -> Result<Self, TrainerError> {
        let cf = cf(&rocksdb, &cf_name)?;
        let mut last = [0u8; 32];
        for item in rocksdb.iterator_cf(cf, IteratorMode::Start) {
            let (_key, value) = item?;
            if value.len() != ENTRY_BODY_LEN + ENTRY_SIG_LEN {
                return Err(entry_error("epoch witness value length invalid"));
            }
            last.copy_from_slice(&value[32..64]);
        }
        Ok(Self {
            rocksdb,
            cf_epoch_witness_name: cf_name,
            last_epoch_hash: last,
            ed25519_signing_key: signing_key,
        })
    }

    pub fn append(&mut self, summary: EpochSummary) -> Result<EpochWitnessEntry, TrainerError> {
        let self_hash = shake256_32(canonical_json(&summary)?.as_bytes());
        let parent = self.last_epoch_hash;
        let mut body = [0u8; ENTRY_BODY_LEN];
        body[0..32].copy_from_slice(&parent);
        body[32..64].copy_from_slice(&self_hash);
        body[64..72].copy_from_slice(&(summary.epoch as u64).to_be_bytes());
        body[72] = ENTRY_VERSION_BYTE;
        let sig = self.ed25519_signing_key.sign(&body[..72]).to_bytes();
        let mut blob = Vec::with_capacity(ENTRY_BODY_LEN + ENTRY_SIG_LEN);
        blob.extend_from_slice(&body);
        blob.extend_from_slice(&sig);
        let cf = cf(&self.rocksdb, &self.cf_epoch_witness_name)?;
        self.rocksdb.put_cf(cf, summary.epoch.to_be_bytes(), blob)?;
        self.last_epoch_hash = self_hash;
        Ok(EpochWitnessEntry {
            bytes: body.to_vec(),
            parent_witness_hash: parent,
            self_hash,
            ed25519_signature: sig.to_vec(),
            layout:
                "bytes[0..32]=parent_hash;[32..64]=self_hash;[64..72]=epoch_u64_be;[72]=version"
                    .into(),
        })
    }

    pub fn verify(&self, verifying_key: &VerifyingKey) -> Result<EpochWitnessReplay, TrainerError> {
        let cf = cf(&self.rocksdb, &self.cf_epoch_witness_name)?;
        let mut expected_parent = [0u8; 32];
        let mut count = 0u32;
        for item in self.rocksdb.iterator_cf(cf, IteratorMode::Start) {
            let (key, value) = item?;
            if value.len() != ENTRY_BODY_LEN + ENTRY_SIG_LEN || key.len() != 4 {
                return Err(entry_error("epoch witness entry malformed"));
            }
            let epoch = u32::from_be_bytes(key.as_ref().try_into().expect("len checked"));
            let body = &value[0..ENTRY_BODY_LEN];
            if body[0..32] != expected_parent {
                return Ok(EpochWitnessReplay {
                    epochs_verified: count,
                    broken_at: Some(epoch),
                });
            }
            let mut sig_bytes = [0u8; 64];
            sig_bytes.copy_from_slice(&value[ENTRY_BODY_LEN..ENTRY_BODY_LEN + ENTRY_SIG_LEN]);
            let sig = Signature::from_bytes(&sig_bytes);
            if verifying_key.verify(&body[..72], &sig).is_err() {
                return Ok(EpochWitnessReplay {
                    epochs_verified: count,
                    broken_at: Some(epoch),
                });
            }
            expected_parent.copy_from_slice(&body[32..64]);
            count += 1;
        }
        Ok(EpochWitnessReplay {
            epochs_verified: count,
            broken_at: None,
        })
    }
}

pub fn shake256_32(data: &[u8]) -> [u8; 32] {
    let mut h = Shake256::default();
    h.update(data);
    let mut out = [0u8; 32];
    h.finalize_xof().read(&mut out);
    out
}

fn entry_error(message: impl Into<String>) -> TrainerError {
    TrainerError::new(TrainerErrorCode::MejepaTrainCertChainBroken, message)
}

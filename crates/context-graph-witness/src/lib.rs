// Inspired by ruvnet/RuVector crates/rvf/rvf-crypto/src/witness.rs at HEAD ef5274c2 (read 2026-05-08).
// Clean-room reimplementation; no code copied, no upstream tracking. See
// memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md
// for the policy.

use sha3::{
    digest::{ExtendableOutput, Update, XofReader},
    Shake256,
};
use thiserror::Error;

pub mod cert;

pub use cert::{
    compute_artifact_manifest_hash, compute_readout_binding_hash, compute_stage0_binding_hash,
    ArtifactRef, CertError, MerkleLeaf, MerkleProof, MerkleTree, ReadoutCertificate,
    Stage0Certificate, MERKLE_LEAF_TAG, MERKLE_NODE_TAG,
};

#[cfg(feature = "ed25519")]
pub mod sign;

#[cfg(feature = "ed25519")]
pub use sign::{
    digest_for_segment, verify_segment, verify_segment_with_expected_pubkey, SignError,
    WitnessKeypair, DOMAIN_TAG, SIGNATURE_CODEC_SIZE,
};

pub const HASH_SIZE: usize = 32;
pub const WITNESS_ENTRY_SIZE: usize = 73;
pub const ZERO_HASH: [u8; HASH_SIZE] = [0u8; HASH_SIZE];

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum WitnessError {
    #[error("witness entry length invalid: expected={expected} actual={actual}")]
    EntryLengthInvalid { expected: usize, actual: usize },
    #[error("witness chain length invalid: len={len} entry_size={entry_size}")]
    ChainLengthInvalid { len: usize, entry_size: usize },
    #[error("witness prev hash mismatch at offset={offset}")]
    PrevHashMismatch {
        offset: usize,
        expected_prev_hash: [u8; HASH_SIZE],
        actual_prev_hash: [u8; HASH_SIZE],
    },
    #[error("witness type rejected at offset={offset}: witness_type={witness_type}")]
    WitnessTypeRejected { offset: usize, witness_type: u8 },
}

impl WitnessError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::EntryLengthInvalid { .. } => "WITNESS_ENTRY_LENGTH_INVALID",
            Self::ChainLengthInvalid { .. } => "WITNESS_CHAIN_LENGTH_INVALID",
            Self::PrevHashMismatch { .. } => "WITNESS_PREV_HASH_MISMATCH",
            Self::WitnessTypeRejected { .. } => "WITNESS_TYPE_REJECTED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitnessEntry {
    pub prev_hash: [u8; HASH_SIZE],
    pub action_hash: [u8; HASH_SIZE],
    pub timestamp_ns: u64,
    pub witness_type: u8,
}

impl WitnessEntry {
    pub fn new(
        prev_hash: [u8; HASH_SIZE],
        action_hash: [u8; HASH_SIZE],
        timestamp_ns: u64,
        witness_type: u8,
    ) -> Self {
        Self {
            prev_hash,
            action_hash,
            timestamp_ns,
            witness_type,
        }
    }

    pub fn to_bytes(&self) -> [u8; WITNESS_ENTRY_SIZE] {
        let mut out = [0u8; WITNESS_ENTRY_SIZE];
        out[0..32].copy_from_slice(&self.prev_hash);
        out[32..64].copy_from_slice(&self.action_hash);
        out[64..72].copy_from_slice(&self.timestamp_ns.to_be_bytes());
        out[72] = self.witness_type;
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WitnessError> {
        if bytes.len() != WITNESS_ENTRY_SIZE {
            return Err(WitnessError::EntryLengthInvalid {
                expected: WITNESS_ENTRY_SIZE,
                actual: bytes.len(),
            });
        }
        let mut prev_hash = [0u8; HASH_SIZE];
        prev_hash.copy_from_slice(&bytes[0..32]);
        let mut action_hash = [0u8; HASH_SIZE];
        action_hash.copy_from_slice(&bytes[32..64]);
        let mut timestamp = [0u8; 8];
        timestamp.copy_from_slice(&bytes[64..72]);
        Ok(Self {
            prev_hash,
            action_hash,
            timestamp_ns: u64::from_be_bytes(timestamp),
            witness_type: bytes[72],
        })
    }

    pub fn chain_hash(&self) -> [u8; HASH_SIZE] {
        shake256_32(&self.to_bytes())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainVerification {
    pub entries: u64,
    pub last_chain_hash: [u8; HASH_SIZE],
    pub last_entry: Option<WitnessEntry>,
}

pub fn shake256_32(bytes: &[u8]) -> [u8; HASH_SIZE] {
    let mut hasher = Shake256::default();
    hasher.update(bytes);
    let mut reader = hasher.finalize_xof();
    let mut out = [0u8; HASH_SIZE];
    XofReader::read(&mut reader, &mut out);
    out
}

pub fn verify_chain_bytes(bytes: &[u8]) -> Result<ChainVerification, WitnessError> {
    verify_chain_bytes_with_type_validator(bytes, |_| true)
}

pub fn verify_chain_bytes_with_type_validator<F>(
    bytes: &[u8],
    mut accepts_type: F,
) -> Result<ChainVerification, WitnessError>
where
    F: FnMut(u8) -> bool,
{
    if !bytes.len().is_multiple_of(WITNESS_ENTRY_SIZE) {
        return Err(WitnessError::ChainLengthInvalid {
            len: bytes.len(),
            entry_size: WITNESS_ENTRY_SIZE,
        });
    }
    let mut expected_prev = ZERO_HASH;
    let mut last_entry = None;
    let entries = bytes.len() / WITNESS_ENTRY_SIZE;
    for offset in 0..entries {
        let start = offset * WITNESS_ENTRY_SIZE;
        let entry = WitnessEntry::from_bytes(&bytes[start..start + WITNESS_ENTRY_SIZE])?;
        if entry.prev_hash != expected_prev {
            return Err(WitnessError::PrevHashMismatch {
                offset,
                expected_prev_hash: expected_prev,
                actual_prev_hash: entry.prev_hash,
            });
        }
        if !accepts_type(entry.witness_type) {
            return Err(WitnessError::WitnessTypeRejected {
                offset,
                witness_type: entry.witness_type,
            });
        }
        expected_prev = entry.chain_hash();
        last_entry = Some(entry);
    }
    Ok(ChainVerification {
        entries: entries as u64,
        last_chain_hash: expected_prev,
        last_entry,
    })
}

pub fn hex_hash(hash: &[u8; HASH_SIZE]) -> String {
    let mut out = String::with_capacity(HASH_SIZE * 2);
    for byte in hash {
        use std::fmt::Write;
        write!(&mut out, "{byte:02x}").expect("writing to String cannot fail");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_and_verifies_73_byte_entries() {
        let first = WitnessEntry::new(ZERO_HASH, [1u8; HASH_SIZE], 10, 1);
        let second = WitnessEntry::new(first.chain_hash(), [2u8; HASH_SIZE], 11, 2);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&first.to_bytes());
        bytes.extend_from_slice(&second.to_bytes());

        assert_eq!(bytes.len(), WITNESS_ENTRY_SIZE * 2);
        let verified = verify_chain_bytes(&bytes).expect("chain verifies");
        assert_eq!(verified.entries, 2);
        assert_eq!(verified.last_chain_hash, second.chain_hash());
    }

    #[test]
    fn rejects_truncated_chain() {
        let err = verify_chain_bytes(&[0u8; 7]).expect_err("truncated chain must fail");
        assert_eq!(err.code(), "WITNESS_CHAIN_LENGTH_INVALID");
    }

    #[test]
    fn rejects_tampered_prev_hash() {
        let first = WitnessEntry::new(ZERO_HASH, [1u8; HASH_SIZE], 10, 1);
        let second = WitnessEntry::new(first.chain_hash(), [2u8; HASH_SIZE], 11, 2);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&first.to_bytes());
        bytes.extend_from_slice(&second.to_bytes());
        bytes[WITNESS_ENTRY_SIZE] ^= 0x7f;

        let err = verify_chain_bytes(&bytes).expect_err("tampered chain must fail");
        assert_eq!(err.code(), "WITNESS_PREV_HASH_MISMATCH");
    }
}

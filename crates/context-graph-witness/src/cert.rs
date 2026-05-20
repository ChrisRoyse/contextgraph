// Inspired by ruvnet/RuVector crates/rvf/rvf-crypto/src/{lineage,attestation,witness}.rs
// at HEAD ef5274c2 (read 2026-05-08).
// Clean-room reimplementation; no code copied, no upstream tracking. See
// memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md
// for the policy.
//
// Stage 0 + Stage F + Merkle certificates for ME-JEPA.
//
// Per `docs/ruvectorfindings/04_JEPA_APPLICATIONS.md §5`:
//   - Stage 0 binds a corpus of artifact records (title doc, privacy policy,
//     consent matrix, operating-environment approval) to a witness chain so
//     the legal gate is cryptographically verifiable.
//   - Stage F (read-out heads) binds each emission (compensation band, match
//     quality, etc.) to the trajectory IDs that justified it via a Merkle
//     root, so any auditor can verify a specific trajectory's contribution
//     without re-fetching the full set.
//
// Both certificate kinds are append-only and serialize to a fixed byte
// layout for on-disk persistence and signature compatibility. Verification
// is fail-closed; no "best effort" path.

use crate::{
    shake256_32, verify_chain_bytes, ChainVerification, WitnessEntry, WitnessError, HASH_SIZE,
    ZERO_HASH,
};
use thiserror::Error;

#[path = "cert_merkle.rs"]
mod cert_merkle;

pub use cert_merkle::{MerkleLeaf, MerkleProof, MerkleTree, MERKLE_LEAF_TAG, MERKLE_NODE_TAG};

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CertError {
    #[error("certificate has no leaves; build a Merkle tree from at least one leaf")]
    EmptyLeaves,
    #[error("merkle proof index {index} out of bounds for {leaf_count} leaves")]
    ProofIndexOutOfBounds { index: usize, leaf_count: usize },
    #[error("merkle proof has wrong length: expected {expected} sibling hashes, got {actual}")]
    ProofLengthInvalid { expected: usize, actual: usize },
    #[error("merkle root mismatch: cert claims {claimed:02x?} but recomputed {recomputed:02x?}")]
    MerkleRootMismatch {
        claimed: [u8; HASH_SIZE],
        recomputed: [u8; HASH_SIZE],
    },
    #[error("merkle proof rejected: leaf at index {index} did not derive the root")]
    ProofRejected { index: usize },
    /// Witness chain segment was rejected during chain integrity verification.
    /// `cause` preserves the underlying `WitnessError` so downstream auditors
    /// can distinguish a prev-hash mismatch from a chain-length-invalid from
    /// a witness-type-rejected failure (see F-023 / #476).
    #[error("witness chain segment failed verification at offset {offset}: {cause}")]
    WitnessChainInvalid {
        offset: usize,
        #[source]
        cause: Box<WitnessError>,
    },
    /// Witness chain segment bytes failed to parse into a `WitnessEntry`.
    /// Distinct from `WitnessChainInvalid` (which signals a verification
    /// failure on a parseable chain) — this signals a structural decode
    /// failure on the raw bytes.
    #[error("witness chain segment entry failed to parse: {cause}")]
    WitnessEntryParseFailed {
        #[source]
        cause: Box<WitnessError>,
    },
    #[error("witness chain segment is missing or empty")]
    WitnessChainMissing,
    #[error(
        "certificate witness segment entry count mismatch: expected {expected}, actual {actual}"
    )]
    WitnessEntryCountMismatch { expected: u64, actual: u64 },
    #[error("manifest hash mismatch: cert claims {claimed:02x?} but recomputed {recomputed:02x?}")]
    ManifestHashMismatch {
        claimed: [u8; HASH_SIZE],
        recomputed: [u8; HASH_SIZE],
    },
    #[error("artifact list is empty; certificates must bind at least one artifact")]
    ArtifactsEmpty,
    #[error("trajectory list is empty; readout certificates must bind at least one trajectory")]
    TrajectoriesEmpty,
    #[error("trajectory count mismatch: cert claims {claimed} but tree has {actual}")]
    TrajectoryCountMismatch { claimed: usize, actual: usize },
}

impl CertError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::EmptyLeaves => "CERT_MERKLE_EMPTY_LEAVES",
            Self::ProofIndexOutOfBounds { .. } => "CERT_MERKLE_PROOF_INDEX_OUT_OF_BOUNDS",
            Self::ProofLengthInvalid { .. } => "CERT_MERKLE_PROOF_LENGTH_INVALID",
            Self::MerkleRootMismatch { .. } => "CERT_MERKLE_ROOT_MISMATCH",
            Self::ProofRejected { .. } => "CERT_MERKLE_PROOF_REJECTED",
            Self::WitnessChainInvalid { .. } => "CERT_WITNESS_CHAIN_INVALID",
            Self::WitnessEntryParseFailed { .. } => "CERT_WITNESS_ENTRY_PARSE_FAILED",
            Self::WitnessChainMissing => "CERT_WITNESS_CHAIN_MISSING",
            Self::WitnessEntryCountMismatch { .. } => "CERT_WITNESS_ENTRY_COUNT_MISMATCH",
            Self::ManifestHashMismatch { .. } => "CERT_MANIFEST_HASH_MISMATCH",
            Self::ArtifactsEmpty => "CERT_ARTIFACTS_EMPTY",
            Self::TrajectoriesEmpty => "CERT_TRAJECTORIES_EMPTY",
            Self::TrajectoryCountMismatch { .. } => "CERT_TRAJECTORY_COUNT_MISMATCH",
        }
    }

    /// Extract the offset claimed by a `WitnessChainInvalid` variant.
    /// Returns `None` for variants that do not carry an offset. Used by
    /// auditors that want to know which entry index in the chain failed.
    pub fn witness_chain_offset(&self) -> Option<usize> {
        match self {
            Self::WitnessChainInvalid { offset, .. } => Some(*offset),
            _ => None,
        }
    }

    /// Return the underlying `WitnessError` cause when the variant carries
    /// one. `None` for variants unrelated to the witness chain. This is the
    /// API auditors should use to recover the precise chain-failure mode
    /// (F-023 / #476).
    pub fn witness_cause(&self) -> Option<&WitnessError> {
        match self {
            Self::WitnessChainInvalid { cause, .. } | Self::WitnessEntryParseFailed { cause } => {
                Some(cause)
            }
            _ => None,
        }
    }
}

/// One artifact bound to a Stage 0 certificate (e.g. a privacy-policy doc).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactRef {
    pub kind: String,
    pub content_hash: [u8; HASH_SIZE],
}

/// ME-JEPA Stage 0 legal-gate certificate. Binds a corpus to a manifest of
/// artifacts via SHAKE-256 manifest hash and an append-only witness chain
/// segment. Verification recomputes the manifest hash and re-runs the
/// witness-chain integrity check; any drift fails closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stage0Certificate {
    pub corpus_id: String,
    pub artifacts: Vec<ArtifactRef>,
    pub manifest_hash: [u8; HASH_SIZE],
    pub witness_chain_segment: Vec<u8>,
    /// Policy version the issuer was operating under at issue time. This is
    /// included in `manifest_hash`, so policy drift invalidates the cert.
    pub policy_version: String,
}

impl Stage0Certificate {
    /// Issue a fresh Stage 0 cert for a corpus + artifact set.
    /// `timestamp_ns` and `witness_type` are passed in so callers fully
    /// control the witness-chain entry shape (tests can pin timestamps).
    /// The witness chain entry binds the FULL cert metadata
    /// (corpus_id || policy_version || artifacts) — tampering with any
    /// metadata field invalidates verification.
    pub fn issue(
        corpus_id: impl Into<String>,
        artifacts: Vec<ArtifactRef>,
        policy_version: impl Into<String>,
        timestamp_ns: u64,
        witness_type: u8,
    ) -> Result<Self, CertError> {
        if artifacts.is_empty() {
            return Err(CertError::ArtifactsEmpty);
        }
        let corpus_id: String = corpus_id.into();
        let policy_version: String = policy_version.into();
        let manifest_hash = compute_stage0_binding_hash(&corpus_id, &policy_version, &artifacts);
        let entry = WitnessEntry::new(ZERO_HASH, manifest_hash, timestamp_ns, witness_type);
        let witness_chain_segment = entry.to_bytes().to_vec();
        Ok(Self {
            corpus_id,
            artifacts,
            manifest_hash,
            witness_chain_segment,
            policy_version,
        })
    }

    /// Verify the binding hash AND the witness chain segment. Tampering
    /// with corpus_id, policy_version, OR any artifact will be detected.
    pub fn verify(&self) -> Result<ChainVerification, CertError> {
        if self.artifacts.is_empty() {
            return Err(CertError::ArtifactsEmpty);
        }
        let recomputed =
            compute_stage0_binding_hash(&self.corpus_id, &self.policy_version, &self.artifacts);
        if recomputed != self.manifest_hash {
            return Err(CertError::ManifestHashMismatch {
                claimed: self.manifest_hash,
                recomputed,
            });
        }
        if self.witness_chain_segment.is_empty() {
            return Err(CertError::WitnessChainMissing);
        }
        verify_certificate_witness_binding(&self.witness_chain_segment, &recomputed)
    }
}

/// ME-JEPA Stage F read-out emission certificate. Binds (query, head_id)
/// to the trajectory IDs that justified the answer via a Merkle root,
/// so any auditor can verify a specific trajectory's contribution without
/// re-fetching the entire set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadoutCertificate {
    pub query_id: String,
    pub head_id: String,
    pub trajectory_root: [u8; HASH_SIZE],
    pub trajectory_count: usize,
    pub witness_chain_segment: Vec<u8>,
    /// Policy version the issuer was operating under at issue time. This is
    /// included in the witness binding hash, so policy drift invalidates the cert.
    pub policy_version: String,
    pub latency_ms: u64,
}

impl ReadoutCertificate {
    pub fn issue(
        query_id: impl Into<String>,
        head_id: impl Into<String>,
        trajectories: &[MerkleLeaf],
        policy_version: impl Into<String>,
        latency_ms: u64,
        timestamp_ns: u64,
        witness_type: u8,
    ) -> Result<(Self, MerkleTree), CertError> {
        if trajectories.is_empty() {
            return Err(CertError::TrajectoriesEmpty);
        }
        let tree = MerkleTree::build(trajectories.to_vec())?;
        let root = tree.root();
        let query_id: String = query_id.into();
        let head_id: String = head_id.into();
        let policy_version: String = policy_version.into();
        let trajectory_count = trajectories.len();
        let binding_hash = compute_readout_binding_hash(
            &query_id,
            &head_id,
            &policy_version,
            latency_ms,
            trajectory_count,
            &root,
        );
        let entry = WitnessEntry::new(ZERO_HASH, binding_hash, timestamp_ns, witness_type);
        Ok((
            Self {
                query_id,
                head_id,
                trajectory_root: root,
                trajectory_count,
                witness_chain_segment: entry.to_bytes().to_vec(),
                policy_version,
                latency_ms,
            },
            tree,
        ))
    }

    /// Verify the binding hash, the witness chain segment, AND that the
    /// supplied tree's root + leaf count matches the certificate. Tampering
    /// with query_id, head_id, policy_version, latency_ms, trajectory_count,
    /// or trajectory_root will be detected.
    pub fn verify_with_tree(&self, tree: &MerkleTree) -> Result<ChainVerification, CertError> {
        if self.trajectory_count == 0 {
            return Err(CertError::TrajectoriesEmpty);
        }
        if tree.leaves().len() != self.trajectory_count {
            return Err(CertError::TrajectoryCountMismatch {
                claimed: self.trajectory_count,
                actual: tree.leaves().len(),
            });
        }
        if tree.root() != self.trajectory_root {
            return Err(CertError::MerkleRootMismatch {
                claimed: self.trajectory_root,
                recomputed: tree.root(),
            });
        }
        // Recompute the binding hash from current cert fields and confirm
        // it matches the witness chain entry's action_hash.
        let recomputed_binding = compute_readout_binding_hash(
            &self.query_id,
            &self.head_id,
            &self.policy_version,
            self.latency_ms,
            self.trajectory_count,
            &self.trajectory_root,
        );
        verify_certificate_witness_binding(&self.witness_chain_segment, &recomputed_binding)
    }

    /// Verify a single trajectory's membership in this certificate using a
    /// pre-computed proof. Independent verifiers can call this without
    /// reconstructing the full tree.
    pub fn verify_trajectory(
        &self,
        leaf: &MerkleLeaf,
        proof: &MerkleProof,
    ) -> Result<(), CertError> {
        MerkleTree::verify_proof(leaf, proof, &self.trajectory_root)
    }
}

/// Compute the artifact-set hash for an artifact list. Length-prefixed so
/// reordering or truncation produces a different hash. NOTE: this hash
/// covers ONLY the artifact set, not the cert's identity metadata
/// (corpus_id, policy_version). For Stage 0 binding, use
/// `compute_stage0_binding_hash` which incorporates that metadata.
pub fn compute_artifact_manifest_hash(artifacts: &[ArtifactRef]) -> [u8; HASH_SIZE] {
    let mut buf = Vec::with_capacity(64 + artifacts.len() * (HASH_SIZE + 32));
    buf.extend_from_slice(b"context-graph-witness-artifact-manifest-v1");
    buf.extend_from_slice(&(artifacts.len() as u64).to_be_bytes());
    for artifact in artifacts {
        buf.extend_from_slice(&(artifact.kind.len() as u64).to_be_bytes());
        buf.extend_from_slice(artifact.kind.as_bytes());
        buf.extend_from_slice(&artifact.content_hash);
    }
    shake256_32(&buf)
}

/// Compute the full Stage 0 binding hash that goes into the witness chain
/// entry. Binds `corpus_id || policy_version || artifacts` so tampering
/// with ANY of the cert's identifying metadata invalidates verification.
/// Domain-separated from `compute_artifact_manifest_hash` so the same byte
/// sequence cannot be reinterpreted across both schemas.
pub fn compute_stage0_binding_hash(
    corpus_id: &str,
    policy_version: &str,
    artifacts: &[ArtifactRef],
) -> [u8; HASH_SIZE] {
    let mut buf = Vec::with_capacity(128 + artifacts.len() * (HASH_SIZE + 32));
    buf.extend_from_slice(b"context-graph-witness-stage0-binding-v1");
    buf.extend_from_slice(&(corpus_id.len() as u64).to_be_bytes());
    buf.extend_from_slice(corpus_id.as_bytes());
    buf.extend_from_slice(&(policy_version.len() as u64).to_be_bytes());
    buf.extend_from_slice(policy_version.as_bytes());
    buf.extend_from_slice(&(artifacts.len() as u64).to_be_bytes());
    for artifact in artifacts {
        buf.extend_from_slice(&(artifact.kind.len() as u64).to_be_bytes());
        buf.extend_from_slice(artifact.kind.as_bytes());
        buf.extend_from_slice(&artifact.content_hash);
    }
    shake256_32(&buf)
}

/// Compute the full Stage F (read-out) binding hash that goes into the
/// witness chain entry. Binds `query_id || head_id || policy_version
/// || latency_ms || trajectory_count || trajectory_root` so tampering with
/// ANY of those fields invalidates verification.
pub fn compute_readout_binding_hash(
    query_id: &str,
    head_id: &str,
    policy_version: &str,
    latency_ms: u64,
    trajectory_count: usize,
    trajectory_root: &[u8; HASH_SIZE],
) -> [u8; HASH_SIZE] {
    let mut buf = Vec::with_capacity(128);
    buf.extend_from_slice(b"context-graph-witness-readout-binding-v1");
    buf.extend_from_slice(&(query_id.len() as u64).to_be_bytes());
    buf.extend_from_slice(query_id.as_bytes());
    buf.extend_from_slice(&(head_id.len() as u64).to_be_bytes());
    buf.extend_from_slice(head_id.as_bytes());
    buf.extend_from_slice(&(policy_version.len() as u64).to_be_bytes());
    buf.extend_from_slice(policy_version.as_bytes());
    buf.extend_from_slice(&latency_ms.to_be_bytes());
    buf.extend_from_slice(&(trajectory_count as u64).to_be_bytes());
    buf.extend_from_slice(trajectory_root);
    shake256_32(&buf)
}

fn verify_certificate_witness_binding(
    segment: &[u8],
    expected_action_hash: &[u8; HASH_SIZE],
) -> Result<ChainVerification, CertError> {
    if segment.is_empty() {
        return Err(CertError::WitnessChainMissing);
    }
    let report = match verify_chain_bytes(segment) {
        Ok(report) => report,
        Err(err) => return Err(witness_chain_invalid(err)),
    };
    if report.entries != 1 {
        return Err(CertError::WitnessEntryCountMismatch {
            expected: 1,
            actual: report.entries,
        });
    }
    let entry =
        WitnessEntry::from_bytes(segment).map_err(|cause| CertError::WitnessEntryParseFailed {
            cause: Box::new(cause),
        })?;
    if entry.action_hash != *expected_action_hash {
        return Err(CertError::ManifestHashMismatch {
            claimed: entry.action_hash,
            recomputed: *expected_action_hash,
        });
    }
    Ok(report)
}

/// Map a `WitnessError` from `verify_chain_bytes` into a cause-preserving
/// `CertError::WitnessChainInvalid`. Offsets are extracted from variants that
/// carry them; variants without an offset use `0` (the chain prefix) and the
/// underlying cause makes the precise reason recoverable (F-023 / #476).
fn witness_chain_invalid(err: WitnessError) -> CertError {
    let offset = match &err {
        WitnessError::PrevHashMismatch { offset, .. } => *offset,
        WitnessError::WitnessTypeRejected { offset, .. } => *offset,
        WitnessError::EntryLengthInvalid { .. } | WitnessError::ChainLengthInvalid { .. } => 0,
    };
    CertError::WitnessChainInvalid {
        offset,
        cause: Box::new(err),
    }
}

#[cfg(test)]
#[path = "cert_tests.rs"]
mod cert_tests;

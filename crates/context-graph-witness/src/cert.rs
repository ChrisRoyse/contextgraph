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

/// Domain-separation tag mixed into Merkle leaf hashes. Distinguishing tag
/// prevents cross-protocol collisions where some external Merkle tree might
/// hash the same byte string into the same digest.
pub const MERKLE_LEAF_TAG: &[u8] = b"context-graph-witness-merkle-leaf-v1";
/// Domain-separation tag mixed into Merkle internal-node hashes. Internal
/// nodes are tagged differently from leaves so a leaf cannot be confused
/// with an internal node at any depth.
pub const MERKLE_NODE_TAG: &[u8] = b"context-graph-witness-merkle-node-v1";

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
    #[error("witness chain segment failed verification at offset {offset}")]
    WitnessChainInvalid { offset: usize },
    #[error("witness chain segment is missing or empty")]
    WitnessChainMissing,
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
            Self::WitnessChainMissing => "CERT_WITNESS_CHAIN_MISSING",
            Self::ManifestHashMismatch { .. } => "CERT_MANIFEST_HASH_MISMATCH",
            Self::ArtifactsEmpty => "CERT_ARTIFACTS_EMPTY",
            Self::TrajectoriesEmpty => "CERT_TRAJECTORIES_EMPTY",
            Self::TrajectoryCountMismatch { .. } => "CERT_TRAJECTORY_COUNT_MISMATCH",
        }
    }
}

/// One leaf of a Merkle tree: a stable ID and the SHA-equivalent
/// (SHAKE-256/32) of the content it certifies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MerkleLeaf {
    pub id: String,
    pub content_hash: [u8; HASH_SIZE],
}

impl MerkleLeaf {
    pub fn new(id: impl Into<String>, content_hash: [u8; HASH_SIZE]) -> Self {
        Self {
            id: id.into(),
            content_hash,
        }
    }

    /// Hash of the leaf with domain-separation tag and id binding.
    /// id || content_hash is bound so a different ID with the same content
    /// produces a different leaf hash.
    fn hash(&self) -> [u8; HASH_SIZE] {
        let mut buf = Vec::with_capacity(MERKLE_LEAF_TAG.len() + self.id.len() + 1 + HASH_SIZE);
        buf.extend_from_slice(MERKLE_LEAF_TAG);
        buf.push(0u8); // delimiter so id || content can't be confused
        buf.extend_from_slice(self.id.as_bytes());
        buf.push(0u8);
        buf.extend_from_slice(&self.content_hash);
        shake256_32(&buf)
    }
}

/// Membership proof for a specific leaf in a Merkle tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MerkleProof {
    /// 0-based index of the leaf this proof corresponds to.
    pub index: usize,
    /// Total number of leaves in the tree at proof construction time.
    pub leaf_count: usize,
    /// Sibling hashes from leaf-up to root, one per tree level.
    pub siblings: Vec<[u8; HASH_SIZE]>,
}

/// Merkle tree over `MerkleLeaf` nodes. Build once, query many times.
/// Internal nodes hash `tag || left || right`; odd levels duplicate the
/// last node (RFC-6962-style). Leaf hashes use a separate domain tag.
#[derive(Debug, Clone)]
pub struct MerkleTree {
    leaves: Vec<MerkleLeaf>,
    /// Per-level node hashes, level 0 = leaf-hashes, last level = root.
    levels: Vec<Vec<[u8; HASH_SIZE]>>,
}

impl MerkleTree {
    pub fn build(leaves: Vec<MerkleLeaf>) -> Result<Self, CertError> {
        if leaves.is_empty() {
            return Err(CertError::EmptyLeaves);
        }
        let mut levels: Vec<Vec<[u8; HASH_SIZE]>> = Vec::new();
        let level_zero: Vec<[u8; HASH_SIZE]> = leaves.iter().map(MerkleLeaf::hash).collect();
        levels.push(level_zero);
        while levels.last().expect("at least one level").len() > 1 {
            let prev = levels.last().expect("at least one level");
            let mut next: Vec<[u8; HASH_SIZE]> = Vec::with_capacity((prev.len() + 1) / 2);
            for chunk in prev.chunks(2) {
                let left = chunk[0];
                let right = if chunk.len() == 2 { chunk[1] } else { chunk[0] };
                next.push(hash_internal(&left, &right));
            }
            levels.push(next);
        }
        Ok(Self { leaves, levels })
    }

    pub fn leaves(&self) -> &[MerkleLeaf] {
        &self.leaves
    }

    pub fn root(&self) -> [u8; HASH_SIZE] {
        *self
            .levels
            .last()
            .expect("build() guarantees at least one level")
            .first()
            .expect("each level has at least one node")
    }

    pub fn proof_for(&self, index: usize) -> Result<MerkleProof, CertError> {
        if index >= self.leaves.len() {
            return Err(CertError::ProofIndexOutOfBounds {
                index,
                leaf_count: self.leaves.len(),
            });
        }
        let mut siblings: Vec<[u8; HASH_SIZE]> = Vec::with_capacity(self.levels.len() - 1);
        let mut cursor = index;
        for level in &self.levels[..self.levels.len() - 1] {
            let sibling_index = cursor ^ 1;
            let sibling = if sibling_index < level.len() {
                level[sibling_index]
            } else {
                level[cursor]
            };
            siblings.push(sibling);
            cursor /= 2;
        }
        Ok(MerkleProof {
            index,
            leaf_count: self.leaves.len(),
            siblings,
        })
    }

    /// Verify a `(leaf, proof)` pair against an expected root. Fail-closed
    /// — any structural anomaly returns a structured error instead of
    /// silently accepting.
    pub fn verify_proof(
        leaf: &MerkleLeaf,
        proof: &MerkleProof,
        expected_root: &[u8; HASH_SIZE],
    ) -> Result<(), CertError> {
        let expected_proof_len = expected_proof_length(proof.leaf_count);
        if proof.siblings.len() != expected_proof_len {
            return Err(CertError::ProofLengthInvalid {
                expected: expected_proof_len,
                actual: proof.siblings.len(),
            });
        }
        if proof.index >= proof.leaf_count {
            return Err(CertError::ProofIndexOutOfBounds {
                index: proof.index,
                leaf_count: proof.leaf_count,
            });
        }
        let mut current = leaf.hash();
        let mut cursor = proof.index;
        for sibling in &proof.siblings {
            let (left, right) = if cursor % 2 == 0 {
                (current, *sibling)
            } else {
                (*sibling, current)
            };
            current = hash_internal(&left, &right);
            cursor /= 2;
        }
        if current != *expected_root {
            return Err(CertError::ProofRejected { index: proof.index });
        }
        Ok(())
    }
}

fn hash_internal(left: &[u8; HASH_SIZE], right: &[u8; HASH_SIZE]) -> [u8; HASH_SIZE] {
    let mut buf = Vec::with_capacity(MERKLE_NODE_TAG.len() + 2 * HASH_SIZE);
    buf.extend_from_slice(MERKLE_NODE_TAG);
    buf.extend_from_slice(left);
    buf.extend_from_slice(right);
    shake256_32(&buf)
}

fn expected_proof_length(leaf_count: usize) -> usize {
    let mut levels = 0usize;
    let mut size = leaf_count;
    while size > 1 {
        size = (size + 1) / 2;
        levels += 1;
    }
    levels
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
        let manifest_hash =
            compute_stage0_binding_hash(&corpus_id, &policy_version, &artifacts);
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
        let recomputed = compute_stage0_binding_hash(
            &self.corpus_id,
            &self.policy_version,
            &self.artifacts,
        );
        if recomputed != self.manifest_hash {
            return Err(CertError::ManifestHashMismatch {
                claimed: self.manifest_hash,
                recomputed,
            });
        }
        if self.witness_chain_segment.is_empty() {
            return Err(CertError::WitnessChainMissing);
        }
        match verify_chain_bytes(&self.witness_chain_segment) {
            Ok(report) => Ok(report),
            Err(WitnessError::PrevHashMismatch { offset, .. }) => {
                Err(CertError::WitnessChainInvalid { offset })
            }
            Err(_) => Err(CertError::WitnessChainInvalid { offset: 0 }),
        }
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
        if self.witness_chain_segment.is_empty() {
            return Err(CertError::WitnessChainMissing);
        }
        let entry = WitnessEntry::from_bytes(
            &self.witness_chain_segment[..crate::WITNESS_ENTRY_SIZE.min(self.witness_chain_segment.len())],
        )
        .map_err(|_| CertError::WitnessChainInvalid { offset: 0 })?;
        if entry.action_hash != recomputed_binding {
            return Err(CertError::ManifestHashMismatch {
                claimed: entry.action_hash,
                recomputed: recomputed_binding,
            });
        }
        match verify_chain_bytes(&self.witness_chain_segment) {
            Ok(report) => Ok(report),
            Err(WitnessError::PrevHashMismatch { offset, .. }) => {
                Err(CertError::WitnessChainInvalid { offset })
            }
            Err(_) => Err(CertError::WitnessChainInvalid { offset: 0 }),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_hash(byte: u8) -> [u8; HASH_SIZE] {
        [byte; HASH_SIZE]
    }

    fn sample_leaves(n: usize) -> Vec<MerkleLeaf> {
        (0..n)
            .map(|i| MerkleLeaf::new(format!("leaf-{i}"), fixed_hash(i as u8)))
            .collect()
    }

    #[test]
    fn rejects_empty_leaves() {
        let err = MerkleTree::build(vec![]).expect_err("empty tree must error");
        assert_eq!(err.code(), "CERT_MERKLE_EMPTY_LEAVES");
    }

    #[test]
    fn single_leaf_root_equals_leaf_hash() {
        let leaves = sample_leaves(1);
        let tree = MerkleTree::build(leaves.clone()).unwrap();
        assert_eq!(tree.root(), leaves[0].hash());
    }

    #[test]
    fn proof_for_each_leaf_verifies() {
        for n in [2usize, 3, 4, 5, 7, 8, 9, 16] {
            let leaves = sample_leaves(n);
            let tree = MerkleTree::build(leaves.clone()).unwrap();
            let root = tree.root();
            for (i, leaf) in leaves.iter().enumerate() {
                let proof = tree.proof_for(i).unwrap();
                MerkleTree::verify_proof(leaf, &proof, &root)
                    .unwrap_or_else(|e| panic!("proof must verify for n={n} i={i}: {e}"));
            }
        }
    }

    #[test]
    fn proof_index_out_of_bounds_rejected() {
        let leaves = sample_leaves(4);
        let tree = MerkleTree::build(leaves).unwrap();
        let err = tree.proof_for(4).expect_err("oob index must error");
        assert_eq!(err.code(), "CERT_MERKLE_PROOF_INDEX_OUT_OF_BOUNDS");
    }

    #[test]
    fn proof_with_tampered_sibling_rejected() {
        let leaves = sample_leaves(4);
        let tree = MerkleTree::build(leaves.clone()).unwrap();
        let mut proof = tree.proof_for(2).unwrap();
        proof.siblings[0][0] ^= 0x01;
        let err =
            MerkleTree::verify_proof(&leaves[2], &proof, &tree.root()).expect_err("tamper rejects");
        assert_eq!(err.code(), "CERT_MERKLE_PROOF_REJECTED");
    }

    #[test]
    fn proof_with_wrong_length_rejected() {
        let leaves = sample_leaves(8);
        let tree = MerkleTree::build(leaves.clone()).unwrap();
        let mut proof = tree.proof_for(0).unwrap();
        proof.siblings.pop();
        let err = MerkleTree::verify_proof(&leaves[0], &proof, &tree.root())
            .expect_err("short proof rejects");
        assert_eq!(err.code(), "CERT_MERKLE_PROOF_LENGTH_INVALID");
    }

    #[test]
    fn proof_pinned_to_wrong_leaf_rejected() {
        let leaves = sample_leaves(4);
        let tree = MerkleTree::build(leaves.clone()).unwrap();
        let proof_for_zero = tree.proof_for(0).unwrap();
        // Verifying leaves[1] with proof for leaves[0] must reject —
        // even though both are valid leaves.
        let err = MerkleTree::verify_proof(&leaves[1], &proof_for_zero, &tree.root())
            .expect_err("wrong leaf rejects");
        assert_eq!(err.code(), "CERT_MERKLE_PROOF_REJECTED");
    }

    #[test]
    fn distinct_leaves_produce_distinct_roots() {
        let a = MerkleTree::build(sample_leaves(8)).unwrap().root();
        let b = MerkleTree::build(sample_leaves(9)).unwrap().root();
        assert_ne!(a, b);
    }

    #[test]
    fn stage0_roundtrip_verifies() {
        let artifacts = vec![
            ArtifactRef {
                kind: "privacy_policy".into(),
                content_hash: fixed_hash(0xAA),
            },
            ArtifactRef {
                kind: "consent_matrix".into(),
                content_hash: fixed_hash(0xBB),
            },
        ];
        let cert = Stage0Certificate::issue(
            "corpus-rec-v1",
            artifacts,
            "policy-v1",
            1_000_000_000,
            0x07,
        )
        .unwrap();
        let report = cert.verify().expect("fresh cert must verify");
        assert_eq!(report.entries, 1);
    }

    #[test]
    fn stage0_rejects_empty_artifacts() {
        let err = Stage0Certificate::issue("corpus-x", vec![], "policy-v1", 1, 1)
            .expect_err("empty artifacts must error");
        assert_eq!(err.code(), "CERT_ARTIFACTS_EMPTY");
    }

    #[test]
    fn stage0_rejects_tampered_artifact_kind() {
        let artifacts = vec![ArtifactRef {
            kind: "privacy_policy".into(),
            content_hash: fixed_hash(0xAA),
        }];
        let mut cert = Stage0Certificate::issue("c", artifacts, "v1", 1, 1).unwrap();
        cert.artifacts[0].kind = "tampered_policy".into();
        let err = cert.verify().expect_err("tampered kind rejects");
        assert_eq!(err.code(), "CERT_MANIFEST_HASH_MISMATCH");
    }

    #[test]
    fn stage0_rejects_tampered_corpus_id() {
        let artifacts = vec![ArtifactRef {
            kind: "privacy_policy".into(),
            content_hash: fixed_hash(0xAA),
        }];
        let mut cert = Stage0Certificate::issue("corpus-a", artifacts, "v1", 1, 1).unwrap();
        cert.corpus_id = "corpus-b".into();
        let err = cert.verify().expect_err("tampered corpus_id rejects");
        assert_eq!(err.code(), "CERT_MANIFEST_HASH_MISMATCH");
    }

    #[test]
    fn stage0_rejects_tampered_policy_version() {
        let artifacts = vec![ArtifactRef {
            kind: "privacy_policy".into(),
            content_hash: fixed_hash(0xAA),
        }];
        let mut cert = Stage0Certificate::issue("corpus-a", artifacts, "v1", 1, 1).unwrap();
        cert.policy_version = "v2".into();
        let err = cert.verify().expect_err("tampered policy_version rejects");
        assert_eq!(err.code(), "CERT_MANIFEST_HASH_MISMATCH");
    }

    #[test]
    fn readout_rejects_tampered_query_id() {
        let trajectories = sample_leaves(4);
        let (mut cert, tree) =
            ReadoutCertificate::issue("q-1", "head-A", &trajectories, "v1", 5, 1, 9).unwrap();
        cert.query_id = "q-2".into();
        let err = cert
            .verify_with_tree(&tree)
            .expect_err("tampered query_id rejects");
        assert_eq!(err.code(), "CERT_MANIFEST_HASH_MISMATCH");
    }

    #[test]
    fn readout_rejects_tampered_head_id() {
        let trajectories = sample_leaves(4);
        let (mut cert, tree) =
            ReadoutCertificate::issue("q-1", "head-A", &trajectories, "v1", 5, 1, 9).unwrap();
        cert.head_id = "head-B".into();
        let err = cert
            .verify_with_tree(&tree)
            .expect_err("tampered head_id rejects");
        assert_eq!(err.code(), "CERT_MANIFEST_HASH_MISMATCH");
    }

    #[test]
    fn readout_rejects_tampered_policy_version() {
        let trajectories = sample_leaves(4);
        let (mut cert, tree) =
            ReadoutCertificate::issue("q-1", "head-A", &trajectories, "v1", 5, 1, 9).unwrap();
        cert.policy_version = "v2".into();
        let err = cert
            .verify_with_tree(&tree)
            .expect_err("tampered policy_version rejects");
        assert_eq!(err.code(), "CERT_MANIFEST_HASH_MISMATCH");
    }

    #[test]
    fn readout_rejects_tampered_latency_ms() {
        let trajectories = sample_leaves(4);
        let (mut cert, tree) =
            ReadoutCertificate::issue("q-1", "head-A", &trajectories, "v1", 5, 1, 9).unwrap();
        cert.latency_ms = 999;
        let err = cert
            .verify_with_tree(&tree)
            .expect_err("tampered latency_ms rejects");
        assert_eq!(err.code(), "CERT_MANIFEST_HASH_MISMATCH");
    }

    #[test]
    fn stage0_rejects_tampered_artifact_content_hash() {
        let artifacts = vec![ArtifactRef {
            kind: "privacy_policy".into(),
            content_hash: fixed_hash(0xAA),
        }];
        let mut cert = Stage0Certificate::issue("c", artifacts, "v1", 1, 1).unwrap();
        cert.artifacts[0].content_hash[0] ^= 0x01;
        let err = cert.verify().expect_err("tampered content rejects");
        assert_eq!(err.code(), "CERT_MANIFEST_HASH_MISMATCH");
    }

    #[test]
    fn readout_cert_roundtrip_verifies() {
        let trajectories = sample_leaves(5);
        let (cert, tree) =
            ReadoutCertificate::issue("q-1", "compensation_band", &trajectories, "v1", 12, 1, 9)
                .unwrap();
        cert.verify_with_tree(&tree).expect("verify must accept");
        let proof = tree.proof_for(2).unwrap();
        cert.verify_trajectory(&trajectories[2], &proof)
            .expect("trajectory proof must verify");
    }

    #[test]
    fn readout_cert_rejects_empty_trajectories() {
        let err = ReadoutCertificate::issue("q-1", "h", &[], "v1", 0, 1, 9)
            .expect_err("empty trajectories must error");
        assert_eq!(err.code(), "CERT_TRAJECTORIES_EMPTY");
    }

    #[test]
    fn readout_cert_rejects_tree_with_different_count() {
        let trajectories_a = sample_leaves(4);
        let trajectories_b = sample_leaves(5);
        let (cert, _) =
            ReadoutCertificate::issue("q", "h", &trajectories_a, "v1", 0, 1, 9).unwrap();
        let other_tree = MerkleTree::build(trajectories_b).unwrap();
        let err = cert
            .verify_with_tree(&other_tree)
            .expect_err("wrong-count tree must reject");
        assert_eq!(err.code(), "CERT_TRAJECTORY_COUNT_MISMATCH");
    }

    #[test]
    fn readout_cert_rejects_same_count_but_different_root() {
        let trajectories_a = sample_leaves(4);
        let trajectories_b: Vec<MerkleLeaf> = (0..4)
            .map(|i| MerkleLeaf::new(format!("other-{i}"), [(0xFF - i as u8); HASH_SIZE]))
            .collect();
        let (cert, _) =
            ReadoutCertificate::issue("q", "h", &trajectories_a, "v1", 0, 1, 9).unwrap();
        let other_tree = MerkleTree::build(trajectories_b).unwrap();
        let err = cert
            .verify_with_tree(&other_tree)
            .expect_err("wrong-root tree must reject");
        assert_eq!(err.code(), "CERT_MERKLE_ROOT_MISMATCH");
    }

    #[test]
    fn readout_cert_trajectory_proof_rejects_wrong_leaf() {
        let trajectories = sample_leaves(8);
        let (cert, tree) = ReadoutCertificate::issue("q", "h", &trajectories, "v1", 0, 1, 9).unwrap();
        let proof_for_three = tree.proof_for(3).unwrap();
        let err = cert
            .verify_trajectory(&trajectories[4], &proof_for_three)
            .expect_err("wrong leaf must reject");
        assert_eq!(err.code(), "CERT_MERKLE_PROOF_REJECTED");
    }

    #[test]
    fn manifest_hash_changes_when_artifacts_reordered() {
        let a = vec![
            ArtifactRef {
                kind: "k1".into(),
                content_hash: fixed_hash(1),
            },
            ArtifactRef {
                kind: "k2".into(),
                content_hash: fixed_hash(2),
            },
        ];
        let mut b = a.clone();
        b.reverse();
        assert_ne!(
            compute_artifact_manifest_hash(&a),
            compute_artifact_manifest_hash(&b)
        );
    }
}

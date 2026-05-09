// Inspired by ruvnet/RuVector crates/rvf/rvf-crypto/src/{lineage,attestation,witness}.rs
// at HEAD ef5274c2 (read 2026-05-08).
// Clean-room reimplementation; no code copied, no upstream tracking. See
// memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md
// for the policy.

use super::CertError;
use crate::{shake256_32, HASH_SIZE};

/// Domain-separation tag mixed into Merkle leaf hashes. Distinguishing tag
/// prevents cross-protocol collisions where some external Merkle tree might
/// hash the same byte string into the same digest.
pub const MERKLE_LEAF_TAG: &[u8] = b"context-graph-witness-merkle-leaf-v1";
/// Domain-separation tag mixed into Merkle internal-node hashes. Internal
/// nodes are tagged differently from leaves so a leaf cannot be confused
/// with an internal node at any depth.
pub const MERKLE_NODE_TAG: &[u8] = b"context-graph-witness-merkle-node-v1";

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
    pub(crate) fn hash(&self) -> [u8; HASH_SIZE] {
        let mut buf = Vec::with_capacity(MERKLE_LEAF_TAG.len() + 8 + self.id.len() + HASH_SIZE);
        buf.extend_from_slice(MERKLE_LEAF_TAG);
        buf.extend_from_slice(&(self.id.len() as u64).to_be_bytes());
        buf.extend_from_slice(self.id.as_bytes());
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
    /// This value is part of proof-shape validation. Callers verifying a
    /// certificate must still compare it to the certificate's source-of-truth
    /// tree size; the Merkle root alone does not make this field authoritative.
    pub leaf_count: usize,
    /// Sibling hashes from leaf-up to root, one per tree level.
    pub siblings: Vec<[u8; HASH_SIZE]>,
}

/// Merkle tree over `MerkleLeaf` nodes. Build once, query many times.
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
        levels.push(leaves.iter().map(MerkleLeaf::hash).collect());
        while levels.last().expect("at least one level").len() > 1 {
            let prev = levels.last().expect("at least one level");
            let mut next: Vec<[u8; HASH_SIZE]> = Vec::with_capacity(prev.len().div_ceil(2));
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
            siblings.push(if sibling_index < level.len() {
                level[sibling_index]
            } else {
                level[cursor]
            });
            cursor /= 2;
        }
        Ok(MerkleProof {
            index,
            leaf_count: self.leaves.len(),
            siblings,
        })
    }

    /// Verify a `(leaf, proof)` pair against an expected root.
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
            let (left, right) = if cursor.is_multiple_of(2) {
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
        size = size.div_ceil(2);
        levels += 1;
    }
    levels
}

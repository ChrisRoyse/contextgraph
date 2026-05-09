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
    let cert =
        Stage0Certificate::issue("corpus-rec-v1", artifacts, "policy-v1", 1_000_000_000, 0x07)
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
fn stage0_rejects_valid_chain_with_wrong_action_hash() {
    let artifacts = vec![ArtifactRef {
        kind: "privacy_policy".into(),
        content_hash: fixed_hash(0xAA),
    }];
    let mut cert = Stage0Certificate::issue("corpus-a", artifacts, "v1", 1, 1).unwrap();
    let wrong = WitnessEntry::new(ZERO_HASH, fixed_hash(0x55), 1, 1);
    cert.witness_chain_segment = wrong.to_bytes().to_vec();
    let err = cert
        .verify()
        .expect_err("valid chain with wrong action hash must reject");
    assert_eq!(err.code(), "CERT_MANIFEST_HASH_MISMATCH");
}

#[test]
fn stage0_rejects_multi_entry_witness_segment() {
    let artifacts = vec![ArtifactRef {
        kind: "privacy_policy".into(),
        content_hash: fixed_hash(0xAA),
    }];
    let mut cert = Stage0Certificate::issue("corpus-a", artifacts, "v1", 1, 1).unwrap();
    let first = WitnessEntry::from_bytes(&cert.witness_chain_segment).unwrap();
    let second = WitnessEntry::new(first.chain_hash(), fixed_hash(0xBB), 2, 1);
    cert.witness_chain_segment
        .extend_from_slice(&second.to_bytes());
    let err = cert
        .verify()
        .expect_err("certificate witness segment must be exactly one entry");
    assert_eq!(err.code(), "CERT_WITNESS_ENTRY_COUNT_MISMATCH");
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
fn readout_rejects_valid_chain_with_wrong_action_hash() {
    let trajectories = sample_leaves(4);
    let (mut cert, tree) =
        ReadoutCertificate::issue("q-1", "head-A", &trajectories, "v1", 5, 1, 9).unwrap();
    let wrong = WitnessEntry::new(ZERO_HASH, fixed_hash(0x44), 1, 9);
    cert.witness_chain_segment = wrong.to_bytes().to_vec();
    let err = cert
        .verify_with_tree(&tree)
        .expect_err("valid chain with wrong action hash must reject");
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
    let (cert, _) = ReadoutCertificate::issue("q", "h", &trajectories_a, "v1", 0, 1, 9).unwrap();
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
    let (cert, _) = ReadoutCertificate::issue("q", "h", &trajectories_a, "v1", 0, 1, 9).unwrap();
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

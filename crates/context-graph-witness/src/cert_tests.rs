// Inspired by ruvnet/RuVector crates/rvf/rvf-crypto/src/{lineage,attestation,witness}.rs
// at HEAD ef5274c2 (read 2026-05-08).
// Clean-room reimplementation; no code copied, no upstream tracking. See
// memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md
// for the policy.

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

// -----------------------------------------------------------------------------
// F-023 / #476 regression tests: WitnessChainInvalid must preserve the
// underlying `WitnessError` cause rather than collapsing every failure to
// `{ offset: 0 }`. Forensic auditors need the exact failure mode to recover.
// -----------------------------------------------------------------------------

fn issue_stage0() -> Stage0Certificate {
    let artifacts = vec![ArtifactRef {
        kind: "privacy_policy".into(),
        content_hash: fixed_hash(0xAA),
    }];
    Stage0Certificate::issue("corpus-a", artifacts, "v1", 1, 1).unwrap()
}

#[test]
fn witness_chain_invalid_preserves_prev_hash_mismatch_cause() {
    let mut cert = issue_stage0();
    // Append a second entry whose prev_hash is wrong (use ZERO_HASH instead
    // of the first entry's chain_hash). verify_chain_bytes will report
    // PrevHashMismatch { offset: 1 }.
    let bogus_second = WitnessEntry::new(ZERO_HASH, fixed_hash(0xBB), 2, 1);
    cert.witness_chain_segment
        .extend_from_slice(&bogus_second.to_bytes());
    let err = cert.verify().expect_err("prev-hash mismatch must reject");
    assert_eq!(err.code(), "CERT_WITNESS_CHAIN_INVALID");
    assert_eq!(
        err.witness_chain_offset(),
        Some(1),
        "must preserve offset from PrevHashMismatch"
    );
    let cause = err
        .witness_cause()
        .expect("F-023: WitnessChainInvalid must carry an underlying cause");
    assert!(
        matches!(cause, WitnessError::PrevHashMismatch { offset: 1, .. }),
        "cause must be PrevHashMismatch with the matching offset; got {cause:?}"
    );
    assert_eq!(cause.code(), "WITNESS_PREV_HASH_MISMATCH");
}

#[test]
fn witness_chain_invalid_preserves_chain_length_invalid_cause() {
    let mut cert = issue_stage0();
    // Truncate to 50 bytes — not a multiple of WITNESS_ENTRY_SIZE (73).
    cert.witness_chain_segment.truncate(50);
    let err = cert.verify().expect_err("chain-length invalid must reject");
    assert_eq!(err.code(), "CERT_WITNESS_CHAIN_INVALID");
    assert_eq!(err.witness_chain_offset(), Some(0));
    let cause = err
        .witness_cause()
        .expect("F-023: chain-length errors must carry cause");
    assert!(
        matches!(cause, WitnessError::ChainLengthInvalid { len: 50, .. }),
        "cause must be ChainLengthInvalid with len=50; got {cause:?}"
    );
    assert_eq!(cause.code(), "WITNESS_CHAIN_LENGTH_INVALID");
}

#[test]
fn witness_chain_invalid_does_not_collapse_distinct_causes() {
    // F-023: BEFORE the fix, all errors collapsed to { offset: 0 } and were
    // string-equal. AFTER the fix, the cause must be distinguishable.
    let mut cert_prev = issue_stage0();
    let bogus = WitnessEntry::new(ZERO_HASH, fixed_hash(0xBB), 2, 1);
    cert_prev
        .witness_chain_segment
        .extend_from_slice(&bogus.to_bytes());
    let err_prev = cert_prev.verify().unwrap_err();

    let mut cert_len = issue_stage0();
    cert_len.witness_chain_segment.truncate(50);
    let err_len = cert_len.verify().unwrap_err();

    // Same outer code, distinct preserved causes.
    assert_eq!(err_prev.code(), err_len.code());
    let cause_prev = err_prev.witness_cause().unwrap();
    let cause_len = err_len.witness_cause().unwrap();
    assert_ne!(
        cause_prev.code(),
        cause_len.code(),
        "F-023: distinct WitnessErrors must remain distinguishable on CertError"
    );
    // And the Display output must include the cause.
    let display_prev = format!("{err_prev}");
    assert!(
        display_prev.contains("prev hash"),
        "Display must surface underlying cause; got: {display_prev}"
    );
}

#[test]
fn witness_entry_parse_failed_distinct_from_chain_invalid() {
    // F-023 refinement: a parse failure on the segment must be the
    // `WitnessEntryParseFailed` variant, not collapsed into WitnessChainInvalid.
    //
    // We construct a segment whose length IS a multiple of 73 (so the chain
    // verification passes the length check) but where the second entry's
    // prev_hash is INVALID. verify_chain_bytes finds the prev_hash mismatch
    // first, so this exercises the WitnessChainInvalid path with cause
    // preservation.
    //
    // To exercise the `from_bytes` failure path we need a single-entry chain
    // that passes verify_chain_bytes (entries==1) but whose bytes still fail
    // re-parse. Since from_bytes only checks length, and verify_chain_bytes
    // also enforces length-as-multiple, both fail on the same length check.
    // The structural guarantee is that any path that calls from_bytes after
    // a successful verify_chain_bytes will succeed on input that has
    // already been validated. Therefore the explicit WitnessEntryParseFailed
    // variant exists as a defensive surface for future contract changes;
    // we verify the variant + code are wired correctly via code().
    use crate::cert::CertError;
    let err = CertError::WitnessEntryParseFailed {
        cause: Box::new(WitnessError::EntryLengthInvalid {
            expected: 73,
            actual: 50,
        }),
    };
    assert_eq!(err.code(), "CERT_WITNESS_ENTRY_PARSE_FAILED");
    let cause = err.witness_cause().expect("parse-failed must carry cause");
    assert_eq!(cause.code(), "WITNESS_ENTRY_LENGTH_INVALID");
}

#[test]
fn witness_cause_returns_none_for_non_witness_variants() {
    use crate::cert::CertError;
    let err = CertError::EmptyLeaves;
    assert!(err.witness_cause().is_none());
    assert!(err.witness_chain_offset().is_none());
}

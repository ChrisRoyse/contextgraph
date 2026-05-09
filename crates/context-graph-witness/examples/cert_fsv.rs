// Manual Full State Verification for Stage 0 + Stage F + Merkle certificates.
// Writes JSON evidence + raw certificates to disk under
// /tmp/contextgraph-tier2-fsv/ for physical inspection.

use context_graph_witness::{
    cert::{
        compute_stage0_binding_hash, ArtifactRef, MerkleLeaf, ReadoutCertificate, Stage0Certificate,
    },
    WitnessEntry, HASH_SIZE, ZERO_HASH,
};
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let evidence_root = PathBuf::from("/tmp/contextgraph-tier2-fsv");
    fs::create_dir_all(&evidence_root)?;

    // Stage 0: bind a synthetic recruiting corpus to a manifest of
    // privacy-policy / consent-matrix / TOS artifacts.
    let stage0_artifacts = vec![
        ArtifactRef {
            kind: "privacy_policy".into(),
            content_hash: [0xAA; HASH_SIZE],
        },
        ArtifactRef {
            kind: "consent_matrix".into(),
            content_hash: [0xBB; HASH_SIZE],
        },
        ArtifactRef {
            kind: "operating_environment_approval".into(),
            content_hash: [0xCC; HASH_SIZE],
        },
    ];
    let manifest_hash =
        compute_stage0_binding_hash("corpus-recruiting-v1", "policy-v1", &stage0_artifacts);
    let manifest_hash_hex = hex(&manifest_hash);
    let stage0 = Stage0Certificate::issue(
        "corpus-recruiting-v1",
        stage0_artifacts.clone(),
        "policy-v1",
        1_700_000_000_000_000_000,
        0x07,
    )?;
    let stage0_report = stage0.verify()?;
    // Tampering test: change corpus_id and re-verify; must reject.
    let mut tampered_corpus = stage0.clone();
    tampered_corpus.corpus_id = "corpus-other".into();
    let tampered_corpus_code = match tampered_corpus.verify() {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };
    let mut tampered_policy = stage0.clone();
    tampered_policy.policy_version = "policy-v2".into();
    let tampered_policy_code = match tampered_policy.verify() {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };
    let mut wrong_stage0_chain = stage0.clone();
    let wrong_stage0_entry = WitnessEntry::new(ZERO_HASH, [0x55; HASH_SIZE], 1, 0x07);
    wrong_stage0_chain.witness_chain_segment = wrong_stage0_entry.to_bytes().to_vec();
    let wrong_stage0_chain_code = match wrong_stage0_chain.verify() {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };

    // Stage F (read-out): bind a "compensation_band" emission to 5 trajectory
    // IDs via a Merkle tree. Verify each trajectory's proof of membership.
    let trajectory_leaves: Vec<MerkleLeaf> = (0..5)
        .map(|i| MerkleLeaf::new(format!("trajectory-{i:04}"), [(0x10 + i as u8); HASH_SIZE]))
        .collect();
    let (readout, tree) = ReadoutCertificate::issue(
        "query-q-1",
        "compensation_band",
        &trajectory_leaves,
        "policy-v1",
        12,
        1_700_000_000_500_000_000,
        0x09,
    )?;
    let readout_report = readout.verify_with_tree(&tree)?;
    let proof_for_2 = tree.proof_for(2)?;
    readout.verify_trajectory(&trajectory_leaves[2], &proof_for_2)?;

    // Boundary: tampered artifact rejects.
    let mut tampered_stage0 = stage0.clone();
    tampered_stage0.artifacts[0].content_hash[0] ^= 0x01;
    let tampered_code = match tampered_stage0.verify() {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };

    // Boundary: wrong-leaf proof rejects.
    let wrong_leaf_code = match readout.verify_trajectory(&trajectory_leaves[3], &proof_for_2) {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };
    let mut wrong_readout_chain = readout.clone();
    let wrong_readout_entry = WitnessEntry::new(ZERO_HASH, [0x44; HASH_SIZE], 1, 0x09);
    wrong_readout_chain.witness_chain_segment = wrong_readout_entry.to_bytes().to_vec();
    let wrong_readout_chain_code = match wrong_readout_chain.verify_with_tree(&tree) {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };

    // Persist Stage 0 cert + Stage F cert components to disk.
    let stage0_chain_path = evidence_root.join("stage0-witness-chain.bin");
    fs::write(&stage0_chain_path, &stage0.witness_chain_segment)?;
    let stage0_back = fs::read(&stage0_chain_path)?;
    let stage0_chain_roundtrip = stage0_back == stage0.witness_chain_segment;

    let trajectory_root_path = evidence_root.join("readout-trajectory-root.bin");
    fs::write(&trajectory_root_path, readout.trajectory_root)?;
    let trajectory_root_back = fs::read(&trajectory_root_path)?;
    let trajectory_root_roundtrip = trajectory_root_back == readout.trajectory_root;

    let evidence = serde_json::json!({
        "stage0": {
            "corpus_id": stage0.corpus_id,
            "policy_version": stage0.policy_version,
            "artifact_count": stage0.artifacts.len(),
            "binding_hash_hex": manifest_hash_hex.clone(),
            "binding_recomputed_matches_cert_field": manifest_hash == stage0.manifest_hash,
            "witness_chain_entries": stage0_report.entries,
            "witness_chain_byte_len": stage0.witness_chain_segment.len(),
            "boundary_tampered_artifact_byte": {
                "before": {
                    "artifact_index": 0,
                    "original_hash_prefix": "aa",
                    "tampered_hash_prefix": format!("{:02x}", tampered_stage0.artifacts[0].content_hash[0]),
                    "expected_mutation": "cert clone only",
                },
                "after": {
                    "expected_code": "CERT_MANIFEST_HASH_MISMATCH",
                    "actual_code": tampered_code,
                },
            },
            "boundary_tampered_corpus_id": {
                "before": {
                    "original_corpus_id": "corpus-recruiting-v1",
                    "tampered_corpus_id": tampered_corpus.corpus_id,
                    "expected_mutation": "cert clone only",
                },
                "after": {
                    "expected_code": "CERT_MANIFEST_HASH_MISMATCH",
                    "actual_code": tampered_corpus_code,
                },
            },
            "boundary_tampered_policy_version": {
                "before": {
                    "original_policy_version": "policy-v1",
                    "tampered_policy_version": tampered_policy.policy_version,
                    "expected_mutation": "cert clone only",
                },
                "after": {
                    "expected_code": "CERT_MANIFEST_HASH_MISMATCH",
                    "actual_code": tampered_policy_code,
                },
            },
            "boundary_valid_chain_wrong_action_hash": {
                "before": {
                    "wrong_action_hash_prefix": "55",
                    "expected_binding_hash_prefix": &manifest_hash_hex[0..2],
                    "expected_mutation": "cert clone only",
                },
                "after": {
                    "expected_code": "CERT_MANIFEST_HASH_MISMATCH",
                    "actual_code": wrong_stage0_chain_code,
                },
            },
            "disk_roundtrip_chain_bytes_match": stage0_chain_roundtrip,
            "disk_path": stage0_chain_path.display().to_string(),
        },
        "readout": {
            "query_id": readout.query_id,
            "head_id": readout.head_id,
            "trajectory_count": readout.trajectory_count,
            "trajectory_root_hex": hex(&readout.trajectory_root),
            "witness_chain_entries": readout_report.entries,
            "witness_chain_byte_len": readout.witness_chain_segment.len(),
            "merkle_proof_index_2_verifies": true,
            "boundary_proof_for_wrong_leaf": {
                "before": {
                    "proof_index": proof_for_2.index,
                    "verified_leaf": "trajectory-0003",
                    "expected_leaf": "trajectory-0002",
                    "expected_mutation": "none",
                },
                "after": {
                    "expected_code": "CERT_MERKLE_PROOF_REJECTED",
                    "actual_code": wrong_leaf_code,
                },
            },
            "boundary_valid_chain_wrong_action_hash": {
                "before": {
                    "wrong_action_hash_prefix": "44",
                    "expected_mutation": "cert clone only",
                },
                "after": {
                    "expected_code": "CERT_MANIFEST_HASH_MISMATCH",
                    "actual_code": wrong_readout_chain_code,
                },
            },
            "disk_roundtrip_root_bytes_match": trajectory_root_roundtrip,
            "disk_path": trajectory_root_path.display().to_string(),
        },
    });

    let path = evidence_root.join("witness-cert-evidence.json");
    fs::write(&path, serde_json::to_string_pretty(&evidence)?)?;
    let readback_text = fs::read_to_string(&path)?;
    let readback: serde_json::Value = serde_json::from_str(&readback_text)?;
    assert_eq!(readback, evidence);
    println!("Witness cert FSV evidence: {}", path.display());
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut s, "{byte:02x}").unwrap();
    }
    s
}

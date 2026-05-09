// Manual Full State Verification for ed25519 segment signing on the
// witness chain. Writes JSON evidence + raw signed segment to disk under
// /tmp/contextgraph-tier1-fsv/ for physical inspection.

use context_graph_witness::{
    sign::{verify_segment, verify_segment_with_expected_pubkey, WitnessKeypair},
    verify_chain_bytes, WitnessEntry, HASH_SIZE, WITNESS_ENTRY_SIZE, ZERO_HASH,
};
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let evidence_root = PathBuf::from("/tmp/contextgraph-tier1-fsv");
    fs::create_dir_all(&evidence_root)?;

    // Build a tiny but real witness segment: 3 chained 73-byte entries.
    let entry_one = WitnessEntry::new(ZERO_HASH, [0xAAu8; HASH_SIZE], 1_000_000_000, 0x01);
    let entry_two = WitnessEntry::new(
        entry_one.chain_hash(),
        [0xBBu8; HASH_SIZE],
        1_000_000_010,
        0x02,
    );
    let entry_three = WitnessEntry::new(
        entry_two.chain_hash(),
        [0xCCu8; HASH_SIZE],
        1_000_000_020,
        0x03,
    );

    let mut segment = Vec::with_capacity(WITNESS_ENTRY_SIZE * 3);
    segment.extend_from_slice(&entry_one.to_bytes());
    segment.extend_from_slice(&entry_two.to_bytes());
    segment.extend_from_slice(&entry_three.to_bytes());

    // Verify the chain itself first (independent of signing).
    let chain_verify = verify_chain_bytes(&segment)?;

    // Derive a keypair from a synthetic seed; sign + verify.
    let seed = [0x99u8; 32];
    let kp = WitnessKeypair::from_seed(&seed)?;
    let pubkey_hex = hex(&kp.public_key_bytes());
    let codec = kp.sign_segment(&segment)?;
    verify_segment(&segment, &codec)?;

    // Pin verification: matches expected pubkey → accepts.
    verify_segment_with_expected_pubkey(&segment, &codec, &kp.public_key_bytes())?;

    // Write segment + codec to disk; re-read; verify; physical proof.
    let segment_path = evidence_root.join("witness-segment.bin");
    let codec_path = evidence_root.join("witness-segment.sig");
    let pubkey_path = evidence_root.join("witness-segment.pub");
    fs::write(&segment_path, &segment)?;
    fs::write(&codec_path, codec)?;
    fs::write(&pubkey_path, kp.public_key_bytes())?;

    let segment_back = fs::read(&segment_path)?;
    let codec_back = fs::read(&codec_path)?;
    let pubkey_back = fs::read(&pubkey_path)?;
    let roundtrip_segment_match = segment_back == segment;
    let roundtrip_codec_match = codec_back == codec.as_slice();
    let roundtrip_pubkey_match = pubkey_back == kp.public_key_bytes().as_slice();
    verify_segment(&segment_back, &codec_back)?;
    verify_segment_with_expected_pubkey(&segment_back, &codec_back, &pubkey_back)?;

    // Boundary 1a: tampered prev_hash byte (entry 0's prev_hash slot) →
    // chain-integrity gate fires FIRST, before ed25519. Either gate is
    // fail-closed; this proves there are TWO defenses in depth.
    let mut tampered_prev = segment.clone();
    tampered_prev[10] ^= 0x01;
    let tamper_prev_code = match verify_segment(&tampered_prev, &codec) {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };

    // Boundary 1b: tampered byte in the LAST entry's action_hash (offset 178).
    // Chain-integrity gate cannot detect this (no successor to compare against),
    // so the failure must come from ed25519 verification.
    let mut tampered_last = segment.clone();
    let last_entry_action_hash_offset = WITNESS_ENTRY_SIZE * 2 + 32;
    tampered_last[last_entry_action_hash_offset] ^= 0x01;
    let tamper_last_code = match verify_segment(&tampered_last, &codec) {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };

    // Boundary 2: tampered signature byte → ed25519 fail-closed.
    let mut bad_codec = codec;
    bad_codec[40] ^= 0x01;
    let tampered_sig_code = match verify_segment(&segment, &bad_codec) {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };

    // Boundary 3: pin to a different pubkey → fail-closed.
    let other = WitnessKeypair::from_seed(&[0xAAu8; 32])?;
    let pin_mismatch =
        verify_segment_with_expected_pubkey(&segment, &codec, &other.public_key_bytes());
    let pin_mismatch_code = match pin_mismatch {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };

    // Boundary 4: short seed (16 bytes) → fail-closed.
    let bad_seed = WitnessKeypair::from_seed(&[0u8; 16]);
    let bad_seed_code = match bad_seed {
        Err(e) => e.code(),
        Ok(_) => "UNEXPECTED_OK",
    };

    let evidence = serde_json::json!({
        "chain_verify": {
            "entries": chain_verify.entries,
            "last_chain_hash_hex": hex(&chain_verify.last_chain_hash),
        },
        "signing_keypair": {
            "public_key_hex": pubkey_hex,
            "public_key_byte_count": kp.public_key_bytes().len(),
            "signing_key_redacted": "<never written to disk>",
        },
        "segment_bytes": {
            "len": segment.len(),
            "expected_len": WITNESS_ENTRY_SIZE * 3,
            "passes": segment.len() == WITNESS_ENTRY_SIZE * 3,
        },
        "codec": {
            "len": codec.len(),
            "expected_len": 96,
            "passes": codec.len() == 96,
        },
        "roundtrip_disk_io": {
            "segment_path": segment_path.display().to_string(),
            "codec_path": codec_path.display().to_string(),
            "pubkey_path": pubkey_path.display().to_string(),
            "segment_match": roundtrip_segment_match,
            "codec_match": roundtrip_codec_match,
            "pubkey_match": roundtrip_pubkey_match,
            "verify_after_disk_roundtrip": "ACCEPTED",
            "pinned_verify_after_disk_roundtrip": "ACCEPTED",
        },
        "boundary_tampered_prev_hash_byte_chain_gate": {
            "expected_code": "WITNESS_SIGNATURE_SEGMENT_PREV_HASH_MISMATCH",
            "actual_code": tamper_prev_code,
            "note": "two-tier defense: chain-integrity check fires before ed25519",
        },
        "boundary_tampered_last_action_hash_ed25519_gate": {
            "expected_code": "WITNESS_SIGNATURE_VERIFICATION_FAILED",
            "actual_code": tamper_last_code,
            "note": "chain check cannot see this; ed25519 must catch it",
        },
        "boundary_tampered_signature_byte": {
            "expected_code": "WITNESS_SIGNATURE_VERIFICATION_FAILED",
            "actual_code": tampered_sig_code,
        },
        "boundary_pinned_pubkey_mismatch": {
            "expected_code": "WITNESS_SIGNATURE_PUBKEY_MISMATCH",
            "actual_code": pin_mismatch_code,
        },
        "boundary_short_seed": {
            "expected_code": "WITNESS_SEED_LENGTH_INVALID",
            "actual_code": bad_seed_code,
        },
    });

    let path = evidence_root.join("witness-ed25519-evidence.json");
    fs::write(&path, serde_json::to_string_pretty(&evidence)?)?;
    println!("Witness ed25519 FSV evidence: {}", path.display());
    println!("{}", serde_json::to_string_pretty(&evidence)?);
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

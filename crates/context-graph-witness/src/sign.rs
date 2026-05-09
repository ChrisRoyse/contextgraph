// Inspired by ruvnet/RuVector crates/rvf/rvf-crypto/src/{sign,footer}.rs at HEAD ef5274c2 (read 2026-05-08).
// Clean-room reimplementation; no code copied, no upstream tracking. See
// memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md
// for the policy.
//
// Ed25519 segment signing for the SHAKE-256 73-byte witness chain.
//
// Design:
// - One signature covers a *segment* — a contiguous slice of the chain
//   (1 to many WitnessEntry-shaped 73-byte rows). Signers sign the
//   SHAKE-256 hash of the canonical segment bytes plus an explicit
//   domain-separation tag and the signer's public-key bytes. This
//   prevents cross-protocol replay (the same key signing some other
//   message that happened to hash to the same digest cannot be
//   reinterpreted as a witness segment signature).
// - Keys are NEVER persisted. Operators supply a 32-byte seed (e.g.
//   from an env var, a secrets manager, or stdin); we derive the
//   keypair, sign, then drop the secret material. `zeroize` wipes
//   secret bytes on drop. The public key is durable; the private key
//   is ephemeral per signing session.
// - The signature codec is fixed-size: 96 bytes
//   (32 public-key + 64 ed25519 signature). The hash bound by the
//   signature is recoverable from the segment + domain tag, so it is
//   not part of the codec.
//
// Failure modes are fail-closed and structured: every error variant
// has a stable code (`code()` method) for telemetry. No silent
// fallback, no "best-effort" verification.

use crate::{shake256_32, WitnessEntry, HASH_SIZE, WITNESS_ENTRY_SIZE};
use ed25519_dalek::{
    Signature, Signer, SigningKey, VerifyingKey, PUBLIC_KEY_LENGTH, SECRET_KEY_LENGTH,
    SIGNATURE_LENGTH,
};
use thiserror::Error;
use zeroize::Zeroize;

/// 32-byte ed25519 public key + 64-byte signature, packed as a fixed-size
/// 96-byte codec. The signature binds the SHAKE-256 hash of
/// `domain_tag || public_key || segment_bytes`.
pub const SIGNATURE_CODEC_SIZE: usize = PUBLIC_KEY_LENGTH + SIGNATURE_LENGTH;

/// Domain-separation tag mixed into the signed digest. Distinct from any
/// other ContextGraph protocol that signs ed25519 to prevent cross-protocol
/// replay.
pub const DOMAIN_TAG: &[u8] = b"context-graph-witness-segment-v1";

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SignError {
    #[error("seed length invalid: expected={expected} actual={actual}")]
    SeedLengthInvalid { expected: usize, actual: usize },
    #[error("public key length invalid: expected={expected} actual={actual}")]
    PublicKeyLengthInvalid { expected: usize, actual: usize },
    #[error("signature codec length invalid: expected={expected} actual={actual}")]
    CodecLengthInvalid { expected: usize, actual: usize },
    #[error("public key bytes did not parse as a valid ed25519 point")]
    PublicKeyMalformed,
    #[error("public key is a weak ed25519 key")]
    PublicKeyWeak,
    #[error("ed25519 verification rejected the signature for this segment")]
    VerificationFailed,
    #[error("public key in codec did not match expected public key")]
    PublicKeyMismatch,
    #[error("witness segment is empty")]
    SegmentEmpty,
    #[error("witness segment length invalid: len={len} entry_size={entry_size}")]
    SegmentLengthInvalid { len: usize, entry_size: usize },
    #[error("witness segment prev hash mismatch at offset={offset}")]
    SegmentPrevHashMismatch {
        offset: usize,
        expected_prev_hash: [u8; HASH_SIZE],
        actual_prev_hash: [u8; HASH_SIZE],
    },
}

impl SignError {
    /// Stable telemetry code for this error variant. Codes are part of the
    /// public API contract for downstream observability tooling.
    pub fn code(&self) -> &'static str {
        match self {
            Self::SeedLengthInvalid { .. } => "WITNESS_SEED_LENGTH_INVALID",
            Self::PublicKeyLengthInvalid { .. } => "WITNESS_PUBKEY_LENGTH_INVALID",
            Self::CodecLengthInvalid { .. } => "WITNESS_SIGNATURE_CODEC_LENGTH_INVALID",
            Self::PublicKeyMalformed => "WITNESS_PUBKEY_MALFORMED",
            Self::PublicKeyWeak => "WITNESS_PUBKEY_WEAK",
            Self::VerificationFailed => "WITNESS_SIGNATURE_VERIFICATION_FAILED",
            Self::PublicKeyMismatch => "WITNESS_SIGNATURE_PUBKEY_MISMATCH",
            Self::SegmentEmpty => "WITNESS_SIGNATURE_SEGMENT_EMPTY",
            Self::SegmentLengthInvalid { .. } => "WITNESS_SIGNATURE_SEGMENT_LENGTH_INVALID",
            Self::SegmentPrevHashMismatch { .. } => "WITNESS_SIGNATURE_SEGMENT_PREV_HASH_MISMATCH",
        }
    }
}

/// Owned ed25519 keypair derived from a 32-byte seed. The signing key
/// is held in memory only as long as the `WitnessKeypair` is alive; the
/// crate feature enables `ed25519-dalek/zeroize` so `SigningKey` zeroizes
/// on drop. The public key is fine to copy / log / persist.
pub struct WitnessKeypair {
    signing: SigningKey,
    verifying: VerifyingKey,
}

impl WitnessKeypair {
    /// Derive a keypair from a 32-byte seed. Reject any other length.
    /// The seed buffer is NOT zeroized by this function — the caller
    /// owns it and is responsible for wiping it.
    pub fn from_seed(seed: &[u8]) -> Result<Self, SignError> {
        if seed.len() != SECRET_KEY_LENGTH {
            return Err(SignError::SeedLengthInvalid {
                expected: SECRET_KEY_LENGTH,
                actual: seed.len(),
            });
        }
        let mut bytes = [0u8; SECRET_KEY_LENGTH];
        bytes.copy_from_slice(seed);
        let signing = SigningKey::from_bytes(&bytes);
        let verifying = signing.verifying_key();
        bytes.zeroize();
        Ok(Self { signing, verifying })
    }

    /// Return the 32-byte ed25519 public key. Safe to log, persist, or
    /// transmit; never carries any secret material.
    pub fn public_key_bytes(&self) -> [u8; PUBLIC_KEY_LENGTH] {
        self.verifying.to_bytes()
    }

    /// Sign one segment. The segment must be a non-empty concatenation of
    /// one or more internally linked 73-byte witness entries. Returns the
    /// 96-byte codec.
    pub fn sign_segment(&self, segment: &[u8]) -> Result<[u8; SIGNATURE_CODEC_SIZE], SignError> {
        let pk = self.public_key_bytes();
        let digest = digest_for_segment(pk, segment)?;
        let signature = self.signing.sign(&digest);
        let mut codec = [0u8; SIGNATURE_CODEC_SIZE];
        codec[0..PUBLIC_KEY_LENGTH].copy_from_slice(&pk);
        codec[PUBLIC_KEY_LENGTH..].copy_from_slice(&signature.to_bytes());
        Ok(codec)
    }
}

impl std::fmt::Debug for WitnessKeypair {
    /// Custom Debug that REDACTS the secret signing key. Only the public
    /// key (32 bytes, hex-encoded) is shown so logs and panic messages
    /// never leak signing material.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let pk = self.public_key_bytes();
        let mut hex = String::with_capacity(PUBLIC_KEY_LENGTH * 2);
        for byte in &pk {
            use std::fmt::Write;
            let _ = write!(&mut hex, "{byte:02x}");
        }
        f.debug_struct("WitnessKeypair")
            .field("public_key_hex", &hex)
            .field("signing_key", &"<redacted>")
            .finish()
    }
}

/// Verify a 96-byte signature codec against a segment. The public key
/// is recovered from the codec — callers that want to PIN a specific
/// public key should additionally compare `codec[..32]` against the
/// expected key (helper: `verify_segment_with_expected_pubkey`).
pub fn verify_segment(segment: &[u8], codec: &[u8]) -> Result<(), SignError> {
    validate_segment(segment)?;
    if codec.len() != SIGNATURE_CODEC_SIZE {
        return Err(SignError::CodecLengthInvalid {
            expected: SIGNATURE_CODEC_SIZE,
            actual: codec.len(),
        });
    }
    let mut pk_bytes = [0u8; PUBLIC_KEY_LENGTH];
    pk_bytes.copy_from_slice(&codec[0..PUBLIC_KEY_LENGTH]);
    let mut sig_bytes = [0u8; SIGNATURE_LENGTH];
    sig_bytes.copy_from_slice(&codec[PUBLIC_KEY_LENGTH..]);

    let verifying =
        VerifyingKey::from_bytes(&pk_bytes).map_err(|_| SignError::PublicKeyMalformed)?;
    if verifying.is_weak() {
        return Err(SignError::PublicKeyWeak);
    }
    let signature = Signature::from_bytes(&sig_bytes);
    let digest = digest_for_segment(pk_bytes, segment)?;
    verifying
        .verify_strict(&digest, &signature)
        .map_err(|_| SignError::VerificationFailed)
}

/// Verify and additionally pin the public key to an expected value.
/// Use this when callers know which key SHOULD have signed and want to
/// fail closed if the codec carries a different key (key-rotation drift,
/// accidental signer mix-up, malicious replay with a different key).
pub fn verify_segment_with_expected_pubkey(
    segment: &[u8],
    codec: &[u8],
    expected_pubkey: &[u8],
) -> Result<(), SignError> {
    if expected_pubkey.len() != PUBLIC_KEY_LENGTH {
        return Err(SignError::PublicKeyLengthInvalid {
            expected: PUBLIC_KEY_LENGTH,
            actual: expected_pubkey.len(),
        });
    }
    if codec.len() != SIGNATURE_CODEC_SIZE {
        return Err(SignError::CodecLengthInvalid {
            expected: SIGNATURE_CODEC_SIZE,
            actual: codec.len(),
        });
    }
    if &codec[0..PUBLIC_KEY_LENGTH] != expected_pubkey {
        return Err(SignError::PublicKeyMismatch);
    }
    verify_segment(segment, codec)
}

/// Compute the SHAKE-256 hash that gets signed. Exposed publicly so
/// independent verifiers can recompute it without depending on this
/// crate's signing code. Fails closed unless `segment` is a non-empty,
/// internally linked witness segment.
pub fn digest_for_segment(
    public_key: [u8; PUBLIC_KEY_LENGTH],
    segment: &[u8],
) -> Result<[u8; HASH_SIZE], SignError> {
    validate_segment(segment)?;
    let mut buf = Vec::with_capacity(DOMAIN_TAG.len() + PUBLIC_KEY_LENGTH + segment.len());
    buf.extend_from_slice(DOMAIN_TAG);
    buf.extend_from_slice(&public_key);
    buf.extend_from_slice(segment);
    Ok(shake256_32(&buf))
}

fn validate_segment(segment: &[u8]) -> Result<(), SignError> {
    if segment.is_empty() {
        return Err(SignError::SegmentEmpty);
    }
    if !segment.len().is_multiple_of(WITNESS_ENTRY_SIZE) {
        return Err(SignError::SegmentLengthInvalid {
            len: segment.len(),
            entry_size: WITNESS_ENTRY_SIZE,
        });
    }
    let entry_count = segment.len() / WITNESS_ENTRY_SIZE;
    let mut previous: Option<WitnessEntry> = None;
    for offset in 0..entry_count {
        let start = offset * WITNESS_ENTRY_SIZE;
        let entry = WitnessEntry::from_bytes(&segment[start..start + WITNESS_ENTRY_SIZE]).map_err(
            |_| SignError::SegmentLengthInvalid {
                len: segment.len(),
                entry_size: WITNESS_ENTRY_SIZE,
            },
        )?;
        if let Some(previous_entry) = &previous {
            let expected_prev_hash = previous_entry.chain_hash();
            if entry.prev_hash != expected_prev_hash {
                return Err(SignError::SegmentPrevHashMismatch {
                    offset,
                    expected_prev_hash,
                    actual_prev_hash: entry.prev_hash,
                });
            }
        }
        previous = Some(entry);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{WitnessEntry, HASH_SIZE, ZERO_HASH};

    fn fixed_seed(byte: u8) -> [u8; SECRET_KEY_LENGTH] {
        [byte; SECRET_KEY_LENGTH]
    }

    fn make_segment() -> Vec<u8> {
        let first = WitnessEntry::new(ZERO_HASH, [1u8; HASH_SIZE], 100, 1);
        let second = WitnessEntry::new(first.chain_hash(), [2u8; HASH_SIZE], 200, 2);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&first.to_bytes());
        bytes.extend_from_slice(&second.to_bytes());
        bytes
    }

    #[test]
    fn roundtrip_sign_then_verify() {
        let kp = WitnessKeypair::from_seed(&fixed_seed(0xAB)).unwrap();
        let segment = make_segment();
        let codec = kp.sign_segment(&segment).unwrap();
        verify_segment(&segment, &codec).expect("verification must pass");
    }

    #[test]
    fn rejects_seed_of_wrong_length() {
        let err = WitnessKeypair::from_seed(&[0u8; 16]).expect_err("short seed must error");
        assert_eq!(err.code(), "WITNESS_SEED_LENGTH_INVALID");
    }

    #[test]
    fn rejects_codec_of_wrong_length() {
        let segment = make_segment();
        let err = verify_segment(&segment, &[0u8; 32]).expect_err("short codec must error");
        assert_eq!(err.code(), "WITNESS_SIGNATURE_CODEC_LENGTH_INVALID");
    }

    #[test]
    fn rejects_tampered_segment_byte() {
        let kp = WitnessKeypair::from_seed(&fixed_seed(0x42)).unwrap();
        let mut segment = make_segment();
        let codec = kp.sign_segment(&segment).unwrap();
        segment[WITNESS_ENTRY_SIZE + HASH_SIZE] ^= 0x01;
        let err = verify_segment(&segment, &codec).expect_err("tamper must reject");
        assert_eq!(err.code(), "WITNESS_SIGNATURE_VERIFICATION_FAILED");
    }

    #[test]
    fn rejects_tampered_signature_byte() {
        let kp = WitnessKeypair::from_seed(&fixed_seed(0x99)).unwrap();
        let segment = make_segment();
        let mut codec = kp.sign_segment(&segment).unwrap();
        codec[PUBLIC_KEY_LENGTH + 5] ^= 0x01;
        let err = verify_segment(&segment, &codec).expect_err("tampered sig must reject");
        assert_eq!(err.code(), "WITNESS_SIGNATURE_VERIFICATION_FAILED");
    }

    #[test]
    fn rejects_wrong_public_key_in_codec() {
        let kp_a = WitnessKeypair::from_seed(&fixed_seed(0xAA)).unwrap();
        let kp_b = WitnessKeypair::from_seed(&fixed_seed(0xBB)).unwrap();
        let segment = make_segment();
        let mut codec = kp_a.sign_segment(&segment).unwrap();
        // Splice B's pubkey on top of A's signature; verification fails
        // because the digest is recomputed with B's pubkey but the
        // signature was made over A's pubkey-bound digest.
        codec[0..PUBLIC_KEY_LENGTH].copy_from_slice(&kp_b.public_key_bytes());
        let err = verify_segment(&segment, &codec).expect_err("pubkey swap must reject");
        assert_eq!(err.code(), "WITNESS_SIGNATURE_VERIFICATION_FAILED");
    }

    #[test]
    fn pinned_pubkey_rejects_when_mismatched() {
        let kp = WitnessKeypair::from_seed(&fixed_seed(0x55)).unwrap();
        let other = WitnessKeypair::from_seed(&fixed_seed(0x66)).unwrap();
        let segment = make_segment();
        let codec = kp.sign_segment(&segment).unwrap();
        let err = verify_segment_with_expected_pubkey(&segment, &codec, &other.public_key_bytes())
            .expect_err("pin mismatch must reject");
        assert_eq!(err.code(), "WITNESS_SIGNATURE_PUBKEY_MISMATCH");
    }

    #[test]
    fn pinned_pubkey_accepts_when_matched() {
        let kp = WitnessKeypair::from_seed(&fixed_seed(0x77)).unwrap();
        let segment = make_segment();
        let codec = kp.sign_segment(&segment).unwrap();
        verify_segment_with_expected_pubkey(&segment, &codec, &kp.public_key_bytes())
            .expect("matching pin must accept");
    }

    #[test]
    fn distinct_seeds_produce_distinct_pubkeys() {
        let a = WitnessKeypair::from_seed(&fixed_seed(0x01)).unwrap();
        let b = WitnessKeypair::from_seed(&fixed_seed(0x02)).unwrap();
        assert_ne!(a.public_key_bytes(), b.public_key_bytes());
    }

    #[test]
    fn signature_codec_size_is_96() {
        let kp = WitnessKeypair::from_seed(&fixed_seed(0x10)).unwrap();
        let segment = make_segment();
        let codec = kp.sign_segment(&segment).unwrap();
        assert_eq!(codec.len(), SIGNATURE_CODEC_SIZE);
        assert_eq!(SIGNATURE_CODEC_SIZE, 96);
    }

    #[test]
    fn rejects_wrong_length_pinned_pubkey() {
        let segment = make_segment();
        let kp = WitnessKeypair::from_seed(&fixed_seed(0x21)).unwrap();
        let codec = kp.sign_segment(&segment).unwrap();
        let err = verify_segment_with_expected_pubkey(&segment, &codec, &[0u8; 16])
            .expect_err("short pubkey must error");
        assert_eq!(err.code(), "WITNESS_PUBKEY_LENGTH_INVALID");
    }

    #[test]
    fn rejects_empty_segment() {
        let kp = WitnessKeypair::from_seed(&fixed_seed(0x33)).unwrap();
        let err = kp.sign_segment(&[]).expect_err("empty segment must error");
        assert_eq!(err.code(), "WITNESS_SIGNATURE_SEGMENT_EMPTY");
    }

    #[test]
    fn rejects_truncated_segment() {
        let kp = WitnessKeypair::from_seed(&fixed_seed(0x34)).unwrap();
        let err = kp
            .sign_segment(&[0u8; WITNESS_ENTRY_SIZE - 1])
            .expect_err("truncated segment must error");
        assert_eq!(err.code(), "WITNESS_SIGNATURE_SEGMENT_LENGTH_INVALID");
    }

    #[test]
    fn rejects_broken_internal_segment_link() {
        let kp = WitnessKeypair::from_seed(&fixed_seed(0x35)).unwrap();
        let mut segment = make_segment();
        segment[WITNESS_ENTRY_SIZE] ^= 0x01;
        let err = kp
            .sign_segment(&segment)
            .expect_err("broken segment link must error");
        assert_eq!(err.code(), "WITNESS_SIGNATURE_SEGMENT_PREV_HASH_MISMATCH");
    }

    #[test]
    fn digest_is_deterministic_per_pubkey_and_segment() {
        let kp = WitnessKeypair::from_seed(&fixed_seed(0x44)).unwrap();
        let segment = make_segment();
        let d1 = digest_for_segment(kp.public_key_bytes(), &segment).unwrap();
        let d2 = digest_for_segment(kp.public_key_bytes(), &segment).unwrap();
        assert_eq!(d1, d2);
    }
}

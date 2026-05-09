// Real synthetic-data tests for tier compression. No mocks.

use super::*;

/// SplitMix64 (matches the one in context-graph-mejepa-corpus / granger_fsv).
/// Pure-functional state for deterministic test inputs.
fn splitmix64(seed: u64) -> u64 {
    let mut z = seed.wrapping_add(0x9E3779B97F4A7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

/// Box-Muller-style pseudo-Gaussian samples in [-1, 1] range (clamped via tanh).
fn gaussian_sample(seed: u64) -> f32 {
    let r1 = splitmix64(seed.wrapping_mul(2)) >> 11;
    let r2 = splitmix64(seed.wrapping_mul(2).wrapping_add(1)) >> 11;
    let u1 = (r1 as f64 + 1.0) / ((1u64 << 53) as f64 + 1.0);
    let u2 = (r2 as f64 + 1.0) / ((1u64 << 53) as f64 + 1.0);
    let r = (-2.0 * u1.ln()).sqrt();
    let theta = 2.0 * std::f64::consts::PI * u2;
    let z = r * theta.cos();
    z.tanh() as f32
}

fn make_synthetic_vector(n: usize, seed: u64) -> Vec<f32> {
    (0..n)
        .map(|i| gaussian_sample(seed.wrapping_add(i as u64)))
        .collect()
}

fn max_abs_error(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).abs())
        .fold(0.0_f32, f32::max)
}

#[test]
fn bit_width_levels_match_powers_of_two() {
    assert_eq!(BitWidth::Eight.levels(), 256);
    assert_eq!(BitWidth::Seven.levels(), 128);
    assert_eq!(BitWidth::Five.levels(), 32);
    assert_eq!(BitWidth::Three.levels(), 8);
}

#[test]
fn round_trip_preserves_endpoints_at_every_width() {
    // The minimum value should map to q=0 → decoded back to exactly `min`.
    // The maximum value should map to q=L-1 → decoded back to exactly `max`.
    let values = vec![-1.0_f32, 0.0, 0.25, 0.5, 0.75, 1.0];
    for bits in BitWidth::all() {
        let blob = encode(&values, bits).unwrap();
        let back = decode(&blob).unwrap();
        assert_eq!(back.len(), values.len(), "len mismatch at {bits:?}");
        assert_eq!(back[0], -1.0, "min not exact at {bits:?}");
        assert_eq!(back[values.len() - 1], 1.0, "max not exact at {bits:?}");
    }
}

#[test]
fn round_trip_error_bound_holds_at_every_width_on_synthetic_768d() {
    let values = make_synthetic_vector(768, 42);
    for bits in BitWidth::all() {
        let blob = encode(&values, bits).unwrap();
        let back = decode(&blob).unwrap();
        let bound = max_reconstruction_error(blob.min, blob.max, bits);
        let err = max_abs_error(&values, &back);
        assert!(
            err <= bound + 1e-6,
            "{bits:?}: per-element error {err} exceeds bound {bound}"
        );
    }
}

#[test]
fn payload_size_matches_expected_compression_ratio_at_every_width() {
    let n = 512;
    let values = make_synthetic_vector(n, 7);
    for bits in BitWidth::all() {
        let blob = encode(&values, bits).unwrap();
        let expected = packed_len(n, bits.bits()).unwrap();
        assert_eq!(
            blob.data.len(),
            expected,
            "payload len mismatch at {bits:?}"
        );
    }
    // 8-bit: 512 bytes; 7-bit: 448 bytes; 5-bit: 320; 3-bit: 192.
    assert_eq!(packed_len(512, 8), Some(512));
    assert_eq!(packed_len(512, 7), Some(448));
    assert_eq!(packed_len(512, 5), Some(320));
    assert_eq!(packed_len(512, 3), Some(192));
    // Overflow case: n × bits exceeds usize::MAX.
    assert_eq!(packed_len(usize::MAX, 8), None);
}

#[test]
fn serialize_deserialize_round_trip_at_every_width() {
    let values = make_synthetic_vector(101, 9);
    for bits in BitWidth::all() {
        let blob = encode(&values, bits).unwrap();
        let bytes = serialize(&blob).unwrap();
        assert_eq!(bytes.len(), blob.serialized_len().unwrap());
        let back = deserialize(&bytes).unwrap();
        assert_eq!(back, blob, "blob round-trip mismatch at {bits:?}");
    }
}

#[test]
fn empty_input_rejects() {
    let err = encode(&[], BitWidth::Eight).unwrap_err();
    assert_eq!(err.code(), "TIER_COMPRESSION_INVALID_INPUT");
}

#[test]
fn nan_input_rejects() {
    let v = vec![1.0_f32, 2.0, f32::NAN, 4.0];
    let err = encode(&v, BitWidth::Eight).unwrap_err();
    assert_eq!(err.code(), "TIER_COMPRESSION_INVALID_INPUT");
}

#[test]
fn positive_infinity_input_rejects() {
    let v = vec![1.0_f32, f32::INFINITY, 3.0];
    let err = encode(&v, BitWidth::Eight).unwrap_err();
    assert_eq!(err.code(), "TIER_COMPRESSION_INVALID_INPUT");
}

#[test]
fn negative_infinity_input_rejects() {
    let v = vec![1.0_f32, f32::NEG_INFINITY, 3.0];
    let err = encode(&v, BitWidth::Eight).unwrap_err();
    assert_eq!(err.code(), "TIER_COMPRESSION_INVALID_INPUT");
}

#[test]
fn all_zeros_input_decodes_to_all_zeros() {
    let v = vec![0.0_f32; 32];
    for bits in BitWidth::all() {
        let blob = encode(&v, bits).unwrap();
        let back = decode(&blob).unwrap();
        for x in &back {
            assert_eq!(*x, 0.0, "all-zero decode failed at {bits:?}");
        }
    }
}

#[test]
fn single_value_input_round_trips() {
    let v = vec![3.125_f32];
    for bits in BitWidth::all() {
        let blob = encode(&v, bits).unwrap();
        let back = decode(&blob).unwrap();
        assert_eq!(back.len(), 1);
        // Single-value vector has min == max == value; range == 0; decoder
        // returns min for every q. Exact round trip.
        assert_eq!(back[0], v[0], "single-value round trip at {bits:?}");
    }
}

#[test]
fn deserialize_truncated_header_rejects() {
    let v = make_synthetic_vector(8, 1);
    let blob = encode(&v, BitWidth::Eight).unwrap();
    let bytes = serialize(&blob).unwrap();
    let truncated = &bytes[..10];
    let err = deserialize(truncated).unwrap_err();
    assert_eq!(err.code(), "TIER_COMPRESSION_INVALID_INPUT");
}

#[test]
fn deserialize_wrong_magic_rejects() {
    let v = make_synthetic_vector(8, 1);
    let blob = encode(&v, BitWidth::Eight).unwrap();
    let mut bytes = serialize(&blob).unwrap();
    bytes[0] ^= 0xFF;
    let err = deserialize(&bytes).unwrap_err();
    assert_eq!(err.code(), "TIER_COMPRESSION_INVALID_INPUT");
}

#[test]
fn deserialize_wrong_version_rejects() {
    let v = make_synthetic_vector(8, 1);
    let blob = encode(&v, BitWidth::Eight).unwrap();
    let mut bytes = serialize(&blob).unwrap();
    bytes[4] = 99;
    let err = deserialize(&bytes).unwrap_err();
    assert_eq!(err.code(), "TIER_COMPRESSION_INVALID_INPUT");
}

#[test]
fn deserialize_unsupported_bits_rejects() {
    let v = make_synthetic_vector(8, 1);
    let blob = encode(&v, BitWidth::Eight).unwrap();
    let mut bytes = serialize(&blob).unwrap();
    bytes[5] = 4;
    let err = deserialize(&bytes).unwrap_err();
    assert_eq!(err.code(), "TIER_COMPRESSION_INVALID_INPUT");
}

#[test]
fn deserialize_truncated_payload_rejects() {
    let v = make_synthetic_vector(64, 5);
    let blob = encode(&v, BitWidth::Five).unwrap();
    let bytes = serialize(&blob).unwrap();
    let truncated = &bytes[..bytes.len() - 5];
    let err = deserialize(truncated).unwrap_err();
    assert_eq!(err.code(), "TIER_COMPRESSION_INVALID_INPUT");
}

#[test]
fn deserialize_extra_payload_rejects() {
    let v = make_synthetic_vector(64, 5);
    let blob = encode(&v, BitWidth::Five).unwrap();
    let mut bytes = serialize(&blob).unwrap();
    bytes.push(0xA5);
    let err = deserialize(&bytes).unwrap_err();
    assert_eq!(err.code(), "TIER_COMPRESSION_INVALID_INPUT");
}

#[test]
fn decode_rejects_min_greater_than_max() {
    let mut blob = encode(&[0.0_f32, 1.0], BitWidth::Eight).unwrap();
    std::mem::swap(&mut blob.min, &mut blob.max);
    let err = decode(&blob).unwrap_err();
    assert_eq!(err.code(), "TIER_COMPRESSION_NUMERICAL_INVARIANT");
}

#[test]
fn decode_rejects_truncated_blob_data() {
    let v = make_synthetic_vector(64, 5);
    let mut blob = encode(&v, BitWidth::Five).unwrap();
    let original_len = blob.data.len();
    blob.data.truncate(original_len.saturating_sub(2));
    let err = decode(&blob).unwrap_err();
    assert_eq!(err.code(), "TIER_COMPRESSION_INVALID_INPUT");
}

#[test]
fn decode_rejects_extra_blob_data() {
    let v = make_synthetic_vector(64, 5);
    let mut blob = encode(&v, BitWidth::Five).unwrap();
    blob.data.push(0);
    let err = decode(&blob).unwrap_err();
    assert_eq!(err.code(), "TIER_COMPRESSION_INVALID_INPUT");
}

#[test]
fn serialize_rejects_noncanonical_blob_data() {
    let v = make_synthetic_vector(64, 5);
    let mut blob = encode(&v, BitWidth::Five).unwrap();
    blob.data.push(0);
    let err = serialize(&blob).unwrap_err();
    assert_eq!(err.code(), "TIER_COMPRESSION_INVALID_INPUT");
}

#[test]
fn three_bit_quantization_has_eight_distinct_levels() {
    // Span [-1, 1] uniformly with 64 values; under 3-bit there should be
    // at most 8 distinct decoded values.
    let values: Vec<f32> = (0..64).map(|i| -1.0 + (i as f32) * (2.0 / 63.0)).collect();
    let blob = encode(&values, BitWidth::Three).unwrap();
    let back = decode(&blob).unwrap();
    let mut distinct: Vec<f32> = back.clone();
    distinct.sort_by(|a, b| a.partial_cmp(b).unwrap());
    distinct.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    assert!(
        distinct.len() <= 8,
        "3-bit decode produced {} distinct levels, expected <= 8",
        distinct.len()
    );
}

#[test]
fn encoding_is_deterministic_for_identical_input() {
    let v = make_synthetic_vector(32, 11);
    for bits in BitWidth::all() {
        let a = encode(&v, bits).unwrap();
        let b = encode(&v, bits).unwrap();
        assert_eq!(a, b, "encoding non-deterministic at {bits:?}");
    }
}

#[test]
fn cosine_similarity_preserved_at_eight_bit() {
    // 8-bit should preserve cosine similarity to within ~0.99 on a 768-dim
    // Gaussian-like vector.
    let v = make_synthetic_vector(768, 13);
    let blob = encode(&v, BitWidth::Eight).unwrap();
    let back = decode(&blob).unwrap();
    let dot: f32 = v.iter().zip(back.iter()).map(|(a, b)| a * b).sum();
    let na: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = back.iter().map(|x| x * x).sum::<f32>().sqrt();
    let cos = dot / (na * nb);
    assert!(cos >= 0.99, "8-bit cosine similarity {cos} < 0.99");
}

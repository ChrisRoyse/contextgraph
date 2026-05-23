//! Text encoding for HDC.
//!
//! Provides text-to-hypervector encoding using character n-grams.
//!
//! ## Noise Tolerance Design (per Constitution E9)
//!
//! E9 (V_robustness) is designed for typo tolerance. The encoding uses a
//! "bag of n-grams" approach WITHOUT absolute position binding to ensure
//! that character insertions/deletions only affect local n-grams, not the
//! entire document vector.
//!
//! - Each n-gram captures relative character ordering (via permute within n-gram)
//! - N-grams are bundled position-independently (no absolute position)
//! - Result: ~5% character difference → ~5% bit difference → high similarity
//!
//! This makes "authentication" and "authetication" (typo) highly similar,
//! which is the core purpose of E9.

use super::operations::{bind, bundle, permute};
use super::types::{Hypervector, HDC_DIMENSION, HDC_PROJECTED_DIMENSION};
use bitvec::prelude::*;
use tracing::{debug, instrument, trace, warn};

const TEXT_IDENTITY_RESIDUAL_WEIGHT: f32 = 1.0 / 16.0;
const TEXT_IDENTITY_RESIDUAL_DOMAIN: u64 = 0xD6E8_FD9A_4F1C_2B73;
const FNV_OFFSET_BASIS: u64 = 14_695_981_039_346_656_037;
const FNV_PRIME: u64 = 1_099_511_628_211;

/// Generates a random hypervector with approximately 50% bits set.
///
/// Uses a deterministic PRNG seeded with the combined seed and input hash.
///
/// # Arguments
/// * `seed` - Base seed for the model
/// * `key` - Unique identifier for this hypervector (hashed with seed)
#[must_use]
pub fn random_hypervector(seed: u64, key: u64) -> Hypervector {
    let mut hv = bitvec![u64, Lsb0; 0; HDC_DIMENSION];
    let mut state = seed.wrapping_add(key);

    for i in 0..HDC_DIMENSION {
        // LCG PRNG: state = state * 6364136223846793005 + 1
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        // Use high bits for better distribution
        if (state >> 63) == 1 {
            hv.set(i, true);
        }
    }

    trace!(
        key = key,
        popcount = hv.count_ones(),
        "Generated random hypervector"
    );
    hv
}

/// Generates a hypervector for a character.
///
/// Characters are hashed deterministically to produce consistent vectors.
#[must_use]
pub fn char_hypervector(seed: u64, c: char) -> Hypervector {
    random_hypervector(seed, c as u64)
}

/// Generates a position permutation vector.
///
/// Position vectors enable positional encoding via binding.
#[must_use]
pub fn position_hypervector(seed: u64, position: usize) -> Hypervector {
    // Use a different seed space for positions
    random_hypervector(seed, 0x8000_0000_0000_0000 | (position as u64))
}

/// Encodes text into a hypervector using character n-grams.
///
/// ## Algorithm (Bag of N-grams for Noise Tolerance)
///
/// 1. Extract sliding window character n-grams
/// 2. For each n-gram:
///    - Bind character vectors with RELATIVE position shifts within the n-gram
///    - This preserves local character ordering (e.g., "abc" ≠ "cba")
/// 3. Bundle all n-gram vectors WITHOUT absolute position binding
///    - This makes encoding robust to insertions/deletions
///    - A typo only affects the 2-3 n-grams containing the error
///
/// ## Why No Absolute Position Binding (per Constitution E9)
///
/// E9 is designed for typo/noise tolerance. Binding with absolute position
/// makes the encoding sensitive to insertions/deletions:
/// - "authentication" at position 5 ≠ "authentication" at position 6
/// - A single character insertion shifts ALL subsequent positions
/// - This destroys the noise tolerance that is E9's core purpose
///
/// By removing absolute position, we get:
/// - "authentication" → same n-grams regardless of position in document
/// - "authetication" → differs in only 2-3 n-grams (the typo region)
/// - Result: high similarity despite the typo
///
/// # Arguments
/// * `seed` - Model seed for deterministic generation
/// * `ngram_size` - Size of character n-grams
/// * `text` - Text to encode
///
/// # Returns
/// Hypervector encoding of the text (bag of n-grams)
#[must_use]
#[instrument(skip(text), fields(text_len = text.len()))]
pub fn encode_text(seed: u64, ngram_size: usize, text: &str) -> Hypervector {
    let chars: Vec<char> = text.chars().collect();

    if chars.is_empty() {
        warn!("Encoding empty text");
        return bitvec![u64, Lsb0; 0; HDC_DIMENSION];
    }

    // For text shorter than ngram_size, use what we have
    let effective_ngram = ngram_size.min(chars.len());

    let mut ngram_vectors = Vec::new();

    for window_start in 0..=chars.len().saturating_sub(effective_ngram) {
        let mut ngram_hv = char_hypervector(seed, chars[window_start]);

        // Bind characters within n-gram with RELATIVE position shifts
        // This preserves the character ordering within each n-gram
        // e.g., "abc" produces different vector than "cba"
        for (pos, &c) in chars[window_start..]
            .iter()
            .take(effective_ngram)
            .enumerate()
            .skip(1)
        {
            let char_hv = char_hypervector(seed, c);
            let shifted = permute(&char_hv, pos);
            ngram_hv = bind(&ngram_hv, &shifted);
        }

        // NOTE: We intentionally DO NOT bind with absolute position.
        // This implements a "bag of n-grams" that is robust to typos.
        // A character insertion/deletion only affects 2-3 local n-grams,
        // not the entire document vector. This is E9's core value proposition.

        ngram_vectors.push(ngram_hv);
    }

    let result = bundle(&ngram_vectors);
    debug!(
        ngrams = ngram_vectors.len(),
        popcount = result.count_ones(),
        "Encoded text (bag of n-grams, no absolute position)"
    );
    result
}

/// Projects a hypervector to the fusion dimension (1024D float).
///
/// Binary to float conversion: Uses weighted averaging of bit chunks.
/// Then L2 normalized.
#[must_use]
pub fn project_to_float(hv: &Hypervector) -> Vec<f32> {
    // Project from 10K bits to 1024 floats
    // Since 10000 doesn't divide evenly by 1024, we use overlapping windows
    let hv_len = hv.len();
    let mut result = Vec::with_capacity(HDC_PROJECTED_DIMENSION);

    for i in 0..HDC_PROJECTED_DIMENSION {
        // Map output index to input bit range
        let start = (i * hv_len) / HDC_PROJECTED_DIMENSION;
        let end = ((i + 1) * hv_len) / HDC_PROJECTED_DIMENSION;
        let chunk_size = end - start;

        if chunk_size == 0 {
            result.push(0.0);
            continue;
        }

        let ones: usize = (start..end)
            .map(|j| if j < hv_len && hv[j] { 1 } else { 0 })
            .sum();
        // Map [0, chunk_size] to [-1, +1]
        let value = (2.0 * ones as f32 / chunk_size as f32) - 1.0;
        result.push(value);
    }

    normalize_projected_vector(&mut result);

    result
}

/// Adds a small deterministic full-text residual to split HDC projection ties.
///
/// The base HDC signal remains dominant; the residual only prevents distinct
/// texts that share the same projected bag-of-ngrams vector from becoming the
/// same durable cache vector.
pub fn apply_text_identity_residual(vector: &mut [f32], seed: u64, text: &str) {
    if vector.is_empty() {
        return;
    }
    let identity = text_identity_hash(seed, text);
    let inv_sqrt_dim = 1.0 / (vector.len() as f32).sqrt();
    for (idx, value) in vector.iter_mut().enumerate() {
        let bit = splitmix64(identity ^ (idx as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let residual = if bit & 1 == 0 {
            -inv_sqrt_dim
        } else {
            inv_sqrt_dim
        };
        *value += TEXT_IDENTITY_RESIDUAL_WEIGHT * residual;
    }
    normalize_projected_vector(vector);
}

fn normalize_projected_vector(vector: &mut [f32]) {
    let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for value in vector.iter_mut() {
            *value /= norm;
        }
    }
}

fn text_identity_hash(seed: u64, text: &str) -> u64 {
    let mut hash =
        FNV_OFFSET_BASIS ^ seed.wrapping_mul(TEXT_IDENTITY_RESIDUAL_DOMAIN) ^ (text.len() as u64);
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

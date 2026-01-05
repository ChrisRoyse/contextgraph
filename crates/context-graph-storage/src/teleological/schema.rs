//! Key format functions for teleological storage.
//!
//! All keys use fixed-size formats for efficient range scans.
//! No variable-length prefixes.
//!
//! # FAIL FAST Policy
//!
//! Key parsing functions panic on invalid input. This ensures:
//! 1. Data corruption is immediately detected
//! 2. No silent degradation of data integrity
//! 3. Clear error messages with full context

use uuid::Uuid;

/// Key for fingerprints CF: UUID as 16 bytes.
///
/// # Arguments
/// * `id` - The fingerprint's UUID
///
/// # Returns
/// Exactly 16 bytes (UUID in big-endian format)
#[inline]
pub fn fingerprint_key(id: &Uuid) -> [u8; 16] {
    *id.as_bytes()
}

/// Key for purpose_vectors CF: UUID as 16 bytes.
///
/// # Arguments
/// * `id` - The fingerprint's UUID
///
/// # Returns
/// Exactly 16 bytes (UUID in big-endian format)
#[inline]
pub fn purpose_vector_key(id: &Uuid) -> [u8; 16] {
    *id.as_bytes()
}

/// Key for e13_splade_inverted CF: term_id as 2 bytes (big-endian).
///
/// # Arguments
/// * `term_id` - The SPLADE vocabulary term index (0..30522)
///
/// # Returns
/// Exactly 2 bytes (u16 in big-endian format)
#[inline]
pub fn e13_splade_inverted_key(term_id: u16) -> [u8; 2] {
    term_id.to_be_bytes()
}

/// Key for e1_matryoshka_128 CF: UUID as 16 bytes.
///
/// # Arguments
/// * `id` - The fingerprint's UUID
///
/// # Returns
/// Exactly 16 bytes (UUID in big-endian format)
#[inline]
pub fn e1_matryoshka_128_key(id: &Uuid) -> [u8; 16] {
    *id.as_bytes()
}

/// Parse fingerprint key back to UUID.
///
/// # Arguments
/// * `key` - Exactly 16 bytes
///
/// # Returns
/// The parsed UUID
///
/// # Panics
/// Panics if key is not exactly 16 bytes (FAIL FAST).
/// Error message includes:
/// - Actual key length
/// - Key data (for debugging)
/// - Context about what went wrong
#[inline]
pub fn parse_fingerprint_key(key: &[u8]) -> Uuid {
    if key.len() != 16 {
        panic!(
            "STORAGE ERROR: fingerprint key must be 16 bytes, got {} bytes. \
             Key data: {:02x?}. This indicates corrupted storage or wrong CF access.",
            key.len(),
            key
        );
    }
    Uuid::from_slice(key).unwrap_or_else(|e| {
        panic!(
            "STORAGE ERROR: Invalid UUID bytes in fingerprint key. \
             Error: {}. Key data: {:02x?}. This should never happen with valid 16-byte input.",
            e, key
        );
    })
}

/// Parse E13 SPLADE inverted key back to term_id.
///
/// # Arguments
/// * `key` - Exactly 2 bytes
///
/// # Returns
/// The parsed term_id (u16)
///
/// # Panics
/// Panics if key is not exactly 2 bytes (FAIL FAST).
#[inline]
pub fn parse_e13_splade_key(key: &[u8]) -> u16 {
    if key.len() != 2 {
        panic!(
            "STORAGE ERROR: e13_splade key must be 2 bytes, got {} bytes. \
             Key data: {:02x?}. This indicates corrupted storage or wrong CF access.",
            key.len(),
            key
        );
    }
    u16::from_be_bytes([key[0], key[1]])
}

/// Parse purpose vector key back to UUID.
///
/// # Arguments
/// * `key` - Exactly 16 bytes
///
/// # Returns
/// The parsed UUID
///
/// # Panics
/// Panics if key is not exactly 16 bytes (FAIL FAST).
#[inline]
pub fn parse_purpose_vector_key(key: &[u8]) -> Uuid {
    if key.len() != 16 {
        panic!(
            "STORAGE ERROR: purpose_vector key must be 16 bytes, got {} bytes. \
             Key data: {:02x?}. This indicates corrupted storage or wrong CF access.",
            key.len(),
            key
        );
    }
    Uuid::from_slice(key).unwrap_or_else(|e| {
        panic!(
            "STORAGE ERROR: Invalid UUID bytes in purpose_vector key. \
             Error: {}. Key data: {:02x?}.",
            e, key
        );
    })
}

/// Parse E1 Matryoshka 128D key back to UUID.
///
/// # Arguments
/// * `key` - Exactly 16 bytes
///
/// # Returns
/// The parsed UUID
///
/// # Panics
/// Panics if key is not exactly 16 bytes (FAIL FAST).
#[inline]
pub fn parse_e1_matryoshka_key(key: &[u8]) -> Uuid {
    if key.len() != 16 {
        panic!(
            "STORAGE ERROR: e1_matryoshka_128 key must be 16 bytes, got {} bytes. \
             Key data: {:02x?}. This indicates corrupted storage or wrong CF access.",
            key.len(),
            key
        );
    }
    Uuid::from_slice(key).unwrap_or_else(|e| {
        panic!(
            "STORAGE ERROR: Invalid UUID bytes in e1_matryoshka_128 key. \
             Error: {}. Key data: {:02x?}.",
            e, key
        );
    })
}

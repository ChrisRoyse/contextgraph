//! Key format functions for teleological storage.
//!
//! All keys use fixed-size formats for efficient range scans.
//! No variable-length prefixes (except teleological_profiles which uses string keys).
//!
//! # Key Formats (TASK-TELEO-006, TASK-CONTENT-001)
//!
//! | CF | Key Format | Size |
//! |----|------------|------|
//! | synergy_matrix | "synergy" (singleton) | 7 bytes |
//! | teleological_profiles | profile_id string | variable (1-255 bytes) |
//! | teleological_vectors | memory_id UUID | 16 bytes |
//! | content | fingerprint_id UUID | 16 bytes |
//!
//! # FAIL FAST Policy
//!
//! Key parsing functions panic on invalid input. This ensures:
//! 1. Data corruption is immediately detected
//! 2. No silent degradation of data integrity
//! 3. Clear error messages with full context

use uuid::Uuid;

// =============================================================================
// TASK-TELEO-006: SINGLETON KEY CONSTANTS
// =============================================================================

/// Singleton key for synergy_matrix CF.
/// Fixed 7-byte string "synergy".
pub const SYNERGY_MATRIX_KEY: &[u8] = b"synergy";

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

// =============================================================================
// TASK-TELEO-006: TELEOLOGICAL PROFILE KEYS (variable-length string)
// =============================================================================

/// Create key for teleological_profiles CF.
///
/// # Arguments
/// * `profile_id` - The profile identifier string (1-255 bytes)
///
/// # Returns
/// The profile_id as UTF-8 bytes
///
/// # Panics
/// Panics if profile_id is empty or longer than 255 bytes (FAIL FAST).
#[inline]
pub fn teleological_profile_key(profile_id: &str) -> Vec<u8> {
    if profile_id.is_empty() {
        panic!(
            "STORAGE ERROR: teleological_profile_key cannot be empty. \
             Profile IDs must be non-empty strings."
        );
    }
    if profile_id.len() > 255 {
        panic!(
            "STORAGE ERROR: teleological_profile_key too long: {} bytes (max 255). \
             Profile ID: '{}'...",
            profile_id.len(),
            &profile_id[..64.min(profile_id.len())]
        );
    }
    profile_id.as_bytes().to_vec()
}

/// Parse teleological_profile key back to profile_id string.
///
/// # Arguments
/// * `key` - UTF-8 bytes (1-255 bytes)
///
/// # Returns
/// The profile_id as String
///
/// # Panics
/// Panics if key is empty, too long, or contains invalid UTF-8 (FAIL FAST).
#[inline]
pub fn parse_teleological_profile_key(key: &[u8]) -> String {
    if key.is_empty() {
        panic!(
            "STORAGE ERROR: teleological_profile key cannot be empty. \
             This indicates corrupted storage or wrong CF access."
        );
    }
    if key.len() > 255 {
        panic!(
            "STORAGE ERROR: teleological_profile key too long: {} bytes (max 255). \
             Key data: {:02x?}...",
            key.len(),
            &key[..64.min(key.len())]
        );
    }
    String::from_utf8(key.to_vec()).unwrap_or_else(|e| {
        panic!(
            "STORAGE ERROR: Invalid UTF-8 in teleological_profile key. \
             Error: {}. Key data: {:02x?}.",
            e, key
        );
    })
}

// =============================================================================
// TASK-TELEO-006: TELEOLOGICAL VECTOR KEYS (UUID = 16 bytes)
// =============================================================================

/// Key for teleological_vectors CF: memory_id UUID as 16 bytes.
///
/// # Arguments
/// * `memory_id` - The memory's UUID
///
/// # Returns
/// Exactly 16 bytes (UUID in big-endian format)
#[inline]
pub fn teleological_vector_key(memory_id: &Uuid) -> [u8; 16] {
    *memory_id.as_bytes()
}

/// Parse teleological_vector key back to memory_id UUID.
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
pub fn parse_teleological_vector_key(key: &[u8]) -> Uuid {
    if key.len() != 16 {
        panic!(
            "STORAGE ERROR: teleological_vector key must be 16 bytes, got {} bytes. \
             Key data: {:02x?}. This indicates corrupted storage or wrong CF access.",
            key.len(),
            key
        );
    }
    Uuid::from_slice(key).unwrap_or_else(|e| {
        panic!(
            "STORAGE ERROR: Invalid UUID bytes in teleological_vector key. \
             Error: {}. Key data: {:02x?}.",
            e, key
        );
    })
}

// =============================================================================
// TASK-CONTENT-002: CONTENT KEYS (UUID = 16 bytes)
// =============================================================================

/// Key for content CF: fingerprint_id UUID as 16 bytes.
///
/// Used to store original text content associated with a fingerprint.
/// Same key format as fingerprint_key for consistency.
///
/// # Arguments
/// * `id` - The fingerprint's UUID
///
/// # Returns
/// Exactly 16 bytes (UUID in big-endian format)
#[inline]
pub fn content_key(id: &Uuid) -> [u8; 16] {
    *id.as_bytes()
}

/// Parse content key back to fingerprint UUID.
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
pub fn parse_content_key(key: &[u8]) -> Uuid {
    if key.len() != 16 {
        panic!(
            "STORAGE ERROR: content key must be 16 bytes, got {} bytes. \
             Key data: {:02x?}. This indicates corrupted storage or wrong CF access.",
            key.len(),
            key
        );
    }
    Uuid::from_slice(key).unwrap_or_else(|e| {
        panic!(
            "STORAGE ERROR: Invalid UUID bytes in content key. \
             Error: {}. Key data: {:02x?}.",
            e, key
        );
    })
}

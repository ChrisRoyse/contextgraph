//! Key format functions for teleological storage.
//!
//! All keys use fixed-size formats for efficient range scans.
//! No variable-length prefixes (except teleological_profiles which uses string keys).
//!
//! # Key Formats (TASK-TELEO-006, TASK-CONTENT-001, TASK-SESSION-04)
//!
//! | CF | Key Format | Size |
//! |----|------------|------|
//! | synergy_matrix | "synergy" (singleton) | 7 bytes |
//! | teleological_profiles | profile_id string | variable (1-255 bytes) |
//! | teleological_vectors | memory_id UUID | 16 bytes |
//! | content | fingerprint_id UUID | 16 bytes |
//! | session_identity | `s:{session_id}` | 2 + variable |
//! | session_identity | `latest` (pointer) | 6 bytes |
//! | session_identity | `t:{timestamp_ms_be}` | 10 bytes |
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

/// Key for topic profile storage (CF_TOPIC_PROFILES): UUID as 16 bytes.
///
/// # Arguments
/// * `id` - The fingerprint's UUID
///
/// # Returns
/// Exactly 16 bytes (UUID in big-endian format)
#[inline]
pub fn topic_profile_key(id: &Uuid) -> [u8; 16] {
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

/// Key for e6_sparse_inverted CF: term_id as 2 bytes (big-endian).
///
/// Used for E6 (V_selectivity) sparse inverted index to enable:
/// - Stage 1: Dual sparse recall with E13 SPLADE
/// - Stage 3.5: Tie-breaker for close E1 scores
///
/// # Arguments
/// * `term_id` - The BERT vocabulary term index (0..30522, same vocab as E13)
///
/// # Returns
/// Exactly 2 bytes (u16 in big-endian format)
#[inline]
pub fn e6_sparse_inverted_key(term_id: u16) -> [u8; 2] {
    term_id.to_be_bytes()
}

/// Parse E6 sparse inverted key back to term_id.
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
pub fn parse_e6_sparse_key(key: &[u8]) -> u16 {
    if key.len() != 2 {
        panic!(
            "STORAGE ERROR: e6_sparse key must be 2 bytes, got {} bytes. \
             Key data: {:02x?}. This indicates corrupted storage or wrong CF access.",
            key.len(),
            key
        );
    }
    u16::from_be_bytes([key[0], key[1]])
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

/// Parse topic profile key back to UUID.
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
pub fn parse_topic_profile_key(key: &[u8]) -> Uuid {
    if key.len() != 16 {
        panic!(
            "STORAGE ERROR: topic_profile key must be 16 bytes, got {} bytes. \
             Key data: {:02x?}. This indicates corrupted storage or wrong CF access.",
            key.len(),
            key
        );
    }
    Uuid::from_slice(key).unwrap_or_else(|e| {
        panic!(
            "STORAGE ERROR: Invalid UUID bytes in topic_profile key. \
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

// =============================================================================
// SOURCE METADATA KEYS (UUID = 16 bytes)
// =============================================================================

/// Key for source_metadata CF: fingerprint_id UUID as 16 bytes.
///
/// Used to store provenance information (source type, file path, etc.)
/// associated with a fingerprint.
///
/// # Arguments
/// * `id` - The fingerprint's UUID
///
/// # Returns
/// Exactly 16 bytes (UUID in big-endian format)
#[inline]
pub fn source_metadata_key(id: &Uuid) -> [u8; 16] {
    *id.as_bytes()
}

/// Parse source_metadata key back to fingerprint UUID.
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
pub fn parse_source_metadata_key(key: &[u8]) -> Uuid {
    if key.len() != 16 {
        panic!(
            "STORAGE ERROR: source_metadata key must be 16 bytes, got {} bytes. \
             Key data: {:02x?}. This indicates corrupted storage or wrong CF access.",
            key.len(),
            key
        );
    }
    Uuid::from_slice(key).unwrap_or_else(|e| {
        panic!(
            "STORAGE ERROR: Invalid UUID bytes in source_metadata key. \
             Error: {}. Key data: {:02x?}.",
            e, key
        );
    })
}

// =============================================================================
// TASK-STORAGE-P2-001: E12 LATE INTERACTION TOKEN KEYS (UUID = 16 bytes)
// =============================================================================

/// Key for e12_late_interaction CF: memory_id UUID as 16 bytes.
///
/// Used to store ColBERT token embeddings for MaxSim scoring in Stage 5.
///
/// # Arguments
/// * `id` - The memory's UUID
///
/// # Returns
/// Exactly 16 bytes (UUID in big-endian format)
#[inline]
pub fn e12_late_interaction_key(id: &Uuid) -> [u8; 16] {
    *id.as_bytes()
}

/// Parse e12_late_interaction key back to memory UUID.
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
pub fn parse_e12_late_interaction_key(key: &[u8]) -> Uuid {
    if key.len() != 16 {
        panic!(
            "STORAGE ERROR: e12_late_interaction key must be 16 bytes, got {} bytes. \
             Key data: {:02x?}. This indicates corrupted storage or wrong CF access.",
            key.len(),
            key
        );
    }
    Uuid::from_slice(key).unwrap_or_else(|e| {
        panic!(
            "STORAGE ERROR: Invalid UUID bytes in e12_late_interaction key. \
             Error: {}. Key data: {:02x?}.",
            e, key
        );
    })
}

// =============================================================================
// TASK-SESSION-04: SESSION KEY HELPERS
// =============================================================================

/// Create temporal index key: `t:{timestamp_ms}` (big-endian for lexicographic ordering)
///
/// Big-endian encoding ensures that keys sort chronologically when iterated.
/// This enables efficient temporal range queries.
///
/// # Arguments
/// * `timestamp_ms` - Unix milliseconds timestamp
///
/// # Returns
/// Key bytes: `t:` prefix (2 bytes) + 8-byte big-endian timestamp (10 bytes total)
///
/// # Example
/// ```ignore
/// let k1 = session_temporal_key(1000);
/// let k2 = session_temporal_key(2000);
/// assert!(k1 < k2); // Lexicographic ordering matches numeric
/// ```
#[inline]
pub fn session_temporal_key(timestamp_ms: i64) -> Vec<u8> {
    let mut key = Vec::with_capacity(10);
    key.extend_from_slice(b"t:");
    key.extend_from_slice(&timestamp_ms.to_be_bytes());
    key
}

/// Parse timestamp from session_temporal_key.
///
/// # Arguments
/// * `key` - Exactly 10 bytes: `t:` prefix + 8-byte big-endian timestamp
///
/// # Returns
/// The parsed timestamp in milliseconds
///
/// # Panics
/// Panics if key doesn't start with `t:` or isn't exactly 10 bytes - FAIL FAST policy.
#[inline]
pub fn parse_session_temporal_key(key: &[u8]) -> i64 {
    assert_eq!(
        key.len(),
        10,
        "STORAGE ERROR: session_temporal_key must be exactly 10 bytes, got {} bytes. \
         Key data: {:02x?}. Expected: 2-byte prefix 't:' + 8-byte timestamp.",
        key.len(),
        key
    );
    assert_eq!(
        &key[0..2],
        b"t:",
        "STORAGE ERROR: session_temporal_key must start with 't:'. \
         Got: {:02x?}. This indicates corrupted storage or wrong CF access.",
        &key[0..2]
    );
    i64::from_be_bytes(
        key[2..10]
            .try_into()
            .expect("timestamp bytes slice is exactly 8 bytes"),
    )
}

// =============================================================================
// CAUSAL RELATIONSHIP KEYS (UUID = 16 bytes)
// =============================================================================

/// Key for causal_relationships CF: causal_relationship_id UUID as 16 bytes.
///
/// Used to store LLM-generated causal relationship descriptions with embeddings.
///
/// # Arguments
/// * `id` - The causal relationship's UUID
///
/// # Returns
/// Exactly 16 bytes (UUID in big-endian format)
#[inline]
pub fn causal_relationship_key(id: &Uuid) -> [u8; 16] {
    *id.as_bytes()
}

/// Parse causal_relationship key back to UUID.
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
pub fn parse_causal_relationship_key(key: &[u8]) -> Uuid {
    if key.len() != 16 {
        panic!(
            "STORAGE ERROR: causal_relationship key must be 16 bytes, got {} bytes. \
             Key data: {:02x?}. This indicates corrupted storage or wrong CF access.",
            key.len(),
            key
        );
    }
    Uuid::from_slice(key).unwrap_or_else(|e| {
        panic!(
            "STORAGE ERROR: Invalid UUID bytes in causal_relationship key. \
             Error: {}. Key data: {:02x?}.",
            e, key
        );
    })
}

/// Key for causal_by_source CF: source_fingerprint_id UUID as 16 bytes.
///
/// Used for secondary index enabling "find all causal relationships from memory X".
///
/// # Arguments
/// * `source_id` - The source fingerprint's UUID
///
/// # Returns
/// Exactly 16 bytes (UUID in big-endian format)
#[inline]
pub fn causal_by_source_key(source_id: &Uuid) -> [u8; 16] {
    *source_id.as_bytes()
}

/// Parse causal_by_source key back to source fingerprint UUID.
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
pub fn parse_causal_by_source_key(key: &[u8]) -> Uuid {
    if key.len() != 16 {
        panic!(
            "STORAGE ERROR: causal_by_source key must be 16 bytes, got {} bytes. \
             Key data: {:02x?}. This indicates corrupted storage or wrong CF access.",
            key.len(),
            key
        );
    }
    Uuid::from_slice(key).unwrap_or_else(|e| {
        panic!(
            "STORAGE ERROR: Invalid UUID bytes in causal_by_source key. \
             Error: {}. Key data: {:02x?}.",
            e, key
        );
    })
}

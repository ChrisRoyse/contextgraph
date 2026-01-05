//! Bincode serialization for TeleologicalFingerprint.
//!
//! Uses bincode 1.3 for efficient binary serialization.
//! Expected serialized size: ~63KB per fingerprint (based on SemanticFingerprint with 15,120 dense dims).
//!
//! # FAIL FAST Policy
//!
//! All serialization functions panic on error. This ensures:
//! 1. Data corruption is immediately detected
//! 2. No partial or corrupted data is stored
//! 3. Clear error messages with full context
//!
//! # Version Handling
//!
//! Each serialized fingerprint is prefixed with a version byte.
//! Version mismatches cause immediate panic (no migration support).

use bincode::{deserialize, serialize};
use context_graph_core::types::fingerprint::TeleologicalFingerprint;
use uuid::Uuid;

/// Serialization version for TeleologicalFingerprint.
///
/// Bump this when struct layout changes. Version mismatches will panic.
pub const TELEOLOGICAL_VERSION: u8 = 1;

/// Minimum expected size for a serialized TeleologicalFingerprint.
///
/// Based on actual SemanticFingerprint size:
/// - TOTAL_DENSE_DIMS = 15,120 → 60,480 bytes for dense embeddings
/// - Plus sparse vectors, JohariFingerprint, PurposeVector, metadata
/// - Expected total: ~63KB
const MIN_FINGERPRINT_SIZE: usize = 55_000;

/// Maximum expected size for a serialized TeleologicalFingerprint.
///
/// With 100 evolution snapshots (MAX_EVOLUTION_SNAPSHOTS), size could grow significantly.
/// Each snapshot contains PurposeVector (52B) + JohariFingerprint (~520B) + trigger + timestamp.
/// Allowing up to 150KB for fully evolved fingerprints.
const MAX_FINGERPRINT_SIZE: usize = 150_000;

/// Serialize TeleologicalFingerprint to bytes.
///
/// # Arguments
/// * `fp` - The TeleologicalFingerprint to serialize
///
/// # Returns
/// ~63KB byte vector containing:
/// - 1 byte: version
/// - N bytes: bincode-encoded TeleologicalFingerprint
///
/// # Panics
/// - Panics if bincode serialization fails (indicates struct incompatibility)
/// - Panics if serialized size is outside [55KB, 150KB] range (indicates missing data or oversized evolution)
///
/// # Example
/// ```ignore
/// use context_graph_storage::teleological::serialize_teleological_fingerprint;
///
/// let fp = TeleologicalFingerprint::new(...);
/// let bytes = serialize_teleological_fingerprint(&fp);
/// assert!(bytes.len() >= 55_000 && bytes.len() <= 150_000);
/// ```
pub fn serialize_teleological_fingerprint(fp: &TeleologicalFingerprint) -> Vec<u8> {
    let mut result = Vec::with_capacity(65_000); // Pre-allocate ~65KB (actual ~63KB)
    result.push(TELEOLOGICAL_VERSION);

    let encoded = serialize(fp).unwrap_or_else(|e| {
        panic!(
            "SERIALIZATION ERROR: Failed to serialize TeleologicalFingerprint. \
             Error: {}. Fingerprint ID: {:?}. \
             This indicates struct incompatibility with bincode 1.3. \
             Check that all fields implement Serialize correctly.",
            e, fp.id
        );
    });

    result.extend(encoded);

    // Verify size is in expected range
    let size = result.len();
    if size < MIN_FINGERPRINT_SIZE || size > MAX_FINGERPRINT_SIZE {
        panic!(
            "SERIALIZATION ERROR: TeleologicalFingerprint size {} bytes outside expected range \
             [{}, {}]. Fingerprint ID: {:?}. Evolution snapshots: {}. \
             This indicates missing or corrupted embeddings, or unexpectedly large evolution history.",
            size,
            MIN_FINGERPRINT_SIZE,
            MAX_FINGERPRINT_SIZE,
            fp.id,
            fp.purpose_evolution.len()
        );
    }

    result
}

/// Deserialize TeleologicalFingerprint from bytes.
///
/// # Arguments
/// * `data` - Serialized bytes (from `serialize_teleological_fingerprint`)
///
/// # Returns
/// The deserialized TeleologicalFingerprint
///
/// # Panics
/// - Panics if data is empty
/// - Panics if version mismatch (no migration support)
/// - Panics if deserialization fails (indicates corruption)
///
/// # Example
/// ```ignore
/// use context_graph_storage::teleological::{
///     serialize_teleological_fingerprint,
///     deserialize_teleological_fingerprint,
/// };
///
/// let original = TeleologicalFingerprint::new(...);
/// let bytes = serialize_teleological_fingerprint(&original);
/// let restored = deserialize_teleological_fingerprint(&bytes);
/// assert_eq!(original.id, restored.id);
/// ```
pub fn deserialize_teleological_fingerprint(data: &[u8]) -> TeleologicalFingerprint {
    if data.is_empty() {
        panic!(
            "DESERIALIZATION ERROR: Empty data for TeleologicalFingerprint. \
             This indicates missing fingerprint or wrong CF lookup. \
             Verify the key exists in the fingerprints column family."
        );
    }

    let version = data[0];
    if version != TELEOLOGICAL_VERSION {
        panic!(
            "DESERIALIZATION ERROR: Version mismatch. Expected {}, got {}. \
             Data length: {} bytes. \
             This indicates stale data requiring migration. \
             No automatic migration is supported - data must be regenerated.",
            TELEOLOGICAL_VERSION,
            version,
            data.len()
        );
    }

    deserialize(&data[1..]).unwrap_or_else(|e| {
        panic!(
            "DESERIALIZATION ERROR: Failed to deserialize TeleologicalFingerprint. \
             Error: {}. Data length: {} bytes, version: {}. \
             This indicates corrupted storage or incompatible struct changes. \
             Check that TeleologicalFingerprint struct matches the stored version.",
            e,
            data.len(),
            version
        );
    })
}

/// Serialize purpose vector (13D × f32 = 52 bytes).
///
/// # Arguments
/// * `vector` - The 13-element purpose alignment vector
///
/// # Returns
/// Exactly 52 bytes (13 × 4 bytes per f32, little-endian)
///
/// # Example
/// ```ignore
/// let purpose = [0.5f32; 13];
/// let bytes = serialize_purpose_vector(&purpose);
/// assert_eq!(bytes.len(), 52);
/// ```
pub fn serialize_purpose_vector(vector: &[f32; 13]) -> [u8; 52] {
    let mut result = [0u8; 52];
    for (i, &v) in vector.iter().enumerate() {
        let bytes = v.to_le_bytes();
        result[i * 4..(i + 1) * 4].copy_from_slice(&bytes);
    }
    result
}

/// Deserialize purpose vector from 52 bytes.
///
/// # Arguments
/// * `data` - Exactly 52 bytes
///
/// # Returns
/// The 13-element purpose alignment vector
///
/// # Panics
/// Panics if data is not exactly 52 bytes.
pub fn deserialize_purpose_vector(data: &[u8]) -> [f32; 13] {
    if data.len() != 52 {
        panic!(
            "DESERIALIZATION ERROR: Purpose vector must be 52 bytes, got {}. \
             Data: {:02x?}. \
             This indicates corrupted storage or wrong CF lookup. \
             Verify the key points to the purpose_vectors column family.",
            data.len(),
            if data.len() <= 20 { data } else { &data[..20] }
        );
    }

    let mut result = [0.0f32; 13];
    for i in 0..13 {
        let bytes: [u8; 4] = data[i * 4..(i + 1) * 4]
            .try_into()
            .expect("slice is exactly 4 bytes");
        result[i] = f32::from_le_bytes(bytes);
    }
    result
}

/// Serialize E1 Matryoshka 128D vector (128 × f32 = 512 bytes).
///
/// # Arguments
/// * `vector` - The 128-element truncated E1 embedding
///
/// # Returns
/// Exactly 512 bytes (128 × 4 bytes per f32, little-endian)
pub fn serialize_e1_matryoshka_128(vector: &[f32; 128]) -> [u8; 512] {
    let mut result = [0u8; 512];
    for (i, &v) in vector.iter().enumerate() {
        let bytes = v.to_le_bytes();
        result[i * 4..(i + 1) * 4].copy_from_slice(&bytes);
    }
    result
}

/// Deserialize E1 Matryoshka 128D vector from 512 bytes.
///
/// # Arguments
/// * `data` - Exactly 512 bytes
///
/// # Returns
/// The 128-element truncated E1 embedding
///
/// # Panics
/// Panics if data is not exactly 512 bytes.
pub fn deserialize_e1_matryoshka_128(data: &[u8]) -> [f32; 128] {
    if data.len() != 512 {
        panic!(
            "DESERIALIZATION ERROR: E1 Matryoshka 128D vector must be 512 bytes, got {}. \
             This indicates corrupted storage or wrong CF lookup. \
             Verify the key points to the e1_matryoshka_128 column family.",
            data.len()
        );
    }

    let mut result = [0.0f32; 128];
    for i in 0..128 {
        let bytes: [u8; 4] = data[i * 4..(i + 1) * 4]
            .try_into()
            .expect("slice is exactly 4 bytes");
        result[i] = f32::from_le_bytes(bytes);
    }
    result
}

/// Serialize memory ID list for E13 SPLADE inverted index.
///
/// Format:
/// - 4 bytes: count (u32, little-endian)
/// - N × 16 bytes: UUIDs
///
/// # Arguments
/// * `ids` - Vector of memory IDs that contain the term
///
/// # Returns
/// Serialized bytes (4 + ids.len() × 16 bytes)
pub fn serialize_memory_id_list(ids: &[Uuid]) -> Vec<u8> {
    let mut result = Vec::with_capacity(ids.len() * 16 + 4);
    result.extend(&(ids.len() as u32).to_le_bytes());
    for id in ids {
        result.extend(id.as_bytes());
    }
    result
}

/// Deserialize memory ID list from E13 SPLADE inverted index.
///
/// # Arguments
/// * `data` - Serialized bytes (from `serialize_memory_id_list`)
///
/// # Returns
/// Vector of memory IDs
///
/// # Panics
/// - Panics if data is less than 4 bytes (missing count)
/// - Panics if data length doesn't match count × 16 + 4
/// - Panics if any UUID is invalid
pub fn deserialize_memory_id_list(data: &[u8]) -> Vec<Uuid> {
    if data.len() < 4 {
        panic!(
            "DESERIALIZATION ERROR: Memory ID list must have at least 4 bytes (count), got {}. \
             Data: {:02x?}. \
             This indicates corrupted storage or truncated write.",
            data.len(),
            data
        );
    }

    let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let expected_len = 4 + count * 16;

    if data.len() != expected_len {
        panic!(
            "DESERIALIZATION ERROR: Memory ID list with {} entries should be {} bytes, got {}. \
             This indicates corrupted storage or partial write. \
             Count bytes: {:02x?}, Total data length: {}.",
            count,
            expected_len,
            data.len(),
            &data[0..4],
            data.len()
        );
    }

    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let start = 4 + i * 16;
        let uuid = Uuid::from_slice(&data[start..start + 16]).unwrap_or_else(|e| {
            panic!(
                "DESERIALIZATION ERROR: Invalid UUID at index {} in memory ID list. \
                 Error: {}. Bytes: {:02x?}. \
                 This indicates data corruption.",
                i,
                e,
                &data[start..start + 16]
            );
        });
        result.push(uuid);
    }
    result
}

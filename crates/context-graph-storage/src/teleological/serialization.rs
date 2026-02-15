//! Bincode serialization for TeleologicalFingerprint.
//!
//! Uses bincode 1.3 for efficient binary serialization.
//! Expected serialized size: ~30KB per fingerprint (based on SemanticFingerprint with 7,424 dense dims).
//!
//! # Error Handling Policy (STG-01)
//!
//! Serialization functions (write path) panic on error because the data originates
//! from in-memory structs we control. Deserialization functions (read path) return
//! `Result<T, CoreError>` so that a single corrupted record in RocksDB does NOT
//! crash the entire server during iteration.
//!
//! # Version Handling
//!
//! Each serialized type is prefixed with a version byte.
//! Version mismatches return errors (no migration support).

use bincode::{deserialize, serialize};
use context_graph_core::error::CoreError;
use context_graph_core::types::fingerprint::TeleologicalFingerprint;
use uuid::Uuid;

/// Serialization version for TeleologicalFingerprint.
///
/// Bump this when struct layout changes. Version mismatches will panic.
pub const TELEOLOGICAL_VERSION: u8 = 1;

/// Minimum expected size for a serialized TeleologicalFingerprint.
///
/// Based on actual SemanticFingerprint size:
/// - TOTAL_DENSE_DIMS = 7,424 → 29,696 bytes for dense embeddings (f32)
/// - Plus sparse vectors and metadata
/// - Bincode may compress zeros efficiently, so actual size varies
/// - Using conservative minimum of 5KB to allow for heavy compression
const MIN_FINGERPRINT_SIZE: usize = 5_000;

/// Maximum expected size for a serialized TeleologicalFingerprint.
///
/// Maximum-size fingerprints with many sparse entries and tokens can exceed 100KB:
/// - 2000 E6 sparse entries: ~12KB (2000 * 6 bytes)
/// - 1500 E13 SPLADE entries: ~9KB (1500 * 6 bytes)
/// - 100 E12 tokens: ~51KB (100 * 128 * 4 bytes)
/// - Dense embeddings: ~30KB
///
/// Total maximum: ~102-110KB
/// Allowing up to 150KB for edge cases with heavy sparse/token usage.
const MAX_FINGERPRINT_SIZE: usize = 150_000;

/// Serialize TeleologicalFingerprint to bytes.
///
/// # Arguments
/// * `fp` - The TeleologicalFingerprint to serialize
///
/// # Returns
/// ~30KB byte vector containing:
/// - 1 byte: version
/// - N bytes: bincode-encoded TeleologicalFingerprint
///
/// # Panics
/// - Panics if bincode serialization fails (indicates struct incompatibility)
/// - Panics if serialized size is outside [5KB, 150KB] range (indicates missing data or oversized evolution)
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
    if !(MIN_FINGERPRINT_SIZE..=MAX_FINGERPRINT_SIZE).contains(&size) {
        panic!(
            "SERIALIZATION ERROR: TeleologicalFingerprint size {} bytes outside expected range \
             [{}, {}]. Fingerprint ID: {:?}. \
             This indicates missing or corrupted embeddings.",
            size, MIN_FINGERPRINT_SIZE, MAX_FINGERPRINT_SIZE, fp.id,
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
/// `Ok(TeleologicalFingerprint)` on success, `Err(CoreError::SerializationError)` on failure.
///
/// # Errors
/// - Empty data
/// - Version mismatch (no migration support)
/// - Bincode deserialization failure (indicates corruption)
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
/// let restored = deserialize_teleological_fingerprint(&bytes)?;
/// assert_eq!(original.id, restored.id);
/// ```
pub fn deserialize_teleological_fingerprint(data: &[u8]) -> Result<TeleologicalFingerprint, CoreError> {
    if data.is_empty() {
        return Err(CoreError::SerializationError(
            "Empty data for TeleologicalFingerprint. \
             This indicates missing fingerprint or wrong CF lookup. \
             Verify the key exists in the fingerprints column family."
                .to_string(),
        ));
    }

    let version = data[0];
    if version != TELEOLOGICAL_VERSION {
        return Err(CoreError::SerializationError(format!(
            "Version mismatch for TeleologicalFingerprint. Expected {}, got {}. \
             Data length: {} bytes. \
             This indicates stale data requiring migration. \
             No automatic migration is supported - data must be regenerated.",
            TELEOLOGICAL_VERSION,
            version,
            data.len()
        )));
    }

    let mut fp: TeleologicalFingerprint = deserialize(&data[1..]).map_err(|e| {
        CoreError::SerializationError(format!(
            "Failed to deserialize TeleologicalFingerprint. \
             Error: {}. Data length: {} bytes, version: {}. \
             This indicates corrupted storage or incompatible struct changes.",
            e,
            data.len(),
            version
        ))
    })?;

    // DAT-3 fix: Migrate legacy single-vector E5/E8 to dual asymmetric format.
    // Without this, asymmetric causal search returns identical results for
    // cause vs effect on pre-migration data.
    fp.semantic.migrate_legacy_e5();
    fp.semantic.migrate_legacy_e8();

    Ok(fp)
}

/// Serialize topic profile (13D × f32 = 52 bytes).
///
/// # Arguments
/// * `vector` - The 13-element topic profile vector
///
/// # Returns
/// Exactly 52 bytes (13 × 4 bytes per f32, little-endian)
///
/// # Example
/// ```ignore
/// let profile = [0.5f32; 13];
/// let bytes = serialize_topic_profile(&profile);
/// assert_eq!(bytes.len(), 52);
/// ```
pub fn serialize_topic_profile(vector: &[f32; 13]) -> [u8; 52] {
    let mut result = [0u8; 52];
    for (i, &v) in vector.iter().enumerate() {
        let bytes = v.to_le_bytes();
        result[i * 4..(i + 1) * 4].copy_from_slice(&bytes);
    }
    result
}

/// Deserialize topic profile from 52 bytes.
///
/// # Arguments
/// * `data` - Exactly 52 bytes
///
/// # Returns
/// `Ok([f32; 13])` on success, `Err(CoreError::SerializationError)` on failure.
///
/// # Errors
/// Returns error if data is not exactly 52 bytes.
pub fn deserialize_topic_profile(data: &[u8]) -> Result<[f32; 13], CoreError> {
    if data.len() != 52 {
        return Err(CoreError::SerializationError(format!(
            "Topic profile must be 52 bytes, got {}. \
             Data prefix: {:02x?}. \
             This indicates corrupted storage or wrong CF lookup.",
            data.len(),
            if data.len() <= 20 { data } else { &data[..20] }
        )));
    }

    let mut result = [0.0f32; 13];
    for i in 0..13 {
        let bytes: [u8; 4] = data[i * 4..(i + 1) * 4]
            .try_into()
            .expect("slice is exactly 4 bytes");
        result[i] = f32::from_le_bytes(bytes);
    }
    Ok(result)
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
/// `Ok(Vec<Uuid>)` on success, `Err(CoreError::SerializationError)` on failure.
///
/// # Errors
/// - Data is less than 4 bytes (missing count)
/// - Data length doesn't match count x 16 + 4
/// - Any UUID is invalid
pub fn deserialize_memory_id_list(data: &[u8]) -> Result<Vec<Uuid>, CoreError> {
    if data.len() < 4 {
        return Err(CoreError::SerializationError(format!(
            "Memory ID list must have at least 4 bytes (count), got {}. \
             Data: {:02x?}. \
             This indicates corrupted storage or truncated write.",
            data.len(),
            data
        )));
    }

    let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let expected_len = 4 + count * 16;

    if data.len() != expected_len {
        return Err(CoreError::SerializationError(format!(
            "Memory ID list with {} entries should be {} bytes, got {}. \
             This indicates corrupted storage or partial write. \
             Count bytes: {:02x?}, Total data length: {}.",
            count,
            expected_len,
            data.len(),
            &data[0..4],
            data.len()
        )));
    }

    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let start = 4 + i * 16;
        let uuid = Uuid::from_slice(&data[start..start + 16]).map_err(|e| {
            CoreError::SerializationError(format!(
                "Invalid UUID at index {} in memory ID list. \
                 Error: {}. Bytes: {:02x?}. \
                 This indicates data corruption.",
                i,
                e,
                &data[start..start + 16]
            ))
        })?;
        result.push(uuid);
    }
    Ok(result)
}

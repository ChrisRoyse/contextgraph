# Task: TASK-F004 - Implement RocksDB Storage Schema for 46KB TeleologicalFingerprints

## Metadata
- **ID**: TASK-F004
- **Layer**: Foundation
- **Priority**: P0 (Critical Path)
- **Estimated Effort**: L (Large)
- **Dependencies**: TASK-F001 (COMPLETE), TASK-F002 (COMPLETE), TASK-F003 (COMPLETE)
- **Traces To**: TS-201, TS-203, FR-301, FR-304
- **Status**: ✅ COMPLETE (2025-01-05)

---

## ⚠️ CRITICAL: NO BACKWARDS COMPATIBILITY

**FAIL FAST. NO FALLBACKS. ROBUST ERROR LOGGING.**

This task implements NEW storage schema extensions. Any error MUST:
1. Immediately panic/return error with full context
2. Log the exact failure point with stack trace
3. Never silently fall back to default behavior
4. Never use mock data or placeholder values

---

## Codebase Audit - Current State (2025-01-05)

### ✅ ALREADY IMPLEMENTED (DO NOT RECREATE)

**Files that exist and ARE the source of truth:**

| File | Purpose | Status |
|------|---------|--------|
| `crates/context-graph-storage/src/column_families.rs` | 12 column families per constitution.yaml | ✅ COMPLETE (552 lines) |
| `crates/context-graph-storage/src/serialization.rs` | MessagePack for nodes, bincode for edges | ✅ COMPLETE (1139 lines) |
| `crates/context-graph-storage/src/lib.rs` | Re-exports, module structure | ✅ COMPLETE |
| `crates/context-graph-storage/Cargo.toml` | Dependencies: rocksdb 0.22, bincode 1.3, rmp-serde 1.1 | ✅ COMPLETE |
| `crates/context-graph-core/src/types/fingerprint/semantic.rs` | SemanticFingerprint with 13 embedders (E1-E13) | ✅ COMPLETE |
| `crates/context-graph-core/src/types/fingerprint/teleological.rs` | TeleologicalFingerprint (~46KB) | ✅ COMPLETE |
| `crates/context-graph-core/src/types/fingerprint/johari.rs` | JohariFingerprint with soft classification | ✅ COMPLETE |

### ✅ IMPLEMENTED (2025-01-05)

| Component | Description | Files |
|-----------|-------------|-------|
| Teleological serialization | Serialize/deserialize ~63KB TeleologicalFingerprint | `crates/context-graph-storage/src/teleological/serialization.rs` |
| 5-Stage Pipeline indexes | E13 SPLADE inverted index, E1 Matryoshka 128D index | `crates/context-graph-storage/src/teleological/schema.rs` |
| Extended column families | 4 new CFs for teleological storage | `crates/context-graph-storage/src/teleological/column_families.rs` |
| Unit tests | 34 tests including edge cases and panic tests | `crates/context-graph-storage/src/teleological/tests.rs` |
| Integration tests | 8 tests with real RocksDB | `crates/context-graph-storage/tests/teleological_integration.rs` |

---

## Description

Extend the existing RocksDB storage layer with teleological fingerprint support. The current storage has 12 column families for basic node/edge storage. This task adds 4 NEW column families specifically for ~63KB TeleologicalFingerprints and 5-stage pipeline indexing.

> **Note**: Actual serialized size is ~63KB due to SemanticFingerprint having 15,120 dense dimensions (TOTAL_DENSE_DIMS = 60,480 bytes for f32 values).

### Storage Architecture Extension

**Existing 12 CFs (DO NOT MODIFY):**
```
nodes, edges, embeddings, metadata,
johari_open, johari_hidden, johari_blind, johari_unknown,
temporal, tags, sources, system
```

**NEW 4 CFs to add (THIS TASK):**
```
fingerprints          - Primary ~63KB TeleologicalFingerprints
purpose_vectors       - 13D purpose vectors (52 bytes)
e13_splade_inverted   - Inverted index for E13 SPLADE sparse vectors
e1_matryoshka_128     - Secondary index for E1 Matryoshka 128D vectors
```

**Total after implementation: 16 column families**

---

## Source of Truth Definitions

| Data Type | Source of Truth | Verification Method |
|-----------|-----------------|---------------------|
| TeleologicalFingerprint struct | `crates/context-graph-core/src/types/fingerprint/teleological.rs` | `cargo doc -p context-graph-core` |
| SemanticFingerprint struct | `crates/context-graph-core/src/types/fingerprint/semantic.rs` | `cargo doc -p context-graph-core` |
| JohariFingerprint struct | `crates/context-graph-core/src/types/fingerprint/johari.rs` | `cargo doc -p context-graph-core` |
| Existing column families | `crates/context-graph-storage/src/column_families.rs:71-84` | Count = 12 |
| Existing serialization | `crates/context-graph-storage/src/serialization.rs` | MessagePack for MemoryNode |
| 256MB shared cache | `crates/context-graph-storage/src/column_families.rs:24` | Constitution.yaml |

---

## Acceptance Criteria

### Must Pass (Blocking)

- [x] 4 new column family definitions added to storage layer
- [x] `serialize_teleological_fingerprint()` produces ~63KB output (validated in tests)
- [x] `deserialize_teleological_fingerprint()` round-trips without data loss
- [x] E13 SPLADE inverted index stores term_id → memory_id mappings
- [x] E1 Matryoshka 128D index stores truncated vectors (512 bytes each)
- [x] All serialization uses bincode 1.3 (NOT bincode 2.0)
- [x] LZ4 compression enabled on fingerprints CF
- [x] 64KB block size for ~63KB fingerprints
- [x] All tests use REAL data (no mocks)
- [x] All errors panic with full context (no fallbacks)

### Must Not (Violations) - VERIFIED

- [x] ❌ Do NOT create mock fingerprints in tests - VERIFIED: Uses create_real_* helpers
- [x] ❌ Do NOT use Option::unwrap_or_default() - VERIFIED: grep found 0 occurrences
- [x] ❌ Do NOT catch errors silently - VERIFIED: All errors panic with context
- [x] ❌ Do NOT modify existing 12 column families - VERIFIED: 12 base CFs unchanged
- [x] ❌ Do NOT use bincode 2.0 (use 1.3) - VERIFIED: Cargo.toml uses bincode 1.3

---

## Implementation Steps

### Step 1: Create Teleological Module Structure

**File: `crates/context-graph-storage/src/teleological/mod.rs`**

```rust
//! Teleological fingerprint storage extensions.
//!
//! Adds 4 column families for 46KB TeleologicalFingerprint storage
//! and 5-stage pipeline indexing.

pub mod column_families;
pub mod schema;
pub mod serialization;

pub use column_families::*;
pub use schema::*;
pub use serialization::*;
```

**Verification:**
```bash
# Execute & Inspect
ls -la crates/context-graph-storage/src/teleological/
# Expected: mod.rs, column_families.rs, schema.rs, serialization.rs
```

### Step 2: Define New Column Families

**File: `crates/context-graph-storage/src/teleological/column_families.rs`**

```rust
//! Extended column families for teleological fingerprint storage.
//!
//! These 4 CFs extend the base 12 CFs defined in ../column_families.rs.
//! Total after integration: 16 column families.

use rocksdb::{BlockBasedOptions, Cache, ColumnFamilyDescriptor, Options};

/// Column family for 46KB TeleologicalFingerprints.
pub const CF_FINGERPRINTS: &str = "fingerprints";

/// Column family for 13D purpose vectors (52 bytes each).
pub const CF_PURPOSE_VECTORS: &str = "purpose_vectors";

/// Column family for E13 SPLADE inverted index.
/// Key: term_id (u16) → Value: Vec<Uuid>
pub const CF_E13_SPLADE_INVERTED: &str = "e13_splade_inverted";

/// Column family for E1 Matryoshka 128D truncated vectors.
/// Key: UUID (16 bytes) → Value: 128 × f32 = 512 bytes
pub const CF_E1_MATRYOSHKA_128: &str = "e1_matryoshka_128";

/// All teleological column family names (4 total).
pub const TELEOLOGICAL_CFS: &[&str] = &[
    CF_FINGERPRINTS,
    CF_PURPOSE_VECTORS,
    CF_E13_SPLADE_INVERTED,
    CF_E1_MATRYOSHKA_128,
];

/// Options for 46KB fingerprint storage.
/// - 64KB block size (fits one fingerprint per block)
/// - LZ4 compression
/// - Bloom filter for point lookups
pub fn fingerprint_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_block_size(64 * 1024); // 64KB for 46KB fingerprints
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.create_if_missing(true);
    // FAIL FAST: No fallback options
    opts
}

/// Options for 52-byte purpose vectors.
/// - Small block size (4KB)
/// - Bloom filter for fast lookups
/// - No compression (too small to benefit)
pub fn purpose_vector_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::None); // 52 bytes, no compression
    opts.optimize_for_point_lookup(64); // 64MB hint
    opts.create_if_missing(true);
    opts
}

/// Options for E13 SPLADE inverted index.
/// - Prefix bloom filter on term_id
/// - LZ4 compression (memory_id lists can be large)
pub fn e13_splade_inverted_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.create_if_missing(true);
    opts
}

/// Options for E1 Matryoshka 128D index (512 bytes per vector).
/// - 4KB block size (fits ~8 vectors per block)
/// - Bloom filter for fast lookups
pub fn e1_matryoshka_128_cf_options(cache: &Cache) -> Options {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_block_size(4 * 1024); // 4KB blocks
    block_opts.set_bloom_filter(10.0, false);
    block_opts.set_cache_index_and_filter_blocks(true);

    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.create_if_missing(true);
    opts
}

/// Get all 4 teleological column family descriptors.
pub fn get_teleological_cf_descriptors(cache: &Cache) -> Vec<ColumnFamilyDescriptor> {
    vec![
        ColumnFamilyDescriptor::new(CF_FINGERPRINTS, fingerprint_cf_options(cache)),
        ColumnFamilyDescriptor::new(CF_PURPOSE_VECTORS, purpose_vector_cf_options(cache)),
        ColumnFamilyDescriptor::new(CF_E13_SPLADE_INVERTED, e13_splade_inverted_cf_options(cache)),
        ColumnFamilyDescriptor::new(CF_E1_MATRYOSHKA_128, e1_matryoshka_128_cf_options(cache)),
    ]
}
```

**Verification:**
```bash
# Execute & Inspect
cargo check -p context-graph-storage 2>&1 | head -20
# Expected: no errors
```

### Step 3: Implement Key Format Functions

**File: `crates/context-graph-storage/src/teleological/schema.rs`**

```rust
//! Key format functions for teleological storage.
//!
//! All keys use fixed-size formats for efficient range scans.
//! No variable-length prefixes.

use uuid::Uuid;

/// Key for fingerprints CF: UUID as 16 bytes.
#[inline]
pub fn fingerprint_key(id: &Uuid) -> [u8; 16] {
    *id.as_bytes()
}

/// Key for purpose_vectors CF: UUID as 16 bytes.
#[inline]
pub fn purpose_vector_key(id: &Uuid) -> [u8; 16] {
    *id.as_bytes()
}

/// Key for e13_splade_inverted CF: term_id as 2 bytes (big-endian).
#[inline]
pub fn e13_splade_inverted_key(term_id: u16) -> [u8; 2] {
    term_id.to_be_bytes()
}

/// Key for e1_matryoshka_128 CF: UUID as 16 bytes.
#[inline]
pub fn e1_matryoshka_128_key(id: &Uuid) -> [u8; 16] {
    *id.as_bytes()
}

/// Parse fingerprint key back to UUID.
///
/// # Panics
/// Panics if key is not exactly 16 bytes (FAIL FAST).
#[inline]
pub fn parse_fingerprint_key(key: &[u8]) -> Uuid {
    if key.len() != 16 {
        panic!(
            "STORAGE ERROR: fingerprint key must be 16 bytes, got {} bytes. \
             Key data: {:?}. This indicates corrupted storage or wrong CF access.",
            key.len(),
            key
        );
    }
    Uuid::from_slice(key).expect("Invalid UUID bytes in fingerprint key")
}

/// Parse E13 SPLADE inverted key back to term_id.
///
/// # Panics
/// Panics if key is not exactly 2 bytes (FAIL FAST).
#[inline]
pub fn parse_e13_splade_key(key: &[u8]) -> u16 {
    if key.len() != 2 {
        panic!(
            "STORAGE ERROR: e13_splade key must be 2 bytes, got {} bytes. \
             Key data: {:?}. This indicates corrupted storage or wrong CF access.",
            key.len(),
            key
        );
    }
    u16::from_be_bytes([key[0], key[1]])
}
```

**Verification:**
```bash
# Execute & Inspect
cargo test -p context-graph-storage teleological::schema 2>&1 | tail -20
# Expected: all tests pass
```

### Step 4: Implement Serialization

**File: `crates/context-graph-storage/src/teleological/serialization.rs`**

```rust
//! Bincode serialization for TeleologicalFingerprint.
//!
//! Uses bincode 1.3 for efficient binary serialization.
//! Expected serialized size: ~46KB per fingerprint.

use bincode::{deserialize, serialize};
use context_graph_core::types::fingerprint::{
    JohariFingerprint, SemanticFingerprint, TeleologicalFingerprint,
};
use uuid::Uuid;

/// Serialization version for TeleologicalFingerprint.
/// Bump this when struct layout changes.
pub const TELEOLOGICAL_VERSION: u8 = 1;

/// Serialize TeleologicalFingerprint to bytes.
///
/// # Returns
/// ~46KB byte vector containing:
/// - 1 byte: version
/// - N bytes: bincode-encoded TeleologicalFingerprint
///
/// # Panics
/// Panics if serialization fails (FAIL FAST - indicates struct incompatibility).
pub fn serialize_teleological_fingerprint(fp: &TeleologicalFingerprint) -> Vec<u8> {
    let mut result = Vec::with_capacity(48_000); // Pre-allocate ~48KB
    result.push(TELEOLOGICAL_VERSION);

    let encoded = serialize(fp).unwrap_or_else(|e| {
        panic!(
            "SERIALIZATION ERROR: Failed to serialize TeleologicalFingerprint. \
             Error: {}. Memory ID: {:?}. This indicates struct incompatibility with bincode.",
            e,
            fp.memory_id
        );
    });

    result.extend(encoded);

    // Verify size is in expected range (40KB - 55KB)
    let size = result.len();
    if size < 40_000 || size > 55_000 {
        panic!(
            "SERIALIZATION ERROR: TeleologicalFingerprint size {} bytes outside expected range \
             [40000, 55000]. Memory ID: {:?}. This indicates missing or corrupted embeddings.",
            size,
            fp.memory_id
        );
    }

    result
}

/// Deserialize TeleologicalFingerprint from bytes.
///
/// # Panics
/// - Panics if version mismatch (FAIL FAST - no migration support)
/// - Panics if deserialization fails (FAIL FAST - indicates corruption)
pub fn deserialize_teleological_fingerprint(data: &[u8]) -> TeleologicalFingerprint {
    if data.is_empty() {
        panic!(
            "DESERIALIZATION ERROR: Empty data for TeleologicalFingerprint. \
             This indicates missing fingerprint or wrong CF lookup."
        );
    }

    let version = data[0];
    if version != TELEOLOGICAL_VERSION {
        panic!(
            "DESERIALIZATION ERROR: Version mismatch. Expected {}, got {}. \
             Data length: {} bytes. This indicates stale data requiring migration.",
            TELEOLOGICAL_VERSION,
            version,
            data.len()
        );
    }

    deserialize(&data[1..]).unwrap_or_else(|e| {
        panic!(
            "DESERIALIZATION ERROR: Failed to deserialize TeleologicalFingerprint. \
             Error: {}. Data length: {} bytes, version: {}. This indicates corrupted storage.",
            e,
            data.len(),
            version
        );
    })
}

/// Serialize purpose vector (13D × f32 = 52 bytes).
///
/// # Panics
/// Panics if vector is not exactly 13 elements.
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
/// # Panics
/// Panics if data is not exactly 52 bytes.
pub fn deserialize_purpose_vector(data: &[u8]) -> [f32; 13] {
    if data.len() != 52 {
        panic!(
            "DESERIALIZATION ERROR: Purpose vector must be 52 bytes, got {}. \
             This indicates corrupted storage or wrong CF lookup.",
            data.len()
        );
    }

    let mut result = [0.0f32; 13];
    for i in 0..13 {
        let bytes: [u8; 4] = data[i * 4..(i + 1) * 4].try_into().unwrap();
        result[i] = f32::from_le_bytes(bytes);
    }
    result
}

/// Serialize E1 Matryoshka 128D vector (128 × f32 = 512 bytes).
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
/// # Panics
/// Panics if data is not exactly 512 bytes.
pub fn deserialize_e1_matryoshka_128(data: &[u8]) -> [f32; 128] {
    if data.len() != 512 {
        panic!(
            "DESERIALIZATION ERROR: E1 Matryoshka 128D vector must be 512 bytes, got {}. \
             This indicates corrupted storage or wrong CF lookup.",
            data.len()
        );
    }

    let mut result = [0.0f32; 128];
    for i in 0..128 {
        let bytes: [u8; 4] = data[i * 4..(i + 1) * 4].try_into().unwrap();
        result[i] = f32::from_le_bytes(bytes);
    }
    result
}

/// Serialize memory ID list for E13 SPLADE inverted index.
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
/// # Panics
/// Panics if data is malformed.
pub fn deserialize_memory_id_list(data: &[u8]) -> Vec<Uuid> {
    if data.len() < 4 {
        panic!(
            "DESERIALIZATION ERROR: Memory ID list must have at least 4 bytes (count), got {}.",
            data.len()
        );
    }

    let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let expected_len = 4 + count * 16;

    if data.len() != expected_len {
        panic!(
            "DESERIALIZATION ERROR: Memory ID list with {} entries should be {} bytes, got {}.",
            count,
            expected_len,
            data.len()
        );
    }

    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let start = 4 + i * 16;
        let uuid = Uuid::from_slice(&data[start..start + 16]).unwrap_or_else(|e| {
            panic!(
                "DESERIALIZATION ERROR: Invalid UUID at index {} in memory ID list. Error: {}.",
                i, e
            );
        });
        result.push(uuid);
    }
    result
}
```

### Step 5: Update lib.rs

**File: `crates/context-graph-storage/src/lib.rs`**

Add to existing file (DO NOT REPLACE):
```rust
// Add after line 22:
pub mod teleological;

// Add to re-exports section:
pub use teleological::{
    CF_FINGERPRINTS, CF_PURPOSE_VECTORS, CF_E13_SPLADE_INVERTED, CF_E1_MATRYOSHKA_128,
    TELEOLOGICAL_CFS, get_teleological_cf_descriptors,
    fingerprint_key, purpose_vector_key, e13_splade_inverted_key, e1_matryoshka_128_key,
    serialize_teleological_fingerprint, deserialize_teleological_fingerprint,
    serialize_purpose_vector, deserialize_purpose_vector,
    serialize_e1_matryoshka_128, deserialize_e1_matryoshka_128,
    serialize_memory_id_list, deserialize_memory_id_list,
    TELEOLOGICAL_VERSION,
};
```

---

## Testing Requirements

### ⚠️ CRITICAL: NO MOCK DATA

All tests MUST use REAL data constructed from actual struct definitions.

### Unit Tests (Real Data Only)

**File: `crates/context-graph-storage/src/teleological/tests.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_core::types::fingerprint::{
        SemanticFingerprint, TeleologicalFingerprint, JohariFingerprint,
    };
    use uuid::Uuid;

    /// Create REAL TeleologicalFingerprint for testing.
    /// NO MOCKS - uses actual struct construction.
    fn create_real_fingerprint() -> TeleologicalFingerprint {
        let memory_id = Uuid::new_v4();

        // Create real SemanticFingerprint with all 13 embedders
        let semantic = SemanticFingerprint::new(memory_id);

        // Create real JohariFingerprint
        let johari = JohariFingerprint::new(memory_id);

        // Create real TeleologicalFingerprint
        TeleologicalFingerprint::new(memory_id, semantic, johari)
    }

    #[test]
    fn test_serialize_teleological_roundtrip() {
        println!("=== TEST: TeleologicalFingerprint serialization round-trip ===");

        let original = create_real_fingerprint();
        println!("BEFORE: Created real fingerprint with ID: {}", original.memory_id);
        println!("  - SemanticFingerprint embedders: 13");
        println!("  - JohariFingerprint quadrants: 4");

        let serialized = serialize_teleological_fingerprint(&original);
        println!("SERIALIZED: {} bytes", serialized.len());

        let deserialized = deserialize_teleological_fingerprint(&serialized);
        println!("AFTER: Deserialized fingerprint ID: {}", deserialized.memory_id);

        assert_eq!(original.memory_id, deserialized.memory_id);
        println!("RESULT: PASS - Round-trip preserved memory_id");
    }

    #[test]
    fn test_fingerprint_size_in_range() {
        println!("=== TEST: Serialized size within 40-55KB range ===");

        let fp = create_real_fingerprint();
        let serialized = serialize_teleological_fingerprint(&fp);

        println!("BEFORE: Expected range [40000, 55000] bytes");
        println!("AFTER: Actual size {} bytes", serialized.len());

        assert!(serialized.len() >= 40_000, "Size {} below minimum 40KB", serialized.len());
        assert!(serialized.len() <= 55_000, "Size {} above maximum 55KB", serialized.len());
        println!("RESULT: PASS - Size in expected range");
    }

    #[test]
    fn test_purpose_vector_roundtrip() {
        println!("=== TEST: Purpose vector (13D) round-trip ===");

        let original: [f32; 13] = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3];
        println!("BEFORE: {:?}", original);

        let serialized = serialize_purpose_vector(&original);
        assert_eq!(serialized.len(), 52);
        println!("SERIALIZED: {} bytes", serialized.len());

        let deserialized = deserialize_purpose_vector(&serialized);
        println!("AFTER: {:?}", deserialized);

        for i in 0..13 {
            assert!((original[i] - deserialized[i]).abs() < 1e-6);
        }
        println!("RESULT: PASS - All 13 dimensions preserved");
    }

    #[test]
    fn test_e1_matryoshka_roundtrip() {
        println!("=== TEST: E1 Matryoshka 128D vector round-trip ===");

        let mut original = [0.0f32; 128];
        for i in 0..128 {
            original[i] = (i as f32) * 0.01;
        }
        println!("BEFORE: 128D vector, first 5 elements: {:?}", &original[..5]);

        let serialized = serialize_e1_matryoshka_128(&original);
        assert_eq!(serialized.len(), 512);
        println!("SERIALIZED: {} bytes", serialized.len());

        let deserialized = deserialize_e1_matryoshka_128(&serialized);
        println!("AFTER: first 5 elements: {:?}", &deserialized[..5]);

        for i in 0..128 {
            assert!((original[i] - deserialized[i]).abs() < 1e-6);
        }
        println!("RESULT: PASS - All 128 dimensions preserved");
    }

    #[test]
    fn test_memory_id_list_roundtrip() {
        println!("=== TEST: Memory ID list round-trip ===");

        let original: Vec<Uuid> = (0..10).map(|_| Uuid::new_v4()).collect();
        println!("BEFORE: {} UUIDs", original.len());
        for (i, id) in original.iter().enumerate() {
            println!("  [{}]: {}", i, id);
        }

        let serialized = serialize_memory_id_list(&original);
        println!("SERIALIZED: {} bytes (expected: {})", serialized.len(), 4 + 10 * 16);

        let deserialized = deserialize_memory_id_list(&serialized);
        println!("AFTER: {} UUIDs", deserialized.len());

        assert_eq!(original, deserialized);
        println!("RESULT: PASS - All UUIDs preserved");
    }

    #[test]
    fn test_key_formats() {
        println!("=== TEST: Key format functions ===");

        let id = Uuid::new_v4();
        println!("UUID: {}", id);

        let fp_key = fingerprint_key(&id);
        assert_eq!(fp_key.len(), 16);
        println!("fingerprint_key: {} bytes", fp_key.len());

        let pv_key = purpose_vector_key(&id);
        assert_eq!(pv_key.len(), 16);
        println!("purpose_vector_key: {} bytes", pv_key.len());

        let e1_key = e1_matryoshka_128_key(&id);
        assert_eq!(e1_key.len(), 16);
        println!("e1_matryoshka_128_key: {} bytes", e1_key.len());

        let term_key = e13_splade_inverted_key(12345);
        assert_eq!(term_key.len(), 2);
        println!("e13_splade_inverted_key(12345): {:?}", term_key);

        println!("RESULT: PASS - All key formats correct");
    }

    // =========================================================================
    // EDGE CASES (3 required with before/after state printing)
    // =========================================================================

    #[test]
    fn edge_case_empty_memory_id_list() {
        println!("=== EDGE CASE 1: Empty memory ID list ===");

        let original: Vec<Uuid> = vec![];
        println!("BEFORE: Empty list, {} UUIDs", original.len());

        let serialized = serialize_memory_id_list(&original);
        println!("SERIALIZED: {} bytes (should be 4 for count only)", serialized.len());
        assert_eq!(serialized.len(), 4);

        let deserialized = deserialize_memory_id_list(&serialized);
        println!("AFTER: {} UUIDs", deserialized.len());

        assert!(deserialized.is_empty());
        println!("RESULT: PASS - Empty list handled correctly");
    }

    #[test]
    fn edge_case_large_memory_id_list() {
        println!("=== EDGE CASE 2: Large memory ID list (1000 entries) ===");

        let original: Vec<Uuid> = (0..1000).map(|_| Uuid::new_v4()).collect();
        println!("BEFORE: {} UUIDs", original.len());
        println!("  First: {}", original[0]);
        println!("  Last: {}", original[999]);

        let serialized = serialize_memory_id_list(&original);
        let expected_size = 4 + 1000 * 16;
        println!("SERIALIZED: {} bytes (expected: {})", serialized.len(), expected_size);
        assert_eq!(serialized.len(), expected_size);

        let deserialized = deserialize_memory_id_list(&serialized);
        println!("AFTER: {} UUIDs", deserialized.len());
        println!("  First: {}", deserialized[0]);
        println!("  Last: {}", deserialized[999]);

        assert_eq!(original, deserialized);
        println!("RESULT: PASS - Large list handled correctly");
    }

    #[test]
    fn edge_case_purpose_vector_extreme_values() {
        println!("=== EDGE CASE 3: Purpose vector with extreme float values ===");

        let original: [f32; 13] = [
            f32::MIN, f32::MAX, 0.0, -0.0,
            f32::EPSILON, -f32::EPSILON,
            1e-38, 1e38, -1e38,
            std::f32::consts::PI, std::f32::consts::E,
            0.123456789, -0.987654321
        ];
        println!("BEFORE: Extreme values including MIN, MAX, EPSILON, PI, E");
        for (i, v) in original.iter().enumerate() {
            println!("  [{}]: {}", i, v);
        }

        let serialized = serialize_purpose_vector(&original);
        println!("SERIALIZED: {} bytes", serialized.len());

        let deserialized = deserialize_purpose_vector(&serialized);
        println!("AFTER: Deserialized values");
        for (i, v) in deserialized.iter().enumerate() {
            println!("  [{}]: {}", i, v);
        }

        for i in 0..13 {
            assert_eq!(original[i].to_bits(), deserialized[i].to_bits(),
                "Bit-exact match failed at index {}", i);
        }
        println!("RESULT: PASS - Extreme values preserved bit-exactly");
    }

    // =========================================================================
    // PANIC TESTS (Verify fail-fast behavior)
    // =========================================================================

    #[test]
    #[should_panic(expected = "DESERIALIZATION ERROR")]
    fn test_panic_on_empty_fingerprint_data() {
        let _ = deserialize_teleological_fingerprint(&[]);
    }

    #[test]
    #[should_panic(expected = "DESERIALIZATION ERROR")]
    fn test_panic_on_wrong_version() {
        let mut data = vec![255u8]; // Wrong version
        data.extend(vec![0u8; 100]); // Garbage data
        let _ = deserialize_teleological_fingerprint(&data);
    }

    #[test]
    #[should_panic(expected = "DESERIALIZATION ERROR")]
    fn test_panic_on_wrong_purpose_vector_size() {
        let _ = deserialize_purpose_vector(&[0u8; 51]); // Should be 52
    }

    #[test]
    #[should_panic(expected = "DESERIALIZATION ERROR")]
    fn test_panic_on_wrong_e1_vector_size() {
        let _ = deserialize_e1_matryoshka_128(&[0u8; 500]); // Should be 512
    }
}
```

### Integration Tests

**File: `crates/context-graph-storage/tests/teleological_integration.rs`**

```rust
//! Integration tests for teleological storage with real RocksDB.

use context_graph_storage::teleological::*;
use context_graph_storage::column_families::cf_names;
use rocksdb::{DB, Cache, Options};
use tempfile::TempDir;
use uuid::Uuid;

#[test]
fn test_rocksdb_open_with_teleological_cfs() {
    println!("=== INTEGRATION: Open RocksDB with 16 column families ===");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let cache = Cache::new_lru_cache(256 * 1024 * 1024);

    // Get base 12 CFs
    let mut descriptors = context_graph_storage::get_column_family_descriptors(&cache);
    println!("BEFORE: {} base column families", descriptors.len());

    // Add 4 teleological CFs
    descriptors.extend(get_teleological_cf_descriptors(&cache));
    println!("AFTER: {} total column families", descriptors.len());

    assert_eq!(descriptors.len(), 16);

    // Open DB
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);

    let db = DB::open_cf_descriptors(&opts, temp_dir.path(), descriptors)
        .expect("Failed to open RocksDB with 16 CFs");

    // Verify all CFs accessible
    for cf_name in cf_names::ALL {
        assert!(db.cf_handle(cf_name).is_some(), "Missing CF: {}", cf_name);
    }
    for cf_name in TELEOLOGICAL_CFS {
        assert!(db.cf_handle(cf_name).is_some(), "Missing CF: {}", cf_name);
    }

    println!("RESULT: PASS - All 16 CFs accessible");
}

#[test]
fn test_rocksdb_store_retrieve_fingerprint() {
    println!("=== INTEGRATION: Store and retrieve TeleologicalFingerprint ===");

    // Setup
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let cache = Cache::new_lru_cache(256 * 1024 * 1024);
    let mut descriptors = context_graph_storage::get_column_family_descriptors(&cache);
    descriptors.extend(get_teleological_cf_descriptors(&cache));

    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);

    let db = DB::open_cf_descriptors(&opts, temp_dir.path(), descriptors)
        .expect("Failed to open RocksDB");

    // Create real fingerprint
    let memory_id = Uuid::new_v4();
    let semantic = context_graph_core::types::fingerprint::SemanticFingerprint::new(memory_id);
    let johari = context_graph_core::types::fingerprint::JohariFingerprint::new(memory_id);
    let original = context_graph_core::types::fingerprint::TeleologicalFingerprint::new(
        memory_id, semantic, johari
    );

    println!("BEFORE: Storing fingerprint {}", memory_id);

    // Store
    let cf = db.cf_handle(CF_FINGERPRINTS).expect("Missing fingerprints CF");
    let key = fingerprint_key(&memory_id);
    let value = serialize_teleological_fingerprint(&original);
    println!("  Serialized size: {} bytes", value.len());

    db.put_cf(&cf, key, &value).expect("Failed to store fingerprint");
    println!("  Stored to RocksDB");

    // Retrieve
    let retrieved_bytes = db.get_cf(&cf, key)
        .expect("Failed to get fingerprint")
        .expect("Fingerprint not found");

    let retrieved = deserialize_teleological_fingerprint(&retrieved_bytes);
    println!("AFTER: Retrieved fingerprint {}", retrieved.memory_id);

    assert_eq!(original.memory_id, retrieved.memory_id);
    println!("RESULT: PASS - Store/retrieve round-trip successful");
}

#[test]
fn test_rocksdb_e13_splade_inverted_index() {
    println!("=== INTEGRATION: E13 SPLADE inverted index operations ===");

    // Setup
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let cache = Cache::new_lru_cache(256 * 1024 * 1024);
    let mut descriptors = context_graph_storage::get_column_family_descriptors(&cache);
    descriptors.extend(get_teleological_cf_descriptors(&cache));

    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);

    let db = DB::open_cf_descriptors(&opts, temp_dir.path(), descriptors)
        .expect("Failed to open RocksDB");

    let cf = db.cf_handle(CF_E13_SPLADE_INVERTED).expect("Missing e13_splade CF");

    // Store term -> memory_ids mapping
    let term_id: u16 = 42;
    let memory_ids: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();

    println!("BEFORE: Storing term {} with {} memory IDs", term_id, memory_ids.len());
    for (i, id) in memory_ids.iter().enumerate() {
        println!("  [{}]: {}", i, id);
    }

    let key = e13_splade_inverted_key(term_id);
    let value = serialize_memory_id_list(&memory_ids);

    db.put_cf(&cf, key, &value).expect("Failed to store inverted index");
    println!("  Stored {} bytes", value.len());

    // Retrieve
    let retrieved_bytes = db.get_cf(&cf, key)
        .expect("Failed to get inverted index")
        .expect("Term not found");

    let retrieved_ids = deserialize_memory_id_list(&retrieved_bytes);
    println!("AFTER: Retrieved {} memory IDs for term {}", retrieved_ids.len(), term_id);
    for (i, id) in retrieved_ids.iter().enumerate() {
        println!("  [{}]: {}", i, id);
    }

    assert_eq!(memory_ids, retrieved_ids);
    println!("RESULT: PASS - Inverted index operations successful");
}
```

---

## Verification Commands

### Execute & Inspect After Each Step

```bash
# Step 1: Module structure
ls -la crates/context-graph-storage/src/teleological/
# EXPECTED OUTPUT: mod.rs, column_families.rs, schema.rs, serialization.rs

# Step 2: Compile check
cargo check -p context-graph-storage 2>&1 | head -30
# EXPECTED OUTPUT: No errors

# Step 3: Run unit tests with output
cargo test -p context-graph-storage teleological -- --nocapture 2>&1
# EXPECTED OUTPUT: All tests pass, state printed for each edge case

# Step 4: Run integration tests with output
cargo test -p context-graph-storage --test teleological_integration -- --nocapture 2>&1
# EXPECTED OUTPUT: All tests pass, RocksDB operations logged

# Step 5: Verify actual database creation
cargo test -p context-graph-storage test_rocksdb_open_with_teleological_cfs -- --nocapture 2>&1
# EXPECTED OUTPUT: "All 16 CFs accessible"
```

### Manual Output Verification

After running tests, manually verify:

1. **Test output shows state changes:**
   ```
   === EDGE CASE 1: Empty memory ID list ===
   BEFORE: Empty list, 0 UUIDs
   SERIALIZED: 4 bytes (should be 4 for count only)
   AFTER: 0 UUIDs
   RESULT: PASS - Empty list handled correctly
   ```

2. **Serialized fingerprint size is ~46KB:**
   ```
   SERIALIZED: 46234 bytes
   ```

3. **All 16 CFs are created:**
   ```
   AFTER: 16 total column families
   RESULT: PASS - All 16 CFs accessible
   ```

---

## Sherlock-Holmes Final Verification

After implementation is complete, spawn sherlock-holmes subagent with this prompt:

```
INVESTIGATE: TASK-F004 Storage Schema Implementation

EVIDENCE REQUIRED:
1. Does `crates/context-graph-storage/src/teleological/` directory exist with 4 files?
2. Does `serialize_teleological_fingerprint()` produce output in range [40000, 55000] bytes?
3. Do all 16 column families open successfully in RocksDB?
4. Do all tests pass without mocks?
5. Do panic tests correctly trigger on invalid input?
6. Are all error messages descriptive with full context?

CRIME SCENE: crates/context-graph-storage/src/teleological/
SUSPECTS: serialization.rs, column_families.rs, schema.rs

VERDICT REQUIRED: INNOCENT (implementation correct) or GUILTY (implementation flawed)
```

---

## Performance Targets

| Operation | Target | Verification |
|-----------|--------|--------------|
| Serialize 46KB fingerprint | <1ms | `cargo bench serialize_teleological` |
| Deserialize 46KB fingerprint | <1ms | `cargo bench deserialize_teleological` |
| RocksDB put (fingerprint) | <5ms | Integration test timing |
| RocksDB get (fingerprint) | <2ms | Integration test timing |
| E13 SPLADE term lookup | <0.5ms | Integration test timing |
| E1 Matryoshka 128D read | <0.5ms | Integration test timing |

---

## Constraints

- **Bincode version**: 1.3 (NOT 2.0) - matches existing Cargo.toml
- **Compression**: LZ4 for fingerprints CF (large values benefit from compression)
- **Block size**: 64KB for fingerprints (holds one 46KB fingerprint per block)
- **Cache**: Shared 256MB LRU cache (per constitution.yaml)
- **Bloom filter**: 10 bits per key on point lookup CFs
- **Total CFs**: 16 (12 existing + 4 new)

---

## Notes

- This task EXTENDS existing storage, does NOT replace it
- Existing 12 column families remain untouched
- New 4 CFs are specifically for TeleologicalFingerprint and 5-stage pipeline
- All serialization panics on error (no silent fallbacks)
- All tests use real struct construction (no mocks)
- State must be printed before/after for edge case tests

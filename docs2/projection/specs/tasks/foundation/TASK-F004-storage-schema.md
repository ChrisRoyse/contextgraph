# Task: TASK-F004 - Implement RocksDB Storage Schema for 46KB Fingerprints

## Metadata
- **ID**: TASK-F004
- **Layer**: Foundation
- **Priority**: P0 (Critical Path)
- **Estimated Effort**: L (Large)
- **Dependencies**: TASK-F001, TASK-F002, TASK-F003
- **Traces To**: TS-201, TS-203, FR-301, FR-304

## Description

Implement the RocksDB column family schema and bincode serialization for storing 50KB TeleologicalFingerprints. This task creates the storage layer that persists the multi-array fingerprints efficiently.

The storage architecture uses 8 column families:
1. **fingerprints** - Primary 50KB fingerprints (optimized for large values, includes E13 SPLADE)
2. **purpose_vectors** - 13D purpose vectors (52 bytes, fast lookups)
3. **johari_index** - Bitmap index by quadrant per embedder (13 embedders)
4. **goal_alignment** - Memory-to-goal alignment cache
5. **purpose_evolution** - Time-series snapshots
6. **metadata** - Node metadata
7. **e13_splade_inverted** - Inverted index for E13 SPLADE sparse vectors
8. **e1_matryoshka_128** - Secondary index for E1 Matryoshka 128D truncated vectors (fast Stage 2)

## Acceptance Criteria

- [ ] RocksDB column family definitions for all 8 families
- [ ] Column family options optimized for value sizes (50KB vs 52 bytes)
- [ ] Key format functions for each family
- [ ] Bincode serialization for SemanticFingerprint (13 embedders including E13 SPLADE)
- [ ] Bincode deserialization with version checking
- [ ] FingerprintHeader for variable-length fields (E6, E12, E13)
- [ ] Compression configuration (LZ4)
- [ ] E13 SPLADE inverted index storage (term_id -> memory_ids)
- [ ] E1 Matryoshka 128D truncated vector storage for fast Stage 2 filtering
- [ ] stage_scores column in fingerprints (5 x f32 = 20 bytes)
- [ ] Unit tests for serialization round-trip
- [ ] Integration tests with actual RocksDB instance

## Implementation Steps

1. Create `crates/context-graph-storage/src/teleological/mod.rs`:
   - Define module structure
2. Create `crates/context-graph-storage/src/teleological/schema.rs`:
   - Define column family name constants
   - Implement `TeleologicalSchema` with CF options
   - Implement key format functions
3. Create `crates/context-graph-storage/src/teleological/serialization.rs`:
   - Define `SERIALIZATION_VERSION = 1`
   - Implement `FingerprintHeader` struct
   - Implement `serialize_semantic_fingerprint()`
   - Implement `deserialize_semantic_fingerprint()`
   - Implement `serialize_teleological_fingerprint()`
   - Implement `deserialize_teleological_fingerprint()`
4. Update `crates/context-graph-storage/src/lib.rs` to export teleological module
5. Add bincode dependency to Cargo.toml if not present

## Files Affected

### Files to Create
- `crates/context-graph-storage/src/teleological/mod.rs` - Module definition
- `crates/context-graph-storage/src/teleological/schema.rs` - RocksDB schema
- `crates/context-graph-storage/src/teleological/serialization.rs` - Bincode serialization

### Files to Modify
- `crates/context-graph-storage/src/lib.rs` - Export teleological module
- `crates/context-graph-storage/Cargo.toml` - Add bincode dependency

### Existing Files to Reference
- `crates/context-graph-storage/src/column_families.rs` - Existing CF patterns
- `crates/context-graph-storage/src/serialization.rs` - Existing serialization patterns

## Code Signature (Definition of Done)

```rust
// schema.rs
pub const CF_FINGERPRINTS: &str = "fingerprints";
pub const CF_PURPOSE_VECTORS: &str = "purpose_vectors";
pub const CF_JOHARI_INDEX: &str = "johari_index";
pub const CF_GOAL_ALIGNMENT: &str = "goal_alignment";
pub const CF_EVOLUTION: &str = "purpose_evolution";
pub const CF_METADATA: &str = "metadata";
pub const CF_E13_SPLADE_INVERTED: &str = "e13_splade_inverted";
pub const CF_E1_MATRYOSHKA_128: &str = "e1_matryoshka_128";

pub struct TeleologicalSchema;

impl TeleologicalSchema {
    /// Options optimized for 50KB values (includes E13 SPLADE)
    pub fn fingerprint_cf_options() -> Options;

    /// Options optimized for 52-byte purpose vectors (13D x 4 bytes)
    pub fn purpose_vector_cf_options() -> Options;

    /// Options optimized for E13 SPLADE inverted index
    pub fn e13_splade_inverted_cf_options() -> Options;

    /// Options optimized for E1 Matryoshka 128D vectors (512 bytes)
    pub fn e1_matryoshka_128_cf_options() -> Options;

    /// Open DB with all 8 column families
    pub fn open(path: impl AsRef<Path>) -> Result<DB, rocksdb::Error>;
}

/// Key: UUID as 16 bytes
pub fn fingerprint_key(id: &Uuid) -> [u8; 16];

/// Key: UUID as 16 bytes
pub fn purpose_vector_key(id: &Uuid) -> [u8; 16];

/// Key: (quadrant_u8, embedder_u8, memory_id_bytes)
pub fn johari_index_key(quadrant: u8, embedder: u8, memory_id: &Uuid) -> Vec<u8>;

/// Key: (memory_id_bytes, goal_id_bytes)
pub fn goal_alignment_key(memory_id: &Uuid, goal_id: &Uuid) -> Vec<u8>;

/// Key: (memory_id_bytes, timestamp_i64_be)
pub fn evolution_key(memory_id: &Uuid, timestamp_nanos: i64) -> Vec<u8>;

/// Key: (term_id_u16) -> Value: (memory_id_list)
/// For E13 SPLADE inverted index
pub fn e13_splade_inverted_key(term_id: u16) -> [u8; 2];

/// Key: UUID as 16 bytes -> Value: 128D f32 vector (512 bytes)
/// For E1 Matryoshka 128D secondary index
pub fn e1_matryoshka_128_key(id: &Uuid) -> [u8; 16];

// serialization.rs
pub const SERIALIZATION_VERSION: u8 = 2;  // Bumped for E13 + stage_scores

#[derive(Debug, Clone, Encode, Decode)]
pub struct FingerprintHeader {
    pub version: u8,
    pub total_size: u32,
    pub e12_token_count: u16,
    pub e6_active_count: u16,
    pub e13_active_count: u16,  // NEW: E13 SPLADE non-zero count
    pub stage_scores_present: bool,  // NEW: indicates stage_scores field present
}

pub fn serialize_semantic_fingerprint(fp: &SemanticFingerprint) -> Vec<u8>;
pub fn deserialize_semantic_fingerprint(data: &[u8]) -> Result<SemanticFingerprint, &'static str>;

pub fn serialize_teleological_fingerprint(fp: &TeleologicalFingerprint) -> Vec<u8>;
pub fn deserialize_teleological_fingerprint(data: &[u8]) -> Result<TeleologicalFingerprint, &'static str>;
```

## Testing Requirements

### Unit Tests
- `test_fingerprint_key_format` - 16-byte UUID key
- `test_johari_index_key_format` - Correct composite key (13 embedders)
- `test_goal_alignment_key_format` - 32-byte composite key
- `test_evolution_key_format` - Sortable timestamp key
- `test_e13_splade_inverted_key_format` - 2-byte term ID key
- `test_e1_matryoshka_128_key_format` - 16-byte UUID key
- `test_serialize_semantic_roundtrip` - Encode then decode matches (13 embedders)
- `test_serialize_teleological_roundtrip` - Full fingerprint round-trip with stage_scores
- `test_serialize_sparse_vector` - E6 variable length handling
- `test_serialize_e13_splade` - E13 SPLADE sparse vector handling
- `test_serialize_token_level` - E12 variable length handling
- `test_serialize_stage_scores` - 5-element stage scores array
- `test_version_check` - Rejects wrong version (v2 required)

### Integration Tests
- `test_rocksdb_open_all_cfs` - Opens DB with 8 column families
- `test_rocksdb_store_retrieve_fingerprint` - Full write/read cycle
- `test_rocksdb_cf_options_large_values` - Correct settings for 50KB
- `test_rocksdb_e13_splade_inverted_index` - Term lookup retrieves correct memory IDs
- `test_rocksdb_e1_matryoshka_128_index` - Fast 128D vector retrieval

## Verification

```bash
# Compile check
cargo check -p context-graph-storage

# Run unit tests
cargo test -p context-graph-storage teleological

# Run integration tests
cargo test -p context-graph-storage --test rocksdb_integration
```

## Constraints

- Bincode 2.0 (rc.3) for efficient binary serialization
- LZ4 compression for space efficiency
- 64KB block size for 50KB values
- 256MB SST files for fingerprints CF
- 64MB SST files for purpose vectors CF
- 32MB SST files for E13 SPLADE inverted index CF
- 128MB SST files for E1 Matryoshka 128D index CF
- Big-endian timestamps for sorted range scans
- Bloom filter on purpose vectors for fast lookups
- Bloom filter on E13 SPLADE inverted index for fast term lookup

## Performance Targets

| Operation | Target |
|-----------|--------|
| Serialization (50KB) | <1ms |
| Deserialization (50KB) | <1ms |
| RocksDB write | <5ms |
| RocksDB read | <2ms |
| E13 SPLADE inverted lookup | <0.5ms |
| E1 Matryoshka 128D read | <0.5ms |

## Notes

This task creates the storage foundation. The actual store implementation that uses these schemas comes in Logic Layer tasks.

The 50KB storage per node is ~8x larger than legacy 6KB fused vectors, but preserves 100% information vs 33% with fusion.

**5-Stage Pipeline Storage Support**:
- E13 SPLADE inverted index enables fast Stage 1 (Recall) term lookup
- E1 Matryoshka 128D index enables fast Stage 2 (Semantic) filtering
- stage_scores field tracks per-stage performance for pipeline optimization

**New Column Families for 5-Stage Pipeline**:
| CF Name | Key | Value | Purpose |
|---------|-----|-------|---------|
| e13_splade_inverted | term_id (u16) | memory_id list | Stage 1 term lookup |
| e1_matryoshka_128 | memory_id (UUID) | 128D vector (512 bytes) | Stage 2 fast filtering |

Reference implementation in TECH-SPEC-001 Sections 2.1 and 2.3 (TS-201, TS-203).

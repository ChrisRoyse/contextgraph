# TASK-SESSION-04: Add CF_SESSION_IDENTITY Column Family

```xml
<task_spec id="TASK-SESSION-04" version="1.0">
<metadata>
  <title>Add CF_SESSION_IDENTITY Column Family</title>
  <status>pending</status>
  <layer>foundation</layer>
  <sequence>4</sequence>
  <implements>
    <requirement_ref>REQ-SESSION-04</requirement_ref>
  </implements>
  <depends_on><!-- None --></depends_on>
  <estimated_hours>1.5</estimated_hours>
</metadata>
```

## Objective

Create the CF_SESSION_IDENTITY column family in RocksDB with optimized configuration for session identity storage.

## Context

Session identity snapshots need a dedicated column family in RocksDB for:
- Isolation from other data
- Optimized bloom filter for fast lookups
- LZ4 compression for smaller storage footprint
- Temporal index for recovery from corruption

## Key Scheme

| Key Pattern | Value | Description |
|-------------|-------|-------------|
| `s:{session_id}` | SessionIdentitySnapshot (bincode) | Primary session data |
| `latest` | session_id string | Pointer to most recent session |
| `t:{timestamp_ms}` | session_id string | Temporal index (big-endian) |

## Implementation Steps

1. Add SESSION_IDENTITY constant to cf_names module
2. Update ALL constant array to include new column family (13 total)
3. Create session_identity_options() function with bloom filter and LZ4 compression
4. Update column family creation in RocksDbMemex initialization
5. Document key scheme in comments
6. Add key helper functions

## Input Context Files

```xml
<input_context_files>
  <file purpose="column_family_pattern">crates/context-graph-storage/src/column_families.rs</file>
  <file purpose="rocksdb_init">crates/context-graph-storage/src/rocksdb_backend.rs</file>
</input_context_files>
```

## Files to Create

None.

## Files to Modify

| File | Change |
|------|--------|
| `crates/context-graph-storage/src/column_families.rs` | Add SESSION_IDENTITY constant, options, and key helpers |
| `crates/context-graph-storage/src/rocksdb_backend.rs` | Initialize CF on open |

## Rust Signatures

```rust
// crates/context-graph-storage/src/column_families.rs

pub mod cf_names {
    // ... existing ...
    pub const SESSION_IDENTITY: &str = "session_identity";

    pub const ALL: &[&str] = &[
        // ... existing 12 ...
        SESSION_IDENTITY, // 13th
    ];
}

/// Options for session_identity column family.
/// Optimized for small, frequently-accessed snapshots.
pub fn session_identity_options(cache: &Cache) -> Options;

// Key helpers
#[inline]
pub fn session_key(session_id: &str) -> Vec<u8>;

#[inline]
pub fn temporal_key(timestamp_ms: i64) -> Vec<u8>;

pub const LATEST_KEY: &[u8] = b"latest";
```

## Definition of Done

### Acceptance Criteria

- [ ] SESSION_IDENTITY constant equals "session_identity"
- [ ] ALL array has 13 elements
- [ ] session_identity_options configures bloom filter (10 bits)
- [ ] session_identity_options configures LZ4 compression
- [ ] RocksDB opens successfully with new column family
- [ ] Key scheme documented: "s:{session_id}", "latest", "t:{timestamp_ms}"
- [ ] session_key() returns `s:{session_id}` as bytes
- [ ] temporal_key() returns `t:{big_endian_timestamp}` as bytes
- [ ] All existing tests pass (no regression)

### Constraints

- Bloom filter: 10 bits per key
- Compression: LZ4
- Must be backward compatible with existing column families
- temporal_key uses big-endian for lexicographic ordering

### Verification Commands

```bash
cargo build -p context-graph-storage
cargo test -p context-graph-storage
```

## Test Cases

### TC-SESSION-CF-01: Column Family Count
```rust
#[test]
fn test_column_family_count() {
    assert_eq!(cf_names::ALL.len(), 13);
    assert!(cf_names::ALL.contains(&cf_names::SESSION_IDENTITY));
}
```

### TC-SESSION-CF-02: Key Helpers
```rust
#[test]
fn test_session_key_format() {
    let key = session_key("abc-123");
    assert_eq!(&key, b"s:abc-123");
}

#[test]
fn test_temporal_key_ordering() {
    let t1 = temporal_key(1000);
    let t2 = temporal_key(2000);
    assert!(t1 < t2); // Lexicographic ordering
}
```

## Exit Conditions

- **Success**: RocksDB opens with 13 column families, all existing tests pass
- **Failure**: Column family creation fails, existing functionality broken - error out with detailed logging

## Next Task

After completion, proceed to **005-TASK-SESSION-05** (save_snapshot/load_snapshot Methods).

```xml
</task_spec>
```

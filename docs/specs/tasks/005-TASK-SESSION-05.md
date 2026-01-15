# TASK-SESSION-05: Create save_snapshot/load_snapshot Methods

```xml
<task_spec id="TASK-SESSION-05" version="1.0">
<metadata>
  <title>Create save_snapshot/load_snapshot Storage Methods</title>
  <status>pending</status>
  <layer>foundation</layer>
  <sequence>5</sequence>
  <implements>
    <requirement_ref>REQ-SESSION-05</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-SESSION-01</task_ref>
    <task_ref>TASK-SESSION-04</task_ref>
  </depends_on>
  <estimated_hours>2.0</estimated_hours>
</metadata>
```

## Objective

Implement storage methods for persisting and retrieving SessionIdentitySnapshot from CF_SESSION_IDENTITY with temporal index recovery.

## Context

These methods provide the persistence layer for session identity. They handle:
- Primary key storage (`s:{session_id}`)
- Latest session pointer (`latest`)
- Temporal index for corruption recovery (`t:{timestamp_ms}`)
- Graceful handling of missing data (fresh install)

## Implementation Steps

1. Create `session_identity.rs` in storage crate
2. Implement save_snapshot() writing to primary key, latest, and temporal index
3. Implement load_snapshot() with optional session_id parameter
4. Implement load_snapshot_by_id() for specific session lookup
5. Implement load_latest() with temporal recovery fallback
6. Implement recover_from_temporal_index() for corruption recovery
7. Add integration tests

## Input Context Files

```xml
<input_context_files>
  <file purpose="snapshot_type">crates/context-graph-core/src/gwt/session_identity/types.rs</file>
  <file purpose="column_family">crates/context-graph-storage/src/column_families.rs</file>
  <file purpose="rocksdb_impl">crates/context-graph-storage/src/rocksdb_backend.rs</file>
  <file purpose="error_types">crates/context-graph-storage/src/error.rs</file>
</input_context_files>
```

## Files to Create

| File | Description |
|------|-------------|
| `crates/context-graph-storage/src/session_identity.rs` | Storage methods |

## Files to Modify

| File | Change |
|------|--------|
| `crates/context-graph-storage/src/lib.rs` | Export session_identity module |

## Rust Signatures

```rust
// crates/context-graph-storage/src/session_identity.rs

impl RocksDbMemex {
    /// Save snapshot to CF_SESSION_IDENTITY.
    /// Writes to: s:{session_id}, latest, t:{timestamp_ms}
    pub fn save_snapshot(&self, snapshot: &SessionIdentitySnapshot) -> StorageResult<()>;

    /// Load snapshot by session_id, or latest if None.
    pub fn load_snapshot(&self, session_id: Option<&str>) -> StorageResult<SessionIdentitySnapshot>;

    /// Load specific session by ID.
    fn load_snapshot_by_id(&self, session_id: &str) -> StorageResult<SessionIdentitySnapshot>;

    /// Load latest session, returning None for fresh install.
    pub fn load_latest(&self) -> StorageResult<Option<SessionIdentitySnapshot>>;

    /// Recover from temporal index if "latest" key is corrupted.
    fn recover_from_temporal_index(&self) -> StorageResult<Option<SessionIdentitySnapshot>>;
}
```

## Definition of Done

### Acceptance Criteria

- [ ] save_snapshot writes to "s:{session_id}" key
- [ ] save_snapshot updates "latest" key
- [ ] save_snapshot writes temporal index "t:{timestamp_ms}"
- [ ] load_snapshot(None) returns latest snapshot
- [ ] load_snapshot(Some(id)) returns specific snapshot
- [ ] load_latest returns None for fresh install
- [ ] Temporal recovery finds most recent snapshot if "latest" corrupted
- [ ] Test case TC-SESSION-05 passes (save/load round-trip)
- [ ] Test case TC-SESSION-06 passes (temporal ordering)

### Constraints

- Use bincode for serialization
- All writes in single WriteBatch for atomicity
- Temporal key uses big-endian timestamp for proper ordering
- Recovery iterates temporal index in reverse order

### Verification Commands

```bash
cargo build -p context-graph-storage
cargo test -p context-graph-storage session_identity
```

## Test Cases

### TC-SESSION-05: Save/Load Round-Trip
```rust
#[test]
fn test_save_load_roundtrip() {
    let storage = test_storage();
    let snapshot = SessionIdentitySnapshot::default();

    storage.save_snapshot(&snapshot).unwrap();
    let loaded = storage.load_snapshot(None).unwrap();

    assert_eq!(snapshot.session_id, loaded.session_id);
}
```

### TC-SESSION-06: Temporal Ordering
```rust
#[test]
fn test_temporal_ordering() {
    let storage = test_storage();

    // Save three snapshots with different timestamps
    let mut s1 = SessionIdentitySnapshot::new("s1");
    s1.timestamp_ms = 1000;
    let mut s2 = SessionIdentitySnapshot::new("s2");
    s2.timestamp_ms = 2000;
    let mut s3 = SessionIdentitySnapshot::new("s3");
    s3.timestamp_ms = 3000;

    storage.save_snapshot(&s1).unwrap();
    storage.save_snapshot(&s2).unwrap();
    storage.save_snapshot(&s3).unwrap();

    // Latest should be s3
    let latest = storage.load_latest().unwrap().unwrap();
    assert_eq!(latest.session_id, "s3");
}
```

### TC-SESSION-07: Fresh Install Returns None
```rust
#[test]
fn test_fresh_install() {
    let storage = test_storage();
    let result = storage.load_latest().unwrap();
    assert!(result.is_none());
}
```

## Exit Conditions

- **Success**: All storage operations work correctly with corruption recovery
- **Failure**: Storage corruption, lost data - error out with detailed logging

## Next Task

After completion, proceed to **006-TASK-SESSION-06** (SessionIdentityManager).

```xml
</task_spec>
```

---
id: "M04-T08a"
title: "Implement Error Conversions (From Traits)"
description: |
  TASK STATUS: NOT STARTED - Only std::io::Error conversion exists.
  REMAINING: Add From<rocksdb::Error>, From<serde_json::Error>, From<bincode::Error>.
  These enable the ? operator in functions returning Result<T, GraphError>.
layer: "foundation"
status: "pending"
priority: "high"
estimated_hours: 1
sequence: 12
depends_on:
  - "M04-T08"
spec_refs:
  - "TECH-GRAPH-004 Section 9"
files_to_create: []
files_to_modify:
  - path: "crates/context-graph-graph/src/error.rs"
    description: "Add From trait implementations for external error types"
test_file: "crates/context-graph-graph/src/error.rs (inline #[cfg(test)])"
---

## CRITICAL: Current Codebase State (2026-01-03)

### What Already Exists

**File**: `crates/context-graph-graph/src/error.rs`

**Existing conversions:**
- `#[from] std::io::Error` - Already implemented via thiserror derive attribute on `Io` variant

**GraphResult<T> type alias:** Already defined as `Result<T, GraphError>`

**Test coverage:** 11 tests exist, including `test_io_error_conversion`

### Dependencies Already in Cargo.toml
```toml
rocksdb = "0.22"
serde_json = "1.0"
bincode = "1.3"
```

### What Is MISSING (Must Implement)

1. **From<rocksdb::Error>** - Convert RocksDB errors to GraphError::Storage
2. **From<serde_json::Error>** - Convert JSON errors to GraphError::Serialization
3. **From<bincode::Error>** - Convert bincode errors to GraphError::Deserialization

### Current Error Variants for Conversion Targets
- `GraphError::Storage(String)` - line 67 in error.rs
- `GraphError::Serialization(String)` - line 149
- `GraphError::Deserialization(String)` - line 153

## Scope

### In Scope
- Implement From<rocksdb::Error> for GraphError
- Implement From<serde_json::Error> for GraphError
- Implement From<bincode::Error> for GraphError
- Ensure ? operator works with these error types
- Add comprehensive tests verifying conversions

### Out of Scope
- GraphError variant definitions (see M04-T08)
- Custom error types from other crates

## Definition of Done

### Required Code Changes

**Add these impl blocks AFTER the GraphError enum definition, BEFORE #[cfg(test)]:**

```rust
// ========== Error Conversions ==========
// Enable ? operator for external error types

impl From<rocksdb::Error> for GraphError {
    fn from(err: rocksdb::Error) -> Self {
        GraphError::Storage(err.to_string())
    }
}

impl From<serde_json::Error> for GraphError {
    fn from(err: serde_json::Error) -> Self {
        // serde_json errors include line/column info in to_string()
        GraphError::Serialization(err.to_string())
    }
}

impl From<bincode::Error> for GraphError {
    fn from(err: bincode::Error) -> Self {
        // Box<bincode::ErrorKind> - deref for message
        GraphError::Deserialization(err.to_string())
    }
}
```

**Add these tests inside the #[cfg(test)] mod tests block:**

```rust
#[test]
fn test_rocksdb_error_conversion() {
    // Create a RocksDB error by attempting invalid operation
    // Note: rocksdb::Error doesn't have public constructors,
    // so we test via a real scenario
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("nonexistent_cf_test");

    // Open DB, then try to get a non-existent column family
    let db = rocksdb::DB::open_default(&path).unwrap();

    // Try to create error via invalid cf operation
    // We'll test the conversion compiles and works via type checking
    fn takes_graph_result<T>(_: crate::error::GraphResult<T>) {}

    // This function would use ? operator with rocksdb
    fn rocksdb_fn() -> crate::error::GraphResult<()> {
        let temp_dir = tempfile::tempdir()?;  // io::Error -> GraphError::Io
        let path = temp_dir.path().join("test.db");
        let _db = rocksdb::DB::open_default(&path)?;  // rocksdb::Error -> GraphError
        Ok(())
    }

    // If this compiles, the From impl works
    let _ = rocksdb_fn();
}

#[test]
fn test_serde_json_error_conversion() {
    // Create invalid JSON to trigger parse error
    let invalid_json = "{ invalid json }";
    let result: Result<serde_json::Value, serde_json::Error> =
        serde_json::from_str(invalid_json);

    let json_err = result.unwrap_err();
    let graph_err: GraphError = json_err.into();

    // Verify it converted to Serialization variant
    match &graph_err {
        GraphError::Serialization(msg) => {
            assert!(msg.contains("expected") || msg.contains("invalid") || msg.len() > 0);
            println!("JSON error converted: {}", msg);
        }
        _ => panic!("Expected GraphError::Serialization, got {:?}", graph_err),
    }
}

#[test]
fn test_bincode_error_conversion() {
    // Create invalid bincode data to trigger deserialize error
    let invalid_data: &[u8] = &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
    let result: Result<String, bincode::Error> =
        bincode::deserialize(invalid_data);

    let bincode_err = result.unwrap_err();
    let graph_err: GraphError = bincode_err.into();

    // Verify it converted to Deserialization variant
    match &graph_err {
        GraphError::Deserialization(msg) => {
            assert!(!msg.is_empty());
            println!("Bincode error converted: {}", msg);
        }
        _ => panic!("Expected GraphError::Deserialization, got {:?}", graph_err),
    }
}

#[test]
fn test_question_mark_operator_with_conversions() {
    // Verify ? operator works in function returning GraphResult

    fn json_parse_fn(json: &str) -> crate::error::GraphResult<serde_json::Value> {
        let value = serde_json::from_str(json)?;  // ? converts serde_json::Error
        Ok(value)
    }

    fn bincode_fn(data: &[u8]) -> crate::error::GraphResult<String> {
        let value = bincode::deserialize(data)?;  // ? converts bincode::Error
        Ok(value)
    }

    // Valid JSON should succeed
    let valid = json_parse_fn(r#"{"key": "value"}"#);
    assert!(valid.is_ok());

    // Invalid JSON should fail with Serialization error
    let invalid = json_parse_fn("not json");
    assert!(matches!(invalid, Err(GraphError::Serialization(_))));

    // Invalid bincode should fail with Deserialization error
    let invalid_bin = bincode_fn(&[0xFF, 0xFF]);
    assert!(matches!(invalid_bin, Err(GraphError::Deserialization(_))));
}
```

### Constraints
- NO BACKWARDS COMPATIBILITY HACKS - if something breaks, it fails fast
- Conversions MUST preserve error information (use to_string())
- MUST NOT panic during conversion
- MUST work with ? operator
- NO mock data in tests - use REAL error scenarios

### Acceptance Criteria
- [ ] From<rocksdb::Error> implemented
- [ ] From<serde_json::Error> implemented
- [ ] From<bincode::Error> implemented
- [ ] ? operator works with all three error types
- [ ] `cargo build -p context-graph-graph` succeeds
- [ ] `cargo test -p context-graph-graph error` - all tests pass (13+ tests)
- [ ] `cargo clippy -p context-graph-graph -- -D warnings` - no warnings

## Verification

### Test Commands (Execute These Exactly)
```bash
# Step 1: Build the crate
cargo build -p context-graph-graph

# Step 2: Run ALL error tests
cargo test -p context-graph-graph error -- --nocapture

# Step 3: Run clippy
cargo clippy -p context-graph-graph -- -D warnings

# Step 4: Count tests to verify new ones added
cargo test -p context-graph-graph error 2>&1 | grep "test result"
# Expected: 15+ passed (11 existing + 4 new)
```

### Manual Verification Checklist
- [ ] RocksDB errors convert to GraphError::Storage
- [ ] serde_json errors convert to GraphError::Serialization
- [ ] bincode errors convert to GraphError::Deserialization
- [ ] Error messages contain original error info
- [ ] All 4 new tests pass
- [ ] All 11 existing tests still pass

## Full State Verification (MANDATORY)

### Source of Truth
The final result is stored in:
- **File**: `crates/context-graph-graph/src/error.rs`

### Execute & Inspect Protocol
After making changes:

1. **Build verification:**
   ```bash
   cargo build -p context-graph-graph
   ```

2. **Test execution:**
   ```bash
   cargo test -p context-graph-graph error -- --nocapture 2>&1 | tee /tmp/error_tests.log
   ```

3. **Inspect test output:**
   ```bash
   cat /tmp/error_tests.log | grep -E "(test |passed|failed)"
   ```

4. **Verify From impls exist:**
   ```bash
   grep -n "impl From<rocksdb::Error>" crates/context-graph-graph/src/error.rs
   grep -n "impl From<serde_json::Error>" crates/context-graph-graph/src/error.rs
   grep -n "impl From<bincode::Error>" crates/context-graph-graph/src/error.rs
   ```

5. **Verify ? operator usage compiles (implicit in test):**
   If tests compile and pass, ? operator works.

### Boundary & Edge Case Audit

**Edge Case 1: RocksDB error with long message**
```rust
// Test in real scenario - DB open failure
fn test_long_rocksdb_error() -> GraphResult<()> {
    let path = "/nonexistent/very/long/path/that/does/not/exist/database.db";
    println!("BEFORE: attempting to open DB at {}", path);
    let result = rocksdb::DB::open_default(path);
    match result {
        Ok(_) => panic!("Should have failed"),
        Err(e) => {
            println!("AFTER: RocksDB error = {}", e);
            let graph_err: GraphError = e.into();
            println!("AFTER: GraphError = {}", graph_err);
            // Verify message preserved
            assert!(graph_err.to_string().len() > 10);
        }
    }
    Ok(())
}
```

**Edge Case 2: serde_json error with position info**
```rust
fn test_json_error_position() {
    let json = r#"{
        "key": "value",
        broken
    }"#;
    println!("BEFORE: parsing multi-line invalid JSON");
    let result: Result<serde_json::Value, _> = serde_json::from_str(json);
    let err = result.unwrap_err();
    println!("AFTER: serde_json error = {}", err);
    let graph_err: GraphError = err.into();
    println!("AFTER: GraphError = {}", graph_err);
    // serde_json includes line/column in error
    let msg = graph_err.to_string();
    assert!(msg.contains("line") || msg.contains("column") || msg.contains("at"));
}
```

**Edge Case 3: bincode error with type info**
```rust
fn test_bincode_type_error() {
    #[derive(serde::Deserialize)]
    struct ExpectedType {
        field: u64,
    }

    let data = bincode::serialize(&"string data").unwrap();
    println!("BEFORE: deserializing string as struct");
    let result: Result<ExpectedType, _> = bincode::deserialize(&data);
    let err = result.unwrap_err();
    println!("AFTER: bincode error = {}", err);
    let graph_err: GraphError = err.into();
    println!("AFTER: GraphError = {}", graph_err);
    assert!(matches!(graph_err, GraphError::Deserialization(_)));
}
```

### Evidence of Success
After completion, provide:
1. Full output of `cargo test -p context-graph-graph error`
2. Grep output showing all 3 From impls exist
3. Test count showing 15+ tests pass
4. Confirmation that clippy produces no warnings

## Implementation Approach

1. Open `crates/context-graph-graph/src/error.rs`
2. Locate line 160 (after `Io(#[from] std::io::Error)` closing brace)
3. Add the 3 From impl blocks before `#[cfg(test)]`
4. Add the 4 new tests inside `mod tests`
5. Add `use super::*;` if not present in tests
6. Run verification commands
7. Provide evidence of success

## Files Reference

```
crates/context-graph-graph/
├── Cargo.toml              # Already has rocksdb, serde_json, bincode
└── src/
    └── error.rs            # Add From impls and tests
```

## Example Usage After Implementation

```rust
use context_graph_graph::error::{GraphError, GraphResult};

// All these functions can now use ? operator seamlessly:

fn load_config(path: &str) -> GraphResult<serde_json::Value> {
    let content = std::fs::read_to_string(path)?;  // io::Error -> GraphError::Io
    let config = serde_json::from_str(&content)?;  // serde_json::Error -> GraphError::Serialization
    Ok(config)
}

fn read_from_db(db: &rocksdb::DB, key: &[u8]) -> GraphResult<Vec<u8>> {
    let value = db.get(key)?  // rocksdb::Error -> GraphError::Storage
        .ok_or_else(|| GraphError::NodeNotFound(format!("key {:?}", key)))?;
    Ok(value)
}

fn deserialize_edge(data: &[u8]) -> GraphResult<GraphEdge> {
    let edge = bincode::deserialize(data)?;  // bincode::Error -> GraphError::Deserialization
    Ok(edge)
}
```

## SHERLOCK-HOLMES VERIFICATION REQUIRED

After completing implementation, spawn `sherlock-holmes` agent to:
1. Verify all 3 From impls compile
2. Verify all tests pass (15+ total)
3. Test ? operator in real function context
4. Verify error messages preserve source error info
5. Check for any clippy warnings
6. Validate no regressions in existing functionality

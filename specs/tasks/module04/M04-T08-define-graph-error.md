---
id: "M04-T08"
title: "Define GraphError Enum"
description: |
  TASK STATUS: PARTIALLY COMPLETE - GraphError enum exists with 26 variants.
  REMAINING: Add missing variants (StorageOpen), verify Send+Sync bounds, add static_assertions.
layer: "foundation"
status: "in_progress"
priority: "high"
estimated_hours: 0.5
sequence: 11
depends_on:
  - "M04-T00"
spec_refs:
  - "TECH-GRAPH-004 Section 9"
files_to_create: []
files_to_modify:
  - path: "crates/context-graph-graph/src/error.rs"
    description: "Add missing variants and compile-time assertions"
test_file: "crates/context-graph-graph/src/error.rs (inline #[cfg(test)])"
---

## CRITICAL: Current Codebase State (2026-01-03)

### What Already Exists

**File**: `crates/context-graph-graph/src/error.rs` (260 lines)

**GraphError enum already has 26 variants:**
1. `FaissIndexCreation(String)`
2. `FaissTrainingFailed(String)`
3. `FaissSearchFailed(String)`
4. `FaissAddFailed(String)`
5. `IndexNotTrained`
6. `InsufficientTrainingData { required: usize, provided: usize }`
7. `GpuResourceAllocation(String)`
8. `GpuTransferFailed(String)`
9. `GpuDeviceUnavailable(String)`
10. `Storage(String)`
11. `ColumnFamilyNotFound(String)`
12. `CorruptedData { location: String, details: String }`
13. `MigrationFailed(String)`
14. `InvalidConfig(String)`
15. `DimensionMismatch { expected: usize, actual: usize }`
16. `NodeNotFound(String)`
17. `EdgeNotFound(String, String)` - tuple variant
18. `DuplicateNode(String)`
19. `InvalidHyperbolicPoint { norm: f32 }`
20. `InvalidCurvature(f32)`
21. `MobiusOperationFailed(String)`
22. `InvalidAperture(f32)`
23. `ZeroConeAxis`
24. `PathNotFound(String, String)`
25. `DepthLimitExceeded(usize)`
26. `CycleDetected(String)`
27. `VectorIdMismatch(String)`
28. `InvalidNtWeights { field: String, value: f32 }`
29. `Serialization(String)`
30. `Deserialization(String)`
31. `Io(#[from] std::io::Error)`

**GraphResult<T> type alias exists.**

**11 unit tests pass.**

### What Is MISSING (Must Implement)

1. **StorageOpen variant** - Missing structured variant for path+cause:
   ```rust
   #[error("Failed to open storage at {path}: {cause}")]
   StorageOpen { path: String, cause: String },
   ```

2. **static_assertions for Send + Sync** - Must add compile-time check:
   ```rust
   // At end of file, outside impl blocks
   static_assertions::assert_impl_all!(GraphError: Send, Sync, std::error::Error);
   ```

3. **static_assertions dependency** - Add to Cargo.toml if not present

## Scope

### In Scope
- Add `StorageOpen { path, cause }` variant
- Add `static_assertions::assert_impl_all!` compile-time check
- Verify all 31+ variants compile and test correctly

### Out of Scope
- From trait implementations (see M04-T08a)
- Error recovery logic

## Definition of Done

### Required Code Changes

**1. Add to Cargo.toml (if missing):**
```toml
static_assertions = "1.1"
```

**2. Add to error.rs imports:**
```rust
// At top of file, after existing imports
```

**3. Add StorageOpen variant after line 67 (before Storage variant):**
```rust
    /// Failed to open storage at specific path.
    #[error("Failed to open storage at {path}: {cause}")]
    StorageOpen { path: String, cause: String },
```

**4. Add static assertion at end of file (before #[cfg(test)]):**
```rust
// Compile-time verification that GraphError is thread-safe
static_assertions::assert_impl_all!(GraphError: Send, Sync, std::error::Error);
```

**5. Add test for new variant:**
```rust
#[test]
fn test_error_display_storage_open() {
    let err = GraphError::StorageOpen {
        path: "/data/graph.db".to_string(),
        cause: "permission denied".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("/data/graph.db"));
    assert!(msg.contains("permission denied"));
}

#[test]
fn test_graph_error_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<GraphError>();
}
```

### Constraints
- NO BACKWARDS COMPATIBILITY HACKS - if something breaks, it fails
- All variants must have descriptive #[error()] messages
- GraphError must be Send + Sync (enforced by static_assertions)
- Tests must use REAL error types, not mocks

### Acceptance Criteria
- [ ] StorageOpen variant added with path and cause fields
- [ ] static_assertions crate added to dependencies
- [ ] assert_impl_all! macro verifies Send + Sync + Error
- [ ] All 32+ variants compile
- [ ] `cargo build -p context-graph-graph` succeeds
- [ ] `cargo test -p context-graph-graph error` - all tests pass
- [ ] `cargo clippy -p context-graph-graph -- -D warnings` - no warnings

## Verification

### Test Commands (Execute These Exactly)
```bash
# Step 1: Build the crate
cargo build -p context-graph-graph

# Step 2: Run error-specific tests
cargo test -p context-graph-graph error -- --nocapture

# Step 3: Run clippy
cargo clippy -p context-graph-graph -- -D warnings

# Step 4: Verify Send+Sync at compile time (implicit via static_assertions)
# If code compiles, Send+Sync is verified
```

### Manual Verification Checklist
- [ ] StorageOpen variant can be constructed
- [ ] StorageOpen Display trait formats message correctly
- [ ] static_assertions line compiles (proves Send+Sync)
- [ ] All existing 11 tests still pass
- [ ] 2 new tests pass (StorageOpen display, Send+Sync)

## Full State Verification (MANDATORY)

### Source of Truth
The final result is stored in:
- **File**: `crates/context-graph-graph/src/error.rs`
- **Cargo.toml**: `crates/context-graph-graph/Cargo.toml`

### Execute & Inspect Protocol
After making changes:
1. Run `cargo build -p context-graph-graph` - MUST compile
2. Run `cargo test -p context-graph-graph error -- --nocapture`
3. Read the test output - verify actual pass/fail counts
4. Grep for `StorageOpen` in error.rs to confirm it exists:
   ```bash
   grep -n "StorageOpen" crates/context-graph-graph/src/error.rs
   ```
5. Verify static_assertions line exists:
   ```bash
   grep -n "assert_impl_all" crates/context-graph-graph/src/error.rs
   ```

### Boundary & Edge Case Audit

**Edge Case 1: Empty path in StorageOpen**
```rust
let err = GraphError::StorageOpen {
    path: "".to_string(),
    cause: "invalid".to_string(),
};
println!("BEFORE: constructing StorageOpen with empty path");
let msg = err.to_string();
println!("AFTER: message = {}", msg);
// Expected: "Failed to open storage at : invalid"
assert!(msg.contains("Failed to open storage"));
```

**Edge Case 2: Unicode in error messages**
```rust
let err = GraphError::StorageOpen {
    path: "/данные/граф.db".to_string(),  // Russian
    cause: "权限被拒绝".to_string(),  // Chinese
};
println!("BEFORE: constructing with unicode");
let msg = err.to_string();
println!("AFTER: message = {}", msg);
assert!(msg.contains("данные"));
```

**Edge Case 3: Very long path**
```rust
let long_path = "a".repeat(10000);
let err = GraphError::StorageOpen {
    path: long_path.clone(),
    cause: "test".to_string(),
};
println!("BEFORE: constructing with 10000 char path");
let msg = err.to_string();
println!("AFTER: message length = {}", msg.len());
assert!(msg.len() > 10000);
```

### Evidence of Success
After completion, provide:
1. Full output of `cargo test -p context-graph-graph error`
2. Output of `grep -n "StorageOpen" crates/context-graph-graph/src/error.rs`
3. Output of `grep -n "assert_impl_all" crates/context-graph-graph/src/error.rs`
4. Confirmation that `cargo clippy` produces no warnings

## Implementation Approach

1. Open `crates/context-graph-graph/Cargo.toml`
2. Add `static_assertions = "1.1"` to [dependencies] if not present
3. Open `crates/context-graph-graph/src/error.rs`
4. Add `StorageOpen` variant after line 67
5. Add `static_assertions::assert_impl_all!` before `#[cfg(test)]`
6. Add 2 new tests
7. Run verification commands
8. Provide evidence of success

## Files Reference

```
crates/context-graph-graph/
├── Cargo.toml              # Add static_assertions dependency
└── src/
    └── error.rs            # Main file to modify (260 lines currently)
```

## SHERLOCK-HOLMES VERIFICATION REQUIRED

After completing implementation, spawn `sherlock-holmes` agent to:
1. Verify all changes compile
2. Verify all tests pass
3. Check for any regressions
4. Confirm Send+Sync bounds are satisfied
5. Validate error messages are descriptive

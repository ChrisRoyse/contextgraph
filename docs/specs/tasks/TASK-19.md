# TASK-19: Add IdentityCritical variant to ExtendedTriggerReason

## METADATA
| Field | Value |
|-------|-------|
| Task ID | TASK-19 (Original: TASK-IDENTITY-001) |
| Status | **COMPLETE** |
| Layer | Integration |
| Phase | 3 |
| Sequence | 19 |
| Implements | REQ-IDENTITY-001, Constitution AP-26, AP-38, IDENTITY-007 |
| Dependencies | **NONE** (This task has no blockers) |
| Blocks | TASK-20, TASK-21, TASK-26 |
| Est. Hours | 1 |

---

## CONTEXT FOR AI AGENT

### What This Task Is
Add a new enum variant `IdentityCritical { ic_value: f32 }` to the existing `ExtendedTriggerReason` enum. This enables dream consolidation to be triggered when Identity Continuity (IC) drops below 0.5 (identity crisis).

### Why This Matters
Per Constitution AP-26 and AP-38: **IC < 0.5 MUST trigger dream consolidation**. The system currently CANNOT trigger dreams based on identity crisis because this variant is missing. This is a CRITICAL constitutional violation.

### Current State (VERIFIED 2026-01-13)
**File**: `crates/context-graph-core/src/dream/types.rs` line 548

The `ExtendedTriggerReason` enum currently has these variants:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExtendedTriggerReason {
    IdleTimeout,      // Activity below 0.15 for idle_duration
    HighEntropy,      // Entropy above 0.7 for 5 minutes
    GpuOverload,      // GPU usage approaching 30% threshold
    MemoryPressure,   // Memory pressure requires consolidation
    Manual,           // User/system manual trigger
    Scheduled,        // Scheduled dream time
}
```

**MISSING**: `IdentityCritical { ic_value: f32 }` variant.

### CRITICAL: `Eq` Derive Must Be Removed

The current enum derives `Eq`. Adding `f32` field breaks `Eq` because `f32` does not implement `Eq` (due to NaN).

**SOLUTION**: Remove `Eq` from the derive macro. The enum will still have `PartialEq` which is sufficient for equality comparisons.

```rust
// BEFORE (won't compile with f32 field):
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]

// AFTER (will compile):
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
```

**Why this is safe**: No code in the codebase uses `Eq` bounds on `ExtendedTriggerReason`. HashMap keys don't use this type. This change has no downstream impact.

### Related Code (Already Exists)
- `WorkspaceEvent::IdentityCritical` exists in `crates/context-graph-core/src/gwt/workspace/events.rs:50` - this is a workspace event, NOT a trigger reason
- `IdentityContinuityMonitor` exists in `crates/context-graph-core/src/gwt/ego_node/monitor.rs` - monitors IC values
- `CrisisProtocol` exists in `crates/context-graph-core/src/gwt/ego_node/crisis_protocol.rs` - executes crisis response
- These will use `ExtendedTriggerReason::IdentityCritical` once added (wiring is TASK-21, TASK-26)

---

## EXACT IMPLEMENTATION

### Step 0: Remove `Eq` from Derive (REQUIRED FIRST)

**File**: `crates/context-graph-core/src/dream/types.rs`

**Location**: Line 547

**Action**: Remove `Eq` from the derive macro

```rust
// FIND THIS:
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]

// REPLACE WITH:
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
```

**WHY**: `f32` does not implement `Eq` (NaN != NaN). Removing `Eq` allows the `IdentityCritical { ic_value: f32 }` variant.

### Step 1: Modify ExtendedTriggerReason Enum

**File**: `crates/context-graph-core/src/dream/types.rs`

**Location**: Lines 548-566 (the `ExtendedTriggerReason` enum)

**Action**: Add `IdentityCritical` variant as the SECOND variant (after `Manual` for priority ordering)

```rust
/// Reason for triggering a dream cycle (extended).
///
/// Priority order (highest to lowest):
/// 1. Manual - User-initiated
/// 2. IdentityCritical - IC < 0.5 threshold (AP-26, AP-38, IDENTITY-007)
/// 3. GpuOverload - GPU approaching 30% budget
/// 4. HighEntropy - Entropy > 0.7 for 5 minutes
/// 5. MemoryPressure - Memory consolidation needed
/// 6. IdleTimeout - Activity below 0.15 for idle_duration
/// 7. Scheduled - Scheduled dream time
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ExtendedTriggerReason {
    /// User/system manual trigger (highest priority)
    Manual,

    /// Identity Continuity crisis (IC < 0.5)
    /// Constitution: AP-26, AP-38, IDENTITY-007
    /// Triggers dream consolidation to restore identity coherence.
    IdentityCritical {
        /// The IC value that triggered the crisis (must be < 0.5)
        ic_value: f32,
    },

    /// Activity below 0.15 for idle_duration (10 min)
    IdleTimeout,

    /// Entropy above 0.7 for 5 minutes
    HighEntropy,

    /// GPU usage approaching threshold (consolidation needed)
    GpuOverload,

    /// Memory pressure requires consolidation
    MemoryPressure,

    /// Scheduled dream time
    Scheduled,
}
```

### Step 2: Update Display Implementation

**File**: `crates/context-graph-core/src/dream/types.rs`

**Location**: Lines 568-578 (the `Display` impl)

**Action**: Add match arm for `IdentityCritical`

```rust
impl std::fmt::Display for ExtendedTriggerReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Manual => write!(f, "manual"),
            Self::IdentityCritical { ic_value } => {
                write!(f, "identity_critical(IC={:.3})", ic_value)
            }
            Self::IdleTimeout => write!(f, "idle_timeout"),
            Self::HighEntropy => write!(f, "high_entropy"),
            Self::GpuOverload => write!(f, "gpu_overload"),
            Self::MemoryPressure => write!(f, "memory_pressure"),
            Self::Scheduled => write!(f, "scheduled"),
        }
    }
}
```

### Step 3: Update Test for Display

**File**: `crates/context-graph-core/src/dream/types.rs`

**Location**: Lines 785-792 (the `test_extended_trigger_reason_display` test)

**Action**: Add assertion for `IdentityCritical` display

```rust
#[test]
fn test_extended_trigger_reason_display() {
    assert_eq!(ExtendedTriggerReason::Manual.to_string(), "manual");
    assert_eq!(
        ExtendedTriggerReason::IdentityCritical { ic_value: 0.423 }.to_string(),
        "identity_critical(IC=0.423)"
    );
    assert_eq!(ExtendedTriggerReason::IdleTimeout.to_string(), "idle_timeout");
    assert_eq!(ExtendedTriggerReason::HighEntropy.to_string(), "high_entropy");
    assert_eq!(ExtendedTriggerReason::GpuOverload.to_string(), "gpu_overload");
    assert_eq!(ExtendedTriggerReason::MemoryPressure.to_string(), "memory_pressure");
    assert_eq!(ExtendedTriggerReason::Scheduled.to_string(), "scheduled");
}
```

### Step 4: Add Validation Test

**File**: `crates/context-graph-core/src/dream/types.rs`

**Action**: Add a new test after the existing tests

```rust
#[test]
fn test_identity_critical_ic_value_serialization() {
    // Test serialization with real IC value
    let reason = ExtendedTriggerReason::IdentityCritical { ic_value: 0.35 };

    // Serialize to JSON
    let json = serde_json::to_string(&reason).expect("serialization should succeed");

    // Verify JSON structure
    assert!(json.contains("IdentityCritical"));
    assert!(json.contains("0.35"));

    // Deserialize back
    let deserialized: ExtendedTriggerReason =
        serde_json::from_str(&json).expect("deserialization should succeed");

    // Verify round-trip
    match deserialized {
        ExtendedTriggerReason::IdentityCritical { ic_value } => {
            assert!((ic_value - 0.35).abs() < 0.001,
                "IC value should survive round-trip: got {}", ic_value);
        }
        _ => panic!("Expected IdentityCritical variant"),
    }
}

#[test]
fn test_identity_critical_display_precision() {
    // Constitution requires 3 decimal places for IC display
    let test_cases = [
        (0.499, "identity_critical(IC=0.499)"),
        (0.1, "identity_critical(IC=0.100)"),
        (0.0, "identity_critical(IC=0.000)"),
        (0.123456, "identity_critical(IC=0.123)"),
    ];

    for (ic_value, expected) in test_cases {
        let reason = ExtendedTriggerReason::IdentityCritical { ic_value };
        assert_eq!(reason.to_string(), expected,
            "IC={} should display as {}", ic_value, expected);
    }
}

#[test]
fn test_identity_critical_equality() {
    let a = ExtendedTriggerReason::IdentityCritical { ic_value: 0.42 };
    let b = ExtendedTriggerReason::IdentityCritical { ic_value: 0.42 };
    let c = ExtendedTriggerReason::IdentityCritical { ic_value: 0.43 };

    assert_eq!(a, b, "Same IC values should be equal");
    assert_ne!(a, c, "Different IC values should not be equal");
    assert_ne!(a, ExtendedTriggerReason::Manual, "Different variants should not be equal");
}
```

---

## VERIFICATION COMMANDS

### Step 1: Compilation Check
```bash
cargo check -p context-graph-core 2>&1 | grep -E "(error|warning:.*ExtendedTriggerReason)"
```
**Expected**: No errors. Warnings about unused Kuramoto constants are OK.

### Step 2: Run Tests
```bash
cargo test -p context-graph-core -- --test-threads=1 trigger_reason 2>&1
```
**Expected**: All `trigger_reason` tests pass.

```bash
cargo test -p context-graph-core types 2>&1
```
**Expected**: All `types` tests pass.

### Step 3: Serialization Test
```bash
cargo test -p context-graph-core identity_critical 2>&1
```
**Expected**: New `identity_critical` tests pass.

---

## FULL STATE VERIFICATION

### Source of Truth
The source of truth is the `ExtendedTriggerReason` enum in `crates/context-graph-core/src/dream/types.rs`.

### Execute & Inspect Protocol
After implementation, run:
```bash
# 1. Verify the variant exists in compiled code
cargo doc -p context-graph-core --no-deps 2>&1 && \
  grep -A5 "IdentityCritical" target/doc/context_graph_core/dream/enum.ExtendedTriggerReason.html

# 2. Run serialization round-trip test
cargo test -p context-graph-core test_identity_critical_ic_value_serialization -- --nocapture

# 3. Run display precision test
cargo test -p context-graph-core test_identity_critical_display_precision -- --nocapture
```

### Boundary & Edge Case Audit

**Edge Case 1: IC at exactly 0.5 (boundary)**
```rust
// Synthetic input
let reason = ExtendedTriggerReason::IdentityCritical { ic_value: 0.5 };
// Expected: Creates variant, displays as "identity_critical(IC=0.500)"
// Note: 0.5 is the BOUNDARY - systems using this should use < 0.5 for triggering
println!("BEFORE: No IdentityCritical variant");
println!("AFTER:  {}", reason); // identity_critical(IC=0.500)
```

**Edge Case 2: IC at 0.0 (minimum)**
```rust
let reason = ExtendedTriggerReason::IdentityCritical { ic_value: 0.0 };
// Expected: Creates variant, displays as "identity_critical(IC=0.000)"
println!("BEFORE: No IdentityCritical variant");
println!("AFTER:  {}", reason); // identity_critical(IC=0.000)
```

**Edge Case 3: Negative IC (invalid but shouldn't panic)**
```rust
let reason = ExtendedTriggerReason::IdentityCritical { ic_value: -0.1 };
// Expected: Creates variant (no validation at enum level)
// Validation is responsibility of calling code (TriggerManager)
println!("BEFORE: No IdentityCritical variant");
println!("AFTER:  {}", reason); // identity_critical(IC=-0.100)
```

### Evidence of Success
After running tests, the log output should show:
```
running X tests
test dream::types::tests::test_extended_trigger_reason_display ... ok
test dream::types::tests::test_identity_critical_ic_value_serialization ... ok
test dream::types::tests::test_identity_critical_display_precision ... ok
test dream::types::tests::test_identity_critical_equality ... ok
```

---

## CONSTRAINTS (MUST FOLLOW)

1. **Variant MUST be named `IdentityCritical`** - exact spelling
2. **Field MUST be named `ic_value`** - exact spelling
3. **Field type MUST be `f32`** - not f64
4. **Display MUST show 3 decimal places** - format "{:.3}"
5. **Documentation MUST reference AP-26, AP-38, IDENTITY-007**
6. **NO backwards compatibility hacks** - just add the variant
7. **NO mock data in tests** - use real f32 values

---

## FILES TO MODIFY

| File | Lines | Action |
|------|-------|--------|
| `crates/context-graph-core/src/dream/types.rs` | 547 | Remove `Eq` from derive macro |
| `crates/context-graph-core/src/dream/types.rs` | 548-566 | Add `IdentityCritical` variant |
| `crates/context-graph-core/src/dream/types.rs` | 568-579 | Update Display impl |
| `crates/context-graph-core/src/dream/types.rs` | 785-793 | Update display test |
| `crates/context-graph-core/src/dream/types.rs` | (end of tests) | Add 3 new tests |

---

## OUT OF SCOPE

- TriggerManager wiring (TASK-21)
- IC threshold configuration (TASK-20)
- DreamEventListener integration (TASK-24)
- MCP tool exposure (TASK-38)

---

## CONSTITUTION REFERENCES

- **AP-26**: "IC<0.5 MUST trigger dream - no silent failures"
- **AP-38**: "IC<0.5 MUST auto-trigger dream"
- **IDENTITY-007**: "IC < 0.5 -> auto-trigger dream"
- **dream.trigger** (lines 255-256): Trigger conditions for dream cycle

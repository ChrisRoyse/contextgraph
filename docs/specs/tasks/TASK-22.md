# TASK-22: Implement GpuMonitor Trait and Error Types

## METADATA
| Field | Value |
|-------|-------|
| Task ID | TASK-22 (Original: TASK-DREAM-001) |
| Status | **COMPLETED** |
| Completed | 2026-01-13 |
| Layer | Integration |
| Phase | 3 |
| Sequence | 22 |
| Implements | REQ-DREAM-001, REQ-DREAM-002 |
| Dependencies | None (can start independently) |
| Blocks | TASK-23 (NvmlGpuMonitor implementation) |
| Est. Hours | 3 |

---

## AI AGENT CONTEXT - READ THIS FIRST

### What This Task Actually Does

Convert the current **concrete `GpuMonitor` struct** into a **trait-based abstraction** that allows:
1. Real NVML implementation for production
2. Mock implementations for testing
3. Stub fallback when no GPU is available

### Critical Current State (Verified 2026-01-13)

**File: `crates/context-graph-core/src/dream/triggers.rs`**

| Item | Line | Status |
|------|------|--------|
| `GpuMonitor` struct (STUB) | 591-598 | EXISTS - **Must be replaced with trait** |
| `GpuMonitor::get_usage()` | 609-620 | EXISTS - Returns 0.0 (STUB!) |
| `GpuMonitor::set_simulated_usage()` | 622-625 | EXISTS - Testing only |
| `MAX_GPU_USAGE = 0.30` constant | mod.rs:164 | EXISTS - Budget threshold |

**Current GpuMonitor struct code (lines 591-638):**
```rust
#[derive(Debug, Clone)]
pub struct GpuMonitor {
    /// Simulated GPU usage (for testing)
    simulated_usage: f32,
    /// Whether to use simulated values
    use_simulated: bool,
}

impl GpuMonitor {
    pub fn new() -> Self {
        Self {
            simulated_usage: 0.0,
            use_simulated: true, // Default to simulated until real impl
        }
    }

    pub fn get_usage(&self) -> f32 {
        if self.use_simulated {
            self.simulated_usage
        } else {
            // TODO(FUTURE): Implement real GPU monitoring via NVML
            0.0  // <-- THIS IS THE PROBLEM: Returns 0.0, not error
        }
    }
    // ...
}
```

**Problem**: `GpuMonitor` is a **stub struct** that always returns `0.0` for GPU usage. This violates Constitution AP-26 (no silent failures) because the system cannot actually monitor GPU.

### Why Two GPU Thresholds Exist

**THIS IS NOT A BUG - THERE ARE TWO DISTINCT THRESHOLDS:**

| Threshold | Value | Constitution Reference | Purpose |
|-----------|-------|----------------------|---------|
| **Eligibility** | 80% | `dream.trigger.gpu: "<80%"` (line 255) | System has capacity to START a dream |
| **Budget** | 30% | `dream.constraints.gpu: "<30%"` (line 273) | Dream must ABORT if GPU exceeds this |

**Logic Explained:**
1. **80% Eligibility**: When GPU < 80%, system is "idle enough" to begin dreaming
2. **30% Budget**: During dream execution, GPU usage must stay < 30% or dream aborts

**Current code only implements 30% budget check. The 80% eligibility check is MISSING.**

---

## EXACT IMPLEMENTATION REQUIREMENTS

### Step 1: Create `GpuMonitorError` enum

**File**: `crates/context-graph-core/src/dream/triggers.rs`

**Location**: Add BEFORE the existing `GpuMonitor` struct (around line 585)

```rust
use thiserror::Error;

/// GPU monitoring error types.
///
/// # Constitution Compliance
/// Per AP-26: Fail-fast, no silent failures. Return explicit errors.
#[derive(Debug, Error, Clone)]
pub enum GpuMonitorError {
    /// NVML library initialization failed
    #[error("NVML initialization failed: {0}")]
    NvmlInitFailed(String),

    /// No GPU devices detected in system
    #[error("No GPU devices found in system")]
    NoDevices,

    /// Failed to access specific GPU device
    #[error("Failed to access GPU device {index}: {message}")]
    DeviceAccessFailed { index: u32, message: String },

    /// GPU utilization query failed
    #[error("GPU utilization query failed: {0}")]
    UtilizationQueryFailed(String),

    /// NVML drivers not installed (laptop/server without GPU)
    #[error("NVML not available - GPU drivers not installed")]
    NvmlNotAvailable,

    /// GPU monitoring explicitly disabled
    #[error("GPU monitoring is disabled")]
    Disabled,
}
```

### Step 2: Create GPU threshold constants

**File**: `crates/context-graph-core/src/dream/triggers.rs`

**Location**: Add near top of file (after imports, around line 15)

```rust
/// GPU utilization thresholds per Constitution.
pub mod gpu_thresholds {
    /// Dream ELIGIBILITY threshold - dreams can START when GPU < 80%
    /// Constitution: `dream.trigger.gpu = "<80%"` (line 255)
    pub const GPU_ELIGIBILITY_THRESHOLD: f32 = 0.80;

    /// Dream BUDGET threshold - dreams must ABORT if GPU > 30%
    /// Constitution: `dream.constraints.gpu = "<30%"` (line 273)
    pub const GPU_BUDGET_THRESHOLD: f32 = 0.30;
}
```

### Step 3: Define `GpuMonitor` trait

**File**: `crates/context-graph-core/src/dream/triggers.rs`

**Location**: REPLACE the existing `GpuMonitor` struct (lines 591-638) with this trait

```rust
/// Trait for GPU monitoring abstraction.
///
/// Allows mocking in tests while enabling real NVML integration in production.
///
/// # Constitution References
/// - `dream.trigger.gpu: "<80%"` - Eligibility threshold
/// - `dream.constraints.gpu: "<30%"` - Budget threshold
/// - AP-26: "No silent failures" - Must return explicit errors
///
/// # Implementors
/// - `StubGpuMonitor`: Testing and systems without GPU
/// - `NvmlGpuMonitor`: Production NVIDIA GPU monitoring (TASK-23)
pub trait GpuMonitor: Send + Sync + std::fmt::Debug {
    /// Get current GPU utilization as fraction [0.0, 1.0].
    ///
    /// # Returns
    /// - `Ok(usage)` where `usage` is in [0.0, 1.0]
    /// - `Err(GpuMonitorError)` if query fails
    ///
    /// # Errors
    /// - `NvmlNotAvailable`: No GPU drivers installed
    /// - `NoDevices`: No GPUs detected
    /// - `UtilizationQueryFailed`: Query to GPU failed
    fn get_utilization(&mut self) -> Result<f32, GpuMonitorError>;

    /// Check if system is eligible to START a dream (GPU < 80%).
    ///
    /// # Constitution
    /// `dream.trigger.gpu = "<80%"` (line 255)
    ///
    /// # Returns
    /// - `Ok(true)` if GPU < 80% (can start dream)
    /// - `Ok(false)` if GPU >= 80% (too busy for dream)
    /// - `Err(_)` if utilization query fails
    fn is_eligible_for_dream(&mut self) -> Result<bool, GpuMonitorError> {
        let usage = self.get_utilization()?;
        Ok(usage < gpu_thresholds::GPU_ELIGIBILITY_THRESHOLD)
    }

    /// Check if dream should ABORT due to GPU budget exceeded (> 30%).
    ///
    /// # Constitution
    /// `dream.constraints.gpu = "<30%"` (line 273)
    ///
    /// # Returns
    /// - `Ok(true)` if GPU > 30% (must abort dream)
    /// - `Ok(false)` if GPU <= 30% (can continue dream)
    /// - `Err(_)` if utilization query fails
    fn should_abort_dream(&mut self) -> Result<bool, GpuMonitorError> {
        let usage = self.get_utilization()?;
        Ok(usage > gpu_thresholds::GPU_BUDGET_THRESHOLD)
    }

    /// Check if GPU monitoring is available.
    ///
    /// # Returns
    /// `true` if GPU can be queried, `false` otherwise
    fn is_available(&self) -> bool;
}
```

### Step 4: Implement `StubGpuMonitor`

**File**: `crates/context-graph-core/src/dream/triggers.rs`

**Location**: After the trait definition

```rust
/// Stub GPU monitor for testing and systems without GPU.
///
/// # Usage
/// - Unit tests: Use `set_usage()` to control behavior
/// - Systems without GPU: Returns `Err(NvmlNotAvailable)` by default
///
/// # Constitution Compliance
/// Per AP-26: When `simulate_unavailable` is true,
/// returns `Err(GpuMonitorError::NvmlNotAvailable)` - NOT 0.0.
#[derive(Debug, Clone)]
pub struct StubGpuMonitor {
    /// Simulated GPU usage [0.0, 1.0]
    simulated_usage: Option<f32>,

    /// Whether to simulate NVML unavailable error
    simulate_unavailable: bool,
}

impl StubGpuMonitor {
    /// Create stub that simulates NVML not available.
    ///
    /// Per AP-26: Returns error, not 0.0.
    pub fn unavailable() -> Self {
        Self {
            simulated_usage: None,
            simulate_unavailable: true,
        }
    }

    /// Create stub with specific simulated usage.
    ///
    /// Use for testing specific GPU load scenarios.
    pub fn with_usage(usage: f32) -> Self {
        Self {
            simulated_usage: Some(usage.clamp(0.0, 1.0)),
            simulate_unavailable: false,
        }
    }

    /// Set simulated GPU usage for testing.
    pub fn set_usage(&mut self, usage: f32) {
        self.simulated_usage = Some(usage.clamp(0.0, 1.0));
        self.simulate_unavailable = false;
    }

    /// Configure to simulate NVML unavailable error.
    pub fn set_unavailable(&mut self) {
        self.simulated_usage = None;
        self.simulate_unavailable = true;
    }
}

impl Default for StubGpuMonitor {
    /// Default: NVML not available (fail-safe per AP-26).
    fn default() -> Self {
        Self::unavailable()
    }
}

impl GpuMonitor for StubGpuMonitor {
    fn get_utilization(&mut self) -> Result<f32, GpuMonitorError> {
        if self.simulate_unavailable {
            return Err(GpuMonitorError::NvmlNotAvailable);
        }

        match self.simulated_usage {
            Some(usage) => Ok(usage),
            None => Err(GpuMonitorError::NvmlNotAvailable),
        }
    }

    fn is_available(&self) -> bool {
        !self.simulate_unavailable && self.simulated_usage.is_some()
    }
}
```

### Step 5: Update `mod.rs` exports

**File**: `crates/context-graph-core/src/dream/mod.rs`

**Location**: Line 98 (current exports)

**Change from:**
```rust
pub use triggers::{EntropyCalculator, GpuMonitor, TriggerConfig, TriggerManager};
```

**Change to:**
```rust
pub use triggers::{
    EntropyCalculator,
    GpuMonitor,
    GpuMonitorError,
    StubGpuMonitor,
    TriggerConfig,
    TriggerManager,
    gpu_thresholds,
};
```

### Step 6: Add tests

**File**: `crates/context-graph-core/src/dream/triggers.rs`

**Location**: Add to `#[cfg(test)] mod tests` section (at end of file)

```rust
    // ============ GpuMonitor Trait Tests ============

    #[test]
    fn test_gpu_thresholds_constitution_compliance() {
        use super::gpu_thresholds::*;

        assert_eq!(
            GPU_ELIGIBILITY_THRESHOLD, 0.80,
            "Eligibility threshold must be 0.80 per Constitution dream.trigger.gpu"
        );
        assert_eq!(
            GPU_BUDGET_THRESHOLD, 0.30,
            "Budget threshold must be 0.30 per Constitution dream.constraints.gpu"
        );

        // Verify eligibility > budget (makes logical sense)
        assert!(
            GPU_ELIGIBILITY_THRESHOLD > GPU_BUDGET_THRESHOLD,
            "Eligibility (80%) must be greater than budget (30%)"
        );
    }

    #[test]
    fn test_stub_gpu_monitor_unavailable_returns_error() {
        let mut monitor = StubGpuMonitor::unavailable();

        let result = monitor.get_utilization();
        assert!(result.is_err(), "Unavailable GPU should return error, not 0.0");

        match result {
            Err(GpuMonitorError::NvmlNotAvailable) => {}, // Expected
            Err(other) => panic!("Expected NvmlNotAvailable, got {:?}", other),
            Ok(val) => panic!("Expected error, got Ok({})", val),
        }
    }

    #[test]
    fn test_stub_gpu_monitor_with_usage() {
        let mut monitor = StubGpuMonitor::with_usage(0.25);

        let usage = monitor.get_utilization().expect("Should return usage");
        assert!((usage - 0.25).abs() < 0.001, "Usage should be 0.25");
        assert!(monitor.is_available(), "Should be available");
    }

    #[test]
    fn test_stub_gpu_monitor_set_usage() {
        let mut monitor = StubGpuMonitor::unavailable();

        // Initially unavailable
        assert!(monitor.get_utilization().is_err());

        // Set usage makes it available
        monitor.set_usage(0.50);
        assert!(monitor.is_available());
        assert_eq!(monitor.get_utilization().unwrap(), 0.50);
    }

    #[test]
    fn test_stub_gpu_monitor_clamping() {
        let mut monitor = StubGpuMonitor::with_usage(1.5);
        assert_eq!(monitor.get_utilization().unwrap(), 1.0, "Should clamp to 1.0");

        monitor.set_usage(-0.5);
        assert_eq!(monitor.get_utilization().unwrap(), 0.0, "Should clamp to 0.0");
    }

    #[test]
    fn test_is_eligible_for_dream_below_threshold() {
        let mut monitor = StubGpuMonitor::with_usage(0.50); // 50% < 80%

        assert!(
            monitor.is_eligible_for_dream().unwrap(),
            "50% usage should be eligible for dream (< 80%)"
        );
    }

    #[test]
    fn test_is_eligible_for_dream_at_threshold() {
        let mut monitor = StubGpuMonitor::with_usage(0.80); // 80% = 80%

        assert!(
            !monitor.is_eligible_for_dream().unwrap(),
            "80% usage should NOT be eligible (must be < 80%, not <= 80%)"
        );
    }

    #[test]
    fn test_is_eligible_for_dream_above_threshold() {
        let mut monitor = StubGpuMonitor::with_usage(0.90); // 90% > 80%

        assert!(
            !monitor.is_eligible_for_dream().unwrap(),
            "90% usage should NOT be eligible for dream"
        );
    }

    #[test]
    fn test_should_abort_dream_below_budget() {
        let mut monitor = StubGpuMonitor::with_usage(0.25); // 25% < 30%

        assert!(
            !monitor.should_abort_dream().unwrap(),
            "25% usage should NOT abort dream (< 30% budget)"
        );
    }

    #[test]
    fn test_should_abort_dream_at_budget() {
        let mut monitor = StubGpuMonitor::with_usage(0.30); // 30% = 30%

        assert!(
            !monitor.should_abort_dream().unwrap(),
            "30% usage should NOT abort dream (must be > 30%, not >= 30%)"
        );
    }

    #[test]
    fn test_should_abort_dream_above_budget() {
        let mut monitor = StubGpuMonitor::with_usage(0.35); // 35% > 30%

        assert!(
            monitor.should_abort_dream().unwrap(),
            "35% usage should abort dream (> 30% budget)"
        );
    }

    #[test]
    fn test_gpu_monitor_error_display() {
        let errors = [
            (GpuMonitorError::NvmlInitFailed("test".to_string()), "NVML initialization failed"),
            (GpuMonitorError::NoDevices, "No GPU devices found"),
            (GpuMonitorError::DeviceAccessFailed { index: 0, message: "test".to_string() }, "Failed to access GPU device"),
            (GpuMonitorError::UtilizationQueryFailed("test".to_string()), "GPU utilization query failed"),
            (GpuMonitorError::NvmlNotAvailable, "NVML not available"),
            (GpuMonitorError::Disabled, "GPU monitoring is disabled"),
        ];

        for (error, expected_prefix) in errors {
            let display = error.to_string();
            assert!(
                display.contains(expected_prefix.split(':').next().unwrap().trim()),
                "Error display '{}' should contain '{}'",
                display, expected_prefix
            );
        }
    }

    #[test]
    fn test_stub_default_is_unavailable() {
        // Per AP-26: Default should fail-safe, not return 0.0
        let mut monitor = StubGpuMonitor::default();

        assert!(!monitor.is_available(), "Default should be unavailable");
        assert!(
            monitor.get_utilization().is_err(),
            "Default should return error, not 0.0"
        );
    }

    #[test]
    fn test_gpu_monitor_boundary_values() {
        // Edge case: exactly at thresholds
        let test_cases: [(f32, bool, bool); 7] = [
            // (usage, is_eligible, should_abort)
            (0.00, true, false),   // Minimum: eligible, don't abort
            (0.29, true, false),   // Just under budget: eligible, don't abort
            (0.30, true, false),   // At budget: eligible, don't abort (> not >=)
            (0.31, true, true),    // Just over budget: eligible but should abort
            (0.79, true, true),    // Just under eligibility: eligible but over budget
            (0.80, false, true),   // At eligibility: NOT eligible, should abort
            (1.00, false, true),   // Maximum: NOT eligible, should abort
        ];

        for (usage, expected_eligible, expected_abort) in test_cases {
            let mut monitor = StubGpuMonitor::with_usage(usage);

            assert_eq!(
                monitor.is_eligible_for_dream().unwrap(),
                expected_eligible,
                "Usage {} eligibility mismatch", usage
            );
            assert_eq!(
                monitor.should_abort_dream().unwrap(),
                expected_abort,
                "Usage {} abort mismatch", usage
            );
        }
    }
```

---

## FULL STATE VERIFICATION PROTOCOL

### Source of Truth
1. `GpuMonitor` trait definition in `triggers.rs`
2. `StubGpuMonitor` struct implementing the trait
3. `gpu_thresholds` module constants
4. Test results from `cargo test gpu_monitor`

### Execute & Inspect Protocol

After implementation, run these verification commands:

```bash
# 1. Verify trait exists
grep -A 20 "pub trait GpuMonitor" crates/context-graph-core/src/dream/triggers.rs

# 2. Verify error enum exists
grep -A 15 "pub enum GpuMonitorError" crates/context-graph-core/src/dream/triggers.rs

# 3. Verify threshold constants
grep -A 5 "GPU_ELIGIBILITY_THRESHOLD\|GPU_BUDGET_THRESHOLD" crates/context-graph-core/src/dream/triggers.rs

# 4. Verify StubGpuMonitor implements trait
grep -A 10 "impl GpuMonitor for StubGpuMonitor" crates/context-graph-core/src/dream/triggers.rs

# 5. Verify exports in mod.rs
grep "GpuMonitor\|GpuMonitorError\|StubGpuMonitor\|gpu_thresholds" crates/context-graph-core/src/dream/mod.rs

# 6. Run all GPU monitor tests
cargo test -p context-graph-core gpu_monitor -- --nocapture

# 7. Verify compilation
cargo check -p context-graph-core
```

### Boundary & Edge Case Audit

**Edge Case 1: GPU at exactly 80% (eligibility threshold)**
```
INPUT: StubGpuMonitor::with_usage(0.80)
CALL: is_eligible_for_dream()
EXPECTED: Ok(false) - 0.80 is NOT < 0.80
BEFORE STATE: monitor.simulated_usage = Some(0.80)
AFTER STATE: No state change (read-only query)
VERIFY: cargo test test_is_eligible_for_dream_at_threshold -- --nocapture
OUTPUT LOG: "80% usage should NOT be eligible (must be < 80%, not <= 80%)"
```

**Edge Case 2: GPU at exactly 30% (budget threshold)**
```
INPUT: StubGpuMonitor::with_usage(0.30)
CALL: should_abort_dream()
EXPECTED: Ok(false) - 0.30 is NOT > 0.30
BEFORE STATE: monitor.simulated_usage = Some(0.30)
AFTER STATE: No state change
VERIFY: cargo test test_should_abort_dream_at_budget -- --nocapture
OUTPUT LOG: "30% usage should NOT abort dream (must be > 30%, not >= 30%)"
```

**Edge Case 3: NVML not available (no GPU)**
```
INPUT: StubGpuMonitor::unavailable()
CALL: get_utilization()
EXPECTED: Err(GpuMonitorError::NvmlNotAvailable)
BEFORE STATE: monitor.simulate_unavailable = true, simulated_usage = None
AFTER STATE: No state change
VERIFY: cargo test test_stub_gpu_monitor_unavailable_returns_error -- --nocapture
OUTPUT LOG: "Expected NvmlNotAvailable error"
```

### Evidence of Success Log

After running all tests, expect:
```
running 14 tests
test dream::triggers::tests::test_gpu_thresholds_constitution_compliance ... ok
test dream::triggers::tests::test_stub_gpu_monitor_unavailable_returns_error ... ok
test dream::triggers::tests::test_stub_gpu_monitor_with_usage ... ok
test dream::triggers::tests::test_stub_gpu_monitor_set_usage ... ok
test dream::triggers::tests::test_stub_gpu_monitor_clamping ... ok
test dream::triggers::tests::test_is_eligible_for_dream_below_threshold ... ok
test dream::triggers::tests::test_is_eligible_for_dream_at_threshold ... ok
test dream::triggers::tests::test_is_eligible_for_dream_above_threshold ... ok
test dream::triggers::tests::test_should_abort_dream_below_budget ... ok
test dream::triggers::tests::test_should_abort_dream_at_budget ... ok
test dream::triggers::tests::test_should_abort_dream_above_budget ... ok
test dream::triggers::tests::test_gpu_monitor_error_display ... ok
test dream::triggers::tests::test_stub_default_is_unavailable ... ok
test dream::triggers::tests::test_gpu_monitor_boundary_values ... ok

test result: ok. 14 passed; 0 failed; 0 ignored
```

---

## MANUAL TESTING PROTOCOL

### Test 1: Compile Check
```bash
cargo check -p context-graph-core 2>&1 | head -30
```
**Expected**: No errors related to GpuMonitor

### Test 2: Verify Trait Methods Work
```bash
cargo test -p context-graph-core test_stub_gpu_monitor -- --nocapture 2>&1
```
**Expected**: All stub tests pass

### Test 3: Verify Constitution Thresholds
```bash
cargo test -p context-graph-core test_gpu_thresholds_constitution_compliance -- --nocapture
```
**Expected**: Test passes, no assertion failures

### Test 4: Full Dream Module Test Suite
```bash
cargo test -p context-graph-core dream:: -- --test-threads=1 2>&1 | tail -30
```
**Expected**: All dream tests pass including new GPU monitor tests

### Test 5: Verify Fail-Fast Behavior (AP-26)
```bash
cargo test -p context-graph-core test_stub_default_is_unavailable -- --nocapture
```
**Expected**: Default StubGpuMonitor returns error, NOT 0.0

---

## CONSTRAINTS (MUST FOLLOW)

1. **Trait MUST be `Send + Sync + Debug`** - Required for async context and logging
2. **Default implementations use trait methods** - `is_eligible_for_dream()` and `should_abort_dream()` have default impls
3. **80% for eligibility, 30% for budget** - Two DISTINCT thresholds per Constitution
4. **`<` for eligibility, `>` for abort** - Strict inequalities, NOT `<=` or `>=`
5. **StubGpuMonitor::default() returns error** - Per AP-26, fail-fast not 0.0
6. **No backwards compatibility** - Replace old struct completely
7. **All errors via `GpuMonitorError`** - No panics, no unwraps
8. **Tests use REAL thresholds** - No mock values for threshold tests

---

## FILES TO MODIFY

| File | Lines | Action |
|------|-------|--------|
| `crates/context-graph-core/src/dream/triggers.rs` | ~15 | Add `gpu_thresholds` module |
| `crates/context-graph-core/src/dream/triggers.rs` | ~585 | Add `GpuMonitorError` enum |
| `crates/context-graph-core/src/dream/triggers.rs` | 591-638 | REPLACE `GpuMonitor` struct with trait |
| `crates/context-graph-core/src/dream/triggers.rs` | After trait | Add `StubGpuMonitor` impl |
| `crates/context-graph-core/src/dream/triggers.rs` | tests section | Add 14 new tests |
| `crates/context-graph-core/src/dream/mod.rs` | 98 | Update exports |
| `crates/context-graph-core/Cargo.toml` | dependencies | Verify `thiserror` present |

---

## OUT OF SCOPE (DO NOT IMPLEMENT)

- `NvmlGpuMonitor` implementation (TASK-23)
- Integration with `TriggerManager` (already uses `GpuTriggerState` - separate refactor)
- ROCm/AMD GPU support (future work)
- Multi-GPU aggregation (future work)

---

## DEPENDENCIES CHECK

**Before starting, verify `thiserror` is available:**
```bash
grep "thiserror" crates/context-graph-core/Cargo.toml
```

**Expected output:**
```
thiserror = "1.0"
```

If not present, add to `[dependencies]`:
```toml
thiserror = "1.0"
```

---

## CONSTITUTION REFERENCES

| Reference | Line | Value | Usage |
|-----------|------|-------|-------|
| `dream.trigger.gpu` | 255 | `<80%` | Eligibility to START dream |
| `dream.constraints.gpu` | 273 | `<30%` | Budget limit during dream |
| `AP-26` | 88 | "no silent failures" | Must return errors, not 0.0 |
| `perf.latency.dream_wake` | 130 | `<100ms` | Wake latency budget |

---

## RELATED TASKS

| Task | Relationship |
|------|-------------|
| TASK-20 | COMPLETE - TriggerConfig exists |
| TASK-21 | COMPLETE - TriggerManager has IC checking |
| TASK-23 | Blocked - Will implement NvmlGpuMonitor using this trait |
| TASK-24 | Related - DreamEventListener may use GPU status |

---

## TROUBLESHOOTING

### If `thiserror` not found:
```bash
cargo add thiserror -p context-graph-core
```

### If "conflicting implementations" error:
- Ensure old `GpuMonitor` struct is COMPLETELY removed (lines 591-638)
- Ensure `impl GpuMonitor for StubGpuMonitor` exists only once

### If tests fail with "trait bound not satisfied":
- Verify `StubGpuMonitor` has `#[derive(Debug, Clone)]`
- Verify trait has `Send + Sync + std::fmt::Debug` bounds

### If "unused import gpu_thresholds":
- Ensure mod.rs exports it: `pub use triggers::gpu_thresholds;`
- Ensure tests use: `use super::gpu_thresholds::*;`

### If "cannot find type GpuMonitor in this scope":
- Verify trait definition comes BEFORE `StubGpuMonitor`
- Verify `impl GpuMonitor for StubGpuMonitor` is after both definitions

---

## WHAT SUCCESS LOOKS LIKE

After completing this task:

1. **`GpuMonitor` is a trait**, not a struct
2. **`StubGpuMonitor` implements the trait** for testing
3. **Two threshold constants exist**: 0.80 (eligibility), 0.30 (budget)
4. **Default behavior is fail-safe** - returns error, not 0.0
5. **All 14 tests pass** with real threshold values
6. **No compilation warnings** related to GPU monitor
7. **Exports available** from `context_graph_core::dream`

---

## TRIGGERING PROCESS CHAIN (For Output Verification)

**Understanding the data flow for verifying outputs actually exist:**

```
[Trigger Event: TriggerManager checks if dream can start]
        |
        v
[Process: GpuMonitor::is_eligible_for_dream() called]
        |
        v
[Query: get_utilization() -> Result<f32, GpuMonitorError>]
        |
        v
[Compare: usage < GPU_ELIGIBILITY_THRESHOLD (0.80)]
        |
        v
[Outcome Y: Ok(true) = can start dream, Ok(false) = too busy, Err = GPU unavailable]
```

**To verify the process worked (manual verification):**

```rust
// 1. Create monitor with KNOWN state (synthetic input)
let mut monitor = StubGpuMonitor::with_usage(0.50);

// 2. BEFORE STATE - Print what we're starting with
println!("BEFORE STATE:");
println!("  is_available: {}", monitor.is_available());
println!("  simulated_usage: {:?}", monitor.get_utilization());

// 3. EXECUTE - Call the method being tested
let eligible = monitor.is_eligible_for_dream();

// 4. AFTER STATE - Verify outcome
println!("AFTER STATE:");
println!("  is_eligible_for_dream() returned: {:?}", eligible);

// 5. ASSERTION - Verify expected output
assert_eq!(eligible, Ok(true), "50% GPU usage should be eligible for dream");
println!("VERIFICATION: 50% < 80% = eligible for dream - PASSED");
```

**Source of Truth Verification:**
- The `StubGpuMonitor.simulated_usage` field IS the source of truth
- After calling `get_utilization()`, verify it returns the expected value
- The comparison `usage < GPU_ELIGIBILITY_THRESHOLD` produces the final boolean

**How to manually verify in tests:**
1. Set known input: `StubGpuMonitor::with_usage(0.50)`
2. Call method: `is_eligible_for_dream()`
3. Check output: `assert_eq!(result, Ok(true))`
4. The test framework reports PASS/FAIL as evidence

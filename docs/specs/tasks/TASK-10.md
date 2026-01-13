# TASK-10: Add KURAMOTO_N constant (13 oscillators)

## STATUS: COMPLETE ✓

**Task ID**: TASK-10 (Original: TASK-GWT-001)
**Layer**: Core Logic | **Phase**: 2
**Sequence**: 10
**Estimated Hours**: 1
**Depends On**: NONE
**Blocks**: TASK-11 (KuramotoNetwork), TASK-12 (KuramotoStepper)

---

## 🚨 CRITICAL: CURRENT STATE ANALYSIS (2026-01-13)

### THE PROBLEM

The **`KURAMOTO_N` constant is currently 8, but constitution mandates 13**.

**Current file**: `crates/context-graph-core/src/layers/coherence/constants.rs`
**Line 16**: `pub const KURAMOTO_N: usize = 8;` ← **WRONG - MUST BE 13**

### Constitution Reference (docs2/constitution.yaml)

```yaml
gwt:
  kuramoto:
    formula: "dθᵢ/dt = ωᵢ + (K/N)Σⱼ sin(θⱼ-θᵢ)"
    frequencies: { E1: 40γ, E2: 8α, E3: 8α, E4: 8α, E5: 25β, E6: 4θ, E7: 25β, E8: 12αβ, E9: 80γ+, E10: 40γ, E11: 15β, E12: 60γ+, E13: 4θ }
```

**13 frequencies = 13 oscillators**, one per embedding space (E1-E13).

### What Already Exists (Verified)

| File | Current State |
|------|---------------|
| `crates/context-graph-core/src/layers/coherence/constants.rs` | `KURAMOTO_N: usize = 8` (WRONG) |
| `crates/context-graph-core/src/layers/coherence/network.rs` | Uses `base_frequencies` array of 8 elements (WRONG) |
| `crates/context-graph-mcp/src/handlers/tests/.../kuramoto_sync.rs` | Tests expect 13 oscillators (CORRECT expectation, currently FAILING) |

### MCP Test Compilation Status

The MCP crate **cannot compile** due to unrelated async trait issues (TASK-08/09 changed traits, implementations not yet updated). However, the test expectations for 13 oscillators are already written:

```rust
// crates/context-graph-mcp/src/handlers/tests/phase3_gwt_consciousness/kuramoto_sync.rs:73-78
let phases = data.get("phases").and_then(|v| v.as_array()).expect("phases must exist");
assert_eq!(phases.len(), 13, "[FSV] CRITICAL: Must have 13 oscillator phases");
```

---

## WHAT THIS TASK MUST DO

### Deliverable 1: Update KURAMOTO_N to 13

**File**: `crates/context-graph-core/src/layers/coherence/constants.rs`

```rust
// BEFORE (line 16)
pub const KURAMOTO_N: usize = 8;

// AFTER
/// Number of oscillators in Kuramoto network (one per embedding space E1-E13).
/// Constitution: gwt.kuramoto.frequencies (13 values)
pub const KURAMOTO_N: usize = 13;
```

### Deliverable 2: Add KURAMOTO_BASE_FREQUENCIES Array

**File**: `crates/context-graph-core/src/layers/coherence/constants.rs`

Add this new constant (constitution values from `gwt.kuramoto.frequencies`):

```rust
/// Base frequencies for each oscillator (Hz).
///
/// Constitution mapping (gwt.kuramoto.frequencies):
/// - [0]  E1_Semantic       = 40.0 Hz (gamma_fast - perception binding)
/// - [1]  E2_TemporalRecent = 8.0 Hz  (theta_slow - memory consolidation)
/// - [2]  E3_TemporalPeriod = 8.0 Hz  (theta_2 - hippocampal rhythm)
/// - [3]  E4_Entity         = 8.0 Hz  (theta_3 - prefrontal sync)
/// - [4]  E5_Causal         = 25.0 Hz (beta_1 - motor planning)
/// - [5]  E6_Sparse         = 4.0 Hz  (delta - deep sleep)
/// - [6]  E7_Code           = 25.0 Hz (beta_2 - active thinking)
/// - [7]  E8_Emotional      = 12.0 Hz (alpha - relaxed awareness)
/// - [8]  E9_HDC            = 80.0 Hz (high_gamma - cross-modal binding)
/// - [9]  E10_Multimodal    = 40.0 Hz (gamma_mid - attention)
/// - [10] E11_EntityKG      = 15.0 Hz (beta_3 - cognitive control)
/// - [11] E12_LateInteract  = 60.0 Hz (gamma_low - sensory processing)
/// - [12] E13_SPLADE        = 4.0 Hz  (delta_slow - slow wave sleep)
pub const KURAMOTO_BASE_FREQUENCIES: [f32; KURAMOTO_N] = [
    40.0,  // E1  gamma_fast
    8.0,   // E2  theta_slow
    8.0,   // E3  theta_2
    8.0,   // E4  theta_3
    25.0,  // E5  beta_1
    4.0,   // E6  delta
    25.0,  // E7  beta_2
    12.0,  // E8  alpha
    80.0,  // E9  high_gamma
    40.0,  // E10 gamma_mid
    15.0,  // E11 beta_3
    60.0,  // E12 gamma_low
    4.0,   // E13 delta_slow
];

/// Default coupling strength for Kuramoto network.
/// Constitution: 2.0 (already correct in KURAMOTO_K)
pub const KURAMOTO_DEFAULT_COUPLING: f32 = 0.5;

/// Step interval for Kuramoto stepper (10ms = 100Hz update rate).
pub const KURAMOTO_STEP_INTERVAL_MS: u64 = 10;
```

### Deliverable 3: Update Re-exports in mod.rs

**File**: `crates/context-graph-core/src/layers/coherence/mod.rs`

Add re-export for new constants:

```rust
// Current line 54-56
#[allow(deprecated)]
pub use constants::{
    FRAGMENTATION_THRESHOLD, GW_THRESHOLD, HYPERSYNC_THRESHOLD, INTEGRATION_STEPS, KURAMOTO_DT,
    KURAMOTO_K, KURAMOTO_N,
};

// ADD after existing re-exports
pub use constants::{KURAMOTO_BASE_FREQUENCIES, KURAMOTO_DEFAULT_COUPLING, KURAMOTO_STEP_INTERVAL_MS};
```

### Deliverable 4: Update network.rs to Use Constants

**File**: `crates/context-graph-core/src/layers/coherence/network.rs`

The `KuramotoNetwork::new()` method (lines 31-50) currently hardcodes 8 frequencies:

```rust
// BEFORE (lines 33-34)
let base_frequencies = [40.0, 8.0, 25.0, 4.0, 12.0, 15.0, 60.0, 40.0];

// AFTER - Use the constant
use super::constants::KURAMOTO_BASE_FREQUENCIES;

// In new() method (line 36 onwards):
let oscillators: Vec<_> = (0..n)
    .map(|i| {
        let phase = (i as f32 / n as f32) * 2.0 * std::f32::consts::PI;
        // Use constitution frequencies with slight variation
        let freq = KURAMOTO_BASE_FREQUENCIES[i % KURAMOTO_BASE_FREQUENCIES.len()]
            * (1.0 + (i as f32 * 0.02));  // Reduced variance for stability
        KuramotoOscillator::new(phase, freq)
    })
    .collect();
```

---

## VERIFICATION PROTOCOL

### Step 1: Compile Check (MUST PASS)

```bash
cargo check -p context-graph-core
# Expected: No errors
```

### Step 2: Run Kuramoto Tests

```bash
cargo test -p context-graph-core kuramoto
# Expected: All 20 tests pass
```

### Step 3: Verify Constant Value

```bash
grep -n "KURAMOTO_N.*=" crates/context-graph-core/src/layers/coherence/constants.rs
# Expected: Line with "KURAMOTO_N: usize = 13"
```

### Step 4: Verify Array Length

```bash
grep -A 15 "KURAMOTO_BASE_FREQUENCIES" crates/context-graph-core/src/layers/coherence/constants.rs | grep -c "Hz"
# Expected: 13 (one comment per frequency)
```

---

## FULL STATE VERIFICATION PROTOCOL

### Source of Truth

- **Primary**: `constants.rs::KURAMOTO_N` value
- **Secondary**: `constants.rs::KURAMOTO_BASE_FREQUENCIES` array length
- **Tertiary**: Tests that verify network has 13 oscillators

### Execute & Inspect Protocol

**1. BEFORE State Capture:**
```bash
echo "=== BEFORE STATE ===" > /tmp/task10-verification.log
echo "KURAMOTO_N value:" >> /tmp/task10-verification.log
grep "KURAMOTO_N.*=" crates/context-graph-core/src/layers/coherence/constants.rs >> /tmp/task10-verification.log
echo "Network base_frequencies count:" >> /tmp/task10-verification.log
grep -c "40.0\|8.0\|25.0\|4.0\|12.0\|15.0\|60.0\|80.0" crates/context-graph-core/src/layers/coherence/network.rs >> /tmp/task10-verification.log
```

**2. AFTER Modification:**
```bash
echo "" >> /tmp/task10-verification.log
echo "=== AFTER STATE ===" >> /tmp/task10-verification.log
echo "KURAMOTO_N value:" >> /tmp/task10-verification.log
grep "KURAMOTO_N.*=" crates/context-graph-core/src/layers/coherence/constants.rs >> /tmp/task10-verification.log
echo "KURAMOTO_BASE_FREQUENCIES exists:" >> /tmp/task10-verification.log
grep -c "KURAMOTO_BASE_FREQUENCIES" crates/context-graph-core/src/layers/coherence/constants.rs >> /tmp/task10-verification.log
```

**3. Evidence of Success:**
```bash
# Compile and run test
cargo test -p context-graph-core layers::coherence::network::tests::test_network_creation 2>&1 | tee -a /tmp/task10-verification.log

# Verify the test shows correct network size
grep -E "(test_network_creation|VERIFIED)" /tmp/task10-verification.log
```

### Boundary & Edge Case Audit

#### Edge Case 1: Zero Oscillators
```rust
// Input: KuramotoNetwork::new(0, 2.0)
// Expected: Empty network, order_parameter() returns 0.0
// Verification: Test in network.rs handles n=0 gracefully
```

#### Edge Case 2: Network Size Mismatch
```rust
// Input: Network created with n != KURAMOTO_N
// Expected: Still works (network is configurable), but layer.rs uses KURAMOTO_N
// Verification: with_kuramoto(n, k) allows custom sizes
```

#### Edge Case 3: Frequency Array Access
```rust
// Input: i >= KURAMOTO_BASE_FREQUENCIES.len()
// Expected: Uses modulo wrap (i % len) for safety
// Verification: Line in new() already uses modulo
```

---

## MANUAL TESTING WITH SYNTHETIC DATA

### Test 1: Network Creation with 13 Oscillators

```bash
# Run specific test
cargo test -p context-graph-core test_network_creation -- --nocapture 2>&1 | grep -E "VERIFIED|oscillators|K="

# Expected output:
# [VERIFIED] Network creation with 13 oscillators and K=2.0
```

### Test 2: Order Parameter Range

```bash
cargo test -p context-graph-core test_order_parameter_range -- --nocapture 2>&1 | grep -E "VERIFIED|r ="

# Expected output:
# [VERIFIED] Order parameter r ∈ [0, 1]: r = <some value>
```

### Test 3: Full Verification Test

```bash
cargo test -p context-graph-core fsv_l5_kuramoto -- --nocapture 2>&1 | grep -E "VERIFIED|KURAMOTO"

# Expected output includes:
# KURAMOTO network: 13 oscillators
```

---

## FILES TO MODIFY

| File | Action | Lines |
|------|--------|-------|
| `crates/context-graph-core/src/layers/coherence/constants.rs` | MODIFY | Line 16: Change 8→13, Add new constants |
| `crates/context-graph-core/src/layers/coherence/mod.rs` | MODIFY | Add re-exports for new constants |
| `crates/context-graph-core/src/layers/coherence/network.rs` | MODIFY | Import and use KURAMOTO_BASE_FREQUENCIES |

## FILES TO CREATE

NONE - this is a constant fix, not new functionality

---

## CONSTRAINTS (MUST ALL PASS)

| Constraint | Requirement | Verification |
|------------|-------------|--------------|
| C1 | KURAMOTO_N == 13 | `grep "KURAMOTO_N.*= 13"` |
| C2 | KURAMOTO_BASE_FREQUENCIES has 13 elements | Array type is `[f32; KURAMOTO_N]` (compile-time check) |
| C3 | Frequencies match constitution | Compare with docs2/constitution.yaml gwt.kuramoto.frequencies |
| C4 | Documentation maps indices to embedders | Each element has E1-E13 comment |
| C5 | `cargo check -p context-graph-core` passes | Zero compilation errors |
| C6 | Kuramoto tests pass | `cargo test -p context-graph-core kuramoto` |

---

## ANTI-PATTERNS TO AVOID

| ❌ Wrong | ✅ Right | Why |
|----------|----------|-----|
| Add backward compat for n=8 | Remove/replace n=8 entirely | No legacy support per constitution |
| Mock data in tests | Use real Kuramoto dynamics | Tests must verify actual behavior |
| Suppress failing tests | Fix root cause | Hiding bugs is forbidden |
| Hardcode frequencies inline | Use named constant array | AP-12: No magic numbers |

---

## SUCCESS CRITERIA CHECKLIST

After implementation, ALL must be true:

- [ ] `KURAMOTO_N: usize = 13` in constants.rs
- [ ] `KURAMOTO_BASE_FREQUENCIES: [f32; KURAMOTO_N]` with 13 elements exists
- [ ] Each frequency has documentation mapping to E1-E13
- [ ] `network.rs` imports and uses `KURAMOTO_BASE_FREQUENCIES`
- [ ] `mod.rs` re-exports new constants
- [ ] `cargo check -p context-graph-core` passes with zero errors
- [ ] `cargo test -p context-graph-core kuramoto` all tests pass
- [ ] Verification log shows BEFORE=8, AFTER=13
- [ ] No backward compatibility hacks added

---

## DEPENDENCY CHAIN

```
TASK-10 (THIS TASK)
KURAMOTO_N = 13, frequencies defined
    |
    v
TASK-11: KuramotoNetwork uses KURAMOTO_N=13 for 13-embedder model
    |
    v
TASK-12: KuramotoStepper lifecycle wired to MCP
    |
    v
TASK-25, 34, 39, 40: MCP tools expose Kuramoto state
```

---

## RELATED CONTEXT

### Constitution Enforcement Rules

- **GWT-002**: "Kuramoto network = exactly 13 oscillators"
- **AP-25**: "Kuramoto must have exactly 13 oscillators" (forbidden to have != 13)
- **AP-12**: "No magic numbers - use named constants"

### Related Issues

- **ISS-001**: "Kuramoto 8 oscillators (wrong)" - CRITICAL severity

### Current Test Expectations (Already Correct)

```rust
// crates/context-graph-mcp/src/handlers/tests/phase3_gwt_consciousness/kuramoto_sync.rs
assert_eq!(phases.len(), 13, "[FSV] CRITICAL: Must have 13 oscillator phases");
assert_eq!(natural_freqs.len(), 13, "[FSV] Must have 13 natural frequencies");
```

These tests will PASS once KURAMOTO_N=13 is implemented.

---

## NEXT TASK

After TASK-10 is complete:
- **TASK-11**: Implement KuramotoNetwork with 13 frequencies (depends on TASK-10)

---

*Task Specification v3.0 - Audited 2026-01-13 against actual codebase*
*KURAMOTO_N=8 (WRONG) → KURAMOTO_N=13 (CORRECT per constitution)*

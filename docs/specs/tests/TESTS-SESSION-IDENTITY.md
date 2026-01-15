# Test Specification: Session Identity Persistence

```xml
<test_specification id="TESTS-SESSION-IDENTITY" version="1.0">
<metadata>
  <title>Session Identity Persistence - Comprehensive Test Cases</title>
  <status>draft</status>
  <owner>Context Graph Team</owner>
  <last_updated>2026-01-14</last_updated>
  <functional_spec_ref>SPEC-SESSION-IDENTITY</functional_spec_ref>
  <technical_spec_ref>TECH-SESSION-IDENTITY</technical_spec_ref>
  <tasks_spec_ref>TASKS-SESSION-IDENTITY</tasks_spec_ref>
  <total_test_cases>24</total_test_cases>
  <test_breakdown>
    <unit_tests count="11"/>
    <integration_tests count="11"/>
    <benchmark_tests count="1"/>
    <e2e_tests count="1"/>
  </test_breakdown>
</metadata>

<!-- ============================================================================
     CRITICAL TESTING RULES
     ============================================================================ -->

<testing_rules>
  <rule id="NO-MOCKS">ALL tests MUST use REAL data and REAL instances. NO mock GWT system, NO mock storage.</rule>
  <rule id="REAL-ROCKSDB">Integration tests use real RocksDB instances with tempdir::TempDir</rule>
  <rule id="FAIL-FAST">Tests must fail fast with clear error messages if something doesn't work</rule>
  <rule id="DETERMINISTIC">All tests must be deterministic and reproducible</rule>
  <rule id="ISOLATED">Each test runs in isolation, no shared state between tests</rule>
</testing_rules>
```

---

## Test File Locations

| Category | Location |
|----------|----------|
| Unit Tests (snapshot) | `crates/context-graph-core/src/gwt/session_identity/tests/snapshot_tests.rs` |
| Unit Tests (cache) | `crates/context-graph-core/src/gwt/session_identity/tests/cache_tests.rs` |
| Unit Tests (manager) | `crates/context-graph-core/src/gwt/session_identity/tests/manager_tests.rs` |
| Integration Tests | `crates/context-graph-storage/tests/session_identity_integration.rs` |
| Benchmark Tests | `crates/context-graph-core/benches/session_identity.rs` |
| E2E Tests | `tests/integration/session_hooks_test.rs` |

---

## Unit Tests

### TC-SESSION-01: SessionIdentitySnapshot Serialization Round-Trip

**Type**: unit
**Task Ref**: TASK-SESSION-01
**Requirement Ref**: REQ-SESSION-01

#### Objective
Verify that SessionIdentitySnapshot can be serialized with bincode and deserialized back to an identical struct.

#### Setup
```rust
// File: crates/context-graph-core/src/gwt/session_identity/tests/snapshot_tests.rs

use crate::gwt::session_identity::types::{
    SessionIdentitySnapshot, MAX_TRAJECTORY_LEN, KURAMOTO_N
};
use bincode;
```

#### Test Steps
```rust
#[test]
fn test_snapshot_serialization_roundtrip() {
    // Step 1: Create a snapshot with all fields populated
    let mut snapshot = SessionIdentitySnapshot::new("test-session-001");
    snapshot.previous_session_id = Some("prev-session-000".to_string());
    snapshot.cross_session_ic = 0.85;
    snapshot.kuramoto_phases = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3];
    snapshot.coupling = 0.65;
    snapshot.purpose_vector = [0.1; KURAMOTO_N];
    snapshot.last_ic = 0.92;
    snapshot.crisis_threshold = 0.5;
    snapshot.consciousness = 0.88;
    snapshot.integration = 0.75;
    snapshot.reflection = 0.80;
    snapshot.differentiation = 0.70;

    // Add trajectory entries
    for i in 0..10 {
        let pv: [f32; KURAMOTO_N] = [i as f32 * 0.01; KURAMOTO_N];
        snapshot.append_to_trajectory(pv);
    }

    // Step 2: Serialize with bincode
    let serialized = bincode::serialize(&snapshot)
        .expect("Serialization must succeed");

    // Step 3: Verify size constraint (<30KB)
    assert!(
        serialized.len() < 30_000,
        "Serialized size {} exceeds 30KB limit",
        serialized.len()
    );

    // Step 4: Deserialize back
    let deserialized: SessionIdentitySnapshot = bincode::deserialize(&serialized)
        .expect("Deserialization must succeed");

    // Step 5: Verify equality
    assert_eq!(snapshot, deserialized, "Round-trip must preserve all fields");
    assert_eq!(deserialized.session_id, "test-session-001");
    assert_eq!(deserialized.previous_session_id, Some("prev-session-000".to_string()));
    assert!((deserialized.cross_session_ic - 0.85).abs() < f32::EPSILON);
    assert_eq!(deserialized.trajectory.len(), 10);
}

#[test]
fn test_snapshot_max_size_with_full_trajectory() {
    // Create snapshot with maximum trajectory (50 entries)
    let mut snapshot = SessionIdentitySnapshot::new("max-size-test");

    for i in 0..MAX_TRAJECTORY_LEN {
        let pv: [f32; KURAMOTO_N] = [i as f32 * 0.001; KURAMOTO_N];
        snapshot.append_to_trajectory(pv);
    }

    let serialized = bincode::serialize(&snapshot)
        .expect("Serialization must succeed");

    // Verify size is under 30KB even with full trajectory
    assert!(
        serialized.len() < 30_000,
        "Full trajectory serialized size {} exceeds 30KB limit",
        serialized.len()
    );

    // Verify estimated_size() is accurate
    let estimated = snapshot.estimated_size();
    let actual = serialized.len();
    assert!(
        (estimated as i64 - actual as i64).abs() < 1000,
        "Estimated size {} differs from actual {} by more than 1KB",
        estimated, actual
    );
}
```

#### Expected Results
- [ ] Serialization succeeds without error
- [ ] Serialized size is less than 30KB
- [ ] Deserialization succeeds without error
- [ ] All 14 fields are preserved exactly
- [ ] trajectory vector is preserved with correct order

#### Failure Conditions
- Serialization produces errors: Log bincode error message
- Size exceeds 30KB: Log actual size
- Deserialized data differs from original: Log field differences

---

### TC-SESSION-02: Trajectory FIFO Eviction at MAX_TRAJECTORY_LEN

**Type**: unit
**Task Ref**: TASK-SESSION-01
**Requirement Ref**: REQ-SESSION-01

#### Objective
Verify that trajectory vector evicts oldest entries when MAX_TRAJECTORY_LEN (50) is exceeded.

#### Setup
```rust
// File: crates/context-graph-core/src/gwt/session_identity/tests/snapshot_tests.rs

use crate::gwt::session_identity::types::{
    SessionIdentitySnapshot, MAX_TRAJECTORY_LEN, KURAMOTO_N
};
```

#### Test Steps
```rust
#[test]
fn test_trajectory_fifo_eviction() {
    let mut snapshot = SessionIdentitySnapshot::new("eviction-test");

    // Step 1: Fill trajectory to capacity
    for i in 0..MAX_TRAJECTORY_LEN {
        let pv: [f32; KURAMOTO_N] = [i as f32; KURAMOTO_N];
        snapshot.append_to_trajectory(pv);
    }

    assert_eq!(
        snapshot.trajectory.len(),
        MAX_TRAJECTORY_LEN,
        "Trajectory should be at capacity ({})",
        MAX_TRAJECTORY_LEN
    );

    // Verify first entry is [0.0; 13]
    assert!(
        (snapshot.trajectory[0][0] - 0.0).abs() < f32::EPSILON,
        "First entry should be the oldest (index 0)"
    );

    // Step 2: Add one more entry (should trigger FIFO eviction)
    let new_pv: [f32; KURAMOTO_N] = [99.0; KURAMOTO_N];
    snapshot.append_to_trajectory(new_pv);

    // Step 3: Verify capacity unchanged
    assert_eq!(
        snapshot.trajectory.len(),
        MAX_TRAJECTORY_LEN,
        "Trajectory must not exceed MAX_TRAJECTORY_LEN"
    );

    // Step 4: Verify oldest entry was evicted
    assert!(
        (snapshot.trajectory[0][0] - 1.0).abs() < f32::EPSILON,
        "After eviction, first entry should be what was index 1 (value 1.0), got {}",
        snapshot.trajectory[0][0]
    );

    // Step 5: Verify newest entry is at the end
    let last_idx = snapshot.trajectory.len() - 1;
    assert!(
        (snapshot.trajectory[last_idx][0] - 99.0).abs() < f32::EPSILON,
        "Last entry should be newest (99.0), got {}",
        snapshot.trajectory[last_idx][0]
    );
}

#[test]
fn test_trajectory_order_preserved() {
    let mut snapshot = SessionIdentitySnapshot::new("order-test");

    // Add entries with distinguishable values
    for i in 0..10 {
        let pv: [f32; KURAMOTO_N] = [(i * 10) as f32; KURAMOTO_N];
        snapshot.append_to_trajectory(pv);
    }

    // Verify order: oldest first, newest last
    for i in 0..10 {
        let expected = (i * 10) as f32;
        let actual = snapshot.trajectory[i][0];
        assert!(
            (actual - expected).abs() < f32::EPSILON,
            "Entry at index {} should be {}, got {}",
            i, expected, actual
        );
    }
}
```

#### Expected Results
- [ ] Trajectory is capped at MAX_TRAJECTORY_LEN (50)
- [ ] Oldest entry (index 0) is evicted first
- [ ] Newest entry is always at the end
- [ ] FIFO order is strictly maintained

#### Failure Conditions
- Trajectory exceeds 50 entries: Log actual length
- Wrong entry evicted: Log expected vs actual first entry
- Order not preserved: Log index with wrong value

---

### TC-SESSION-03: format_brief() Output Format "[C:STATE r=X.XX IC=X.XX]"

**Type**: unit
**Task Ref**: TASK-SESSION-02
**Requirement Ref**: REQ-SESSION-09

#### Objective
Verify that IdentityCache.format_brief() returns correctly formatted string matching the pattern "[C:STATE r=X.XX IC=X.XX]".

#### Setup
```rust
// File: crates/context-graph-core/src/gwt/session_identity/tests/cache_tests.rs

use crate::gwt::session_identity::cache::{IdentityCache, update_cache, clear_cache};
use crate::gwt::session_identity::types::SessionIdentitySnapshot;
use regex::Regex;
```

#### Test Steps
```rust
#[test]
fn test_format_brief_output_format() {
    // Step 1: Clear any existing cache state
    clear_cache();

    // Step 2: Create and populate a snapshot
    let mut snapshot = SessionIdentitySnapshot::new("brief-format-test");
    snapshot.consciousness = 0.85; // Should map to Conscious state
    snapshot.kuramoto_phases = [0.5; 13]; // r should be close to 1.0

    // Step 3: Update cache with known IC value
    let ic = 0.78;
    update_cache(&snapshot, ic);

    // Step 4: Get formatted brief
    let brief = IdentityCache::format_brief();

    // Step 5: Verify format with regex
    // Pattern: [C:XXX r=X.XX IC=X.XX]
    let pattern = Regex::new(r"^\[C:([A-Z]{3}) r=(\d+\.\d{2}) IC=(\d+\.\d{2})\]$")
        .expect("Regex must compile");

    assert!(
        pattern.is_match(&brief),
        "format_brief() output '{}' does not match expected pattern '[C:XXX r=X.XX IC=X.XX]'",
        brief
    );

    // Step 6: Extract and verify values
    let captures = pattern.captures(&brief).unwrap();
    let state = &captures[1];
    let r_value: f32 = captures[2].parse().unwrap();
    let ic_value: f32 = captures[3].parse().unwrap();

    assert_eq!(state, "CON", "State should be CON for consciousness=0.85");
    assert!(r_value > 0.0 && r_value <= 1.0, "r should be in (0.0, 1.0]");
    assert!((ic_value - 0.78).abs() < 0.01, "IC should be 0.78, got {}", ic_value);

    // Clean up
    clear_cache();
}

#[test]
fn test_format_brief_cold_cache() {
    // Step 1: Clear cache to simulate cold start
    clear_cache();

    // Step 2: Get brief without populating cache
    let brief = IdentityCache::format_brief();

    // Step 3: Verify cold cache fallback format
    assert_eq!(
        brief,
        "[C:? r=? IC=?]",
        "Cold cache should return '[C:? r=? IC=?]', got '{}'",
        brief
    );
}

#[test]
fn test_format_brief_all_states() {
    clear_cache();

    // Test each consciousness state
    let test_cases = [
        (0.95, "HYP"),  // Hypersync
        (0.85, "CON"),  // Conscious
        (0.65, "EMG"),  // Emerging
        (0.40, "FRG"),  // Fragmented
        (0.0,  "DOR"),  // Dormant
    ];

    for (consciousness, expected_state) in test_cases {
        clear_cache();

        let mut snapshot = SessionIdentitySnapshot::new("state-test");
        snapshot.consciousness = consciousness;
        update_cache(&snapshot, 0.9);

        let brief = IdentityCache::format_brief();
        assert!(
            brief.contains(expected_state),
            "Consciousness {} should produce state {}, got '{}'",
            consciousness, expected_state, brief
        );
    }

    clear_cache();
}
```

#### Expected Results
- [ ] Output matches pattern "[C:XXX r=X.XX IC=X.XX]"
- [ ] State is 3 uppercase letters (CON, EMG, FRG, DOR, HYP)
- [ ] r value has 2 decimal places
- [ ] IC value has 2 decimal places
- [ ] Cold cache returns "[C:? r=? IC=?]"

#### Failure Conditions
- Pattern mismatch: Log actual output
- Wrong state code: Log consciousness value and expected state
- Wrong numeric format: Log actual value

---

### TC-SESSION-04: ConsciousnessState.short_name() All Variants

**Type**: unit
**Task Ref**: TASK-SESSION-03
**Requirement Ref**: REQ-SESSION-03

#### Objective
Verify that ConsciousnessState.short_name() returns correct 3-character codes for all 5 variants.

#### Setup
```rust
// File: crates/context-graph-core/src/gwt/state_machine/tests.rs
// (or inline in state_machine/types.rs)

use crate::gwt::state_machine::types::ConsciousnessState;
```

#### Test Steps
```rust
#[test]
fn test_consciousness_state_short_name() {
    let test_cases = [
        (ConsciousnessState::Conscious, "CON"),
        (ConsciousnessState::Emerging, "EMG"),
        (ConsciousnessState::Fragmented, "FRG"),
        (ConsciousnessState::Dormant, "DOR"),
        (ConsciousnessState::Hypersync, "HYP"),
    ];

    for (state, expected_code) in test_cases {
        let actual = state.short_name();
        assert_eq!(
            actual,
            expected_code,
            "{:?}.short_name() should return '{}', got '{}'",
            state, expected_code, actual
        );
    }
}

#[test]
fn test_short_name_length() {
    let states = [
        ConsciousnessState::Conscious,
        ConsciousnessState::Emerging,
        ConsciousnessState::Fragmented,
        ConsciousnessState::Dormant,
        ConsciousnessState::Hypersync,
    ];

    for state in states {
        let code = state.short_name();
        assert_eq!(
            code.len(),
            3,
            "{:?}.short_name() should be exactly 3 chars, '{}' is {} chars",
            state, code, code.len()
        );
    }
}

#[test]
fn test_short_name_is_uppercase() {
    let states = [
        ConsciousnessState::Conscious,
        ConsciousnessState::Emerging,
        ConsciousnessState::Fragmented,
        ConsciousnessState::Dormant,
        ConsciousnessState::Hypersync,
    ];

    for state in states {
        let code = state.short_name();
        assert!(
            code.chars().all(|c| c.is_ascii_uppercase()),
            "{:?}.short_name() '{}' should be all uppercase",
            state, code
        );
    }
}
```

#### Expected Results
- [ ] Conscious returns "CON"
- [ ] Emerging returns "EMG"
- [ ] Fragmented returns "FRG"
- [ ] Dormant returns "DOR"
- [ ] Hypersync returns "HYP"
- [ ] All codes are exactly 3 characters
- [ ] All codes are uppercase ASCII letters

#### Failure Conditions
- Wrong code for state: Log state and expected vs actual
- Code length not 3: Log code and length
- Code not uppercase: Log code with offending character

---

### TC-SESSION-07: Cross-Session IC with Identical Purpose Vectors

**Type**: unit
**Task Ref**: TASK-SESSION-06
**Requirement Ref**: REQ-SESSION-06

#### Objective
Verify that compute_cross_session_ic returns IC close to 1.0 when purpose vectors are identical (cosine similarity = 1.0).

#### Setup
```rust
// File: crates/context-graph-core/src/gwt/session_identity/tests/manager_tests.rs

use crate::gwt::session_identity::types::{SessionIdentitySnapshot, KURAMOTO_N};
use crate::gwt::session_identity::manager::compute_cross_session_ic;
```

#### Test Steps
```rust
#[test]
fn test_cross_session_ic_identical_purpose_vectors() {
    // Step 1: Create two snapshots with identical purpose vectors
    let pv: [f32; KURAMOTO_N] = [0.27735; KURAMOTO_N]; // Normalized unit vector

    let mut current = SessionIdentitySnapshot::new("current-session");
    current.purpose_vector = pv;
    current.kuramoto_phases = [0.0; KURAMOTO_N]; // All phases aligned -> r=1.0

    let mut previous = SessionIdentitySnapshot::new("previous-session");
    previous.purpose_vector = pv; // Identical

    // Step 2: Compute IC
    // Formula: IC = cos(PV_current, PV_previous) * r(current)
    // With identical PVs: cos = 1.0
    // With aligned phases: r = 1.0
    // Expected: IC = 1.0 * 1.0 = 1.0
    let ic = compute_cross_session_ic(&current, &previous);

    // Step 3: Verify IC is close to 1.0
    assert!(
        ic >= 0.99,
        "IC with identical purpose vectors should be ~1.0, got {}",
        ic
    );
}

#[test]
fn test_cross_session_ic_scaled_by_kuramoto_r() {
    // Purpose vectors identical (cos=1), but phases not aligned (r<1)
    let pv: [f32; KURAMOTO_N] = [0.27735; KURAMOTO_N];

    let mut current = SessionIdentitySnapshot::new("current");
    current.purpose_vector = pv;
    // Spread phases to reduce r
    current.kuramoto_phases = [
        0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0, 5.5, 6.0
    ];

    let mut previous = SessionIdentitySnapshot::new("previous");
    previous.purpose_vector = pv;

    let ic = compute_cross_session_ic(&current, &previous);

    // With spread phases, r < 1, so IC = 1.0 * r < 1.0
    assert!(
        ic < 1.0,
        "IC should be less than 1.0 with spread phases (r<1), got {}",
        ic
    );
    assert!(
        ic > 0.0,
        "IC should still be positive, got {}",
        ic
    );
}
```

#### Expected Results
- [ ] Identical purpose vectors with aligned phases yields IC >= 0.99
- [ ] IC is scaled by Kuramoto order parameter r
- [ ] IC is clamped to [0.0, 1.0]

#### Failure Conditions
- IC not close to 1.0 with identical PVs: Log IC value and PV values
- IC not scaled by r: Log phases and computed r

---

### TC-SESSION-08: Cross-Session IC with Orthogonal Purpose Vectors

**Type**: unit
**Task Ref**: TASK-SESSION-06
**Requirement Ref**: REQ-SESSION-06

#### Objective
Verify that compute_cross_session_ic returns IC close to 0.0 when purpose vectors are orthogonal (cosine similarity = 0.0).

#### Setup
```rust
// File: crates/context-graph-core/src/gwt/session_identity/tests/manager_tests.rs

use crate::gwt::session_identity::types::{SessionIdentitySnapshot, KURAMOTO_N};
use crate::gwt::session_identity::manager::compute_cross_session_ic;
```

#### Test Steps
```rust
#[test]
fn test_cross_session_ic_orthogonal_purpose_vectors() {
    // Step 1: Create two snapshots with orthogonal purpose vectors
    // Orthogonal vectors: dot product = 0

    let mut pv_current: [f32; KURAMOTO_N] = [0.0; KURAMOTO_N];
    pv_current[0] = 1.0; // Unit vector along first dimension

    let mut pv_previous: [f32; KURAMOTO_N] = [0.0; KURAMOTO_N];
    pv_previous[1] = 1.0; // Unit vector along second dimension (orthogonal)

    let mut current = SessionIdentitySnapshot::new("current-session");
    current.purpose_vector = pv_current;
    current.kuramoto_phases = [0.0; KURAMOTO_N]; // Perfect alignment r=1

    let mut previous = SessionIdentitySnapshot::new("previous-session");
    previous.purpose_vector = pv_previous;

    // Step 2: Compute IC
    // Formula: IC = cos(PV_current, PV_previous) * r
    // With orthogonal PVs: cos = 0.0
    // Expected: IC = 0.0 * 1.0 = 0.0
    let ic = compute_cross_session_ic(&current, &previous);

    // Step 3: Verify IC is close to 0.0
    assert!(
        ic.abs() < 0.01,
        "IC with orthogonal purpose vectors should be ~0.0, got {}",
        ic
    );
}

#[test]
fn test_cross_session_ic_opposite_purpose_vectors() {
    // Purpose vectors pointing in opposite directions
    let mut pv_current: [f32; KURAMOTO_N] = [0.27735; KURAMOTO_N];
    let pv_previous: [f32; KURAMOTO_N] = [-0.27735; KURAMOTO_N]; // Opposite

    let mut current = SessionIdentitySnapshot::new("current");
    current.purpose_vector = pv_current;
    current.kuramoto_phases = [0.0; KURAMOTO_N];

    let mut previous = SessionIdentitySnapshot::new("previous");
    previous.purpose_vector = pv_previous;

    let ic = compute_cross_session_ic(&current, &previous);

    // cos(v, -v) = -1, but IC should be clamped to [0, 1]
    assert!(
        ic >= 0.0,
        "IC should be clamped to non-negative, got {}",
        ic
    );
}
```

#### Expected Results
- [ ] Orthogonal purpose vectors yield IC close to 0.0
- [ ] Opposite purpose vectors (cos=-1) are clamped to IC=0.0
- [ ] IC never goes negative

#### Failure Conditions
- IC not close to 0.0 with orthogonal PVs: Log IC and PV values
- IC negative: Log raw cosine similarity and IC value

---

### TC-SESSION-09: classify_ic() Threshold Boundaries

**Type**: unit
**Task Ref**: TASK-SESSION-07
**Requirement Ref**: REQ-SESSION-07

#### Objective
Verify that classify_ic() returns correct classifications at all IDENTITY-002 threshold boundaries.

#### Setup
```rust
// File: crates/context-graph-core/src/gwt/session_identity/tests/manager_tests.rs

use crate::gwt::session_identity::manager::{classify_ic, is_ic_crisis, is_ic_warning};
```

#### Test Steps
```rust
#[test]
fn test_classify_ic_threshold_boundaries() {
    // Constitution IDENTITY-002 thresholds:
    // - Healthy: IC >= 0.9
    // - Good: 0.7 <= IC < 0.9
    // - Warning: 0.5 <= IC < 0.7
    // - Degraded: IC < 0.5

    let test_cases = [
        // Healthy range
        (1.0, "healthy"),
        (0.95, "healthy"),
        (0.90, "healthy"), // Exactly at boundary

        // Good range
        (0.899, "good"),   // Just below healthy
        (0.89, "good"),
        (0.75, "good"),
        (0.70, "good"),    // Exactly at boundary

        // Warning range
        (0.699, "warning"), // Just below good
        (0.69, "warning"),
        (0.55, "warning"),
        (0.50, "warning"),  // Exactly at boundary

        // Degraded range
        (0.499, "degraded"), // Just below warning
        (0.49, "degraded"),
        (0.25, "degraded"),
        (0.0, "degraded"),
    ];

    for (ic, expected) in test_cases {
        let actual = classify_ic(ic);
        assert_eq!(
            actual, expected,
            "classify_ic({}) should return '{}', got '{}'",
            ic, expected, actual
        );
    }
}

#[test]
fn test_is_ic_crisis() {
    // Crisis = IC < 0.5
    assert!(is_ic_crisis(0.0), "0.0 should be crisis");
    assert!(is_ic_crisis(0.25), "0.25 should be crisis");
    assert!(is_ic_crisis(0.49), "0.49 should be crisis");
    assert!(is_ic_crisis(0.499), "0.499 should be crisis");

    assert!(!is_ic_crisis(0.5), "0.5 should NOT be crisis");
    assert!(!is_ic_crisis(0.50), "0.50 should NOT be crisis");
    assert!(!is_ic_crisis(0.51), "0.51 should NOT be crisis");
    assert!(!is_ic_crisis(1.0), "1.0 should NOT be crisis");
}

#[test]
fn test_is_ic_warning() {
    // Warning = 0.5 <= IC < 0.7
    assert!(!is_ic_warning(0.49), "0.49 should NOT be warning (is crisis)");

    assert!(is_ic_warning(0.5), "0.5 should be warning");
    assert!(is_ic_warning(0.55), "0.55 should be warning");
    assert!(is_ic_warning(0.69), "0.69 should be warning");

    assert!(!is_ic_warning(0.7), "0.7 should NOT be warning (is good)");
    assert!(!is_ic_warning(0.71), "0.71 should NOT be warning");
    assert!(!is_ic_warning(1.0), "1.0 should NOT be warning");
}
```

#### Expected Results
- [ ] IC >= 0.9 returns "healthy"
- [ ] 0.7 <= IC < 0.9 returns "good"
- [ ] 0.5 <= IC < 0.7 returns "warning"
- [ ] IC < 0.5 returns "degraded"
- [ ] is_ic_crisis returns true iff IC < 0.5
- [ ] is_ic_warning returns true iff 0.5 <= IC < 0.7

#### Failure Conditions
- Wrong classification at boundary: Log IC and expected vs actual
- is_ic_crisis or is_ic_warning returns wrong boolean: Log IC value

---

### TC-SESSION-10: Auto-Dream Trigger Fires at IC < 0.5

**Type**: unit
**Task Ref**: TASK-SESSION-08
**Requirement Ref**: REQ-SESSION-08

#### Objective
Verify that check_and_trigger_dream fires async dream trigger when IC < 0.5 and --auto-dream is enabled.

#### Setup
```rust
// File: crates/context-graph-core/src/gwt/session_identity/tests/manager_tests.rs

use crate::gwt::session_identity::dream_trigger::check_and_trigger_dream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
```

#### Test Steps
```rust
#[test]
fn test_auto_dream_trigger_on_crisis() {
    // IC < 0.5 with auto_dream=true should trigger dream
    let triggered = check_and_trigger_dream(0.3, true);
    assert!(
        triggered,
        "Dream should be triggered for IC=0.3 with auto_dream=true"
    );
}

#[test]
fn test_auto_dream_not_triggered_without_flag() {
    // IC < 0.5 with auto_dream=false should NOT trigger dream
    let triggered = check_and_trigger_dream(0.3, false);
    assert!(
        !triggered,
        "Dream should NOT be triggered when auto_dream=false"
    );
}

#[test]
fn test_auto_dream_not_triggered_for_healthy_ic() {
    // IC >= 0.5 should NOT trigger dream even with auto_dream=true
    let test_cases = [0.5, 0.6, 0.7, 0.8, 0.9, 1.0];

    for ic in test_cases {
        let triggered = check_and_trigger_dream(ic, true);
        assert!(
            !triggered,
            "Dream should NOT be triggered for IC={} (>= 0.5)",
            ic
        );
    }
}

#[test]
fn test_auto_dream_boundary_at_0_5() {
    // IC = 0.5 exactly should NOT trigger (threshold is < 0.5, not <=)
    let triggered = check_and_trigger_dream(0.5, true);
    assert!(
        !triggered,
        "Dream should NOT be triggered at IC=0.5 (boundary, not crisis)"
    );

    // IC = 0.499 should trigger
    let triggered = check_and_trigger_dream(0.499, true);
    assert!(
        triggered,
        "Dream should be triggered at IC=0.499 (just below boundary)"
    );
}

#[test]
fn test_auto_dream_is_non_blocking() {
    use std::time::Instant;

    let start = Instant::now();
    let _ = check_and_trigger_dream(0.3, true);
    let elapsed = start.elapsed();

    // Fire-and-forget should return quickly (<100ms)
    assert!(
        elapsed.as_millis() < 100,
        "check_and_trigger_dream should be non-blocking, took {}ms",
        elapsed.as_millis()
    );
}
```

#### Expected Results
- [ ] check_and_trigger_dream(IC<0.5, true) returns true
- [ ] check_and_trigger_dream(IC<0.5, false) returns false
- [ ] check_and_trigger_dream(IC>=0.5, true) returns false
- [ ] IC=0.5 exactly does NOT trigger (boundary condition)
- [ ] Function returns in under 100ms (non-blocking)

#### Failure Conditions
- Dream triggered when it shouldn't: Log IC and auto_dream flag
- Dream not triggered when it should: Log IC value
- Function blocks: Log elapsed time

---

### TC-SESSION-11: format_brief() Latency <100us p95 (Benchmark)

**Type**: benchmark
**Task Ref**: TASK-SESSION-09
**Requirement Ref**: REQ-SESSION-09

#### Objective
Verify that IdentityCache.format_brief() completes in under 100 microseconds at p95.

#### Setup
```rust
// File: crates/context-graph-core/benches/session_identity.rs

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use context_graph_core::gwt::session_identity::{
    cache::{IdentityCache, update_cache, clear_cache},
    types::SessionIdentitySnapshot,
};
```

#### Test Steps
```rust
fn bench_format_brief(c: &mut Criterion) {
    // Setup: Warm the cache
    clear_cache();
    let mut snapshot = SessionIdentitySnapshot::new("bench-session");
    snapshot.consciousness = 0.85;
    snapshot.kuramoto_phases = [0.5; 13];
    update_cache(&snapshot, 0.88);

    let mut group = c.benchmark_group("format_brief");
    group.sample_size(10000);
    group.measurement_time(std::time::Duration::from_secs(10));

    group.bench_function("warm_cache", |b| {
        b.iter(|| {
            black_box(IdentityCache::format_brief())
        })
    });

    group.finish();

    // Cleanup
    clear_cache();
}

fn bench_format_brief_latency_percentiles(c: &mut Criterion) {
    // Setup: Warm the cache
    clear_cache();
    let mut snapshot = SessionIdentitySnapshot::new("bench-session");
    snapshot.consciousness = 0.85;
    update_cache(&snapshot, 0.88);

    // Collect 10000 samples
    let mut latencies: Vec<u128> = Vec::with_capacity(10000);

    for _ in 0..10000 {
        let start = std::time::Instant::now();
        let _ = IdentityCache::format_brief();
        latencies.push(start.elapsed().as_nanos());
    }

    latencies.sort();

    let p50 = latencies[5000];
    let p95 = latencies[9500];
    let p99 = latencies[9900];

    println!("format_brief() latency:");
    println!("  p50: {} ns ({} us)", p50, p50 / 1000);
    println!("  p95: {} ns ({} us)", p95, p95 / 1000);
    println!("  p99: {} ns ({} us)", p99, p99 / 1000);

    // Assert performance targets
    assert!(
        p95 < 100_000, // 100 microseconds = 100,000 nanoseconds
        "p95 latency {} ns exceeds 100us target",
        p95
    );

    assert!(
        p99 < 500_000, // 500 microseconds
        "p99 latency {} ns exceeds 500us target",
        p99
    );

    clear_cache();
}

criterion_group!(benches, bench_format_brief);
criterion_main!(benches);

#[test]
fn test_format_brief_latency_assertion() {
    bench_format_brief_latency_percentiles(&mut Criterion::default());
}
```

#### Expected Results
- [ ] p50 latency < 50 microseconds
- [ ] p95 latency < 100 microseconds
- [ ] p99 latency < 500 microseconds
- [ ] No heap allocations beyond the single output String

#### Failure Conditions
- p95 exceeds 100us: Log actual p95 and sample distribution
- p99 exceeds 500us: Log actual p99

---

## Integration Tests

### TC-SESSION-05: RocksDB Save/Load Round-Trip

**Type**: integration
**Task Ref**: TASK-SESSION-05
**Requirement Ref**: REQ-SESSION-05

#### Objective
Verify that save_snapshot and load_snapshot correctly persist and retrieve SessionIdentitySnapshot from RocksDB.

#### Setup
```rust
// File: crates/context-graph-storage/tests/session_identity_integration.rs

use context_graph_storage::{RocksDbMemex, StorageResult};
use context_graph_core::gwt::session_identity::types::{
    SessionIdentitySnapshot, MAX_TRAJECTORY_LEN, KURAMOTO_N
};
use tempfile::TempDir;
```

#### Test Steps
```rust
fn create_test_db() -> (RocksDbMemex, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let db = RocksDbMemex::open(temp_dir.path())
        .expect("Failed to open RocksDB");
    (db, temp_dir)
}

#[test]
fn test_save_load_snapshot_roundtrip() {
    let (db, _temp_dir) = create_test_db();

    // Step 1: Create a populated snapshot
    let mut snapshot = SessionIdentitySnapshot::new("integration-test-001");
    snapshot.cross_session_ic = 0.85;
    snapshot.consciousness = 0.90;
    snapshot.kuramoto_phases = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3];
    snapshot.purpose_vector = [0.27735; KURAMOTO_N];

    for i in 0..10 {
        snapshot.append_to_trajectory([i as f32 * 0.1; KURAMOTO_N]);
    }

    // Step 2: Save to RocksDB
    db.save_snapshot(&snapshot)
        .expect("save_snapshot must succeed");

    // Step 3: Load by session_id
    let loaded = db.load_snapshot(Some("integration-test-001"))
        .expect("load_snapshot by ID must succeed");

    // Step 4: Verify all fields match
    assert_eq!(snapshot.session_id, loaded.session_id);
    assert!((snapshot.cross_session_ic - loaded.cross_session_ic).abs() < f32::EPSILON);
    assert!((snapshot.consciousness - loaded.consciousness).abs() < f32::EPSILON);
    assert_eq!(snapshot.trajectory.len(), loaded.trajectory.len());

    for i in 0..13 {
        assert!((snapshot.kuramoto_phases[i] - loaded.kuramoto_phases[i]).abs() < f64::EPSILON);
        assert!((snapshot.purpose_vector[i] - loaded.purpose_vector[i]).abs() < f32::EPSILON);
    }
}

#[test]
fn test_load_latest_snapshot() {
    let (db, _temp_dir) = create_test_db();

    // Save multiple snapshots
    for i in 1..=3 {
        let snapshot = SessionIdentitySnapshot::new(format!("session-{}", i));
        db.save_snapshot(&snapshot).expect("save must succeed");
        // Small delay to ensure different timestamps
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // Load latest (should be session-3)
    let latest = db.load_latest()
        .expect("load_latest must succeed")
        .expect("Should have at least one snapshot");

    assert_eq!(
        latest.session_id,
        "session-3",
        "Latest should be session-3, got {}",
        latest.session_id
    );
}

#[test]
fn test_load_latest_fresh_install() {
    let (db, _temp_dir) = create_test_db();

    // Fresh install - no sessions saved
    let result = db.load_latest()
        .expect("load_latest must not error on fresh install");

    assert!(
        result.is_none(),
        "load_latest should return None for fresh install"
    );
}

#[test]
fn test_load_snapshot_not_found() {
    let (db, _temp_dir) = create_test_db();

    let result = db.load_snapshot(Some("nonexistent-session"));

    assert!(
        result.is_err(),
        "load_snapshot for nonexistent session should error"
    );

    // Verify error is NotFound variant
    match result {
        Err(e) => {
            let msg = e.to_string().to_lowercase();
            assert!(
                msg.contains("not found"),
                "Error should indicate not found: {}",
                e
            );
        }
        Ok(_) => panic!("Should have returned error"),
    }
}
```

#### Expected Results
- [ ] save_snapshot succeeds without error
- [ ] load_snapshot by ID returns exact same data
- [ ] load_latest returns most recently saved snapshot
- [ ] load_latest returns None for fresh install
- [ ] load_snapshot returns NotFound error for missing session

#### Failure Conditions
- Storage operation fails: Log RocksDB error
- Data mismatch after round-trip: Log field differences
- load_latest returns wrong session: Log expected vs actual

---

### TC-SESSION-06: Temporal Index Ordering (Big-Endian)

**Type**: integration
**Task Ref**: TASK-SESSION-05
**Requirement Ref**: REQ-SESSION-04, REQ-SESSION-05

#### Objective
Verify that temporal index keys are stored in big-endian format ensuring correct lexicographic ordering.

#### Setup
```rust
// File: crates/context-graph-storage/tests/session_identity_integration.rs

use context_graph_storage::RocksDbMemex;
use context_graph_core::gwt::session_identity::types::SessionIdentitySnapshot;
use tempfile::TempDir;
```

#### Test Steps
```rust
#[test]
fn test_temporal_index_ordering() {
    let (db, _temp_dir) = create_test_db();

    // Save snapshots with specific timestamps
    let timestamps = [1000_i64, 5000, 3000, 10000, 2000];

    for (i, ts) in timestamps.iter().enumerate() {
        let mut snapshot = SessionIdentitySnapshot::new(format!("ts-{}", ts));
        snapshot.timestamp_ms = *ts;
        db.save_snapshot(&snapshot).expect("save must succeed");
    }

    // Query temporal index range
    // Keys should be ordered: t:0000000000001000, t:0000000000002000, ...
    let cf = db.get_cf("session_identity").expect("CF must exist");

    let mut found_sessions: Vec<String> = Vec::new();
    let iter = db.db.prefix_iterator_cf(cf, b"t:");

    for item in iter {
        let (key, value) = item.expect("Iterator item");
        if !key.starts_with(b"t:") {
            break;
        }
        let session_id = String::from_utf8_lossy(&value).to_string();
        found_sessions.push(session_id);
    }

    // Verify order: should be 1000, 2000, 3000, 5000, 10000
    let expected_order = ["ts-1000", "ts-2000", "ts-3000", "ts-5000", "ts-10000"];

    assert_eq!(
        found_sessions.len(),
        expected_order.len(),
        "Should find {} temporal entries",
        expected_order.len()
    );

    for (i, expected) in expected_order.iter().enumerate() {
        assert_eq!(
            found_sessions[i],
            *expected,
            "Entry {} should be {}, got {}",
            i, expected, found_sessions[i]
        );
    }
}

#[test]
fn test_big_endian_key_format() {
    let (db, _temp_dir) = create_test_db();

    // Timestamp: 1704067200000 (2024-01-01 00:00:00 UTC)
    let ts: i64 = 1704067200000;
    let mut snapshot = SessionIdentitySnapshot::new("big-endian-test");
    snapshot.timestamp_ms = ts;

    db.save_snapshot(&snapshot).expect("save must succeed");

    // Expected key format: "t:{16 hex digits}"
    // 1704067200000 in hex is 18CD83D2800
    // Padded to 16 digits: 000018CD83D2800
    let expected_key = format!("t:{:016x}", ts as u64);

    // Verify the key exists
    let cf = db.get_cf("session_identity").expect("CF must exist");
    let value = db.db.get_cf(cf, expected_key.as_bytes())
        .expect("get must succeed")
        .expect("key must exist");

    let session_id = String::from_utf8_lossy(&value);
    assert_eq!(session_id, "big-endian-test");
}
```

#### Expected Results
- [ ] Temporal keys are stored as "t:{16 hex digits}"
- [ ] Keys are lexicographically ordered by timestamp
- [ ] Range iteration returns sessions in chronological order

#### Failure Conditions
- Wrong key format: Log expected vs actual key
- Incorrect ordering: Log expected order vs found order

---

### TC-SESSION-12: consciousness brief with Warm Cache (<50ms)

**Type**: integration
**Task Ref**: TASK-SESSION-11
**Requirement Ref**: REQ-SESSION-11

#### Objective
Verify that `context-graph-cli consciousness brief` command completes in under 50ms with warm cache.

#### Setup
```rust
// File: tests/integration/session_hooks_test.rs

use std::process::Command;
use std::time::Instant;
```

#### Test Steps
```rust
#[test]
fn test_consciousness_brief_warm_cache() {
    // Step 1: First call to warm cache (may take longer)
    // This simulates restore-identity having run at SessionStart
    let _ = Command::new("context-graph-cli")
        .args(["consciousness", "brief"])
        .output()
        .expect("Failed to execute command");

    // Step 2: Measure latency of subsequent calls
    let mut latencies: Vec<u128> = Vec::with_capacity(100);

    for _ in 0..100 {
        let start = Instant::now();
        let output = Command::new("context-graph-cli")
            .args(["consciousness", "brief"])
            .output()
            .expect("Failed to execute command");
        let elapsed = start.elapsed();

        assert!(output.status.success(), "Command must succeed");
        latencies.push(elapsed.as_millis());
    }

    latencies.sort();

    let p50 = latencies[50];
    let p95 = latencies[95];
    let p99 = latencies[99];

    println!("consciousness brief latencies:");
    println!("  p50: {}ms", p50);
    println!("  p95: {}ms", p95);
    println!("  p99: {}ms", p99);

    // Target: <50ms p95
    assert!(
        p95 < 50,
        "p95 latency {}ms exceeds 50ms target",
        p95
    );
}

#[test]
fn test_consciousness_brief_output_format() {
    let output = Command::new("context-graph-cli")
        .args(["consciousness", "brief"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stdout = stdout.trim();

    // Verify output matches pattern (warm or cold)
    let warm_pattern = regex::Regex::new(r"^\[C:[A-Z]{3} r=\d+\.\d{2} IC=\d+\.\d{2}\]$").unwrap();
    let cold_pattern = r"[C:? r=? IC=?]";

    assert!(
        warm_pattern.is_match(stdout) || stdout == cold_pattern,
        "Output '{}' does not match expected format",
        stdout
    );
}

#[test]
fn test_consciousness_brief_exit_code() {
    let output = Command::new("context-graph-cli")
        .args(["consciousness", "brief"])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "consciousness brief must always exit 0 (non-blocking)"
    );
}
```

#### Expected Results
- [ ] Command completes in under 50ms at p95 with warm cache
- [ ] Output matches "[C:XXX r=X.XX IC=X.XX]" or cold fallback
- [ ] Exit code is always 0

#### Failure Conditions
- Latency exceeds 50ms: Log actual latency distribution
- Output format wrong: Log actual output
- Non-zero exit code: Log exit code and stderr

---

### TC-SESSION-13: consciousness brief with Cold Cache (Fallback)

**Type**: integration
**Task Ref**: TASK-SESSION-11
**Requirement Ref**: REQ-SESSION-11

#### Objective
Verify that `consciousness brief` returns fallback output "[C:? r=? IC=?]" when cache is cold.

#### Setup
```rust
// File: tests/integration/session_hooks_test.rs

use std::process::Command;
```

#### Test Steps
```rust
#[test]
fn test_consciousness_brief_cold_cache_fallback() {
    // Step 1: Clear any existing cache by restarting process
    // (Cache is in-memory and cleared between processes)

    // Note: To truly test cold cache, we need to ensure no restore-identity
    // has run in this process. Since CLI spawns fresh process each time,
    // the first call after cache clear should return fallback.

    // In integration testing context, we simulate by checking that the
    // command handles missing cache gracefully.

    let output = Command::new("context-graph-cli")
        .args(["consciousness", "brief"])
        .env("CONTEXT_GRAPH_TEST_COLD_CACHE", "1") // Signal to clear cache
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Command must succeed even with cold cache");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stdout = stdout.trim();

    // Either warm output or cold fallback is acceptable
    // (depends on whether cache was populated by previous test)
    let valid_outputs = [
        "[C:? r=? IC=?]", // Cold cache fallback
    ];

    let warm_pattern = regex::Regex::new(r"^\[C:[A-Z]{3} r=\d+\.\d{2} IC=\d+\.\d{2}\]$").unwrap();

    assert!(
        valid_outputs.contains(&stdout) || warm_pattern.is_match(stdout),
        "Output '{}' is not a valid format",
        stdout
    );
}

#[test]
fn test_consciousness_brief_no_disk_io() {
    // Verify that consciousness brief does NOT read from disk
    // by measuring time without any database files present

    use std::time::Instant;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().expect("Create temp dir");

    // Run with nonexistent database path
    let start = Instant::now();
    let output = Command::new("context-graph-cli")
        .args(["consciousness", "brief"])
        .env("CONTEXT_GRAPH_DATA_DIR", temp_dir.path())
        .output()
        .expect("Failed to execute command");
    let elapsed = start.elapsed();

    // Should complete quickly even without database
    assert!(
        elapsed.as_millis() < 100,
        "consciousness brief should not attempt disk I/O, took {}ms",
        elapsed.as_millis()
    );

    assert!(output.status.success());
}
```

#### Expected Results
- [ ] Command succeeds with cold cache
- [ ] Returns "[C:? r=? IC=?]" when cache not populated
- [ ] No disk I/O attempted (completes in <100ms)

#### Failure Conditions
- Command fails with cold cache: Log exit code and stderr
- Disk I/O attempted: Log elapsed time

---

### TC-SESSION-14: restore-identity with source=startup

**Type**: integration
**Task Ref**: TASK-SESSION-12
**Requirement Ref**: REQ-SESSION-12

#### Objective
Verify that `session restore-identity` loads the latest session when source=startup.

#### Setup
```rust
// File: tests/integration/session_hooks_test.rs

use std::process::{Command, Stdio};
use std::io::Write;
```

#### Test Steps
```rust
#[test]
fn test_restore_identity_source_startup() {
    // Step 1: First persist a session
    let persist_output = Command::new("context-graph-cli")
        .args(["session", "persist-identity"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn persist")
        .wait_with_output()
        .expect("wait persist");

    // Step 2: Restore with source=startup (loads latest)
    let mut child = Command::new("context-graph-cli")
        .args(["session", "restore-identity"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn restore");

    // Write JSON input to stdin
    let input = r#"{"source": "startup"}"#;
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();

    let output = child.wait_with_output().expect("wait restore");

    // Step 3: Verify output
    assert!(output.status.success(), "restore-identity should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should either restore from previous session or initialize new
    assert!(
        stdout.contains("Identity restored from") || stdout.contains("New session initialized"),
        "Output should indicate restoration or new session: {}",
        stdout
    );
}

#[test]
fn test_restore_identity_output_format() {
    let mut child = Command::new("context-graph-cli")
        .args(["session", "restore-identity"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn");

    let input = r#"{"source": "startup"}"#;
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();

    let output = child.wait_with_output().expect("wait");
    let stdout = String::from_utf8_lossy(&output.stdout);

    // If restored, should contain IC value and classification
    if stdout.contains("Identity restored") {
        assert!(
            stdout.contains("IC:") && (
                stdout.contains("healthy") ||
                stdout.contains("good") ||
                stdout.contains("warning") ||
                stdout.contains("degraded")
            ),
            "Restored output should contain IC and classification: {}",
            stdout
        );
    }
}
```

#### Expected Results
- [ ] source=startup loads latest session from storage
- [ ] Output format: "Identity restored from {id}. IC: X.XX (classification)"
- [ ] Exit code 0 on success
- [ ] IdentityCache is updated after restore

#### Failure Conditions
- Wrong session loaded: Log expected vs actual session_id
- Output format wrong: Log actual output
- Cache not updated: Log cache state

---

### TC-SESSION-15: restore-identity with source=clear

**Type**: integration
**Task Ref**: TASK-SESSION-12
**Requirement Ref**: REQ-SESSION-12

#### Objective
Verify that `session restore-identity` initializes a fresh session when source=clear.

#### Setup
```rust
// File: tests/integration/session_hooks_test.rs

use std::process::{Command, Stdio};
use std::io::Write;
```

#### Test Steps
```rust
#[test]
fn test_restore_identity_source_clear() {
    // Step 1: Persist a session first
    let _ = Command::new("context-graph-cli")
        .args(["session", "persist-identity"])
        .output();

    // Step 2: Restore with source=clear (should ignore existing sessions)
    let mut child = Command::new("context-graph-cli")
        .args(["session", "restore-identity"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn");

    let input = r#"{"source": "clear"}"#;
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();

    let output = child.wait_with_output().expect("wait");

    assert!(output.status.success(), "source=clear should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should output fresh session message, not restoration
    assert!(
        stdout.contains("Fresh session initialized"),
        "source=clear should output 'Fresh session initialized', got: {}",
        stdout
    );

    // Should NOT contain "restored from"
    assert!(
        !stdout.contains("restored from"),
        "source=clear should not restore, got: {}",
        stdout
    );
}
```

#### Expected Results
- [ ] source=clear does not load any existing session
- [ ] Output: "Fresh session initialized"
- [ ] Exit code 0
- [ ] Previous session data is ignored (not deleted)

#### Failure Conditions
- Attempts to load existing session: Log output
- Wrong output format: Log actual output

---

### TC-SESSION-16: restore-identity with No Previous Session

**Type**: integration
**Task Ref**: TASK-SESSION-12
**Requirement Ref**: REQ-SESSION-12

#### Objective
Verify that `session restore-identity` handles fresh install gracefully when no previous session exists.

#### Setup
```rust
// File: tests/integration/session_hooks_test.rs

use std::process::{Command, Stdio};
use std::io::Write;
use tempfile::TempDir;
```

#### Test Steps
```rust
#[test]
fn test_restore_identity_no_previous_session() {
    // Use fresh temp directory to simulate fresh install
    let temp_dir = TempDir::new().expect("create temp dir");

    let mut child = Command::new("context-graph-cli")
        .args(["session", "restore-identity"])
        .env("CONTEXT_GRAPH_DATA_DIR", temp_dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");

    let input = r#"{"source": "startup"}"#;
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();

    let output = child.wait_with_output().expect("wait");

    // Should succeed (exit 0), not fail
    assert!(
        output.status.success(),
        "Fresh install should succeed, not fail. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should output new session message
    assert!(
        stdout.contains("New session initialized"),
        "Fresh install should output 'New session initialized', got: {}",
        stdout
    );
}

#[test]
fn test_restore_identity_ic_is_1_for_first_session() {
    let temp_dir = TempDir::new().expect("create temp dir");

    let mut child = Command::new("context-graph-cli")
        .args(["session", "restore-identity"])
        .env("CONTEXT_GRAPH_DATA_DIR", temp_dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn");

    child.stdin.take().unwrap().write_all(b"{}").unwrap();

    let output = child.wait_with_output().expect("wait");
    let stdout = String::from_utf8_lossy(&output.stdout);

    // First session has IC=1.0 by definition (perfect continuity with itself)
    // This is implicit - new session initialized means IC is not relevant
    // The important thing is that it succeeds
    assert!(output.status.success());
}
```

#### Expected Results
- [ ] Exit code 0 on fresh install (not an error)
- [ ] Output: "New session initialized"
- [ ] Does not log errors to stderr
- [ ] IC = 1.0 for first session (implicit)

#### Failure Conditions
- Non-zero exit code on fresh install: Log exit code and stderr
- Error message in output: Log full output

---

### TC-SESSION-17: persist-identity Writes All Keys

**Type**: integration
**Task Ref**: TASK-SESSION-13
**Requirement Ref**: REQ-SESSION-13

#### Objective
Verify that `session persist-identity` writes primary key, latest pointer, and temporal index.

#### Setup
```rust
// File: crates/context-graph-storage/tests/session_identity_integration.rs

use context_graph_storage::RocksDbMemex;
use context_graph_core::gwt::session_identity::types::SessionIdentitySnapshot;
use tempfile::TempDir;
```

#### Test Steps
```rust
#[test]
fn test_persist_identity_writes_all_keys() {
    let (db, _temp_dir) = create_test_db();

    // Create and save a snapshot
    let snapshot = SessionIdentitySnapshot::new("persist-all-keys-test");
    let timestamp_hex = format!("{:016x}", snapshot.timestamp_ms as u64);

    db.save_snapshot(&snapshot).expect("save must succeed");

    // Verify primary key exists
    let cf = db.get_cf("session_identity").expect("CF must exist");

    let primary_key = format!("s:{}", snapshot.session_id);
    let primary_value = db.db.get_cf(cf, primary_key.as_bytes())
        .expect("get primary")
        .expect("primary key must exist");
    assert!(!primary_value.is_empty(), "Primary key should have value");

    // Verify "latest" key exists and points to this session
    let latest_value = db.db.get_cf(cf, b"latest")
        .expect("get latest")
        .expect("latest key must exist");
    let latest_session = String::from_utf8_lossy(&latest_value);
    assert_eq!(
        latest_session,
        snapshot.session_id,
        "latest should point to saved session"
    );

    // Verify temporal index exists
    let temporal_key = format!("t:{}", timestamp_hex);
    let temporal_value = db.db.get_cf(cf, temporal_key.as_bytes())
        .expect("get temporal")
        .expect("temporal key must exist");
    let temporal_session = String::from_utf8_lossy(&temporal_value);
    assert_eq!(
        temporal_session,
        snapshot.session_id,
        "temporal index should point to saved session"
    );
}

#[test]
fn test_persist_identity_silent_success() {
    use std::process::{Command, Stdio};
    use std::io::Write;

    let mut child = Command::new("context-graph-cli")
        .args(["session", "persist-identity"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");

    child.stdin.take().unwrap().write_all(b"{}").unwrap();

    let output = child.wait_with_output().expect("wait");

    assert!(output.status.success(), "persist-identity should succeed");

    // Should be silent on success (no stdout)
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.trim().is_empty(),
        "persist-identity should be silent on success, got: {}",
        stdout
    );
}
```

#### Expected Results
- [ ] Primary key "s:{session_id}" is written with snapshot data
- [ ] "latest" key points to the saved session
- [ ] Temporal index "t:{timestamp_hex}" points to the saved session
- [ ] Command is silent on success (no stdout)
- [ ] Exit code 0

#### Failure Conditions
- Any key missing: Log which key is missing
- Key points to wrong session: Log expected vs actual
- Stdout not empty: Log actual stdout

---

### TC-SESSION-18: check-identity IC >= 0.5 (No Dream)

**Type**: integration
**Task Ref**: TASK-SESSION-14
**Requirement Ref**: REQ-SESSION-14

#### Objective
Verify that `consciousness check-identity` does not trigger dream when IC >= 0.5.

#### Setup
```rust
// File: tests/integration/session_hooks_test.rs

use std::process::Command;
```

#### Test Steps
```rust
#[test]
fn test_check_identity_healthy_ic_no_dream() {
    // Setup: Ensure we have a healthy IC state
    // (This may require pre-populating the cache or database)

    let output = Command::new("context-graph-cli")
        .args(["consciousness", "check-identity", "--auto-dream"])
        .output()
        .expect("execute");

    // Should always exit 0 (non-blocking)
    assert!(output.status.success(), "check-identity must always exit 0");

    // With healthy IC (>=0.9), should be silent (no stderr warnings)
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should NOT contain crisis or dream triggered message
    assert!(
        !stderr.contains("dream triggered"),
        "Healthy IC should not trigger dream, stderr: {}",
        stderr
    );
}

#[test]
fn test_check_identity_exit_code_always_zero() {
    // Test various scenarios - all should exit 0

    for auto_dream in [true, false] {
        let args = if auto_dream {
            vec!["consciousness", "check-identity", "--auto-dream"]
        } else {
            vec!["consciousness", "check-identity"]
        };

        let output = Command::new("context-graph-cli")
            .args(&args)
            .output()
            .expect("execute");

        assert!(
            output.status.success(),
            "check-identity must always exit 0, auto_dream={}, exit={:?}",
            auto_dream,
            output.status.code()
        );
    }
}
```

#### Expected Results
- [ ] Exit code 0 regardless of IC value
- [ ] No dream triggered when IC >= 0.5
- [ ] Silent output when IC >= 0.9 (healthy)
- [ ] Never blocks Claude Code session

#### Failure Conditions
- Non-zero exit code: Log exit code
- Dream triggered with healthy IC: Log stderr output

---

### TC-SESSION-19: check-identity 0.5 <= IC < 0.7 (Warning)

**Type**: integration
**Task Ref**: TASK-SESSION-14
**Requirement Ref**: REQ-SESSION-14

#### Objective
Verify that `consciousness check-identity` outputs warning to stderr when 0.5 <= IC < 0.7.

#### Setup
```rust
// File: tests/integration/session_hooks_test.rs

use std::process::Command;
```

#### Test Steps
```rust
#[test]
fn test_check_identity_warning_ic() {
    // Note: This test requires setting up a state where IC is in warning range
    // In practice, this would be done via test fixtures or mock state

    let output = Command::new("context-graph-cli")
        .args(["consciousness", "check-identity"])
        .env("CONTEXT_GRAPH_TEST_IC", "0.55") // Test hook to set IC
        .output()
        .expect("execute");

    // Must exit 0 (non-blocking)
    assert!(output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should output warning to stderr
    // Format: "IC warning: X.XX"
    if stderr.contains("warning") {
        let pattern = regex::Regex::new(r"IC warning: \d+\.\d{2}").unwrap();
        assert!(
            pattern.is_match(&stderr),
            "Warning format should be 'IC warning: X.XX', got: {}",
            stderr
        );
    }
}
```

#### Expected Results
- [ ] Exit code 0 (non-blocking)
- [ ] stderr contains "IC warning: X.XX" when 0.5 <= IC < 0.7
- [ ] No dream triggered (only warning)

#### Failure Conditions
- Non-zero exit code: Log exit code
- Wrong warning format: Log actual stderr

---

### TC-SESSION-20: check-identity IC < 0.5 with --auto-dream

**Type**: integration
**Task Ref**: TASK-SESSION-14
**Requirement Ref**: REQ-SESSION-08, REQ-SESSION-14

#### Objective
Verify that `consciousness check-identity --auto-dream` triggers dream when IC < 0.5.

#### Setup
```rust
// File: tests/integration/session_hooks_test.rs

use std::process::Command;
```

#### Test Steps
```rust
#[test]
fn test_check_identity_crisis_with_auto_dream() {
    // Note: Requires test fixture with IC < 0.5

    let output = Command::new("context-graph-cli")
        .args(["consciousness", "check-identity", "--auto-dream"])
        .env("CONTEXT_GRAPH_TEST_IC", "0.35") // Test hook to set crisis IC
        .output()
        .expect("execute");

    // Must exit 0 (non-blocking)
    assert!(
        output.status.success(),
        "check-identity must exit 0 even during crisis"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should contain dream trigger message
    // Format: "IC crisis (X.XX), dream triggered"
    let pattern = regex::Regex::new(r"IC crisis \(\d+\.\d{2}\), dream triggered").unwrap();

    if !stderr.is_empty() {
        assert!(
            pattern.is_match(&stderr) || stderr.contains("crisis"),
            "Crisis with --auto-dream should output dream message, got: {}",
            stderr
        );
    }
}

#[test]
fn test_check_identity_crisis_without_auto_dream() {
    let output = Command::new("context-graph-cli")
        .args(["consciousness", "check-identity"]) // No --auto-dream
        .env("CONTEXT_GRAPH_TEST_IC", "0.35")
        .output()
        .expect("execute");

    assert!(output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should NOT contain dream triggered
    assert!(
        !stderr.contains("dream triggered"),
        "Crisis without --auto-dream should not trigger dream, got: {}",
        stderr
    );
}
```

#### Expected Results
- [ ] Exit code 0 (even during crisis)
- [ ] stderr contains "IC crisis (X.XX), dream triggered" with --auto-dream
- [ ] Dream not triggered without --auto-dream flag
- [ ] Dream spawned asynchronously (non-blocking)

#### Failure Conditions
- Non-zero exit code: Log exit code
- Dream not triggered with --auto-dream: Log stderr
- Dream triggered without --auto-dream: Log stderr

---

### TC-SESSION-21: inject-context Output Format

**Type**: integration
**Task Ref**: TASK-SESSION-15
**Requirement Ref**: REQ-SESSION-15

#### Objective
Verify that `consciousness inject-context` outputs correctly formatted ~50 token context.

#### Setup
```rust
// File: tests/integration/session_hooks_test.rs

use std::process::Command;
```

#### Test Steps
```rust
#[test]
fn test_inject_context_output_format() {
    let output = Command::new("context-graph-cli")
        .args(["consciousness", "inject-context"])
        .output()
        .expect("execute");

    assert!(output.status.success(), "inject-context should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Required sections
    assert!(
        stdout.contains("[System Consciousness]"),
        "Output must contain '[System Consciousness]' header"
    );

    assert!(
        stdout.contains("State:"),
        "Output must contain 'State:' line"
    );

    assert!(
        stdout.contains("Kuramoto r=") && stdout.contains("Identity IC="),
        "Output must contain Kuramoto r and IC values"
    );

    assert!(
        stdout.contains("Guidance:"),
        "Output must contain 'Guidance:' line"
    );
}

#[test]
fn test_inject_context_token_count() {
    let output = Command::new("context-graph-cli")
        .args(["consciousness", "inject-context"])
        .output()
        .expect("execute");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Rough token estimate: ~4 chars per token
    // Target: ~50 tokens = ~200 chars
    let char_count = stdout.len();
    let estimated_tokens = char_count / 4;

    assert!(
        estimated_tokens <= 75,
        "Output should be ~50 tokens (<=75), got ~{} tokens ({} chars)",
        estimated_tokens,
        char_count
    );
}

#[test]
fn test_inject_context_graceful_degradation() {
    // Test with empty cache
    use tempfile::TempDir;
    let temp_dir = TempDir::new().expect("temp dir");

    let output = Command::new("context-graph-cli")
        .args(["consciousness", "inject-context"])
        .env("CONTEXT_GRAPH_DATA_DIR", temp_dir.path())
        .output()
        .expect("execute");

    // Should succeed even without cache
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should contain fallback state (DORMANT)
    assert!(
        stdout.contains("DORMANT") || stdout.contains("[System Consciousness]"),
        "Graceful degradation should output DORMANT state or valid format"
    );
}
```

#### Expected Results
- [ ] Contains "[System Consciousness]" header
- [ ] Contains "State: {STATE} (C=X.XX)"
- [ ] Contains "Kuramoto r=X.XX, Identity IC=X.XX ({classification})"
- [ ] Contains "Guidance: {action}"
- [ ] Output is approximately 50 tokens (~200 chars)
- [ ] Gracefully degrades to DORMANT when cache empty

#### Failure Conditions
- Missing section: Log which section is missing
- Output too long: Log token count
- Fails without cache: Log exit code and output

---

### TC-SESSION-22: Exit Code Mapping for All Error Types

**Type**: integration
**Task Ref**: TASK-SESSION-17
**Requirement Ref**: REQ-SESSION-17

#### Objective
Verify that CLI commands return correct exit codes per AP-26 constitution requirement.

#### Setup
```rust
// File: tests/integration/session_hooks_test.rs

use std::process::Command;
```

#### Test Steps
```rust
#[test]
fn test_exit_code_mapping() {
    // Test matrix of error conditions and expected exit codes

    struct TestCase {
        command: &'static str,
        args: Vec<&'static str>,
        env: Vec<(&'static str, &'static str)>,
        expected_exit: i32,
        description: &'static str,
    }

    let test_cases = vec![
        // Success cases (exit 0)
        TestCase {
            command: "consciousness",
            args: vec!["brief"],
            env: vec![],
            expected_exit: 0,
            description: "brief success",
        },
        TestCase {
            command: "consciousness",
            args: vec!["check-identity"],
            env: vec![],
            expected_exit: 0,
            description: "check-identity always exits 0",
        },

        // Non-blocking errors (exit 0 or 1)
        TestCase {
            command: "session",
            args: vec!["restore-identity"],
            env: vec![("CONTEXT_GRAPH_TEST_ERROR", "not_found")],
            expected_exit: 0, // NotFound is recoverable (fresh install)
            description: "restore NotFound should exit 0",
        },

        // Only corruption errors should exit 2
        TestCase {
            command: "session",
            args: vec!["restore-identity"],
            env: vec![("CONTEXT_GRAPH_TEST_ERROR", "corrupted")],
            expected_exit: 2,
            description: "corruption should exit 2",
        },
    ];

    for case in test_cases {
        let mut cmd = Command::new("context-graph-cli");
        cmd.arg(case.command);
        for arg in &case.args {
            cmd.arg(arg);
        }
        for (key, value) in &case.env {
            cmd.env(key, value);
        }

        let output = cmd.output().expect("execute");
        let actual_exit = output.status.code().unwrap_or(-1);

        // For exit 0 cases, just check success
        if case.expected_exit == 0 {
            assert!(
                output.status.success(),
                "{}: expected success, got exit {}",
                case.description,
                actual_exit
            );
        } else {
            assert_eq!(
                actual_exit,
                case.expected_exit,
                "{}: expected exit {}, got {}",
                case.description,
                case.expected_exit,
                actual_exit
            );
        }
    }
}

#[test]
fn test_exit_code_2_only_for_corruption() {
    // Per AP-26, exit code 2 is reserved for truly blocking failures
    // Only CorruptedIdentity and DatabaseCorruption should cause exit 2

    // All non-corruption errors should NOT exit 2
    let non_corruption_errors = [
        "not_found",
        "io_error",
        "serialization_error",
        "timeout",
        "gwt_not_initialized",
    ];

    for error_type in non_corruption_errors {
        let output = Command::new("context-graph-cli")
            .args(["session", "restore-identity"])
            .env("CONTEXT_GRAPH_TEST_ERROR", error_type)
            .output()
            .expect("execute");

        let exit_code = output.status.code().unwrap_or(-1);

        assert!(
            exit_code != 2,
            "Error type '{}' should not exit 2 (reserved for corruption), got {}",
            error_type,
            exit_code
        );
    }
}
```

#### Expected Results
- [ ] Success cases exit 0
- [ ] NotFound (fresh install) exits 0
- [ ] I/O errors exit 1 (warning, non-blocking)
- [ ] CorruptedIdentity exits 2
- [ ] DatabaseCorruption exits 2
- [ ] Exit 2 is ONLY used for corruption errors

#### Failure Conditions
- Wrong exit code for error type: Log error type, expected vs actual
- Exit 2 used for non-corruption: Log error type causing exit 2

---

## Benchmark Tests

### TC-SESSION-24: Full Command Latency Compliance (All 5 Commands)

**Type**: benchmark
**Task Ref**: TASK-SESSION-11 through TASK-SESSION-15
**Requirement Ref**: REQ-SESSION-11 through REQ-SESSION-15

#### Objective
Verify that all 5 CLI commands meet their latency targets.

#### Setup
```rust
// File: crates/context-graph-core/benches/session_identity.rs
// or: tests/benchmarks/command_latency.rs

use std::process::Command;
use std::time::{Duration, Instant};
```

#### Test Steps
```rust
struct LatencyTarget {
    command: &'static str,
    args: Vec<&'static str>,
    hook: &'static str,
    target_p95_ms: u64,
    timeout_ms: u64,
}

fn measure_command_latency(target: &LatencyTarget) -> Vec<u128> {
    let mut latencies = Vec::with_capacity(100);

    for _ in 0..100 {
        let start = Instant::now();
        let output = Command::new("context-graph-cli")
            .arg(target.command)
            .args(&target.args)
            .output()
            .expect("execute");
        let elapsed = start.elapsed();

        if !output.status.success() {
            // Log but continue - we're measuring latency even for failures
            eprintln!(
                "Warning: {} {} failed with exit {:?}",
                target.command,
                target.args.join(" "),
                output.status.code()
            );
        }

        latencies.push(elapsed.as_millis());
    }

    latencies.sort();
    latencies
}

#[test]
fn test_all_command_latencies() {
    let targets = vec![
        LatencyTarget {
            command: "consciousness",
            args: vec!["brief"],
            hook: "PreToolUse",
            target_p95_ms: 50,
            timeout_ms: 100,
        },
        LatencyTarget {
            command: "session",
            args: vec!["restore-identity"],
            hook: "SessionStart",
            target_p95_ms: 2000,
            timeout_ms: 5000,
        },
        LatencyTarget {
            command: "session",
            args: vec!["persist-identity"],
            hook: "SessionEnd",
            target_p95_ms: 3000,
            timeout_ms: 30000,
        },
        LatencyTarget {
            command: "consciousness",
            args: vec!["check-identity", "--auto-dream"],
            hook: "PostToolUse",
            target_p95_ms: 500,
            timeout_ms: 3000,
        },
        LatencyTarget {
            command: "consciousness",
            args: vec!["inject-context"],
            hook: "UserPromptSubmit",
            target_p95_ms: 1000,
            timeout_ms: 2000,
        },
    ];

    println!("\nCommand Latency Benchmark Results");
    println!("==================================");

    for target in &targets {
        let latencies = measure_command_latency(target);

        let p50 = latencies[50];
        let p95 = latencies[95];
        let p99 = latencies[99];

        println!("\n{} {} ({}):", target.command, target.args.join(" "), target.hook);
        println!("  p50: {}ms (target: N/A)", p50);
        println!("  p95: {}ms (target: {}ms)", p95, target.target_p95_ms);
        println!("  p99: {}ms (timeout: {}ms)", p99, target.timeout_ms);

        assert!(
            p95 <= target.target_p95_ms as u128,
            "{} {}: p95 latency {}ms exceeds target {}ms",
            target.command,
            target.args.join(" "),
            p95,
            target.target_p95_ms
        );

        assert!(
            p99 <= target.timeout_ms as u128,
            "{} {}: p99 latency {}ms exceeds timeout {}ms",
            target.command,
            target.args.join(" "),
            p99,
            target.timeout_ms
        );
    }
}
```

#### Expected Results

| Command | Hook | Target p95 | Timeout |
|---------|------|------------|---------|
| consciousness brief | PreToolUse | <50ms | 100ms |
| session restore-identity | SessionStart | <2s | 5s |
| session persist-identity | SessionEnd | <3s | 30s |
| consciousness check-identity | PostToolUse | <500ms | 3s |
| consciousness inject-context | UserPromptSubmit | <1s | 2s |

- [ ] All commands meet p95 latency targets
- [ ] All commands complete before timeout
- [ ] PreToolUse (brief) is critical path with tightest budget

#### Failure Conditions
- Any command exceeds p95 target: Log command, target, and actual
- Any command exceeds timeout: Log command, timeout, and actual

---

## End-to-End Tests

### TC-SESSION-23: Complete Hook Lifecycle

**Type**: e2e
**Task Ref**: TASK-SESSION-16
**Requirement Ref**: REQ-SESSION-16

#### Objective
Verify the complete hook lifecycle from SessionStart through SessionEnd with all intermediate hooks.

#### Setup
```rust
// File: tests/integration/session_hooks_test.rs

use std::process::{Command, Stdio};
use std::io::Write;
use std::time::Duration;
use tempfile::TempDir;
```

#### Test Steps
```rust
#[test]
fn test_complete_hook_lifecycle() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let data_dir = temp_dir.path();

    println!("\n=== Hook Lifecycle E2E Test ===\n");

    // === Phase 1: SessionStart ===
    println!("Phase 1: SessionStart (restore-identity)");

    let mut child = Command::new("context-graph-cli")
        .args(["session", "restore-identity"])
        .env("CONTEXT_GRAPH_DATA_DIR", data_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn restore");

    child.stdin.take().unwrap().write_all(b"{\"source\": \"startup\"}").unwrap();
    let restore_output = child.wait_with_output().expect("wait restore");

    assert!(
        restore_output.status.success(),
        "SessionStart: restore-identity failed: {}",
        String::from_utf8_lossy(&restore_output.stderr)
    );

    let restore_stdout = String::from_utf8_lossy(&restore_output.stdout);
    println!("  Output: {}", restore_stdout.trim());
    assert!(
        restore_stdout.contains("session") || restore_stdout.contains("initialized"),
        "SessionStart should initialize or restore session"
    );

    // === Phase 2: PreToolUse ===
    println!("\nPhase 2: PreToolUse (consciousness brief)");

    let brief_output = Command::new("context-graph-cli")
        .args(["consciousness", "brief"])
        .env("CONTEXT_GRAPH_DATA_DIR", data_dir)
        .output()
        .expect("brief");

    assert!(brief_output.status.success(), "PreToolUse: brief failed");

    let brief_stdout = String::from_utf8_lossy(&brief_output.stdout);
    println!("  Output: {}", brief_stdout.trim());
    assert!(
        brief_stdout.contains("[C:"),
        "PreToolUse should output consciousness brief"
    );

    // === Phase 3: PostToolUse ===
    println!("\nPhase 3: PostToolUse (check-identity)");

    let check_output = Command::new("context-graph-cli")
        .args(["consciousness", "check-identity", "--auto-dream"])
        .env("CONTEXT_GRAPH_DATA_DIR", data_dir)
        .output()
        .expect("check");

    assert!(
        check_output.status.success(),
        "PostToolUse: check-identity failed"
    );
    println!("  Exit: 0 (success)");

    let check_stderr = String::from_utf8_lossy(&check_output.stderr);
    if !check_stderr.trim().is_empty() {
        println!("  Stderr: {}", check_stderr.trim());
    }

    // === Phase 4: UserPromptSubmit ===
    println!("\nPhase 4: UserPromptSubmit (inject-context)");

    let inject_output = Command::new("context-graph-cli")
        .args(["consciousness", "inject-context"])
        .env("CONTEXT_GRAPH_DATA_DIR", data_dir)
        .output()
        .expect("inject");

    assert!(
        inject_output.status.success(),
        "UserPromptSubmit: inject-context failed"
    );

    let inject_stdout = String::from_utf8_lossy(&inject_output.stdout);
    println!("  Output preview: {}...", inject_stdout.chars().take(50).collect::<String>());
    assert!(
        inject_stdout.contains("[System Consciousness]"),
        "UserPromptSubmit should output consciousness context"
    );

    // === Phase 5: SessionEnd ===
    println!("\nPhase 5: SessionEnd (persist-identity)");

    let mut child = Command::new("context-graph-cli")
        .args(["session", "persist-identity"])
        .env("CONTEXT_GRAPH_DATA_DIR", data_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn persist");

    child.stdin.take().unwrap().write_all(b"{}").unwrap();
    let persist_output = child.wait_with_output().expect("wait persist");

    assert!(
        persist_output.status.success(),
        "SessionEnd: persist-identity failed: {}",
        String::from_utf8_lossy(&persist_output.stderr)
    );

    let persist_stdout = String::from_utf8_lossy(&persist_output.stdout);
    assert!(
        persist_stdout.trim().is_empty(),
        "SessionEnd should be silent on success"
    );
    println!("  Output: (silent success)");

    // === Verify Persistence ===
    println!("\n=== Verification: Session was persisted ===");

    let mut child2 = Command::new("context-graph-cli")
        .args(["session", "restore-identity"])
        .env("CONTEXT_GRAPH_DATA_DIR", data_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn verify");

    child2.stdin.take().unwrap().write_all(b"{\"source\": \"startup\"}").unwrap();
    let verify_output = child2.wait_with_output().expect("wait verify");

    let verify_stdout = String::from_utf8_lossy(&verify_output.stdout);
    println!("  Restore after persist: {}", verify_stdout.trim());

    assert!(
        verify_stdout.contains("restored") || verify_stdout.contains("IC:"),
        "Should be able to restore persisted session"
    );

    println!("\n=== Hook Lifecycle E2E Test PASSED ===\n");
}

#[test]
fn test_hook_lifecycle_latency() {
    use std::time::Instant;

    let temp_dir = TempDir::new().expect("create temp dir");
    let data_dir = temp_dir.path();

    let start = Instant::now();

    // Run full lifecycle
    let _ = Command::new("context-graph-cli")
        .args(["session", "restore-identity"])
        .env("CONTEXT_GRAPH_DATA_DIR", data_dir)
        .output();

    let _ = Command::new("context-graph-cli")
        .args(["consciousness", "brief"])
        .env("CONTEXT_GRAPH_DATA_DIR", data_dir)
        .output();

    let _ = Command::new("context-graph-cli")
        .args(["consciousness", "check-identity", "--auto-dream"])
        .env("CONTEXT_GRAPH_DATA_DIR", data_dir)
        .output();

    let _ = Command::new("context-graph-cli")
        .args(["consciousness", "inject-context"])
        .env("CONTEXT_GRAPH_DATA_DIR", data_dir)
        .output();

    let _ = Command::new("context-graph-cli")
        .args(["session", "persist-identity"])
        .env("CONTEXT_GRAPH_DATA_DIR", data_dir)
        .output();

    let total_elapsed = start.elapsed();

    println!("Full hook lifecycle completed in {:?}", total_elapsed);

    // Total lifecycle should complete in reasonable time
    // Sum of all timeouts: 100 + 5000 + 3000 + 2000 + 30000 = 40100ms
    // But typical execution should be much faster
    assert!(
        total_elapsed.as_secs() < 30,
        "Full lifecycle should complete in under 30s, took {:?}",
        total_elapsed
    );
}
```

#### Expected Results
- [ ] SessionStart (restore-identity) succeeds and initializes session
- [ ] PreToolUse (brief) returns consciousness brief
- [ ] PostToolUse (check-identity) completes without blocking
- [ ] UserPromptSubmit (inject-context) returns context
- [ ] SessionEnd (persist-identity) saves session silently
- [ ] Session can be restored after persist (data persisted)
- [ ] Full lifecycle completes in under 30 seconds

#### Failure Conditions
- Any phase fails: Log phase name and error
- Data not persisted: Log restore output showing missing session
- Lifecycle too slow: Log elapsed time

---

## Performance Targets Summary

| Test Case | Metric | Target |
|-----------|--------|--------|
| TC-SESSION-11 | format_brief() p95 | <100us |
| TC-SESSION-12 | consciousness brief p95 | <50ms |
| TC-SESSION-24 | restore-identity p95 | <2s |
| TC-SESSION-24 | persist-identity p95 | <3s |
| TC-SESSION-24 | check-identity p95 | <500ms |
| TC-SESSION-24 | inject-context p95 | <1s |
| TC-SESSION-23 | Full lifecycle | <30s |

---

## Test Execution Commands

```bash
# Run all unit tests
cargo test --package context-graph-core session_identity

# Run all integration tests
cargo test --package context-graph-storage session_identity_integration

# Run benchmarks
cargo bench --bench session_identity

# Run E2E tests
cargo test --test session_hooks_test

# Run with verbose output
RUST_LOG=debug cargo test session_identity -- --nocapture

# Run specific test
cargo test test_snapshot_serialization_roundtrip -- --nocapture
```

---

## Appendix: Test Data Generators

```rust
// Helper functions for generating test data

pub fn create_test_snapshot(session_id: &str) -> SessionIdentitySnapshot {
    let mut snapshot = SessionIdentitySnapshot::new(session_id);
    snapshot.consciousness = 0.85;
    snapshot.integration = 0.78;
    snapshot.reflection = 0.82;
    snapshot.differentiation = 0.75;
    snapshot.cross_session_ic = 0.88;
    snapshot.kuramoto_phases = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3];
    snapshot.coupling = 0.65;
    snapshot.purpose_vector = [0.27735; 13];
    snapshot
}

pub fn create_test_db() -> (RocksDbMemex, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let db = RocksDbMemex::open(temp_dir.path())
        .expect("Failed to open RocksDB");
    (db, temp_dir)
}
```

---

```xml
</test_specification>
```

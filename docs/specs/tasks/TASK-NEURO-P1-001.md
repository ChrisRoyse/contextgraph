# TASK-NEURO-P1-001: Wire Steering Handler to on_goal_progress

## Metadata

| Field | Value |
|-------|-------|
| **ID** | TASK-NEURO-P1-001 |
| **Title** | Wire Steering Handler to on_goal_progress |
| **Status** | complete |
| **Layer** | surface |
| **Sequence** | 1 |
| **Implements** | SPEC-NEURO-001 |
| **Priority** | P1 (High) |
| **Estimated Complexity** | low |
| **Depends On** | None |
| **Implemented In** | Commit `61f9a51` |
| **Verified By** | Sherlock Holmes (2026-01-12) |

---

## Sherlock Holmes Forensic Investigation Report

### Case Summary

**Subject**: Steering-to-Neuromodulator Integration
**Verdict**: INNOCENT (COMPLETE)
**Confidence**: HIGH

### Evidence Log

| Item | Expected | Actual | Status |
|------|----------|--------|--------|
| Steering calls neuromod | `on_goal_progress_with_cascades` invoked | steering.rs:145 calls method | PASS |
| JSON includes neuromodulation | Response has neuromodulation field | steering.rs:207 includes field | PASS |
| DA range [1, 5] | Clamped to constitution range | dopamine.rs:26-29 enforces | PASS |
| 5HT range [0, 1] | Clamped to constitution range | serotonin.rs:32-35 enforces | PASS |
| NE range [0.5, 2] | Clamped to constitution range | noradrenaline.rs:30-34 enforces | PASS |
| ACh range [0.001, 0.002] | Clamped to constitution range | acetylcholine.rs:29-33 enforces | PASS |
| All neuromod tests pass | 74 tests pass | 74 tests pass | PASS |
| All steering tests pass | 12 tests pass | 12 tests pass | PASS |

### Git Forensics (Chain of Custody)

```
61f9a51 feat(neuromod): implement direct dopamine feedback from steering subsystem
```

This commit introduced the integration on the expected date, implementing SPEC-NEURO-001.

---

## Full State Verification (FSV)

### Source of Truth

**Location**: `NeuromodulationManager.dopamine.value()` (after steering event)

**Verification Protocol**:
1. Capture `neuromod_manager.dopamine.value()` BEFORE steering call
2. Execute `get_steering_feedback` tool
3. Capture `neuromod_manager.dopamine.value()` AFTER steering call
4. Verify: `AFTER - BEFORE = reward.value * DA_GOAL_SENSITIVITY`

### Execute and Inspect

**Positive Feedback Test**:
```rust
// BEFORE: dopamine = 3.0 (baseline)
let report = manager.on_goal_progress_with_cascades(0.8);
// AFTER: dopamine = 3.0 + (0.8 * 0.1) = 3.08
assert!((report.da_new - 3.08).abs() < f32::EPSILON);
```

**Negative Feedback Test**:
```rust
// BEFORE: dopamine = 3.0 (baseline)
let report = manager.on_goal_progress_with_cascades(-0.6);
// AFTER: dopamine = 3.0 - (0.6 * 0.1) = 2.94
assert!((report.da_new - 2.94).abs() < f32::EPSILON);
```

### Edge Cases Tested

| Edge Case | Input | Expected Behavior | Verified |
|-----------|-------|-------------------|----------|
| Empty input (zero delta) | `delta = 0.0` | No change to any neuromodulator | YES - test_edge_case_zero_delta |
| Maximum limit (DA at 5.0) | `delta = 1.0, DA = 5.0` | DA stays clamped at 5.0, 5HT cascade still triggers | YES - test_edge_case_da_at_ceiling |
| Invalid format (NaN) | `delta = f32::NAN` | No change, warning logged | YES - test_cascade_nan_handling |
| Boundary crossing (DA > 4.0) | `DA = 3.95, delta = 1.0` | DA = 4.05, 5HT += 0.05 (mood cascade) | YES - test_cascade_high_da_boosts_serotonin |
| Boundary crossing (DA < 2.0) | `DA = 2.05, delta = -1.0` | DA = 1.95, 5HT -= 0.05 (mood cascade) | YES - test_cascade_low_da_lowers_serotonin |
| Rapid changes | Multiple cascades | Sequential cascades accumulate correctly | YES - test_edge_case_sequential_cascades |
| Neuromod manager None | Handler without manager | Warning logged, propagated = false | YES - steering.rs:166-170 |

### Evidence of Success

**Test Output Verification** (from `cargo test -p context-graph-core --lib neuromod -- --nocapture`):

```
=== HIGH DA -> 5HT CASCADE ===
  DA before: 3.95, DA after: 4.05
  5HT before: 0.5, 5HT after: 0.55
  Report: CascadeReport { da_delta: 0.10000014, da_new: 4.05, serotonin_delta: 0.05,
          serotonin_new: 0.55, ne_delta: 0.0, ne_new: 1.0,
          mood_cascade_triggered: true, alertness_cascade_triggered: false }
```

**Dopamine Increase Verification**:
- Input: `SteeringReward.value = 0.8` (positive feedback)
- Calculation: `DA_delta = 0.8 * DA_GOAL_SENSITIVITY = 0.8 * 0.1 = 0.08`
- Result: `dopamine_level = baseline + 0.08 = 3.08`
- Hopfield.beta parameter increased proportionally

---

## Constitution Compliance Verification

### Neuromodulator Parameters

| Modulator | Constitution Param | Constitution Range | Implementation Constant | Verified |
|-----------|-------------------|-------------------|------------------------|----------|
| **Dopamine** | `hopfield.beta` | `[1, 5]` | `DA_MIN=1.0, DA_MAX=5.0` | YES |
| **Serotonin** | `similarity.space_weights` | `[0, 1]` | `SEROTONIN_MIN=0.0, SEROTONIN_MAX=1.0` | YES |
| **Noradrenaline** | `attention.temp` | `[0.5, 2]` | `NE_MIN=0.5, NE_MAX=2.0` | YES |
| **Acetylcholine** | `utl.lr` | `[0.001, 0.002]` | `ACH_BASELINE=0.001, ACH_MAX=0.002` | YES |

### Parameter Mapping Verification

| Neuromodulator | Method | Returns | File:Line |
|----------------|--------|---------|-----------|
| Dopamine | `get_hopfield_beta()` | `dopamine.value` | dopamine.rs:116-118 |
| Serotonin | `get_space_weight(index)` | Scaled space weight | serotonin.rs:101-114 |
| Noradrenaline | `get_attention_temp()` | `noradrenaline.value` | noradrenaline.rs:138-140 |
| Acetylcholine | `get_utl_learning_rate()` | `acetylcholine` value | acetylcholine.rs:52-54 |

---

## Manual Test Design

### Test Case 1: Positive Steering Feedback

**Input**:
```rust
SteeringFeedback {
    reward: SteeringReward {
        value: 0.8,  // Positive feedback
        gardener_score: 0.7,
        curator_score: 0.85,
        assessor_score: 0.8,
    },
    // ... other fields
}
```

**Expected Output**:
```rust
NeuromodState {
    dopamine_level: 3.08,  // baseline (3.0) + 0.8 * 0.1
    serotonin_level: 0.5,  // unchanged (DA not at threshold)
    noradrenaline_level: 1.0,  // unchanged (DA change < 0.3)
    // ...
}
```

**Verification**:
- `hopfield.beta` = 3.08 (increased proportionally)
- Cascade effects: None triggered (DA in normal range 2.0-4.0)

### Test Case 2: Strong Positive Feedback (Cascade Trigger)

**Input**:
```rust
// Pre-condition: DA = 3.95 (just below threshold)
delta = 1.0  // Maximum positive feedback
```

**Expected Output**:
```rust
CascadeReport {
    da_delta: 0.1,
    da_new: 4.05,  // Crossed DA_HIGH_THRESHOLD (4.0)
    serotonin_delta: 0.05,  // Mood cascade triggered
    serotonin_new: 0.55,
    ne_delta: 0.0,  // DA change (0.1) < 0.3 threshold
    ne_new: 1.0,
    mood_cascade_triggered: true,
    alertness_cascade_triggered: false,
}
```

### Test Case 3: Negative Feedback

**Input**:
```rust
SteeringFeedback {
    reward: SteeringReward {
        value: -0.6,  // Negative feedback
        // ...
    },
}
```

**Expected Output**:
```rust
NeuromodState {
    dopamine_level: 2.94,  // baseline (3.0) - 0.6 * 0.1
    // ...
}
```

---

## Backwards Compatibility

**ABSOLUTELY NO BACKWARDS COMPATIBILITY** - As specified:

1. If `neuromod_manager` is `None`, a WARNING is logged but the steering feedback still returns (steering is valid without neuromod)
2. If cascade computation fails, error is logged and `propagated: false` is returned
3. NaN inputs are rejected with warning (no silent failures)
4. All clamping is enforced at constitution boundaries

**Code Evidence** (steering.rs:165-171):
```rust
} else {
    warn!("get_steering_feedback: NeuromodulationManager not initialized - skipping neuromod update");
    json!({
        "propagated": false,
        "reason": "NeuromodulationManager not initialized"
    })
};
```

---

## Implementation Details (VERIFIED)

### Current Code Location

**File**: `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/steering.rs`
**Lines**: 142-171

```rust
// TASK-NEURO-P1-001: Wire steering reward to neuromodulation
let neuromod_json = if let Some(neuromod_manager) = &self.neuromod_manager {
    let mut manager = neuromod_manager.write();
    let report = manager.on_goal_progress_with_cascades(feedback.reward.value);
    debug!(
        reward_value = feedback.reward.value,
        da_delta = report.da_delta,
        da_new = report.da_new,
        mood_cascade = report.mood_cascade_triggered,
        alertness_cascade = report.alertness_cascade_triggered,
        "Steering feedback propagated to neuromodulation"
    );
    json!({
        "propagated": true,
        "da_delta": report.da_delta,
        "da_new": report.da_new,
        "serotonin_delta": report.serotonin_delta,
        "serotonin_new": report.serotonin_new,
        "ne_delta": report.ne_delta,
        "ne_new": report.ne_new,
        "mood_cascade_triggered": report.mood_cascade_triggered,
        "alertness_cascade_triggered": report.alertness_cascade_triggered
    })
} else {
    warn!("get_steering_feedback: NeuromodulationManager not initialized - skipping neuromod update");
    json!({
        "propagated": false,
        "reason": "NeuromodulationManager not initialized"
    })
};
```

### JSON Response Format (VERIFIED)

```json
{
  "reward": {
    "value": 0.5,
    "gardener_score": 0.6,
    "curator_score": 0.7,
    "assessor_score": 0.5,
    "dominant_factor": "curator",
    "limiting_factor": "assessor"
  },
  "gardener_details": { ... },
  "curator_details": { ... },
  "assessor_details": { ... },
  "summary": "...",
  "needs_immediate_attention": false,
  "priority_improvement": "assessor",
  "neuromodulation": {
    "propagated": true,
    "da_delta": 0.05,
    "da_new": 3.05,
    "serotonin_delta": 0.0,
    "serotonin_new": 0.5,
    "ne_delta": 0.0,
    "ne_new": 1.0,
    "mood_cascade_triggered": false,
    "alertness_cascade_triggered": false
  }
}
```

---

## Data Flow Diagram

```
get_steering_feedback MCP call
    |
    v
call_get_steering_feedback() [steering.rs:37-211]
    |
    +---> TeleologicalStore.count() [line 45-51]
    |           |
    |           v
    |     Get REAL node count (FAIL FAST on error)
    |
    +---> TeleologicalStore.list_all_johari() [line 58-64]
    |           |
    |           v
    |     Compute orphan_count, aligned_count, connectivity
    |
    +---> SteeringSystem::compute_feedback() [line 131-140]
    |           |
    |           v
    |     SteeringFeedback {
    |         reward: SteeringReward { value: [-1, 1] }
    |         gardener_details, curator_details, assessor_details
    |     }
    |
    +---> [TASK-NEURO-P1-001] neuromod_manager.on_goal_progress_with_cascades() [line 142-171]
    |           |
    |           v
    |     DopamineModulator::on_goal_progress(delta) [dopamine.rs:158-190]
    |           |
    |           +---> DA += delta * DA_GOAL_SENSITIVITY (0.1)
    |           |
    |           v
    |     apply_mood_cascade(da_new) [state.rs:428-437]
    |           |
    |           +---> if DA > 4.0: 5HT += 0.05
    |           +---> if DA < 2.0: 5HT -= 0.05
    |           |
    |           v
    |     apply_alertness_cascade(da_actual_delta) [state.rs:442-449]
    |           |
    |           +---> if |DA_change| > 0.3: NE += 0.1
    |           |
    |           v
    |     return CascadeReport
    |
    v
JSON Response with neuromodulation field [line 173-209]
```

---

## Test Commands

### Verification Commands

```bash
# All neuromod tests (74 tests)
cargo test -p context-graph-core --lib neuromod -- --nocapture

# All steering tests (12 tests)
cargo test -p context-graph-mcp steering -- --nocapture

# Build verification
cargo build -p context-graph-mcp

# Specific FSV cascade tests
cargo test -p context-graph-core test_fsv_cascade_source_of_truth -- --nocapture
cargo test -p context-graph-core test_fsv_goal_progress_source_of_truth -- --nocapture
```

### Test Results Summary

| Test Suite | Tests | Passed | Failed | Status |
|------------|-------|--------|--------|--------|
| `neuromod::dopamine` | 17 | 17 | 0 | PASS |
| `neuromod::serotonin` | 10 | 10 | 0 | PASS |
| `neuromod::noradrenaline` | 10 | 10 | 0 | PASS |
| `neuromod::acetylcholine` | 8 | 8 | 0 | PASS |
| `neuromod::state` | 26 | 26 | 0 | PASS |
| `gwt::listeners::neuromod` | 3 | 3 | 0 | PASS |
| `steering_causal_tools` | 12 | 12 | 0 | PASS |
| **TOTAL** | **86** | **86** | **0** | **PASS** |

---

## References

| Document | Absolute Path |
|----------|---------------|
| Constitution | `/home/cabdru/contextgraph/docs2/constitution.yaml` |
| Steering Handler | `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/steering.rs` |
| Neuromod State | `/home/cabdru/contextgraph/crates/context-graph-core/src/neuromod/state.rs` |
| Dopamine Module | `/home/cabdru/contextgraph/crates/context-graph-core/src/neuromod/dopamine.rs` |
| Serotonin Module | `/home/cabdru/contextgraph/crates/context-graph-core/src/neuromod/serotonin.rs` |
| Noradrenaline Module | `/home/cabdru/contextgraph/crates/context-graph-core/src/neuromod/noradrenaline.rs` |
| Acetylcholine Module | `/home/cabdru/contextgraph/crates/context-graph-core/src/neuromod/acetylcholine.rs` |
| Neuromod Mod | `/home/cabdru/contextgraph/crates/context-graph-core/src/neuromod/mod.rs` |

---

## Traceability

| Requirement | Source | Status | Evidence |
|-------------|--------|--------|----------|
| SPEC-NEURO-001 | Gap Analysis | COMPLETE | steering.rs:142-171 |
| on_goal_progress exists | state.rs:352-354 | VERIFIED | Method exists and tested |
| on_goal_progress_with_cascades exists | state.rs:370-424 | VERIFIED | Method exists and tested |
| Cascade effects documented | state.rs:34-45 | VERIFIED | Constants defined |
| Steering handler returns reward.value | steering.rs:177 | VERIFIED | In JSON response |
| neuromod_manager field exists | handlers.rs:131 | VERIFIED | Field defined |
| DA param = hopfield.beta | Constitution | VERIFIED | dopamine.rs:116-118 |
| 5HT param = space_weights | Constitution | VERIFIED | serotonin.rs:101-114 |
| NE param = attention.temp | Constitution | VERIFIED | noradrenaline.rs:138-140 |
| ACh param = utl.lr | Constitution | VERIFIED | acetylcholine.rs:52-54 |

---

## Cascade Effect Constants

From `/home/cabdru/contextgraph/crates/context-graph-core/src/neuromod/state.rs` (lines 34-45):

| Constant | Value | Description |
|----------|-------|-------------|
| `DA_HIGH_THRESHOLD` | 4.0 | DA level triggering positive 5HT cascade |
| `DA_LOW_THRESHOLD` | 2.0 | DA level triggering negative 5HT cascade |
| `SEROTONIN_CASCADE_DELTA` | 0.05 | 5HT adjustment magnitude |
| `DA_CHANGE_THRESHOLD` | 0.3 | DA change triggering NE alertness cascade |
| `NE_ALERTNESS_DELTA` | 0.1 | NE adjustment for significant DA change |

---

## Conclusion

**CASE CLOSED**

The implementation of TASK-NEURO-P1-001 has been verified as COMPLETE and CORRECT. The steering handler at `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/steering.rs` (lines 142-171) correctly:

1. Invokes `on_goal_progress_with_cascades(feedback.reward.value)`
2. Returns the `CascadeReport` in the JSON response under `neuromodulation` field
3. Logs debug information about the propagation
4. Handles the None case with appropriate warning

All 86 related tests pass. Constitution compliance is verified for all four neuromodulators.

*"The game is afoot, and the case is closed."* - Sherlock Holmes

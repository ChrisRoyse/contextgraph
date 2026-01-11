# TASK-DREAM-P0-001: Dream Layer Types and Interfaces

## Metadata

| Field | Value |
|-------|-------|
| **Task ID** | TASK-DREAM-P0-001 |
| **Spec Ref** | SPEC-DREAM-001 |
| **Layer** | 1 (Foundation) |
| **Priority** | P0 - Critical |
| **Effort** | 2 hours |
| **Dependencies** | None |
| **Blocks** | TASK-DREAM-P0-002, TASK-DREAM-P0-003, TASK-DREAM-P0-004, TASK-DREAM-P0-005 |
| **Last Audited** | 2026-01-11 |
| **Status** | **COMPLETED** |
| **Completed Date** | 2026-01-11 |
| **Verification** | All 18 tests pass, cargo build succeeds |

---

## COMPLETION SUMMARY

This task is **FULLY IMPLEMENTED**. All types and interfaces are in place.

### Files Created/Modified

| File | Action | Lines |
|------|--------|-------|
| `crates/context-graph-core/src/dream/types.rs` | Created | 794 |
| `crates/context-graph-core/src/dream/mod.rs` | Modified | Added `pub mod types` + re-exports |
| `crates/context-graph-core/Cargo.toml` | Modified | Added `serde_with = "3.4"` dependency |

### Types Implemented

| Type | Purpose | Test Coverage |
|------|---------|---------------|
| `HebbianConfig` | NREM Hebbian learning config | 3 tests |
| `NodeActivation` | Node firing during replay | 4 tests |
| `HyperbolicWalkConfig` | REM hyperbolic walk config | 1 test |
| `WalkStep` | Single step in random walk | 2 tests |
| `EntropyWindow` | Entropy trigger tracking | 2 tests |
| `GpuTriggerState` | GPU budget monitoring | 5 tests |
| `ExtendedTriggerReason` | Trigger reasons enum | 1 test |

### Verification Commands

```bash
# All tests pass (18/18)
cargo test -p context-graph-core dream::types

# Build succeeds
cargo build -p context-graph-core
```

---

## Constitution Compliance (VERIFIED)

All values verified against `docs2/constitution.yaml`:

| Parameter | Value | Constitution Ref |
|-----------|-------|------------------|
| `coupling_strength` | **0.9** | line 393 (dream.phases.nrem) |
| `gpu_threshold` | **0.30** | line 394 (dream.constraints.gpu) |
| `temperature` | **2.0** | line 393 (dream.phases.rem.temp) |
| `semantic_leap` | **0.7** | line 394 (dream.constraints.semantic_leap) |

---

## For Future Tasks

### Import Path

```rust
use context_graph_core::dream::{
    HebbianConfig,
    NodeActivation,
    HyperbolicWalkConfig,
    WalkStep,
    EntropyWindow,
    GpuTriggerState,
    ExtendedTriggerReason,
};
```

### Constants Module

The `dream::constants` module provides all constitution-mandated values:

```rust
use context_graph_core::dream::constants;

// NREM/REM durations
constants::NREM_DURATION        // 3 min
constants::REM_DURATION         // 2 min

// Trigger thresholds
constants::ACTIVITY_THRESHOLD   // 0.15
constants::MAX_GPU_USAGE        // 0.30
constants::MIN_SEMANTIC_LEAP    // 0.7

// Phase parameters
constants::NREM_COUPLING        // 0.9
constants::REM_TEMPERATURE      // 2.0
constants::NREM_RECENCY_BIAS    // 0.8

// Shortcut creation
constants::MIN_SHORTCUT_HOPS    // 3
constants::MIN_SHORTCUT_TRAVERSALS // 5
constants::SHORTCUT_CONFIDENCE_THRESHOLD // 0.7
```

### Existing Controller Types (in controller.rs)

These types already exist and should NOT be recreated:

- `DreamState` - Enum with Awake, EnteringDream, Nrem, Rem, Waking variants
- `DreamStatus` - Status struct with state, gpu_usage, activity_level
- `DreamReport` - Cycle report with nrem_report, rem_report, shortcuts_created

---

## Source of Truth

### File Locations

```
crates/context-graph-core/src/dream/
├── mod.rs          # Module exports + constants
├── types.rs        # NEW - Created by this task
├── controller.rs   # DreamController, DreamState, DreamStatus, DreamReport
├── nrem.rs         # NremPhase, NremReport
├── rem.rs          # RemPhase, RemReport
├── scheduler.rs    # DreamScheduler
└── amortized.rs    # AmortizedLearner, ShortcutCandidate
```

### Constitution Values Location

`docs2/constitution.yaml` lines 391-394:

```yaml
dream:
  trigger:
    activity: "<0.15"
    idle_duration: "10min"
  phases:
    nrem:
      duration: "3min"
      recency_bias: 0.8
      coupling_strength: 0.9  # NOT 10.0
    rem:
      duration: "2min"
      temperature: 2.0
  constraints:
    max_queries: 100
    semantic_leap: ">=0.7"
    abort_on_query: true
    wake_latency: "<100ms"
    gpu_usage: "<30%"         # NOT 80%
```

---

## Dependency Information for Blocked Tasks

### TASK-DREAM-P0-002 (Poincare Ball Math)

Can now use:
- `WalkStep` with 64D position arrays
- Position validation (norm < 1.0 enforced)

### TASK-DREAM-P0-003 (Hebbian Learning)

Can now use:
- `HebbianConfig` for learning parameters
- `NodeActivation` for node firing values
- Constitution-compliant coupling_strength = 0.9

### TASK-DREAM-P0-004 (Hyperbolic Walk)

Can now use:
- `HyperbolicWalkConfig` for walk parameters
- `WalkStep` for trajectory recording
- Constitution-compliant temperature = 2.0

### TASK-DREAM-P0-005 (Dream Triggers)

Can now use:
- `EntropyWindow` for entropy tracking
- `GpuTriggerState` for GPU monitoring
- `ExtendedTriggerReason` for trigger classification
- Constitution-compliant thresholds (GPU = 0.30, entropy = 0.7)

---

## Test Results Evidence

```
$ cargo test -p context-graph-core dream::types -- --nocapture

running 18 tests
test dream::types::tests::test_extended_trigger_reason_display ... ok
test dream::types::tests::test_gpu_trigger_constitution_compliance ... ok
test dream::types::tests::test_gpu_trigger_once_only ... ok
test dream::types::tests::test_gpu_trigger_reset_allows_retrigger ... ok
test dream::types::tests::test_gpu_trigger_threshold_behavior ... ok
test dream::types::tests::test_gpu_trigger_rejects_high_threshold - should panic ... ok
test dream::types::tests::test_hebbian_config_constitution_compliance ... ok
test dream::types::tests::test_hebbian_config_validation_passes ... ok
test dream::types::tests::test_hebbian_config_validation_rejects_bad_learning_rate - should panic ... ok
test dream::types::tests::test_hyperbolic_walk_config_constitution_compliance ... ok
test dream::types::tests::test_node_activation_clamping ... ok
test dream::types::tests::test_node_activation_rejects_nil_uuid - should panic ... ok
test dream::types::tests::test_node_activation_significance_threshold ... ok
test dream::types::tests::test_node_activation_uses_real_uuid ... ok
test dream::types::tests::test_walk_step_validates_poincare_ball ... ok
test dream::types::tests::test_walk_step_rejects_outside_ball - should panic ... ok
test dream::types::tests::test_entropy_window_uses_real_time ... ok
test dream::types::tests::test_entropy_window_below_threshold_resets_tracking ... ok

test result: ok. 18 passed; 0 failed; 0 ignored
```

---

## Change Log

| Date | Change |
|------|--------|
| 2026-01-11 | Task created |
| 2026-01-11 | Implementation completed - all types created |
| 2026-01-11 | Tests verified passing (18/18) |
| 2026-01-11 | Task document updated with completion evidence |

# NREM/REM Skip Flag Investigation Report

**Date:** 2026-01-19
**Issue:** `skip_nrem` and `skip_rem` parameters in `trigger_dream` are not honored
**Severity:** Medium (feature gap, not a bug)
**Status:** ROOT CAUSE IDENTIFIED

---

## Executive Summary

The `trigger_dream` MCP tool accepts `skip_nrem` and `skip_rem` boolean parameters, but these flags are **completely ignored**. Both NREM and REM phases always execute regardless of the flag values. This is an **implementation gap** where the API was designed but the underlying controller logic was never updated to support selective phase execution.

---

## Evidence

### Test Results

| Test | skip_nrem | skip_rem | Expected | Actual |
|------|-----------|----------|----------|--------|
| Test 1 | true | false | REM only | Both NREM and REM ran |
| Test 2 | false | true | NREM only | Both NREM and REM ran |
| Test 3 | false | false | Both | Both (correct) |

### Observed Response from `skip_rem=true` Test

```json
{
  "dream_id": "395a53d7-1fbb-496d-86a6-dc2f4413ca26",
  "dry_run": false,
  "nrem_result": {
    "completed": true,
    "memories_replayed": 0,
    "duration_ms": 0
  },
  "rem_result": {
    "completed": true,
    "blind_spots_found": 100,
    "new_edges_created": 100,
    "duration_ms": 19
  },
  "status": "completed"
}
```

**Expected:** `rem_result` should be `null` when `skip_rem=true`
**Actual:** REM phase ran and returned results

---

## Root Cause Analysis

### Location 1: MCP Handler (`dream_tools.rs:159-168`)

```rust
// Execute the dream cycle
// Note: skip_nrem/skip_rem are logged but both use start_dream_cycle
// (selective phase execution to be implemented in DreamController)
if request.skip_nrem {
    info!(dream_id = %dream_id, "trigger_dream: running REM only");
} else if request.skip_rem {
    info!(dream_id = %dream_id, "trigger_dream: running NREM only");
} else {
    info!(dream_id = %dream_id, "trigger_dream: running full NREM+REM cycle");
}
let cycle_result = controller.start_dream_cycle().await;  // <-- FLAGS NOT PASSED
```

**Finding:** The handler:
1. Receives `skip_nrem` and `skip_rem` from the request
2. Logs which phases SHOULD run
3. **Ignores the flags** and calls `start_dream_cycle()` unconditionally

### Location 2: DreamController (`controller.rs:279-444`)

```rust
pub async fn start_dream_cycle(&mut self) -> CoreResult<DreamReport> {
    // ... setup code ...

    // Execute NREM phase (ALWAYS)
    let nrem_result = self.execute_nrem_phase().await;

    // ... interrupt check ...

    // Execute REM phase (ALWAYS)
    let rem_result = self.execute_rem_phase().await;

    // ... completion code ...
}
```

**Finding:** The `start_dream_cycle()` method:
1. Takes NO parameters
2. ALWAYS executes `execute_nrem_phase()`
3. ALWAYS executes `execute_rem_phase()`
4. Has no conditional logic to skip either phase

### Explicit TODO Comment

The code contains an explicit acknowledgment at line 159-160:

```rust
// Note: skip_nrem/skip_rem are logged but both use start_dream_cycle
// (selective phase execution to be implemented in DreamController)
```

---

## Code Flow Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│                         MCP Client Request                          │
│  { "skip_nrem": true, "skip_rem": false }                          │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    dream_tools.rs:call_trigger_dream()              │
│                                                                     │
│  request.skip_nrem = true   ✓ Parsed correctly                     │
│  request.skip_rem = false   ✓ Parsed correctly                     │
│                                                                     │
│  if request.skip_nrem {                                             │
│      info!("running REM only");  ← Log says REM only               │
│  }                                                                  │
│                                                                     │
│  controller.start_dream_cycle().await  ← FLAGS NOT PASSED!         │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                  controller.rs:start_dream_cycle()                  │
│                                                                     │
│  // NO PARAMETERS - cannot receive skip flags                       │
│                                                                     │
│  self.execute_nrem_phase().await   ← ALWAYS RUNS                   │
│  self.execute_rem_phase().await    ← ALWAYS RUNS                   │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                          Response                                   │
│  Both nrem_result AND rem_result present (incorrect)               │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Files Involved

| File | Role | Issue |
|------|------|-------|
| `crates/context-graph-mcp/src/handlers/tools/dream_dtos.rs` | DTO definitions | ✅ Correctly defines `skip_nrem`, `skip_rem` parameters |
| `crates/context-graph-mcp/src/handlers/tools/dream_tools.rs` | MCP handler | ❌ Logs flags but doesn't pass them to controller |
| `crates/context-graph-mcp/src/tools/definitions/dream.rs` | Tool schema | ✅ Correctly exposes parameters to MCP clients |
| `crates/context-graph-core/src/dream/controller.rs` | Dream orchestrator | ❌ `start_dream_cycle()` has no parameters, always runs both phases |

---

## Validation of Both Phases

The `dream_tools.rs:151-156` correctly validates that at least one phase must run:

```rust
// Validate phase selection
if request.skip_nrem && request.skip_rem {
    return self.tool_error_with_pulse(
        id,
        "Cannot skip both NREM and REM phases - at least one must run",
    );
}
```

This validation works - you cannot skip both phases. The issue is that skipping ONE phase doesn't work.

---

## Dry Run Behavior (Works Correctly)

Interestingly, the **dry run mode correctly handles skip flags** at lines 80-108:

```rust
if request.dry_run {
    let response = TriggerDreamResponse {
        // ...
        nrem_result: if !request.skip_nrem {
            Some(NremResult { /* ... */ })
        } else {
            None  // ✅ Correctly returns None when skipped
        },
        rem_result: if !request.skip_rem {
            Some(RemResult { /* ... */ })
        } else {
            None  // ✅ Correctly returns None when skipped
        },
        // ...
    };
}
```

This shows the **intended behavior** - dry run mode respects the skip flags in its response structure.

---

## Recommended Fix

### Option 1: Modify DreamController Interface

Add a new method or modify `start_dream_cycle()`:

```rust
// controller.rs
pub async fn start_dream_cycle_selective(
    &mut self,
    run_nrem: bool,
    run_rem: bool,
) -> CoreResult<DreamReport> {
    // ... existing setup ...

    let nrem_report = if run_nrem {
        Some(self.execute_nrem_phase().await?)
    } else {
        None
    };

    let rem_report = if run_rem {
        Some(self.execute_rem_phase().await?)
    } else {
        None
    };

    // ... rest of completion logic ...
}
```

### Option 2: Add Configuration Struct

```rust
pub struct DreamCycleConfig {
    pub run_nrem: bool,
    pub run_rem: bool,
    pub max_duration: Duration,
}

impl DreamController {
    pub async fn start_dream_cycle_with_config(
        &mut self,
        config: DreamCycleConfig,
    ) -> CoreResult<DreamReport> {
        // Respect config.run_nrem and config.run_rem
    }
}
```

### MCP Handler Update

```rust
// dream_tools.rs
let cycle_result = controller.start_dream_cycle_selective(
    !request.skip_nrem,  // run_nrem = true unless skip_nrem is true
    !request.skip_rem,   // run_rem = true unless skip_rem is true
).await;
```

---

## Impact Assessment

| Aspect | Impact |
|--------|--------|
| **Functionality** | Medium - Users cannot run NREM-only or REM-only cycles |
| **Performance** | Low - Both phases are fast (~20ms total in tests) |
| **Correctness** | Low - System works, just doesn't support selective phases |
| **API Contract** | Medium - Documented parameters don't work as expected |

---

## Constitution References

From `constitution.yaml` dream section:

```yaml
dream:
  phases:
    nrem:
      duration: "3min"
      purpose: "Hebbian learning replay"
    rem:
      duration: "2min"
      purpose: "Blind spot discovery via hyperbolic random walk"
```

The constitution doesn't mandate that phases must be skippable, but the MCP API offers this feature which should work.

---

## Conclusion

**Root Cause:** The `skip_nrem` and `skip_rem` flags are parsed and logged but never passed to `DreamController::start_dream_cycle()`. The controller method has no parameters and always executes both phases unconditionally.

**Classification:** Implementation gap / incomplete feature

**Evidence:**
1. Explicit TODO comment in code acknowledging the gap
2. Dry run mode correctly handles skip flags (showing intended behavior)
3. Validation prevents skipping both phases (partial implementation)

**Recommendation:** Implement selective phase execution in `DreamController` and update the MCP handler to pass the skip flags.

---

*Report generated by Context Graph Investigation - 2026-01-19*

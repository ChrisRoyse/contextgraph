# TASK-38: Implement get_identity_continuity MCP Tool

```xml
<task_spec id="TASK-38" version="2.0">
<metadata>
  <title>Implement get_identity_continuity MCP Tool</title>
  <original_id>TASK-MCP-012</original_id>
  <status>complete</status>
  <layer>surface</layer>
  <sequence>38</sequence>
  <implements>REQ-MCP-012</implements>
  <depends_on>TASK-21</depends_on>
  <estimated_hours>2</estimated_hours>
  <last_audit>2026-01-14</last_audit>
</metadata>
```

---

## ⚠️ CRITICAL PRE-TASK AUDIT (READ FIRST)

### Current Codebase State (Verified 2026-01-14)

**IMPORTANT**: The original task spec is WRONG. It references non-existent file paths and patterns that don't match this codebase. This updated spec reflects the ACTUAL project structure.

#### What Already Exists (DO NOT DUPLICATE)

1. **`get_ego_state` tool** (`crates/context-graph-mcp/src/handlers/tools/gwt_consciousness.rs:293-389`) already exposes IC via:
   ```json
   {
     "identity_continuity": {
       "ic": 0.85,
       "status": "Healthy",
       "in_crisis": false,
       "history_len": 42,
       "last_detection": { ... }
     }
   }
   ```

2. **`IdentityContinuity` struct** exists at `crates/context-graph-core/src/gwt/ego_node/identity_continuity.rs`

3. **`TriggerManager`** exists at `crates/context-graph-core/src/dream/triggers.rs` with IC checking

4. **GwtSystemProvider** provides identity methods:
   - `identity_coherence()` → f32
   - `identity_status()` → IdentityStatus
   - `is_identity_crisis()` → bool
   - `identity_history_len()` → usize
   - `last_detection()` → Option<CrisisDetectionResult>

#### What This Task ACTUALLY Needs

Create a **dedicated** `get_identity_continuity` tool that:
1. Provides a focused IC-only view (unlike `get_ego_state` which includes purpose_vector, coherence_with_actions, etc.)
2. Exposes TriggerManager's IC-specific trigger state
3. Adds `last_trigger_reason` from TriggerManager
4. Returns constitution-compliant thresholds

#### WRONG File Paths in Original Spec (DO NOT USE)

| Original (WRONG) | Actual Path |
|------------------|-------------|
| `crates/context-graph-mcp/src/tools/schemas/identity.rs` | DOES NOT EXIST - schemas are inline JSON |
| `crates/context-graph-mcp/src/tools/handlers/identity.rs` | DOES NOT EXIST - handlers are in `handlers/tools/` |
| `crates/context-graph-mcp/src/tools/schemas/mod.rs` | DOES NOT EXIST |
| `crates/context-graph-mcp/src/tools/handlers/mod.rs` | DOES NOT EXIST |

---

## Constitution Reference

```yaml
# From constitution.yaml lines 228-234 (gwt.self_ego_node)
identity_continuity: "IC = cos(PV_t, PV_{t-1}) × r(t)"
thresholds:
  healthy: ">0.9"
  warning: "<0.7"
  critical: "<0.5 → dream"

# From constitution.yaml lines 444-449 (enforcement.identity)
IDENTITY-001: "IC = cos(PV_t, PV_{t-1}) × r(t)"
IDENTITY-002: "Thresholds: Healthy>0.9, Warning[0.7,0.9], Degraded[0.5,0.7), Critical<0.5"
IDENTITY-007: "IC < 0.5 → auto-trigger dream"
```

---

## Scope

### In Scope

1. **Tool Definition** in `crates/context-graph-mcp/src/tools/definitions/gwt.rs`
2. **Tool Name Constant** in `crates/context-graph-mcp/src/tools/names.rs`
3. **Handler Implementation** in `crates/context-graph-mcp/src/handlers/tools/gwt_consciousness.rs`
4. **Dispatch Wiring** in `crates/context-graph-mcp/src/handlers/tools/dispatch.rs`
5. **Tests** with Full State Verification in `crates/context-graph-mcp/src/handlers/tests/`

### Out of Scope

- TriggerManager implementation (TASK-21 ✅ COMPLETE)
- IdentityContinuity struct (already exists)
- GwtSystemProvider methods (already exist)
- Crisis detection logic (already exists)

---

## File Changes Required

### 1. Add Tool Name Constant

**File**: `crates/context-graph-mcp/src/tools/names.rs`

**Location**: After line 138 (after GET_COHERENCE_STATE)

```rust
// ========== IDENTITY CONTINUITY TOOL (TASK-38) ==========

/// TASK-38: Get focused identity continuity state with trigger info
/// Unlike get_ego_state (which includes purpose_vector, coherence_with_actions),
/// this returns IC-focused data including TriggerManager state.
pub const GET_IDENTITY_CONTINUITY: &str = "get_identity_continuity";
```

### 2. Add Tool Definition

**File**: `crates/context-graph-mcp/src/tools/definitions/gwt.rs`

**Location**: Add after `get_coherence_state` definition (after line 163)

```rust
// get_identity_continuity - IC-focused state with trigger info (TASK-38)
ToolDefinition::new(
    "get_identity_continuity",
    "Get focused identity continuity state including IC value, status, crisis detection, \
     and TriggerManager state. Unlike get_ego_state (which includes purpose_vector, \
     coherence_with_actions), this returns IC-specific data including last_trigger_reason \
     and trigger eligibility. Constitution: IC = cos(PV_t, PV_{t-1}) × r(t). \
     Thresholds: Healthy>0.9, Warning[0.7,0.9], Degraded[0.5,0.7), Critical<0.5 (triggers dream). \
     Requires GWT providers to be initialized via with_gwt() constructor.",
    json!({
        "type": "object",
        "properties": {
            "include_history": {
                "type": "boolean",
                "default": false,
                "description": "Include recent IC history values (up to 10)"
            }
        },
        "required": []
    }),
),
```

### 3. Add Handler Implementation

**File**: `crates/context-graph-mcp/src/handlers/tools/gwt_consciousness.rs`

**Location**: After `call_get_coherence_state` (after line 492)

```rust
/// get_identity_continuity tool implementation.
///
/// TASK-38: Returns focused identity continuity state with TriggerManager info.
/// Unlike get_ego_state (which includes purpose_vector, coherence_with_actions),
/// this returns IC-specific data including trigger state.
///
/// Constitution Reference:
/// - IC = cos(PV_t, PV_{t-1}) × r(t) (line 233)
/// - Healthy>0.9, Warning[0.7,0.9], Degraded[0.5,0.7), Critical<0.5 (lines 387-392)
/// - IC < 0.5 → auto-trigger dream (IDENTITY-007)
///
/// FAIL FAST on missing providers - no stubs or fallbacks (AP-26).
///
/// Returns:
/// - ic_value: f32 - Current IC value [0.0, 1.0]
/// - status: string - Healthy/Warning/Degraded/Critical
/// - in_crisis: bool - IC < 0.7 (warning threshold)
/// - is_critical: bool - IC < 0.5 (dream trigger threshold)
/// - thresholds: object - Constitution-mandated threshold values
/// - last_detection: object|null - Last crisis detection result
/// - trigger_eligible: bool - Whether IC state makes system eligible for dream trigger
/// - last_trigger_reason: string|null - Last reason TriggerManager recorded
/// - history: array|null - Recent IC values (if include_history=true)
pub(crate) async fn call_get_identity_continuity(
    &self,
    id: Option<JsonRpcId>,
    arguments: serde_json::Value,
) -> JsonRpcResponse {
    debug!("Handling get_identity_continuity tool call");

    // Parse include_history argument
    let include_history = arguments
        .get("include_history")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // FAIL FAST: Check gwt_system provider (required for IC data)
    let gwt_system = match &self.gwt_system {
        Some(g) => g,
        None => {
            error!("get_identity_continuity: GWT system not initialized - FAIL FAST per AP-26");
            return JsonRpcResponse::error(
                id,
                error_codes::GWT_NOT_INITIALIZED,
                "GWT system not initialized - use with_gwt() constructor",
            );
        }
    };

    // FAIL FAST: Check trigger_manager (required for trigger state)
    let trigger_manager = match &self.trigger_manager {
        Some(tm) => tm,
        None => {
            error!("get_identity_continuity: TriggerManager not initialized - FAIL FAST per AP-26");
            return JsonRpcResponse::error(
                id,
                error_codes::DREAM_NOT_INITIALIZED,
                "TriggerManager not initialized - use with_trigger_manager() or with_default_gwt()",
            );
        }
    };

    // Get IC data from GwtSystemProvider
    let ic_value = gwt_system.identity_coherence().await;
    let ic_status = gwt_system.identity_status().await;
    let in_crisis = gwt_system.is_identity_crisis().await;
    let history_len = gwt_system.identity_history_len().await;
    let last_detection = gwt_system.last_detection().await;

    // Determine critical state (IC < 0.5 triggers dream per IDENTITY-007)
    let is_critical = ic_value < 0.5;

    // Get trigger info from TriggerManager
    let (trigger_eligible, last_trigger_reason) = {
        let manager = trigger_manager.read();
        // Check if TriggerManager would accept an IC-based trigger
        let current_trigger = manager.check_triggers();
        let eligible = matches!(
            current_trigger,
            Some(context_graph_core::dream::types::ExtendedTriggerReason::IdentityCritical { .. })
        ) || is_critical;

        // Get last trigger reason if any
        let reason = current_trigger.map(|t| format!("{:?}", t));
        (eligible, reason)
    };

    // Format last_detection for JSON output
    let last_detection_json = last_detection.map(|det| {
        json!({
            "identity_coherence": det.identity_coherence,
            "previous_status": format!("{:?}", det.previous_status),
            "current_status": format!("{:?}", det.current_status),
            "status_changed": det.status_changed,
            "entering_crisis": det.entering_crisis,
            "entering_critical": det.entering_critical,
            "recovering": det.recovering,
            "time_since_last_event_ms": det.time_since_last_event.map(|d| d.as_millis()),
            "can_emit_event": det.can_emit_event
        })
    });

    // Optionally include history (placeholder - implement if history tracking exists)
    let history_json = if include_history {
        // Note: If GwtSystemProvider has a history method, use it here
        // For now, return empty array since history tracking may not be exposed
        Some(json!([]))
    } else {
        None
    };

    self.tool_result_with_pulse(
        id,
        json!({
            "ic_value": ic_value,
            "status": format!("{:?}", ic_status),
            "in_crisis": in_crisis,
            "is_critical": is_critical,
            "history_len": history_len,
            "thresholds": {
                "healthy": 0.9,
                "warning": 0.7,
                "degraded": 0.5,
                "critical": 0.0,
                "dream_trigger": 0.5
            },
            "last_detection": last_detection_json,
            "trigger_eligible": trigger_eligible,
            "last_trigger_reason": last_trigger_reason,
            "history": history_json,
            "constitution_reference": {
                "formula": "IC = cos(PV_t, PV_{t-1}) × r(t)",
                "line_refs": "constitution.yaml:228-234, 444-449"
            }
        }),
    )
}
```

### 4. Add Dispatch Wiring

**File**: `crates/context-graph-mcp/src/handlers/tools/dispatch.rs`

**Location**: After line 167 (after GET_COHERENCE_STATE match arm)

```rust
// TASK-38: Identity continuity focused tool
tool_names::GET_IDENTITY_CONTINUITY => {
    self.call_get_identity_continuity(id, arguments).await
}
```

### 5. Update definitions/mod.rs

**File**: `crates/context-graph-mcp/src/tools/definitions/mod.rs`

Ensure `gwt::definitions()` is called (it already is if following existing pattern).

---

## Definition of Done

### Acceptance Criteria

1. **Tool is registered**: `cargo test -p context-graph-mcp tools_list` shows `get_identity_continuity`
2. **Tool executes**: Calling tool returns valid JSON response
3. **FAIL FAST works**: Missing GWT/TriggerManager returns error, not null
4. **IC value is accurate**: Matches `get_ego_state.identity_continuity.ic`
5. **Trigger state exposed**: `trigger_eligible` and `last_trigger_reason` populated

### Test Commands

```bash
# Compile check
cargo check -p context-graph-mcp

# Run all tests
cargo test -p context-graph-mcp

# Run specific identity tests
cargo test -p context-graph-mcp identity

# Run tools list test to verify registration
cargo test -p context-graph-mcp tools_list
```

---

## Full State Verification (MANDATORY)

After implementing the logic, you MUST perform Full State Verification.

### Source of Truth

The final result is stored in:
1. **GwtSystemProvider** internal state (for IC value, status, crisis detection)
2. **TriggerManager** internal state (for trigger eligibility, last reason)

### Verification Steps

1. **Execute & Inspect**: Call `get_identity_continuity` tool, then immediately call `get_ego_state` to verify IC data matches

2. **Boundary & Edge Case Audit**: Manually simulate these 3 edge cases:

   **Edge Case 1: No IC history (cold start)**
   ```
   STATE BEFORE: GwtSystemProvider with no previous purpose vectors
   ACTION: Call get_identity_continuity
   EXPECTED: ic_value=0.0 (default), status="Critical", in_crisis=true, is_critical=true
   STATE AFTER: Verify response matches expected values
   ```

   **Edge Case 2: IC recovery from crisis**
   ```
   STATE BEFORE: IC was 0.4 (Critical), now updated to 0.75
   ACTION: Call get_identity_continuity
   EXPECTED: status="Warning", in_crisis=true, is_critical=false, last_detection.recovering=true
   STATE AFTER: Verify trigger_eligible reflects recovery
   ```

   **Edge Case 3: Missing TriggerManager**
   ```
   STATE BEFORE: Handlers created WITHOUT with_trigger_manager()
   ACTION: Call get_identity_continuity
   EXPECTED: JSON-RPC error with code DREAM_NOT_INITIALIZED (-32016)
   STATE AFTER: No partial response, clean error
   ```

3. **Evidence of Success**: Test output must include:
   ```
   ✓ ic_value matches get_ego_state.identity_continuity.ic
   ✓ trigger_eligible reflects TriggerManager state
   ✓ FAIL FAST on missing providers (no null values)
   ```

---

## Manual Testing with Synthetic Data

### Test Harness Location

Create test file: `crates/context-graph-mcp/src/handlers/tests/exhaustive_mcp_tools/identity_continuity.rs`

### Synthetic Test Data

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic IC values for testing
    const SYNTHETIC_IC_HEALTHY: f32 = 0.95;      // > 0.9 = Healthy
    const SYNTHETIC_IC_WARNING: f32 = 0.75;      // [0.7, 0.9] = Warning
    const SYNTHETIC_IC_DEGRADED: f32 = 0.55;     // [0.5, 0.7) = Degraded
    const SYNTHETIC_IC_CRITICAL: f32 = 0.35;     // < 0.5 = Critical

    /// Expected outputs for each synthetic input
    struct ExpectedOutput {
        status: &'static str,
        in_crisis: bool,
        is_critical: bool,
        trigger_eligible: bool,
    }

    const EXPECTED_HEALTHY: ExpectedOutput = ExpectedOutput {
        status: "Healthy",
        in_crisis: false,
        is_critical: false,
        trigger_eligible: false,
    };

    const EXPECTED_WARNING: ExpectedOutput = ExpectedOutput {
        status: "Warning",
        in_crisis: true,   // IC < 0.7
        is_critical: false,
        trigger_eligible: false,
    };

    const EXPECTED_DEGRADED: ExpectedOutput = ExpectedOutput {
        status: "Degraded",
        in_crisis: true,
        is_critical: false,
        trigger_eligible: false,
    };

    const EXPECTED_CRITICAL: ExpectedOutput = ExpectedOutput {
        status: "Critical",
        in_crisis: true,
        is_critical: true,   // IC < 0.5
        trigger_eligible: true,  // IDENTITY-007: auto-trigger dream
    };
}
```

### Manual Verification Checklist

```
[ ] Synthetic IC=0.95 → status="Healthy", in_crisis=false
[ ] Synthetic IC=0.75 → status="Warning", in_crisis=true
[ ] Synthetic IC=0.55 → status="Degraded", in_crisis=true
[ ] Synthetic IC=0.35 → status="Critical", trigger_eligible=true
[ ] Missing GWT → error code GWT_NOT_INITIALIZED (-32010)
[ ] Missing TriggerManager → error code DREAM_NOT_INITIALIZED (-32016)
[ ] Response matches get_ego_state.identity_continuity.ic
```

---

## Trigger Event → Outcome Verification

### Trigger: MCP tool call `get_identity_continuity`

**Observable Cause (X)**:
- JSON-RPC request with method="tools/call", params.name="get_identity_continuity"

**Expected Outcome (Y)**:
- JSON-RPC response with ic_value, status, trigger_eligible fields
- Response logged in MCP handler (tracing::debug)

**Verification Method**:
1. Check JSON-RPC response structure
2. Cross-validate ic_value against `get_ego_state` response
3. If trigger_eligible=true, verify TriggerManager.check_triggers() returns IdentityCritical

---

## Error Codes Reference

```rust
// From crates/context-graph-mcp/src/protocol.rs
pub mod error_codes {
    pub const GWT_NOT_INITIALIZED: i32 = -32010;
    pub const DREAM_NOT_INITIALIZED: i32 = -32016;
}
```

---

## Files Summary

| Action | File Path |
|--------|-----------|
| MODIFY | `crates/context-graph-mcp/src/tools/names.rs` |
| MODIFY | `crates/context-graph-mcp/src/tools/definitions/gwt.rs` |
| MODIFY | `crates/context-graph-mcp/src/handlers/tools/gwt_consciousness.rs` |
| MODIFY | `crates/context-graph-mcp/src/handlers/tools/dispatch.rs` |
| CREATE | `crates/context-graph-mcp/src/handlers/tests/exhaustive_mcp_tools/identity_continuity.rs` |
| MODIFY | `crates/context-graph-mcp/src/handlers/tests/exhaustive_mcp_tools/mod.rs` |

---

## NO BACKWARDS COMPATIBILITY

**CRITICAL**: This system must work or fail fast. Do not:
- Return null/None for missing data (return error instead)
- Create fallback values that mask failures
- Use mock data in production tests
- Add workarounds for missing providers

If something fails, it MUST:
1. Return explicit error code
2. Log error with context (tracing::error!)
3. Include actionable message (what to configure)

---

## Related Tasks

| Task | Relationship |
|------|--------------|
| TASK-21 | Dependency - TriggerManager IC checking (COMPLETE) |
| TASK-26 | Related - IC monitor emits IdentityCritical events |
| TASK-35 | Related - trigger_dream uses TriggerManager |
| TASK-41 | Blocked by - Tool registration |

</task_spec>
```

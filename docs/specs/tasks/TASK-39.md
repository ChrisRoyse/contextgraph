# TASK-39: Implement get_kuramoto_state tool

```xml
<task_spec id="TASK-39" version="3.0">
<metadata>
  <title>Implement get_kuramoto_state MCP Tool</title>
  <original_id>TASK-MCP-013</original_id>
  <status>ready</status>
  <layer>surface</layer>
  <sequence>39</sequence>
  <implements><requirement_ref>REQ-MCP-013</requirement_ref></implements>
  <depends_on>TASK-12</depends_on>
  <estimated_hours>2</estimated_hours>
  <blocks>TASK-41</blocks>
</metadata>

<executive_summary>
## CRITICAL DISCOVERY - TASK MAY BE REDUNDANT

The MCP server ALREADY has `get_kuramoto_sync` tool which returns ALL the data that
`get_kuramoto_state` would return:
- is_running: Can be derived from state field (DORMANT = not running)
- phases: Already returned as 13-element array
- frequencies: Returned as `natural_freqs` (13-element array)
- coupling: Already returned
- order_parameter: Already returned as `r`
- mean_phase: Already returned as `psi`

**DECISION REQUIRED**: Either:
1. **Option A**: Mark task as N/A - functionality exists in `get_kuramoto_sync`
2. **Option B**: Implement `get_kuramoto_state` as thin wrapper with renamed fields for API consistency
3. **Option C**: Implement `get_kuramoto_state` with ADDITIONAL stepper status not in `get_kuramoto_sync`

This task document assumes **Option C** - implement with stepper-specific status that `get_kuramoto_sync` lacks.
</executive_summary>

<current_state>
## WHAT EXISTS (2026-01-14)

### 1. Existing Tool: get_kuramoto_sync (COMPLETE)
**Tool name**: `get_kuramoto_sync`
**Dispatch location**: `crates/context-graph-mcp/src/handlers/tools/dispatch.rs:76`
**Handler location**: `crates/context-graph-mcp/src/handlers/tools/gwt_consciousness.rs:207-275`

Returns:
```json
{
  "r": 0.85,                      // Order parameter [0,1]
  "psi": 3.14,                    // Mean phase [0, 2pi]
  "synchronization": 0.85,        // Same as r
  "state": "CONSCIOUS",           // DORMANT/FRAGMENTED/EMERGING/CONSCIOUS/HYPERSYNC
  "phases": [0.1, 0.2, ...],      // 13 oscillator phases
  "natural_freqs": [40, 8, ...],  // 13 natural frequencies (Hz)
  "coupling": 0.5,                // Coupling strength K
  "elapsed_seconds": 1.5,         // Time since creation
  "embedding_labels": [...],      // 13 embedding space names
  "thresholds": { ... }           // State transition thresholds
}
```

### 2. Existing Tool: get_coherence_state (COMPLETE - TASK-34)
**Tool name**: `get_coherence_state`
**Handler location**: `crates/context-graph-mcp/src/handlers/tools/gwt_consciousness.rs:407-493`

Returns high-level coherence summary (order_parameter, coherence_level, is_broadcasting, has_conflict).

### 3. KuramotoStepper Integration (COMPLETE - TASK-12)
**Stepper location**: `crates/context-graph-mcp/src/handlers/kuramoto_stepper.rs`
**Handlers integration**: `crates/context-graph-mcp/src/handlers/core/handlers.rs`

The stepper runs at 100Hz (10ms interval) and updates the KuramotoNetwork phases.
Methods available:
- `handlers.is_kuramoto_running()` - Check if stepper is running
- `handlers.start_kuramoto_stepper()` - Start the background task
- `handlers.stop_kuramoto_stepper()` - Stop the background task

### 4. What get_kuramoto_state ADDS Over get_kuramoto_sync
The key difference is **stepper status** - knowing if the background stepping task is running:
- `is_running`: Whether the KuramotoStepper background task is actively stepping
- This is NOT available in get_kuramoto_sync which only reads network state

This makes get_kuramoto_state valuable for debugging/monitoring the GWT system lifecycle.
</current_state>

<scope>
<in_scope>
1. Add `GET_KURAMOTO_STATE` constant to `tools/names.rs`
2. Add tool definition to `tools/definitions/gwt.rs`
3. Implement `call_get_kuramoto_state` handler in `handlers/tools/gwt_consciousness.rs`
4. Wire dispatch in `handlers/tools/dispatch.rs`
5. Return stepper running status + network state (phases, frequencies, coupling, r, psi)
6. Integration tests with Full State Verification
</in_scope>
<out_of_scope>
- Modifying KuramotoStepper implementation (TASK-12 complete)
- Modifying KuramotoNetwork implementation (TASK-11 complete)
- Creating new provider traits (use existing KuramotoProvider)
</out_of_scope>
</scope>

<implementation_plan>
## STEP 1: Add Tool Name Constant

**File**: `crates/context-graph-mcp/src/tools/names.rs`
**Location**: After line 138 (GET_IDENTITY_CONTINUITY)

```rust
// ========== KURAMOTO STATE TOOL (TASK-39) ==========

/// TASK-39: Get detailed Kuramoto network state including stepper status.
/// Unlike get_kuramoto_sync (network data only), this includes is_running
/// which indicates whether the KuramotoStepper background task is active.
/// Requires GWT providers to be initialized via with_gwt() constructor.
pub const GET_KURAMOTO_STATE: &str = "get_kuramoto_state";
```

## STEP 2: Add Tool Definition

**File**: `crates/context-graph-mcp/src/tools/definitions/gwt.rs`
**Location**: Add to the `definitions()` function's Vec

```rust
// get_kuramoto_state - Kuramoto state with stepper status (TASK-39)
ToolDefinition::new(
    "get_kuramoto_state",
    "Get detailed Kuramoto oscillator network state including stepper running status. \
     Returns is_running (stepper active), phases (13 oscillator phases), frequencies \
     (13 natural frequencies), coupling (K), order_parameter (r), mean_phase (psi). \
     Unlike get_kuramoto_sync, includes stepper lifecycle status. \
     Requires GWT providers to be initialized via with_gwt() constructor.",
    json!({
        "type": "object",
        "properties": {},
        "required": []
    }),
),
```

## STEP 3: Implement Handler

**File**: `crates/context-graph-mcp/src/handlers/tools/gwt_consciousness.rs`
**Location**: Add after `call_get_identity_continuity` (after line 574)

```rust
/// get_kuramoto_state tool implementation.
///
/// TASK-39: Returns Kuramoto network state WITH stepper running status.
/// Unlike get_kuramoto_sync (network data only), this includes is_running
/// which indicates whether the background stepping task is active.
///
/// FAIL FAST on missing GWT components - no stubs or fallbacks.
///
/// Returns:
/// - is_running: Whether KuramotoStepper background task is running
/// - phases: All 13 oscillator phases [0, 2pi]
/// - frequencies: All 13 natural frequencies (Hz per constitution.yaml line 221)
/// - coupling: Coupling strength K
/// - order_parameter: r in [0, 1]
/// - mean_phase: psi in [0, 2pi]
pub(crate) async fn call_get_kuramoto_state(
    &self,
    id: Option<JsonRpcId>,
) -> JsonRpcResponse {
    debug!("Handling get_kuramoto_state tool call");

    // FAIL FAST: Check kuramoto provider
    let kuramoto = match &self.kuramoto_network {
        Some(k) => k,
        None => {
            error!("get_kuramoto_state: Kuramoto network not initialized");
            return JsonRpcResponse::error(
                id,
                error_codes::GWT_NOT_INITIALIZED,
                "Kuramoto network not initialized - use with_gwt() constructor",
            );
        }
    };

    // Get stepper running status (unique to get_kuramoto_state)
    let is_running = self.is_kuramoto_running();

    // Acquire read lock on network
    let network = kuramoto.read();

    // Get order parameter (r, psi)
    let (r, psi) = network.order_parameter();

    // Get all 13 oscillator phases
    let phases = network.phases();

    // Get all 13 natural frequencies
    let frequencies = network.natural_frequencies();

    // Get coupling strength K
    let coupling = network.coupling_strength();

    // Return complete state
    self.tool_result_with_pulse(
        id,
        json!({
            "is_running": is_running,
            "phases": phases.to_vec(),
            "frequencies": frequencies.to_vec(),
            "coupling": coupling,
            "order_parameter": r,
            "mean_phase": psi
        }),
    )
}
```

## STEP 4: Wire Dispatch

**File**: `crates/context-graph-mcp/src/handlers/tools/dispatch.rs`
**Location**: Add case before the `_ =>` default match arm (around line 172)

```rust
// TASK-39: Kuramoto state with stepper status
tool_names::GET_KURAMOTO_STATE => self.call_get_kuramoto_state(id).await,
```

## STEP 5: Update gwt.rs Definitions Count Comment

**File**: `crates/context-graph-mcp/src/tools/definitions/gwt.rs`
**Line 9**: Update comment from "8 tools" to "9 tools"
</implementation_plan>

<files_to_modify>
| File | Change |
|------|--------|
| `crates/context-graph-mcp/src/tools/names.rs` | Add GET_KURAMOTO_STATE constant |
| `crates/context-graph-mcp/src/tools/definitions/gwt.rs` | Add tool definition |
| `crates/context-graph-mcp/src/handlers/tools/gwt_consciousness.rs` | Add call_get_kuramoto_state handler |
| `crates/context-graph-mcp/src/handlers/tools/dispatch.rs` | Wire GET_KURAMOTO_STATE dispatch |
</files_to_modify>

<definition_of_done>
## Acceptance Criteria

### AC-1: Tool Name Constant Exists
`crates/context-graph-mcp/src/tools/names.rs` contains `GET_KURAMOTO_STATE` constant.

### AC-2: Tool Definition Registered
`cargo run -p context-graph-mcp -- tools/list` includes `get_kuramoto_state` with proper schema.

### AC-3: Handler Dispatches Correctly
Calling `tools/call` with `name: "get_kuramoto_state"` executes the handler.

### AC-4: Response Schema Matches
Response includes exactly these fields:
- `is_running`: boolean
- `phases`: array of 13 floats
- `frequencies`: array of 13 floats
- `coupling`: float
- `order_parameter`: float
- `mean_phase`: float

### AC-5: Phases Have 13 Elements
`phases.length == 13` (constitution.yaml AP-25: exactly 13 oscillators)

### AC-6: Frequencies Match Constitution
`frequencies` contains values matching constitution.yaml line 221:
E1: 40γ, E2: 8α, E3: 8α, E4: 8α, E5: 25β, E6: 4θ, E7: 25β, E8: 12αβ, E9: 80γ+, E10: 40γ, E11: 15β, E12: 60γ+, E13: 4θ

### AC-7: Tests Pass
```bash
cargo test -p context-graph-mcp kuramoto_state -- --nocapture
# Must pass all tests
```

### AC-8: Compilation Clean
```bash
cargo check -p context-graph-mcp
# No errors
```
</definition_of_done>

<full_state_verification>
## MANDATORY: Full State Verification Protocol

After implementing, you MUST verify the tool works end-to-end.

### 1. Source of Truth Definition
The sources of truth are:
- **is_running**: `handlers.is_kuramoto_running()` method
- **phases**: `kuramoto_network.read().phases()` returns `[f64; 13]`
- **frequencies**: `kuramoto_network.read().natural_frequencies()` returns `[f64; 13]`
- **coupling**: `kuramoto_network.read().coupling_strength()` returns `f64`
- **order_parameter**: `kuramoto_network.read().order_parameter().0` returns `f64`
- **mean_phase**: `kuramoto_network.read().order_parameter().1` returns `f64`

### 2. Execute & Inspect Protocol

Create test in `crates/context-graph-mcp/src/handlers/tests/full_state_verification_gwt/kuramoto_tests.rs`:

```rust
#[tokio::test]
async fn test_get_kuramoto_state_returns_real_data_with_stepper_status() {
    // === SETUP ===
    let handlers = create_handlers_with_gwt();

    // === STATE BEFORE ===
    let is_running_direct = handlers.is_kuramoto_running();
    println!("[FSV] BEFORE: is_running (direct) = {}", is_running_direct);

    // === EXECUTE: Call MCP tool ===
    let request = make_tool_call_request(tool_names::GET_KURAMOTO_STATE, None);
    let response = handlers.dispatch(request).await;

    // === VERIFY: Response structure ===
    let response_json = serde_json::to_value(&response).expect("serialize");
    assert!(response_json.get("error").is_none(), "Expected success");

    let content = extract_tool_content(&response_json).expect("content");

    // FSV-1: is_running field exists and matches direct query
    let is_running_mcp = content["is_running"].as_bool().expect("is_running must be bool");
    assert_eq!(is_running_mcp, is_running_direct, "is_running must match direct query");
    println!("[FSV] is_running: MCP={}, Direct={}", is_running_mcp, is_running_direct);

    // FSV-2: phases has exactly 13 elements
    let phases = content["phases"].as_array().expect("phases array");
    assert_eq!(phases.len(), 13, "Must have 13 phases");
    println!("[FSV] phases count: {}", phases.len());

    // FSV-3: frequencies has exactly 13 elements
    let freqs = content["frequencies"].as_array().expect("frequencies array");
    assert_eq!(freqs.len(), 13, "Must have 13 frequencies");
    println!("[FSV] frequencies count: {}", freqs.len());

    // FSV-4: order_parameter in [0, 1]
    let r = content["order_parameter"].as_f64().expect("order_parameter");
    assert!((0.0..=1.0).contains(&r), "r must be in [0,1]");
    println!("[FSV] order_parameter: {:.4}", r);

    // FSV-5: mean_phase in [0, 2pi]
    let psi = content["mean_phase"].as_f64().expect("mean_phase");
    assert!((0.0..=std::f64::consts::TAU).contains(&psi), "psi in [0, 2pi]");
    println!("[FSV] mean_phase: {:.4}", psi);

    // FSV-6: coupling is positive
    let coupling = content["coupling"].as_f64().expect("coupling");
    assert!(coupling > 0.0, "coupling must be positive");
    println!("[FSV] coupling: {:.4}", coupling);

    // === EVIDENCE ===
    println!("[FSV] TASK-39 get_kuramoto_state VERIFIED");
    println!("  is_running: {}", is_running_mcp);
    println!("  phases[0..3]: {:?}", &phases[0..3]);
    println!("  frequencies[0..3]: {:?}", &freqs[0..3]);
    println!("  r={:.4}, psi={:.4}, K={:.4}", r, psi, coupling);
}
```

### 3. Edge Case Audit

**Edge Case 1: GWT Not Initialized**
```rust
#[tokio::test]
async fn test_get_kuramoto_state_fails_fast_without_gwt() {
    // Create handlers WITHOUT GWT
    let handlers = Handlers::new(graph, johari, system_monitor, utl);

    let request = make_tool_call_request(tool_names::GET_KURAMOTO_STATE, None);
    let response = handlers.dispatch(request).await;

    let response_json = serde_json::to_value(&response).expect("serialize");
    let error = response_json.get("error").expect("must have error");
    let code = error["code"].as_i64().expect("error code");

    // FAIL FAST: Must return GWT_NOT_INITIALIZED error
    assert_eq!(code, error_codes::GWT_NOT_INITIALIZED as i64);
    println!("[FSV] Edge case: GWT not initialized → error code {}", code);
}
```

**Edge Case 2: Stepper Started vs Not Started**
```rust
#[tokio::test]
async fn test_get_kuramoto_state_reflects_stepper_lifecycle() {
    let mut handlers = create_handlers_with_default_gwt();

    // BEFORE: Stepper not started
    let req1 = make_tool_call_request(tool_names::GET_KURAMOTO_STATE, None);
    let resp1 = handlers.dispatch(req1).await;
    let content1 = extract_tool_content(&serde_json::to_value(&resp1).unwrap()).unwrap();
    let is_running_before = content1["is_running"].as_bool().unwrap();
    println!("[FSV] Before start: is_running = {}", is_running_before);

    // START stepper
    handlers.start_kuramoto_stepper().expect("start");

    // AFTER: Stepper running
    let req2 = make_tool_call_request(tool_names::GET_KURAMOTO_STATE, None);
    let resp2 = handlers.dispatch(req2).await;
    let content2 = extract_tool_content(&serde_json::to_value(&resp2).unwrap()).unwrap();
    let is_running_after = content2["is_running"].as_bool().unwrap();
    println!("[FSV] After start: is_running = {}", is_running_after);

    assert!(is_running_after, "Stepper must be running after start");

    // STOP stepper
    handlers.stop_kuramoto_stepper().await.expect("stop");

    // AFTER STOP: Stepper not running
    let req3 = make_tool_call_request(tool_names::GET_KURAMOTO_STATE, None);
    let resp3 = handlers.dispatch(req3).await;
    let content3 = extract_tool_content(&serde_json::to_value(&resp3).unwrap()).unwrap();
    let is_running_stopped = content3["is_running"].as_bool().unwrap();
    println!("[FSV] After stop: is_running = {}", is_running_stopped);

    assert!(!is_running_stopped, "Stepper must not be running after stop");
}
```

**Edge Case 3: Validate 13 Frequencies Match Constitution**
```rust
#[tokio::test]
async fn test_get_kuramoto_state_frequencies_match_constitution() {
    // Constitution.yaml line 221:
    // E1:40γ, E2:8α, E3:8α, E4:8α, E5:25β, E6:4θ, E7:25β, E8:12αβ, E9:80γ+, E10:40γ, E11:15β, E12:60γ+, E13:4θ
    let expected_freqs: [f64; 13] = [40.0, 8.0, 8.0, 8.0, 25.0, 4.0, 25.0, 12.0, 80.0, 40.0, 15.0, 60.0, 4.0];

    let handlers = create_handlers_with_gwt();
    let request = make_tool_call_request(tool_names::GET_KURAMOTO_STATE, None);
    let response = handlers.dispatch(request).await;

    let response_json = serde_json::to_value(&response).unwrap();
    let content = extract_tool_content(&response_json).unwrap();
    let freqs = content["frequencies"].as_array().unwrap();

    for (i, (actual, expected)) in freqs.iter().zip(expected_freqs.iter()).enumerate() {
        let actual_f = actual.as_f64().unwrap();
        assert!(
            (actual_f - expected).abs() < 0.001,
            "Frequency[{}] mismatch: got {}, expected {}",
            i, actual_f, expected
        );
    }
    println!("[FSV] All 13 frequencies match constitution.yaml");
}
```

### 4. Manual Verification Steps

After implementation, run these commands:

```bash
# 1. Compile check
cargo check -p context-graph-mcp

# 2. Run all kuramoto-related tests
cargo test -p context-graph-mcp kuramoto -- --nocapture

# 3. Run specific FSV test
cargo test -p context-graph-mcp test_get_kuramoto_state_returns_real_data -- --nocapture

# 4. Verify tool appears in tools/list
cargo run -p context-graph-mcp 2>/dev/null << 'EOF'
{"jsonrpc":"2.0","id":1,"method":"tools/list"}
EOF
# Must see "get_kuramoto_state" in response

# 5. Call the tool directly
cargo run -p context-graph-mcp 2>/dev/null << 'EOF'
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_kuramoto_state"}}
EOF
# Must see response with is_running, phases, frequencies, coupling, order_parameter, mean_phase
```
</full_state_verification>

<test_commands>
```bash
# Run all kuramoto tests
cargo test -p context-graph-mcp kuramoto -- --nocapture

# Run specific tests
cargo test -p context-graph-mcp test_get_kuramoto_state -- --nocapture
cargo test -p context-graph-mcp test_get_kuramoto_sync -- --nocapture

# Compile check
cargo check -p context-graph-mcp

# Run workspace tests
cargo test --workspace
```
</test_commands>

<constitution_compliance>
## Constitution Requirements

| Rule | Requirement | How Satisfied |
|------|-------------|---------------|
| AP-25 | Kuramoto must have exactly 13 oscillators | phases.len() == 13, frequencies.len() == 13 |
| GWT-006 | KuramotoStepper wired to MCP lifecycle | is_running reflects stepper state |
| AP-14 | No .unwrap() in library code | Uses match/? for error handling |
| ARCH-06 | All memory ops through MCP tools | Tool reads via KuramotoProvider |
</constitution_compliance>

<anti_patterns>
## FORBIDDEN Actions

1. **DO NOT** duplicate get_kuramoto_sync functionality without adding value
2. **DO NOT** return mock/stub data - use real KuramotoProvider
3. **DO NOT** swallow errors - fail fast with GWT_NOT_INITIALIZED
4. **DO NOT** use .unwrap() - use ? or match with proper error handling
5. **DO NOT** create workarounds if something doesn't work - fix root cause
6. **DO NOT** skip is_running field - this is what differentiates from get_kuramoto_sync
7. **DO NOT** modify existing get_kuramoto_sync - implement new tool
</anti_patterns>

<dependencies>
## Dependency Status

| Dependency | Status | Evidence |
|------------|--------|----------|
| TASK-12 | COMPLETE | KuramotoStepper wired to server lifecycle |
| TASK-11 | COMPLETE | KuramotoNetwork with 13 oscillators |
| TASK-10 | COMPLETE | KURAMOTO_N=13 constant |

## This Task Unblocks

| Task | Description |
|------|-------------|
| TASK-41 | Add tool registration to MCP server |
</dependencies>

<quick_reference>
## File Locations

| Component | Path |
|-----------|------|
| Tool names | `crates/context-graph-mcp/src/tools/names.rs` |
| Tool definitions | `crates/context-graph-mcp/src/tools/definitions/gwt.rs` |
| Handler impl | `crates/context-graph-mcp/src/handlers/tools/gwt_consciousness.rs` |
| Dispatch | `crates/context-graph-mcp/src/handlers/tools/dispatch.rs` |
| Stepper | `crates/context-graph-mcp/src/handlers/kuramoto_stepper.rs` |
| Provider trait | `crates/context-graph-mcp/src/handlers/gwt_traits.rs` |
| Provider impl | `crates/context-graph-mcp/src/handlers/gwt_providers.rs` |
| Kuramoto tests | `crates/context-graph-mcp/src/handlers/tests/full_state_verification_gwt/kuramoto_tests.rs` |

## Expected Output Schema
```json
{
  "is_running": true,
  "phases": [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3],
  "frequencies": [40.0, 8.0, 8.0, 8.0, 25.0, 4.0, 25.0, 12.0, 80.0, 40.0, 15.0, 60.0, 4.0],
  "coupling": 0.5,
  "order_parameter": 0.85,
  "mean_phase": 3.14
}
```
</quick_reference>
</task_spec>
```

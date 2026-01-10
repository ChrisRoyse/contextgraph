# TASK-INTEG-003: Consciousness MCP Method Integration

```xml
<task_spec id="TASK-INTEG-003" version="5.0">
<metadata>
  <title>Integrate Consciousness JSON-RPC Methods with Existing GWT Tools</title>
  <status>DONE</status>
  <layer>integration</layer>
  <sequence>23</sequence>
  <implements>
    <requirement_ref>REQ-MCP-CONSCIOUSNESS-01</requirement_ref>
  </implements>
  <depends_on>
    <task_ref status="DONE">TASK-GWT-001</task_ref>
    <task_ref status="DONE">TASK-INTEG-001</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_hours>4-8</estimated_hours>
  <last_audit>2026-01-10</last_audit>
</metadata>

<critical_context>
## EXISTING INFRASTRUCTURE (Verified 2026-01-10)

### GWT TOOLS ALREADY FULLY IMPLEMENTED

The consciousness/GWT functionality is **100% COMPLETE** as MCP tools. This task only adds
JSON-RPC method aliases to enable hook integration per the MCP spec (docs2/refactor/08-MCP-TOOLS.md).

| Tool Name | Location | Lines | Status |
|-----------|----------|-------|--------|
| `get_consciousness_state` | handlers/tools.rs | 960-1113 | **DONE** |
| `get_kuramoto_sync` | handlers/tools.rs | 1133-1201 | **DONE** |
| `get_workspace_status` | handlers/tools.rs | 1203+ | **DONE** |
| `get_ego_state` | handlers/tools.rs | ~1300 | **DONE** |
| `trigger_workspace_broadcast` | handlers/tools.rs | ~1400 | **DONE** |
| `adjust_coupling` | handlers/tools.rs | ~1500 | **DONE** |

### GWT PROVIDER INFRASTRUCTURE (TASK-GWT-001 COMPLETE)

| Component | Location | Purpose |
|-----------|----------|---------|
| KuramotoProvider trait | handlers/gwt_traits.rs:28-75 | Kuramoto oscillator interface |
| GwtSystemProvider trait | handlers/gwt_traits.rs:81-120 | Consciousness computation interface |
| WorkspaceProvider trait | handlers/gwt_traits.rs:126-154 | Global workspace interface |
| MetaCognitiveProvider trait | handlers/gwt_traits.rs:159-184 | Meta-cognitive loop interface |
| SelfEgoProvider trait | handlers/gwt_traits.rs:188-203 | Self-ego node interface |
| KuramotoProviderImpl | handlers/gwt_providers.rs:42-135 | Real KuramotoNetwork wrapper |
| GwtSystemProviderImpl | handlers/gwt_providers.rs:143-217 | Real ConsciousnessCalculator wrapper |
| WorkspaceProviderImpl | handlers/gwt_providers.rs:225-288 | Real GlobalWorkspace wrapper |
| MetaCognitiveProviderImpl | handlers/gwt_providers.rs:296-350 | Real MetaCognitiveLoop wrapper |
| SelfEgoProviderImpl | handlers/gwt_providers.rs:358-431 | Real SelfEgoNode wrapper |

### CORE GWT COMPONENTS (context-graph-core/src/gwt/)

| File | Purpose |
|------|---------|
| consciousness.rs | ConsciousnessCalculator - C(t) = I(t) x R(t) x D(t) |
| state_machine.rs | StateMachineManager - 5 states (DORMANT/FRAGMENTED/EMERGING/CONSCIOUS/HYPERSYNC) |
| workspace.rs | GlobalWorkspace - Winner-take-all memory selection |
| meta_cognitive.rs | MetaCognitiveLoop - Learning rate modulation |
| ego_node.rs | SelfEgoNode + IdentityContinuity |
| mod.rs | Module exports |

### KURAMOTO NETWORK (context-graph-utl/src/phase/)

| Component | Description |
|-----------|-------------|
| KuramotoNetwork | 13 coupled oscillators (one per embedder) |
| Natural frequencies | E1=40Hz, E2-4=8Hz, E5=25Hz, E6=4Hz, E7=25Hz, E8=12Hz, E9=80Hz, E10=40Hz, E11=15Hz, E12=60Hz, E13=4Hz |
| Coupling strength K | Range [0, 10], default configurable |
| Order parameter r | Synchronization level in [0, 1] |

### EXISTING TOOL_NAMES CONSTANTS (tools.rs:1046-1066)

```rust
pub mod tool_names {
    pub const INJECT_CONTEXT: &str = "inject_context";
    pub const STORE_MEMORY: &str = "store_memory";
    pub const GET_MEMETIC_STATUS: &str = "get_memetic_status";
    pub const GET_GRAPH_MANIFEST: &str = "get_graph_manifest";
    pub const SEARCH_GRAPH: &str = "search_graph";
    pub const UTL_STATUS: &str = "utl_status";
    pub const GET_CONSCIOUSNESS_STATE: &str = "get_consciousness_state";
    pub const GET_KURAMOTO_SYNC: &str = "get_kuramoto_sync";
    pub const GET_WORKSPACE_STATUS: &str = "get_workspace_status";
    pub const GET_EGO_STATE: &str = "get_ego_state";
    pub const TRIGGER_WORKSPACE_BROADCAST: &str = "trigger_workspace_broadcast";
    pub const ADJUST_COUPLING: &str = "adjust_coupling";
    // ... more tools
}
```

### EXISTING PROTOCOL CONSTANTS (protocol.rs:335-348)

```rust
// GWT/Consciousness operations (TASK-GWT-001)
pub const GWT_KURAMOTO_STATUS: &str = "gwt/kuramoto_status";
pub const GWT_CONSCIOUSNESS_LEVEL: &str = "gwt/consciousness_level";
pub const GWT_WORKSPACE_STATUS: &str = "gwt/workspace_status";
pub const GWT_STATE_STATUS: &str = "gwt/state_status";
pub const GWT_META_COGNITIVE_STATUS: &str = "gwt/meta_cognitive_status";
pub const GWT_SELF_EGO_STATUS: &str = "gwt/self_ego_status";
```
</critical_context>

<objective>
**ADD JSON-RPC method dispatch routes** for MCP spec-defined consciousness methods:

1. Add `consciousness/get_state` method -> delegates to existing `call_get_consciousness_state`
2. Add `consciousness/sync_level` method -> delegates to existing `call_get_kuramoto_sync`
3. Add protocol constants for these methods
4. Add dispatch cases in core.rs
5. Write Full State Verification tests

**THIS IS NOT A CREATION TASK** - all logic already exists. This is pure wiring.
</objective>

<rationale>
Per docs2/refactor/08-MCP-TOOLS.md Section 5:
- `consciousness/get_state` is triggered by SessionStart hooks
- `consciousness/sync_level` is triggered by Notification hooks (periodic health checks)
- Current tools work via tools/call but hooks need direct JSON-RPC method access
</rationale>

<architecture_constraints>
## From constitution.yaml (MUST NOT VIOLATE)

- **AP-007 FAIL FAST**: All errors are FATAL. No fallbacks. No `unwrap_or_default()`. No mock data.
- **ARCH-01**: TeleologicalArray is atomic - all 13 embeddings stored/retrieved together
- **ARCH-02**: Apples-to-apples comparison - E1 compares with E1 ONLY
- **Kuramoto r thresholds** (lines 394-408):
  - DORMANT: r &lt; 0.3
  - FRAGMENTED: 0.3 &lt;= r &lt; 0.5
  - EMERGING: 0.5 &lt;= r &lt; 0.8
  - CONSCIOUS: 0.8 &lt;= r &lt;= 0.95
  - HYPERSYNC: r &gt; 0.95

## Error Codes (protocol.rs)

| Code | Name | When |
|------|------|------|
| -32062 | CONSCIOUSNESS_COMPUTATION_FAILED | ConsciousnessCalculator error |
| -32063 | GWT_NOT_INITIALIZED | GWT providers not wired |
</architecture_constraints>

<implementation_requirements>
## 1. ADD PROTOCOL CONSTANTS (protocol.rs)

Location: `crates/context-graph-mcp/src/protocol.rs` after line 348

```rust
// Consciousness JSON-RPC methods (TASK-INTEG-003)
/// Get full consciousness state (GWT + Kuramoto + Workspace + Identity)
pub const CONSCIOUSNESS_GET_STATE: &amp;str = "consciousness/get_state";
/// Get lightweight sync level for health checks
pub const CONSCIOUSNESS_SYNC_LEVEL: &amp;str = "consciousness/sync_level";
```

## 2. ADD DISPATCH ROUTES (core.rs)

Location: `crates/context-graph-mcp/src/handlers/core.rs` in dispatch_request() match block

Find the method dispatch section (look for pattern like `methods::PURPOSE_QUERY =>`).
Add after existing method dispatches:

```rust
// Consciousness methods (TASK-INTEG-003)
methods::CONSCIOUSNESS_GET_STATE => {
    // Delegate to existing get_consciousness_state tool implementation
    self.call_get_consciousness_state(request.id).await
}
methods::CONSCIOUSNESS_SYNC_LEVEL => {
    // Delegate to existing get_kuramoto_sync tool implementation
    self.call_get_kuramoto_sync(request.id).await
}
```

## 3. NO NEW FILES REQUIRED

**DO NOT** create `handlers/consciousness.rs` - all logic exists in:
- `handlers/tools.rs:960-1201` (tool implementations)
- `handlers/gwt_traits.rs` (provider traits)
- `handlers/gwt_providers.rs` (real implementations)

## 4. VERIFICATION COMMANDS

```bash
# Run existing GWT tests to ensure nothing breaks
cargo test -p context-graph-mcp handlers::tests::gwt -- --nocapture

# Run tools tests
cargo test -p context-graph-mcp handlers::tests::tools_call -- --nocapture

# Run new dispatch tests after implementation
cargo test -p context-graph-mcp handlers::tests::consciousness_dispatch -- --nocapture
```
</implementation_requirements>

<full_state_verification status="REQUIRED">
## Source of Truth

| Data | Location | Verification Method |
|------|----------|---------------------|
| Kuramoto r (sync level) | KuramotoNetwork in memory | kuramoto.read().order_parameter() |
| Consciousness state | StateMachineManager | gwt_system.current_state() |
| Workspace active memory | GlobalWorkspace | workspace.get_active_memory() |
| Identity coherence | SelfEgoNode | self_ego.identity_coherence() |

## Execute &amp; Inspect Protocol

After calling `consciousness/get_state`:

1. **Read the source of truth** directly via provider traits:
   ```rust
   let (r, _psi) = kuramoto.read().order_parameter();
   let state = gwt_system.current_state();
   let active = workspace.read().await.get_active_memory();
   let coherence = self_ego.read().await.identity_coherence();
   ```

2. **Compare response to source**:
   - Response `r` must equal `kuramoto.order_parameter().0`
   - Response `state` must equal `ConsciousnessState::from_level(r as f32).name()`
   - Response `workspace.active_memory` must equal `workspace.get_active_memory()`
   - Response `identity.coherence` must equal `self_ego.identity_coherence()`

3. **Log evidence**:
   ```
   [FSV] consciousness/get_state verification:
   [FSV]   Kuramoto r: source=0.847, response=0.847 OK
   [FSV]   State: source=CONSCIOUS, response=CONSCIOUS OK
   [FSV]   Workspace: source=Some(uuid), response=Some(uuid) OK
   [FSV]   Identity: source=0.92, response=0.92 OK
   ```

## Boundary &amp; Edge Case Audit

### Edge Case 1: GWT Not Initialized
**Input**: Call `consciousness/get_state` when Handlers created WITHOUT `with_default_gwt()`
**Expected**: Error code -32063 (GWT_NOT_INITIALIZED)
**Before State**: kuramoto_network = None
**After State**: kuramoto_network = None (unchanged)
**Evidence**: Response contains `{"error": {"code": -32063, "message": "...not initialized..."}}`

### Edge Case 2: Synchronized Network (r >= 0.8)
**Setup**: Use `KuramotoProviderImpl::synchronized()` which sets all phases to 0
**Input**: Call `consciousness/sync_level`
**Expected**: r > 0.99, state = "CONSCIOUS" or "HYPERSYNC"
**Before State**: phases = [0.0; 13]
**After State**: phases = [0.0; 13] (read-only call)
**Evidence**: Response contains `{"sync_level": 0.99+, "state": "CONSCIOUS"|"HYPERSYNC"}`

### Edge Case 3: Fragmented Network (r &lt; 0.5)
**Setup**: Use `KuramotoProviderImpl::incoherent()` with random phases
**Input**: Call `consciousness/sync_level`
**Expected**: r &lt; 0.1, state = "DORMANT" or "FRAGMENTED"
**Before State**: phases = random distribution
**After State**: phases unchanged (read-only call)
**Evidence**: Response contains `{"sync_level": &lt;0.5, "state": "DORMANT"|"FRAGMENTED"}`

### Edge Case 4: Hypersync Warning (r > 0.95)
**Input**: Network with very high coupling (K=10) after evolution
**Expected**: r > 0.95, state = "HYPERSYNC"
**Before State**: K = 10, phases evolving toward sync
**After State**: Same state (read-only)
**Evidence**: Response contains `{"state": "HYPERSYNC"}` - warning condition
</full_state_verification>

<manual_testing_protocol>
## Synthetic Test Data

### Test 1: Fresh Handler (No GWT)
**Setup**: `Handlers::new(...)` without GWT providers
**Command**: `{"jsonrpc":"2.0","id":1,"method":"consciousness/get_state","params":{}}`
**Expected Output**:
```json
{"jsonrpc":"2.0","id":1,"error":{"code":-32063,"message":"Kuramoto network not initialized - use with_gwt() constructor"}}
```
**Verification**: Check error code is exactly -32063

### Test 2: Initialized Handler (With GWT)
**Setup**: `Handlers::with_default_gwt(...)` creates all real providers
**Command**: `{"jsonrpc":"2.0","id":1,"method":"consciousness/get_state","params":{}}`
**Expected Output Structure**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "C": "[f32 in [0,1]]",
    "r": "[f64 in [0,1]]",
    "psi": "[f64 in [0, 2*PI]]",
    "meta_score": "[f32]",
    "differentiation": "[f32]",
    "integration": "[f32]",
    "reflection": "[f32]",
    "state": "DORMANT|FRAGMENTED|EMERGING|CONSCIOUS|HYPERSYNC",
    "gwt_state": "...",
    "time_in_state_ms": "[u128]",
    "workspace": {
      "active_memory": "null|[uuid string]",
      "is_broadcasting": "[bool]",
      "has_conflict": "[bool]",
      "coherence_threshold": 0.8
    },
    "identity": {
      "coherence": "[f32]",
      "status": "Healthy|Warning|Critical",
      "trajectory_length": "[usize]",
      "purpose_vector": "[[13 f32 values]]"
    },
    "component_analysis": {
      "integration_sufficient": "[bool]",
      "reflection_sufficient": "[bool]",
      "differentiation_sufficient": "[bool]",
      "limiting_factor": "None|Integration|Reflection|Differentiation"
    }
  }
}
```
**Verification**:
1. `r` is between 0.0 and 1.0
2. `psi` is between 0.0 and 2*PI
3. `workspace.coherence_threshold` is exactly 0.8
4. `identity.purpose_vector` has exactly 13 elements
5. `state` matches `ConsciousnessState::from_level(r as f32).name()`

### Test 3: Sync Level (Lightweight Check)
**Setup**: `Handlers::with_default_gwt(...)`
**Command**: `{"jsonrpc":"2.0","id":2,"method":"consciousness/sync_level","params":{}}`
**Expected Output Structure**:
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "r": "[f64]",
    "psi": "[f64]",
    "synchronization": "[f64]",
    "state": "...",
    "phases": "[[13 f64 values]]",
    "natural_freqs": "[[13 f64 values]]",
    "coupling": "[f64]",
    "elapsed_seconds": "[f64]",
    "embedding_labels": ["E1_semantic", "E2_temporal_recent", "E3_temporal_periodic", "E4_temporal_positional", "E5_causal", "E6_sparse", "E7_code", "E8_graph", "E9_hdc", "E10_multimodal", "E11_entity", "E12_late_interaction", "E13_splade"],
    "thresholds": {
      "conscious": 0.8,
      "fragmented": 0.5,
      "hypersync": 0.95
    }
  }
}
```
**Verification**:
1. `r` equals `synchronization`
2. `phases` has exactly 13 elements
3. `natural_freqs` has exactly 13 elements (Hz values: 40, 8, 8, 8, 25, 4, 25, 12, 80, 40, 15, 60, 4)
4. `thresholds.conscious` is exactly 0.8
5. `thresholds.fragmented` is exactly 0.5
6. `thresholds.hypersync` is exactly 0.95
</manual_testing_protocol>

<test_implementation>
## Tests to Add (handlers/tests/consciousness_dispatch.rs)

```rust
//! Consciousness JSON-RPC method dispatch tests.
//!
//! TASK-INTEG-003: Tests that consciousness/* methods dispatch correctly
//! and return REAL data from the GWT provider infrastructure.
//!
//! NO MOCK DATA. All tests use real providers via with_default_gwt().

use serde_json::json;
use crate::protocol::{methods, JsonRpcRequest, JsonRpcId, JsonRpcResponse};
use crate::handlers::Handlers;
use super::test_utils::{create_test_handlers_with_gwt, create_test_handlers_no_gwt};

/// FSV Test: consciousness/get_state with GWT initialized
#[tokio::test]
async fn test_consciousness_get_state_returns_real_data() {
    // Setup: Create handlers with real GWT providers
    let handlers = create_test_handlers_with_gwt().await;

    // Execute
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(JsonRpcId::Number(1)),
        method: methods::CONSCIOUSNESS_GET_STATE.to_string(),
        params: Some(json!({})),
    };
    let response = handlers.dispatch_request(request).await;

    // Verify: Response has real data, not mocks
    assert!(response.error.is_none(), "Should not error with GWT initialized");
    let result = response.result.expect("Should have result");

    // FSV: Check source of truth matches response
    let r_response = result.get("r").and_then(|v| v.as_f64()).expect("r must exist");
    assert!((0.0..=1.0).contains(&amp;r_response), "r must be in [0,1], got {}", r_response);

    let state = result.get("state").and_then(|v| v.as_str()).expect("state must exist");
    assert!(["DORMANT", "FRAGMENTED", "EMERGING", "CONSCIOUS", "HYPERSYNC"].contains(&amp;state));

    // FSV: Verify workspace data is real
    let workspace = result.get("workspace").expect("workspace must exist");
    let threshold = workspace.get("coherence_threshold").and_then(|v| v.as_f64()).expect("threshold exists");
    assert!((threshold - 0.8).abs() &lt; 0.001, "Coherence threshold must be 0.8, got {}", threshold);

    // FSV: Verify identity has 13-element purpose vector
    let identity = result.get("identity").expect("identity must exist");
    let pv = identity.get("purpose_vector").and_then(|v| v.as_array()).expect("purpose_vector exists");
    assert_eq!(pv.len(), 13, "Purpose vector must have 13 elements");

    println!("[FSV] consciousness/get_state verification PASSED");
    println!("[FSV]   r={}, state={}", r_response, state);
}

/// FSV Test: consciousness/get_state fails fast without GWT
#[tokio::test]
async fn test_consciousness_get_state_fails_without_gwt() {
    // Setup: Create handlers WITHOUT GWT (using basic new())
    let handlers = create_test_handlers_no_gwt().await;

    // Execute
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(JsonRpcId::Number(1)),
        method: methods::CONSCIOUSNESS_GET_STATE.to_string(),
        params: Some(json!({})),
    };
    let response = handlers.dispatch_request(request).await;

    // Verify: Must FAIL FAST with correct error code
    assert!(response.result.is_none(), "Should not have result without GWT");
    let error = response.error.expect("Should have error");
    assert_eq!(error.code, -32063, "Error code must be GWT_NOT_INITIALIZED (-32063)");
    assert!(error.message.contains("not initialized"), "Error message must mention initialization");

    println!("[FSV] consciousness/get_state FAIL FAST verification PASSED");
}

/// FSV Test: consciousness/sync_level returns lightweight data
#[tokio::test]
async fn test_consciousness_sync_level_lightweight_check() {
    let handlers = create_test_handlers_with_gwt().await;

    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(JsonRpcId::Number(2)),
        method: methods::CONSCIOUSNESS_SYNC_LEVEL.to_string(),
        params: Some(json!({})),
    };
    let response = handlers.dispatch_request(request).await;

    assert!(response.error.is_none());
    let result = response.result.expect("Should have result");

    // FSV: Verify phases array has 13 elements
    let phases = result.get("phases").and_then(|v| v.as_array()).expect("phases exists");
    assert_eq!(phases.len(), 13, "Must have 13 oscillator phases");

    // FSV: Verify natural frequencies are correct Hz values
    let freqs = result.get("natural_freqs").and_then(|v| v.as_array()).expect("natural_freqs exists");
    assert_eq!(freqs.len(), 13, "Must have 13 natural frequencies");
    // E1=40Hz, E2-4=8Hz, E5=25Hz, E6=4Hz, E7=25Hz, E8=12Hz, E9=80Hz, E10=40Hz, E11=15Hz, E12=60Hz, E13=4Hz
    let expected_freqs = [40.0, 8.0, 8.0, 8.0, 25.0, 4.0, 25.0, 12.0, 80.0, 40.0, 15.0, 60.0, 4.0];
    for (i, (actual, expected)) in freqs.iter().zip(expected_freqs.iter()).enumerate() {
        let actual_val = actual.as_f64().expect("freq is f64");
        assert!((actual_val - expected).abs() &lt; 0.01, "Freq[{}] should be {}, got {}", i, expected, actual_val);
    }

    // FSV: Verify thresholds are constitution-mandated values
    let thresholds = result.get("thresholds").expect("thresholds exists");
    assert_eq!(thresholds.get("conscious").and_then(|v| v.as_f64()), Some(0.8));
    assert_eq!(thresholds.get("fragmented").and_then(|v| v.as_f64()), Some(0.5));
    assert_eq!(thresholds.get("hypersync").and_then(|v| v.as_f64()), Some(0.95));

    println!("[FSV] consciousness/sync_level verification PASSED");
}

/// Edge case: Synchronized network returns high r
#[tokio::test]
async fn test_synchronized_network_high_r() {
    // Setup with synchronized Kuramoto network
    let handlers = create_test_handlers_synchronized().await;

    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(JsonRpcId::Number(3)),
        method: methods::CONSCIOUSNESS_SYNC_LEVEL.to_string(),
        params: Some(json!({})),
    };
    let response = handlers.dispatch_request(request).await;

    let result = response.result.expect("Should have result");
    let r = result.get("r").and_then(|v| v.as_f64()).expect("r exists");

    // Synchronized network should have r > 0.99
    assert!(r > 0.99, "Synchronized network should have r > 0.99, got {}", r);

    let state = result.get("state").and_then(|v| v.as_str()).expect("state exists");
    assert!(["CONSCIOUS", "HYPERSYNC"].contains(&amp;state), "State should be CONSCIOUS or HYPERSYNC, got {}", state);

    println!("[FSV] Synchronized network edge case PASSED: r={}, state={}", r, state);
}

/// Edge case: Incoherent network returns low r
#[tokio::test]
async fn test_incoherent_network_low_r() {
    // Setup with incoherent Kuramoto network
    let handlers = create_test_handlers_incoherent().await;

    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(JsonRpcId::Number(4)),
        method: methods::CONSCIOUSNESS_SYNC_LEVEL.to_string(),
        params: Some(json!({})),
    };
    let response = handlers.dispatch_request(request).await;

    let result = response.result.expect("Should have result");
    let r = result.get("r").and_then(|v| v.as_f64()).expect("r exists");

    // Incoherent network should have r &lt; 0.1
    assert!(r &lt; 0.1, "Incoherent network should have r &lt; 0.1, got {}", r);

    let state = result.get("state").and_then(|v| v.as_str()).expect("state exists");
    assert!(["DORMANT", "FRAGMENTED"].contains(&amp;state), "State should be DORMANT or FRAGMENTED, got {}", state);

    println!("[FSV] Incoherent network edge case PASSED: r={}, state={}", r, state);
}
```

## Test Utility Functions Needed

Add to `handlers/tests/test_utils.rs`:

```rust
/// Create handlers with default GWT providers (REAL, not mock)
pub async fn create_test_handlers_with_gwt() -> Handlers {
    // Use the actual with_default_gwt() constructor
    Handlers::with_default_gwt(/* required params */)
}

/// Create handlers without GWT (for FAIL FAST testing)
pub async fn create_test_handlers_no_gwt() -> Handlers {
    // Use basic new() without GWT providers
    Handlers::new(/* required params */)
}

/// Create handlers with synchronized Kuramoto network (all phases = 0)
pub async fn create_test_handlers_synchronized() -> Handlers {
    // Create with KuramotoProviderImpl::synchronized()
    let kuramoto = KuramotoProviderImpl::synchronized();
    Handlers::with_custom_gwt(kuramoto, /* ... */)
}

/// Create handlers with incoherent Kuramoto network (random phases)
pub async fn create_test_handlers_incoherent() -> Handlers {
    // Create with KuramotoProviderImpl::incoherent()
    let kuramoto = KuramotoProviderImpl::incoherent();
    Handlers::with_custom_gwt(kuramoto, /* ... */)
}
```
</test_implementation>

<files_to_modify>
  <file path="crates/context-graph-mcp/src/protocol.rs">
    Add after line 348:
    pub const CONSCIOUSNESS_GET_STATE: &amp;str = "consciousness/get_state";
    pub const CONSCIOUSNESS_SYNC_LEVEL: &amp;str = "consciousness/sync_level";
  </file>
  <file path="crates/context-graph-mcp/src/handlers/core.rs">
    Add in dispatch_request() match block:
    methods::CONSCIOUSNESS_GET_STATE => self.call_get_consciousness_state(request.id).await,
    methods::CONSCIOUSNESS_SYNC_LEVEL => self.call_get_kuramoto_sync(request.id).await,
  </file>
  <file path="crates/context-graph-mcp/src/handlers/tests/mod.rs">
    Add: pub mod consciousness_dispatch;
  </file>
  <file path="crates/context-graph-mcp/src/handlers/tests/test_utils.rs">
    Add: create_test_handlers_with_gwt, create_test_handlers_no_gwt,
    create_test_handlers_synchronized, create_test_handlers_incoherent
  </file>
</files_to_modify>

<files_to_create>
  <file path="crates/context-graph-mcp/src/handlers/tests/consciousness_dispatch.rs">
    FSV tests for consciousness/* method dispatch (code provided above)
  </file>
</files_to_create>

<do_not_create>
  handlers/consciousness.rs - ALL LOGIC EXISTS IN handlers/tools.rs
</do_not_create>

<validation_criteria>
  <criterion>consciousness/get_state dispatches to call_get_consciousness_state</criterion>
  <criterion>consciousness/sync_level dispatches to call_get_kuramoto_sync</criterion>
  <criterion>Response r value matches kuramoto.order_parameter().0</criterion>
  <criterion>Response state matches ConsciousnessState::from_level(r).name()</criterion>
  <criterion>Response workspace.coherence_threshold is exactly 0.8</criterion>
  <criterion>Response phases array has exactly 13 elements</criterion>
  <criterion>Response natural_freqs matches constitution Hz values</criterion>
  <criterion>Error -32063 when GWT not initialized (FAIL FAST)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-mcp handlers::tests::consciousness_dispatch -- --nocapture</command>
  <command>cargo test -p context-graph-mcp handlers::tests::gwt -- --nocapture</command>
</test_commands>

<evidence_of_success>
After implementation, run:
```bash
cargo test -p context-graph-mcp handlers::tests::consciousness_dispatch -- --nocapture 2>&amp;1
```

Expected output includes:
```
[FSV] consciousness/get_state verification PASSED
[FSV]   r=0.xxx, state=EMERGING
[FSV] consciousness/get_state FAIL FAST verification PASSED
[FSV] consciousness/sync_level verification PASSED
[FSV] Synchronized network edge case PASSED: r=0.99+, state=CONSCIOUS
[FSV] Incoherent network edge case PASSED: r=0.0x, state=DORMANT

test result: ok. 5 passed; 0 failed; 0 ignored
```

Also verify dispatch routes work:
```bash
# In a test, send raw JSON-RPC and check response structure
echo '{"jsonrpc":"2.0","id":1,"method":"consciousness/get_state","params":{}}' | cargo run -p context-graph-mcp --bin mcp_server
# Response must have r, state, workspace, identity, component_analysis fields
```
</evidence_of_success>

<next_tasks>
  After TASK-INTEG-003:
  - TASK-INTEG-004: Hook Protocol &amp; Core Handlers (uses consciousness/* methods)
  - TASK-INTEG-005: Edit Hooks
</next_tasks>
</task_spec>
```

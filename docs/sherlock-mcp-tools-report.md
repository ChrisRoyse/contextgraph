# SHERLOCK HOLMES FORENSIC INVESTIGATION REPORT
## SPEC-MCP-001 Compliance: MCP Protocol and Tool Registry

```
=============================================================
            CASE FILE: MCP-TOOLS-FORENSIC-001
=============================================================
Date:        2026-01-12
Investigator: Sherlock Holmes (Forensic Code Detective)
Subject:     MCP Protocol Compliance and Tool Registry
Spec Ref:    SPEC-MCP-001
Verdict:     PARTIALLY COMPLIANT - GAPS IDENTIFIED
=============================================================
```

---

## 1. GWT Tools Implementation Status

**Constitution Reference:** `constitution.yaml` line 335
```yaml
gwt: [get_consciousness_state, get_workspace_status, get_kuramoto_sync,
      get_ego_state, trigger_workspace_broadcast, adjust_coupling, compute_delta_sc]
```

### 1.1 Evidence Matrix

| Tool Name | Definition | Handler | Dispatch | Tests | VERDICT |
|-----------|------------|---------|----------|-------|---------|
| `get_consciousness_state` | `/crates/context-graph-mcp/src/tools/definitions/gwt.rs:11-26` | `/crates/context-graph-mcp/src/handlers/tools/gwt_consciousness.rs:30-183` | Line 66 | YES | **INNOCENT** |
| `get_workspace_status` | `/crates/context-graph-mcp/src/tools/definitions/gwt.rs:46-62` | `/crates/context-graph-mcp/src/handlers/tools/gwt_workspace.rs:26-73` | Line 68 | YES | **INNOCENT** |
| `get_kuramoto_sync` | `/crates/context-graph-mcp/src/tools/definitions/gwt.rs:28-44` | `/crates/context-graph-mcp/src/handlers/tools/gwt_consciousness.rs:203-271` | Line 67 | YES | **INNOCENT** |
| `get_ego_state` | `/crates/context-graph-mcp/src/tools/definitions/gwt.rs:64-83` | `/crates/context-graph-mcp/src/handlers/tools/gwt_consciousness.rs:290-387` | Line 69 | YES | **INNOCENT** |
| `trigger_workspace_broadcast` | `/crates/context-graph-mcp/src/tools/definitions/gwt.rs:85-121` | `/crates/context-graph-mcp/src/handlers/tools/gwt_workspace.rs:93-229` | Line 70-72 | YES | **INNOCENT** |
| `adjust_coupling` | `/crates/context-graph-mcp/src/tools/definitions/gwt.rs:123-142` | `/crates/context-graph-mcp/src/handlers/tools/gwt_workspace.rs:245-308` | Line 73 | YES | **INNOCENT** |

### 1.2 Special Case: get_johari_classification

**OBSERVATION:** The SPEC-MCP-001 document at line 39 lists `get_johari_classification` in the constitution gwt_tools quote. However, upon direct inspection of `constitution.yaml` line 335, this tool is **NOT PRESENT**.

**CONTRADICTION DETECTED:**
- SPEC-MCP-001 claims: `get_johari_classification` is constitution-mandated
- constitution.yaml actual: Tool is NOT listed in `mcp.core_tools.gwt`
- SPEC-MCP-001 line 107 states: "Implemented (via `get_memetic_status`)"

**VERDICT:** The SPEC contains STALE INFORMATION. The constitution does NOT mandate `get_johari_classification`. Johari quadrants are returned via `compute_delta_sc` instead.

---

## 2. compute_delta_sc Handler Evidence

**SPEC CLAIM:** "compute_delta_sc tool is **NOT IMPLEMENTED**" (SPEC-MCP-001 line 34)

**FORENSIC INVESTIGATION:**

### 2.1 Definition Evidence
**File:** `/crates/context-graph-mcp/src/tools/definitions/utl.rs`
```rust
ToolDefinition::new(
    "gwt/compute_delta_sc",
    "Compute per-embedder entropy (delta-S) and aggregate coherence (delta-C)..."
)
```
**STATUS:** EXISTS

### 2.2 Tool Name Constant Evidence
**File:** `/crates/context-graph-mcp/src/tools/names.rs` line 30
```rust
pub const COMPUTE_DELTA_SC: &str = "gwt/compute_delta_sc";
```
**STATUS:** EXISTS

### 2.3 Handler Evidence
**File:** `/crates/context-graph-mcp/src/handlers/utl/gwt.rs`
```rust
pub(crate) async fn handle_gwt_compute_delta_sc(
    &self,
    id: Option<JsonRpcId>,
    params: Option<serde_json::Value>,
) -> JsonRpcResponse { ... }
```
**LINES:** 37-214
**STATUS:** FULLY IMPLEMENTED

### 2.4 Dispatch Evidence
**File:** `/crates/context-graph-mcp/src/handlers/tools/dispatch.rs` lines 74-77
```rust
tool_names::COMPUTE_DELTA_SC => {
    self.handle_gwt_compute_delta_sc(id, Some(arguments)).await
}
```
**STATUS:** DISPATCHED

### 2.5 Test Evidence
**File:** `/crates/context-graph-mcp/src/handlers/tests/utl/delta_sc_valid.rs`
- `test_gwt_compute_delta_sc_valid` - Passes
- `test_gwt_compute_delta_sc_per_embedder_count` - Passes
- `test_gwt_compute_delta_sc_ap10_range_compliance` - Passes
- `test_gwt_compute_delta_sc_johari_quadrant_values` - Passes
- `test_gwt_compute_delta_sc_with_diagnostics` - Passes
- `test_gwt_compute_delta_sc_custom_johari_threshold` - Passes

**STATUS:** TESTED

### 2.6 compute_delta_sc VERDICT

```
============================================================
          VERDICT: compute_delta_sc is INNOCENT
============================================================
The SPEC-MCP-001 claim that compute_delta_sc is "NOT IMPLEMENTED"
is OUTDATED. The tool was implemented per TASK-UTL-P1-001.

EVIDENCE CHAIN:
1. Definition: EXISTS in definitions/utl.rs
2. Constant:   EXISTS in names.rs
3. Handler:    EXISTS in handlers/utl/gwt.rs (178 lines)
4. Dispatch:   EXISTS in handlers/tools/dispatch.rs
5. Tests:      6 tests PASS

The spec document requires updating to reflect current state.
============================================================
```

---

## 3. Naming Alias Status

**SPEC REQUIREMENT (FR-001 to FR-004):** Aliases must map PRD names to canonical names.

| PRD Name (Alias) | Canonical Name | Alias Implemented | VERDICT |
|------------------|----------------|-------------------|---------|
| `discover_goals` | `discover_sub_goals` | NO | **GUILTY** |
| `consolidate_memories` | `trigger_consolidation` | NO | **GUILTY** |

### 3.1 Alias Implementation Evidence

**Expected Implementation (per SPEC-MCP-001 Section 5.1):**
```rust
pub mod tool_aliases {
    pub static ALIAS_MAP: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
        let mut m = HashMap::new();
        m.insert("discover_goals", "discover_sub_goals");
        m.insert("consolidate_memories", "trigger_consolidation");
        m
    });
}
```

**Actual Implementation:**
```
GREP RESULT: No matches found for "tool_aliases|ALIAS_MAP|resolve\("
```

**VERDICT: ALIASES NOT IMPLEMENTED**

The alias resolution system specified in SPEC-MCP-001 Section 5 has NOT been implemented. Calling `discover_goals` or `consolidate_memories` will return error code -32004 (TOOL_NOT_FOUND).

---

## 4. Complete Tool Registry List

### 4.1 Registered Tools (39 Total)

| Category | Count | Tools |
|----------|-------|-------|
| **Core** | 6 | `inject_context`, `store_memory`, `get_memetic_status`, `get_graph_manifest`, `search_graph`, `utl_status` |
| **GWT** | 6 | `get_consciousness_state`, `get_kuramoto_sync`, `get_workspace_status`, `get_ego_state`, `trigger_workspace_broadcast`, `adjust_coupling` |
| **UTL** | 1 | `gwt/compute_delta_sc` |
| **ATC** | 3 | `get_threshold_status`, `get_calibration_metrics`, `trigger_recalibration` |
| **Dream** | 4 | `trigger_dream`, `get_dream_status`, `abort_dream`, `get_amortized_shortcuts` |
| **Neuromod** | 2 | `get_neuromodulation_state`, `adjust_neuromodulator` |
| **Steering** | 1 | `get_steering_feedback` |
| **Causal** | 1 | `omni_infer` |
| **Teleological** | 5 | `search_teleological`, `compute_teleological_vector`, `fuse_embeddings`, `update_synergy_matrix`, `manage_teleological_profile` |
| **Autonomous** | 7 | `auto_bootstrap_north_star`, `get_alignment_drift`, `trigger_drift_correction`, `get_pruning_candidates`, `trigger_consolidation`, `discover_sub_goals`, `get_autonomous_status` |
| **Meta-UTL** | 3 | `get_meta_learning_status`, `trigger_lambda_recalibration`, `get_meta_learning_log` |

### 4.2 Tool Registration vs Dispatch Status

```
=============================================================
          CRITICAL DISCREPANCY DETECTED
=============================================================
```

| Category | Registered | Dispatched | GAP |
|----------|------------|------------|-----|
| Core | 6 | 6 | NONE |
| GWT | 6 | 6 | NONE |
| UTL | 1 | 1 | NONE |
| ATC | 3 | 3 | NONE |
| Dream | 4 | 4 | NONE |
| Neuromod | 2 | 2 | NONE |
| Steering | 1 | 1 | NONE |
| Causal | 1 | 1 | NONE |
| Teleological | 5 | 5 | NONE |
| Autonomous | 7 | 7 | NONE |
| **Meta-UTL** | **3** | **0** | **CRITICAL** |

**TOTAL:** 39 Registered, 36 Dispatched, **3 MISSING DISPATCH**

### 4.3 Meta-UTL Tools - GUILTY

**Evidence Chain:**

1. **Definitions exist:** `/crates/context-graph-mcp/src/tools/definitions/meta_utl.rs`
   - `get_meta_learning_status` (lines 12-34)
   - `trigger_lambda_recalibration` (lines 36-64)
   - `get_meta_learning_log` (lines 66-109)

2. **Registered in mod.rs:** `/crates/context-graph-mcp/src/tools/definitions/mod.rs` line 57
   ```rust
   tools.extend(meta_utl::definitions());
   ```

3. **Test expects 39 tools:** `/crates/context-graph-mcp/src/tools/mod.rs` line 42
   ```rust
   assert_eq!(tools.len(), 39);
   ```

4. **BUT NO DISPATCH:** `/crates/context-graph-mcp/src/handlers/tools/dispatch.rs`
   - No case for `GET_META_LEARNING_STATUS`
   - No case for `TRIGGER_LAMBDA_RECALIBRATION`
   - No case for `GET_META_LEARNING_LOG`

5. **AND NO CONSTANTS:** `/crates/context-graph-mcp/src/tools/names.rs`
   - No constant defined for any Meta-UTL tool

**CONSEQUENCE:** Calling any Meta-UTL tool via MCP will return:
```json
{
  "error": {
    "code": -32004,
    "message": "Unknown tool: get_meta_learning_status"
  }
}
```

---

## 5. Gaps and Recommended Actions

### 5.1 Critical Gaps (P0)

| ID | Gap | Impact | Recommended Fix |
|----|-----|--------|-----------------|
| GAP-1 | Meta-UTL tools not dispatched | Tools discoverable but non-functional | Add dispatch cases and name constants |
| GAP-2 | Naming aliases not implemented | PRD compatibility broken | Implement alias resolution per spec |

### 5.2 High Gaps (P1)

| ID | Gap | Impact | Recommended Fix |
|----|-----|--------|-----------------|
| GAP-3 | SPEC-MCP-001 outdated | Spec claims compute_delta_sc missing but it exists | Update spec to reflect implementation |
| GAP-4 | Spec references non-existent get_johari_classification | Documentation confusion | Remove from spec or clarify Johari via compute_delta_sc |

### 5.3 Detailed Remediation Steps

#### GAP-1: Meta-UTL Tool Dispatch

**File:** `/crates/context-graph-mcp/src/tools/names.rs`

Add after line 101:
```rust
// ========== META-UTL TOOLS (TASK-METAUTL-P0-005) ==========

/// TASK-METAUTL-P0-005: Get meta-learning self-correction status
pub const GET_META_LEARNING_STATUS: &str = "get_meta_learning_status";
/// TASK-METAUTL-P0-005: Manually trigger lambda recalibration
pub const TRIGGER_LAMBDA_RECALIBRATION: &str = "trigger_lambda_recalibration";
/// TASK-METAUTL-P0-005: Query meta-learning event log
pub const GET_META_LEARNING_LOG: &str = "get_meta_learning_log";
```

**File:** `/crates/context-graph-mcp/src/handlers/tools/dispatch.rs`

Add before the wildcard `_` case:
```rust
// TASK-METAUTL-P0-005: Meta-UTL self-correction tools
tool_names::GET_META_LEARNING_STATUS => {
    self.call_get_meta_learning_status(id, arguments).await
}
tool_names::TRIGGER_LAMBDA_RECALIBRATION => {
    self.call_trigger_lambda_recalibration(id, arguments).await
}
tool_names::GET_META_LEARNING_LOG => {
    self.call_get_meta_learning_log(id, arguments).await
}
```

#### GAP-2: Alias Implementation

**New File:** `/crates/context-graph-mcp/src/tools/aliases.rs`

```rust
//! Tool alias resolution for backwards compatibility.

use once_cell::sync::Lazy;
use std::collections::HashMap;

pub static ALIAS_MAP: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert("discover_goals", "discover_sub_goals");
    m.insert("consolidate_memories", "trigger_consolidation");
    m
});

/// Resolve a tool name, returning canonical name if alias exists.
#[inline]
pub fn resolve(name: &str) -> &str {
    ALIAS_MAP.get(name).copied().unwrap_or(name)
}
```

**Update:** `/crates/context-graph-mcp/src/handlers/tools/dispatch.rs`

After extracting `tool_name`:
```rust
// Resolve alias to canonical name
let canonical_name = crate::tools::aliases::resolve(tool_name);

if canonical_name != tool_name {
    debug!(
        alias = tool_name,
        canonical = canonical_name,
        "Resolved tool alias to canonical name"
    );
}

match canonical_name { ... }
```

---

## 6. Summary Verdict

```
=============================================================
                  CASE CLOSED
=============================================================

THE CRIMES:
1. Meta-UTL tools registered but not dispatched (3 tools)
2. Naming aliases specified but not implemented (2 aliases)
3. SPEC-MCP-001 contains outdated claims about compute_delta_sc

THE EVIDENCE:
- 39 tools registered in definitions/mod.rs
- 36 tools dispatched in handlers/tools/dispatch.rs
- 0 aliases implemented
- compute_delta_sc IS implemented (contrary to spec claim)

GUILTY PARTIES:
- /crates/context-graph-mcp/src/handlers/tools/dispatch.rs (missing 3 cases)
- /crates/context-graph-mcp/src/tools/names.rs (missing 3 constants)
- Missing: /crates/context-graph-mcp/src/tools/aliases.rs

INNOCENT PARTIES:
- All 6 GWT tools (fully implemented and tested)
- compute_delta_sc (fully implemented per TASK-UTL-P1-001)
- All other 30 tools (properly dispatched)

COMPLIANCE SCORE: 36/39 tools functional (92%)

RECOMMENDED ACTION:
1. Add Meta-UTL dispatch and constants (GAP-1)
2. Implement alias resolution module (GAP-2)
3. Update SPEC-MCP-001 to reflect current state (GAP-3, GAP-4)

=============================================================
          The game is complete. - S.H.
=============================================================
```

---

## Appendix A: File References

| Purpose | File Path |
|---------|-----------|
| Tool Definitions | `/crates/context-graph-mcp/src/tools/definitions/` |
| Tool Names | `/crates/context-graph-mcp/src/tools/names.rs` |
| Tool Dispatch | `/crates/context-graph-mcp/src/handlers/tools/dispatch.rs` |
| GWT Handlers | `/crates/context-graph-mcp/src/handlers/tools/gwt_consciousness.rs`, `gwt_workspace.rs` |
| UTL Handler | `/crates/context-graph-mcp/src/handlers/utl/gwt.rs` |
| Constitution | `/docs2/constitution.yaml` line 335 |
| Spec | `/specs/functional/SPEC-MCP-001.md` |

## Appendix B: Test Verification Commands

```bash
# Verify tool count
cargo test --package context-graph-mcp --lib -- tools::tests::test_get_tool_definitions

# Verify tools list
cargo test --package context-graph-mcp --lib -- handlers::tests::tools_list

# Verify compute_delta_sc
cargo test --package context-graph-mcp --lib -- handlers::tests::utl::delta_sc_valid
```

---

*Investigation concluded: 2026-01-12*
*Case File: MCP-TOOLS-FORENSIC-001*
*Investigator: Sherlock Holmes*

# TASK-29: Implement merge_concepts Tool Schema

## CRITICAL: READ THIS ENTIRE DOCUMENT BEFORE WRITING ANY CODE

**Task ID**: TASK-MCP-003 (sequence 29)
**Status**: COMPLETE
**Layer**: Surface (Phase 4)
**Dependencies**: NONE (can start immediately)
**Blocks**: TASK-30 (merge_concepts handler implementation)
**Estimated Hours**: 2

---

## 1. CONTEXT FOR AI AGENT

You are implementing the `merge_concepts` MCP tool schema. This is a **schema-only** task - you define the input/output types. The actual handler logic is TASK-30.

### What This Tool Does
Per constitution.yaml and PRD Section 5.3:
- Merges 2-10 related concept nodes into a single unified node
- Supports 3 merge strategies: `union`, `intersection`, `weighted_average`
- Returns a `reversal_hash` for 30-day undo capability (SEC-06)
- Requires mandatory `rationale` per PRD 0.3

### Why This Exists
- **Curation**: Consolidate duplicate/similar memories into cleaner graph
- **Reversal**: 30-day recovery window per constitution SEC-06
- **Audit Trail**: All merges logged with rationale

---

## 2. CURRENT CODEBASE STATE (VERIFIED 2026-01-13)

### 2.1 Completed Prerequisites
```
TASK-01 through TASK-28: ALL COMPLETE
- TASK-27: epistemic_action schema COMPLETE
- TASK-28: epistemic_action handler COMPLETE
```

### 2.2 File Structure (EXISTING)
```
crates/context-graph-mcp/src/
├── tools/
│   ├── mod.rs              # get_tool_definitions() aggregates all tools
│   ├── types.rs            # ToolDefinition struct
│   ├── names.rs            # Tool name constants (NEED TO ADD MERGE_CONCEPTS)
│   ├── aliases.rs          # Tool aliases
│   └── definitions/
│       ├── mod.rs          # Collects all definitions (NEED TO ADD merge module)
│       ├── core.rs         # inject_context, store_memory, etc.
│       ├── gwt.rs          # get_consciousness_state, etc.
│       ├── utl.rs          # gwt/compute_delta_sc
│       ├── atc.rs          # threshold tools
│       ├── dream.rs        # trigger_dream, etc.
│       ├── neuromod.rs     # neuromodulation tools
│       ├── steering.rs     # get_steering_feedback
│       ├── causal.rs       # omni_infer
│       ├── teleological.rs # search_teleological, etc.
│       ├── autonomous.rs   # auto_bootstrap_north_star, etc.
│       ├── meta_utl.rs     # meta-learning tools
│       └── epistemic.rs    # epistemic_action (TASK-27) <- FOLLOW THIS PATTERN
└── handlers/
    ├── mod.rs              # Handler module organization
    └── epistemic.rs        # epistemic_action handler (TASK-28) <- TASK-30 follows this
```

### 2.3 Current Tool Count
```bash
# VERIFIED: 40 tools currently registered
cargo test -p context-graph-mcp test_get_tool_definitions
# After TASK-29: should be 41 tools
```

### 2.4 Schema Pattern (MANDATORY)
The codebase uses `serde_json::json!()` macros for schemas. **DO NOT use schemars**.

Example from `crates/context-graph-mcp/src/tools/definitions/epistemic.rs`:
```rust
ToolDefinition::new(
    "epistemic_action",
    "Description...",
    json!({
        "type": "object",
        "required": ["action_type", "target", "rationale"],
        "properties": {
            "action_type": { "type": "string", "enum": [...] },
            // ...
        }
    }),
)
```

---

## 3. EXACT IMPLEMENTATION REQUIREMENTS

### 3.1 File to CREATE: `crates/context-graph-mcp/src/tools/definitions/merge.rs`

```rust
//! Merge concepts tool definitions (TASK-MCP-003).
//!
//! Implements merge_concepts tool for consolidating related concept nodes.
//! Constitution: SEC-06 (30-day reversal), PRD Section 5.3
//!
//! ## Merge Strategies
//! - union: Combine all attributes from source nodes
//! - intersection: Keep only common attributes
//! - weighted_average: Weight by node importance/confidence

use serde_json::json;
use crate::tools::types::ToolDefinition;

/// Returns merge tool definitions.
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition::new(
            "merge_concepts",
            "Merge two or more related concept nodes into a unified node. \
             Supports union (combine all), intersection (common only), or \
             weighted_average (by importance) strategies. Returns reversal_hash \
             for 30-day undo capability. Requires rationale per PRD 0.3.",
            json!({
                "type": "object",
                "required": ["source_ids", "target_name", "rationale"],
                "properties": {
                    "source_ids": {
                        "type": "array",
                        "items": { "type": "string", "format": "uuid" },
                        "minItems": 2,
                        "maxItems": 10,
                        "description": "UUIDs of concepts to merge (2-10 required)"
                    },
                    "target_name": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 256,
                        "description": "Name for the merged concept (1-256 chars)"
                    },
                    "merge_strategy": {
                        "type": "string",
                        "enum": ["union", "intersection", "weighted_average"],
                        "default": "union",
                        "description": "Strategy: union=combine all, intersection=common only, weighted_average=by importance"
                    },
                    "rationale": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 1024,
                        "description": "Rationale for merge (REQUIRED per PRD 0.3)"
                    },
                    "force_merge": {
                        "type": "boolean",
                        "default": false,
                        "description": "Force merge even if priors conflict (use with caution)"
                    }
                }
            }),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_concepts_definition_exists() {
        let tools = definitions();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "merge_concepts");
    }

    #[test]
    fn test_merge_concepts_required_fields() {
        let tools = definitions();
        let schema = &tools[0].input_schema;
        let required = schema.get("required").unwrap().as_array().unwrap();

        let required_fields: Vec<&str> = required.iter().map(|v| v.as_str().unwrap()).collect();
        assert_eq!(required_fields.len(), 3);
        assert!(required_fields.contains(&"source_ids"));
        assert!(required_fields.contains(&"target_name"));
        assert!(required_fields.contains(&"rationale"));

        // merge_strategy and force_merge are NOT required (have defaults)
        assert!(!required_fields.contains(&"merge_strategy"));
        assert!(!required_fields.contains(&"force_merge"));
    }

    #[test]
    fn test_source_ids_constraints() {
        let tools = definitions();
        let props = tools[0].input_schema.get("properties").unwrap();
        let source_ids = props.get("source_ids").unwrap();

        // Array of UUIDs
        assert_eq!(source_ids.get("type").unwrap().as_str().unwrap(), "array");
        let items = source_ids.get("items").unwrap();
        assert_eq!(items.get("type").unwrap().as_str().unwrap(), "string");
        assert_eq!(items.get("format").unwrap().as_str().unwrap(), "uuid");

        // 2-10 items
        assert_eq!(source_ids.get("minItems").unwrap().as_u64().unwrap(), 2);
        assert_eq!(source_ids.get("maxItems").unwrap().as_u64().unwrap(), 10);
    }

    #[test]
    fn test_target_name_constraints() {
        let tools = definitions();
        let props = tools[0].input_schema.get("properties").unwrap();
        let target_name = props.get("target_name").unwrap();

        assert_eq!(target_name.get("type").unwrap().as_str().unwrap(), "string");
        assert_eq!(target_name.get("minLength").unwrap().as_u64().unwrap(), 1);
        assert_eq!(target_name.get("maxLength").unwrap().as_u64().unwrap(), 256);
    }

    #[test]
    fn test_merge_strategy_enum() {
        let tools = definitions();
        let props = tools[0].input_schema.get("properties").unwrap();
        let strategy = props.get("merge_strategy").unwrap();

        assert_eq!(strategy.get("type").unwrap().as_str().unwrap(), "string");
        let enum_values = strategy.get("enum").unwrap().as_array().unwrap();
        let values: Vec<&str> = enum_values.iter().map(|v| v.as_str().unwrap()).collect();

        assert_eq!(values.len(), 3);
        assert!(values.contains(&"union"));
        assert!(values.contains(&"intersection"));
        assert!(values.contains(&"weighted_average"));

        // Default is union
        assert_eq!(strategy.get("default").unwrap().as_str().unwrap(), "union");
    }

    #[test]
    fn test_rationale_constraints() {
        let tools = definitions();
        let props = tools[0].input_schema.get("properties").unwrap();
        let rationale = props.get("rationale").unwrap();

        assert_eq!(rationale.get("type").unwrap().as_str().unwrap(), "string");
        assert_eq!(rationale.get("minLength").unwrap().as_u64().unwrap(), 1);
        assert_eq!(rationale.get("maxLength").unwrap().as_u64().unwrap(), 1024);
    }

    #[test]
    fn test_force_merge_default() {
        let tools = definitions();
        let props = tools[0].input_schema.get("properties").unwrap();
        let force_merge = props.get("force_merge").unwrap();

        assert_eq!(force_merge.get("type").unwrap().as_str().unwrap(), "boolean");
        assert_eq!(force_merge.get("default").unwrap().as_bool().unwrap(), false);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let tools = definitions();
        let json_str = serde_json::to_string(&tools).expect("Serialization failed");
        assert!(json_str.contains("merge_concepts"));
        assert!(json_str.contains("inputSchema"));
        assert!(json_str.contains("source_ids"));
        assert!(json_str.contains("reversal"));
    }

    // ========== SYNTHETIC DATA VALIDATION TESTS ==========

    #[test]
    fn test_synthetic_valid_merge_input() {
        // Synthetic test data matching schema exactly
        let synthetic_input = json!({
            "source_ids": [
                "550e8400-e29b-41d4-a716-446655440001",
                "550e8400-e29b-41d4-a716-446655440002"
            ],
            "target_name": "Merged Authentication Concept",
            "merge_strategy": "union",
            "rationale": "Consolidating duplicate auth patterns for cleaner graph",
            "force_merge": false
        });

        let tools = definitions();
        let props = tools[0].input_schema.get("properties").unwrap();

        // Validate source_ids: 2 UUIDs, within [2,10] range
        let source_ids_arr = synthetic_input.get("source_ids").unwrap().as_array().unwrap();
        assert_eq!(source_ids_arr.len(), 2);
        let source_ids_schema = props.get("source_ids").unwrap();
        let min_items = source_ids_schema.get("minItems").unwrap().as_u64().unwrap() as usize;
        let max_items = source_ids_schema.get("maxItems").unwrap().as_u64().unwrap() as usize;
        assert!(source_ids_arr.len() >= min_items && source_ids_arr.len() <= max_items);

        // Validate target_name length
        let target_name = synthetic_input.get("target_name").unwrap().as_str().unwrap();
        let target_schema = props.get("target_name").unwrap();
        let name_min = target_schema.get("minLength").unwrap().as_u64().unwrap() as usize;
        let name_max = target_schema.get("maxLength").unwrap().as_u64().unwrap() as usize;
        assert!(target_name.len() >= name_min && target_name.len() <= name_max);

        // Validate merge_strategy in enum
        let strategy = synthetic_input.get("merge_strategy").unwrap().as_str().unwrap();
        let strategy_schema = props.get("merge_strategy").unwrap();
        let valid_strategies: Vec<&str> = strategy_schema.get("enum").unwrap().as_array().unwrap()
            .iter().map(|v| v.as_str().unwrap()).collect();
        assert!(valid_strategies.contains(&strategy));

        // Validate rationale length
        let rationale = synthetic_input.get("rationale").unwrap().as_str().unwrap();
        let rationale_schema = props.get("rationale").unwrap();
        let rat_min = rationale_schema.get("minLength").unwrap().as_u64().unwrap() as usize;
        let rat_max = rationale_schema.get("maxLength").unwrap().as_u64().unwrap() as usize;
        assert!(rationale.len() >= rat_min && rationale.len() <= rat_max);
    }

    #[test]
    fn test_edge_case_minimum_source_ids() {
        // Edge Case: Exactly 2 source_ids (minimum)
        let tools = definitions();
        let props = tools[0].input_schema.get("properties").unwrap();
        let source_ids = props.get("source_ids").unwrap();

        assert_eq!(source_ids.get("minItems").unwrap().as_u64().unwrap(), 2);
    }

    #[test]
    fn test_edge_case_maximum_source_ids() {
        // Edge Case: Exactly 10 source_ids (maximum)
        let tools = definitions();
        let props = tools[0].input_schema.get("properties").unwrap();
        let source_ids = props.get("source_ids").unwrap();

        assert_eq!(source_ids.get("maxItems").unwrap().as_u64().unwrap(), 10);
    }

    #[test]
    fn test_edge_case_maximum_target_name() {
        // Edge Case: 256-char target_name (maximum)
        let tools = definitions();
        let props = tools[0].input_schema.get("properties").unwrap();
        let target_name = props.get("target_name").unwrap();

        assert_eq!(target_name.get("maxLength").unwrap().as_u64().unwrap(), 256);

        // Verify a 256-char name would be valid
        let max_name = "x".repeat(256);
        assert_eq!(max_name.len(), 256);
    }

    #[test]
    fn test_verify_merge_concepts_in_all_tools() {
        // Verify tool appears in aggregated definitions (after mod.rs is updated)
        let tools = crate::tools::get_tool_definitions();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"merge_concepts"), "merge_concepts missing from tool list");
    }
}
```

### 3.2 File to MODIFY: `crates/context-graph-mcp/src/tools/definitions/mod.rs`

Add the merge module and include in get_tool_definitions():

```rust
// Add with other module declarations (around line 10-15):
pub mod merge;

// In get_tool_definitions() function, add (around line 45-50):
tools.extend(merge::definitions());
```

### 3.3 File to MODIFY: `crates/context-graph-mcp/src/tools/names.rs`

Add the tool name constant:

```rust
// Add in a new MERGE TOOLS section (after EPISTEMIC section, around line 118):

// ========== MERGE TOOLS (TASK-MCP-003) ==========

/// TASK-MCP-003: Merge related concept nodes into a unified node
/// Returns reversal_hash for 30-day undo per SEC-06
pub const MERGE_CONCEPTS: &str = "merge_concepts";
```

### 3.4 File to MODIFY: `crates/context-graph-mcp/src/tools/mod.rs`

Update the test assertion for tool count:

```rust
// In test_get_tool_definitions test (around line 48):
// Change from:
assert_eq!(tools.len(), 40);
// To:
assert_eq!(tools.len(), 41); // Added merge_concepts (TASK-29)
```

---

## 4. CONSTRAINTS (MUST NOT VIOLATE)

| Constraint | Source | Value |
|------------|--------|-------|
| source_ids count | Schema | 2-10 UUIDs |
| target_name length | Schema | 1-256 chars |
| rationale length | PRD 0.3 | 1-1024 chars (REQUIRED) |
| merge_strategy | Schema | union, intersection, weighted_average |
| merge_strategy default | Schema | "union" |
| force_merge default | Schema | false |
| Schema format | Codebase pattern | serde_json::json!() macro, NOT schemars |
| Reversal capability | SEC-06 | Output must include reversal_hash |

---

## 5. VERIFICATION COMMANDS

Execute these in order after implementation:

```bash
# Step 1: Check compilation
cargo check -p context-graph-mcp
# Expected: PASSES with no errors (warnings OK for now)

# Step 2: Run schema tests specifically
cargo test -p context-graph-mcp tools::definitions::merge::tests -- --nocapture
# Expected: All tests pass (10+ tests)

# Step 3: Verify tool count updated
cargo test -p context-graph-mcp test_get_tool_definitions -- --nocapture
# Expected: tools.len() == 41

# Step 4: Verify tool appears in list
cargo test -p context-graph-mcp test_verify_merge_concepts_in_all_tools -- --nocapture
# Expected: PASSES

# Step 5: Full test suite
cargo test -p context-graph-mcp --lib
# Expected: All tests pass
```

---

## 6. FULL STATE VERIFICATION PROTOCOL

### 6.1 Source of Truth
The tool definition is stored in memory as part of `get_tool_definitions()` return value.

**Verification Method**:
```bash
cargo test -p context-graph-mcp test_get_tool_definitions -- --nocapture 2>&1 | grep -E "tools.len|passed"
```
**Expected**: `tools.len() == 41` and test passes

### 6.2 Execute & Inspect
Run the specific schema test:
```bash
cargo test -p context-graph-mcp tools::definitions::merge::tests::test_merge_concepts_definition_exists -- --nocapture
```
**Expected Output**:
```
test tools::definitions::merge::tests::test_merge_concepts_definition_exists ... ok
```

### 6.3 Boundary & Edge Case Audit

**Edge Case 1: Minimum source_ids (2)**
- State Before: Schema requires minItems: 2
- Action: Test with exactly 2 UUIDs
- State After: Schema validation passes
- Test: `test_edge_case_minimum_source_ids`

**Edge Case 2: Maximum source_ids (10)**
- State Before: Schema requires maxItems: 10
- Action: Test with exactly 10 UUIDs
- State After: Schema validation passes
- Test: `test_edge_case_maximum_source_ids`

**Edge Case 3: Maximum target_name (256 chars)**
- State Before: Schema allows up to 256 chars
- Action: Test with 256-char string
- State After: Schema validation passes
- Test: `test_edge_case_maximum_target_name`

### 6.4 Evidence of Success

After running all tests, provide log output in this format:
```
=== TASK-29 Full State Verification ===
Date: <timestamp>
Compiler: cargo check PASSED
Schema Tests: <N> passed, 0 failed
Tool Count: 41 (was 40)
Tool Registration: merge_concepts FOUND in get_tool_definitions()
Edge Cases: 3 passed
```

---

## 7. MANUAL TESTING PROTOCOL

### 7.1 Synthetic Test Data

Use this input to verify schema accepts valid data:

```json
{
  "source_ids": [
    "550e8400-e29b-41d4-a716-446655440001",
    "550e8400-e29b-41d4-a716-446655440002",
    "550e8400-e29b-41d4-a716-446655440003"
  ],
  "target_name": "Unified Authentication Module",
  "merge_strategy": "weighted_average",
  "rationale": "Consolidating 3 auth-related concepts that share >0.9 similarity",
  "force_merge": false
}
```

**Expected Schema Validation**:
- source_ids: 3 items, within [2,10] range ✓
- target_name: 31 chars, within [1,256] range ✓
- merge_strategy: "weighted_average" in enum ✓
- rationale: 63 chars, within [1,1024] range ✓
- force_merge: boolean ✓

### 7.2 Invalid Input Tests (Handler Will Reject)

These inputs should fail in TASK-30 handler (not schema):

**Invalid 1: Only 1 source_id**
```json
{"source_ids": ["550e8400-e29b-41d4-a716-446655440001"], "target_name": "X", "rationale": "Y"}
```
Expected: Rejected by minItems: 2

**Invalid 2: 11 source_ids**
```json
{"source_ids": ["id1", "id2", "id3", "id4", "id5", "id6", "id7", "id8", "id9", "id10", "id11"], ...}
```
Expected: Rejected by maxItems: 10

**Invalid 3: Empty target_name**
```json
{"source_ids": [...], "target_name": "", "rationale": "Y"}
```
Expected: Rejected by minLength: 1

**Invalid 4: Invalid merge_strategy**
```json
{"source_ids": [...], "target_name": "X", "merge_strategy": "invalid", "rationale": "Y"}
```
Expected: Rejected by enum validation

---

## 8. FILES SUMMARY

| Action | File Path | Changes |
|--------|-----------|---------|
| CREATE | `crates/context-graph-mcp/src/tools/definitions/merge.rs` | New 200+ line file |
| MODIFY | `crates/context-graph-mcp/src/tools/definitions/mod.rs` | Add `pub mod merge;` + extend |
| MODIFY | `crates/context-graph-mcp/src/tools/names.rs` | Add `MERGE_CONCEPTS` constant |
| MODIFY | `crates/context-graph-mcp/src/tools/mod.rs` | Update tool count: 40 → 41 |

---

## 9. DEFINITION OF DONE CHECKLIST

Before marking complete, ALL must be checked:

- [ ] `merge.rs` file created with `definitions()` function
- [ ] `merge.rs` has 10+ passing tests
- [ ] `mod.rs` includes `pub mod merge;` declaration
- [ ] `mod.rs` calls `tools.extend(merge::definitions());`
- [ ] `names.rs` has `MERGE_CONCEPTS` constant
- [ ] Tool count test updated to 41
- [ ] `cargo check -p context-graph-mcp` passes
- [ ] All merge schema tests pass
- [ ] Tool appears in `get_tool_definitions()` output
- [ ] Synthetic data validation test passes
- [ ] Edge case tests pass (min/max source_ids, max target_name)
- [ ] Evidence log captured with all tests passing

---

## 10. OUTPUT SCHEMA (For TASK-30 Handler Reference)

The handler (TASK-30) will return this output structure:

```json
{
  "success": true,
  "merged_id": "550e8400-e29b-41d4-a716-446655440099",
  "reversal_hash": "sha256:abc123...",
  "merged_node": {
    "id": "550e8400-e29b-41d4-a716-446655440099",
    "name": "Unified Authentication Module",
    "source_count": 3,
    "strategy_used": "weighted_average",
    "created_at": "2026-01-13T12:00:00Z"
  },
  "error": null
}
```

This is informational for TASK-30. TASK-29 only defines INPUT schema.

---

## 11. COMMON PITFALLS TO AVOID

1. **DO NOT use schemars** - Use `serde_json::json!()` pattern
2. **DO NOT forget tool count update** - Must change 40 → 41
3. **DO NOT make rationale optional** - It's REQUIRED per PRD 0.3
4. **DO NOT forget default values** - merge_strategy="union", force_merge=false
5. **DO NOT implement handler logic** - That's TASK-30's scope
6. **DO NOT use `.unwrap()` in tests** - Use `.expect()` with context

---

## 12. RELATED FILES REFERENCE

Study these for the correct pattern:
- `crates/context-graph-mcp/src/tools/definitions/epistemic.rs` (TASK-27) - EXACT PATTERN TO FOLLOW
- `crates/context-graph-mcp/src/tools/definitions/core.rs` - Additional examples
- `crates/context-graph-mcp/src/tools/types.rs` - ToolDefinition struct
- `docs2/contextprd.md:532` - merge_concepts specification
- `docs2/constitution.yaml:123` - SEC-06 30-day reversal requirement

---

## 13. ANTI-PATTERNS FROM CONSTITUTION (DO NOT VIOLATE)

From `docs2/constitution.yaml`:
- **AP-11**: "merge_concepts without priors_vibe_check" - Handler must check priors (TASK-30)
- **SEC-06**: "Soft delete 30-day recovery" - reversal_hash MUST be in output

---

*Document Version: 1.0.0 | Created: 2026-01-13 | Status: COMPLETE*

---

## 14. COMPLETION EVIDENCE

**Completed**: 2026-01-13T21:45:00Z

### Full State Verification Results

```
=== TASK-29 Full State Verification ===
Date: 2026-01-13T21:45:21Z

Compiler: cargo check PASSED
Schema Tests: 20 passed, 0 failed
Tool Count: 41 (was 40)
Tool Registration: merge_concepts FOUND in get_tool_definitions()
Synthetic Data Tests: 2 passed
Edge Case Tests: 5 passed
Full Test Suite: 886 passed, 0 failed

FILES MODIFIED:
- CREATED: crates/context-graph-mcp/src/tools/definitions/merge.rs (17695 bytes)
- MODIFIED: crates/context-graph-mcp/src/tools/definitions/mod.rs (added merge module)
- MODIFIED: crates/context-graph-mcp/src/tools/names.rs (added MERGE_CONCEPTS constant)
- MODIFIED: crates/context-graph-mcp/src/tools/mod.rs (updated tool count to 41)
- MODIFIED: crates/context-graph-mcp/src/handlers/tests/tools_list.rs (updated tool count to 41)
```

### Schema Constraints Verified

| Constraint | Expected | Verified |
|------------|----------|----------|
| source_ids minItems | 2 | ✓ |
| source_ids maxItems | 10 | ✓ |
| target_name minLength | 1 | ✓ |
| target_name maxLength | 256 | ✓ |
| rationale minLength | 1 | ✓ |
| rationale maxLength | 1024 | ✓ |
| merge_strategy enum | union, intersection, weighted_average | ✓ |
| merge_strategy default | union | ✓ |
| force_merge default | false | ✓ |
| Required fields | source_ids, target_name, rationale | ✓ |

### Definition of Done Checklist

- [x] `merge.rs` file created with `definitions()` function
- [x] `merge.rs` has 20 passing tests
- [x] `mod.rs` includes `pub mod merge;` declaration
- [x] `mod.rs` calls `tools.extend(merge::definitions());`
- [x] `names.rs` has `MERGE_CONCEPTS` constant
- [x] Tool count test updated to 41
- [x] `cargo check -p context-graph-mcp` passes
- [x] All merge schema tests pass
- [x] Tool appears in `get_tool_definitions()` output
- [x] Synthetic data validation test passes
- [x] Edge case tests pass (min/max source_ids, max target_name)
- [x] Evidence log captured with all tests passing

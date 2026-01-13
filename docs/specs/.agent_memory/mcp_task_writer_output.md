# MCP Dispatch Task Spec Writer Output

## Session: 2026-01-12
## Agent: MCP Dispatch Task Spec Writer
## Status: COMPLETE

---

## Files Created

| File | Absolute Path | Purpose |
|------|---------------|---------|
| TASK-MCP-P0-001.md | `/home/cabdru/contextgraph/docs/specs/tasks/TASK-MCP-P0-001.md` | Add Meta-UTL Tool Dispatch |
| TASK-MCP-P1-001.md | `/home/cabdru/contextgraph/docs/specs/tasks/TASK-MCP-P1-001.md` | Implement Tool Name Aliases |
| Agent Memory | `/home/cabdru/contextgraph/docs/specs/.agent_memory/mcp_task_writer_output.md` | This file - coordination with next agent |

---

## Task Summary

### TASK-MCP-P0-001: Add Meta-UTL Tool Dispatch (P0 Critical)

**Gap Addressed**: 3 Meta-UTL tools are defined but NOT dispatched

**Tools Missing from Dispatch**:
1. `get_meta_learning_status` - Returns self-correction state, accuracy, lambdas
2. `trigger_lambda_recalibration` - Manually triggers gradient/Bayesian recalibration
3. `get_meta_learning_log` - Queries meta-learning event log

**Key Insight**: Tool definitions exist in `meta_utl.rs`, handler functions exist in `meta_learning.rs`, but:
- Tool name constants are MISSING from `names.rs`
- Dispatch cases are MISSING from `dispatch.rs`
- Calling these tools returns -32004 (TOOL_NOT_FOUND)

**Fix Required**:
1. Add 3 tool name constants to `names.rs`
2. Add 3 dispatch cases to `dispatch.rs` match statement
3. Create `call_*` wrapper methods for Handlers impl

**Files to Modify**:
- `crates/context-graph-mcp/src/tools/names.rs`
- `crates/context-graph-mcp/src/handlers/tools/dispatch.rs`

**Constitution Rules**: METAUTL-001 through METAUTL-005

---

### TASK-MCP-P1-001: Implement Tool Name Aliases (P1 High)

**Gap Addressed**: 2 tool name aliases are expected but NOT implemented

**Aliases Required**:
1. `discover_goals` -> `discover_sub_goals`
2. `consolidate_memories` -> `trigger_consolidation`

**Key Insight**: The canonical tools exist and dispatch correctly, but legacy names return -32004 (TOOL_NOT_FOUND)

**Fix Required**:
1. Create `aliases.rs` module with `ALIASES` map and `resolve_alias()` function
2. Wire `resolve_alias()` into `dispatch.rs` before match statement
3. Add tests verifying alias resolution

**Files to Create**:
- `crates/context-graph-mcp/src/tools/aliases.rs`

**Files to Modify**:
- `crates/context-graph-mcp/Cargo.toml` (add phf dependency, optional)
- `crates/context-graph-mcp/src/tools/mod.rs`
- `crates/context-graph-mcp/src/handlers/tools/dispatch.rs`

---

## Key Implementation Insights

### 1. Handler Functions Already Exist

The meta_learning.rs file contains COMPLETE handler implementations:
- `handle_get_meta_learning_status()` - Line 226
- `handle_trigger_lambda_recalibration()` - Line 280
- `handle_get_meta_learning_log()` - Line 317

These are pure functions that take input structs and return output structs.

### 2. MetaLearningService Pattern

`MetaLearningService` in `meta_utl_service.rs` is the facade for MCP handlers:
- Wraps `AdaptiveLambdaWeights`, `EscalationManager`, `MetaLearningEventLog`
- Can be created on-demand with `MetaLearningService::with_defaults()`
- For production, should be stored in Handlers struct (similar to dream_controller pattern)

### 3. Alias Resolution Design

Two options for alias implementation:

**Option A: phf (Perfect Hash Function)**
```rust
pub static ALIASES: phf::Map<&'static str, &'static str> = phf_map! {
    "discover_goals" => "discover_sub_goals",
    "consolidate_memories" => "trigger_consolidation",
};
```
- Compile-time computed
- O(1) lookup
- Requires phf dependency

**Option B: Simple Match**
```rust
pub fn resolve_alias(name: &str) -> &str {
    match name {
        "discover_goals" => "discover_sub_goals",
        "consolidate_memories" => "trigger_consolidation",
        other => other,
    }
}
```
- No dependencies
- Equally fast for 2 aliases
- Simpler to understand

Both are valid. The task spec documents Option A but notes Option B as alternative.

---

## What's NOT in Scope

These tasks modify ONLY the dispatch layer. They do NOT change:
- Tool definitions in `meta_utl.rs` (already correct)
- Handler logic in `meta_learning.rs` (already correct)
- MetaLearningService implementation (already correct)
- Other tool handlers
- Persistence layer

---

## Blockers Discovered

### No Major Blockers

Both tasks are **READY** with no hard dependencies:
- TASK-MCP-P0-001: Handler functions already exist, just need dispatch wiring
- TASK-MCP-P1-001: Canonical tools already dispatch, just need alias resolution

### Minor Note: MetaLearningService Wiring

The task spec notes that `MetaLearningService` is created on-demand with defaults.
For production, it SHOULD be stored in the Handlers struct and wired via constructor.
This is OUT OF SCOPE for TASK-MCP-P0-001 but recommended as future work.

---

## Recommended Execution Order

```
1. TASK-MCP-P0-001 (Meta-UTL Dispatch) - No dependencies
2. TASK-MCP-P1-001 (Aliases) - Can run in parallel, but logically P0 first
```

These tasks can be executed independently since they touch different code paths:
- P0 adds new dispatch cases
- P1 adds pre-dispatch alias resolution

---

## Verification Commands

```bash
# After TASK-MCP-P0-001
cargo test -p context-graph-mcp --lib tools::names -- --nocapture
cargo test -p context-graph-mcp --lib handlers::meta_learning -- --nocapture
cargo test -p context-graph-mcp meta_learning_status -- --nocapture

# After TASK-MCP-P1-001
cargo test -p context-graph-mcp --lib tools::aliases -- --nocapture
cargo test -p context-graph-mcp -- discover_goals --nocapture

# Verify no TOOL_NOT_FOUND for all 5 tools
cargo test -p context-graph-mcp 2>&1 | grep -c "32004"  # Should be 0 for these tools
```

---

## Reference Documents Used

- `/home/cabdru/contextgraph/docs/specs/_index.md` - Gap inventory
- `/home/cabdru/contextgraph/docs/specs/_traceability.md` - Requirements mapping
- `/home/cabdru/contextgraph/docs/specs/.agent_memory/architect_output.md` - Architecture context
- `/home/cabdru/contextgraph/docs/specs/.agent_memory/identity_task_writer_output.md` - Task format reference
- Source files in `crates/context-graph-mcp/src/`

---

## Source Code Analysis Summary

### names.rs (102 lines)
- Contains 32 tool name constants
- MISSING: GET_META_LEARNING_STATUS, TRIGGER_LAMBDA_RECALIBRATION, GET_META_LEARNING_LOG

### dispatch.rs (143 lines)
- Contains 32 dispatch cases in match statement
- MISSING: 3 Meta-UTL tool dispatch cases

### meta_utl.rs (175 lines)
- Contains 3 tool definitions (get_meta_learning_status, trigger_lambda_recalibration, get_meta_learning_log)
- All definitions are COMPLETE and tested

### meta_learning.rs (733 lines)
- Contains handler functions for 3 Meta-UTL tools
- All functions are COMPLETE with error handling and FSV tests

### meta_utl_service.rs (828 lines)
- Contains MetaLearningService facade
- Implements MetaLearningCallback trait for MetaCognitiveLoop integration

---

*MCP Dispatch Task Spec Writer Agent Complete*
*Format: prdtospec.md v1.0*

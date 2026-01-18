# Task 04: Add MCP Tool Name Constants

## Metadata
- **Task ID**: TASK-GAP-004
- **Phase**: 2 (MCP Infrastructure)
- **Priority**: High
- **Complexity**: Low
- **Estimated Time**: 30 minutes
- **Dependencies**: task01 (TASK-GAP-001 - tests must compile)

## Objective

Add the 6 PRD-required MCP tool name constants that are currently commented out with TODO markers in `names.rs`. These constants are needed for the tool dispatch system and must be defined before handlers can be implemented.

## Input Context

Files to READ before starting:
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/tools/names.rs` - Current tool name constants with TODOs
- `/home/cabdru/contextgraph/docs/TECH_SPEC_PRD_GAPS.md` - Section 5.1 for tool names specification

## Files to Create/Modify

**Files to Modify:**
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/tools/names.rs`

## Implementation Steps

### Step 1: Open names.rs and identify TODO section

The file currently has these commented lines (lines 21-27):
```rust
// TODO: Add these PRD-required tools:
// pub const GET_TOPIC_PORTFOLIO: &str = "get_topic_portfolio";
// pub const GET_TOPIC_STABILITY: &str = "get_topic_stability";
// pub const DETECT_TOPICS: &str = "detect_topics";
// pub const GET_DIVERGENCE_ALERTS: &str = "get_divergence_alerts";
// pub const FORGET_CONCEPT: &str = "forget_concept";
// pub const BOOST_IMPORTANCE: &str = "boost_importance";
```

### Step 2: Replace the TODO block with active constants

Remove the TODO comment block and add properly organized constants with section headers.

## Code/Content to Implement

### /home/cabdru/contextgraph/crates/context-graph-mcp/src/tools/names.rs

Replace the entire file with:

```rust
//! Tool names as constants for dispatch matching.
//!
//! Per PRD v6 Section 10, only these MCP tools should be exposed:
//! - Core: inject_context, search_graph, store_memory, get_memetic_status
//! - Topic: get_topic_portfolio, get_topic_stability, detect_topics, get_divergence_alerts
//! - Consolidation: trigger_consolidation
//! - Curation: merge_concepts, forget_concept, boost_importance

// ========== CORE TOOLS (PRD Section 10.1) ==========
pub const INJECT_CONTEXT: &str = "inject_context";
pub const STORE_MEMORY: &str = "store_memory";
pub const GET_MEMETIC_STATUS: &str = "get_memetic_status";
pub const SEARCH_GRAPH: &str = "search_graph";

// ========== CONSOLIDATION TOOLS (PRD Section 10.1) ==========
pub const TRIGGER_CONSOLIDATION: &str = "trigger_consolidation";

// ========== TOPIC TOOLS (PRD Section 10.2) ==========
pub const GET_TOPIC_PORTFOLIO: &str = "get_topic_portfolio";
pub const GET_TOPIC_STABILITY: &str = "get_topic_stability";
pub const DETECT_TOPICS: &str = "detect_topics";
pub const GET_DIVERGENCE_ALERTS: &str = "get_divergence_alerts";

// ========== CURATION TOOLS (PRD Section 10.3) ==========
pub const MERGE_CONCEPTS: &str = "merge_concepts";
pub const FORGET_CONCEPT: &str = "forget_concept";
pub const BOOST_IMPORTANCE: &str = "boost_importance";
```

## Definition of Done

- [ ] File contains all 12 tool name constants (6 existing + 6 new)
- [ ] No TODO comments remain for tool definitions
- [ ] Constants are organized by category with section headers
- [ ] Constant names use SCREAMING_SNAKE_CASE per constitution
- [ ] String values use snake_case per constitution
- [ ] `cargo check -p context-graph-mcp` passes
- [ ] `cargo clippy -p context-graph-mcp -- -D warnings` passes

## Verification

```bash
cd /home/cabdru/contextgraph

# Verify file compiles
cargo check -p context-graph-mcp

# Verify no clippy warnings
cargo clippy -p context-graph-mcp -- -D warnings

# Verify all 12 constants are defined
grep "pub const" crates/context-graph-mcp/src/tools/names.rs | wc -l
# Expected: 12

# Verify new topic tools are defined
grep -E "GET_TOPIC_PORTFOLIO|GET_TOPIC_STABILITY|DETECT_TOPICS|GET_DIVERGENCE_ALERTS" \
    crates/context-graph-mcp/src/tools/names.rs
# Should show 4 lines

# Verify new curation tools are defined
grep -E "FORGET_CONCEPT|BOOST_IMPORTANCE" crates/context-graph-mcp/src/tools/names.rs
# Should show 2 lines

# Verify no TODO comments remain
grep -i "TODO" crates/context-graph-mcp/src/tools/names.rs
# Should return empty
```

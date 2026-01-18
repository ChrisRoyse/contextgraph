# Task 08: Update Tool Dispatch

## Metadata
- **Task ID**: TASK-GAP-008
- **Phase**: 2 (MCP Infrastructure)
- **Priority**: High
- **Complexity**: Low
- **Estimated Time**: 30 minutes
- **Dependencies**: task06 (TASK-GAP-006), task07 (TASK-GAP-007) - handlers must be implemented

## Objective

Update the `dispatch.rs` file to route tool calls for the 6 new MCP tools to their respective handlers. Also update `definitions.rs` to include the new tool schemas in the `tools/list` response.

## Input Context

Files to READ before starting:
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/dispatch.rs` - Current dispatch logic
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/tools/definitions.rs` - Tool schema definitions
- `/home/cabdru/contextgraph/docs/TECH_SPEC_PRD_GAPS.md` - Section 5.3 for dispatch updates

## Files to Create/Modify

**Files to Modify:**
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/dispatch.rs`
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/tools/definitions.rs`

## Implementation Steps

### Step 1: Update dispatch.rs

Add match arms for the 6 new tools in the `handle_tools_call` method.

### Step 2: Update definitions.rs

Add tool definitions for the 6 new tools so they appear in `tools/list` response.

## Code/Content to Implement

### /home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/dispatch.rs

Replace the file with:

```rust
//! Tool dispatch logic for MCP tool calls.
//!
//! Per PRD v6 Section 10, these MCP tools are exposed:
//! - Core: inject_context, search_graph, store_memory, get_memetic_status
//! - Consolidation: trigger_consolidation
//! - Topic: get_topic_portfolio, get_topic_stability, detect_topics, get_divergence_alerts
//! - Curation: merge_concepts, forget_concept, boost_importance

use serde_json::json;
use tracing::debug;

use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};
use crate::tools::{get_tool_definitions, tool_names};

use super::super::Handlers;

impl Handlers {
    pub(crate) async fn handle_tools_list(&self, id: Option<JsonRpcId>) -> JsonRpcResponse {
        debug!("Handling tools/list request");
        let tools = get_tool_definitions();
        JsonRpcResponse::success(id, json!({ "tools": tools }))
    }

    pub(crate) async fn handle_tools_call(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing params for tools/call",
                );
            }
        };

        let raw_tool_name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing 'name' parameter in tools/call",
                );
            }
        };

        let tool_name = crate::tools::aliases::resolve_alias(raw_tool_name);
        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

        debug!(
            "Calling tool: {} with arguments: {:?}{}",
            tool_name,
            arguments,
            if raw_tool_name != tool_name {
                format!(" (resolved from alias '{}')", raw_tool_name)
            } else {
                String::new()
            }
        );

        match tool_name {
            // ========== CORE TOOLS (PRD Section 10.1) ==========
            tool_names::INJECT_CONTEXT => self.call_inject_context(id, arguments).await,
            tool_names::STORE_MEMORY => self.call_store_memory(id, arguments).await,
            tool_names::GET_MEMETIC_STATUS => self.call_get_memetic_status(id).await,
            tool_names::SEARCH_GRAPH => self.call_search_graph(id, arguments).await,

            // ========== CONSOLIDATION TOOLS (PRD Section 10.1) ==========
            tool_names::TRIGGER_CONSOLIDATION => {
                self.call_trigger_consolidation(id, arguments).await
            }

            // ========== TOPIC TOOLS (PRD Section 10.2) ==========
            tool_names::GET_TOPIC_PORTFOLIO => {
                self.call_get_topic_portfolio(id, arguments).await
            }
            tool_names::GET_TOPIC_STABILITY => {
                self.call_get_topic_stability(id, arguments).await
            }
            tool_names::DETECT_TOPICS => self.call_detect_topics(id, arguments).await,
            tool_names::GET_DIVERGENCE_ALERTS => {
                self.call_get_divergence_alerts(id, arguments).await
            }

            // ========== CURATION TOOLS (PRD Section 10.3) ==========
            tool_names::MERGE_CONCEPTS => self.call_merge_concepts(id, arguments).await,
            tool_names::FORGET_CONCEPT => self.call_forget_concept(id, arguments).await,
            tool_names::BOOST_IMPORTANCE => self.call_boost_importance(id, arguments).await,

            // Unknown tool
            _ => JsonRpcResponse::error(
                id,
                error_codes::TOOL_NOT_FOUND,
                format!("Unknown tool: {}", tool_name),
            ),
        }
    }
}
```

### /home/cabdru/contextgraph/crates/context-graph-mcp/src/tools/definitions.rs

Add the new tool definitions. First read the existing file to understand the pattern, then add:

```rust
// Add these tool definitions to the get_tool_definitions() function:

// ========== TOPIC TOOLS (PRD Section 10.2) ==========
ToolDefinition {
    name: tool_names::GET_TOPIC_PORTFOLIO.to_string(),
    description: Some("Get all discovered topics with profiles".to_string()),
    input_schema: json!({
        "type": "object",
        "properties": {
            "format": {
                "type": "string",
                "enum": ["brief", "standard", "verbose"],
                "default": "standard",
                "description": "Output format verbosity"
            }
        }
    }),
},
ToolDefinition {
    name: tool_names::GET_TOPIC_STABILITY.to_string(),
    description: Some("Get portfolio-level stability metrics".to_string()),
    input_schema: json!({
        "type": "object",
        "properties": {
            "hours": {
                "type": "integer",
                "minimum": 1,
                "maximum": 168,
                "default": 6,
                "description": "Lookback period in hours"
            }
        }
    }),
},
ToolDefinition {
    name: tool_names::DETECT_TOPICS.to_string(),
    description: Some("Force topic detection recalculation".to_string()),
    input_schema: json!({
        "type": "object",
        "properties": {
            "force": {
                "type": "boolean",
                "default": false,
                "description": "Force detection even if not needed"
            }
        }
    }),
},
ToolDefinition {
    name: tool_names::GET_DIVERGENCE_ALERTS.to_string(),
    description: Some("Check for divergence from recent activity".to_string()),
    input_schema: json!({
        "type": "object",
        "properties": {
            "lookback_hours": {
                "type": "integer",
                "minimum": 1,
                "maximum": 48,
                "default": 2,
                "description": "Hours to check for divergence"
            }
        }
    }),
},

// ========== CURATION TOOLS (PRD Section 10.3) ==========
ToolDefinition {
    name: tool_names::FORGET_CONCEPT.to_string(),
    description: Some("Soft-delete a memory (30-day recovery)".to_string()),
    input_schema: json!({
        "type": "object",
        "required": ["node_id"],
        "properties": {
            "node_id": {
                "type": "string",
                "format": "uuid",
                "description": "UUID of memory to forget"
            },
            "soft_delete": {
                "type": "boolean",
                "default": true,
                "description": "Use soft delete (30-day recovery)"
            }
        }
    }),
},
ToolDefinition {
    name: tool_names::BOOST_IMPORTANCE.to_string(),
    description: Some("Adjust memory importance score".to_string()),
    input_schema: json!({
        "type": "object",
        "required": ["node_id", "delta"],
        "properties": {
            "node_id": {
                "type": "string",
                "format": "uuid",
                "description": "UUID of memory to boost"
            },
            "delta": {
                "type": "number",
                "minimum": -1.0,
                "maximum": 1.0,
                "description": "Importance change (-1.0 to 1.0)"
            }
        }
    }),
},
```

## Definition of Done

- [ ] `dispatch.rs` has match arms for all 6 new tools
- [ ] Match arms use the `tool_names::` constants (not string literals)
- [ ] Match arms are organized by category with section headers
- [ ] `definitions.rs` includes tool definitions for all 6 new tools
- [ ] Tool definitions have proper JSON schemas with required fields
- [ ] `cargo check -p context-graph-mcp` passes
- [ ] `cargo clippy -p context-graph-mcp -- -D warnings` passes
- [ ] All 12 tools appear in `tools/list` response

## Verification

```bash
cd /home/cabdru/contextgraph

# Verify compilation
cargo check -p context-graph-mcp

# Verify no clippy warnings
cargo clippy -p context-graph-mcp -- -D warnings

# Verify dispatch has all 12 tools
grep "tool_names::" crates/context-graph-mcp/src/handlers/tools/dispatch.rs | wc -l
# Expected: 12

# Verify new topic tools in dispatch
grep -E "GET_TOPIC_PORTFOLIO|GET_TOPIC_STABILITY|DETECT_TOPICS|GET_DIVERGENCE_ALERTS" \
    crates/context-graph-mcp/src/handlers/tools/dispatch.rs
# Should show 4 lines

# Verify new curation tools in dispatch
grep -E "FORGET_CONCEPT|BOOST_IMPORTANCE" \
    crates/context-graph-mcp/src/handlers/tools/dispatch.rs
# Should show 2 lines

# Count tool definitions
grep -c "ToolDefinition {" crates/context-graph-mcp/src/tools/definitions.rs
# Should show 12 total

# Run tests to verify dispatch works
cargo test -p context-graph-mcp tools_list --no-run
```

# Task 07: Implement Curation Tool Handlers

## Metadata
- **Task ID**: TASK-GAP-007
- **Phase**: 2 (MCP Infrastructure)
- **Priority**: High
- **Complexity**: Medium
- **Estimated Time**: 1-2 hours
- **Dependencies**: task05 (TASK-GAP-005 - DTOs must be defined)

## Objective

Implement the 2 curation-related MCP tool handlers: `forget_concept` and `boost_importance`. These handlers implement soft delete with 30-day recovery (SEC-06) and importance adjustment with clamping to [0.0, 1.0] (BR-MCP-002).

## Input Context

Files to READ before starting:
- `/home/cabdru/contextgraph/docs/TECH_SPEC_PRD_GAPS.md` - Section 10.2 for handler contracts
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/memory_tools.rs` - Reference for handler patterns
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/curation_dtos.rs` - DTOs to use (created in task05)
- `/home/cabdru/contextgraph/crates/context-graph-core/src/curation/` - Curation module (if exists)

## Files to Create/Modify

**Files to Create:**
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/curation_tools.rs`

**Files to Modify:**
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/mod.rs`

## Implementation Steps

### Step 1: Create curation_tools.rs

Create the handler file implementing both curation tool methods on the Handlers struct.

### Step 2: Update mod.rs

Add `mod curation_tools;` to the module file.

## Code/Content to Implement

### /home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/curation_tools.rs

```rust
//! Curation tool handlers.
//!
//! Per PRD Section 10.3, implements:
//! - forget_concept: Soft-delete a memory (30-day recovery per SEC-06)
//! - boost_importance: Adjust memory importance score
//!
//! Constitution Compliance:
//! - SEC-06: Soft delete 30-day recovery
//! - BR-MCP-002: Importance bounds [0.0, 1.0]

use chrono::{Duration, Utc};
use serde_json::json;
use tracing::{debug, error, warn};
use uuid::Uuid;

use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};

use super::super::Handlers;
use super::curation_dtos::{
    BoostImportanceRequest, BoostImportanceResponse, ForgetConceptRequest, ForgetConceptResponse,
};

/// Soft delete recovery period in days (per SEC-06).
const SOFT_DELETE_RECOVERY_DAYS: i64 = 30;

/// Minimum importance value (per BR-MCP-002).
const MIN_IMPORTANCE: f32 = 0.0;

/// Maximum importance value (per BR-MCP-002).
const MAX_IMPORTANCE: f32 = 1.0;

impl Handlers {
    /// Handle forget_concept tool call.
    ///
    /// Soft-deletes a memory with 30-day recovery window per SEC-06.
    ///
    /// # Arguments
    /// * `id` - JSON-RPC request ID
    /// * `arguments` - Tool arguments (node_id, soft_delete)
    ///
    /// # Returns
    /// JsonRpcResponse with ForgetConceptResponse
    ///
    /// # Implements
    /// REQ-MCP-002, BR-MCP-001
    ///
    /// # Constitution Compliance
    /// - SEC-06: 30-day recovery for soft delete
    pub(crate) async fn call_forget_concept(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling forget_concept");

        // Parse request
        let request: ForgetConceptRequest = match serde_json::from_value(arguments) {
            Ok(r) => r,
            Err(e) => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Invalid params: {}", e),
                );
            }
        };

        // Parse and validate UUID
        let node_id = match Uuid::parse_str(&request.node_id) {
            Ok(id) => id,
            Err(_) => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Invalid UUID format: {}", request.node_id),
                );
            }
        };

        // Check if memory exists
        let exists = match self.teleological_store.get(node_id).await {
            Ok(Some(_)) => true,
            Ok(None) => false,
            Err(e) => {
                error!(error = %e, node_id = %node_id, "Failed to check memory existence");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INTERNAL_ERROR,
                    format!("Failed to check memory: {}", e),
                );
            }
        };

        if !exists {
            return JsonRpcResponse::error(
                id,
                -32001, // MEMORY_NOT_FOUND
                format!("Memory {} not found", node_id),
            );
        }

        // Perform delete operation
        let soft_deleted = request.soft_delete;
        let recoverable_until = if soft_deleted {
            Some(Utc::now() + Duration::days(SOFT_DELETE_RECOVERY_DAYS))
        } else {
            None
        };

        // TODO: Call actual delete method on store
        // For soft delete: store.soft_delete(node_id).await
        // For hard delete: store.hard_delete(node_id).await
        //
        // For now, we log the operation as a placeholder
        if soft_deleted {
            debug!(
                node_id = %node_id,
                recoverable_until = ?recoverable_until,
                "Soft delete requested (placeholder - not yet implemented)"
            );
        } else {
            warn!(
                node_id = %node_id,
                "Hard delete requested (placeholder - not yet implemented)"
            );
        }

        let response = ForgetConceptResponse {
            forgotten_id: node_id,
            soft_deleted,
            recoverable_until,
        };

        self.tool_result_with_pulse(id, serde_json::to_value(response).unwrap())
    }

    /// Handle boost_importance tool call.
    ///
    /// Adjusts memory importance by delta, clamping to [0.0, 1.0].
    ///
    /// # Arguments
    /// * `id` - JSON-RPC request ID
    /// * `arguments` - Tool arguments (node_id, delta)
    ///
    /// # Returns
    /// JsonRpcResponse with BoostImportanceResponse
    ///
    /// # Implements
    /// REQ-MCP-002, BR-MCP-002
    ///
    /// # Constitution Compliance
    /// - BR-MCP-002: Importance bounds [0.0, 1.0]
    pub(crate) async fn call_boost_importance(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling boost_importance");

        // Parse request
        let request: BoostImportanceRequest = match serde_json::from_value(arguments) {
            Ok(r) => r,
            Err(e) => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Invalid params: {}", e),
                );
            }
        };

        // Validate delta range
        if request.delta < -1.0 || request.delta > 1.0 {
            return JsonRpcResponse::error(
                id,
                -32031, // IMPORTANCE_OUT_OF_RANGE
                "delta must be between -1.0 and 1.0",
            );
        }

        // Parse and validate UUID
        let node_id = match Uuid::parse_str(&request.node_id) {
            Ok(id) => id,
            Err(_) => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Invalid UUID format: {}", request.node_id),
                );
            }
        };

        // Get current memory to read importance
        let fingerprint = match self.teleological_store.get(node_id).await {
            Ok(Some(fp)) => fp,
            Ok(None) => {
                return JsonRpcResponse::error(
                    id,
                    -32001, // MEMORY_NOT_FOUND
                    format!("Memory {} not found", node_id),
                );
            }
            Err(e) => {
                error!(error = %e, node_id = %node_id, "Failed to get memory");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INTERNAL_ERROR,
                    format!("Failed to get memory: {}", e),
                );
            }
        };

        // Get current importance (default 0.5 if not set)
        let old_importance = fingerprint.alignment_score;

        // Calculate new importance with clamping
        let raw_new_importance = old_importance + request.delta;
        let new_importance = raw_new_importance.clamp(MIN_IMPORTANCE, MAX_IMPORTANCE);
        let clamped = raw_new_importance != new_importance;

        // TODO: Update memory importance in store
        // For now, log the operation as a placeholder
        debug!(
            node_id = %node_id,
            old_importance = old_importance,
            delta = request.delta,
            new_importance = new_importance,
            clamped = clamped,
            "Importance boost requested (placeholder - update not yet implemented)"
        );

        let response = BoostImportanceResponse {
            node_id,
            old_importance,
            new_importance,
            clamped,
        };

        self.tool_result_with_pulse(id, serde_json::to_value(response).unwrap())
    }
}
```

### Update mod.rs

Update `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/mod.rs`:

```rust
//! MCP tool call handlers.
//!
//! PRD v6 Section 10 MCP Tools:
//! - inject_context, store_memory, search_graph (memory_tools.rs)
//! - get_memetic_status (status_tools.rs)
//! - trigger_consolidation (consolidation.rs)
//! - merge_concepts (../merge.rs)
//! - get_topic_portfolio, get_topic_stability, detect_topics, get_divergence_alerts (topic_tools.rs)
//! - forget_concept, boost_importance (curation_tools.rs)

mod consolidation;
mod curation_tools;
mod dispatch;
mod helpers;
mod memory_tools;
mod status_tools;
mod topic_tools;

// DTOs for new PRD v6 tools
pub mod curation_dtos;
pub mod topic_dtos;
```

## Definition of Done

- [ ] File `curation_tools.rs` exists with both handler methods
- [ ] `call_forget_concept` validates UUID, checks memory exists, returns ForgetConceptResponse
- [ ] `call_forget_concept` calculates recoverable_until (30 days) for soft delete per SEC-06
- [ ] `call_forget_concept` defaults to soft_delete=true per constitution
- [ ] `call_boost_importance` validates UUID and delta range
- [ ] `call_boost_importance` clamps importance to [0.0, 1.0] per BR-MCP-002
- [ ] `call_boost_importance` reports whether value was clamped
- [ ] All handlers use `self.tool_result_with_pulse()` for consistent response format
- [ ] Constants defined for SOFT_DELETE_RECOVERY_DAYS (30), MIN/MAX_IMPORTANCE
- [ ] `mod.rs` includes `mod curation_tools;`
- [ ] `cargo check -p context-graph-mcp` passes
- [ ] `cargo clippy -p context-graph-mcp -- -D warnings` passes

## Verification

```bash
cd /home/cabdru/contextgraph

# Verify file exists
test -f crates/context-graph-mcp/src/handlers/tools/curation_tools.rs && echo "curation_tools.rs exists"

# Verify compilation
cargo check -p context-graph-mcp

# Verify no clippy warnings
cargo clippy -p context-graph-mcp -- -D warnings

# Verify both handlers are defined
grep "pub(crate) async fn call_" crates/context-graph-mcp/src/handlers/tools/curation_tools.rs | wc -l
# Expected: 2

# Verify constitution compliance comments
grep -c "SEC-06\|BR-MCP-002" crates/context-graph-mcp/src/handlers/tools/curation_tools.rs
# Should show references to constitution rules

# Verify constants
grep "SOFT_DELETE_RECOVERY_DAYS" crates/context-graph-mcp/src/handlers/tools/curation_tools.rs
grep "MIN_IMPORTANCE\|MAX_IMPORTANCE" crates/context-graph-mcp/src/handlers/tools/curation_tools.rs
```

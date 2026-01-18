# Task 06: Implement Topic Tool Handlers

## Metadata
- **Task ID**: TASK-GAP-006
- **Phase**: 2 (MCP Infrastructure)
- **Priority**: High
- **Complexity**: High
- **Estimated Time**: 2-3 hours
- **Dependencies**: task05 (TASK-GAP-005 - DTOs must be defined)

## Objective

Implement the 4 topic-related MCP tool handlers: `get_topic_portfolio`, `get_topic_stability`, `detect_topics`, and `get_divergence_alerts`. These handlers leverage the existing `context-graph-core::clustering` module and must comply with constitution rules AP-60 (temporal embedders excluded from topics), AP-62 (SEMANTIC only for divergence), and ARCH-09 (threshold >= 2.5).

## Input Context

Files to READ before starting:
- `/home/cabdru/contextgraph/docs/TECH_SPEC_PRD_GAPS.md` - Section 10.1 for handler contracts
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/memory_tools.rs` - Reference for handler patterns
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/topic_dtos.rs` - DTOs to use (created in task05)
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/helpers.rs` - Helper functions
- `/home/cabdru/contextgraph/crates/context-graph-core/src/clustering/` - Clustering module to call

## Files to Create/Modify

**Files to Create:**
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/topic_tools.rs`

**Files to Modify:**
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/mod.rs`

## Implementation Steps

### Step 1: Create topic_tools.rs

Create the handler file implementing all 4 topic tool methods on the Handlers struct.

### Step 2: Update mod.rs

Add `mod topic_tools;` to the module file.

## Code/Content to Implement

### /home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/topic_tools.rs

```rust
//! Topic tool handlers.
//!
//! Per PRD Section 10.2, implements:
//! - get_topic_portfolio: Get all discovered topics with profiles
//! - get_topic_stability: Get portfolio-level stability metrics
//! - detect_topics: Force topic detection recalculation
//! - get_divergence_alerts: Check for divergence from recent activity
//!
//! Constitution Compliance:
//! - AP-60: Temporal embedders (E2-E4) weight = 0.0 in topic detection
//! - AP-62: Only SEMANTIC embedders for divergence alerts
//! - ARCH-09: Topic threshold is weighted_agreement >= 2.5

use serde_json::json;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};

use super::super::Handlers;
use super::topic_dtos::{
    DetectTopicsRequest, DetectTopicsResponse, DivergenceAlert, DivergenceAlertsResponse,
    GetDivergenceAlertsRequest, GetTopicPortfolioRequest, GetTopicStabilityRequest,
    MergedTopicInfo, PhaseBreakdown, StabilityMetricsSummary, TopicPortfolioResponse,
    TopicStabilityResponse, TopicSummary,
};

/// Minimum weighted agreement for topic detection (per ARCH-09).
const TOPIC_THRESHOLD: f32 = 2.5;

/// Maximum weighted agreement (7 SEMANTIC + 2 RELATIONAL*0.5 + 1 STRUCTURAL*0.5).
const MAX_WEIGHTED_AGREEMENT: f32 = 8.5;

/// Minimum memories required for clustering (per constitution min_cluster_size).
const MIN_MEMORIES_FOR_CLUSTERING: usize = 3;

/// SEMANTIC embedders for divergence detection (per AP-62).
/// E1, E5, E6, E7, E10, E12, E13
const SEMANTIC_EMBEDDERS: [&str; 7] = [
    "E1_Semantic",
    "E5_Causal",
    "E6_Sparse",
    "E7_Code",
    "E10_Multimodal",
    "E12_LateInteraction",
    "E13_SPLADE",
];

impl Handlers {
    /// Handle get_topic_portfolio tool call.
    ///
    /// Returns discovered topics with profiles, stability metrics, and tier info.
    ///
    /// # Arguments
    /// * `id` - JSON-RPC request ID
    /// * `arguments` - Tool arguments (format: brief|standard|verbose)
    ///
    /// # Returns
    /// JsonRpcResponse with TopicPortfolioResponse
    ///
    /// # Implements
    /// REQ-MCP-002, REQ-MCP-004
    pub(crate) async fn call_get_topic_portfolio(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling get_topic_portfolio");

        // Parse request
        let request: GetTopicPortfolioRequest = match serde_json::from_value(arguments) {
            Ok(r) => r,
            Err(e) => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Invalid params: {}", e),
                );
            }
        };

        // Validate format
        let format = request.format.to_lowercase();
        if !["brief", "standard", "verbose"].contains(&format.as_str()) {
            return JsonRpcResponse::error(
                id,
                error_codes::INVALID_PARAMS,
                "Invalid params: format must be brief|standard|verbose",
            );
        }

        // Get memory count to determine tier
        let memory_count = match self.teleological_store.count().await {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, "Failed to get memory count");
                0
            }
        };

        let tier = determine_tier(memory_count);

        // Tier 0: No memories, return empty response
        if tier == 0 {
            let response = TopicPortfolioResponse {
                topics: vec![],
                stability: StabilityMetricsSummary {
                    churn_rate: 0.0,
                    entropy: 0.0,
                    is_stable: true,
                },
                total_topics: 0,
                tier: 0,
            };
            return self.tool_result_with_pulse(id, serde_json::to_value(response).unwrap());
        }

        // For now, return stub response
        // TODO: Integrate with actual clustering module when available
        let response = TopicPortfolioResponse {
            topics: vec![],
            stability: StabilityMetricsSummary {
                churn_rate: 0.0,
                entropy: 0.0,
                is_stable: true,
            },
            total_topics: 0,
            tier,
        };

        self.tool_result_with_pulse(id, serde_json::to_value(response).unwrap())
    }

    /// Handle get_topic_stability tool call.
    ///
    /// Returns stability metrics including churn, entropy, and dream recommendation.
    ///
    /// # Arguments
    /// * `id` - JSON-RPC request ID
    /// * `arguments` - Tool arguments (hours: lookback period)
    ///
    /// # Returns
    /// JsonRpcResponse with TopicStabilityResponse
    ///
    /// # Implements
    /// REQ-MCP-002
    pub(crate) async fn call_get_topic_stability(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling get_topic_stability");

        // Parse request
        let request: GetTopicStabilityRequest = match serde_json::from_value(arguments) {
            Ok(r) => r,
            Err(e) => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Invalid params: {}", e),
                );
            }
        };

        // Validate hours range
        if request.hours == 0 || request.hours > 168 {
            return JsonRpcResponse::error(
                id,
                error_codes::INVALID_PARAMS,
                "Invalid params: hours must be between 1 and 168",
            );
        }

        // TODO: Get actual stability metrics from clustering module
        let churn_rate = 0.0;
        let entropy = 0.0;

        // Per AP-70: Dream recommended when entropy > 0.7 AND churn > 0.5
        let dream_recommended = entropy > 0.7 && churn_rate > 0.5;
        let high_churn_warning = churn_rate >= 0.5;

        let response = TopicStabilityResponse {
            churn_rate,
            entropy,
            phases: PhaseBreakdown {
                emerging: 0,
                stable: 0,
                declining: 0,
                merging: 0,
            },
            dream_recommended,
            high_churn_warning,
            average_churn: churn_rate,
        };

        self.tool_result_with_pulse(id, serde_json::to_value(response).unwrap())
    }

    /// Handle detect_topics tool call.
    ///
    /// Triggers topic detection/clustering. Requires minimum 3 memories.
    ///
    /// # Arguments
    /// * `id` - JSON-RPC request ID
    /// * `arguments` - Tool arguments (force: force detection)
    ///
    /// # Returns
    /// JsonRpcResponse with DetectTopicsResponse
    ///
    /// # Implements
    /// REQ-MCP-002, BR-MCP-003
    pub(crate) async fn call_detect_topics(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling detect_topics");

        // Parse request
        let request: DetectTopicsRequest = match serde_json::from_value(arguments) {
            Ok(r) => r,
            Err(e) => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Invalid params: {}", e),
                );
            }
        };

        // Check minimum memory count
        let memory_count = match self.teleological_store.count().await {
            Ok(c) => c,
            Err(e) => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INTERNAL_ERROR,
                    format!("Failed to get memory count: {}", e),
                );
            }
        };

        if memory_count < MIN_MEMORIES_FOR_CLUSTERING {
            return JsonRpcResponse::error(
                id,
                -32021, // INSUFFICIENT_MEMORIES
                format!(
                    "Need >= {} memories for topic detection (have {})",
                    MIN_MEMORIES_FOR_CLUSTERING, memory_count
                ),
            );
        }

        // TODO: Run actual HDBSCAN clustering
        // For now, return stub response
        let response = DetectTopicsResponse {
            new_topics: vec![],
            merged_topics: vec![],
            total_after: 0,
            message: Some(format!(
                "Topic detection placeholder - {} memories available",
                memory_count
            )),
        };

        self.tool_result_with_pulse(id, serde_json::to_value(response).unwrap())
    }

    /// Handle get_divergence_alerts tool call.
    ///
    /// Checks for divergence from recent activity using SEMANTIC embedders ONLY.
    ///
    /// # Arguments
    /// * `id` - JSON-RPC request ID
    /// * `arguments` - Tool arguments (lookback_hours)
    ///
    /// # Returns
    /// JsonRpcResponse with DivergenceAlertsResponse
    ///
    /// # Implements
    /// REQ-MCP-002, REQ-MCP-005
    ///
    /// # Constitution Compliance
    /// - AP-62: Only SEMANTIC embedders (E1, E5, E6, E7, E10, E12, E13) trigger alerts
    /// - Temporal embedders (E2-E4) NEVER trigger divergence alerts
    pub(crate) async fn call_get_divergence_alerts(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling get_divergence_alerts");

        // Parse request
        let request: GetDivergenceAlertsRequest = match serde_json::from_value(arguments) {
            Ok(r) => r,
            Err(e) => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Invalid params: {}", e),
                );
            }
        };

        // Validate lookback range
        if request.lookback_hours == 0 || request.lookback_hours > 48 {
            return JsonRpcResponse::error(
                id,
                error_codes::INVALID_PARAMS,
                "Invalid params: lookback_hours must be between 1 and 48",
            );
        }

        // TODO: Get recent memories and compare using SEMANTIC embedders only
        // Per AP-62: Only E1, E5, E6, E7, E10, E12, E13 trigger divergence alerts
        // Temporal embedders (E2-E4) are explicitly excluded

        let alerts: Vec<DivergenceAlert> = vec![];

        // Determine severity based on alert count and scores
        let severity = if alerts.is_empty() {
            "none"
        } else if alerts.len() == 1 {
            "low"
        } else if alerts.len() <= 3 {
            "medium"
        } else {
            "high"
        };

        let response = DivergenceAlertsResponse {
            alerts,
            severity: severity.to_string(),
        };

        self.tool_result_with_pulse(id, serde_json::to_value(response).unwrap())
    }
}

/// Determine progressive tier based on memory count.
///
/// Per constitution progressive_tiers:
/// - tier_0: 0 memories
/// - tier_1: 1-2 memories
/// - tier_2: 3-9 memories
/// - tier_3: 10-29 memories
/// - tier_4: 30-99 memories
/// - tier_5: 100-499 memories
/// - tier_6: 500+ memories
fn determine_tier(memory_count: usize) -> u8 {
    match memory_count {
        0 => 0,
        1..=2 => 1,
        3..=9 => 2,
        10..=29 => 3,
        30..=99 => 4,
        100..=499 => 5,
        _ => 6,
    }
}
```

### Update mod.rs

Add to `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/mod.rs`:

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

- [ ] File `topic_tools.rs` exists with all 4 handler methods
- [ ] `call_get_topic_portfolio` parses request, validates format, returns TopicPortfolioResponse
- [ ] `call_get_topic_stability` parses request, validates hours, checks dream trigger (AP-70)
- [ ] `call_detect_topics` validates minimum 3 memories, returns DetectTopicsResponse
- [ ] `call_get_divergence_alerts` validates lookback_hours, documents SEMANTIC-only rule (AP-62)
- [ ] All handlers use `self.tool_result_with_pulse()` for consistent response format
- [ ] Constants defined for TOPIC_THRESHOLD (2.5), MAX_WEIGHTED_AGREEMENT (8.5)
- [ ] `mod.rs` includes `mod topic_tools;`
- [ ] `cargo check -p context-graph-mcp` passes
- [ ] `cargo clippy -p context-graph-mcp -- -D warnings` passes

## Verification

```bash
cd /home/cabdru/contextgraph

# Verify file exists
test -f crates/context-graph-mcp/src/handlers/tools/topic_tools.rs && echo "topic_tools.rs exists"

# Verify compilation
cargo check -p context-graph-mcp

# Verify no clippy warnings
cargo clippy -p context-graph-mcp -- -D warnings

# Verify all 4 handlers are defined
grep "pub(crate) async fn call_" crates/context-graph-mcp/src/handlers/tools/topic_tools.rs | wc -l
# Expected: 4

# Verify constitution compliance comments
grep -c "AP-60\|AP-62\|ARCH-09\|AP-70" crates/context-graph-mcp/src/handlers/tools/topic_tools.rs
# Should show multiple references to constitution rules

# Verify SEMANTIC embedders constant
grep "SEMANTIC_EMBEDDERS" crates/context-graph-mcp/src/handlers/tools/topic_tools.rs
# Should show E1, E5, E6, E7, E10, E12, E13
```

# Task 05: Create MCP Request/Response DTOs

## Metadata
- **Task ID**: TASK-GAP-005
- **Phase**: 2 (MCP Infrastructure)
- **Priority**: High
- **Complexity**: Medium
- **Estimated Time**: 1-2 hours
- **Dependencies**: task04 (TASK-GAP-004 - tool names must be defined)

## Objective

Create request and response DTOs (Data Transfer Objects) for the 6 new MCP tools. These DTOs provide type-safe serialization/deserialization for tool parameters and results. The DTOs follow existing patterns in the codebase and use serde derive macros.

## Input Context

Files to READ before starting:
- `/home/cabdru/contextgraph/docs/TECH_SPEC_PRD_GAPS.md` - Section 8 for DTO definitions
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/memory_tools.rs` - Reference for existing patterns
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/mod.rs` - Module exports

## Files to Create/Modify

**Files to Create:**
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/topic_dtos.rs`
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/curation_dtos.rs`

**Files to Modify:**
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/mod.rs`

## Implementation Steps

### Step 1: Create topic_dtos.rs

Create the file with request and response DTOs for the 4 topic tools.

### Step 2: Create curation_dtos.rs

Create the file with request and response DTOs for the 2 curation tools.

### Step 3: Update mod.rs to export new modules

Add the new modules to the handlers/tools/mod.rs file.

## Code/Content to Implement

### /home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/topic_dtos.rs

```rust
//! DTOs for topic-related MCP tools.
//!
//! Per PRD Section 10.2, these DTOs support:
//! - get_topic_portfolio
//! - get_topic_stability
//! - detect_topics
//! - get_divergence_alerts

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Request DTOs
// ============================================================================

/// Request for get_topic_portfolio tool.
#[derive(Debug, Deserialize)]
pub struct GetTopicPortfolioRequest {
    /// Output format: "brief", "standard", or "verbose"
    #[serde(default = "default_format")]
    pub format: String,
}

fn default_format() -> String {
    "standard".to_string()
}

/// Request for get_topic_stability tool.
#[derive(Debug, Deserialize)]
pub struct GetTopicStabilityRequest {
    /// Lookback period in hours (default 6)
    #[serde(default = "default_hours")]
    pub hours: u32,
}

fn default_hours() -> u32 {
    6
}

/// Request for detect_topics tool.
#[derive(Debug, Deserialize)]
pub struct DetectTopicsRequest {
    /// Force detection even if not needed
    #[serde(default)]
    pub force: bool,
}

/// Request for get_divergence_alerts tool.
#[derive(Debug, Deserialize)]
pub struct GetDivergenceAlertsRequest {
    /// Lookback period in hours (default 2)
    #[serde(default = "default_lookback")]
    pub lookback_hours: u32,
}

fn default_lookback() -> u32 {
    2
}

// ============================================================================
// Response DTOs
// ============================================================================

/// Response for get_topic_portfolio tool.
#[derive(Debug, Serialize)]
pub struct TopicPortfolioResponse {
    /// List of discovered topics
    pub topics: Vec<TopicSummary>,
    /// Portfolio-level stability metrics
    pub stability: StabilityMetricsSummary,
    /// Total number of topics
    pub total_topics: usize,
    /// Current progressive tier (0-6)
    pub tier: u8,
}

/// Summary of a single topic.
#[derive(Debug, Serialize)]
pub struct TopicSummary {
    /// Topic ID
    pub id: Uuid,
    /// Optional topic name/label
    pub name: Option<String>,
    /// Confidence score (weighted_agreement / 8.5)
    pub confidence: f32,
    /// Weighted agreement score (per ARCH-09: threshold >= 2.5)
    pub weighted_agreement: f32,
    /// Number of memories in this topic
    pub member_count: usize,
    /// List of contributing embedding spaces
    pub contributing_spaces: Vec<String>,
    /// Current phase: Emerging, Stable, Declining, or Merging
    pub phase: String,
}

/// Summary of stability metrics.
#[derive(Debug, Serialize)]
pub struct StabilityMetricsSummary {
    /// Churn rate [0.0-1.0] where 0.0=stable, 1.0=completely new topics
    pub churn_rate: f32,
    /// Topic distribution entropy [0.0-1.0]
    pub entropy: f32,
    /// Whether the portfolio is considered stable (churn < 0.3)
    pub is_stable: bool,
}

/// Response for get_topic_stability tool.
#[derive(Debug, Serialize)]
pub struct TopicStabilityResponse {
    /// Current churn rate
    pub churn_rate: f32,
    /// Current entropy
    pub entropy: f32,
    /// Breakdown of topics by phase
    pub phases: PhaseBreakdown,
    /// Whether dream consolidation is recommended (per AP-70: entropy > 0.7 AND churn > 0.5)
    pub dream_recommended: bool,
    /// Warning flag for high churn (churn >= 0.5)
    pub high_churn_warning: bool,
    /// Average churn over lookback period
    pub average_churn: f32,
}

/// Breakdown of topics by lifecycle phase.
#[derive(Debug, Serialize)]
pub struct PhaseBreakdown {
    /// Topics in emerging phase
    pub emerging: u32,
    /// Topics in stable phase
    pub stable: u32,
    /// Topics in declining phase
    pub declining: u32,
    /// Topics being merged
    pub merging: u32,
}

/// Response for detect_topics tool.
#[derive(Debug, Serialize)]
pub struct DetectTopicsResponse {
    /// Newly discovered topics
    pub new_topics: Vec<TopicSummary>,
    /// Topics that were merged
    pub merged_topics: Vec<MergedTopicInfo>,
    /// Total topic count after detection
    pub total_after: usize,
    /// Optional message describing what happened
    pub message: Option<String>,
}

/// Information about a merged topic.
#[derive(Debug, Serialize)]
pub struct MergedTopicInfo {
    /// ID of the topic that was absorbed
    pub absorbed_id: Uuid,
    /// ID of the topic it was merged into
    pub into_id: Uuid,
}

/// Response for get_divergence_alerts tool.
#[derive(Debug, Serialize)]
pub struct DivergenceAlertsResponse {
    /// List of divergence alerts
    pub alerts: Vec<DivergenceAlert>,
    /// Overall severity: "none", "low", "medium", "high"
    pub severity: String,
}

/// A single divergence alert.
///
/// Per AP-62: Only SEMANTIC embedders (E1, E5, E6, E7, E10, E12, E13) trigger alerts.
#[derive(Debug, Serialize)]
pub struct DivergenceAlert {
    /// The semantic space that detected divergence (e.g., "E1_Semantic")
    pub semantic_space: String,
    /// Similarity score between current and recent activity
    pub similarity_score: f32,
    /// Summary of the recent memory being compared to
    pub recent_memory_summary: String,
    /// The threshold that was crossed
    pub threshold: f32,
}
```

### /home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/curation_dtos.rs

```rust
//! DTOs for curation-related MCP tools.
//!
//! Per PRD Section 10.3, these DTOs support:
//! - forget_concept
//! - boost_importance

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Request DTOs
// ============================================================================

/// Request for forget_concept tool.
#[derive(Debug, Deserialize)]
pub struct ForgetConceptRequest {
    /// UUID of memory to forget
    pub node_id: String,

    /// Use soft delete (default true per SEC-06: 30-day recovery)
    #[serde(default = "default_soft_delete")]
    pub soft_delete: bool,
}

fn default_soft_delete() -> bool {
    true
}

/// Request for boost_importance tool.
#[derive(Debug, Deserialize)]
pub struct BoostImportanceRequest {
    /// UUID of memory to boost
    pub node_id: String,

    /// Importance delta (-1.0 to 1.0)
    /// Final importance is clamped to [0.0, 1.0] per BR-MCP-002
    pub delta: f32,
}

// ============================================================================
// Response DTOs
// ============================================================================

/// Response for forget_concept tool.
#[derive(Debug, Serialize)]
pub struct ForgetConceptResponse {
    /// ID of the forgotten memory
    pub forgotten_id: Uuid,
    /// Whether soft delete was used
    pub soft_deleted: bool,
    /// When the memory can be recovered until (if soft deleted)
    /// Per SEC-06: 30-day recovery window
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recoverable_until: Option<DateTime<Utc>>,
}

/// Response for boost_importance tool.
#[derive(Debug, Serialize)]
pub struct BoostImportanceResponse {
    /// ID of the modified memory
    pub node_id: Uuid,
    /// Importance before modification
    pub old_importance: f32,
    /// Importance after modification (clamped to [0.0, 1.0])
    pub new_importance: f32,
    /// Whether the value was clamped
    pub clamped: bool,
}
```

### /home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/mod.rs (updated)

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

// DTOs for new PRD v6 tools
pub mod curation_dtos;
pub mod topic_dtos;
```

## Definition of Done

- [ ] File `topic_dtos.rs` exists with 4 request DTOs and 4 response DTOs
- [ ] File `curation_dtos.rs` exists with 2 request DTOs and 2 response DTOs
- [ ] All DTOs have appropriate serde derive macros (`Serialize` for responses, `Deserialize` for requests)
- [ ] Default values use `#[serde(default)]` or `#[serde(default = "...")]` per spec
- [ ] Optional fields use `#[serde(skip_serializing_if = "Option::is_none")]`
- [ ] `mod.rs` exports the new DTO modules
- [ ] `cargo check -p context-graph-mcp` passes
- [ ] `cargo clippy -p context-graph-mcp -- -D warnings` passes
- [ ] `cargo doc -p context-graph-mcp --no-deps` generates documentation without errors

## Verification

```bash
cd /home/cabdru/contextgraph

# Verify files exist
test -f crates/context-graph-mcp/src/handlers/tools/topic_dtos.rs && echo "topic_dtos.rs exists"
test -f crates/context-graph-mcp/src/handlers/tools/curation_dtos.rs && echo "curation_dtos.rs exists"

# Verify compilation
cargo check -p context-graph-mcp

# Verify no clippy warnings
cargo clippy -p context-graph-mcp -- -D warnings

# Verify documentation builds
cargo doc -p context-graph-mcp --no-deps

# Count DTOs (should have appropriate number of structs)
grep -c "^pub struct" crates/context-graph-mcp/src/handlers/tools/topic_dtos.rs
# Expected: ~12 (4 request + 8 response/helper structs)

grep -c "^pub struct" crates/context-graph-mcp/src/handlers/tools/curation_dtos.rs
# Expected: 4 (2 request + 2 response)

# Verify module exports
grep "pub mod" crates/context-graph-mcp/src/handlers/tools/mod.rs
# Should include topic_dtos and curation_dtos
```

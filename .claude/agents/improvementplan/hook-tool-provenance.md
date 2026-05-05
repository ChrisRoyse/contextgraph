---
name: hook-tool-provenance
description: |
  Implements hook and tool call provenance: tool_use_id tracking in SourceMetadata,
  CF_TOOL_CALL_INDEX for tool-to-memory mapping, hook execution audit logging, and
  consolidation recommendation persistence. Use for Phase 5 (items 5.12, 5.13, 5.14)
  of the provenance improvement plan. Keywords: tool call, hook execution, tool_use_id,
  mcp_request_id, consolidation recommendations.
tools: Read,Write,Edit,Glob,Grep,Bash
model: sonnet
---

You are an expert Rust developer implementing hook and tool call provenance for the Context Graph system. You are implementing Phase 5 (items 5.12, 5.13, 5.14) of the Provenance Improvement Plan.

## Your Mission

Three gaps to close:
1. No way to trace which tool call created which memories
2. Hook executions are not logged
3. Consolidation recommendations are ephemeral (discarded after response)

## What You Must Build

### 1. Tool Call Registry (5.12)

**Add to SourceMetadata** (`crates/context-graph-core/src/types/source_metadata.rs`):
```rust
pub tool_use_id: Option<String>,
pub mcp_request_id: Option<String>,
pub hook_execution_timestamp_ms: Option<i64>,
```

**New CF:** `CF_TOOL_CALL_INDEX` in column_families.rs
- Key: `tool_use_id`
- Value: `Vec<Uuid>` (fingerprint IDs created by this tool call)

**Modify store_memory handler** (`crates/context-graph-mcp/src/handlers/tools/memory_tools.rs`):
- Accept optional `tool_use_id` parameter
- Write to CF_TOOL_CALL_INDEX mapping tool_use_id to created fingerprint IDs

**Modify post_tool_use handler** (`crates/context-graph-mcp/src/handlers/tools/post_tool_use.rs`):
- Capture tool_use_id from HookPayload if available
- Thread through to SourceMetadata

### 2. Hook Execution Audit Log (5.13)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookExecutionRecord {
    pub hook_type: String,
    pub session_id: String,
    pub timestamp: DateTime<Utc>,
    pub duration_ms: u64,
    pub exit_code: i32,
    pub tool_name: Option<String>,
    pub tool_use_id: Option<String>,
    pub success: bool,
    pub error_message: Option<String>,
    pub memories_created: Vec<Uuid>,
}
```

Store as AuditOperation variant or in CF_AUDIT_LOG if available. Otherwise use a dedicated log.

### 3. Consolidation Recommendation Persistence (5.14)

**New CF:** `CF_CONSOLIDATION_RECOMMENDATIONS`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationRecommendation {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub strategy: String,
    pub candidates: Vec<ConsolidationCandidate>,
    pub session_id: Option<String>,
    pub operator_id: Option<String>,
    pub status: RecommendationStatus,
    pub acted_on_at: Option<DateTime<Utc>>,
    pub resulting_merge_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationCandidate {
    pub source_ids: Vec<Uuid>,
    pub target_id: Uuid,
    pub similarity: f32,
    pub combined_alignment: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationStatus {
    Pending,
    Accepted { merge_id: Uuid },
    Rejected { reason: Option<String> },
    Expired,
}
```

**Modify consolidation handler** (`crates/context-graph-mcp/src/handlers/tools/consolidation.rs`):
- Persist recommendations to CF_CONSOLIDATION_RECOMMENDATIONS
- Add `persist_recommendations: bool` parameter (default: true)

**Modify merge handler** (`crates/context-graph-mcp/src/handlers/merge.rs`):
- When merging, check if it matches a pending recommendation
- If so, update recommendation status to Accepted with merge_id

## Process

1. Read SourceMetadata for existing fields
2. Read the post_tool_use handler for HookPayload structure
3. Read the consolidation handler for recommendation flow
4. Read the merge handler for merge flow
5. Add fields to SourceMetadata
6. Create CFs and types
7. Wire through the handlers
8. Run `cargo check` on affected crates

## Critical Rules

- CF_TOOL_CALL_INDEX has session-lifetime retention (can be pruned)
- CF_CONSOLIDATION_RECOMMENDATIONS has 90-day retention
- All new SourceMetadata fields are Option<T>
- Hook execution records should be lightweight (don't store full payloads)
- Recommendation matching during merge should be best-effort (don't block merge if lookup fails)

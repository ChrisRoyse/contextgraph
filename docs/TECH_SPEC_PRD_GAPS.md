# Technical Specification: PRD v6 Gap Remediation

**Spec ID**: TECH-GAP-001
**Version**: 1.0
**Status**: draft
**Implements**: SPEC-GAP-001
**Created**: 2026-01-18
**Last Updated**: 2026-01-18
**Tech Lead**: Context Graph Team

---

## Table of Contents

1. [Overview](#1-overview)
2. [Architecture Overview](#2-architecture-overview)
3. [Feature 1: Skills Framework](#3-feature-1-skills-framework)
4. [Feature 2: Stop Hook](#4-feature-2-stop-hook)
5. [Feature 3: Missing MCP Tools](#5-feature-3-missing-mcp-tools)
6. [Feature 4: Test Suite Fixes](#6-feature-4-test-suite-fixes)
7. [Feature 5: Documentation Updates](#7-feature-5-documentation-updates)
8. [Data Models](#8-data-models)
9. [API Contracts](#9-api-contracts)
10. [Component Contracts](#10-component-contracts)
11. [Error Handling](#11-error-handling)
12. [Integration Points](#12-integration-points)
13. [Security Implementation](#13-security-implementation)
14. [Performance Implementation](#14-performance-implementation)
15. [Migration Strategy](#15-migration-strategy)
16. [Testing Strategy](#16-testing-strategy)

---

## 1. Overview

This technical specification defines HOW to implement the features identified in the functional specification SPEC-GAP-001. The implementation leverages existing core modules (`context-graph-core/clustering`, `context-graph-core/curation`, `context-graph-cli`) while adding MCP tool handlers and Claude Code integration points.

### 1.1 Key Design Decisions

1. **Skills as Markdown Files**: Per Claude Code architecture, skills are SKILL.md files containing instructions and MCP tool invocations
2. **Shell Script Hooks**: Per AP-53, all hook logic resides in shell scripts calling `context-graph-cli`
3. **Handler Reuse**: New MCP tools leverage existing clustering, curation, and divergence detection modules
4. **Minimal Test Changes**: Fix imports only; preserve test logic where possible

### 1.2 Technology Choices

| Component | Technology | Rationale |
|-----------|------------|-----------|
| Skills | Markdown (SKILL.md) | Claude Code native format |
| Hooks | Bash + context-graph-cli | Per AP-53 constitution requirement |
| MCP Handlers | Rust async/await | Consistent with existing handlers |
| Topic System | `context-graph-core::clustering` | Already implemented per PRD |

---

## 2. Architecture Overview

### 2.1 Component Diagram

```
                     +-------------------+
                     |   Claude Code     |
                     |    CLI Client     |
                     +--------+----------+
                              |
          +-------------------+-------------------+
          |                   |                   |
          v                   v                   v
  +---------------+   +---------------+   +---------------+
  | Skills        |   | Native Hooks  |   | MCP Server    |
  | (SKILL.md)    |   | (settings.json)|  | (stdio/sse)   |
  +-------+-------+   +-------+-------+   +-------+-------+
          |                   |                   |
          |                   v                   |
          |           +---------------+           |
          |           | Shell Scripts |           |
          |           | (hooks/*.sh)  |           |
          |           +-------+-------+           |
          |                   |                   |
          |                   v                   |
          |           +---------------+           |
          +---------->| context-graph |<----------+
                      |     CLI       |
                      +-------+-------+
                              |
                              v
                      +---------------+
                      | MCP Handlers  |
                      | (handlers/)   |
                      +-------+-------+
                              |
          +-------------------+-------------------+
          |                   |                   |
          v                   v                   v
  +---------------+   +---------------+   +---------------+
  | Topic System  |   | Memory Store  |   | Curation      |
  | (clustering/) |   | (storage/)    |   | (core/)       |
  +---------------+   +---------------+   +---------------+
```

### 2.2 Sequence Diagram: Skill Invocation

```
User             Claude Code        SKILL.md           MCP Server        Core
  |                  |                 |                   |              |
  |--/topic-explorer->|                 |                   |              |
  |                  |--read skill----->|                   |              |
  |                  |<--instructions---|                   |              |
  |                  |--tools/call(get_topic_portfolio)--->|              |
  |                  |                 |                   |--get_topics->|
  |                  |                 |                   |<--TopicPortfolio
  |                  |<--JSON response-|-------------------|              |
  |<--formatted output|                 |                   |              |
```

### 2.3 Sequence Diagram: Stop Hook

```
Claude Code          settings.json       stop.sh          CLI            Core
    |                     |                |                |              |
    |--response complete->|                |                |              |
    |                     |--exec hook---->|                |              |
    |                     |                |--capture-response-->|         |
    |                     |                |                |--embed(13)-->|
    |                     |                |                |<--fingerprint|
    |                     |                |                |--store------>|
    |                     |                |<--success------|              |
    |                     |<--exit 0-------|                |              |
    |<--hook complete-----|                |                |              |
```

---

## 3. Feature 1: Skills Framework

### 3.1 File Structure

```
.claude/
  skills/
    topic-explorer/
      SKILL.md
    memory-inject/
      SKILL.md
    semantic-search/
      SKILL.md
    dream-consolidation/
      SKILL.md
    curation/
      SKILL.md
```

### 3.2 SKILL.md Template

Each SKILL.md follows this structure:

```markdown
---
model: sonnet|haiku
user_invocable: true
---

# [Skill Name]

[Description and keywords]

## Instructions

[Step-by-step instructions for Claude]

## MCP Tools

[List of MCP tools to use]

## Output Format

[Expected output format]
```

### 3.3 Skill Implementations

#### 3.3.1 topic-explorer/SKILL.md

**Path**: `.claude/skills/topic-explorer/SKILL.md`

**Implements**: REQ-SKILL-001, REQ-SKILL-002, REQ-SKILL-003, REQ-SKILL-004

```markdown
---
model: sonnet
user_invocable: true
---

# Topic Explorer

Explore emergent topic portfolio, topic stability metrics, and weighted agreement scores.

**Keywords**: topics, portfolio, stability, churn, weighted agreement

## Instructions

When the user asks about topics, topic stability, or the knowledge graph structure:

1. Call `get_topic_portfolio` to retrieve current topics
2. If user asks about stability, also call `get_topic_stability`
3. Format the response showing:
   - Topic names/IDs and their confidence scores
   - Contributing embedding spaces (semantic weight 1.0, relational 0.5, structural 0.5)
   - Current phase (Emerging, Stable, Declining, Merging)
   - If stability requested: churn rate, entropy, dream recommendation

## MCP Tools

- `get_topic_portfolio`: Get all discovered topics with profiles
  - Parameters: `format` (optional): "brief" | "standard" | "verbose"
  - Returns: `{ topics: Topic[], stability: StabilityMetrics }`

- `get_topic_stability`: Get portfolio-level stability metrics
  - Parameters: `hours` (optional): lookback period (default 6)
  - Returns: `{ churn_rate: f32, entropy: f32, phases: PhaseBreakdown, dream_recommended: bool }`

## Output Format

### Brief Format
```
Topics (N discovered):
1. [Topic Name] - confidence: X.XX, phase: Stable
2. [Topic Name] - confidence: X.XX, phase: Emerging
...
Stability: churn=0.XX, entropy=0.XX
```

### Standard Format
Include contributing spaces and member counts.

## Edge Cases

- If no topics discovered: "No topics discovered yet. Topics emerge when memories cluster in 3+ semantic spaces (weighted agreement >= 2.5)."
- If at Tier 0 (0 memories): "System at Tier 0 - no memories stored yet."
- High churn warning: If churn > 0.5, note "High churn detected - consider running /dream-consolidation"
```

#### 3.3.2 memory-inject/SKILL.md

**Path**: `.claude/skills/memory-inject/SKILL.md`

**Implements**: REQ-SKILL-001, REQ-SKILL-002, REQ-SKILL-003, REQ-SKILL-004

```markdown
---
model: haiku
user_invocable: true
---

# Memory Inject

Retrieve and inject contextual memories for the current task.

**Keywords**: memory, context, inject, retrieve, recall, background

## Instructions

When the user needs context, background, or wants to recall previous work:

1. Determine the query from user input or current task context
2. Call `inject_context` with the query
3. Present retrieved memories with relevance scores
4. If verbose requested, include similarity scores per embedding space

## MCP Tools

- `inject_context`: Retrieve and format relevant memories
  - Parameters:
    - `query` (required): Search query text
    - `max_tokens` (optional): Token budget (default 1000)
    - `verbosity` (optional): "compact" | "standard" | "verbose"
  - Returns: `{ memories: Memory[], total_found: u32, token_count: u32 }`

## Output Format

### Compact
Brief memory summaries within token budget.

### Standard
Memory content with source and timestamp.

### Verbose
Include similarity scores per semantic space (E1, E5, E6, E7, E10, E12, E13).

## Edge Cases

- No relevant memories: "No relevant memories found for this query."
- Token budget exceeded: Automatically truncates; note "Results truncated to fit budget"
```

#### 3.3.3 semantic-search/SKILL.md

**Path**: `.claude/skills/semantic-search/SKILL.md`

**Implements**: REQ-SKILL-001, REQ-SKILL-002, REQ-SKILL-003, REQ-SKILL-004

```markdown
---
model: haiku
user_invocable: true
---

# Semantic Search

Search the knowledge graph using multi-space retrieval.

**Keywords**: search, find, query, lookup, semantic, causal

## Instructions

When the user wants to search for specific information:

1. Parse the query and optional mode (semantic, causal, code)
2. Call `search_graph` with appropriate parameters
3. Present results ranked by relevance

## MCP Tools

- `search_graph`: Multi-space vector search
  - Parameters:
    - `query` (required): Search text
    - `top_k` (optional): Number of results (default 10)
    - `mode` (optional): "semantic" | "causal" | "code" | "entity"
    - `min_similarity` (optional): Minimum similarity threshold (default 0.3)
  - Returns: `{ results: SearchResult[], mode: string }`

## Mode Behavior

- **semantic** (default): Prioritizes E1 (Semantic) embedder
- **causal**: Prioritizes E5 (Causal) embedder for "why/because" queries
- **code**: Prioritizes E7 (Code) embedder for technical content
- **entity**: Prioritizes E11 (Entity/TransE) embedder for named entities

## Output Format

```
Search Results (N found):
1. [Content preview] - relevance: 0.XX
   Source: [HookDescription|ClaudeResponse|MDFileChunk] | Created: [timestamp]
2. ...
```

## Edge Cases

- No results: "No memories match your search criteria."
- Mode-specific empty: "No [causal|code|entity] matches found. Try default semantic search."
```

#### 3.3.4 dream-consolidation/SKILL.md

**Path**: `.claude/skills/dream-consolidation/SKILL.md`

**Implements**: REQ-SKILL-001, REQ-SKILL-002, REQ-SKILL-003, REQ-SKILL-004

```markdown
---
model: sonnet
user_invocable: true
---

# Dream Consolidation

Trigger memory consolidation via NREM and REM dream phases.

**Keywords**: dream, consolidate, nrem, rem, blind spots, entropy, churn

## Instructions

When the user requests consolidation or system metrics indicate need:

1. Call `get_memetic_status` to check current entropy and churn
2. Evaluate if consolidation is recommended (entropy > 0.7 AND churn > 0.5)
3. If recommended or user insists, call `trigger_consolidation`
4. Report results including blind spots discovered

## MCP Tools

- `get_memetic_status`: Get system health and metrics
  - Returns: `{ entropy: f32, coherence: f32, churn: f32, layers: LayerStatus }`

- `trigger_consolidation`: Execute dream cycle
  - Parameters:
    - `blocking` (optional): Wait for completion (default true)
    - `dry_run` (optional): Show what would happen without executing (default false)
  - Returns: `{ nrem_result: NREMResult, rem_result: REMResult, blind_spots: BlindSpot[] }`

## Dream Phases

### NREM (Non-REM)
- Duration: ~3 minutes
- Purpose: Hebbian learning replay
- Strengthens high-importance connections

### REM
- Duration: ~2 minutes
- Purpose: Blind spot discovery via hyperbolic random walk
- Discovers unexpected connections in Poincare ball model

## Output Format

```
Dream Consolidation [completed|dry-run]

NREM Phase:
- Edges strengthened: N
- Weight adjustments: [summary]

REM Phase:
- Blind spots discovered: N
- New connections: [list]

Recommendation: [next steps]
```

## Edge Cases

- Not recommended: "Current metrics (entropy=X.XX, churn=X.XX) don't indicate consolidation need. Proceed anyway? (thresholds: entropy>0.7, churn>0.5)"
- Dream in progress: "Dream cycle already in progress. Status: [phase]"
- Dry run: Show projected effects without execution
```

#### 3.3.5 curation/SKILL.md

**Path**: `.claude/skills/curation/SKILL.md`

**Implements**: REQ-SKILL-001, REQ-SKILL-002, REQ-SKILL-003, REQ-SKILL-004

```markdown
---
model: sonnet
user_invocable: true
---

# Knowledge Curation

Curate the knowledge graph by merging, forgetting, or boosting memories.

**Keywords**: curate, merge, forget, annotate, prune, duplicate

## Instructions

When the user wants to curate knowledge:

1. Call `get_memetic_status` to see pending curation tasks
2. Based on user intent:
   - **merge**: Call `merge_concepts` with source IDs
   - **forget**: Call `forget_concept` with memory ID (soft delete)
   - **boost**: Call `boost_importance` with memory ID and delta
3. Confirm action results

## MCP Tools

- `get_memetic_status`: Get curation task suggestions
  - Returns includes `{ curation_tasks: CurationTask[] }`

- `merge_concepts`: Merge duplicate memories
  - Parameters:
    - `source_node_ids` (required): Array of UUIDs to merge
    - `merge_strategy` (optional): "keep_newest" | "combine" (default "combine")
  - Returns: `{ merged_id: UUID, sources_removed: u32 }`

- `forget_concept`: Soft-delete a memory (30-day recovery per SEC-06)
  - Parameters:
    - `node_id` (required): UUID of memory to forget
    - `soft_delete` (optional): Use soft delete (default true)
  - Returns: `{ forgotten_id: UUID, recoverable_until: DateTime }`

- `boost_importance`: Adjust memory importance
  - Parameters:
    - `node_id` (required): UUID of memory
    - `delta` (required): Importance change (-1.0 to 1.0)
  - Returns: `{ node_id: UUID, old_importance: f32, new_importance: f32 }`

## Output Format

### Curation Tasks
```
Pending Curation Tasks:
1. [Merge] IDs [X, Y] - similarity: 0.95
2. [Review] ID [Z] - low access, consider forgetting
```

### Action Results
```
[Action] completed:
- [Details of change]
- [Recovery info if applicable]
```

## Edge Cases

- Invalid UUID: "Memory ID [X] not found"
- Already deleted: "Memory [X] already soft-deleted"
- Importance bounds: Values clamped to [0.0, 1.0]
```

---

## 4. Feature 2: Stop Hook

### 4.1 Settings.json Changes

**Path**: `.claude/settings.json`

**Implements**: REQ-STOP-001

Add the Stop hook configuration to the existing hooks object:

```json
{
  "hooks": {
    "SessionStart": [ ... ],
    "SessionEnd": [ ... ],
    "PreToolUse": [ ... ],
    "PostToolUse": [ ... ],
    "UserPromptSubmit": [ ... ],
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": ".claude/hooks/stop.sh",
            "timeout": 3000
          }
        ]
      }
    ]
  }
}
```

### 4.2 Shell Script Implementation

**Path**: `.claude/hooks/stop.sh`

**Implements**: REQ-STOP-002, REQ-STOP-003, REQ-STOP-004

```bash
#!/bin/bash
# Claude Code Hook: Stop
# Timeout: 3000ms
#
# Constitution: AP-50, AP-53, ARCH-11
# Exit Codes: 0=success, 1=cli_not_found, 2=timeout, 3=db_error, 4=invalid_input
#
# Input from Claude Code: {"response_text":"...", "session_id":"..."}
# Stores response as ClaudeResponse memory with all 13 embeddings

set -euo pipefail

INPUT=$(cat)
if [ -z "$INPUT" ]; then
    echo '{"success":false,"error":"Empty stdin","exit_code":4}' >&2
    exit 4
fi

# Validate JSON input
if ! echo "$INPUT" | jq empty 2>/dev/null; then
    echo '{"success":false,"error":"Invalid JSON input","exit_code":4}' >&2
    exit 4
fi

# Find CLI binary
CONTEXT_GRAPH_CLI="${CONTEXT_GRAPH_CLI:-context-graph-cli}"
if ! command -v "$CONTEXT_GRAPH_CLI" &>/dev/null; then
    for candidate in \
        "./target/release/context-graph-cli" \
        "./target/debug/context-graph-cli" \
        "$HOME/.cargo/bin/context-graph-cli" \
    ; do
        if [ -x "$candidate" ]; then
            CONTEXT_GRAPH_CLI="$candidate"
            break
        fi
    done
fi

if ! command -v "$CONTEXT_GRAPH_CLI" &>/dev/null && [ ! -x "$CONTEXT_GRAPH_CLI" ]; then
    echo '{"success":false,"error":"CLI binary not found","exit_code":1}' >&2
    exit 1
fi

# Parse input JSON
RESPONSE_TEXT=$(echo "$INPUT" | jq -r '.response_text // empty')
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // empty')
TIMESTAMP_MS=$(date +%s%3N)

# Skip if response is empty
if [ -z "$RESPONSE_TEXT" ]; then
    echo '{"success":true,"skipped":true,"reason":"Empty response"}'
    exit 0
fi

# Truncate response if > 10000 chars (per schema constraint)
RESPONSE_TEXT=$(echo "$RESPONSE_TEXT" | head -c 10000)

# Build HookInput JSON for capture-response command
HOOK_INPUT=$(cat <<EOF
{
    "hook_type": "stop",
    "session_id": "$SESSION_ID",
    "timestamp_ms": $TIMESTAMP_MS,
    "payload": {
        "type": "stop",
        "data": {
            "response_text": $(echo "$RESPONSE_TEXT" | jq -Rs .),
            "source": "claude_response"
        }
    }
}
EOF
)

# Execute CLI with 3s timeout
# Captures response, embeds with all 13 embedders, stores as ClaudeResponse
echo "$HOOK_INPUT" | timeout 3s "$CONTEXT_GRAPH_CLI" hooks capture-response --stdin --format json
exit_code=$?

if [ $exit_code -eq 124 ]; then
    echo '{"success":false,"error":"Timeout after 3000ms","exit_code":2}' >&2
    exit 2
fi
exit $exit_code
```

### 4.3 CLI Command Implementation

**Path**: `crates/context-graph-cli/src/commands/capture.rs`

The `capture-response` command must:

1. Parse HookInput from stdin
2. Extract response_text from payload
3. Generate all 13 embeddings via `MultiArrayEmbeddingProvider`
4. Create `TeleologicalFingerprint` with source = `ClaudeResponse`
5. Store via `TeleologicalMemoryStore::store()`

```rust
// Signature for the capture-response subcommand handler
pub async fn handle_capture_response(
    stdin_input: &str,
    store: Arc<dyn TeleologicalMemoryStore>,
    embedder: Arc<dyn MultiArrayEmbeddingProvider>,
) -> Result<CaptureResponseOutput, CliError>
```

---

## 5. Feature 3: Missing MCP Tools

### 5.1 Tool Name Constants

**Path**: `crates/context-graph-mcp/src/tools/names.rs`

**Implements**: REQ-MCP-001

Add the following constants (currently commented as TODO):

```rust
// ========== TOPIC TOOLS (PRD Section 10.2) ==========
pub const GET_TOPIC_PORTFOLIO: &str = "get_topic_portfolio";
pub const GET_TOPIC_STABILITY: &str = "get_topic_stability";
pub const DETECT_TOPICS: &str = "detect_topics";
pub const GET_DIVERGENCE_ALERTS: &str = "get_divergence_alerts";

// ========== CURATION TOOLS (PRD Section 10.3) ==========
pub const FORGET_CONCEPT: &str = "forget_concept";
pub const BOOST_IMPORTANCE: &str = "boost_importance";
```

### 5.2 Handler Module Structure

**Path**: `crates/context-graph-mcp/src/handlers/tools/topic_tools.rs` (new file)

**Path**: `crates/context-graph-mcp/src/handlers/tools/curation_tools.rs` (new file)

### 5.3 Tool Dispatch Updates

**Path**: `crates/context-graph-mcp/src/handlers/tools/dispatch.rs`

Add match arms for new tools:

```rust
match tool_name {
    // ... existing tools ...

    // ========== TOPIC TOOLS (PRD Section 10.2) ==========
    tool_names::GET_TOPIC_PORTFOLIO => self.call_get_topic_portfolio(id, arguments).await,
    tool_names::GET_TOPIC_STABILITY => self.call_get_topic_stability(id, arguments).await,
    tool_names::DETECT_TOPICS => self.call_detect_topics(id, arguments).await,
    tool_names::GET_DIVERGENCE_ALERTS => self.call_get_divergence_alerts(id, arguments).await,

    // ========== CURATION TOOLS (PRD Section 10.3) ==========
    tool_names::FORGET_CONCEPT => self.call_forget_concept(id, arguments).await,
    tool_names::BOOST_IMPORTANCE => self.call_boost_importance(id, arguments).await,

    // Unknown tool
    _ => JsonRpcResponse::error(...),
}
```

### 5.4 Handlers Struct Extension

**Path**: `crates/context-graph-mcp/src/handlers/core/handlers.rs`

Add clustering and curation dependencies:

```rust
pub struct Handlers {
    // ... existing fields ...

    /// Cluster manager for topic operations.
    pub(in crate::handlers) cluster_manager: Option<Arc<RwLock<MultiSpaceClusterManager>>>,

    /// Topic stability tracker for portfolio metrics.
    pub(in crate::handlers) stability_tracker: Option<Arc<RwLock<TopicStabilityTracker>>>,
}
```

---

## 6. Feature 4: Test Suite Fixes

### 6.1 Affected Files Analysis

**Error Source**: Commit `fab0622` removed modules but test imports remained.

**Files to Modify**:

1. `crates/context-graph-mcp/src/handlers/tests/mod.rs`
2. `crates/context-graph-mcp/src/handlers/tests/task_emb_024_verification.rs`
3. `crates/context-graph-mcp/src/handlers/tests/manual_fsv_verification.rs`

### 6.2 Fix: tests/mod.rs

**Current Broken Code** (lines 700-704):

```rust
use crate::handlers::core::MetaUtlTracker;
use crate::handlers::gwt_providers::{
    GwtSystemProviderImpl, MetaCognitiveProviderImpl, WorkspaceProviderImpl,
};
use crate::handlers::gwt_traits::{GwtSystemProvider, MetaCognitiveProvider, WorkspaceProvider};
```

**Fix**: Remove or conditionally compile these imports and the functions that use them:

```rust
// REMOVED: These modules no longer exist after PRD v6 compliance refactor
// use crate::handlers::core::MetaUtlTracker;
// use crate::handlers::gwt_providers::{...};
// use crate::handlers::gwt_traits::{...};

// Also remove functions that depend on deleted modules:
// - create_test_handlers_with_warm_gwt()
// - create_test_handlers_with_warm_gwt_rocksdb()
// - create_test_handlers_with_all_components()
```

Alternatively, if MetaUtlTracker is still needed, verify it exists in `core/mod.rs`:

```rust
// In handlers/core/mod.rs - check if MetaUtlTracker is exported
pub use handlers::Handlers;
// pub use meta_utl::MetaUtlTracker;  // May have been removed
```

### 6.3 Fix: task_emb_024_verification.rs

**Current Broken Import** (line 28):

```rust
use crate::handlers::core::MetaUtlTracker;
```

**Fix**: Remove the import and the function using it:

```rust
// REMOVED: MetaUtlTracker no longer exists
// use crate::handlers::core::MetaUtlTracker;

// Remove or stub the function:
// fn create_handlers_with_tracker() -> (Handlers, Arc<RwLock<MetaUtlTracker>>)
```

### 6.4 Fix: manual_fsv_verification.rs

**Current Broken Import** (line 21):

```rust
use crate::handlers::core::MetaUtlTracker;
```

**Fix**: Same approach as above - remove import and dependent code.

### 6.5 Detailed Fix Instructions

For each affected file:

1. **Read the file** to identify all broken imports
2. **Grep for usages** of the imported items
3. **Either remove** the test function if it tests deleted functionality
4. **Or stub** the test with `#[ignore]` and TODO comment if functionality should be restored later

Example stub:

```rust
#[tokio::test]
#[ignore = "TODO: MetaUtlTracker removed in fab0622 - restore when Meta-UTL system reimplemented"]
async fn test_meta_utl_tracker_verification() {
    // Test body removed - was testing deleted MetaUtlTracker
}
```

---

## 7. Feature 5: Documentation Updates

### 7.1 CLAUDE.md MCP Tools Section

**Path**: `/home/cabdru/contextgraph/CLAUDE.md`

**Current Section** (lines 479-510): Lists 30+ tools that don't exist.

**Replace With**:

```yaml
mcp:
  version: "2024-11-05"
  transport: [stdio, sse]

  exposed_tools:
    # Core (PRD Section 10.1)
    inject_context:
      purpose: "Retrieve and inject relevant memories for current context"
      parameters:
        query: "string (required) - Search query"
        max_tokens: "number (optional) - Token budget, default 1000"
        verbosity: "string (optional) - compact|standard|verbose"
      example: "inject_context({query: 'authentication flow'})"

    store_memory:
      purpose: "Store new memory with all 13 embeddings"
      parameters:
        content: "string (required) - Memory content"
        importance: "number (optional) - 0.0-1.0, default 0.5"
      example: "store_memory({content: 'User prefers dark mode', importance: 0.8})"

    search_graph:
      purpose: "Multi-space vector search"
      parameters:
        query: "string (required) - Search text"
        top_k: "number (optional) - Results count, default 10"
        mode: "string (optional) - semantic|causal|code|entity"
      example: "search_graph({query: 'error handling', mode: 'code'})"

    get_memetic_status:
      purpose: "Get system health, entropy, and curation tasks"
      parameters: "none"
      example: "get_memetic_status({})"

    # Topic (PRD Section 10.2)
    get_topic_portfolio:
      purpose: "Get all discovered topics with profiles"
      parameters:
        format: "string (optional) - brief|standard|verbose"
      example: "get_topic_portfolio({format: 'standard'})"

    get_topic_stability:
      purpose: "Get portfolio-level stability metrics"
      parameters:
        hours: "number (optional) - Lookback period, default 6"
      example: "get_topic_stability({hours: 12})"

    detect_topics:
      purpose: "Force topic detection recalculation"
      parameters:
        force: "boolean (optional) - Force even if not needed"
      example: "detect_topics({force: false})"

    get_divergence_alerts:
      purpose: "Check for divergence from recent activity"
      parameters:
        lookback_hours: "number (optional) - Hours to check, default 2"
      example: "get_divergence_alerts({lookback_hours: 4})"

    # Consolidation
    trigger_consolidation:
      purpose: "Trigger NREM/REM dream consolidation"
      parameters:
        blocking: "boolean (optional) - Wait for completion, default true"
      example: "trigger_consolidation({blocking: true})"

    # Curation (PRD Section 10.3)
    merge_concepts:
      purpose: "Merge duplicate memories"
      parameters:
        source_node_ids: "string[] (required) - UUIDs to merge"
        merge_strategy: "string (optional) - keep_newest|combine"
      example: "merge_concepts({source_node_ids: ['uuid1', 'uuid2']})"

    forget_concept:
      purpose: "Soft-delete a memory (30-day recovery)"
      parameters:
        node_id: "string (required) - UUID to forget"
        soft_delete: "boolean (optional) - Use soft delete, default true"
      example: "forget_concept({node_id: 'uuid', soft_delete: true})"

    boost_importance:
      purpose: "Adjust memory importance score"
      parameters:
        node_id: "string (required) - UUID of memory"
        delta: "number (required) - Change amount (-1.0 to 1.0)"
      example: "boost_importance({node_id: 'uuid', delta: 0.2})"

  total_tools: 12
```

---

## 8. Data Models

### 8.1 Request DTOs

#### GetTopicPortfolioRequest

```rust
#[derive(Debug, Deserialize)]
pub struct GetTopicPortfolioRequest {
    /// Output format: "brief", "standard", or "verbose"
    #[serde(default = "default_format")]
    pub format: String,
}

fn default_format() -> String {
    "standard".to_string()
}
```

#### GetTopicStabilityRequest

```rust
#[derive(Debug, Deserialize)]
pub struct GetTopicStabilityRequest {
    /// Lookback period in hours (default 6)
    #[serde(default = "default_hours")]
    pub hours: u32,
}

fn default_hours() -> u32 {
    6
}
```

#### DetectTopicsRequest

```rust
#[derive(Debug, Deserialize)]
pub struct DetectTopicsRequest {
    /// Force detection even if not needed
    #[serde(default)]
    pub force: bool,
}
```

#### GetDivergenceAlertsRequest

```rust
#[derive(Debug, Deserialize)]
pub struct GetDivergenceAlertsRequest {
    /// Lookback period in hours (default 2)
    #[serde(default = "default_lookback")]
    pub lookback_hours: u32,
}

fn default_lookback() -> u32 {
    2
}
```

#### ForgetConceptRequest

```rust
#[derive(Debug, Deserialize)]
pub struct ForgetConceptRequest {
    /// UUID of memory to forget
    pub node_id: String,

    /// Use soft delete (default true per SEC-06)
    #[serde(default = "default_soft_delete")]
    pub soft_delete: bool,
}

fn default_soft_delete() -> bool {
    true
}
```

#### BoostImportanceRequest

```rust
#[derive(Debug, Deserialize)]
pub struct BoostImportanceRequest {
    /// UUID of memory to boost
    pub node_id: String,

    /// Importance delta (-1.0 to 1.0)
    pub delta: f32,
}
```

### 8.2 Response DTOs

#### TopicPortfolioResponse

```rust
#[derive(Debug, Serialize)]
pub struct TopicPortfolioResponse {
    pub topics: Vec<TopicSummary>,
    pub stability: StabilityMetricsSummary,
    pub total_topics: usize,
    pub tier: u8,
}

#[derive(Debug, Serialize)]
pub struct TopicSummary {
    pub id: Uuid,
    pub name: Option<String>,
    pub confidence: f32,
    pub weighted_agreement: f32,
    pub member_count: usize,
    pub contributing_spaces: Vec<String>,
    pub phase: String,
}

#[derive(Debug, Serialize)]
pub struct StabilityMetricsSummary {
    pub churn_rate: f32,
    pub entropy: f32,
    pub is_stable: bool,
}
```

#### TopicStabilityResponse

```rust
#[derive(Debug, Serialize)]
pub struct TopicStabilityResponse {
    pub churn_rate: f32,
    pub entropy: f32,
    pub phases: PhaseBreakdown,
    pub dream_recommended: bool,
    pub high_churn_warning: bool,
    pub average_churn_6h: f32,
}

#[derive(Debug, Serialize)]
pub struct PhaseBreakdown {
    pub emerging: u32,
    pub stable: u32,
    pub declining: u32,
    pub merging: u32,
}
```

#### DetectTopicsResponse

```rust
#[derive(Debug, Serialize)]
pub struct DetectTopicsResponse {
    pub new_topics: Vec<TopicSummary>,
    pub merged_topics: Vec<MergedTopicInfo>,
    pub total_after: usize,
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MergedTopicInfo {
    pub absorbed_id: Uuid,
    pub into_id: Uuid,
}
```

#### DivergenceAlertsResponse

```rust
#[derive(Debug, Serialize)]
pub struct DivergenceAlertsResponse {
    pub alerts: Vec<DivergenceAlert>,
    pub severity: String,
}

#[derive(Debug, Serialize)]
pub struct DivergenceAlert {
    pub semantic_space: String,
    pub similarity_score: f32,
    pub recent_memory_summary: String,
    pub threshold: f32,
}
```

#### ForgetConceptResponse

```rust
#[derive(Debug, Serialize)]
pub struct ForgetConceptResponse {
    pub forgotten_id: Uuid,
    pub soft_deleted: bool,
    pub recoverable_until: Option<DateTime<Utc>>,
}
```

#### BoostImportanceResponse

```rust
#[derive(Debug, Serialize)]
pub struct BoostImportanceResponse {
    pub node_id: Uuid,
    pub old_importance: f32,
    pub new_importance: f32,
    pub clamped: bool,
}
```

---

## 9. API Contracts

### 9.1 get_topic_portfolio

```yaml
endpoint: tools/call
method: POST
tool_name: get_topic_portfolio

request_schema:
  type: object
  properties:
    format:
      type: string
      enum: [brief, standard, verbose]
      default: standard

response_200:
  content:
    - type: text
      text: |
        {
          "topics": [
            {
              "id": "uuid",
              "name": "Topic Name",
              "confidence": 0.35,
              "weighted_agreement": 3.0,
              "member_count": 15,
              "contributing_spaces": ["Semantic", "Causal", "Code"],
              "phase": "Stable"
            }
          ],
          "stability": {
            "churn_rate": 0.15,
            "entropy": 0.45,
            "is_stable": true
          },
          "total_topics": 5,
          "tier": 4
        }

response_error:
  - code: -32602
    message: "Invalid params: format must be brief|standard|verbose"
```

### 9.2 get_topic_stability

```yaml
endpoint: tools/call
method: POST
tool_name: get_topic_stability

request_schema:
  type: object
  properties:
    hours:
      type: integer
      minimum: 1
      maximum: 168
      default: 6

response_200:
  content:
    - type: text
      text: |
        {
          "churn_rate": 0.25,
          "entropy": 0.55,
          "phases": {
            "emerging": 2,
            "stable": 8,
            "declining": 1,
            "merging": 0
          },
          "dream_recommended": false,
          "high_churn_warning": false,
          "average_churn_6h": 0.22
        }
```

### 9.3 detect_topics

```yaml
endpoint: tools/call
method: POST
tool_name: detect_topics

request_schema:
  type: object
  properties:
    force:
      type: boolean
      default: false

response_200:
  content:
    - type: text
      text: |
        {
          "new_topics": [...],
          "merged_topics": [...],
          "total_after": 12,
          "message": "Detected 2 new topics, merged 1"
        }

response_error:
  - code: -32002
    message: "Need >= 3 memories for topic detection"
```

### 9.4 get_divergence_alerts

```yaml
endpoint: tools/call
method: POST
tool_name: get_divergence_alerts

request_schema:
  type: object
  properties:
    lookback_hours:
      type: integer
      minimum: 1
      maximum: 48
      default: 2

response_200:
  content:
    - type: text
      text: |
        {
          "alerts": [
            {
              "semantic_space": "E1_Semantic",
              "similarity_score": 0.22,
              "recent_memory_summary": "Working on authentication...",
              "threshold": 0.30
            }
          ],
          "severity": "medium"
        }

notes:
  - "Only SEMANTIC embedders (E1, E5, E6, E7, E10, E12, E13) trigger alerts per AP-62"
  - "Temporal embedders (E2-E4) NEVER trigger divergence alerts"
```

### 9.5 forget_concept

```yaml
endpoint: tools/call
method: POST
tool_name: forget_concept

request_schema:
  type: object
  required: [node_id]
  properties:
    node_id:
      type: string
      format: uuid
    soft_delete:
      type: boolean
      default: true

response_200:
  content:
    - type: text
      text: |
        {
          "forgotten_id": "uuid",
          "soft_deleted": true,
          "recoverable_until": "2026-02-17T12:00:00Z"
        }

response_error:
  - code: -32001
    message: "Memory {id} not found"
  - code: -32602
    message: "Invalid UUID format"
```

### 9.6 boost_importance

```yaml
endpoint: tools/call
method: POST
tool_name: boost_importance

request_schema:
  type: object
  required: [node_id, delta]
  properties:
    node_id:
      type: string
      format: uuid
    delta:
      type: number
      minimum: -1.0
      maximum: 1.0

response_200:
  content:
    - type: text
      text: |
        {
          "node_id": "uuid",
          "old_importance": 0.5,
          "new_importance": 0.7,
          "clamped": false
        }

response_error:
  - code: -32001
    message: "Memory {id} not found"
  - code: -32602
    message: "delta must be between -1.0 and 1.0"
```

---

## 10. Component Contracts

### 10.1 TopicToolsHandler

**Path**: `crates/context-graph-mcp/src/handlers/tools/topic_tools.rs`

```rust
impl Handlers {
    /// Handle get_topic_portfolio tool call.
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
        // 1. Parse arguments
        // 2. Get topics from cluster_manager
        // 3. Get stability from stability_tracker
        // 4. Format based on verbosity
        // 5. Return MCP-formatted response
    }

    /// Handle get_topic_stability tool call.
    ///
    /// # Implements
    /// REQ-MCP-002
    pub(crate) async fn call_get_topic_stability(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        // 1. Parse hours parameter
        // 2. Get stability metrics
        // 3. Check dream trigger conditions (AP-70)
        // 4. Return stability response
    }

    /// Handle detect_topics tool call.
    ///
    /// # Implements
    /// REQ-MCP-002, BR-MCP-003
    pub(crate) async fn call_detect_topics(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        // 1. Check minimum memory count (min_cluster_size = 3)
        // 2. Run HDBSCAN batch clustering
        // 3. Synthesize topics via TopicSynthesizer
        // 4. Return new/merged topics
    }

    /// Handle get_divergence_alerts tool call.
    ///
    /// # Implements
    /// REQ-MCP-002, REQ-MCP-005
    pub(crate) async fn call_get_divergence_alerts(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        // 1. Parse lookback_hours
        // 2. Get recent memories
        // 3. Compare current context against recent (SEMANTIC spaces only per AP-62)
        // 4. Generate alerts for low similarity
        // 5. Return alerts with severity
    }
}
```

### 10.2 CurationToolsHandler

**Path**: `crates/context-graph-mcp/src/handlers/tools/curation_tools.rs`

```rust
impl Handlers {
    /// Handle forget_concept tool call.
    ///
    /// # Implements
    /// REQ-MCP-002, BR-MCP-001
    pub(crate) async fn call_forget_concept(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        // 1. Parse and validate UUID
        // 2. Check memory exists
        // 3. Soft delete (default) or hard delete
        // 4. Return confirmation with recovery deadline (30 days per SEC-06)
    }

    /// Handle boost_importance tool call.
    ///
    /// # Implements
    /// REQ-MCP-002, BR-MCP-002
    pub(crate) async fn call_boost_importance(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        // 1. Parse node_id and delta
        // 2. Validate delta range
        // 3. Retrieve memory
        // 4. Apply delta, clamp to [0.0, 1.0]
        // 5. Update memory
        // 6. Return old/new importance
    }
}
```

---

## 11. Error Handling

### 11.1 Error Types

**Path**: `crates/context-graph-mcp/src/protocol/error_codes.rs`

Add new error codes:

```rust
// Topic errors
pub const TOPIC_NOT_FOUND: i32 = -32020;
pub const INSUFFICIENT_MEMORIES: i32 = -32021;
pub const TOPIC_DETECTION_FAILED: i32 = -32022;

// Curation errors
pub const MEMORY_ALREADY_DELETED: i32 = -32030;
pub const IMPORTANCE_OUT_OF_RANGE: i32 = -32031;
```

### 11.2 Error Mapping

| Condition | Error Code | HTTP Status | User Message |
|-----------|------------|-------------|--------------|
| Invalid UUID format | -32602 | 400 | "Invalid UUID format: {details}" |
| Memory not found | -32001 | 404 | "Memory {id} not found" |
| Memory already deleted | -32030 | 409 | "Memory {id} already deleted" |
| Insufficient memories | -32021 | 400 | "Need >= 3 memories for topic detection" |
| Delta out of range | -32031 | 400 | "delta must be between -1.0 and 1.0" |
| Internal error | -32603 | 500 | "Internal error: {details}" |

---

## 12. Integration Points

### 12.1 Clustering Module Integration

**Dependency**: `context-graph-core::clustering`

```rust
// In Handlers constructor
let cluster_manager = Arc::new(RwLock::new(
    MultiSpaceClusterManager::new(ManagerParams::default())
));

let stability_tracker = Arc::new(RwLock::new(
    TopicStabilityTracker::new()
));
```

### 12.2 Storage Integration

**Dependency**: `context-graph-storage::teleological`

```rust
// Soft delete implementation
impl TeleologicalMemoryStore {
    async fn soft_delete(&self, id: Uuid) -> Result<DateTime<Utc>, StorageError> {
        // Set deleted_at timestamp
        // Return recoverable_until = deleted_at + 30 days
    }
}
```

### 12.3 CLI Integration

**Dependency**: `context-graph-cli::commands`

New subcommand for stop hook:

```
context-graph-cli hooks capture-response --stdin --format json
```

---

## 13. Security Implementation

### 13.1 PII Scrubbing (SEC-02)

All skills and tools must scrub PII before processing:

```rust
// In tool handler
let scrubbed_content = pii_scrubber.scrub(&request.content)?;
```

### 13.2 Soft Delete Recovery (SEC-06)

```rust
const SOFT_DELETE_RECOVERY_DAYS: i64 = 30;

fn compute_recovery_deadline(deleted_at: DateTime<Utc>) -> DateTime<Utc> {
    deleted_at + Duration::days(SOFT_DELETE_RECOVERY_DAYS)
}
```

---

## 14. Performance Implementation

### 14.1 Latency Targets

| Operation | Target | Implementation |
|-----------|--------|----------------|
| get_topic_portfolio | <100ms | Cache topic summaries |
| get_topic_stability | <50ms | Pre-computed metrics |
| detect_topics | <1s | Async HDBSCAN on GPU |
| get_divergence_alerts | <100ms | Recent memory cache |
| forget_concept | <50ms | Async soft delete |
| boost_importance | <50ms | Direct update |

### 14.2 Caching Strategy

```rust
// Topic portfolio cache (5-minute TTL)
struct TopicCache {
    portfolio: Option<(Instant, TopicPortfolioResponse)>,
    ttl: Duration,
}

impl TopicCache {
    fn get_or_compute<F>(&mut self, compute: F) -> TopicPortfolioResponse
    where F: FnOnce() -> TopicPortfolioResponse;
}
```

---

## 15. Migration Strategy

### 15.1 Backward Compatibility

No breaking changes to existing MCP tools. New tools are additive.

### 15.2 Deployment Steps

1. Deploy new CLI with capture-response command
2. Add stop.sh hook script
3. Update settings.json with Stop hook
4. Deploy MCP server with new tools
5. Add skill files to .claude/skills/
6. Update CLAUDE.md documentation

### 15.3 Rollback Procedure

1. Remove Stop hook from settings.json
2. Revert to previous MCP server
3. Remove skill files (optional - they're inert without tools)

---

## 16. Testing Strategy

### 16.1 Unit Tests

| Component | Test Type | Coverage Target |
|-----------|-----------|-----------------|
| TopicToolsHandler | Unit | 90% |
| CurationToolsHandler | Unit | 90% |
| Request DTOs | Unit | 100% |
| Response DTOs | Unit | 100% |

### 16.2 Integration Tests

| Test | Description | Req Ref |
|------|-------------|---------|
| TC-TOPIC-001 | get_topic_portfolio returns data | REQ-MCP-002 |
| TC-TOPIC-002 | get_topic_stability returns metrics | REQ-MCP-002 |
| TC-TOPIC-003 | detect_topics finds clusters | REQ-MCP-002 |
| TC-TOPIC-004 | get_divergence_alerts uses SEMANTIC only | REQ-MCP-005 |
| TC-CURATION-001 | forget_concept soft deletes | BR-MCP-001 |
| TC-CURATION-002 | boost_importance clamps values | BR-MCP-002 |

### 16.3 E2E Tests

| Test | Description |
|------|-------------|
| TC-SKILL-E2E-001 | /topic-explorer invokes MCP tools |
| TC-STOP-E2E-001 | Stop hook captures response |

### 16.4 Mocking Strategy

```rust
// Mock cluster manager for unit tests
struct MockClusterManager {
    topics: Vec<Topic>,
    stability: TopicStabilityTracker,
}

impl MockClusterManager {
    fn with_topics(topics: Vec<Topic>) -> Self;
}
```

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-18 | Technical Spec Writer | Initial specification |

---

## TECHNICAL SPEC WRITER - DOCUMENT COMPLETE

### Document Created:
- **Path**: /home/cabdru/contextgraph/docs/TECH_SPEC_PRD_GAPS.md
- **Spec ID**: TECH-GAP-001
- **Version**: 1.0
- **Status**: draft
- **Implements**: SPEC-GAP-001

### Specification Summary:
- **Data Models**: 12 DTOs defined (6 request, 6 response)
- **API Endpoints**: 6 new MCP tools specified
- **Component Methods**: 6 handler methods designed
- **Error Types**: 6 new error codes defined
- **Integration Points**: 3 documented (clustering, storage, CLI)

### Architecture Diagrams:
- [x] Component Diagram (Section 2.1)
- [x] Sequence Diagram - Skill Invocation (Section 2.2)
- [x] Sequence Diagram - Stop Hook (Section 2.3)

### Requirement Coverage:
| Requirement | Implementation |
|-------------|----------------|
| REQ-SKILL-001 | SKILL.md files in Section 3 |
| REQ-SKILL-002 | Model metadata in SKILL.md |
| REQ-SKILL-003 | MCP tool mappings in each skill |
| REQ-SKILL-004 | Keywords documented in skills |
| REQ-STOP-001 | settings.json in Section 4.1 |
| REQ-STOP-002 | stop.sh in Section 4.2 |
| REQ-STOP-003 | ClaudeResponse source type |
| REQ-STOP-004 | 13 embeddings via CLI |
| REQ-MCP-001 | Tool constants in Section 5.1 |
| REQ-MCP-002 | Handlers in Section 10 |
| REQ-MCP-003 | Schemas in Section 9 |
| REQ-MCP-004 | Weighted agreement in handlers |
| REQ-MCP-005 | SEMANTIC-only divergence |
| REQ-TEST-001 | Fix instructions in Section 6 |
| REQ-DOC-001 | CLAUDE.md updates in Section 7 |

### Constitution Compliance:
- [x] Technology stack matches (Rust, tokio, serde)
- [x] Naming conventions followed (snake_case, PascalCase)
- [x] Security requirements addressed (SEC-02, SEC-06)
- [x] Performance budgets achievable
- [x] No anti-patterns used (AP-60, AP-62, AP-70)

### Quality Checklist:
- [x] All requirements have implementation design
- [x] All endpoints fully specified
- [x] All methods have signatures
- [x] Traceability complete
- [x] Diagrams included

### Next Steps:
1. Review with tech lead
2. Validate against constitution
3. Move to "approved" status
4. Hand off to Atomic Task Generator

### Related Documents:
- Constitution: /home/cabdru/contextgraph/CLAUDE.md
- Functional Spec: /home/cabdru/contextgraph/docs/FUNC_SPEC_PRD_GAPS.md
- PRD: /home/cabdru/contextgraph/docs2/contextprd.md
- Gap Analysis: /home/cabdru/contextgraph/docs/PRD_GAP_ANALYSIS.md

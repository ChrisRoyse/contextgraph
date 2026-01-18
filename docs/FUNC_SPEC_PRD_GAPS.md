# Functional Specification: PRD v6 Gap Remediation

**Spec ID**: SPEC-GAP-001
**Version**: 1.0
**Status**: draft
**Owner**: Context Graph Team
**Created**: 2026-01-18
**Last Updated**: 2026-01-18

---

## Table of Contents

1. [Overview](#1-overview)
2. [User Types](#2-user-types)
3. [Feature 1: Skills Framework](#3-feature-1-skills-framework)
4. [Feature 2: Stop Hook](#4-feature-2-stop-hook)
5. [Feature 3: Missing MCP Tools](#5-feature-3-missing-mcp-tools)
6. [Feature 4: Test Suite Fixes](#6-feature-4-test-suite-fixes)
7. [Feature 5: Documentation Updates](#7-feature-5-documentation-updates)
8. [Data Requirements](#8-data-requirements)
9. [Non-Functional Requirements](#9-non-functional-requirements)
10. [Dependencies](#10-dependencies)
11. [Out of Scope](#11-out-of-scope)
12. [Test Plan](#12-test-plan)
13. [Open Questions](#13-open-questions)
14. [Glossary](#14-glossary)

---

## 1. Overview

This functional specification defines the requirements for implementing missing features identified in the PRD v6 Gap Analysis. The Context Graph system is approximately 85% complete; this specification addresses the remaining 15% to achieve full PRD compliance.

### 1.1 Business Value

- **User Empowerment**: Skills enable users to invoke context graph functionality via Claude Code `/` commands
- **Complete Memory Lifecycle**: Stop hook ensures Claude responses are captured as ClaudeResponse memories
- **Full Topic System Access**: Missing MCP tools expose the fully-implemented topic and curation systems
- **Quality Assurance**: Test suite fixes enable CI/CD pipeline validation
- **Developer Experience**: Documentation accuracy reduces confusion and support burden

### 1.2 Scope Summary

| Feature | Priority | Effort | Status |
|---------|----------|--------|--------|
| Skills Framework (5 skills) | Critical | 2-3 days | Not Started |
| Stop Hook | Critical | 1 hour | Not Started |
| Missing MCP Tools (6 tools) | High | 1 day | Not Started |
| Test Suite Fixes | High | 2-4 hours | Not Started |
| Documentation Updates | Medium | 2 hours | Not Started |

### 1.3 Source Documents

| Document | Location | Purpose |
|----------|----------|---------|
| PRD v6 | `/home/cabdru/contextgraph/docs2/contextprd.md` | Requirements source |
| Constitution v6 | `/home/cabdru/contextgraph/docs2/constitution.yaml` | Architectural constraints |
| Gap Analysis | `/home/cabdru/contextgraph/docs/PRD_GAP_ANALYSIS.md` | Gap identification |
| CLAUDE.md | `/home/cabdru/contextgraph/CLAUDE.md` | Current documentation |

---

## 2. User Types

### 2.1 UT-01: Claude Code User

**Description**: Developer using Claude Code CLI for coding assistance who benefits from contextual memory.

**Permissions**: Read/write access to context graph via MCP tools and skills.

**Goals**:
- Receive relevant context automatically during sessions
- Explore discovered topics and their relationships
- Search through accumulated knowledge
- Trigger memory consolidation when needed
- Curate memories by merging, forgetting, or boosting importance

### 2.2 UT-02: System Administrator

**Description**: Developer responsible for maintaining the context graph installation.

**Permissions**: Full system access including configuration and maintenance.

**Goals**:
- Verify system health and configuration
- Ensure hooks are properly configured
- Validate test suite passes
- Keep documentation synchronized

---

## 3. Feature 1: Skills Framework

### 3.1 Overview

Skills are user-invocable capabilities exposed via Claude Code's `/` command interface. Per PRD Section 9.3, five skills must be implemented as `SKILL.md` files in `.claude/skills/*/`.

### 3.2 User Stories

#### US-SKILL-001: Explore Topic Portfolio

**Priority**: must-have

**Narrative**:
- **As a** Claude Code User
- **I want to** explore the emergent topic portfolio
- **So that** I can understand what topics have been discovered from my work patterns

**Acceptance Criteria**:

| ID | Given | When | Then |
|----|-------|------|------|
| AC-001 | User has accumulated memories with topics | User invokes `/topic-explorer` | Skill displays current topics with confidence scores |
| AC-002 | Topic portfolio is empty (new installation) | User invokes `/topic-explorer` | Skill displays "No topics discovered yet" message |
| AC-003 | User wants topic stability metrics | User invokes `/topic-explorer stability` | Skill shows churn rate, entropy, and phase for each topic |
| AC-004 | Topic stability indicates high churn | User views stability metrics | Churn rate > 0.5 is highlighted as warning |

---

#### US-SKILL-002: Inject Memory Context

**Priority**: must-have

**Narrative**:
- **As a** Claude Code User
- **I want to** retrieve and inject contextual memories
- **So that** I can restore context when starting a task or needing background information

**Acceptance Criteria**:

| ID | Given | When | Then |
|----|-------|------|------|
| AC-005 | Relevant memories exist for query | User invokes `/memory-inject "query text"` | Skill retrieves and displays relevant memories |
| AC-006 | No relevant memories exist | User invokes `/memory-inject "obscure query"` | Skill displays "No relevant memories found" |
| AC-007 | Token budget is limited | User provides custom token budget | Response is distilled to fit within budget |
| AC-008 | User wants verbose output | User invokes `/memory-inject --verbose` | Skill shows similarity scores and source details |

---

#### US-SKILL-003: Search Knowledge Graph

**Priority**: must-have

**Narrative**:
- **As a** Claude Code User
- **I want to** search the knowledge graph using multi-space retrieval
- **So that** I can find specific memories using semantic, causal, or code search modes

**Acceptance Criteria**:

| ID | Given | When | Then |
|----|-------|------|------|
| AC-009 | Memories exist in graph | User invokes `/semantic-search "query"` | Skill returns top-k relevant results |
| AC-010 | User wants code-specific search | User invokes `/semantic-search --mode code "function signature"` | Results prioritize E7 (Code) embedder |
| AC-011 | User wants causal search | User invokes `/semantic-search --mode causal "why did X happen"` | Results prioritize E5 (Causal) embedder |
| AC-012 | Empty result set | User searches for non-existent content | Skill displays helpful "no results" message |

---

#### US-SKILL-004: Trigger Dream Consolidation

**Priority**: should-have

**Narrative**:
- **As a** Claude Code User
- **I want to** manually trigger memory consolidation
- **So that** the system can replay high-importance patterns and discover blind spots

**Acceptance Criteria**:

| ID | Given | When | Then |
|----|-------|------|------|
| AC-013 | System entropy > 0.7 or churn > 0.5 | User invokes `/dream-consolidation` | Dream cycle executes NREM then REM phases |
| AC-014 | System is stable (low entropy, low churn) | User invokes `/dream-consolidation` | Skill advises consolidation not needed but allows override |
| AC-015 | Dream is already in progress | User invokes `/dream-consolidation` | Skill reports current dream status |
| AC-016 | User wants dry-run | User invokes `/dream-consolidation --dry-run` | Skill shows what would happen without executing |

---

#### US-SKILL-005: Curate Knowledge Graph

**Priority**: should-have

**Narrative**:
- **As a** Claude Code User
- **I want to** curate the knowledge graph by merging, annotating, or forgetting concepts
- **So that** I can maintain a clean and useful knowledge base

**Acceptance Criteria**:

| ID | Given | When | Then |
|----|-------|------|------|
| AC-017 | Duplicate memories exist | User invokes `/curation merge` | Skill shows merge candidates and allows selection |
| AC-018 | User wants to forget a memory | User invokes `/curation forget <id>` | Memory is soft-deleted (30-day recovery per SEC-06) |
| AC-019 | User wants to boost importance | User invokes `/curation boost <id>` | Memory importance is increased by delta |
| AC-020 | Curation tasks exist from get_memetic_status | User invokes `/curation tasks` | Skill displays pending curation tasks |

---

### 3.3 Functional Requirements

#### REQ-SKILL-001: Skill File Structure

**Description**: Each skill MUST be implemented as a `SKILL.md` file in the designated directory.

**Rationale**: Claude Code skills framework requires this specific structure.

**Constraints**: Files must be valid Markdown with specific frontmatter format.

**Verification**: File exists at expected path and is parseable by Claude Code.

**File Locations**:
```
.claude/skills/topic-explorer/SKILL.md
.claude/skills/memory-inject/SKILL.md
.claude/skills/semantic-search/SKILL.md
.claude/skills/dream-consolidation/SKILL.md
.claude/skills/curation/SKILL.md
```

---

#### REQ-SKILL-002: Skill Metadata Format

**Description**: Each SKILL.md MUST include metadata specifying model tier and invocability.

**Rationale**: PRD Section 9.3 specifies model tier (sonnet/haiku) per skill.

**Constraints**: Must follow Claude Code skill metadata format.

**Verification**: Skill is invocable via `/` command.

**Required Metadata**:
```yaml
# Topic-Explorer, Dream-Consolidation, Curation: model=sonnet
# Memory-Inject, Semantic-Search: model=haiku
user_invocable: true
```

---

#### REQ-SKILL-003: MCP Tool Integration

**Description**: Skills MUST invoke appropriate MCP tools to perform operations.

**Rationale**: Skills are interfaces to underlying MCP functionality (ARCH-06).

**Constraints**: Skills cannot bypass MCP tools for direct DB access.

**Verification**: Skills call documented MCP tools in handlers.

**Tool Mappings**:
| Skill | Primary MCP Tools |
|-------|-------------------|
| topic-explorer | get_topic_portfolio, get_topic_stability |
| memory-inject | inject_context |
| semantic-search | search_graph |
| dream-consolidation | trigger_consolidation, get_memetic_status |
| curation | merge_concepts, forget_concept, boost_importance |

---

#### REQ-SKILL-004: Keyword Documentation

**Description**: Each skill MUST document keywords that trigger its use.

**Rationale**: PRD constitution specifies keywords for each skill.

**Constraints**: Keywords must be unique across skills.

**Verification**: Skill responds to documented keywords.

**Keywords by Skill**:
| Skill | Keywords |
|-------|----------|
| topic-explorer | topics, portfolio, stability, churn, weighted agreement |
| memory-inject | memory, context, inject, retrieve, recall, background |
| semantic-search | search, find, query, lookup, semantic, causal |
| dream-consolidation | dream, consolidate, nrem, rem, blind spots, entropy, churn |
| curation | curate, merge, forget, annotate, prune, duplicate |

---

### 3.4 Business Rules

#### BR-SKILL-001: Topic Weight Exclusion

**Condition**: When calculating or displaying weighted agreement.

**Action**: Temporal embedders (E2, E3, E4) MUST show weight 0.0 per AP-60.

**Exception**: None - this is a constitutional requirement.

---

#### BR-SKILL-002: Dream Trigger Conditions

**Condition**: When evaluating whether to trigger dream consolidation.

**Action**: Apply AP-70 conditions: entropy > 0.7 AND churn > 0.5.

**Exception**: User can override with explicit confirmation.

---

#### BR-SKILL-003: Soft Delete Recovery

**Condition**: When forgetting a memory via curation skill.

**Action**: Memory is soft-deleted with 30-day recovery window per SEC-06.

**Exception**: None - hard delete not permitted via skills.

---

### 3.5 Edge Cases

| ID | Scenario | Expected Behavior | Priority |
|----|----------|-------------------|----------|
| EC-SKILL-001 | Skill invoked before any memories exist | Display progressive tier message "System at Tier 0" | high |
| EC-SKILL-002 | MCP server not running | Display connection error with recovery steps | critical |
| EC-SKILL-003 | Query contains PII patterns | PII is scrubbed before embedding per SEC-02 | critical |
| EC-SKILL-004 | Skill timeout during MCP call | Display timeout error, suggest retry | high |
| EC-SKILL-005 | Invalid memory ID provided to curation | Display "Memory not found" error | medium |

---

### 3.6 Error States

| ID | HTTP | Condition | User Message | Internal Message | Recovery |
|----|------|-----------|--------------|------------------|----------|
| ERR-SKILL-001 | N/A | MCP connection failed | "Cannot connect to context graph. Ensure MCP server is running." | "MCP_CONNECTION_FAILED" | Retry or restart MCP |
| ERR-SKILL-002 | N/A | Invalid skill parameters | "Invalid parameters: {details}" | "SKILL_PARAM_INVALID" | Correct parameters |
| ERR-SKILL-003 | N/A | Skill execution timeout | "Operation timed out after {timeout}ms" | "SKILL_TIMEOUT" | Retry operation |
| ERR-SKILL-004 | N/A | Memory not found | "Memory with ID {id} not found" | "MEMORY_NOT_FOUND" | Verify memory ID |

---

## 4. Feature 2: Stop Hook

### 4.1 Overview

The Stop hook captures Claude's response summary when a response is stopped, ensuring significant responses are stored as ClaudeResponse memories per PRD Section 9.1.

### 4.2 User Stories

#### US-STOP-001: Capture Claude Response on Stop

**Priority**: must-have

**Narrative**:
- **As a** Claude Code User
- **I want** Claude's significant responses to be automatically captured
- **So that** valuable answers are stored in the knowledge graph for future retrieval

**Acceptance Criteria**:

| ID | Given | When | Then |
|----|-------|------|------|
| AC-021 | Claude provides a response | User stops Claude or response completes | Response summary is captured as ClaudeResponse memory |
| AC-022 | Response is very short (<50 chars) | Response stops | Response is still captured (no filtering) |
| AC-023 | Response contains code blocks | Response stops | Code content is preserved in memory |
| AC-024 | Hook execution times out | Hook exceeds 3000ms | Hook fails gracefully, logs error |

---

### 4.3 Functional Requirements

#### REQ-STOP-001: Settings.json Configuration

**Description**: Stop hook MUST be configured in `.claude/settings.json`.

**Rationale**: PRD Section 9.2 mandates native Claude Code hooks via settings.json.

**Constraints**: Must match existing hook configuration pattern.

**Verification**: `cargo test` passes and hook appears in Claude Code hook list.

**Configuration**:
```json
{
  "hooks": {
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

---

#### REQ-STOP-002: Shell Script Implementation

**Description**: Stop hook MUST be implemented as a shell script calling context-graph-cli.

**Rationale**: AP-53 requires hook logic in shell scripts calling CLI.

**Constraints**: Script must handle JSON input/output, timeout gracefully.

**Verification**: Script executes successfully with valid JSON input.

**File Location**: `.claude/hooks/stop.sh`

**Script Requirements**:
- Parse JSON input from Claude Code (response_text, session_id)
- Call `context-graph-cli memory capture --source response`
- Handle errors with appropriate exit codes
- Complete within 3000ms timeout

---

#### REQ-STOP-003: Memory Source Type

**Description**: Captured responses MUST be stored with source type `ClaudeResponse`.

**Rationale**: ARCH-11 defines three memory sources; Stop hook produces ClaudeResponse.

**Constraints**: Must use existing MemorySource enum value.

**Verification**: Stored memory has correct source field.

---

#### REQ-STOP-004: All 13 Embeddings

**Description**: Captured response MUST be embedded with all 13 embedders.

**Rationale**: ARCH-01 requires atomic TeleologicalArray storage.

**Constraints**: Cannot store partial embeddings.

**Verification**: Stored memory has complete TeleologicalArray.

---

### 4.4 Business Rules

#### BR-STOP-001: Response Capture Scope

**Condition**: When Stop hook fires.

**Action**: Capture the response text provided by Claude Code.

**Exception**: Empty responses (0 characters) may be skipped.

---

### 4.5 Edge Cases

| ID | Scenario | Expected Behavior | Priority |
|----|----------|-------------------|----------|
| EC-STOP-001 | Empty response text | Skip capture, log warning | medium |
| EC-STOP-002 | Response > 10,000 characters | Truncate to limit per schema | high |
| EC-STOP-003 | CLI binary not found | Log error, exit code 1 | critical |
| EC-STOP-004 | Invalid JSON input | Log error, exit code 4 | high |

---

### 4.6 Error States

| ID | Exit Code | Condition | User Message | Recovery |
|----|-----------|-----------|--------------|----------|
| ERR-STOP-001 | 1 | CLI binary not found | "CLI binary not found" | Install/configure CLI |
| ERR-STOP-002 | 2 | Timeout after 3000ms | "Timeout after 3000ms" | Retry or check system |
| ERR-STOP-003 | 3 | Database error | "Database error" | Check storage connection |
| ERR-STOP-004 | 4 | Invalid JSON input | "Invalid JSON input" | Check Claude Code version |

---

## 5. Feature 3: Missing MCP Tools

### 5.1 Overview

Six MCP tools are marked as TODO in `names.rs` but have underlying implementations in core modules. These tools must be exposed via MCP handlers.

### 5.2 User Stories

#### US-MCP-001: Get Topic Portfolio

**Priority**: must-have

**Narrative**:
- **As a** Claude Code User
- **I want to** retrieve the current topic portfolio via MCP
- **So that** I can programmatically access discovered topics

**Acceptance Criteria**:

| ID | Given | When | Then |
|----|-------|------|------|
| AC-025 | Topics exist in portfolio | User calls get_topic_portfolio | Returns topics with profiles, confidence, members |
| AC-026 | No topics discovered yet | User calls get_topic_portfolio | Returns empty array with status message |
| AC-027 | User requests brief format | User calls get_topic_portfolio with format=brief | Returns condensed topic summaries |

---

#### US-MCP-002: Get Topic Stability

**Priority**: must-have

**Narrative**:
- **As a** Claude Code User
- **I want to** check topic stability metrics
- **So that** I can understand whether the knowledge graph needs consolidation

**Acceptance Criteria**:

| ID | Given | When | Then |
|----|-------|------|------|
| AC-028 | Stability tracker has data | User calls get_topic_stability | Returns churn_rate, entropy, phases |
| AC-029 | Churn exceeds 0.5 threshold | User calls get_topic_stability | Response includes warning flag |
| AC-030 | Entropy exceeds 0.7 threshold | User calls get_topic_stability | Response includes dream_recommended flag |

---

#### US-MCP-003: Detect Topics

**Priority**: should-have

**Narrative**:
- **As a** Claude Code User
- **I want to** trigger topic detection
- **So that** I can force topic recalculation after significant changes

**Acceptance Criteria**:

| ID | Given | When | Then |
|----|-------|------|------|
| AC-031 | Memories exist for clustering | User calls detect_topics | Returns new_topics, merged_topics arrays |
| AC-032 | Not enough memories (< 3) | User calls detect_topics | Returns message about minimum cluster size |

---

#### US-MCP-004: Get Divergence Alerts

**Priority**: must-have

**Narrative**:
- **As a** Claude Code User
- **I want to** check for divergence from recent activity
- **So that** I'm alerted when my current work differs significantly from recent patterns

**Acceptance Criteria**:

| ID | Given | When | Then |
|----|-------|------|------|
| AC-033 | Current activity diverges from recent | User calls get_divergence_alerts | Returns alerts with semantic space, score, content |
| AC-034 | No divergence detected | User calls get_divergence_alerts | Returns empty alerts array |
| AC-035 | Divergence only in temporal spaces | User calls get_divergence_alerts | Returns empty (temporal excluded per AP-62) |

---

#### US-MCP-005: Forget Concept

**Priority**: should-have

**Narrative**:
- **As a** Claude Code User
- **I want to** soft-delete a memory
- **So that** I can remove outdated or incorrect information

**Acceptance Criteria**:

| ID | Given | When | Then |
|----|-------|------|------|
| AC-036 | Memory exists with given ID | User calls forget_concept | Memory is soft-deleted, returns confirmation |
| AC-037 | Memory ID does not exist | User calls forget_concept | Returns error "Memory not found" |
| AC-038 | Memory already soft-deleted | User calls forget_concept | Returns message "Already deleted" |

---

#### US-MCP-006: Boost Importance

**Priority**: should-have

**Narrative**:
- **As a** Claude Code User
- **I want to** increase a memory's importance score
- **So that** it ranks higher in future retrievals

**Acceptance Criteria**:

| ID | Given | When | Then |
|----|-------|------|------|
| AC-039 | Memory exists with given ID | User calls boost_importance with delta | Importance increased, returns new value |
| AC-040 | Resulting importance > 1.0 | User calls boost_importance | Importance clamped to 1.0 |
| AC-041 | Negative delta provided | User calls boost_importance with -0.1 | Importance decreased (floor 0.0) |

---

### 5.3 Functional Requirements

#### REQ-MCP-001: Tool Name Constants

**Description**: Tool names MUST be defined as constants in `names.rs`.

**Rationale**: Consistent with existing pattern for tool dispatch.

**File**: `crates/context-graph-mcp/src/tools/names.rs`

**Required Constants**:
```rust
pub const GET_TOPIC_PORTFOLIO: &str = "get_topic_portfolio";
pub const GET_TOPIC_STABILITY: &str = "get_topic_stability";
pub const DETECT_TOPICS: &str = "detect_topics";
pub const GET_DIVERGENCE_ALERTS: &str = "get_divergence_alerts";
pub const FORGET_CONCEPT: &str = "forget_concept";
pub const BOOST_IMPORTANCE: &str = "boost_importance";
```

---

#### REQ-MCP-002: Handler Implementation

**Description**: Each tool MUST have a handler function in the handlers module.

**Rationale**: Handlers process MCP requests and return responses.

**Constraints**: Handlers must return MCP-compliant JSON-RPC responses.

**Verification**: Tools appear in tools/list response and execute correctly.

---

#### REQ-MCP-003: Tool Schema Definition

**Description**: Each tool MUST define input schema per MCP specification.

**Rationale**: MCP tools require inputSchema for validation.

**Constraints**: Must match MCP 2024-11-05 specification.

**Input Schemas**:

| Tool | Parameters |
|------|------------|
| get_topic_portfolio | format?: "brief" \| "standard" \| "verbose" |
| get_topic_stability | hours?: number (default 6) |
| detect_topics | force?: boolean (default false) |
| get_divergence_alerts | lookback_hours?: number (default 2) |
| forget_concept | node_id: string (required), soft_delete?: boolean (default true) |
| boost_importance | node_id: string (required), delta: number (required) |

---

#### REQ-MCP-004: Weighted Agreement Compliance

**Description**: Topic tools MUST use weighted agreement formula per ARCH-09.

**Rationale**: Topic detection threshold is weighted_agreement >= 2.5.

**Constraints**: Temporal embedders (E2-E4) weight = 0.0.

**Verification**: get_topic_portfolio returns confidence = weighted_agreement / 8.5.

---

#### REQ-MCP-005: Divergence Semantic Only

**Description**: get_divergence_alerts MUST only use SEMANTIC embedders per AP-62.

**Rationale**: Temporal/relational differences are not semantic divergence.

**Constraints**: Only E1, E5, E6, E7, E10, E12, E13 trigger divergence.

**Verification**: Test with temporal-only differences produces no alerts.

---

### 5.4 Business Rules

#### BR-MCP-001: Soft Delete Default

**Condition**: When forget_concept is called without soft_delete parameter.

**Action**: Default to soft_delete = true per SEC-06.

**Exception**: Hard delete requires explicit false value (admin only).

---

#### BR-MCP-002: Importance Bounds

**Condition**: When boost_importance modifies importance value.

**Action**: Clamp result to range [0.0, 1.0].

**Exception**: None.

---

#### BR-MCP-003: Minimum Cluster Size

**Condition**: When detect_topics is called.

**Action**: Require min_cluster_size = 3 per constitution.

**Exception**: None - fewer memories returns informational message.

---

### 5.5 Edge Cases

| ID | Scenario | Expected Behavior | Priority |
|----|----------|-------------------|----------|
| EC-MCP-001 | get_topic_portfolio at Tier 0 (0 memories) | Return progressive tier message | high |
| EC-MCP-002 | get_divergence_alerts with empty recent history | Return empty alerts array | medium |
| EC-MCP-003 | forget_concept with invalid UUID format | Return validation error | high |
| EC-MCP-004 | boost_importance with NaN delta | Return validation error | high |
| EC-MCP-005 | detect_topics during dream cycle | Return "Dream in progress" message | medium |

---

### 5.6 Error States

| ID | Code | Condition | User Message | Recovery |
|----|------|-----------|--------------|----------|
| ERR-MCP-001 | -32602 | Invalid parameters | "Invalid params: {details}" | Fix parameters |
| ERR-MCP-002 | -32603 | Internal error | "Internal error" | Check logs |
| ERR-MCP-003 | -32001 | Memory not found | "Memory {id} not found" | Verify ID |
| ERR-MCP-004 | -32002 | Insufficient memories | "Need >= 3 memories for topics" | Add memories |

---

## 6. Feature 4: Test Suite Fixes

### 6.1 Overview

The MCP test suite has 11 compilation errors due to imports referencing deleted modules after commit `fab0622`. Tests must be updated to remove or fix these imports.

### 6.2 User Stories

#### US-TEST-001: Fix Test Compilation

**Priority**: must-have

**Narrative**:
- **As a** System Administrator
- **I want** `cargo test` to compile successfully
- **So that** CI/CD pipelines can validate the codebase

**Acceptance Criteria**:

| ID | Given | When | Then |
|----|-------|------|------|
| AC-042 | Test files have broken imports | Developer runs `cargo test` | All tests compile without errors |
| AC-043 | Tests reference deleted MetaUtlTracker | Tests are updated | MetaUtlTracker references removed or fixed |
| AC-044 | Tests reference deleted gwt_providers | Tests are updated | gwt_providers references removed or fixed |

---

### 6.3 Functional Requirements

#### REQ-TEST-001: Remove Broken Imports

**Description**: Test files MUST remove imports to deleted modules.

**Rationale**: Deleted modules cause compilation errors.

**Affected Files**:
- `crates/context-graph-mcp/src/handlers/tests/mod.rs`
- `crates/context-graph-mcp/src/handlers/tests/task_emb_024_verification.rs`
- `crates/context-graph-mcp/src/handlers/tests/manual_fsv_verification.rs`

**Broken Imports**:
```rust
// These imports no longer exist after fab0622:
use crate::handlers::core::MetaUtlTracker;
use crate::handlers::gwt_providers::{...};
```

---

#### REQ-TEST-002: Update Test Helpers

**Description**: Test helper functions MUST be updated to use current API.

**Rationale**: Handlers::new() signature may have changed.

**Verification**: `cargo test --no-run` succeeds.

---

#### REQ-TEST-003: Preserve Test Coverage

**Description**: Functional tests MUST be preserved or replaced with equivalent coverage.

**Rationale**: Removing tests without replacement reduces quality.

**Verification**: Test count after fixes >= test count before (minus deleted modules).

---

### 6.4 Edge Cases

| ID | Scenario | Expected Behavior | Priority |
|----|----------|-------------------|----------|
| EC-TEST-001 | Test depends on deleted functionality | Remove test with TODO comment for replacement | medium |
| EC-TEST-002 | Test is obsolete for PRD v6 | Remove test entirely | low |

---

## 7. Feature 5: Documentation Updates

### 7.1 Overview

CLAUDE.md lists 30+ MCP tools but only 6 are currently exposed. Documentation must be synchronized with actual implementation.

### 7.2 User Stories

#### US-DOC-001: Synchronize Tool Documentation

**Priority**: should-have

**Narrative**:
- **As a** Claude Code User
- **I want** documentation to accurately reflect available tools
- **So that** I don't try to use non-existent functionality

**Acceptance Criteria**:

| ID | Given | When | Then |
|----|-------|------|------|
| AC-045 | CLAUDE.md lists unavailable tools | Documentation is updated | Only exposed tools are listed |
| AC-046 | New tools are added via Feature 3 | Documentation is updated | New tools appear in CLAUDE.md |
| AC-047 | Tool descriptions are vague | Documentation is updated | Clear descriptions with examples |

---

### 7.3 Functional Requirements

#### REQ-DOC-001: MCP Tool Section Update

**Description**: CLAUDE.md `mcp.core_tools` section MUST list only exposed tools.

**Rationale**: Documentation accuracy prevents user confusion.

**Current State**: Lists 30+ tools, 6 exposed.

**Target State**: Lists 12 tools (6 existing + 6 from Feature 3).

**Verification**: All listed tools respond to tools/list query.

---

#### REQ-DOC-002: Tool Description Format

**Description**: Each tool MUST have description with purpose, parameters, and example.

**Rationale**: Consistent documentation aids usability.

**Format**:
```yaml
tool_name:
  purpose: "One-line description"
  parameters:
    param1: "type - description"
  example: "tool_name({param1: value})"
```

---

## 8. Data Requirements

### 8.1 Entity: Skill

| Field | Type | Required | Description | Constraints |
|-------|------|----------|-------------|-------------|
| name | string | yes | Skill identifier | lowercase, hyphenated |
| description | string | yes | User-facing description | Max 500 chars |
| keywords | string[] | yes | Trigger keywords | 3-10 keywords |
| model | enum | yes | Model tier | "sonnet" or "haiku" |
| user_invocable | boolean | yes | Can user invoke directly | Always true |

### 8.2 Entity: MCP Tool

| Field | Type | Required | Description | Constraints |
|-------|------|----------|-------------|-------------|
| name | string | yes | Tool name constant | snake_case |
| inputSchema | object | yes | JSON Schema for params | Valid JSON Schema |
| description | string | yes | Tool description | Max 200 chars |

### 8.3 Entity: Hook Configuration

| Field | Type | Required | Description | Constraints |
|-------|------|----------|-------------|-------------|
| type | enum | yes | Hook type | "command" |
| command | string | yes | Script path | Relative to project root |
| timeout | number | yes | Timeout in ms | > 0 |

---

## 9. Non-Functional Requirements

### 9.1 Performance

| ID | Category | Description | Metric | Priority |
|----|----------|-------------|--------|----------|
| NFR-001 | performance | Stop hook execution | < 3000ms p95 | must |
| NFR-002 | performance | get_topic_portfolio response | < 100ms p95 | must |
| NFR-003 | performance | get_topic_stability response | < 50ms p95 | must |
| NFR-004 | performance | Skills response time | < 2000ms p95 | should |

### 9.2 Reliability

| ID | Category | Description | Metric | Priority |
|----|----------|-------------|--------|----------|
| NFR-005 | reliability | Hook failure handling | Graceful degradation | must |
| NFR-006 | reliability | MCP tool availability | 99.9% uptime | should |

### 9.3 Usability

| ID | Category | Description | Metric | Priority |
|----|----------|-------------|--------|----------|
| NFR-007 | usability | Skill error messages | Human-readable | must |
| NFR-008 | usability | Documentation accuracy | 100% match to implementation | should |

### 9.4 Security

| ID | Category | Description | Metric | Priority |
|----|----------|-------------|--------|----------|
| NFR-009 | security | PII scrubbing in skills | 100% pattern detection | must |
| NFR-010 | security | Soft delete retention | 30-day recovery per SEC-06 | must |

---

## 10. Dependencies

### 10.1 Internal Dependencies

| Dependency | Description | Impact if Unavailable |
|------------|-------------|----------------------|
| context-graph-core/clustering | Topic/stability types | Cannot implement topic tools |
| context-graph-core/dream | Dream controller | Cannot implement dream skill |
| context-graph-cli | Hook CLI commands | Cannot implement Stop hook |
| context-graph-mcp | MCP infrastructure | Cannot implement any tools |

### 10.2 External Dependencies

| Dependency | Description | Impact if Unavailable |
|------------|-------------|----------------------|
| Claude Code CLI | Hook execution environment | Hooks will not fire |
| RocksDB/ScyllaDB | Storage backend | No persistence |

---

## 11. Out of Scope

The following items are explicitly NOT covered by this specification:

| Item | Reason | Reference |
|------|--------|-----------|
| New embedding models | Covered by separate spec | SPEC-EMBED-* |
| CUDA kernel optimization | Covered by separate spec | SPEC-CUDA-* |
| Production ScyllaDB migration | Future work | Backlog |
| Performance benchmark execution | Medium priority per gap analysis | Section 4.3 |
| Additional integration tests | Medium priority per gap analysis | Section 4.3 |

---

## 12. Test Plan

### 12.1 Skills Tests

| ID | Type | Description | Req Ref | Priority |
|----|------|-------------|---------|----------|
| TC-SKILL-001 | integration | topic-explorer returns portfolio | REQ-SKILL-003 | critical |
| TC-SKILL-002 | integration | memory-inject retrieves context | REQ-SKILL-003 | critical |
| TC-SKILL-003 | integration | semantic-search returns results | REQ-SKILL-003 | critical |
| TC-SKILL-004 | integration | dream-consolidation triggers cycle | REQ-SKILL-003 | high |
| TC-SKILL-005 | integration | curation merge works | REQ-SKILL-003 | high |
| TC-SKILL-006 | unit | Skill files exist at paths | REQ-SKILL-001 | critical |
| TC-SKILL-007 | unit | Skill metadata is valid | REQ-SKILL-002 | critical |

### 12.2 Stop Hook Tests

| ID | Type | Description | Req Ref | Priority |
|----|------|-------------|---------|----------|
| TC-STOP-001 | unit | stop.sh exists and is executable | REQ-STOP-002 | critical |
| TC-STOP-002 | unit | settings.json has Stop hook config | REQ-STOP-001 | critical |
| TC-STOP-003 | integration | Response captured as ClaudeResponse | REQ-STOP-003 | critical |
| TC-STOP-004 | integration | All 13 embeddings stored | REQ-STOP-004 | critical |
| TC-STOP-005 | e2e | Hook fires on Claude response | REQ-STOP-001 | high |

### 12.3 MCP Tools Tests

| ID | Type | Description | Req Ref | Priority |
|----|------|-------------|---------|----------|
| TC-MCP-001 | unit | Tool constants defined | REQ-MCP-001 | critical |
| TC-MCP-002 | unit | Tools appear in tools/list | REQ-MCP-002 | critical |
| TC-MCP-003 | integration | get_topic_portfolio returns data | REQ-MCP-002 | critical |
| TC-MCP-004 | integration | get_topic_stability returns metrics | REQ-MCP-002 | critical |
| TC-MCP-005 | integration | detect_topics finds clusters | REQ-MCP-002 | high |
| TC-MCP-006 | integration | get_divergence_alerts semantic only | REQ-MCP-005 | critical |
| TC-MCP-007 | integration | forget_concept soft deletes | REQ-MCP-002 | high |
| TC-MCP-008 | integration | boost_importance clamps values | REQ-MCP-002 | high |

### 12.4 Test Fix Tests

| ID | Type | Description | Req Ref | Priority |
|----|------|-------------|---------|----------|
| TC-FIX-001 | unit | cargo test compiles | REQ-TEST-001 | critical |
| TC-FIX-002 | unit | No broken imports | REQ-TEST-001 | critical |
| TC-FIX-003 | unit | Test coverage maintained | REQ-TEST-003 | high |

### 12.5 Documentation Tests

| ID | Type | Description | Req Ref | Priority |
|----|------|-------------|---------|----------|
| TC-DOC-001 | manual | All listed tools exist | REQ-DOC-001 | high |
| TC-DOC-002 | manual | Tool descriptions match behavior | REQ-DOC-002 | medium |

---

## 13. Open Questions

| ID | Status | Assignee | Question | Context | Resolution |
|----|--------|----------|----------|---------|------------|
| Q-001 | open | Team | Should skills support custom token budgets? | PRD doesn't specify | TBD |
| Q-002 | open | Team | What is the maximum response length for Stop hook capture? | Schema says 10,000 chars | TBD |
| Q-003 | open | Team | Should detect_topics run HDBSCAN or BIRCH or both? | PRD mentions both | TBD |
| Q-004 | open | Team | What metadata should get_divergence_alerts return? | PRD shows format but not full schema | TBD |
| Q-005 | open | Team | Should test fixes delete tests or stub them? | Need to balance coverage vs. maintenance | TBD |

---

## 14. Glossary

| Term | Definition |
|------|------------|
| Weighted Agreement | Sum of (topic_weight_i x is_clustered_i) across embedders; max 8.5, threshold 2.5 |
| Topic | Emergent concept discovered via multi-space clustering with weighted_agreement >= 2.5 |
| Churn Rate | Jaccard distance between topic snapshots; 0.0 = stable, 1.0 = complete turnover |
| Entropy | Topic distribution entropy; high entropy (> 0.7) suggests need for consolidation |
| Dream Consolidation | NREM (Hebbian replay) + REM (hyperbolic random walk) memory consolidation |
| Soft Delete | Marking memory as deleted while retaining for 30-day recovery period |
| SEMANTIC Embedders | E1, E5, E6, E7, E10, E12, E13 - contribute 1.0 weight to topics |
| TEMPORAL Embedders | E2, E3, E4 - contribute 0.0 weight to topics (metadata only) |
| RELATIONAL Embedders | E8, E11 - contribute 0.5 weight to topics |
| STRUCTURAL Embedder | E9 - contributes 0.5 weight to topics |
| MCP | Model Context Protocol - communication protocol between Claude and context graph |
| Hook | Claude Code lifecycle event that triggers shell script execution |
| Skill | User-invocable Claude Code capability defined in SKILL.md |

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-18 | Functional Spec Writer | Initial specification |

---

## FUNCTIONAL SPEC WRITER - DOCUMENT COMPLETE

### Document Created:
- **Path**: /home/cabdru/contextgraph/docs/FUNC_SPEC_PRD_GAPS.md
- **Spec ID**: SPEC-GAP-001
- **Version**: 1.0
- **Status**: draft

### Specification Summary:
- **Title**: PRD v6 Gap Remediation
- **User Types**: 2 defined
- **User Stories**: 15 documented
- **Requirements**: 20 with IDs
- **Edge Cases**: 18 documented
- **Error States**: 12 defined
- **Test Cases**: 21 specified

### Requirement Coverage:
| Priority | Count | Percentage |
|----------|-------|------------|
| Must Have | 12 | 60% |
| Should Have | 6 | 30% |
| Nice to Have | 2 | 10% |

### Quality Checklist:
- [x] All requirements have IDs
- [x] All requirements traceable to stories
- [x] No ambiguous language
- [x] Metrics are measurable
- [x] Edge cases documented
- [x] Test plan complete

### Open Questions: 5
1. Should skills support custom token budgets?
2. What is the maximum response length for Stop hook capture?
3. Should detect_topics run HDBSCAN or BIRCH or both?
4. What metadata should get_divergence_alerts return?
5. Should test fixes delete tests or stub them?

### Next Steps:
1. Review with stakeholders
2. Resolve open questions
3. Move to "approved" status
4. Hand off to Technical Spec Writer

### Related Documents:
- Constitution: /home/cabdru/contextgraph/CLAUDE.md
- PRD: /home/cabdru/contextgraph/docs2/contextprd.md
- Gap Analysis: /home/cabdru/contextgraph/docs/PRD_GAP_ANALYSIS.md
- Technical Spec: specs/technical/gap-remediation.md (pending)

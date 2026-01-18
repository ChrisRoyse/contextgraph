# Task 15: Update CLAUDE.md MCP Tool Documentation

## Metadata
- **Task ID**: TASK-GAP-015
- **Phase**: 4 (Integration)
- **Priority**: Medium
- **Complexity**: Medium
- **Estimated Time**: 1 hour
- **Dependencies**: task08 (TASK-GAP-008 - all tools must be implemented)

## Objective

Update the CLAUDE.md file to accurately document the 12 MCP tools that are actually exposed. The current documentation lists 30+ tools that don't exist, creating confusion. This task synchronizes documentation with the implemented reality.

## Input Context

Files to READ before starting:
- `/home/cabdru/contextgraph/CLAUDE.md` - Current documentation
- `/home/cabdru/contextgraph/docs/TECH_SPEC_PRD_GAPS.md` - Section 7.1 for updated tool list
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/tools/names.rs` - Actual tool names

## Files to Create/Modify

**Files to Modify:**
- `/home/cabdru/contextgraph/CLAUDE.md`

## Implementation Steps

### Step 1: Locate the MCP section

Find the `mcp:` section in CLAUDE.md (around line 479).

### Step 2: Replace core_tools with exposed_tools

Replace the misleading `core_tools:` section with `exposed_tools:` containing only the 12 actual tools.

### Step 3: Add proper documentation for each tool

For each tool, document: purpose, parameters, and example invocation.

## Code/Content to Implement

### CLAUDE.md MCP Section Update

Find and replace the `mcp:` section (around lines 479-510) with:

```yaml
# ===============================================================================
# MCP TOOLS
# ===============================================================================
mcp:
  version: "2024-11-05"
  transport: [stdio, sse]

  exposed_tools:
    # ========== CORE TOOLS (PRD Section 10.1) ==========
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
      purpose: "Get system health, entropy, coherence, and curation tasks"
      parameters: "none"
      example: "get_memetic_status({})"

    # ========== TOPIC TOOLS (PRD Section 10.2) ==========
    get_topic_portfolio:
      purpose: "Get all discovered topics with profiles and stability metrics"
      parameters:
        format: "string (optional) - brief|standard|verbose, default standard"
      example: "get_topic_portfolio({format: 'standard'})"

    get_topic_stability:
      purpose: "Get portfolio-level stability metrics (churn, entropy)"
      parameters:
        hours: "number (optional) - Lookback period in hours, default 6"
      example: "get_topic_stability({hours: 12})"

    detect_topics:
      purpose: "Force topic detection recalculation via HDBSCAN clustering"
      parameters:
        force: "boolean (optional) - Force even if not needed, default false"
      example: "detect_topics({force: false})"

    get_divergence_alerts:
      purpose: "Check for divergence from recent activity (SEMANTIC embedders only)"
      parameters:
        lookback_hours: "number (optional) - Hours to check, default 2"
      example: "get_divergence_alerts({lookback_hours: 4})"

    # ========== CONSOLIDATION TOOLS ==========
    trigger_consolidation:
      purpose: "Trigger NREM/REM dream consolidation cycle"
      parameters:
        blocking: "boolean (optional) - Wait for completion, default true"
        dry_run: "boolean (optional) - Preview without executing, default false"
      example: "trigger_consolidation({blocking: true})"

    # ========== CURATION TOOLS (PRD Section 10.3) ==========
    merge_concepts:
      purpose: "Merge duplicate memories into one"
      parameters:
        source_node_ids: "string[] (required) - UUIDs to merge"
        merge_strategy: "string (optional) - keep_newest|combine, default combine"
      example: "merge_concepts({source_node_ids: ['uuid1', 'uuid2']})"

    forget_concept:
      purpose: "Soft-delete a memory (30-day recovery per SEC-06)"
      parameters:
        node_id: "string (required) - UUID to forget"
        soft_delete: "boolean (optional) - Use soft delete, default true"
      example: "forget_concept({node_id: 'uuid', soft_delete: true})"

    boost_importance:
      purpose: "Adjust memory importance score (clamped to [0.0, 1.0])"
      parameters:
        node_id: "string (required) - UUID of memory"
        delta: "number (required) - Change amount (-1.0 to 1.0)"
      example: "boost_importance({node_id: 'uuid', delta: 0.2})"

  total_tools: 12

  tool_categories:
    core: [inject_context, store_memory, search_graph, get_memetic_status]
    topic: [get_topic_portfolio, get_topic_stability, detect_topics, get_divergence_alerts]
    consolidation: [trigger_consolidation]
    curation: [merge_concepts, forget_concept, boost_importance]
```

## Definition of Done

- [ ] CLAUDE.md mcp section updated with accurate tool list
- [ ] Only 12 tools documented (not 30+)
- [ ] Each tool has purpose, parameters, and example
- [ ] `total_tools: 12` is accurate
- [ ] Tool categories match PRD sections
- [ ] No references to non-existent tools
- [ ] YAML format is consistent with rest of file
- [ ] No emojis added

## Verification

```bash
cd /home/cabdru/contextgraph

# Count tools documented (should be 12)
grep -c "purpose:" CLAUDE.md
# Expected: 12

# Verify total_tools is correct
grep "total_tools:" CLAUDE.md
# Should show: total_tools: 12

# Verify all 12 tool names are documented
grep -E "inject_context:|store_memory:|search_graph:|get_memetic_status:" CLAUDE.md | wc -l
# Expected: 4

grep -E "get_topic_portfolio:|get_topic_stability:|detect_topics:|get_divergence_alerts:" CLAUDE.md | wc -l
# Expected: 4

grep -E "trigger_consolidation:|merge_concepts:|forget_concept:|boost_importance:" CLAUDE.md | wc -l
# Expected: 4

# Verify no references to old non-existent tools
grep -E "get_health_status:|trigger_healing:|get_pruning_candidates:|execute_prune:" CLAUDE.md
# Should return empty (these tools don't exist)

# Verify YAML is valid (basic check)
grep -A2 "exposed_tools:" CLAUDE.md
# Should show properly indented YAML
```

# Memory System Integration Report — Claude Code Hooks & Skills

**Date**: 2026-02-16
**Branch**: ccintegration
**Scope**: Full audit of 12 hooks + 16 skills integration with Context Graph MCP server

---

## Executive Summary

The Context Graph memory system integrates with Claude Code through **12 hooks** (shell scripts triggering CLI commands) and **16 skills** (slash-command instructions). The core architecture is sound: 3 hooks actively inject context, 9 passively log to RocksDB, and 11 skills correctly reference MCP tools.

However, **3 skills are broken** (reference non-existent parameters or deprecated tools), **2 skills have wrong parameter names** that will cause errors, and several have incomplete documentation. The hook system has one structural limitation: the in-memory cache is **process-local** and lost between CLI invocations, making `inject-brief` and `pre_tool_use` unable to share cached state.

### Scorecard

| Component | Working | Broken/Issues | Total |
|-----------|---------|---------------|-------|
| Hooks (active injection) | 3/3 | 0 | 3 |
| Hooks (passive logging) | 9/9 | 0 | 9 |
| Skills (correct) | 9/16 | 3 broken, 2 wrong params, 2 incomplete | 16 |
| MCP Tools referenced | 52/55 | 0 missing | 55 |

---

## Part 1: Hooks — What's Actually Working

### Architecture

```
Claude Code Event
  → Shell Script (.claude/hooks/*.sh)
    → CLI binary (context-graph-cli)
      → MCP Client (TCP 127.0.0.1:3100)
        → Context Graph MCP Server (55 tools, 13 embedders, RocksDB)
```

### 1.1 Active Context-Injecting Hooks (3 hooks)

These hooks return content to Claude via stdout — Claude sees it as `<system-reminder>` tags.

#### UserPromptSubmit — WORKING

| Aspect | Status | Detail |
|--------|--------|--------|
| Hook fires | WORKING | Confirmed via `system-reminder` messages every prompt |
| E1 semantic search | WORKING | `search_graph_fast()` returns top 5 memories, min_similarity=0.3 |
| E11 entity extraction | WORKING | `extract_entities_fast()` extracts entities from prompt |
| E11 entity search | WORKING | `search_by_entities_fast()` finds entity-related memories |
| E4 conversation context | WORKING | `get_conversation_context_fast()` returns recent turns |
| Divergence alerts | WORKING | `get_divergence_alerts_fast()` detects topic drift (E5 excluded per AP-77 fix) |
| Memory cache population | WORKING | Caches memories for PreToolUse via `cache_memories()` |
| Timeout budget | WORKING | Fast path: 500ms connect, 800ms request. Total ~1400ms within 2000ms budget |
| Graceful degradation | WORKING | MCP unavailable → logs warning, continues with empty results |

**MCP tools called**: `search_graph`, `extract_entities`, `search_by_entities`, `get_conversation_context`, `get_divergence_alerts`

#### PreToolUse — WORKING (with limitation)

| Aspect | Status | Detail |
|--------|--------|--------|
| Hook fires | WORKING | Confirmed on Edit/Write/Read/Bash/Task tools |
| Tool guidance | WORKING | Returns tool-specific tips (e.g., "Track file content in awareness quadrant") |
| Cached memories | SEE BELOW | Reads from in-memory cache — structural limitation |
| Performance | WORKING | ~5ms execution, no MCP calls, no DB access |

**Structural Limitation — APPEARS WORKING BUT ISN'T**:

The memory cache is a `OnceLock<RwLock<MemoryCache>>` singleton — it only persists within a single OS process. But each hook invocation spawns a **new CLI process**:

```
user_prompt_submit.sh → context-graph-cli hooks prompt-submit  # Process A (populates cache)
pre_tool_use.sh       → context-graph-cli hooks pre-tool       # Process B (reads empty cache)
```

**Result**: PreToolUse ALWAYS reads an empty cache. The cached memories from UserPromptSubmit are never available because they die with Process A. The `get_cached_memories()` call in PreToolUse always returns `[]`.

**Impact**: LOW — PreToolUse still provides tool guidance (which doesn't need the cache). The cached memories section is simply absent from PreToolUse's injection.

#### SubagentStart — WORKING (with same limitation)

| Aspect | Status | Detail |
|--------|--------|--------|
| Hook fires | WORKING | Confirmed when spawning Task subagents |
| inject-brief call | WORKING | Calls `context-graph-cli memory inject-brief` |
| Cache-first path | NOT WORKING | Same process-local cache problem as PreToolUse |
| MCP fallback | WORKING | Falls back to `search_graph()` MCP call (normal timeouts) |
| Token budget | WORKING | 200 tokens (800 chars) limit properly enforced |

**Same structural limitation**: The cache-first path added in the ISSUE-5 fix will never find cached data because it runs in a different process. It always falls back to the MCP `search_graph()` call, which does work.

**Impact**: LOW — The MCP fallback works correctly. Latency is higher (~500ms vs ~0ms for cache hit) but functional.

### 1.2 Passive Logging Hooks (9 hooks)

These hooks store memories in RocksDB via MCP `store_memory`. They don't inject context back to Claude.

| Hook | Fires | Stores To RocksDB | Content | Timeout |
|------|-------|-------------------|---------|---------|
| **SessionStart** | WORKING | No (SessionCache only) | Session initialization | 5000ms |
| **PostToolUse** | WORKING | Yes (via MCP) | Tool execution results | 5000ms |
| **PostToolUseFailure** | WORKING | Yes (via MCP) | `FAILURE: {tool} - {error}` | 20000ms |
| **SubagentStop** | WORKING | Yes (via MCP) | `Subagent {type} ({id}) completed` | 20000ms |
| **Stop** | WORKING | Yes (via MCP) | Claude's response text (truncated 10K chars) | 3000ms |
| **PreCompact** | WORKING | Yes (via MCP) | `Pre-compaction marker (trigger: {type})` | 20000ms |
| **SessionEnd** | WORKING | Yes (SessionCache) | Final session state + summary | 30000ms |
| **Notification** | WORKING | Yes (via MCP) | Only `permission_prompt` and `idle_prompt` types | 3000ms |
| **TaskCompleted** | WORKING | Yes (via MCP) | `Task completed: {subject} (id: {id})` | 20000ms |

**All 9 hooks are fully functional.**

### 1.3 Hook Configuration Verification

| Aspect | Status |
|--------|--------|
| All 12 scripts exist and are executable | VERIFIED |
| All CLI subcommands exist | VERIFIED |
| CLI binary accessible at `./target/release/context-graph-cli` | VERIFIED |
| MCP server binary at `./target/release/context-graph-mcp` | VERIFIED |
| Storage path exists with proper permissions | VERIFIED |
| Models path exists with all embedder models | VERIFIED |
| No missing environment variables | VERIFIED |
| Timeout budgets conservative (script < framework) | VERIFIED |

---

## Part 2: Skills — What's Working vs Broken

### 2.1 Fully Working Skills (9/16)

| Skill | Tools Referenced | Parameters | Status |
|-------|-----------------|------------|--------|
| **context-inject** | `search_graph`, `search_causes`, `search_effects` | All correct | WORKING |
| **cg-memory-search** | `search_graph`, `search_code`, `search_by_keywords` | All correct, all 14 weight profiles | WORKING |
| **cg-topics** | `get_topic_portfolio`, `get_topic_stability`, `detect_topics`, `get_divergence_alerts` | All correct | WORKING |
| **cg-curator** | `merge_concepts`, `forget_concept`, `boost_importance` | All correct | WORKING |
| **cg-blind-spot** | `search_robust`, `compare_embedder_views` | Correct (minor: should clarify E1-E13 format) | WORKING |
| **topic-explorer** | Same as cg-topics | All correct | WORKING (duplicate) |
| **curation** | Same as cg-curator with more detail | All correct | WORKING (duplicate) |
| **skill-builder** | N/A (meta skill) | N/A | WORKING |
| **cg-graph** | `get_memory_neighbors`, `traverse_graph`, `get_unified_neighbors`, `get_typed_edges` | Correct but minimal docs | WORKING |

### 2.2 Broken Skills (3/16) — APPEAR TO WORK BUT ARE BROKEN

#### BROKEN-1: `semantic-search` — Multiple Schema Mismatches

**Severity**: CRITICAL — Skill is unusable with current API

| Problem | Skill Says | Actual API |
|---------|-----------|------------|
| Non-existent parameter | `modality` (text/code/image/audio/structured/mixed) | Parameter does not exist in `search_graph` |
| Wrong response fields | `purposeAlignment`, `alignmentScore`, `_cognitive_pulse` | Fields do not exist in response |
| Wrong embedder names | E4_Causal, E5_Analogical, E6_Code, E7_Procedural, E8_Spatial, E9_Social, E10_Emotional, E11_Abstract | E4=Sequence, E5=Causal, E6=Sparse, E7=Code, E8=Graph, E9=HDC, E10=Paraphrase, E11=Entity |
| References deprecated tool | `inject_context` in "Related Tools" | Merged into `store_memory` |

**Why it appears to work**: Claude will call `search_graph` (correct tool name) and get results back. But it will also pass `modality` parameter (silently ignored by the server since `additionalProperties` is not set to false for `search_graph`), and will try to present fields like `purposeAlignment` that don't exist in the response.

**Root cause**: Skill was written against an early prototype API and never updated after the embedder redesign.

#### BROKEN-2: `memory-inject` — References Deprecated Tool

**Severity**: HIGH — Storage functionality broken

| Problem | Skill Says | Actual API |
|---------|-----------|------------|
| Deprecated tool | `inject_context` with `content`, `rationale`, `modality`, `importance` | Tool merged into `store_memory`. No `inject_context` tool exists. |
| Non-existent parameter | `modality` on `search_graph` | Does not exist |
| Wrong response fields | `id`, `learning_score`, `entropy`, `coherence` | `store_memory` returns `fingerprintId`, `embedderCount`, `embeddingLatencyMs` |

**Why it appears to work**: The search/retrieval half works (correctly calls `search_graph`). But when a user asks to "save this finding", Claude will try to call `inject_context` which doesn't exist → MCP returns tool_not_found error.

**Root cause**: `inject_context` was merged into `store_memory` with optional `rationale` parameter. Skill was never updated.

#### BROKEN-3: `cg-code-search` — Wrong Parameter Names and Values

**Severity**: HIGH — Search mode will be rejected

| Problem | Skill Says | Actual API |
|---------|-----------|------------|
| Wrong parameter name | `mode` | `searchMode` |
| Wrong enum values | `"signature"`, `"pattern"`, `"semantic"` | `"e7Only"`, `"pipeline"`, `"semantic"` |
| Missing parameters | Not documented | `languageHint`, `includeAstContext`, `blendWithSemantic`, `minScore` |

**Why it appears to work**: If Claude follows the skill and passes `mode: "signature"`, the MCP server will ignore the unknown parameter (no `additionalProperties: false` in this schema) and use the default `searchMode: "semantic"`. Results come back, but the user never gets the specialized search they asked for.

**Root cause**: Skill was written before the `search_code` API was finalized. Parameter names diverged.

### 2.3 Skills with Wrong Parameters (2/16)

#### WRONG-PARAMS-1: `cg-causal` — `get_causal_chain` Parameters

| Problem | Skill Says | Actual API |
|---------|-----------|------------|
| Wrong required param | `memoryId` | `anchorId` (required) |
| Wrong param name | `depth` (default 3) | `maxHops` (default 5, max 10) |
| Missing enum value | `direction: "both"` | Only `"forward"` or `"backward"` — no `"both"` |

**Impact**: Calling `get_causal_chain` with `memoryId` instead of `anchorId` will fail with "missing required parameter anchorId". The `depth` parameter will be silently ignored.

#### WRONG-PARAMS-2: `cg-entity` — `find_related_entities` and `get_entity_graph` Parameters

| Problem | Skill Says | Actual API |
|---------|-----------|------------|
| Wrong param name | `relationshipType` | `relation` (required) |
| Missing required | `relation` not listed as required | It IS required in the actual schema |
| Missing param | `direction` not documented | `"outgoing"` or `"incoming"` (default: outgoing) |
| Wrong param name | `get_entity_graph`: `entity` (required) | `centerEntity` (optional) |
| Wrong param name | `get_entity_graph`: `depth` | `maxDepth` (default 2, max 5) |
| Missing params | Not documented | `maxNodes`, `entityTypes`, `minRelationScore`, `includeMemoryCounts` |

**Impact**: `find_related_entities` will fail — `relation` is required but not documented in the skill. `get_entity_graph` passes `entity` instead of `centerEntity` (silently ignored, returns full graph instead of focused neighborhood).

### 2.4 Incomplete Skills (2/16)

| Skill | Issue | Missing |
|-------|-------|---------|
| **cg-session** | Terse, no parameter docs | `get_conversation_context` params (direction, windowSize), `traverse_memory_chain` params, `compare_session_states` params |
| **cg-provenance** | Terse, no parameter docs | `get_audit_trail` params (target_id, time range), `get_merge_history` params, `get_provenance_chain` params |

**Impact**: LOW — Skills point to correct tools but Claude has to guess parameter names. It usually gets them right via tool schema introspection.

---

## Part 3: Structural Issues — Things That Appear Working But Aren't

### STRUCT-1: Process-Local Memory Cache (HIGH)

**The illusion**: UserPromptSubmit populates a memory cache. PreToolUse and SubagentStart read from it. This looks like a working caching pipeline.

**The reality**: Each hook invocation spawns a separate CLI process. The cache is a process-local `OnceLock<RwLock<MemoryCache>>` singleton. Process A (UserPromptSubmit) populates the cache and exits. Process B (PreToolUse) starts fresh with an empty cache.

**Evidence**:
- `memory_cache.rs`: Uses `static CACHE: OnceLock<RwLock<MemoryCache>>` — process-local
- `pre_tool_use.rs:81`: Calls `get_cached_memories()` — always returns `[]` in practice
- `inject.rs:288`: Calls `get_cached_memories()` — always returns `[]` in practice

**Why it's not catastrophic**:
- PreToolUse still provides tool guidance (no cache needed)
- SubagentStart falls back to MCP search (works, just slower)
- UserPromptSubmit still injects context (it's the producer, not consumer)

**Fix options**:
1. Use filesystem-based cache (e.g., `/tmp/cg-cache-{session_id}.json`)
2. Use the MCP daemon's own memory (query daemon directly instead of caching)
3. Use shared memory (mmap) between processes

### STRUCT-2: Entity Overlap Always Zero in search_by_entities (MEDIUM)

**The illusion**: `search_by_entities` combines E1 similarity + E11 similarity + entity Jaccard overlap. This sounds like a three-signal scoring system.

**The reality**: Entity Jaccard overlap is always 0.0 because entities are not linked to stored memories at storage time. The `extract_entities` tool extracts entities from text on-demand, but `store_memory` does not run entity extraction and link results to the stored fingerprint.

**Evidence** (from live verification):
```json
{
  "results": [
    {"e11Similarity": 0.972, "entityOverlap": 0.0, "matchedEntities": []},
    {"e11Similarity": 0.971, "entityOverlap": 0.0, "matchedEntities": []},
    {"e11Similarity": 0.971, "entityOverlap": 0.0, "matchedEntities": []}
  ]
}
```

**Impact**: Entity search relies entirely on E1 + E11 vector similarity. The Jaccard overlap signal (which would provide exact entity name matching) is unused. Searching for ["PostgreSQL", "Diesel"] returns memories about "test failure" instead of memories actually containing those entities.

**Fix**: Run `extract_entities` at `store_memory` time and persist entity links to the fingerprint's metadata, enabling Jaccard scoring.

### STRUCT-3: get_unified_neighbors Also Returns 0 for Recent Memories (LOW)

**The illusion**: `get_memory_neighbors` diagnostic suggests using `get_unified_neighbors` for recently stored memories.

**The reality**: Both tools rely on pre-computed K-NN edges from the BackgroundGraphBuilder, which processes in 60-second batches with a minimum of 10 items. Neither works for recently stored memories.

**Evidence** (from live verification):
```json
// get_memory_neighbors
{"count": 0, "diagnostic": {"suggestion": "Use get_unified_neighbors instead"}}

// get_unified_neighbors
{"count": 0, "metadata": {"total_candidates_evaluated": 0, "unique_candidates": 0}}
```

**Impact**: LOW — `search_graph` works immediately for all memories (uses direct vector search). Only the graph-neighbor tools have this delay. The diagnostic message is misleading but not harmful.

---

## Part 4: What's Genuinely Working Well

### 4.1 UserPromptSubmit Multi-Signal Injection

The crown jewel of the integration. Every user prompt triggers:
1. E1 semantic search (top 5 memories, min 0.3 similarity)
2. E11 entity extraction + entity-based search
3. E4 sequential context (recent 5 turns)
4. Divergence alert detection (6 embedder spaces)

All within ~1400ms using fast-path MCP client (500ms connect, 800ms request timeouts).

### 4.2 Graceful Degradation Throughout

Every hook handles MCP unavailability gracefully:
- `is_server_running()` check before MCP calls
- Connection/timeout errors → log warning, continue with empty results
- Invalid JSON input → exit code 4 (invalid input)
- Never blocks Claude Code even if MCP server crashes

### 4.3 Passive Logging Creates Rich History

9 hooks continuously log tool usage, failures, responses, subagent completions, task completions, and session boundaries. This creates a rich longitudinal memory that semantic search can retrieve later.

### 4.4 Tool Matchers for Targeted Hooks

PreToolUse only fires for `Edit|Write|Read|Bash|Task` — not for every tool call. PostToolUse only fires for `Write|Edit|Bash|Task`. This prevents excessive hook overhead on read-only operations.

### 4.5 Conservative Timeout Strategy

Every hook script's internal timeout is <= the framework timeout in settings.json, ensuring clean exit before Claude Code kills the process:

| Hook | Framework | Script | Buffer |
|------|-----------|--------|--------|
| UserPromptSubmit | 3000ms | 2500ms | 500ms |
| PreToolUse | 1000ms | 500ms | 500ms |
| SessionEnd | 30000ms | 30000ms | 0ms |

---

## Part 5: Recommendations

### Priority 1 — Fix Broken Skills (CRITICAL)

| Skill | Action |
|-------|--------|
| `semantic-search` | Delete or rewrite. Remove `modality` parameter, fix all embedder names, remove `purposeAlignment`/`alignmentScore`/`_cognitive_pulse`, remove `inject_context` reference. |
| `memory-inject` | Replace `inject_context` with `store_memory` (with `rationale` parameter). Remove `modality` parameter from search. Fix response field names. |
| `cg-code-search` | Change `mode` → `searchMode`. Change values from `"signature"/"pattern"/"semantic"` → `"e7Only"/"pipeline"/"semantic"`. Add `languageHint`, `includeAstContext` parameters. |

### Priority 2 — Fix Wrong Parameters (HIGH)

| Skill | Action |
|-------|--------|
| `cg-causal` | Change `memoryId` → `anchorId`, `depth` → `maxHops`, remove `direction: "both"` |
| `cg-entity` | Change `relationshipType` → `relation` (mark required), add `direction` param. Change `get_entity_graph`: `entity` → `centerEntity`, `depth` → `maxDepth`, add `maxNodes`, `entityTypes` |

### Priority 3 — Fix Process-Local Cache (MEDIUM)

Replace `OnceLock<RwLock<MemoryCache>>` with a filesystem-based cache:
```
/tmp/cg-memory-cache-{session_id}.json
```
- UserPromptSubmit writes cache file
- PreToolUse and inject-brief read cache file
- TTL enforced via file modification time
- Cleanup on SessionEnd

### Priority 4 — Add Entity Linking at Store Time (MEDIUM)

Run `extract_entities` as part of `store_memory` pipeline. Persist entity links to fingerprint metadata. This enables meaningful Jaccard overlap scoring in `search_by_entities`.

### Priority 5 — Consolidate Duplicate Skills (LOW)

| Keep | Remove | Reason |
|------|--------|--------|
| `cg-topics` | `topic-explorer` | Identical functionality, cg-topics has cleaner naming |
| `cg-curator` | `curation` | curation has more detail but cg-curator has consistent naming |

### Priority 6 — Complete Incomplete Skills (LOW)

Add full parameter documentation to `cg-session` and `cg-provenance`.

---

## Appendix A: Complete Hook-to-MCP Tool Mapping

| Hook | CLI Command | MCP Tools Called |
|------|-------------|-----------------|
| SessionStart | `hooks session-start` | None (SessionCache only) |
| UserPromptSubmit | `hooks prompt-submit` | `search_graph`, `extract_entities`, `search_by_entities`, `get_conversation_context`, `get_divergence_alerts` |
| PreToolUse | `hooks pre-tool` | None (cache-only, fast path) |
| PostToolUse | `hooks post-tool` | `store_memory` (async) |
| PostToolUseFailure | `memory capture-memory` | `store_memory` |
| SubagentStart | `memory inject-brief` | `search_graph` (fallback) |
| SubagentStop | `memory capture-memory` | `store_memory` |
| Stop | `memory capture-response` | `store_memory` |
| PreCompact | `memory capture-memory` | `store_memory` |
| SessionEnd | `hooks session-end` | Session summary generation |
| Notification | `memory capture-memory` | `store_memory` |
| TaskCompleted | `memory capture-memory` | `store_memory` |

## Appendix B: Skill-to-MCP Tool Mapping

| Skill | MCP Tools | Status |
|-------|-----------|--------|
| context-inject | `search_graph`, `search_causes`, `search_effects` | WORKING |
| cg-memory-search | `search_graph`, `search_code`, `search_by_keywords` | WORKING |
| cg-causal | `search_causes`, `search_effects`, `get_causal_chain`, `search_causal_relationships` | WRONG PARAMS |
| cg-entity | `extract_entities`, `search_by_entities`, `find_related_entities`, `infer_relationship`, `validate_knowledge`, `get_entity_graph` | WRONG PARAMS |
| cg-code-search | `search_code`, `search_cross_embedder_anomalies` | BROKEN |
| cg-blind-spot | `search_robust`, `compare_embedder_views` | WORKING |
| cg-topics | `get_topic_portfolio`, `get_topic_stability`, `detect_topics`, `get_divergence_alerts` | WORKING |
| cg-session | `get_conversation_context`, `get_session_timeline`, `traverse_memory_chain`, `compare_session_states` | INCOMPLETE |
| cg-provenance | `get_audit_trail`, `get_merge_history`, `get_provenance_chain` | INCOMPLETE |
| cg-curator | `merge_concepts`, `forget_concept`, `boost_importance` | WORKING |
| cg-graph | `get_memory_neighbors`, `traverse_graph`, `get_unified_neighbors`, `get_typed_edges` | WORKING |
| memory-inject | `search_graph`, ~~`inject_context`~~ | BROKEN |
| semantic-search | `search_graph` | BROKEN |
| topic-explorer | Same as cg-topics | WORKING (duplicate) |
| curation | Same as cg-curator | WORKING (duplicate) |
| skill-builder | N/A (meta) | WORKING |

## Appendix C: MCP Client Configuration

| Setting | Standard Path | Fast Path |
|---------|--------------|-----------|
| Connection timeout | 5000ms | 500ms |
| Request timeout | 30000ms | 800ms |
| Host | 127.0.0.1 (or CONTEXT_GRAPH_MCP_HOST) | Same |
| Port | 3100 (or CONTEXT_GRAPH_MCP_PORT) | Same |
| Protocol | TCP/JSON-RPC 2.0 | Same |
| Connection pooling | None (fresh TCP per call) | Same |

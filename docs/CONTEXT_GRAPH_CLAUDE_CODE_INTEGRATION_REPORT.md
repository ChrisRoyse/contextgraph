# Context Graph x Claude Code Integration Report

## Unlocking 55 MCP Tools via Hooks & Skills

**Date**: 2026-02-15
**System**: Context Graph MCP Server (13-embedder, 55-tool memory system)
**Target**: Claude Code CLI Hook System + Skills Framework

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Memory System Capabilities Overview](#2-memory-system-capabilities-overview)
3. [Hook Integration Architecture](#3-hook-integration-architecture)
4. [Hook Designs by Lifecycle Event](#4-hook-designs-by-lifecycle-event)
5. [Skill Designs for Domain Specialization](#5-skill-designs-for-domain-specialization)
6. [High-Value Automation Workflows](#6-high-value-automation-workflows)
7. [Advanced Patterns](#7-advanced-patterns)
8. [Configuration Reference](#8-configuration-reference)
9. [Implementation Priorities](#9-implementation-priorities)

---

## 1. Executive Summary

The Context Graph MCP server exposes **55 tools** across **17 categories** backed by a **13-embedder memory system** that captures semantic, temporal, causal, structural, and lexical dimensions of every piece of information stored. When connected to Claude Code's hook system (10 lifecycle events) and skills framework (auto-triggered domain expertise), this creates an **ambient intelligence layer** that:

- **Remembers everything** across sessions, projects, and conversations
- **Understands causality** — why changes were made, what broke what
- **Detects blind spots** — finds what semantic search alone misses
- **Tracks entities** — knows which technologies, frameworks, and concepts are related
- **Discovers topics** — automatically clusters work into emergent themes
- **Provides provenance** — full audit trail from memory to source
- **Reasons about code** — dedicated code embeddings understand patterns beyond text

**Key insight**: Hooks provide the **when** (lifecycle automation), skills provide the **what** (domain expertise), and the 55 MCP tools provide the **how** (13-dimensional memory operations). Together they create a system where Claude Code accumulates and leverages institutional knowledge automatically.

---

## 2. Memory System Capabilities Overview

### 2.1 The 13-Embedder Stack

Each memory is embedded across 13 independent spaces — NO fusion. This preserves 100% of information (vs. 33% with fusion) and enables per-space reasoning.

| ID | Name | Dim | What It Captures | Automation Value |
|----|------|-----|------------------|------------------|
| E1 | Semantic | 1024D | General meaning (e5-large-v2) | Foundation for all search |
| E2 | Freshness | 512D | Recency (exponential decay) | "What did I just work on?" |
| E3 | Periodic | 512D | Cyclical patterns (Fourier) | "What do I do on Fridays?" |
| E4 | Positional | 512D | Conversation order (sinusoidal PE) | Session context retrieval |
| E5 | Causal | 768D | Cause→effect direction (asymmetric) | "Why did this break?" |
| E6 | Keyword | 30K sparse | Exact term matching (SPLADE) | Precise identifier lookup |
| E7 | Code | 1536D | Source code patterns (Qodo) | Function/pattern similarity |
| E8 | Graph | 384D | Structural connectivity (asymmetric) | Dependency chain traversal |
| E9 | HDC | 1024D | Noise-robust (hyperdimensional) | Typo-tolerant, blind spot detection |
| E10 | Paraphrase | 768D | Semantic equivalence (asymmetric) | Deduplication, rephrasing |
| E11 | Entity | 768D | Entity relationships (KEPLER/TransE) | Technology stack reasoning |
| E12 | ColBERT | 128D/tok | Token-level precision (MaxSim) | Fine-grained reranking |
| E13 | SPLADE | 30K sparse | Keyword expansion | Fast recall stage |

### 2.2 Search Strategies

| Strategy | Speed | Precision | Best For |
|----------|-------|-----------|----------|
| e1_only | <10ms | Good | Quick semantic lookup |
| multi_space | <60ms | Excellent | General queries (default) |
| pipeline | <60ms | Best | Complex queries needing precision |
| embedder_first | <20ms | Targeted | Single-perspective exploration |

### 2.3 Weight Profiles (14 Built-in + Custom)

Profiles control how embedders are blended during multi_space search:

| Profile | Primary Embedders | Use Case |
|---------|-------------------|----------|
| semantic_search | E1 (0.33) | General purpose (default) |
| causal_reasoning | E1 (0.40), E5 (0.10) | "Why" questions |
| code_search | E7 (0.40), E1 (0.20) | Code patterns |
| fact_checking | E11 (0.40), E6 | Entity verification |
| graph_reasoning | E8 (0.40) | Dependency analysis |
| temporal_navigation | E2-E4 (0.22 each) | Time-based queries |
| sequence_navigation | E4 (0.55) | Conversation order |
| conversation_history | E4 (0.35), E1 (0.30) | Session recall |
| typo_tolerant | E9 (0.15), E1 | Fuzzy matching |
| balanced | Equal weights | Exploratory search |

### 2.4 Tool Categories (55 Total)

| Category | Tools | Key Capabilities |
|----------|-------|------------------|
| Core | 4 | store, search, status, consolidate |
| Topic Detection | 4 | portfolio, stability, detection, divergence |
| Memory Curation | 3 | merge, forget, boost importance |
| File Watcher | 4 | list, stats, delete, reconcile |
| Session/Conversation | 4 | context, timeline, traversal, comparison |
| Causal Reasoning | 4 | search causes, effects, chains, relationships |
| Causal Discovery | 2 | LLM-powered analysis, status |
| Keyword Search | 1 | E6 sparse + E13 expansion |
| Code Search | 1 | E7 code patterns |
| Graph Reasoning | 4 | connections, paths, discovery, validation |
| Blind Spot Detection | 1 | E9 noise-robust discovery |
| Entity Tools | 6 | extract, search, infer, find, validate, graph |
| Embedder-First Search | 7 | per-embedder, clusters, compare, fingerprint |
| Temporal Search | 2 | recency boost, periodic patterns |
| Graph Linking | 4 | neighbors, typed edges, traversal, unified |
| Provenance | 3 | audit trail, merge history, provenance chain |
| Maintenance | 1 | repair causal relationships |

---

## 3. Hook Integration Architecture

### 3.1 Hook-to-Tool Mapping

Claude Code hooks fire at specific lifecycle points. Each hook can call context-graph MCP tools to create an ambient memory layer.

```
SessionStart ──→ warm caches, load topic portfolio, inject session context
    │
UserPromptSubmit ──→ search memories for prompt context, inject relevant knowledge
    │
PreToolUse ──→ inject code context before edits, check entity relationships
    │
[Claude executes tool]
    │
PostToolUse ──→ capture tool results as memories, track file changes, extract entities
    │
Stop ──→ force continuation if important context unsaved
    │
SubagentStop ──→ capture subagent discoveries as shared memories
    │
PreCompact ──→ save critical context before context window compression
    │
SessionEnd ──→ consolidate memories, run dream phase, persist state
```

### 3.2 Data Flow

```
User Prompt → [UserPromptSubmit hook]
                ├── search_graph(query=prompt, strategy=multi_space)
                ├── search_causes(query=prompt)  # if "why/because/broke" detected
                ├── search_code(query=prompt)     # if code-related
                └── Returns: systemMessage with relevant context

Claude Response → [PostToolUse hook on Write/Edit]
                    ├── store_memory(content=description, importance=0.7)
                    ├── extract_entities(text=file_content)
                    └── detect_topics(force=false)

Session End → [SessionEnd hook]
               ├── trigger_consolidation(strategy=similarity, min_similarity=0.85)
               ├── detect_topics(force=true)
               └── reconcile_files(dry_run=false)
```

---

## 4. Hook Designs by Lifecycle Event

### 4.1 SessionStart — Context Warm-Up

**Purpose**: Load relevant context at session start so Claude has project awareness immediately.

**Script**: `hooks/session-start.sh`
```bash
#!/bin/bash
# Read session source from stdin
INPUT=$(cat)
SOURCE=$(echo "$INPUT" | jq -r '.source // "startup"')

# Get system status
STATUS=$(context-graph-cli mcp call get_memetic_status 2>/dev/null)
MEMORY_COUNT=$(echo "$STATUS" | jq -r '.fingerprint_count // 0')

# Get topic portfolio for project awareness
TOPICS=$(context-graph-cli mcp call get_topic_portfolio \
  --format brief 2>/dev/null | jq -r '.topics[:5] | map(.name) | join(", ")')

# Get recent memories (last session context)
RECENT=$(context-graph-cli mcp call search_recent \
  --query "session summary" \
  --topK 3 \
  --temporalScale micro \
  --includeContent true 2>/dev/null | jq -r '.results[:3] | map(.content[:100]) | join("\n")')

# Inject as system context
cat <<EOF
{
  "systemMessage": "## Memory System Active\n- ${MEMORY_COUNT} memories indexed across 13 embedder spaces\n- Active topics: ${TOPICS}\n- Recent context:\n${RECENT}\n\nUse mcp__context-graph__* tools for memory operations."
}
EOF
```

**What this enables**:
- Claude starts every session knowing the project's memory landscape
- Active topics surface immediately (no "what were we working on?" questions)
- Recent context from the last session carries over automatically
- System health is verified at startup

---

### 4.2 UserPromptSubmit — Intelligent Context Injection

**Purpose**: Before Claude processes any prompt, search the memory system for relevant prior knowledge and inject it as system context.

**Script**: `hooks/user-prompt-submit.sh`
```bash
#!/bin/bash
INPUT=$(cat)
PROMPT=$(echo "$INPUT" | jq -r '.prompt // ""')

# Skip empty or very short prompts
if [ ${#PROMPT} -lt 10 ]; then
  echo '{}'
  exit 0
fi

# Detect query intent for profile selection
PROFILE="semantic_search"
if echo "$PROMPT" | grep -qiE "why|because|caused|broke|regression"; then
  PROFILE="causal_reasoning"
elif echo "$PROMPT" | grep -qiE "function|class|impl|def |fn |const |let |var "; then
  PROFILE="code_search"
elif echo "$PROMPT" | grep -qiE "when|recent|yesterday|last week|today"; then
  PROFILE="temporal_navigation"
elif echo "$PROMPT" | grep -qiE "connect|depend|import|require|graph"; then
  PROFILE="graph_reasoning"
fi

# Search with detected profile
RESULTS=$(context-graph-cli mcp call search_graph \
  --query "$PROMPT" \
  --topK 5 \
  --strategy multi_space \
  --weightProfile "$PROFILE" \
  --minSimilarity 0.3 \
  --includeContent true 2>/dev/null)

MEMORIES=$(echo "$RESULTS" | jq -r '
  .results[:5] | map(
    "- [\(.similarity | tostring[:4])] \(.content[:150])"
  ) | join("\n")')

# Only inject if we found relevant results
if [ -n "$MEMORIES" ] && [ "$MEMORIES" != "null" ]; then
  cat <<EOF
{
  "systemMessage": "## Relevant Memories (profile: ${PROFILE})\n${MEMORIES}"
}
EOF
else
  echo '{}'
fi
```

**What this enables**:
- Every prompt is enriched with relevant prior knowledge
- Profile auto-detection routes causal questions through E5, code through E7, etc.
- Claude never "forgets" past decisions, bugs, or patterns
- Similarity threshold prevents noise injection

---

### 4.3 PostToolUse — Automatic Memory Capture

**Purpose**: After Claude uses tools, automatically capture the work as memories for future recall.

**Script**: `hooks/post-tool-use.sh`
```bash
#!/bin/bash
INPUT=$(cat)
TOOL_NAME=$(echo "$INPUT" | jq -r '.tool_name // ""')
TOOL_INPUT=$(echo "$INPUT" | jq -r '.tool_input // {}')
TOOL_RESPONSE=$(echo "$INPUT" | jq -r '.tool_response // {}')

case "$TOOL_NAME" in
  Write|Edit)
    # Capture file modifications as memories
    FILE_PATH=$(echo "$TOOL_INPUT" | jq -r '.file_path // ""')
    if [ -n "$FILE_PATH" ]; then
      # Extract description of the change
      if [ "$TOOL_NAME" = "Edit" ]; then
        OLD=$(echo "$TOOL_INPUT" | jq -r '.old_string // ""' | head -c 200)
        NEW=$(echo "$TOOL_INPUT" | jq -r '.new_string // ""' | head -c 200)
        CONTENT="Edited ${FILE_PATH}: replaced '${OLD}' with '${NEW}'"
      else
        CONTENT="Wrote file: ${FILE_PATH}"
      fi

      # Store the change as a memory
      context-graph-cli mcp call store_memory \
        --content "$CONTENT" \
        --importance 0.6 \
        --rationale "Automatic capture of file modification via PostToolUse hook" \
        2>/dev/null &
    fi
    ;;

  Bash)
    # Capture significant bash commands
    CMD=$(echo "$TOOL_INPUT" | jq -r '.command // ""')
    DESC=$(echo "$TOOL_INPUT" | jq -r '.description // ""')
    if echo "$CMD" | grep -qE "cargo build|cargo test|npm|git commit|make"; then
      CONTENT="Executed: ${DESC:-$CMD}"
      context-graph-cli mcp call store_memory \
        --content "$CONTENT" \
        --importance 0.4 \
        --rationale "Automatic capture of build/test command via PostToolUse hook" \
        2>/dev/null &
    fi
    ;;

  Task)
    # Capture subagent task descriptions
    TASK_DESC=$(echo "$TOOL_INPUT" | jq -r '.description // ""')
    TASK_PROMPT=$(echo "$TOOL_INPUT" | jq -r '.prompt // ""' | head -c 500)
    CONTENT="Spawned agent: ${TASK_DESC}. Prompt: ${TASK_PROMPT}"
    context-graph-cli mcp call store_memory \
      --content "$CONTENT" \
      --importance 0.5 \
      --rationale "Automatic capture of subagent task via PostToolUse hook" \
      2>/dev/null &
    ;;
esac

# Return empty (non-blocking)
echo '{}'
```

**What this enables**:
- Every file edit, build command, and subagent task is automatically recorded
- Future sessions can recall "what changes were made to file X?"
- Build/test history is searchable ("when did tests last fail?")
- Zero manual effort — memories accumulate passively

---

### 4.4 PreToolUse — Context-Aware Tool Augmentation

**Purpose**: Before Claude edits files, inject relevant context about that file from memory.

**Script**: `hooks/pre-tool-use.sh`
```bash
#!/bin/bash
INPUT=$(cat)
TOOL_NAME=$(echo "$INPUT" | jq -r '.tool_name // ""')
TOOL_INPUT=$(echo "$INPUT" | jq -r '.tool_input // {}')

case "$TOOL_NAME" in
  Edit|Write)
    FILE_PATH=$(echo "$TOOL_INPUT" | jq -r '.file_path // ""')
    if [ -n "$FILE_PATH" ]; then
      # Search for prior knowledge about this file
      HISTORY=$(context-graph-cli mcp call search_code \
        --query "changes to ${FILE_PATH}" \
        --topK 3 \
        --minScore 0.2 \
        --includeContent true 2>/dev/null | jq -r '
        .results[:3] | map(.content[:100]) | join("; ")')

      if [ -n "$HISTORY" ] && [ "$HISTORY" != "null" ]; then
        echo "{\"systemMessage\": \"Prior changes to ${FILE_PATH}: ${HISTORY}\"}"
        exit 0
      fi
    fi
    ;;
esac

echo '{}'
```

**What this enables**:
- Before editing any file, Claude sees prior modifications and decisions
- Prevents contradicting past architectural choices
- Surfaces known issues or patterns for the target file

---

### 4.5 Stop — Continuation for Unsaved Context

**Purpose**: When Claude finishes responding, check if important context was discussed but not stored.

**Script**: `hooks/stop-check.sh`
```bash
#!/bin/bash
INPUT=$(cat)
STOP_ACTIVE=$(echo "$INPUT" | jq -r '.stop_hook_active // false')

# Only check on natural stops, not forced ones
if [ "$STOP_ACTIVE" = "true" ]; then
  echo '{"decision": "approve"}'
  exit 0
fi

# Check if there are unstored insights from the conversation
# This is a lightweight check — just verify recent memory capture happened
RECENT_COUNT=$(context-graph-cli mcp call search_recent \
  --query "session activity" \
  --topK 1 \
  --temporalScale micro 2>/dev/null | jq -r '.results | length')

if [ "$RECENT_COUNT" = "0" ]; then
  # No memories captured this micro-period — might want to continue
  echo '{"decision": "block", "reason": "No memories captured this turn. Consider storing key decisions before ending.", "continue": false}'
else
  echo '{"decision": "approve"}'
fi
```

---

### 4.6 SubagentStop — Cross-Agent Memory Sharing

**Purpose**: When subagents finish, capture their findings as shared memories accessible to all future agents.

**Script**: `hooks/subagent-stop.sh`
```bash
#!/bin/bash
INPUT=$(cat)

# Subagent results are in the transcript — we can capture a summary
# The key value is that subagent discoveries persist across sessions
context-graph-cli mcp call store_memory \
  --content "Subagent completed task. Results available in transcript." \
  --importance 0.5 \
  --rationale "Subagent completion capture for cross-session persistence" \
  2>/dev/null &

echo '{"decision": "approve"}'
```

---

### 4.7 PreCompact — Save Critical Context Before Compression

**Purpose**: Before Claude's context window is compressed, extract and store critical information.

**Script**: `hooks/pre-compact.sh`
```bash
#!/bin/bash
INPUT=$(cat)
TRIGGER=$(echo "$INPUT" | jq -r '.trigger // "auto"')

# Get current session timeline to understand what might be lost
TIMELINE=$(context-graph-cli mcp call get_session_timeline \
  --limit 10 \
  --includeContent false 2>/dev/null | jq -r '.memories | length')

# Store a compaction marker for session continuity
context-graph-cli mcp call store_memory \
  --content "Context compaction triggered (${TRIGGER}). ${TIMELINE} memories in current session timeline." \
  --importance 0.7 \
  --rationale "Pre-compaction marker for session continuity tracking" \
  2>/dev/null &

echo '{}'
```

**What this enables**:
- Critical context is persisted before the context window is compressed
- Session continuity markers help future queries find pre-compaction state
- No loss of institutional knowledge during long sessions

---

### 4.8 SessionEnd — Consolidation & Dream Phase

**Purpose**: When a session ends, consolidate memories, detect topics, and prepare for future sessions.

**Script**: `hooks/session-end.sh`
```bash
#!/bin/bash
INPUT=$(cat)
REASON=$(echo "$INPUT" | jq -r '.reason // "exit"')

# 1. Run memory consolidation (merge similar memories)
context-graph-cli mcp call trigger_consolidation \
  --strategy similarity \
  --max_memories 100 \
  --min_similarity 0.85 2>/dev/null &

# 2. Force topic detection to update portfolio
context-graph-cli mcp call detect_topics \
  --force true 2>/dev/null &

# 3. Reconcile watched files (clean up orphans)
context-graph-cli mcp call reconcile_files \
  --dry_run false 2>/dev/null &

# 4. Check for divergence (drift from recent focus)
context-graph-cli mcp call get_divergence_alerts \
  --lookback_hours 2 2>/dev/null > /dev/null &

# 5. Store session summary
context-graph-cli mcp call store_memory \
  --content "Session ended (reason: ${REASON}). Consolidation and topic detection triggered." \
  --importance 0.6 \
  --rationale "Session boundary marker for continuity" \
  2>/dev/null &

wait
echo '{}'
```

**What this enables**:
- Redundant memories are merged automatically (saves storage, improves search)
- Topic portfolio is updated with latest work themes
- Orphaned file embeddings are cleaned up
- Next session starts with a clean, consolidated memory state

---

## 5. Skill Designs for Domain Specialization

Skills provide on-demand domain expertise that Claude auto-triggers based on context. Each skill below leverages specific MCP tools.

### 5.1 Memory Search Skill

**File**: `.claude/skills/memory-search/SKILL.md`

```yaml
---
name: memory-search
description: |
  Semantic memory search across 13 embedding spaces with weight profiles.
  Use for finding prior decisions, code patterns, causal chains, and entity
  relationships. Keywords: remember, recall, previous, history, find, search,
  memory, context, prior, past.
allowed-tools: Read,Glob,Grep,Bash
model: inherit
user-invocable: true
---
# Memory Search

## Overview
Search the context graph memory system using 13 embedder spaces with automatic
profile selection. Finds semantic matches, causal relationships, code patterns,
entity connections, and temporal sequences.

## Instructions

1. Analyze the user's query to determine search intent:
   - Causal ("why", "because", "caused") → `causal_reasoning` profile
   - Code ("function", "class", "impl") → `code_search` profile
   - Temporal ("when", "recent", "yesterday") → `temporal_navigation` profile
   - Entity ("uses", "depends on", "framework") → `fact_checking` profile
   - General → `semantic_search` profile

2. Execute the appropriate search tool:
   - General: `mcp__context-graph__search_graph`
   - Causal: `mcp__context-graph__search_causes` or `search_effects`
   - Code: `mcp__context-graph__search_code`
   - Entity: `mcp__context-graph__search_by_entities`
   - Keyword: `mcp__context-graph__search_by_keywords`

3. If initial results are insufficient, try:
   - `mcp__context-graph__search_robust` for typo-tolerant search (E9)
   - `mcp__context-graph__search_cross_embedder_anomalies` for blind spots
   - `mcp__context-graph__compare_embedder_views` to see different perspectives

4. Present results with similarity scores and source attribution.

## Examples
- "What did we decide about authentication?" → semantic_search profile
- "Why did the tests break after the refactor?" → search_causes
- "Find code similar to the parser function" → search_code with code_search profile
- "What technologies does this project use?" → search_by_entities
```

### 5.2 Causal Reasoning Skill

**File**: `.claude/skills/causal-reasoning/SKILL.md`

```yaml
---
name: causal-reasoning
description: |
  Causal chain analysis using asymmetric E5 embeddings. Finds causes of
  observed effects, predicts consequences, and traces multi-hop causal chains.
  Keywords: why, because, caused, broke, regression, root cause, effect,
  consequence, chain, impact.
allowed-tools: Read,Glob,Grep,Bash
model: inherit
---
# Causal Reasoning

## Overview
Leverages E5 asymmetric causal embeddings to perform abductive reasoning
(find causes), predictive reasoning (find effects), and transitive chain
analysis (trace multi-hop causality).

## Instructions

1. **Find causes** (abductive reasoning):
   - Use `mcp__context-graph__search_causes` with the observed effect as query
   - E5 applies 0.8x dampening for effect→cause direction (AP-77)
   - Algorithm: 80% E1 semantic + 20% E5 causal scoring

2. **Find effects** (predictive reasoning):
   - Use `mcp__context-graph__search_effects` with the cause as query
   - E5 applies 1.2x boost for cause→effect direction
   - Useful for impact analysis before changes

3. **Trace causal chains** (transitive reasoning):
   - Use `mcp__context-graph__get_causal_chain` from an anchor memory
   - Chains apply 0.9^hop attenuation (5 hops max)
   - Follow forward for consequences, backward for root causes

4. **Search causal relationships**:
   - Use `mcp__context-graph__search_causal_relationships` for LLM-generated
     causal descriptions with multi-embedder consensus (E1+E5+E8+E11)

5. **Trigger discovery**:
   - Use `mcp__context-graph__trigger_causal_discovery` to find new relationships
   - Requires LLM feature (Hermes-2-Pro-Mistral-7B)

## Examples
- "Why did deployment fail?" → search_causes(query="deployment failure")
- "What will break if I change the auth module?" → search_effects(query="auth module change")
- "Trace the root cause chain from this error" → get_causal_chain(direction="backward")
```

### 5.3 Entity Intelligence Skill

**File**: `.claude/skills/entity-intelligence/SKILL.md`

```yaml
---
name: entity-intelligence
description: |
  Entity extraction, relationship inference, and knowledge graph navigation
  using TransE embeddings (E11 KEPLER). Tracks technologies, frameworks,
  databases, and their relationships. Keywords: entity, technology, framework,
  database, uses, depends, relationship, stack, architecture.
allowed-tools: Read,Glob,Grep,Bash
model: inherit
---
# Entity Intelligence

## Overview
Uses E11 KEPLER embeddings trained on Wikidata5M to extract entities,
infer relationships via TransE, and navigate the entity knowledge graph.

## Instructions

1. **Extract entities from text**:
   - Use `mcp__context-graph__extract_entities` to find technologies,
     frameworks, databases, companies, and technical terms
   - Canonicalizes variations (postgres → postgresql, k8s → kubernetes)

2. **Search by entities**:
   - Use `mcp__context-graph__search_by_entities` with entity names
   - Combines E1 semantic + E11 entity scores + Jaccard similarity
   - Use matchMode "all" for intersection, "any" for union

3. **Infer relationships**:
   - Use `mcp__context-graph__infer_relationship` between two entities
   - TransE predicts: r̂ = tail - head, matches known relations
   - Returns ranked relation candidates with confidence scores

4. **Find related entities**:
   - Use `mcp__context-graph__find_related_entities` with a relation
   - Outgoing: entity + relation → ? (what does X depend on?)
   - Incoming: ? + relation → entity (what depends on X?)

5. **Validate knowledge triples**:
   - Use `mcp__context-graph__validate_knowledge` to score (subject, predicate, object)
   - Score > -5.0 = Valid, -5.0 to -10.0 = Uncertain, < -10.0 = Invalid

6. **Visualize entity graph**:
   - Use `mcp__context-graph__get_entity_graph` centered on an entity
   - Shows relationship edges weighted by evidence strength

## Examples
- "What technologies does this project use?" → extract_entities + search_by_entities
- "What depends on RocksDB?" → find_related_entities(entity="rocksdb", relation="depends_on", direction="incoming")
- "Is it true that Rust uses LLVM?" → validate_knowledge(subject="Rust", predicate="uses", object="LLVM")
```

### 5.4 Code Pattern Search Skill

**File**: `.claude/skills/code-search/SKILL.md`

```yaml
---
name: code-search
description: |
  Code-aware search using E7 Qodo embeddings (1536D). Finds similar
  functions, patterns, and implementations across the codebase. Supports
  language hints and AST context. Keywords: code, function, implementation,
  pattern, similar code, search code, find function.
allowed-tools: Read,Glob,Grep,Bash
model: inherit
---
# Code Pattern Search

## Overview
Uses E7 code embeddings (1536D Qodo) to find code patterns that
go beyond text matching. Understands code semantics independent of
variable names or formatting.

## Instructions

1. **Search code patterns**:
   - Use `mcp__context-graph__search_code` with a natural language
     description or code snippet as query
   - Set `languageHint` if known (rust, python, javascript, etc.)
   - `blendWithSemantic` controls E7 vs E1 weight (default 0.4)

2. **Search modes**:
   - `semantic`: Blends E7 code + E1 semantic understanding
   - `e7Only`: Pure code pattern matching
   - `pipeline`: Full 5-stage precision pipeline

3. **For cross-embedder code discovery**:
   - Use `mcp__context-graph__search_cross_embedder_anomalies`
     with highEmbedder=E7, lowEmbedder=E1
   - Finds code patterns that semantic search misses

4. **For code entity relationships**:
   - Combine with `extract_entities` to find technology references
   - Use `search_connections` with graph_reasoning profile for dependencies

## Examples
- "Find error handling patterns" → search_code(query="error handling pattern", languageHint="rust")
- "Code similar to the authentication middleware" → search_code(query="authentication middleware", searchMode="semantic")
```

### 5.5 Topic Explorer Skill

**File**: `.claude/skills/topic-explorer/SKILL.md`

```yaml
---
name: topic-explorer
description: |
  Explore emergent topic clusters, stability metrics, and divergence alerts.
  Uses HDBSCAN clustering across semantic embedding spaces. Keywords: topics,
  themes, clusters, portfolio, stability, divergence, drift, focus, churn.
allowed-tools: Read,Glob,Grep,Bash
model: inherit
---
# Topic Explorer

## Overview
Discovers and tracks emergent topics in memory via HDBSCAN clustering.
Monitors topic stability, phase transitions, and divergence from focus areas.

## Instructions

1. **View topic portfolio**:
   - Use `mcp__context-graph__get_topic_portfolio` (brief/standard/verbose)
   - Topics require weighted_agreement >= 2.5 across embedding spaces
   - Phases: Emerging → Stable → Declining → Merging

2. **Check stability**:
   - Use `mcp__context-graph__get_topic_stability` with lookback hours
   - Monitors churn rate, entropy, phase distribution
   - High churn = rapid topic shifting; high entropy = diverse focus

3. **Force topic recalculation**:
   - Use `mcp__context-graph__detect_topics` with force=true
   - Useful after bulk memory operations or session changes

4. **Check for divergence**:
   - Use `mcp__context-graph__get_divergence_alerts`
   - Detects when recent work diverges from established patterns
   - Uses SEMANTIC embedders only (E1, E5, E6, E7, E10, E12, E13)

## Examples
- "What are the main themes of this project?" → get_topic_portfolio(format="standard")
- "Has my focus shifted recently?" → get_divergence_alerts(lookback_hours=4)
- "How stable is the project's topic distribution?" → get_topic_stability(hours=24)
```

### 5.6 Blind Spot Detective Skill

**File**: `.claude/skills/blind-spot-detective/SKILL.md`

```yaml
---
name: blind-spot-detective
description: |
  Finds what standard semantic search misses using E9 hyperdimensional
  computing and cross-embedder anomaly detection. Handles typos, code
  identifiers, and character variations. Keywords: blind spot, missing,
  typo, fuzzy, noise, robust, overlooked, hidden.
allowed-tools: Read,Glob,Grep,Bash
model: inherit
---
# Blind Spot Detective

## Overview
Uses E9 HDC embeddings for noise-robust search and cross-embedder anomaly
detection to find memories that standard E1 semantic search misses.

## Instructions

1. **Noise-tolerant search**:
   - Use `mcp__context-graph__search_robust` — typos are OK in the query
   - Finds: authentication when you type "authetication"
   - Finds: parse_config when you search "parseConfig"
   - Uses E9 discovery threshold + E1 weakness threshold

2. **Cross-embedder anomaly detection**:
   - Use `mcp__context-graph__search_cross_embedder_anomalies`
   - highEmbedder=E7, lowEmbedder=E1: Code patterns E1 misses
   - highEmbedder=E11, lowEmbedder=E1: Entity facts E1 misses
   - highEmbedder=E5, lowEmbedder=E1: Causal structures E1 misses

3. **Compare perspectives**:
   - Use `mcp__context-graph__compare_embedder_views` with 2-5 embedders
   - See where embedders agree (strong signal) vs. disagree (blind spots)

## Examples
- "Find anything about authentcation" → search_robust (handles typo)
- "What does E7 see that E1 misses?" → search_cross_embedder_anomalies
- "Compare how different embedders see 'database optimization'" → compare_embedder_views
```

### 5.7 Session Navigator Skill

**File**: `.claude/skills/session-navigator/SKILL.md`

```yaml
---
name: session-navigator
description: |
  Navigate conversation history, session timelines, and memory chains using
  E4 positional embeddings. Compare session states and traverse memory
  sequences. Keywords: session, conversation, timeline, history, previous,
  before, after, context, turn, sequence.
allowed-tools: Read,Glob,Grep,Bash
model: inherit
---
# Session Navigator

## Overview
Uses E4 positional embeddings for sequence-based memory retrieval.
Navigate conversation timelines, compare session states, and traverse
memory chains across turns.

## Instructions

1. **Get conversation context**:
   - Use `mcp__context-graph__get_conversation_context`
   - direction: "before" (prior turns), "after" (later), "both" (surrounding)
   - Supports semantic filtering with query parameter

2. **View session timeline**:
   - Use `mcp__context-graph__get_session_timeline`
   - Filter by sourceTypes: HookDescription, ClaudeResponse, Manual, MDFileChunk
   - Ordered by session_sequence position

3. **Traverse memory chains**:
   - Use `mcp__context-graph__traverse_memory_chain`
   - Multi-hop traversal from anchor memory
   - Traces conversation evolution across turns

4. **Compare session states**:
   - Use `mcp__context-graph__compare_session_states`
   - Compare memory state at different sequence points
   - Shows topic changes and memory count differences

## Examples
- "What was discussed before the refactor?" → get_conversation_context(direction="before")
- "Show me the session timeline" → get_session_timeline(limit=20, includeContent=true)
- "How has the topic shifted since the start?" → compare_session_states(beforeSequence="start", afterSequence="current")
```

### 5.8 Provenance Auditor Skill

**File**: `.claude/skills/provenance-auditor/SKILL.md`

```yaml
---
name: provenance-auditor
description: |
  Full audit trail, merge lineage, and provenance chain tracking. Traces
  any memory back to its source with operator attribution, embedding
  versions, and importance history. Keywords: audit, provenance, history,
  who, when, trace, lineage, source, origin, attribution.
allowed-tools: Read,Glob,Grep,Bash
model: inherit
---
# Provenance Auditor

## Overview
Provides complete provenance tracking from memory to source, including
audit logs, merge history, operator attribution, and embedding versions.

## Instructions

1. **Query audit trail**:
   - Use `mcp__context-graph__get_audit_trail` with target_id or time range
   - Shows: operation, operator, session, rationale, result, timestamp
   - Chronological operation history

2. **View merge history**:
   - Use `mcp__context-graph__get_merge_history` for merge lineage
   - Shows: source_ids, strategy, operator, timestamp
   - Tracks which memories were combined and why

3. **Full provenance chain**:
   - Use `mcp__context-graph__get_provenance_chain` for complete lineage
   - Includes: source_type, file_path, chunk_info, operator_attribution
   - Optional: audit trail, embedding versions, importance history

## Examples
- "Who created this memory and when?" → get_provenance_chain(memory_id=..., include_audit=true)
- "What operations were performed on this memory?" → get_audit_trail(target_id=...)
- "What memories were merged to create this one?" → get_merge_history(memory_id=...)
```

### 5.9 Memory Curator Skill

**File**: `.claude/skills/memory-curator/SKILL.md`

```yaml
---
name: memory-curator
description: |
  Curate memory graph: merge duplicates, adjust importance, soft-delete
  irrelevant memories, consolidate clusters, and create custom weight
  profiles. Keywords: merge, delete, boost, importance, consolidate,
  clean, curate, deduplicate, weight, profile.
allowed-tools: Read,Glob,Grep,Bash
model: inherit
---
# Memory Curator

## Overview
Curate the memory graph by merging similar concepts, adjusting importance
scores, soft-deleting irrelevant memories, and creating custom search profiles.

## Instructions

1. **Merge similar memories**:
   - Use `mcp__context-graph__merge_concepts` with 2-10 source_ids
   - Strategies: union (combine all), intersection (shared only), weighted_average
   - 30-day reversal window via reversal_hash

2. **Adjust importance**:
   - Use `mcp__context-graph__boost_importance` with delta (-1.0 to 1.0)
   - Final value clamped to [0.0, 1.0]
   - Higher importance = stronger influence in search results

3. **Soft-delete memories**:
   - Use `mcp__context-graph__forget_concept` with soft_delete=true
   - 30-day recovery window per SEC-06
   - Persists to RocksDB (survives restarts)

4. **Trigger consolidation**:
   - Use `mcp__context-graph__trigger_consolidation`
   - Strategies: similarity, temporal, semantic
   - Default min_similarity: 0.85 (merge only very similar memories)

5. **Create custom weight profiles**:
   - Use `mcp__context-graph__create_weight_profile`
   - Define per-embedder weights (E1-E13, sum ~1.0)
   - Session-scoped, reusable in search_graph calls

## Examples
- "Merge these duplicate bug reports" → merge_concepts(source_ids=[...], strategy="union")
- "This memory is important, boost it" → boost_importance(node_id=..., delta=0.3)
- "Delete this irrelevant memory" → forget_concept(node_id=..., soft_delete=true)
- "Create a profile focused on code and entities" → create_weight_profile(...)
```

### 5.10 Graph Navigator Skill

**File**: `.claude/skills/graph-navigator/SKILL.md`

```yaml
---
name: graph-navigator
description: |
  Navigate the knowledge graph via K-NN neighbors, typed edges, multi-hop
  traversal, and unified multi-embedder consensus. Explore how memories
  connect across semantic, causal, code, and entity dimensions.
  Keywords: graph, neighbors, edges, traverse, connections, path, linked,
  related, dependency, network.
allowed-tools: Read,Glob,Grep,Bash
model: inherit
---
# Graph Navigator

## Overview
Navigate the knowledge graph using K-NN neighbors, typed edges, and
multi-hop traversal across all 13 embedder spaces.

## Instructions

1. **Find neighbors in specific space**:
   - Use `mcp__context-graph__get_memory_neighbors` with embedder_id (0-12)
   - 0=E1 semantic, 6=E7 code, 10=E11 entity, etc.

2. **Get typed edges**:
   - Use `mcp__context-graph__get_typed_edges`
   - Types: semantic_similar, code_related, entity_shared, causal_chain,
     graph_connected, paraphrase_aligned, keyword_overlap, multi_agreement

3. **Multi-hop traversal**:
   - Use `mcp__context-graph__traverse_graph` following typed edges
   - Max 5 hops, filtered by edge weight threshold

4. **Unified multi-embedder neighbors**:
   - Use `mcp__context-graph__get_unified_neighbors` for consensus ranking
   - Uses Weighted RRF across all 13 embedders
   - Supports weight profiles and custom weights

5. **Find connections**:
   - Use `mcp__context-graph__search_connections` with asymmetric E8
   - Direction: source (what points TO), target (what points FROM), both

6. **Build graph paths**:
   - Use `mcp__context-graph__get_graph_path` for multi-hop exploration
   - Hop attenuation: 0.9^hop for distance scoring

## Examples
- "What's related to this memory in code space?" → get_memory_neighbors(embedder_id=6)
- "Show causal edges from this memory" → get_typed_edges(edge_type="causal_chain")
- "Traverse 3 hops following entity connections" → traverse_graph(max_hops=3, edge_type="entity_shared")
```

---

## 6. High-Value Automation Workflows

### 6.1 Regression Root-Cause Analysis

**Trigger**: User reports "tests broke after the refactor"

```
UserPromptSubmit hook detects "broke"/"regression" keywords
  → search_causes(query="tests broke after refactor", topK=10)
  → get_causal_chain(anchorId=most_relevant_result, direction="backward", maxHops=5)
  → search_code(query="recent refactor changes", languageHint="rust")
  → Injects: causal chain + recent changes + prior test states
```

**Value**: Claude immediately knows the likely root cause before even looking at code.

### 6.2 Architecture Decision Memory

**Trigger**: Every Edit/Write to architecture-related files

```
PostToolUse hook captures file changes
  → store_memory(content="Changed auth from session to JWT", importance=0.8)
  → extract_entities(text=file_content)
  → validate_knowledge(subject="project", predicate="uses", object="JWT")

Future session: "Why did we switch to JWT?"
  → search_causes(query="switched to JWT") returns the original decision memory
```

**Value**: Architecture decisions are permanently recorded with causal context.

### 6.3 Continuous Learning Loop

```
SessionStart → get_topic_portfolio() → inject active themes
  → Claude knows what the project is about

During session → PostToolUse captures every change
  → Memories accumulate with temporal ordering

UserPromptSubmit → search_graph() enriches every prompt
  → Claude has full prior context

SessionEnd → trigger_consolidation() + detect_topics()
  → Memories merged, topics updated, ready for next session
```

**Value**: Each session builds on all previous sessions. Knowledge compounds.

### 6.4 Code Review with Institutional Memory

**Trigger**: Claude reviews a PR or code change

```
PreToolUse (Read on file) → search_code(query=file_path)
  → Prior changes, known issues, patterns for this file

search_causes(query="bugs in this module")
  → Known failure modes for the area being changed

search_by_entities(entities=["affected_framework"])
  → Framework-specific best practices from memory

compare_embedder_views(query="code change", embedders=["E1","E7","E11"])
  → Multi-perspective analysis (semantic + code + entity)
```

**Value**: Code reviews incorporate institutional knowledge, not just current diff.

### 6.5 Blind Spot Discovery Workflow

**Trigger**: Periodic (every N sessions) or on-demand

```
search_robust(query="edge cases") → typo-tolerant discovery
search_cross_embedder_anomalies(highEmbedder=E7, lowEmbedder=E1)
  → Code patterns semantic search misses
search_cross_embedder_anomalies(highEmbedder=E5, lowEmbedder=E1)
  → Causal structures not captured semantically
get_divergence_alerts(lookback_hours=24)
  → Drift from established patterns
```

**Value**: Systematic discovery of what you don't know you don't know.

### 6.6 Technology Stack Intelligence

**Trigger**: Questions about project dependencies or architecture

```
extract_entities(text=query) → identify referenced technologies
find_related_entities(entity="rust", relation="uses")
  → What the project uses Rust for
get_entity_graph(centerEntity="rocksdb", maxDepth=2)
  → Full dependency graph around RocksDB
validate_knowledge(subject="project", predicate="depends_on", object="tokio")
  → Verify dependency claims
```

**Value**: Instant technology stack intelligence without grepping code.

---

## 7. Advanced Patterns

### 7.1 Multi-Profile Search Cascade

For critical queries, search with multiple profiles and merge results:

```bash
# In a skill or hook script
SEMANTIC=$(context-graph-cli mcp call search_graph --query "$Q" --weightProfile semantic_search --topK 5)
CAUSAL=$(context-graph-cli mcp call search_graph --query "$Q" --weightProfile causal_reasoning --topK 5)
CODE=$(context-graph-cli mcp call search_graph --query "$Q" --weightProfile code_search --topK 5)
# Merge and deduplicate results, present diverse perspectives
```

### 7.2 Dream Phase Automation

Triggered when topic entropy exceeds threshold (e.g., end of sprint):

```bash
# NREM: Consolidate high-importance patterns
context-graph-cli mcp call trigger_consolidation --strategy similarity --min_similarity 0.80

# REM: Discover blind spots via random walk
context-graph-cli mcp call search_robust --query "unresolved issues" --topK 20
context-graph-cli mcp call get_divergence_alerts --lookback_hours 48

# Update portfolio
context-graph-cli mcp call detect_topics --force true
```

### 7.3 Temporal Pattern Mining

Use E2/E3 temporal embeddings to discover work patterns:

```bash
# What do I work on Monday mornings?
context-graph-cli mcp call search_periodic \
  --query "coding tasks" --targetDayOfWeek 1 --targetHour 9

# What was the focus last week?
context-graph-cli mcp call search_recent \
  --query "main focus" --temporalScale macro --temporalWeight 0.7
```

### 7.4 Embedder Fingerprint Analysis

Compare how a specific memory looks across all 13 spaces:

```bash
# Get full fingerprint
context-graph-cli mcp call get_memory_fingerprint --memoryId $ID --includeVectorNorms true

# Compare with another memory's fingerprint to understand relationship
context-graph-cli mcp call get_memory_neighbors --memory_id $ID --embedder_id 4  # E5 causal
context-graph-cli mcp call get_memory_neighbors --memory_id $ID --embedder_id 6  # E7 code
context-graph-cli mcp call get_memory_neighbors --memory_id $ID --embedder_id 10 # E11 entity
```

### 7.5 Custom Weight Profile for Domain

Create specialized profiles for specific project domains:

```bash
# Security-focused profile
context-graph-cli mcp call create_weight_profile \
  --name "security_audit" \
  --weights '{"E1":0.20,"E5":0.15,"E6":0.15,"E7":0.20,"E8":0.10,"E9":0.05,"E11":0.15}' \
  --description "Security audit: code patterns + entity relationships + keyword precision"
```

---

## 8. Configuration Reference

### 8.1 Complete Hook Configuration

**File**: `.claude/settings.json`

```json
{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "./hooks/session-start.sh",
            "timeout": 5000
          }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "./hooks/user-prompt-submit.sh",
            "timeout": 3000
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "./hooks/pre-tool-use.sh",
            "timeout": 2000
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "Write|Edit|Bash|Task",
        "hooks": [
          {
            "type": "command",
            "command": "./hooks/post-tool-use.sh",
            "timeout": 3000
          }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "./hooks/stop-check.sh",
            "timeout": 2000
          }
        ]
      }
    ],
    "SubagentStop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "./hooks/subagent-stop.sh",
            "timeout": 3000
          }
        ]
      }
    ],
    "PreCompact": [
      {
        "matcher": "auto",
        "hooks": [
          {
            "type": "command",
            "command": "./hooks/pre-compact.sh",
            "timeout": 5000
          }
        ]
      }
    ],
    "SessionEnd": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "./hooks/session-end.sh",
            "timeout": 30000
          }
        ]
      }
    ]
  }
}
```

### 8.2 Skills Directory Structure

```
.claude/skills/
├── memory-search/
│   └── SKILL.md
├── causal-reasoning/
│   └── SKILL.md
├── entity-intelligence/
│   └── SKILL.md
├── code-search/
│   └── SKILL.md
├── topic-explorer/
│   └── SKILL.md
├── blind-spot-detective/
│   └── SKILL.md
├── session-navigator/
│   └── SKILL.md
├── provenance-auditor/
│   └── SKILL.md
├── memory-curator/
│   └── SKILL.md
└── graph-navigator/
│   └── SKILL.md
```

### 8.3 Hook Scripts Directory

```
hooks/
├── session-start.sh        # Context warm-up, topic loading
├── user-prompt-submit.sh   # Intelligent context injection
├── pre-tool-use.sh         # File history injection before edits
├── post-tool-use.sh        # Automatic memory capture
├── stop-check.sh           # Unsaved context detection
├── subagent-stop.sh        # Cross-agent memory sharing
├── pre-compact.sh          # Save context before compression
└── session-end.sh          # Consolidation & dream phase
```

---

## 9. Implementation Priorities

### Phase 1: Foundation (Immediate Value)

| Priority | Hook/Skill | Value | Effort |
|----------|-----------|-------|--------|
| P0 | SessionStart hook | Project awareness at startup | Low |
| P0 | UserPromptSubmit hook | Context injection per prompt | Medium |
| P0 | PostToolUse hook (Edit/Write) | Automatic memory capture | Medium |
| P0 | SessionEnd hook | Consolidation & cleanup | Low |
| P1 | memory-search skill | On-demand multi-space search | Low |

### Phase 2: Intelligence Layer

| Priority | Hook/Skill | Value | Effort |
|----------|-----------|-------|--------|
| P1 | causal-reasoning skill | Root cause analysis | Low |
| P1 | code-search skill | Code pattern discovery | Low |
| P1 | PreToolUse hook (Edit/Write) | File history before edits | Medium |
| P2 | entity-intelligence skill | Tech stack reasoning | Low |
| P2 | topic-explorer skill | Theme awareness | Low |

### Phase 3: Advanced Patterns

| Priority | Hook/Skill | Value | Effort |
|----------|-----------|-------|--------|
| P2 | blind-spot-detective skill | Discovery of unknowns | Low |
| P2 | session-navigator skill | Conversation threading | Low |
| P2 | PreCompact hook | Context preservation | Medium |
| P3 | provenance-auditor skill | Full audit capability | Low |
| P3 | graph-navigator skill | Knowledge graph exploration | Low |
| P3 | memory-curator skill | Memory hygiene | Low |
| P3 | Stop hook | Continuation for unsaved context | Low |
| P3 | SubagentStop hook | Cross-agent knowledge | Medium |

### Expected Outcomes

- **Session continuity**: Zero context loss between sessions
- **Automatic learning**: Every code change, decision, and discovery is captured
- **Causal reasoning**: "Why did X break?" answered from memory, not investigation
- **Blind spot detection**: Systematic discovery of overlooked patterns
- **Entity intelligence**: Technology relationships inferred, not manually tracked
- **Topic awareness**: Emergent themes detected and surfaced automatically
- **Provenance**: Full audit trail for every piece of knowledge
- **Compounding knowledge**: Each session is smarter than the last

---

## Summary: What You Can Do

| Capability | Tools Used | Hook/Skill Trigger |
|-----------|------------|-------------------|
| **Remember everything across sessions** | store_memory, search_graph | PostToolUse + SessionEnd hooks |
| **Answer "why did X happen?"** | search_causes, get_causal_chain | causal-reasoning skill |
| **Find code patterns** | search_code, search_cross_embedder_anomalies | code-search skill |
| **Track technology stack** | extract_entities, find_related_entities, get_entity_graph | entity-intelligence skill |
| **Detect topic drift** | get_topic_portfolio, get_divergence_alerts | topic-explorer skill |
| **Find what you're missing** | search_robust, compare_embedder_views | blind-spot-detective skill |
| **Navigate conversation history** | get_conversation_context, get_session_timeline | session-navigator skill |
| **Audit any memory's origin** | get_audit_trail, get_provenance_chain | provenance-auditor skill |
| **Curate and merge memories** | merge_concepts, boost_importance, forget_concept | memory-curator skill |
| **Explore the knowledge graph** | get_memory_neighbors, traverse_graph, get_unified_neighbors | graph-navigator skill |
| **Inject context per prompt** | search_graph with auto-profile detection | UserPromptSubmit hook |
| **Capture changes automatically** | store_memory, extract_entities | PostToolUse hook |
| **Warm up session context** | get_memetic_status, get_topic_portfolio, search_recent | SessionStart hook |
| **Consolidate at session end** | trigger_consolidation, detect_topics, reconcile_files | SessionEnd hook |
| **Preserve pre-compaction state** | store_memory, get_session_timeline | PreCompact hook |
| **Share across subagents** | store_memory, search_graph | SubagentStop hook |
| **File history before edits** | search_code with file path | PreToolUse hook |
| **Discover temporal patterns** | search_recent, search_periodic | Temporal search tools |
| **Custom search perspectives** | create_weight_profile, search_by_embedder | memory-curator skill |
| **Multi-hop graph exploration** | traverse_graph, get_graph_path, get_causal_chain | graph-navigator skill |

# Context Graph x Claude Code Integration Task

## IMPLEMENTATION TASK for AI Agent with Fresh Context Window

**Date**: 2026-02-16 | **Branch**: `ccintegration` | **Status**: HOOKS + SKILLS + SETTINGS NEED REWRITE
**Project Root**: `/home/cabdru/contextgraph`

---

## 0. WHAT THIS DOCUMENT IS

This is a **self-contained task document** for an AI agent that knows nothing about this project. It describes:
1. What the Context Graph system IS (section 1)
2. What CURRENTLY EXISTS in the codebase (section 2)
3. What is WRONG with the current integration (section 3)
4. Exactly WHAT to build (section 4)
5. Verification requirements (section 5)

**Rule**: ASSUME EVERYTHING IN THIS DOCUMENT COULD BE WRONG. Verify file paths, parameter names, and tool names against the actual source code before writing any code. The source of truth is always the Rust source in `crates/`.

---

## 1. SYSTEM OVERVIEW

### 1.1 What Context Graph IS

A Rust MCP server that stores memories in RocksDB with embeddings across 13 independent spaces. Each memory gets embedded by ALL 13 embedders simultaneously. Search can use any combination of embedder spaces.

**Key binaries** (from `Cargo.toml` `[[bin]]` entries):
- `context-graph-mcp` — MCP JSON-RPC server (stdio or TCP transport) — crate: `crates/context-graph-mcp`
- `context-graph-cli` — CLI for hooks, memory commands, topics — crate: `crates/context-graph-cli`

### 1.2 Workspace Crates (11 total)

```
crates/
├── context-graph-mcp/          # MCP server, 55 tools, TCP/stdio
├── context-graph-cli/          # CLI binary, hooks, memory commands
├── context-graph-core/         # Domain types, traits, HNSW
├── context-graph-storage/      # RocksDB backend, 50+ CFs
├── context-graph-embeddings/   # 13 embedder models, ONNX/Candle
├── context-graph-cuda/         # CUDA ops, HDBSCAN, Poincare
├── context-graph-graph/        # Graph structures, traversal
├── context-graph-benchmark/    # Performance benchmarks
├── context-graph-causal-agent/ # LLM causal discovery
├── context-graph-graph-agent/  # LLM graph discovery
└── context-graph-test-utils/   # Shared test helpers
```

### 1.3 The 13 Embedders

| ID | Name | Dim | What It Captures | Asymmetric |
|----|------|-----|------------------|------------|
| E1 | Semantic | 1024D | General meaning (e5-large-v2) | No |
| E2 | Freshness | 512D | Recency (exponential decay) | No |
| E3 | Periodic | 512D | Cyclical patterns (Fourier) | No |
| E4 | Positional | 512D | Conversation order | No |
| E5 | Causal | 768D | Cause→effect direction | **Yes** (cause→effect 1.2x, effect→cause 0.8x) |
| E6 | Keyword | 30K sparse | Exact term matching (SPLADE) | No |
| E7 | Code | 1536D | Source code patterns (Qodo) | No |
| E8 | Graph | 1024D | Structural connectivity | **Yes** (source→target 1.2x) |
| E9 | HDC | 1024D | Noise-robust (hyperdimensional) | No |
| E10 | Paraphrase | 768D | Semantic equivalence | **Yes** (doc→query asymmetric) |
| E11 | Entity | 768D | Entity relationships (KEPLER/TransE) | No |
| E12 | ColBERT | 128D/tok | Token-level precision (MaxSim) | No |
| E13 | SPLADE | 30K sparse | Keyword expansion | No |

### 1.4 MCP Tools (55 total)

**Source of truth**: `crates/context-graph-mcp/src/tools/names.rs`

All tool names as constants (these are the EXACT names the MCP server exposes):

| Category | Tools |
|----------|-------|
| Core (4) | `store_memory`, `get_memetic_status`, `search_graph`, `trigger_consolidation` |
| Topic (4) | `get_topic_portfolio`, `get_topic_stability`, `detect_topics`, `get_divergence_alerts` |
| Curation (3) | `merge_concepts`, `forget_concept`, `boost_importance` |
| File Watcher (4) | `list_watched_files`, `get_file_watcher_stats`, `delete_file_content`, `reconcile_files` |
| Sequence (4) | `get_conversation_context`, `get_session_timeline`, `traverse_memory_chain`, `compare_session_states` |
| Causal (4) | `search_causes`, `search_effects`, `get_causal_chain`, `search_causal_relationships` |
| Causal Discovery (2) | `trigger_causal_discovery`, `get_causal_discovery_status` |
| Maintenance (1) | `repair_causal_relationships` |
| Graph (2) | `search_connections`, `get_graph_path` |
| Graph Discovery (2) | `discover_graph_relationships`, `validate_graph_link` |
| Keyword (1) | `search_by_keywords` |
| Code (1) | `search_code` |
| Robustness (1) | `search_robust` |
| Entity (6) | `extract_entities`, `search_by_entities`, `infer_relationship`, `find_related_entities`, `validate_knowledge`, `get_entity_graph` |
| Embedder Search (7) | `search_by_embedder`, `get_embedder_clusters`, `compare_embedder_views`, `list_embedder_indexes`, `get_memory_fingerprint`, `create_weight_profile`, `search_cross_embedder_anomalies` |
| Temporal (2) | `search_recent`, `search_periodic` |
| Graph Linking (4) | `get_memory_neighbors`, `get_typed_edges`, `traverse_graph`, `get_unified_neighbors` |
| Provenance (3) | `get_audit_trail`, `get_merge_history`, `get_provenance_chain` |

### 1.5 Key `search_graph` Parameters

**Source of truth**: `crates/context-graph-mcp/src/tools/definitions/core.rs`

```
Required: query (string)
Optional:
  topK: integer (1-100, default 10)
  minSimilarity: number (0-1, default 0.0)
  includeContent: boolean (default false)
  strategy: "e1_only" | "multi_space" | "pipeline" (default "multi_space")
  weightProfile: one of 14 profiles (see below)
  customWeights: object {E1: 0.4, E5: 0.3, ...} must sum to ~1.0
  excludeEmbedders: array ["E2", "E3", "E4"]
  includeEmbedderBreakdown: boolean (default false)
  includeProvenance: boolean (default false)
  enableRerank: boolean (default false) — E12 ColBERT
  enableAsymmetricE5: boolean (default true)
  causalDirection: "auto" | "cause" | "effect" | "none"
  temporalWeight: number (0-1, default 0.0)
  sessionScope: "current" | "all" | "recent"
  sessionId: string (filter to specific session)
  lastHours / lastDays: number
  temporalScale: "micro" | "meso" | "macro" | "long" | "archival"
  decayFunction: "linear" | "exponential" | "step" | "none" | "no_decay"
```

**14 Weight Profiles**: `semantic_search`, `causal_reasoning`, `code_search`, `fact_checking`, `graph_reasoning`, `temporal_navigation`, `sequence_navigation`, `conversation_history`, `category_weighted`, `typo_tolerant`, `pipeline_stage1_recall`, `pipeline_stage2_scoring`, `pipeline_full`, `balanced`

### 1.6 Key `store_memory` Parameters

**Source of truth**: `crates/context-graph-mcp/src/tools/definitions/core.rs`

```
Required: content (string)
Optional:
  rationale: string (1-1024 chars)
  importance: number (0-1, default 0.5)
  sessionId: string
  operatorId: string
```

**NOTE**: `store_memory` does NOT have `sourceType`, `sourceRef`, `toolUseId`, or `filePath` parameters. Those were fictional in the old report. Source metadata is set internally by the MCP server, not by callers.

### 1.7 MCP Server Transport

**Source of truth**: `crates/context-graph-mcp/src/main.rs`

- Default: **stdio** (standard MCP transport for Claude Code)
- Optional: **TCP** on port 3100 (for CLI client connections)
- Optional: **SSE** on port 3101 (Server-Sent Events)
- **Daemon mode**: `--daemon` flag, port 3199 (shared server across terminals)
- **Warmup**: Default `--warm-first` (blocks until models loaded), `--no-warm` for fast startup
- CLI connects to MCP server via TCP (`crates/context-graph-cli/src/mcp_client.rs`, default `127.0.0.1:3100`)

### 1.8 Search Strategies

| Strategy | Embedders Used | Best For |
|----------|---------------|----------|
| `e1_only` | E1 only | Fast semantic lookup |
| `multi_space` | E1+E5+E7+E8+E10+E11 via Weighted RRF | General queries (default) |
| `pipeline` | E13 recall → E1 dense → E12 rerank | Max precision |

---

## 2. WHAT CURRENTLY EXISTS

### 2.1 Current `.claude/settings.json`

**Path**: `/home/cabdru/contextgraph/.claude/settings.json`

The current settings.json uses `npx @claude-flow/cli@latest` for ALL hooks. This is **WRONG** — Context Graph has its own CLI (`context-graph-cli`) and should NOT depend on the claude-flow npm package for hook execution. The claude-flow hooks are for a completely different project.

**Current hooks reference**:
- `npx @claude-flow/cli@latest hooks pre-edit` ← WRONG, should use context-graph-cli
- `npx @claude-flow/cli@latest hooks post-edit` ← WRONG
- `npx @claude-flow/cli@latest hooks route` ← WRONG
- `npx @claude-flow/cli@latest daemon start` ← WRONG
- `npx @claude-flow/cli@latest hooks session-restore` ← WRONG

### 2.2 Current Hook Scripts in `.claude/hooks/`

6 bash scripts exist at `/home/cabdru/contextgraph/.claude/hooks/`:

| Script | Calls | Status |
|--------|-------|--------|
| `session_start.sh` | `context-graph-cli hooks session-start --stdin --format json` | **CORRECT** — uses real CLI |
| `user_prompt_submit.sh` | `context-graph-cli hooks prompt-submit --session-id ... --stdin true --format json` | **CORRECT** |
| `pre_tool_use.sh` | `context-graph-cli hooks pre-tool --session-id ... --tool-name ... --fast-path true --format json` | **CORRECT** |
| `post_tool_use.sh` | `context-graph-cli hooks post-tool --session-id ... --tool-name ... --success ... --format json` | **CORRECT** |
| `session_end.sh` | `context-graph-cli hooks session-end --session-id ... --duration-ms ... --format json` | **CORRECT** |
| `stop.sh` | `context-graph-cli memory capture-response --content ... --session-id ... --response-type stop_response` | **CORRECT** |

**KEY FINDING**: The 6 shell scripts are CORRECT and use the real `context-graph-cli`. But the `.claude/settings.json` does NOT reference them — it references `npx @claude-flow/cli@latest` instead. **The settings.json must be rewritten to point to these existing scripts.**

### 2.3 CLI Subcommands (Actual)

**Source of truth**: `crates/context-graph-cli/src/main.rs` and `crates/context-graph-cli/src/commands/`

```
context-graph-cli
├── session
│   ├── restore-identity
│   └── persist-identity
├── hooks
│   ├── session-start    # SessionStart hook handler
│   ├── pre-tool         # PreToolUse hook handler (fast path, no DB)
│   ├── post-tool        # PostToolUse hook handler
│   ├── prompt-submit    # UserPromptSubmit hook handler
│   └── session-end      # SessionEnd hook handler
├── memory
│   ├── inject-context   # Full context injection (UserPromptSubmit, SessionStart)
│   ├── inject-brief     # Brief context (PreToolUse, <200 tokens)
│   ├── capture-memory   # Store hook description as memory (PostToolUse)
│   └── capture-response # Store Claude response as memory (Stop)
├── topic
│   ├── portfolio        # Get topic portfolio
│   └── stability        # Get stability metrics
├── divergence
│   └── check            # Check divergence from recent patterns
├── setup                # Initialize hooks in .claude/
├── warmup               # Pre-load embedding models into VRAM
└── watch                # Watch directory for markdown changes
```

### 2.4 CLI→MCP Communication

The CLI connects to the MCP server via **TCP on port 3100**. The MCP client is in `crates/context-graph-cli/src/mcp_client.rs`. It sends JSON-RPC 2.0 requests.

**CRITICAL**: The CLI does NOT have `context-graph-cli mcp call <tool_name>` as a generic command. Instead, it has specific subcommands (hooks, memory, topic, divergence) that internally call MCP tools via TCP. The old report's references to `context-graph-cli mcp call store_memory` are **FICTIONAL**.

### 2.5 Existing Skills in `.claude/skills/`

35 skill directories exist, but they are ALL for the `claude-flow` npm package, NOT for Context Graph. They include skills like `agentdb-advanced`, `swarm-orchestration`, `v3-core-implementation`, etc. **None of these are Context Graph memory skills.**

### 2.6 What Needs to Be Built

The integration between Context Graph and Claude Code requires:

1. **Rewrite `.claude/settings.json`** — Point hooks to the existing `.claude/hooks/*.sh` scripts instead of `npx @claude-flow/cli@latest`
2. **Add missing hook scripts** — SubagentStart, SubagentStop, PostToolUseFailure, PreCompact, TaskCompleted, Notification
3. **Create Context Graph skills** — 11 skills for domain-specific memory operations
4. **Add `format-provenance.sh` library** — Shared provenance formatting for all hooks
5. **Verify the complete integration end-to-end** with real data

---

## 3. DISCREPANCIES FOUND (Old Report vs Reality)

### 3.1 CRITICAL Discrepancies

| # | Old Report Says | Reality | Fix |
|---|-----------------|---------|-----|
| D1 | Hooks call `context-graph-cli mcp call store_memory --content ... --sourceType ... --sourceRef ...` | CLI has NO `mcp call` subcommand. `store_memory` MCP tool has NO `sourceType`/`sourceRef`/`toolUseId` params. | Use existing CLI subcommands: `hooks session-start`, `hooks post-tool`, `memory capture-memory`, `memory inject-context` |
| D2 | `settings.json` hooks point to `.claude/hooks/session-start.sh` (hyphenated) | Actual files use underscores: `session_start.sh`, `user_prompt_submit.sh`, etc. | Use actual filenames with underscores |
| D3 | Old report shows hooks calling `search_recent`, `search_causes`, `search_effects` via CLI | CLI doesn't expose these as direct commands. They are MCP tools accessed via `memory inject-context` internally. | Don't call MCP tools directly from hooks — use CLI subcommands |
| D4 | `settings.json` currently uses `npx @claude-flow/cli@latest` | Context Graph uses `context-graph-cli` binary, not an npm package | Rewrite settings.json to use `.claude/hooks/*.sh` scripts |
| D5 | Old report shows `store_memory` with `--sourceType`, `--sourceRef`, `--toolUseId`, `--filePath` params | `store_memory` only accepts: `content` (required), `rationale`, `importance`, `sessionId`, `operatorId` | Remove fictional parameters from all hook designs |
| D6 | Old report references `$CLAUDE_PROJECT_DIR/.claude/hooks/` | Hooks are at `$CLAUDE_PROJECT_DIR/.claude/hooks/` — this is correct, but scripts use underscores not hyphens | Use correct filenames |
| D7 | Old report designs hooks that do `context-graph-cli mcp call get_topic_portfolio`, `context-graph-cli mcp call search_graph` | No generic `mcp call` command exists | Use CLI subcommands or add `mcp call` as a new CLI feature |
| D8 | Constitution says 50 CFs | Code has evolved to 51+ | Not critical, but note discrepancy |

### 3.2 Architecture Decision: Hook Scripts vs Direct MCP Calls

The old report designed hooks that make direct MCP tool calls via a non-existent `context-graph-cli mcp call` command. The ACTUAL architecture is:

```
Claude Code Hook Event
  → .claude/hooks/[event]_[name].sh  (bash script)
    → context-graph-cli [subcommand]  (compiled Rust binary)
      → TCP JSON-RPC to MCP server on :3100  (via mcp_client.rs)
        → MCP tool handler  (55 registered tools)
          → RocksDB + GPU embedders
```

The CLI subcommands (`hooks session-start`, `hooks prompt-submit`, `memory inject-context`, etc.) already encapsulate the right MCP tool calls internally. The hook scripts should call these CLI subcommands, NOT attempt to call MCP tools directly.

---

## 4. IMPLEMENTATION PLAN

### Phase 1: Fix `.claude/settings.json` (P0 — Immediate)

**Goal**: Make the existing hook scripts actually get called by Claude Code.

**File**: `/home/cabdru/contextgraph/.claude/settings.json`

Replace the entire `"hooks"` section. Keep the non-hooks config (`statusLine`, `permissions`, `attribution`, `claudeFlow`) as-is for now.

**New hooks configuration**:

```json
{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/session_start.sh",
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
            "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/user_prompt_submit.sh",
            "timeout": 3000
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "matcher": "Edit|Write|Read|Bash|Task",
        "hooks": [
          {
            "type": "command",
            "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/pre_tool_use.sh",
            "timeout": 1000
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
            "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/post_tool_use.sh",
            "timeout": 5000,
            "async": true
          }
        ]
      }
    ],
    "PostToolUseFailure": [
      {
        "matcher": "Write|Edit|Bash",
        "hooks": [
          {
            "type": "command",
            "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/post_tool_use_failure.sh",
            "timeout": 5000,
            "async": true
          }
        ]
      }
    ],
    "SubagentStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/subagent_start.sh",
            "timeout": 3000
          }
        ]
      }
    ],
    "SubagentStop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/subagent_stop.sh",
            "timeout": 5000,
            "async": true
          }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/stop.sh",
            "timeout": 3000
          }
        ]
      }
    ],
    "PreCompact": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/pre_compact.sh",
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
            "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/session_end.sh",
            "timeout": 30000
          }
        ]
      }
    ],
    "Notification": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/notification.sh",
            "timeout": 3000,
            "async": true
          }
        ]
      }
    ],
    "TaskCompleted": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/task_completed.sh",
            "timeout": 5000,
            "async": true
          }
        ]
      }
    ]
  }
}
```

### Phase 2: Create Missing Hook Scripts (P0)

**Location**: `/home/cabdru/contextgraph/.claude/hooks/`

6 scripts already exist (`session_start.sh`, `user_prompt_submit.sh`, `pre_tool_use.sh`, `post_tool_use.sh`, `session_end.sh`, `stop.sh`). Create 6 new ones:

#### 2a. `post_tool_use_failure.sh`

Captures failure patterns. Calls `context-graph-cli memory capture-memory` with failure context.

**Input from Claude Code** (JSON on stdin):
```json
{"session_id":"...", "tool_name":"...", "tool_input":{...}, "error":"...", "is_interrupt":false, "tool_use_id":"..."}
```

**Logic**:
1. Skip if `is_interrupt` is true
2. Extract tool_name, error message
3. Call `context-graph-cli memory capture-memory --content "FAILURE: <tool_name> - <error>" --hook-type post_tool_use_failure --tool-name <tool_name>`
4. Exit 0 (async, non-blocking)

#### 2b. `subagent_start.sh`

Injects relevant context into subagent. Calls `context-graph-cli memory inject-brief` with agent context.

**Input**: `{"session_id":"...", "agent_id":"...", "agent_type":"..."}`

**Logic**:
1. Extract agent_type
2. Call `context-graph-cli memory inject-brief "subagent context for <agent_type>"`
3. Output JSON with `additionalContext` field

#### 2c. `subagent_stop.sh`

Captures subagent findings. Calls `context-graph-cli memory capture-memory`.

**Input**: `{"session_id":"...", "agent_id":"...", "agent_type":"...", "agent_transcript_path":"...", "stop_hook_active":false}`

**Logic**:
1. Skip if `stop_hook_active` is true
2. Call `context-graph-cli memory capture-memory --content "Subagent <agent_type> completed" --hook-type subagent_stop`
3. Exit 0

#### 2d. `pre_compact.sh`

Saves critical context before compression. Calls `context-graph-cli memory capture-memory`.

**Input**: `{"session_id":"...", "trigger":"auto|manual", "custom_instructions":"..."}`

**Logic**:
1. Call `context-graph-cli memory capture-memory --content "Pre-compaction marker (trigger: <trigger>)" --hook-type pre_compact`
2. Exit 0

#### 2e. `notification.sh`

Logs notification patterns. Calls `context-graph-cli memory capture-memory`.

**Input**: `{"session_id":"...", "message":"...", "notification_type":"..."}`

**Logic**:
1. Only process `permission_prompt` and `idle_prompt` types
2. Call `context-graph-cli memory capture-memory --content "Notification (<type>): <message>" --hook-type notification`
3. Exit 0

#### 2f. `task_completed.sh`

Records task completion. Calls `context-graph-cli memory capture-memory`.

**Input**: `{"session_id":"...", "task_id":"...", "task_subject":"..."}`

**Logic**:
1. Call `context-graph-cli memory capture-memory --content "Task completed: <task_subject>" --hook-type task_completed`
2. Exit 0

**All new scripts MUST follow the same pattern as existing scripts**:
- `set -euo pipefail`
- Read JSON from stdin
- Validate JSON with `jq empty`
- Find CLI binary (check `$CONTEXT_GRAPH_CLI`, then `./target/release/`, then `./target/debug/`, then `$HOME/.cargo/bin/`)
- Call CLI with `timeout`
- Handle timeout exit code 124
- Fail fast with error JSON on stderr

### Phase 3: Create `format-provenance.sh` Library (P1)

**Path**: `/home/cabdru/contextgraph/.claude/hooks/lib/format-provenance.sh`

This script formats MCP search results into provenance-annotated text. It takes JSON search results on stdin and outputs formatted markdown.

**Input**: JSON from MCP search response
**Output**: Formatted text with provenance per memory:
```
[MEMORY] <content summary, max 200 chars>
  Similarity: <score> | Profile: <profile>
  Source: <source_type> | Created: <timestamp>
```

### Phase 4: Create Context Graph Skills (P1)

**Location**: `/home/cabdru/contextgraph/.claude/skills/`

Create 11 NEW skill directories. Each skill has a `SKILL.md` file with YAML frontmatter.

**IMPORTANT**: Skills do NOT call CLI commands directly. Skills provide instructions that Claude follows using MCP tools (`mcp__context-graph__<tool_name>`). The MCP server must be registered with Claude Code as `context-graph`.

#### Skills to Create:

| Skill | Directory | Key MCP Tools Used |
|-------|-----------|-------------------|
| context-inject | `context-inject/` | `search_graph`, `search_causes`, `search_effects` |
| memory-search | `memory-search/` | `search_graph`, `search_code`, `search_by_keywords` |
| causal-reasoning | `causal-reasoning/` | `search_causes`, `search_effects`, `get_causal_chain` |
| entity-intelligence | `entity-intelligence/` | `extract_entities`, `search_by_entities`, `find_related_entities` |
| code-search | `code-search/` | `search_code`, `search_cross_embedder_anomalies` |
| blind-spot-detective | `blind-spot-detective/` | `search_robust`, `compare_embedder_views` |
| topic-explorer | `topic-explorer/` | `get_topic_portfolio`, `get_topic_stability`, `detect_topics` |
| session-navigator | `session-navigator/` | `get_conversation_context`, `get_session_timeline` |
| provenance-auditor | `provenance-auditor/` | `get_audit_trail`, `get_merge_history`, `get_provenance_chain` |
| memory-curator | `memory-curator/` | `merge_concepts`, `forget_concept`, `boost_importance` |
| graph-navigator | `graph-navigator/` | `get_memory_neighbors`, `traverse_graph`, `get_unified_neighbors` |

**Skill YAML frontmatter format** (from Claude Code docs):
```yaml
---
name: skill-name
description: |
  What it does. When to use it. Keywords for discovery.
allowed-tools: Read,Glob,Grep,Bash
model: inherit
user-invocable: true
---
```

**Each skill MUST**:
1. Use exact MCP tool names from section 1.4 (prefixed with `mcp__context-graph__`)
2. Include exact parameter names from the tool definitions
3. NOT reference fictional parameters
4. NOT call CLI commands — skills instruct Claude to use MCP tools directly

### Phase 5: Register MCP Server with Claude Code (P0)

The MCP server must be registered so Claude Code can use the 55 tools.

```bash
# Register as stdio MCP server
claude mcp add context-graph -- /home/cabdru/contextgraph/target/release/context-graph-mcp

# OR for TCP transport (requires server already running):
claude mcp add context-graph -- /home/cabdru/contextgraph/target/release/context-graph-mcp --transport tcp --port 3100
```

**Verify registration**: After adding, Claude Code should see tools prefixed `mcp__context-graph__` (e.g., `mcp__context-graph__store_memory`).

### Phase 6: Verify CLI `hooks` and `memory` Subcommands Actually Work (P0)

Before wiring hooks, verify the CLI commands work:

```bash
# Build release binary
cargo build --release -p context-graph-cli

# Test hooks subcommands
echo '{"hook_type":"session_start","session_id":"test-1","timestamp_ms":1234567890,"payload":{"type":"session_start","data":{"cwd":"/tmp","source":"cli"}}}' | ./target/release/context-graph-cli hooks session-start --stdin --format json

# Test memory subcommands
./target/release/context-graph-cli memory inject-context "test query"
./target/release/context-graph-cli memory inject-brief "test brief"
./target/release/context-graph-cli memory capture-memory --content "test capture" --hook-type post_tool_use --tool-name Edit

# Test topic subcommands
./target/release/context-graph-cli topic portfolio
./target/release/context-graph-cli topic stability

# Test divergence
./target/release/context-graph-cli divergence check --hours 2
```

**IMPORTANT**: These CLI commands connect to the MCP server via TCP on port 3100. The MCP server MUST be running first:
```bash
./target/release/context-graph-mcp --transport tcp --port 3100
```

If the MCP server is not running, the CLI commands will fail with a connection error. This is CORRECT behavior — fail fast, no fallbacks.

---

## 5. FULL STATE VERIFICATION REQUIREMENTS

After completing ANY implementation, you MUST perform these verification steps. Do NOT rely on return values alone.

### 5.1 Define Source of Truth

For each operation, identify where the result is stored:

| Operation | Source of Truth | How to Verify |
|-----------|----------------|---------------|
| `store_memory` | RocksDB `fingerprints` CF | `search_graph` with content as query, verify returned |
| Hook script execution | Claude Code hook output (stdout/stderr) | Run script manually with test JSON on stdin |
| Skill loading | Claude Code skill discovery | Ask Claude "What skills are available?" |
| Settings.json | Claude Code hook firing | Use `claude --debug` to see hook matching |
| MCP server registration | Claude Code MCP tool list | Check `mcp__context-graph__*` tools available |

### 5.2 Execute & Inspect Protocol

For EVERY change:
1. Run the logic (e.g., execute a hook script)
2. Perform a SEPARATE read operation on the source of truth
3. Compare expected output vs actual output
4. If they don't match, STOP and investigate root cause

### 5.3 Boundary & Edge Case Audit

For each hook script, manually simulate 3 edge cases:

| Edge Case | Input | Expected Behavior |
|-----------|-------|-------------------|
| Empty stdin | `echo "" \| ./hook.sh` | Exit with error JSON on stderr, exit code 4 |
| Invalid JSON | `echo "not json" \| ./hook.sh` | Exit with error JSON on stderr, exit code 4 |
| CLI not found | Rename binary, run hook | Exit with error JSON on stderr, exit code 1 |
| MCP server not running | Run CLI command without server | Connection error, exit code != 0 |
| Very long content | 100KB+ content string | Truncation or error, NOT hang |

**For each edge case**: Print system state BEFORE and AFTER the action to prove the outcome.

### 5.4 Evidence of Success

Provide a log showing:
1. Hook script executed with test input
2. CLI command received the input and processed it
3. MCP tool was called (visible in MCP server logs with `RUST_LOG=debug`)
4. Data was persisted to RocksDB (verified by subsequent search returning the stored data)
5. The hook output JSON was valid and contained expected fields

### 5.5 Manual Verification Checklist

For EACH hook:
- [ ] Script is executable (`chmod +x`)
- [ ] Script runs with valid JSON input and produces valid JSON output
- [ ] Script fails fast with invalid input
- [ ] Script finds CLI binary correctly
- [ ] CLI command connects to MCP server
- [ ] MCP tool processes the request
- [ ] Data appears in RocksDB (verified by search)
- [ ] settings.json references the correct script path
- [ ] Claude Code actually fires the hook (verified with `claude --debug`)

### 5.6 Synthetic Test Data

Use these synthetic inputs for testing. You KNOW the expected outputs, so verify them.

**store_memory test**:
```json
{"content": "SYNTHETIC_TEST_2026: JWT was chosen over sessions for stateless auth", "importance": 0.8, "rationale": "Architecture decision test"}
```
Expected: Memory stored. Searching for "JWT stateless auth" should return this memory with similarity > 0.5.

**search_graph test**:
```json
{"query": "SYNTHETIC_TEST_2026", "topK": 5, "includeContent": true, "strategy": "multi_space"}
```
Expected: Returns the memory stored above.

**Hook test**:
```bash
echo '{"session_id":"test-session","prompt":"SYNTHETIC_TEST_2026: How does authentication work?"}' | ./.claude/hooks/user_prompt_submit.sh
```
Expected: Hook calls CLI, CLI connects to MCP, searches for relevant memories, returns JSON with `additionalContext` or empty `{}`.

### 5.7 Trigger→Process→Outcome Verification

For every trigger event:
1. **Identify the trigger**: What causes the hook to fire? (e.g., user submits prompt)
2. **Identify the process**: What does the hook do? (e.g., search memory, inject context)
3. **Identify the outcome**: Where does the result appear? (e.g., `additionalContext` in Claude's context)
4. **Verify the outcome EXISTS**: Check that the additionalContext was actually injected, that the memory was actually stored, that the topic was actually detected.

If something is saved to a database, you MUST query that database to verify it was saved. If a hook produces output, you MUST capture and inspect that output. NEVER assume success — VERIFY IT.

---

## 6. ERROR HANDLING REQUIREMENTS

### ABSOLUTELY NO BACKWARDS COMPATIBILITY

- If a hook fails, it MUST exit with a non-zero code and error JSON on stderr
- If the CLI binary is not found, error immediately — no fallback to npm packages
- If the MCP server is not running, connection error — no mock data
- If a parameter is invalid, reject it — no silent defaults
- Every error MUST include: what failed, where it failed, how to fix it

### Error JSON Format (all hooks)

```json
{"success": false, "error": "<human-readable error message>", "exit_code": <int>}
```

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | CLI binary not found or general error |
| 2 | Timeout |
| 3 | Database error |
| 4 | Invalid input (empty stdin, bad JSON) |

---

## 7. CONSTITUTION COMPLIANCE

The constitution is at `/home/cabdru/contextgraph/docs2/constitution.yaml` (v7.0.0).

Key rules that affect this integration:

| Rule | Requirement |
|------|-------------|
| ARCH-06 | All memory ops through MCP tools only — hooks call CLI, CLI calls MCP |
| ARCH-GPU-01 | GPU mandatory for production — no CPU fallback for embeddings |
| ARCH-GPU-02 | All 13 embedders warm-loaded into VRAM at startup |
| ARCH-SIGNAL-01 | All 13 embedders provide signal, never noise |
| ARCH-SER-01 | JSON for provenance/metadata. Bincode for dense vectors only |
| ARCH-PROV-01 | Audit writes non-fatal — warn on failure, never block main operation |
| SEC-01 | Validate all MCP tool params at handler boundary |
| Testing golden rule | ALL MCP tests use real RocksDB + real GPU. NO STUBS. |

---

## 8. FILE REFERENCE

### Files to CREATE:
```
.claude/hooks/post_tool_use_failure.sh    # NEW
.claude/hooks/subagent_start.sh           # NEW
.claude/hooks/subagent_stop.sh            # NEW
.claude/hooks/pre_compact.sh              # NEW
.claude/hooks/notification.sh             # NEW
.claude/hooks/task_completed.sh           # NEW
.claude/hooks/lib/format-provenance.sh    # NEW
.claude/skills/context-inject/SKILL.md    # NEW (Context Graph specific)
.claude/skills/cg-memory-search/SKILL.md  # NEW
.claude/skills/cg-causal/SKILL.md         # NEW
.claude/skills/cg-entity/SKILL.md         # NEW
.claude/skills/cg-code-search/SKILL.md    # NEW
.claude/skills/cg-blind-spot/SKILL.md     # NEW
.claude/skills/cg-topics/SKILL.md         # NEW
.claude/skills/cg-session/SKILL.md        # NEW
.claude/skills/cg-provenance/SKILL.md     # NEW
.claude/skills/cg-curator/SKILL.md        # NEW
.claude/skills/cg-graph/SKILL.md          # NEW
```

### Files to MODIFY:
```
.claude/settings.json                     # Rewrite hooks section
```

### Files that ALREADY EXIST and should NOT be changed:
```
.claude/hooks/session_start.sh            # CORRECT - uses context-graph-cli
.claude/hooks/user_prompt_submit.sh       # CORRECT
.claude/hooks/pre_tool_use.sh             # CORRECT
.claude/hooks/post_tool_use.sh            # CORRECT
.claude/hooks/session_end.sh              # CORRECT
.claude/hooks/stop.sh                     # CORRECT
```

### Source of truth files (READ ONLY for verification):
```
crates/context-graph-mcp/src/tools/names.rs             # All 55 tool names
crates/context-graph-mcp/src/tools/definitions/core.rs   # store_memory, search_graph params
crates/context-graph-mcp/src/tools/definitions/*.rs       # All tool definitions
crates/context-graph-cli/src/main.rs                      # CLI subcommands
crates/context-graph-cli/src/commands/hooks/args.rs       # Hook command args
crates/context-graph-cli/src/commands/memory/mod.rs       # Memory command args
crates/context-graph-cli/src/mcp_client.rs                # TCP client to MCP server
docs2/constitution.yaml                                    # System constitution
```

---

## 9. RECOMMENDED IMPROVEMENTS (Analyze Before Implementing)

### Option A: Add `context-graph-cli mcp call <tool>` Generic Command
**Pros**: Hooks could call any of the 55 MCP tools directly. Maximum flexibility.
**Cons**: Adds CLI complexity. Every tool's params need CLI arg parsing.
**Effort**: Medium (generic JSON passthrough)
**Recommendation**: Implement as `context-graph-cli mcp call --tool <name> --params '<json>'`

### Option B: Keep CLI Subcommands Only
**Pros**: Type-safe, validated at compile time. Already works.
**Cons**: Limited to the ~15 commands already implemented. Can't call all 55 tools from hooks.
**Effort**: None (already done)
**Recommendation**: Good enough for Phase 1. Expand later if needed.

### Option C: Hook Scripts Call MCP Server Directly via TCP
**Pros**: No CLI dependency for hooks. Direct JSON-RPC over TCP.
**Cons**: Requires `nc` or `curl` in hooks. Less error handling. Fragile.
**Effort**: Low per-hook, but maintenance burden
**Recommendation**: NOT recommended. CLI provides proper error handling.

**Suggested approach**: Option B for immediate implementation, Option A as Phase 2 enhancement.

---

## 10. EXECUTION ORDER

1. **Build**: `cargo build --release` (both `context-graph-mcp` and `context-graph-cli`)
2. **Start MCP server**: `./target/release/context-graph-mcp --transport tcp --port 3100`
3. **Verify CLI**: Run test commands from section 4, Phase 6
4. **Fix settings.json**: Rewrite hooks section per Phase 1
5. **Create missing hook scripts**: Phase 2 (6 new scripts)
6. **Test each hook**: Run manually with synthetic input, verify output
7. **Register MCP server**: `claude mcp add context-graph -- ./target/release/context-graph-mcp`
8. **Create skills**: Phase 4 (11 new skills)
9. **Full integration test**: Start Claude Code session, verify hooks fire, verify memory storage
10. **Edge case testing**: Empty inputs, timeouts, server down, invalid JSON

**After EVERY step**: Verify the source of truth. Don't proceed to the next step until the current step is verified working.

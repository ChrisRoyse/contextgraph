# CLI Reference

Source files covered:
- `crates/context-graph-cli/src/main.rs`
- `crates/context-graph-cli/src/error.rs`
- `crates/context-graph-cli/src/mcp_client.rs`
- `crates/context-graph-cli/src/mcp_helpers.rs`
- `crates/context-graph-cli/src/commands/mod.rs`
- `crates/context-graph-cli/src/commands/hooks/mod.rs`
- `crates/context-graph-cli/src/commands/hooks/args.rs`
- `crates/context-graph-cli/src/commands/hooks/types.rs`
- `crates/context-graph-cli/src/commands/hooks/error.rs`
- `crates/context-graph-cli/src/commands/hooks/pre_tool_use.rs`
- `crates/context-graph-cli/src/commands/hooks/post_tool_use.rs`
- `crates/context-graph-cli/src/commands/hooks/session_start.rs`
- `crates/context-graph-cli/src/commands/hooks/session_end.rs`
- `crates/context-graph-cli/src/commands/hooks/user_prompt_submit.rs`
- `crates/context-graph-cli/src/commands/hooks/pre_compact.rs`
- `crates/context-graph-cli/src/commands/hooks/task_completed.rs`
- `crates/context-graph-cli/src/commands/hooks/session_state.rs`
- `crates/context-graph-cli/src/commands/hooks/memory_cache.rs`
- `crates/context-graph-cli/src/commands/memory/mod.rs`
- `crates/context-graph-cli/src/commands/memory/capture.rs`
- `crates/context-graph-cli/src/commands/memory/inject.rs`
- `crates/context-graph-cli/src/commands/session/mod.rs`
- `crates/context-graph-cli/src/commands/session/persist.rs`
- `crates/context-graph-cli/src/commands/session/restore.rs`
- `crates/context-graph-cli/src/commands/setup.rs`
- `crates/context-graph-cli/src/commands/warmup.rs`
- `crates/context-graph-cli/src/commands/watch.rs`
- `crates/context-graph-cli/src/commands/topic.rs`
- `crates/context-graph-cli/src/commands/divergence.rs`
- `crates/context-graph-cli/src/commands/export_training.rs`

---

## 1. Binary Name and Invocation

**Binary**: `context-graph-cli`

**Version**: 0.1.0

**Author**: Context Graph Team

**Synopsis**:

```
context-graph-cli [OPTIONS] <COMMAND>
```

The CLI provides hooks integration for Claude Code via `.claude/settings.json`, memory management, topic analysis, divergence detection, and model warmup utilities.

### 1.1 Global Options

| Flag | Short | Description |
|------|-------|-------------|
| `--verbose` | `-v` | Verbosity level. Repeat for more detail: `-v` (info), `-vv` (debug), `-vvv` (trace). Default: `warn` (also respects `RUST_LOG` env var). |
| `--version` | `-V` | Print version information. |
| `--help` | `-h` | Print help. |

### 1.2 Logging

All logging goes to **stderr** (not stdout) via `tracing_subscriber`. Verbosity mapping:

| Count | Level |
|-------|-------|
| 0 | `warn` (or `RUST_LOG` env var) |
| 1 (`-v`) | `info` |
| 2 (`-vv`) | `debug` |
| 3+ (`-vvv`) | `trace` |

---

## 2. Top-Level Commands

| Command | Description |
|---------|-------------|
| `session` | Session persistence commands (restore/persist identity) |
| `hooks` | Claude Code native hooks commands (session-start, pre-tool, post-tool, etc.) |
| `memory` | Memory capture and context injection commands |
| `topic` | Topic portfolio and stability commands |
| `divergence` | Divergence detection commands |
| `setup` | Initialize context-graph hooks for Claude Code |
| `warmup` | Pre-load embedding models into VRAM |
| `watch` | Watch a directory for markdown file changes |
| `export` | Export training data (Parquet format) from RocksDB column families |

---

## 2.1 Export Command

**Subcommand**: `context-graph-cli export training-corpus`

**Source**: `crates/context-graph-cli/src/commands/export_training.rs`

Export training records from `CF_TRAINING_RECORDS` to a Parquet file. Records are byte-for-byte identical to the on-disk RocksDB values (version byte + bincode).

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--kind` | string | `training-corpus` | Export kind (only `training-corpus` supported) |
| `--format` | string | `parquet` | Output format (only `parquet` supported) |
| `--out` | path | (required) | Output file path; parent directory must exist |
| `--storage` | path | (required) | RocksDB data directory |
| `--batch-size` | usize | `500` | Records per Parquet row group |
| `--limit` | usize | (none) | Optional cap on total records exported |

**Parquet Schema**:

| Column | Type | Description |
|--------|------|-------------|
| `memory_id` | STRING (Utf8) | Memory UUID as string |
| `record_bytes` | BINARY | Raw `[version_byte][bincode(TrainingRecord)]` payload |

**Compression**: Zstd (default level).

**Exit Codes**: 0 = success (prints `ExportSummary` to stdout), 1 = error (partially-written Parquet file is deleted on failure).

**Output**: `ExportSummary { total_records, bytes_written, row_groups, elapsed_ms }`.

**Cross-reference**: See [16_training_subsystem.md](16_training_subsystem.md) for `TrainingRecord` structure and the MCP-side `export_training_corpus` tool.

---

## 3. Hook System

The hook system integrates with Claude Code's native hook mechanism. Each hook type has a strict timeout budget enforced by Claude Code. All hook commands live under `context-graph-cli hooks <SUBCOMMAND>`.

### 3.1 Hook Type Summary

| Hook Type | CLI Subcommand | Timeout | DB Access | Description |
|-----------|---------------|---------|-----------|-------------|
| SessionStart | `session-start` | 5000ms | No (cache) | Session initialization and state restoration |
| PreToolUse | `pre-tool` | 500ms | **No** (fast path) | Pre-tool brief injection; cache only, no disk I/O |
| PostToolUse | `post-tool` | 3000ms | Yes (MCP) | Post-tool state verification and memory capture |
| UserPromptSubmit | `prompt-submit` | 2000ms | Yes (MCP) | User prompt context injection |
| SessionEnd | `session-end` | 30000ms | Yes (cache) | Session persistence and final state |
| PreCompact | `pre-compact` | 20000ms | Yes (MCP) | Saves session summary before context compression |
| TaskCompleted | `task-completed` | 20000ms | Yes (MCP) | Extracts learnings from completed tasks |
| GenerateConfig | `generate-config` | N/A | No | Generates hook configuration files (not implemented; use `setup` instead) |

### 3.2 Hook Output Format

All hook commands emit JSON to **stdout** on success and JSON to **stderr** on error. The output conforms to the `HookOutput` structure:

```json
{
  "success": true,
  "error": null,
  "execution_time_ms": 12,
  "coherence_state": {
    "coherence": 0.75,
    "integration": 0.80,
    "reflection": 0.70,
    "differentiation": 0.75,
    "topic_stability": 1.0
  },
  "stability_classification": {
    "value": 0.75,
    "level": "Healthy",
    "crisis_triggered": false
  },
  "context_injection": "## Relevant Context\n..."
}
```

### 3.3 Hook Exit Codes

| Code | Meaning | Description |
|------|---------|-------------|
| 0 | Success | Hook executed successfully |
| 1 | General Error | Unspecified error |
| 4 | Invalid Input | Malformed input data |

### 3.4 Output Format Enum

All hook subcommands accept a `--format` flag:

| Value | Description |
|-------|-------------|
| `json` | JSON format with pretty printing (default for hook integration) |
| `json-compact` | Compact JSON (single line, minimal whitespace) |
| `text` | Human-readable text (for debugging) |

### 3.5 `hooks session-start`

Initializes session identity and state.

```
context-graph-cli hooks session-start [OPTIONS]
```

| Flag | Required | Default | Description |
|------|----------|---------|-------------|
| `--session-id <ID>` | No | Auto-generated (`session-<timestamp>`) | Session identifier |
| `--previous-session-id <ID>` | No | None | Previous session ID for session linking |
| `--stdin` | No | `false` | Read `HookInput` JSON from stdin |
| `--format <FORMAT>` | No | `json` | Output format (`json`, `json-compact`, `text`) |

**Behavior**: Loads or creates a `SessionSnapshot` from the in-memory `SessionCache`. If `--previous-session-id` is provided, links to the previous session. Returns `HookOutput` with coherence state and stability classification.

### 3.6 `hooks pre-tool`

Fast-path pre-tool execution hook. Must complete within 500ms. No database access.

```
context-graph-cli hooks pre-tool --session-id <ID> [OPTIONS]
```

| Flag | Required | Default | Description |
|------|----------|---------|-------------|
| `--session-id <ID>` | **Yes** | -- | Session identifier |
| `--tool-name <NAME>` | No | None | Tool name being invoked (e.g., `Read`, `Write`, `Bash`) |
| `--stdin <BOOL>` | No | `false` | Read `HookInput` JSON from stdin |
| `--fast-path <BOOL>` | No | `true` | Skip database access for faster response. Must remain `true` to meet 500ms timeout. |
| `--format <FORMAT>` | No | `json` | Output format |

**Behavior**: Returns cached coherence state with default values. Provides tool-specific guidance strings for known tools. Retrieves cached memories from the filesystem memory cache (populated by `prompt-submit`).

**Tool guidance mapping** (pure computation, no I/O):

| Tool Name | Guidance |
|-----------|----------|
| `Read` | Track file content in awareness quadrant |
| `Write`, `Edit`, `MultiEdit` | File modification - track in self-knowledge |
| `Bash` | Shell command - monitor output for identity markers |
| `WebFetch`, `WebSearch` | External data - evaluate for awareness expansion |
| `Git` | Git operation - track project state changes |
| `LSP` | Code intelligence - update technical awareness |
| `NotebookEdit` | Notebook modification - track analysis state |
| `TodoWrite`, `TodoRead` | Task tracking - update operational context |
| `Glob`, `Grep` | Search operation - expand file awareness |
| `Task` | Agent spawn - coordinate swarm awareness |
| (other) | No guidance |

### 3.7 `hooks post-tool`

Post-tool execution hook. Captures tool responses and updates coherence.

```
context-graph-cli hooks post-tool --session-id <ID> [OPTIONS]
```

| Flag | Required | Default | Description |
|------|----------|---------|-------------|
| `--session-id <ID>` | **Yes** | -- | Session identifier |
| `--tool-name <NAME>` | No* | None | Tool name that was executed. *Required when `--stdin` is `false`. |
| `--success <BOOL>` | No | `true` (when not using stdin) | Whether tool execution succeeded |
| `--stdin <BOOL>` | No | `false` | Read `HookInput` JSON from stdin |
| `--format <FORMAT>` | No | `json` | Output format |

**Behavior**: Parses tool response, analyzes impact level, updates `SessionSnapshot` coherence metrics, and optionally captures the tool response as a memory via MCP `inject_context`.

**Impact levels by tool**:

| Tool | Impact Level |
|------|-------------|
| `Read` | Low |
| `Write`, `Edit`, `MultiEdit` | Medium |
| `Bash` (success) | Medium |
| `Bash` (failure) | High |
| `WebFetch`, `WebSearch` | Medium |
| `Git` | Low |
| `Task` | High |
| (other) | None |

**Memory capture**: Responses from `Read`, `Bash`, `WebFetch`, `Grep`, and `LSP` are stored as `HookDescription` memories (importance 0.4) if they are at least 50 characters. Failed tool responses from all tools are captured as `FailurePattern` memories (importance 0.3). Responses are truncated to 2000 characters.

### 3.8 `hooks prompt-submit`

User prompt submission hook. Injects relevant context from memory.

```
context-graph-cli hooks prompt-submit --session-id <ID> [OPTIONS]
```

| Flag | Required | Default | Description |
|------|----------|---------|-------------|
| `--session-id <ID>` | **Yes** | -- | Session identifier |
| `--prompt <TEXT>` | No | None | User prompt text (alternative to stdin) |
| `--stdin <BOOL>` | No | `false` | Read `HookInput` JSON from stdin |
| `--format <FORMAT>` | No | `json` | Output format |

**Behavior**: Searches the knowledge graph via MCP for memories relevant to the user prompt. Caches retrieved memories to the filesystem cache at `/tmp/cg-memory-cache-{session_id}.json` for fast retrieval by `pre-tool`. Supports adaptive token budgets: 2000 characters for normal prompts, 3000 characters for complex prompts (>500 characters). Minimum query length for search is 10 characters. Minimum memory similarity threshold is 0.4.

### 3.9 `hooks session-end`

Session termination hook. Persists final state.

```
context-graph-cli hooks session-end --session-id <ID> [OPTIONS]
```

| Flag | Required | Default | Description |
|------|----------|---------|-------------|
| `--session-id <ID>` | **Yes** | -- | Session identifier |
| `--duration-ms <MS>` | No | None | Session duration in milliseconds |
| `--stdin <BOOL>` | No | `false` | Read `HookInput` JSON from stdin |
| `--generate-summary <BOOL>` | No | `true` | Generate session summary on end |
| `--format <FORMAT>` | No | `json` | Output format |

**Behavior**: Reads warm cache, persists snapshot to `SessionCache`, cleans up the filesystem memory cache for the session, and returns final coherence state. When reading from stdin, expects `SessionEnd` payload with `duration_ms`, `status`, and `reason` fields. Status can be `Normal`, `Timeout`, `Error`, or `Interrupted`.

### 3.10 `hooks pre-compact`

Captures a session summary before Claude Code compresses the context window.

```
context-graph-cli hooks pre-compact --session-id <ID> [OPTIONS]
```

| Flag | Required | Default | Description |
|------|----------|---------|-------------|
| `--session-id <ID>` | **Yes** | -- | Session identifier |
| `--stdin <BOOL>` | No | `false` | Read `HookInput` JSON from stdin |
| `--format <FORMAT>` | No | `json` | Output format |

**Behavior**: Extracts recent user prompts from conversation context (up to 5), builds a session summary (max 2000 characters), and stores it via MCP `inject_context` with importance 0.8 and tags `session-summary`, `pre-compact`. Returns `HookOutput::error` if MCP server is unavailable or storage fails.

### 3.11 `hooks task-completed`

Extracts learnings from completed tasks and stores them as memories.

```
context-graph-cli hooks task-completed --session-id <ID> [OPTIONS]
```

| Flag | Required | Default | Description |
|------|----------|---------|-------------|
| `--session-id <ID>` | **Yes** | -- | Session identifier |
| `--stdin <BOOL>` | No | `false` | Read `HookInput` JSON from stdin |
| `--format <FORMAT>` | No | `json` | Output format |

**Behavior**: Parses `TaskCompleted` payload from stdin (with `task_subject`, `task_id`, `task_result`), builds a summary, and stores it via MCP `inject_context` with importance 0.6 and tags `task-completion`, `learning`. Task results are truncated to 1500 characters. Returns `HookOutput::error` if MCP is unavailable.

### 3.12 `hooks generate-config`

Generates hook configuration files. **Not implemented** -- logs an error directing the user to run `context-graph-cli setup --generate-hooks` instead.

```
context-graph-cli hooks generate-config [OPTIONS]
```

| Flag | Required | Default | Description |
|------|----------|---------|-------------|
| `--output-dir <PATH>` | No | `.claude/hooks` | Output directory for hook scripts |
| `--force <BOOL>` | No | `false` | Overwrite existing files |
| `--hooks <LIST>` | No | All | Comma-separated hook types: `session-start`, `pre-tool-use`, `post-tool-use`, `user-prompt-submit`, `session-end` |
| `--shell <SHELL>` | No | `bash` | Shell to target: `bash`, `zsh`, `fish`, `powershell` |

---

## 4. Memory Commands

Memory commands handle memory storage, retrieval, and context injection for Claude Code hook integration. All memory operations go through the MCP server (no direct DB access).

### 4.1 `memory inject-context`

Injects relevant context from the memory store. Called by `UserPromptSubmit` and `SessionStart` hooks.

```
context-graph-cli memory inject-context [OPTIONS] [QUERY]
```

| Argument/Flag | Required | Default | Description |
|--------------|----------|---------|-------------|
| `[QUERY]` | No | `USER_PROMPT` env var | Query text for context retrieval |
| `--session-id <ID>` | No | `CLAUDE_SESSION_ID` env var, then `"default"` | Session identifier |
| `--budget <TOKENS>` | No | `1200` | Token budget for context (per constitution `injection.priorities`) |
| `--top-k <N>` | No | `10` | Maximum number of results to retrieve |

**Environment variables**:
- `USER_PROMPT`: Query text source (fallback when no positional argument)
- `CLAUDE_SESSION_ID`: Session identifier
- `CONTEXT_GRAPH_MCP_HOST`: MCP server host (default: `127.0.0.1`)
- `CONTEXT_GRAPH_MCP_PORT`: MCP server port (default: `3100`)

**Behavior**: Searches the knowledge graph via MCP `search_graph`. Formats results as markdown with similarity scores. Respects token budget (estimated at 4 chars/token). Empty query or empty result returns exit 0 with no stdout output.

**Output** (stdout): Markdown-formatted memories with similarity scores, or empty if no results.

### 4.2 `memory inject-brief`

Injects brief context for the `PreToolUse` hook (under 200 tokens).

```
context-graph-cli memory inject-brief [OPTIONS] [QUERY]
```

| Argument/Flag | Required | Default | Description |
|--------------|----------|---------|-------------|
| `[QUERY]` | No | `TOOL_DESCRIPTION` env var, then `TOOL_NAME` | Query text |
| `--session-id <ID>` | No | `CLAUDE_SESSION_ID` env var, then `"default"` | Session identifier |
| `--top-k <N>` | No | `5` | Maximum number of results to retrieve |

**Note**: No `--budget` flag. Brief context always uses 200 tokens (`BRIEF_BUDGET` constant).

**Environment variables**:
- `TOOL_DESCRIPTION`: Primary query source
- `TOOL_NAME`: Fallback query source
- `CLAUDE_SESSION_ID`: Session identifier
- `CONTEXT_GRAPH_MCP_HOST` / `CONTEXT_GRAPH_MCP_PORT`: MCP server address

**Behavior**: First checks the filesystem memory cache (populated by `prompt-submit` hook) for instant access without MCP round-trip. Falls back to MCP `search_graph` if cache is empty.

### 4.3 `memory capture-memory`

Captures hook descriptions as memories. Called by `PostToolUse` and `SessionEnd` hooks. Silent on success (no stdout output).

```
context-graph-cli memory capture-memory [OPTIONS]
```

| Flag | Required | Default | Description |
|------|----------|---------|-------------|
| `--content <TEXT>` | No | `TOOL_DESCRIPTION` env var, then `SESSION_SUMMARY` | Content to capture |
| `--source <TYPE>` | No | `"hook"` | Memory source type (always `HookDescription`) |
| `--session-id <ID>` | No | `CLAUDE_SESSION_ID` env var, then `"default"` | Session identifier |
| `--hook-type <TYPE>` | No | `post_tool_use` | Hook type. Valid values: `session_start`, `user_prompt_submit`, `pre_tool_use`, `post_tool_use`, `stop`, `session_end` (case-insensitive, also accepts concatenated forms like `sessionstart`) |
| `--tool-name <NAME>` | No | None | Tool name for `PreToolUse`/`PostToolUse` hooks |
| `--importance <FLOAT>` | No | `0.5` | Importance score [0.0, 1.0] |

**Environment variables**:
- `TOOL_DESCRIPTION`: Primary content source
- `SESSION_SUMMARY`: Fallback content source
- `CLAUDE_SESSION_ID`: Session identifier

**Content validation**: Maximum 10,000 characters. Empty/whitespace-only content returns exit 0 (silent ignore).

### 4.4 `memory capture-response`

Captures Claude responses as memories. Called by the Stop hook. Silent on success.

```
context-graph-cli memory capture-response [OPTIONS]
```

| Flag | Required | Default | Description |
|------|----------|---------|-------------|
| `--content <TEXT>` | No | `RESPONSE_SUMMARY` env var | Response content to capture |
| `--session-id <ID>` | No | `CLAUDE_SESSION_ID` env var, then `"default"` | Session identifier |
| `--response-type <TYPE>` | No | `stop_response` | Response type. Valid values: `session_summary`, `stop_response`, `significant_response` (case-insensitive, also accepts concatenated forms) |
| `--importance <FLOAT>` | No | `0.5` | Importance score [0.0, 1.0] |

**Environment variables**:
- `RESPONSE_SUMMARY`: Content source
- `CLAUDE_SESSION_ID`: Session identifier

---

## 5. Session Commands

Session commands manage session identity persistence and restoration using the in-memory `SessionCache` (per PRD v6 Section 14).

### 5.1 `session restore-identity`

Restores session state from storage.

```
context-graph-cli session restore-identity [OPTIONS]
```

| Flag | Required | Default | Description |
|------|----------|---------|-------------|
| `--db-path <PATH>` | No | `CONTEXT_GRAPH_DB_PATH` env var, then `~/.context-graph/db` | Path to RocksDB database directory (for reference; actual state uses in-memory cache) |
| `--format <FORMAT>` | No | `prd` | Output format: `prd` (PRD Section 15.2 compliant, ~100 tokens) or `json` |

**Stdin input** (JSON, optional):

```json
{
  "session_id": "optional-specific-session",
  "source": "startup"
}
```

**Source variants**:

| Source | Behavior |
|--------|----------|
| `startup` (default) | Loads latest session from cache, or creates new one |
| `resume` | Loads specific session by `session_id`. Falls back to `startup` if no ID provided. |
| `clear` | Creates a fresh session (new session ID) |

**PRD output format**:

```
## Session State
- State: Active (coherence=0.82)
- Session: session-1736985432 (source=startup)
```

**Coherence state names**: `Active` (>=0.8), `Aware` (>=0.5), `DIM` (>=0.2), `DOR` (<0.2).

### 5.2 `session persist-identity`

Persists current session identity to storage. Silent on success (no stdout output) as required by Claude Code `SessionEnd` semantics.

```
context-graph-cli session persist-identity [OPTIONS]
```

| Flag | Required | Default | Description |
|------|----------|---------|-------------|
| `--db-path <PATH>` | No | `CONTEXT_GRAPH_DB_PATH` env var, then `~/.context-graph/db` | Path to RocksDB database directory (for reference; actual state uses in-memory cache) |

**Stdin input** (JSON, optional):

```json
{
  "session_id": "optional-session-id",
  "reason": "exit"
}
```

**Reason values**: `exit`, `clear`, `logout`, `prompt_input_exit`, `other`. Default: `exit`.

**Behavior**: Gets the current snapshot from `SessionCache`, optionally overrides `session_id` from stdin, and stores the updated snapshot back to the cache. Returns exit 0 (silent) even when cache is cold (nothing to persist).

---

## 6. Setup Command

Initializes context-graph hooks for Claude Code by creating `.claude/settings.json` and `.claude/hooks/` directory with all required hook scripts.

```
context-graph-cli setup [OPTIONS]
```

| Flag | Short | Required | Default | Description |
|------|-------|----------|---------|-------------|
| `--force` | `-f` | No | `false` | Force overwrite existing configuration |
| `--target-dir <PATH>` | -- | No | Current working directory | Target directory for hook setup |
| `--skip-chmod` | -- | No (hidden) | `false` | Skip making scripts executable (testing only) |

**Behavior**:

1. Creates `.claude/` and `.claude/hooks/` directories.
2. Writes or merges `.claude/settings.json` with hook configuration. Existing non-hook settings are preserved.
3. Writes 5 hook shell scripts:
   - `session_start.sh` (timeout: 5000ms)
   - `pre_tool_use.sh` (timeout: 500ms)
   - `post_tool_use.sh` (timeout: 3000ms)
   - `user_prompt_submit.sh` (timeout: 2000ms)
   - `session_end.sh` (timeout: 30000ms)
4. Sets scripts to `chmod 755` on Unix.

**Guard**: If `.claude/settings.json` already contains a `"hooks"` key and `--force` is not set, the command exits with error code 1.

**Generated `settings.json` hook structure**:

```json
{
  "hooks": {
    "SessionStart": [{ "hooks": [{ "type": "command", "command": ".claude/hooks/session_start.sh", "timeout": 5000 }] }],
    "SessionEnd":   [{ "hooks": [{ "type": "command", "command": ".claude/hooks/session_end.sh", "timeout": 30000 }] }],
    "PreToolUse":   [{ "matcher": ".*", "hooks": [{ "type": "command", "command": ".claude/hooks/pre_tool_use.sh", "timeout": 500 }] }],
    "PostToolUse":  [{ "matcher": ".*", "hooks": [{ "type": "command", "command": ".claude/hooks/post_tool_use.sh", "timeout": 3000 }] }],
    "UserPromptSubmit": [{ "hooks": [{ "type": "command", "command": ".claude/hooks/user_prompt_submit.sh", "timeout": 2000 }] }]
  }
}
```

**Generated shell scripts**: Each script locates the `context-graph-cli` binary (checking `$CONTEXT_GRAPH_CLI` env var, then `./target/release/`, `./target/debug/`, and `~/.cargo/bin/`), requires `jq` for JSON parsing, constructs a `HookInput` JSON, and pipes it to the appropriate CLI subcommand via `--stdin`.

---

## 7. Warmup Command

Pre-loads all 13 embedding models into GPU VRAM before the MCP server starts.

```
context-graph-cli warmup [OPTIONS]
```

| Flag | Required | Default | Description |
|------|----------|---------|-------------|
| `--skip-if-warm <BOOL>` | No | `true` | Skip warmup if models are already warm |
| `--timeout-secs <SECS>` | No | `300` | Timeout in seconds (0 = no timeout) |

**Behavior**: Calls `initialize_global_warm_provider()` to load all 13 embedding models into VRAM. Takes approximately 20-30 seconds on RTX 5090 (32GB VRAM). If models are already warm, returns immediately with exit 0.

**Exit codes**:

| Code | Description |
|------|-------------|
| 0 | Success (models are warm, including "already warm") |
| 1 | Failed to initialize models (CUDA/GPU error, timeout) |

**Prerequisites**: CUDA 13.1+, RTX 5090 or compatible GPU, models downloaded at `CONTEXT_GRAPH_MODELS_PATH`.

---

## 8. Watch Command

Watches a directory for markdown file changes and automatically chunks and stores them as memories.

```
context-graph-cli watch --path <DIR> [OPTIONS]
```

| Flag | Short | Required | Default | Description |
|------|-------|----------|---------|-------------|
| `--path <DIR>` | `-p` | **Yes** | -- | Directory path to watch for `.md` files |
| `--session-id <ID>` | `-s` | No | `file-watcher-session` | Session ID for captured memories |
| `--db-path <PATH>` | -- | No | `CONTEXT_GRAPH_DATA_DIR` env var, then `./contextgraph_data` | Database path for storing memories |

**Behavior**:

1. Validates the path exists and is a directory.
2. Initializes GPU embedding models if not already warm (may take 20-30 seconds).
3. Creates a `MemoryStore` in `{db_path}/watcher_memory/`.
4. Opens a `TeleologicalMemoryStore` at `{db_path}` for MCP search integration (graceful fallback to local-only storage if unavailable).
5. Starts `GitFileWatcher` monitoring the directory.
6. Processes file change events every 500ms.
7. On Ctrl+C, stops the watcher and prints final statistics.

**File processing**: New/modified `.md` files are chunked using `TextChunker` (200 words, 50-word overlap). Each chunk is stored with `MDFileChunk` source metadata including file path and chunk index.

**Exit codes**:

| Code | Description |
|------|-------------|
| 0 | Clean shutdown |
| 1 | Error (path doesn't exist, GPU init failed, watcher failed) |

---

## 9. Topic Commands

Topic commands explore emergent topic clusters and stability metrics. Topics emerge from weighted multi-space clustering (threshold: `weighted_agreement >= 2.5`).

### 9.1 `topic portfolio`

Gets discovered topics with profiles and stability metrics.

```
context-graph-cli topic portfolio [OPTIONS]
```

| Flag | Short | Required | Default | Description |
|------|-------|----------|---------|-------------|
| `--format <FORMAT>` | `-f` | No | `standard` | Output format: `brief` (names + confidence only), `standard` (includes contributing embedding spaces), `verbose` (full profiles with all 13 embedder strengths) |
| `--json` | -- | No | `false` | Output as JSON instead of human-readable |

### 9.2 `topic stability`

Gets portfolio-level stability metrics.

```
context-graph-cli topic stability [OPTIONS]
```

| Flag | Short | Required | Default | Description |
|------|-------|----------|---------|-------------|
| `--hours <N>` | `-H` | No | `6` | Lookback period in hours (valid range: 1-168) |
| `--json` | -- | No | `false` | Output as JSON instead of human-readable |

**Validation**: `--hours` must be between 1 and 168. Values of 0 or >168 return exit 1.

**Output indicators**:

| Metric | Healthy | Warning | Unstable/High |
|--------|---------|---------|---------------|
| Churn Rate | < 0.3 | < 0.5 | >= 0.5 |
| Entropy | < 0.5 (Low) | < 0.7 (Moderate) | >= 0.7 (High) |

Dream consolidation is recommended when entropy > 0.7 AND churn > 0.5.

### 9.3 `topic detect`

Forces topic detection recalculation.

```
context-graph-cli topic detect [OPTIONS]
```

| Flag | Short | Required | Default | Description |
|------|-------|----------|---------|-------------|
| `--force` | `-f` | No | `false` | Force detection even if recently computed |
| `--json` | -- | No | `false` | Output as JSON instead of human-readable |

**Requirements**: Minimum 3 memories in the graph (per `clustering.parameters.min_cluster_size`). Topics require `weighted_agreement >= 2.5` to be recognized.

---

## 10. Divergence Commands

Divergence commands detect when current work diverges from recent activity patterns. Uses **SEMANTIC embedders only** (E1, E5, E6, E7, E10, E12, E13). Temporal embedders (E2-E4) are excluded per AP-62, AP-63.

### 10.1 `divergence check`

Checks for divergence from recent activity.

```
context-graph-cli divergence check [OPTIONS]
```

| Flag | Short | Required | Default | Description |
|------|-------|----------|---------|-------------|
| `--hours <N>` | `-H` | No | `2` | Lookback period in hours (valid range: 1-48) |
| `--json` | -- | No | `false` | Output as JSON instead of human-readable |
| `--verbose` | `-v` | No | `false` | Show detailed embedder-level divergence scores |

**Validation**: `--hours` must be between 1 and 48. Values of 0 or >48 return exit 1.

**Human-readable output**:
- **STATUS: ALIGNED** -- Current activity aligns with recent patterns.
- **STATUS: DIVERGENT** -- Current activity differs from recent patterns. Alerts are shown with severity indicators: `!!` (high/critical), `!` (medium/warning), `-` (info/low).

**Verbose mode**: Shows per-embedder divergence scores with visual bar chart.

---

## 11. MCP Client Integration

The CLI communicates with the running MCP server via TCP using JSON-RPC 2.0. The MCP client is defined in `mcp_client.rs` and used by memory, topic, divergence, and hook commands.

### 11.1 Connection Configuration

| Parameter | Env Variable | Default |
|-----------|-------------|---------|
| Host | `CONTEXT_GRAPH_MCP_HOST` | `127.0.0.1` |
| Port | `CONTEXT_GRAPH_MCP_PORT` | `3100` |

### 11.2 Timeout Configuration

| Path | Connection Timeout | Request Timeout |
|------|-------------------|-----------------|
| Normal | 5000ms | 30000ms |
| Fast (time-critical hooks) | 500ms | 800ms |

**Response size limit**: 10 MB per response line (bounded read to prevent OOM).

### 11.3 Protocol

The client sends JSON-RPC 2.0 requests over a TCP stream using newline-delimited JSON:

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"store_memory","arguments":{...}}}
```

Responses follow MCP content wrapper format:

```json
{"id":1,"result":{"content":[{"text":"{...actual JSON...}","type":"text"}],"isError":false}}
```

The client automatically unwraps the `content[0].text` field and parses it as JSON.

### 11.4 MCP Tool Methods

The client exposes methods for the following MCP tools:

| Client Method | MCP Tool | Timeout Path | Description |
|--------------|----------|-------------|-------------|
| `store_memory()` | `store_memory` | Normal | Store memory with content, importance, modality, tags, session_id |
| `inject_context()` | `inject_context` | Normal | Store context with rationale and tags |
| `search_graph()` | `search_graph` | Normal | Semantic search with top_k limit |
| `search_graph_with_content()` | `search_graph` | Normal | Search with optional content hydration |
| `search_graph_fast()` | `search_graph` | Fast | Time-critical search with strategy selection |
| `search_causal_fast()` | `search_causes` | Fast | E5 causal search with direction |
| `get_divergence_alerts()` | `get_divergence_alerts` | Normal | Divergence detection |
| `get_divergence_alerts_fast()` | `get_divergence_alerts` | Fast | Time-critical divergence check |
| `get_conversation_context_fast()` | `get_conversation_context` | Fast | Recent session turns with position labels |
| `get_memetic_status()` | `get_memetic_status` | Normal | System status with UTL metrics |
| `get_topic_portfolio()` | `get_topic_portfolio` | Normal | Topic portfolio with format option |
| `get_topic_stability()` | `get_topic_stability` | Normal | Portfolio stability metrics |
| `detect_topics()` | `detect_topics` | Normal | Force topic detection |

### 11.5 MCP Client Error Types

| Error Variant | Description |
|--------------|-------------|
| `ServerNotRunning` | MCP server not running or unreachable (host, port, source IO error) |
| `ConnectionTimeout` | Connection timeout exceeded (host, port, timeout_ms) |
| `RequestTimeout` | Request timeout exceeded (timeout_ms) |
| `McpError` | MCP server returned an error (code, message) |
| `IoError` | IO error during communication |
| `JsonError` | JSON serialization/deserialization error |
| `NoResult` | Response missing expected result |

### 11.6 Session ID Resolution

When a command needs a session ID, resolution follows this priority:

1. CLI argument (`--session-id`)
2. `CLAUDE_SESSION_ID` environment variable
3. Default: `"default"`

Whitespace-only values are treated as absent.

### 11.7 Server Health Check

The `require_mcp_server()` helper (in `mcp_helpers.rs`) checks server availability before executing commands. If the server is not running, it prints an error to stderr and returns exit code 1 (Warning).

---

## 12. Exit Codes and Error Handling

### 12.1 CLI Exit Codes (AP-26)

| Code | Name | Description | Claude Code Behavior |
|------|------|-------------|---------------------|
| 0 | Success | Command completed successfully | stdout captured |
| 1 | Warning | Recoverable error (server down, timeout, invalid args) | stderr shown to user, does **not** block |
| 2 | Blocking | Data corruption detected | stderr shown to Claude, **blocks** action |

**Exit code 2 is reserved exclusively for corruption scenarios** where proceeding would be dangerous.

### 12.2 Hook-Specific Exit Codes

Hook commands use an extended set:

| Code | Name | Description |
|------|------|-------------|
| 0 | Success | Hook executed successfully |
| 1 | General Error | Unspecified error |
| 4 | Invalid Input | Malformed input data (bad JSON, missing required fields) |

### 12.3 StorageError Classification

`StorageError` variants from `context_graph_storage` are mapped to exit codes:

| StorageError Variant | Exit Code | Rationale |
|---------------------|-----------|-----------|
| `IndexCorrupted` | 2 (Blocking) | Data corruption |
| `Serialization` (with corruption indicator) | 2 (Blocking) | Data corruption in serialized data |
| `ReadFailed` (with corruption indicator) | 2 (Blocking) | Corruption during read |
| `NotFound` | 0 (Success) | Fresh install is valid |
| `OpenFailed` | 1 (Warning) | Recoverable |
| `ColumnFamilyNotFound` | 1 (Warning) | Recoverable |
| `WriteFailed` | 1 (Warning) | Recoverable |
| `ReadFailed` (normal) | 1 (Warning) | Recoverable |
| `FlushFailed` | 1 (Warning) | Recoverable |
| `Serialization` (normal) | 1 (Warning) | Recoverable |
| `ValidationFailed` | 1 (Warning) | Recoverable |
| `Internal` | 1 (Warning) | Recoverable |

### 12.4 Corruption Indicators

The following strings (case-insensitive) in error messages trigger exit code 2 (Blocking):

- `corruption` / `corrupted`
- `stale index`
- `checksum mismatch`
- `bad magic`
- `crc error`
- `truncated block`
- `bad table magic number`
- `block checksum`

The following are explicitly **NOT** corruption indicators (per H4/M9 fixes): `invalid`, `malformed`, `not found`, `truncated file`.

### 12.5 MCP Error to Exit Code Mapping

| MCP Error | Exit Code |
|-----------|-----------|
| Server not running | 1 (Warning) |
| Connection timeout | 1 (Warning) |
| Request timeout | 1 (Warning) |
| MCP error (with corruption indicator) | 2 (Blocking) |
| MCP error (normal) | 1 (Warning) |
| IO error | 1 (Warning) |
| JSON error | 1 (Warning) |
| No result | 1 (Warning) |

---

## 13. Filesystem Memory Cache

The filesystem-based memory cache enables cross-process context sharing between hook invocations. Since each Claude Code hook invocation spawns a separate CLI process, in-memory caches are not shared.

### 13.1 Cache Location

```
/tmp/cg-memory-cache-{session_id}.json
```

Within the directory `/tmp/cg-memory-cache/`. Session IDs are sanitized to only allow alphanumeric characters, hyphens, and underscores (max 128 characters) to prevent path traversal.

### 13.2 Cache Lifecycle

| Operation | Hook | Description |
|-----------|------|-------------|
| Write | `prompt-submit` (user_prompt_submit) | Stores retrieved memories after MCP search |
| Read | `pre-tool` (pre_tool_use), `inject-brief` | Reads cached memories for fast context injection |
| Delete | `session-end` (session_end) | Cleans up cache files for the session |

### 13.3 Cache Properties

| Property | Value |
|----------|-------|
| TTL | 300 seconds (5 minutes) |
| Max memories per session | 10 |
| Write strategy | Atomic (write to `.tmp` then rename) |
| Expired cache behavior | Silently removed on read |
| Missing cache behavior | Returns empty vec (not an error) |

---

## 14. Session State Management

### 14.1 SessionCache

The `SessionCache` is a global `Mutex<Option<SessionSnapshot>>` singleton. It is **process-scoped** and does NOT persist across separate CLI invocations (per MED-24, PRD v6 Section 14).

**Limitation**: Each hook invocation is a separate process. `SessionCache::get()` returns `None` in most hook invocations (cold start). The cache is primarily useful within the long-running MCP server process.

### 14.2 SessionSnapshot Fields

| Field | Type | Description |
|-------|------|-------------|
| `session_id` | `String` | Session identifier |
| `topic_profile` | `[f32; 13]` | Topic profile across 13 embedder spaces |
| `integration` | `f32` | Integration metric [0.0, 1.0], default 0.5 |
| `reflection` | `f32` | Reflection metric [0.0, 1.0], default 0.5 |
| `differentiation` | `f32` | Differentiation metric [0.0, 1.0], default 0.5 |
| `trajectory` | `Vec<[f32; 13]>` | Trajectory of previous topic profiles (max 100) |
| `previous_session_id` | `Option<String>` | Link to previous session |
| `timestamp_ms` | `u64` | Timestamp in milliseconds |

---

## 15. Environment Variables Reference

| Variable | Used By | Default | Description |
|----------|---------|---------|-------------|
| `CONTEXT_GRAPH_MCP_HOST` | All MCP commands | `127.0.0.1` | MCP server hostname |
| `CONTEXT_GRAPH_MCP_PORT` | All MCP commands | `3100` | MCP server TCP port |
| `CONTEXT_GRAPH_DB_PATH` | Session commands | `~/.context-graph/db` | RocksDB database path |
| `CONTEXT_GRAPH_DATA_DIR` | Watch command | `./contextgraph_data` | Data directory for storage |
| `CONTEXT_GRAPH_CLI` | Hook shell scripts | `context-graph-cli` | CLI binary path |
| `CLAUDE_SESSION_ID` | Memory/hook commands | `"default"` | Session identifier |
| `USER_PROMPT` | `inject-context` | -- | Query text for context retrieval |
| `TOOL_DESCRIPTION` | `inject-brief`, `capture-memory` | -- | Tool description for query/content |
| `TOOL_NAME` | `inject-brief` | -- | Fallback query for brief injection |
| `SESSION_SUMMARY` | `capture-memory` | -- | Fallback content for memory capture |
| `RESPONSE_SUMMARY` | `capture-response` | -- | Content for response capture |
| `RUST_LOG` | Logging | `warn` | Log level filter (overridden by `-v` flags) |
| `CONTEXT_GRAPH_MODELS_PATH` | Warmup | -- | Path to embedding model files |

---

## 16. Quick Start

```bash
# 1. Setup hooks in your project
context-graph-cli setup

# 2. Pre-warm embedding models (20-30s on RTX 5090)
context-graph-cli warmup

# 3. Start the MCP server (separate terminal)
context-graph-mcp

# 4. Start a Claude Code session (hooks are now active)

# 5. (Optional) Watch docs for automatic memory capture
context-graph-cli watch --path ./docs --session-id my-session

# 6. (Optional) Check topic portfolio
context-graph-cli topic portfolio

# 7. (Optional) Check for divergence
context-graph-cli divergence check --hours 4 --verbose
```

# CLI Reference

Source files covered:

| File | Role |
|------|------|
| `crates/context-graph-cli/src/main.rs` | Clap top-level dispatch, tracing setup |
| `crates/context-graph-cli/src/lib.rs` | Library re-exports (`error`, `mcp_client`, `mcp_helpers`, `commands`) |
| `crates/context-graph-cli/src/error.rs` | `CliExitCode` (0/1/2) per AP-26, `CORRUPTION_INDICATORS` |
| `crates/context-graph-cli/src/mcp_client.rs` | `McpClient` — JSON-RPC 2.0 over TCP to daemon (default `127.0.0.1:3100`) |
| `crates/context-graph-cli/src/mcp_helpers.rs` | Shared helpers: `resolve_session_id`, `mcp_error_to_exit_code`, `DEFAULT_SESSION_ID = "default"` |
| `crates/context-graph-cli/src/commands/mod.rs` | Command module registry |
| `crates/context-graph-cli/src/commands/session/{mod,persist,restore}.rs` | `session restore-identity`, `persist-identity` — SessionCache wrapper |
| `crates/context-graph-cli/src/commands/hooks/` | Hook subcommands + embedded shell script templates |
| `crates/context-graph-cli/src/commands/memory/{mod,capture,inject}.rs` | Memory capture + context injection via MCP |
| `crates/context-graph-cli/src/commands/topic.rs` | Topic portfolio/stability/detect via MCP |
| `crates/context-graph-cli/src/commands/divergence.rs` | Divergence alert retrieval via MCP |
| `crates/context-graph-cli/src/commands/setup.rs` | Creates `.claude/settings.json` + `.claude/hooks/*.sh` |
| `crates/context-graph-cli/src/commands/warmup.rs` | Pre-load 13 embedders into VRAM |
| `crates/context-graph-cli/src/commands/watch.rs` | Directory watcher ingesting `.md` files |
| `crates/context-graph-cli/src/commands/export_training.rs` | `export training-corpus` → Parquet |

Cross references: [05_embedding_subsystem.md](05_embedding_subsystem.md), [06_search_and_fusion.md](06_search_and_fusion.md), [08_mcp_tools_reference.md](08_mcp_tools_reference.md), [11_cuda_acceleration.md](11_cuda_acceleration.md), [12_clustering_and_topics.md](12_clustering_and_topics.md).

---

## 1. Binary

**Name**: `context-graph-cli` · **Version**: `0.1.0` · **Entry**: `crates/context-graph-cli/src/main.rs`

```
context-graph-cli [--verbose]... <COMMAND> [ARGS]
```

### 1.1 Global Options

| Flag | Short | Description |
|------|-------|-------------|
| `--verbose` | `-v` | Repeatable. `-v` → info, `-vv` → debug, `-vvv`+ → trace. Default respects `RUST_LOG`, falls back to `warn`. |
| `--version` | `-V` | Print version. |
| `--help` | `-h` | Print help. |

All tracing output goes to **stderr** (so JSON hook responses on stdout stay clean).

### 1.2 Exit Codes (`CliExitCode`, AP-26)

| Code | Variant | Meaning |
|------|---------|---------|
| 0 | `Success` | OK; stdout is Claude-visible. |
| 1 | `Warning` | Recoverable — stderr shown to user, hook does not block. |
| 2 | `Blocking` | Corruption only — stderr shown to Claude, blocks action. |

`CliExitCode::from(&StorageError)` mapping: `IndexCorrupted` / `Serialization` / `ReadFailed` with corruption indicator → 2. `NotFound` → 0. All others → 1.

**`CORRUPTION_INDICATORS`** (lowercase substring match, `error.rs:91`): `corruption`, `corrupted`, `stale index`, `checksum mismatch`, `bad magic`, `crc error`, `truncated block`, `bad table magic number`, `block checksum`.

Hook handlers additionally expose codes `2`=timeout, `3`=db_error, `4`=invalid_input in their JSON output (see `commands/hooks/error.rs`).

---

## 2. Top-Level Command Map

| Command | Subcommand Count | Source |
|---------|------------------|--------|
| `session` | 2 | `commands/session/mod.rs` |
| `hooks` | 8 | `commands/hooks/args.rs :: HooksCommands` |
| `memory` | 4 | `commands/memory/mod.rs` |
| `topic` | 3 | `commands/topic.rs` |
| `divergence` | 1 | `commands/divergence.rs` |
| `setup` | — (flat) | `commands/setup.rs` |
| `warmup` | — (flat) | `commands/warmup.rs` |
| `watch` | — (flat) | `commands/watch.rs` |
| `export` | 1 | `commands/export_training.rs` |

---

## 3. `session` — Session Persistence

Uses the **in-memory** `SessionCache` (`commands/hooks/session_state.rs`); there is no direct RocksDB write path per PRD v6 §14. Persistence flows through the daemon's topic stability tracker.

### 3.1 `session restore-identity`

Restores a previous identity from cache (or latest if none specified), emitting a PRD-formatted block on stdout.

```
context-graph-cli session restore-identity [--db-path PATH] [--format prd|json]
```

| Arg | Env | Default | Description |
|-----|-----|---------|-------------|
| `--db-path` | `CONTEXT_GRAPH_DB_PATH` | — | RocksDB directory (kept for forward compat; unused in current handler). |
| `--format` | — | `prd` | `prd` = `## Session State\n- State: EMG (C=0.82)\n- Session: ...`; `json` for programmatic use. |
| stdin | — | `{}` | Optional JSON: `{ "session_id": "...", "source": "startup\|resume\|clear" }`. |

### 3.2 `session persist-identity`

Persists the current `SessionCache` snapshot. **Silent on success** (SessionEnd semantics require no stdout).

```
context-graph-cli session persist-identity [--db-path PATH]
```

| Arg | Env | Default | Description |
|-----|-----|---------|-------------|
| `--db-path` | `CONTEXT_GRAPH_DB_PATH` | — | RocksDB directory (unused in handler). |
| stdin | — | `{}` | `{ "session_id": "...", "reason": "exit\|clear\|logout\|prompt_input_exit\|other" }`. |

---

## 4. `hooks` — Claude Code Native Integration

Every subcommand accepts `--stdin` to read a `HookInput` JSON body (`types::HookEventType` discriminant). Timeouts come from `constitution.yaml`; the wrapper shell scripts enforce them with `timeout`. All produce a `HookOutput` JSON document on stdout.

### 4.1 Subcommand Timings and Flags

| Subcommand | Clap name | Budget | Required | Key flags | DB access |
|------------|-----------|--------|----------|-----------|-----------|
| `session-start` | `SessionStart` | 5000ms | — | `--session-id`, `--previous-session-id`, `--stdin`, `--format` | Via MCP |
| `pre-tool` | `PreTool` | 500ms | `--session-id` | `--tool-name`, `--fast-path` (default `true`, MUST stay true to meet budget), `--stdin`, `--format` | **None** (cache-only) |
| `post-tool` | `PostTool` | 3000ms | `--session-id` | `--tool-name`, `--success`, `--stdin`, `--format` | Via MCP |
| `prompt-submit` | `PromptSubmit` | 2000ms | `--session-id` | `--prompt`, `--stdin`, `--format` | Via MCP (fast-path) |
| `session-end` | `SessionEnd` | 30000ms | `--session-id` | `--duration-ms`, `--generate-summary` (default `true`), `--stdin`, `--format` | Via MCP |
| `pre-compact` | `PreCompact` | 20000ms | `--session-id` | `--stdin`, `--format` | Via MCP |
| `task-completed` | `TaskCompleted` | 20000ms | `--session-id` | `--stdin`, `--format` | Via MCP |
| `generate-config` | `GenerateConfig` | — | — | `--output-dir` (default `.claude/hooks`), `--force`, `--hooks` (csv), `--shell` | — (deprecated; prints error, use `setup`) |

### 4.2 Shared Enums

**`OutputFormat`** (`--format`): `json` (default, pretty), `json-compact`, `text`.

**`HookType`** (for `--hooks` in `generate-config`): `session-start`, `pre-tool-use`, `post-tool-use`, `user-prompt-submit`, `session-end`.

**`ShellType`** (for `--shell`): `bash` (default), `zsh`, `fish`, `powershell`.

### 4.3 `HookInput` Payload Shape (`commands/hooks/types.rs`)

```jsonc
{
  "hook_type": "session_start|pre_tool_use|post_tool_use|user_prompt_submit|session_end|pre_compact|task_completed",
  "session_id": "session-...",
  "timestamp_ms": 1713000000000,
  "payload": {
    "type": "<same as hook_type>",
    "data": { /* event-specific fields */ }
  }
}
```

Per-event `data` fields (`commands/hooks/types.rs`): `session_start.data = { cwd, source, previous_session_id? }`; `pre_tool_use.data = { tool_name, tool_input }`; `post_tool_use.data = { tool_name, tool_result, success }`; `user_prompt_submit.data = { prompt, context }`; `session_end.data = { stats }`.

### 4.4 `HookOutput` Shape

```jsonc
{
  "success": true,
  "stability_level": "stable|drifting|divergent|corrupt",
  "coherence_state": { /* ... */ },
  "drift_metrics": { /* ... */ },
  "extra": { /* event-specific */ }
}
```

### 4.5 Example

```bash
# Synthesized by .claude/hooks/pre_tool_use.sh (see §8.2)
echo "$INPUT" | timeout 0.5s context-graph-cli hooks pre-tool \
    --session-id "$SESSION_ID" --tool-name "$TOOL_NAME" \
    --fast-path true --format json
```

Handler returns exit code 0 + JSON on stdout on success. The shell wrapper translates exit 124 (GNU `timeout`) into hook `exit_code: 2`.

---

## 5. `memory` — Capture and Context Injection

All four subcommands talk to the daemon via `McpClient`. They translate `McpClientError` → exit code via `mcp_helpers::mcp_error_to_exit_code` (most → 1, corruption-like strings → 2).

### 5.1 `memory inject-context`

Embeds a query through the warm provider, retrieves similar memories via `search_graph`, and emits **markdown** sized to a token budget. Used by `UserPromptSubmit` and `SessionStart` hooks.

```
context-graph-cli memory inject-context [--query <Q>] [--budget N] [--top-k N] [--session-id S] [<QUERY>]
```

| Arg | Default | Description |
|-----|---------|-------------|
| positional `<QUERY>` | — | Overrides `--query`. |
| `--query` | — | Query text. |
| `--budget` | `DEFAULT_CONTEXT_BUDGET` | Max tokens emitted on stdout. |
| `--top-k` | `10` | Max memories fetched. |
| env `USER_PROMPT` | — | Fallback query source. |
| env `CLAUDE_SESSION_ID` | — | Session ID fallback (`resolve_session_id`). |

### 5.2 `memory inject-brief`

Compact (<200 token) context for PreToolUse. **No** divergence alerts.

```
context-graph-cli memory inject-brief [--top-k N] [<QUERY>]
```

| Arg | Default | Description |
|-----|---------|-------------|
| positional `<QUERY>` | — | Override. |
| `--top-k` | `5` | Results fetched. |
| env `TOOL_DESCRIPTION` | — | Primary query source. |
| env `TOOL_NAME` | — | Fallback. |

### 5.3 `memory capture-memory`

Stores a `HookDescription` memory. **Silent on success.**

```
context-graph-cli memory capture-memory --hook-type <T> [--content C] [--tool-name T] [--source S] [--importance F]
```

| Arg | Default | Description |
|-----|---------|-------------|
| `--hook-type` | required | e.g. `post_tool_use`, `session_end`. |
| `--content` | — | Primary content. |
| `--source` | `hook` | Source tag. |
| `--tool-name` | — | Optional. |
| `--importance` | `0.5` | Clamped to [0,1] server-side. |
| env `TOOL_DESCRIPTION` | — | Content fallback. |
| env `SESSION_SUMMARY` | — | Second fallback. |
| env `CLAUDE_SESSION_ID` | — | Session. |
| env `CONTEXT_GRAPH_DATA_DIR` | — | DB path (passed through to MCP). |

### 5.4 `memory capture-response`

Stores a `ClaudeResponse` memory. Silent on success. Flags mirror `capture-memory` with `--source` default `stop_response` and no `--hook-type`. Env var `RESPONSE_SUMMARY` is the fallback content source.

---

## 6. `topic` — Topic Portfolio Commands

All call MCP tools and abort with exit 1 if `McpClient::is_server_running()` returns `Ok(false)`. See [12_clustering_and_topics.md](12_clustering_and_topics.md) for the underlying model.

| Subcommand | MCP tool | Flags | Range checks |
|------------|----------|-------|--------------|
| `topic portfolio` | `get_topic_portfolio` | `-f/--format brief\|standard\|verbose` (default `standard`), `--json` | — |
| `topic stability` | `get_topic_stability` | `-H/--hours N` (default `6`), `--json` | `hours ∈ [1, 168]` else exit 1 |
| `topic detect` | `detect_topics` | `-f/--force`, `--json` | — |

### 6.1 Human Format Notes

`portfolio` printer: renders `Topic: <name>`, `Confidence`, `Weighted Agreement`, and optionally `Embedder Strengths` (verbose). Empty portfolio: `No topics discovered yet. Tip: Topics emerge when weighted_agreement >= 2.5`.

`stability` printer: churn labeled `Healthy` (<0.3) / `Warning` (<0.5) / `Unstable`; entropy labeled `Low` (<0.5) / `Moderate` (<0.7) / `High`. Emits `! High entropy/churn detected` when `dream_recommended=true` (entropy > 0.7 AND churn > 0.5).

---

## 7. `divergence` — Activity Divergence

Single subcommand `check` calling MCP `get_divergence_alerts`. Uses **semantic embedders only** (E1, E5-E7, E10, E12, E13) — temporal embedders E2-E4 excluded per AP-62/AP-63.

```
context-graph-cli divergence check [-H|--hours N] [--json] [-v|--verbose]
```

| Arg | Default | Validation |
|-----|---------|------------|
| `--hours` | `2` | `hours ∈ [1, 48]` else exit 1 |
| `--json` | false | Raw MCP response. |
| `--verbose` | false | Adds per-embedder bar chart `#####|` scaled to 20 chars. |

Human-format statuses: `STATUS: DIVERGENT` (with severity-prefixed alerts `!!` high/critical, `!` medium/warning, `-` info) or `STATUS: ALIGNED`.

---

## 8. `setup` — Hook Installer

Writes `.claude/settings.json` + `.claude/hooks/*.sh` into the target dir. The shell scripts are **embedded constants** in `commands/setup.rs` (`SESSION_START_SCRIPT`, `PRE_TOOL_USE_SCRIPT`, `POST_TOOL_USE_SCRIPT`, `USER_PROMPT_SUBMIT_SCRIPT`, `SESSION_END_SCRIPT`).

```
context-graph-cli setup [--force|-f] [--target-dir PATH] [--skip-chmod]
```

| Arg | Default | Description |
|-----|---------|-------------|
| `--force` / `-f` | false | Overwrite existing hooks block in `settings.json` (other keys preserved via merge). |
| `--target-dir` | `cwd` | Where to write `.claude/…`. Must exist. |
| `--skip-chmod` | false (hidden) | Skip `chmod 0o755` on scripts (tests on non-Unix). |

### 8.1 Generated `settings.json` hook registrations

| Event | Script | Timeout | Matcher |
|-------|--------|---------|---------|
| `SessionStart` | `.claude/hooks/session_start.sh` | 5000 | — |
| `SessionEnd` | `.claude/hooks/session_end.sh` | 30000 | — |
| `PreToolUse` | `.claude/hooks/pre_tool_use.sh` | 500 | `.*` |
| `PostToolUse` | `.claude/hooks/post_tool_use.sh` | 3000 | `.*` |
| `UserPromptSubmit` | `.claude/hooks/user_prompt_submit.sh` | 2000 | — |

### 8.2 Generated Script Contract

Each script:
1. `set -euo pipefail`; require `jq`.
2. Read stdin JSON; validate with `jq empty`; empty → exit 4.
3. Locate CLI: env `CONTEXT_GRAPH_CLI` > `./target/release/context-graph-cli` > `./target/debug/...` > `$HOME/.cargo/bin/...`. Missing → exit 1.
4. Build a `HookInput` via `jq -n` (so special chars in `$CWD` / prompt survive).
5. Pipe to `timeout <Ns> "$CONTEXT_GRAPH_CLI" hooks <sub> ...`.
6. Map exit 124 → JSON `{"error":"Timeout after Nms","exit_code":2}` and exit 2.

Existing `settings.json` handling: `--force` absent + `hooks` key present → exit 1. Otherwise the file is parsed, the `hooks` object is merged in (overwriting only that key), and the rest is preserved.

### 8.3 Exit Codes

| Code | Reason |
|------|--------|
| 0 | Files written. |
| 1 | Target missing, parse failure, hooks already present without `--force`, write/chmod error. |

---

## 9. `warmup` — VRAM Pre-load

Idempotent. Loads all 13 embedders into GPU VRAM via `context_graph_embeddings::initialize_global_warm_provider` so the first MCP request is instant. See [05_embedding_subsystem.md](05_embedding_subsystem.md) for the warm-provider architecture.

```
context-graph-cli warmup [--skip-if-warm BOOL] [--timeout-secs N]
```

| Arg | Default | Description |
|-----|---------|-------------|
| `--skip-if-warm` | `true` | Short-circuit if `is_warm_initialized()`. |
| `--timeout-secs` | `300` | 0 = no timeout. |

Exit codes: `0` success (including "already warm"), `1` init failure / timeout / CUDA missing.

Typical sequence:
```bash
context-graph-cli warmup          # ~20-30s on RTX 5090
context-graph-mcp                 # ready instantly
```

---

## 10. `watch` — Markdown Directory Watcher

Starts `GitFileWatcher` on a directory, chunks `.md` files via `TextChunker` (200 words, 50-word overlap), and ingests chunks into both `MemoryStore` (local) and `TeleologicalStore` (MCP-searchable) with `SourceMetadata::MDFileChunk { file_path, chunk_index }`. Runs until SIGINT.

```
context-graph-cli watch -p|--path DIR [-s|--session-id SID] [--db-path PATH]
```

| Arg | Env | Default | Description |
|-----|-----|---------|-------------|
| `-p/--path` | — | required | Directory to watch. Must be a dir. |
| `-s/--session-id` | — | `file-watcher-session` | Session tag applied to captured chunks. |
| `--db-path` | `CONTEXT_GRAPH_DATA_DIR` | `./contextgraph_data` | RocksDB root; watcher uses `<root>/watcher_memory` for local store and `<root>` for the shared teleological store. |

Exit codes: `0` on clean shutdown; `1` if path missing, warm provider init fails, store creation fails, or watcher start fails.

---

## 11. `export training-corpus`

Streams every row of `CF_TRAINING_RECORDS` from a RocksDB directory into a Parquet file using schema `(memory_id: Utf8, record_bytes: Binary)`. Bytes are produced by the production `encode_training_record` helper — byte-for-byte identical to what RocksDB stores.

```
context-graph-cli export training-corpus \
    --out FILE.parquet \
    --storage DIR \
    [--kind training-corpus] [--format parquet] \
    [--batch-size N] [--limit N]
```

| Arg | Env | Default | Description |
|-----|-----|---------|-------------|
| `--out` | — | required | Parquet file path. Parent dir must exist. |
| `--storage` | `CONTEXT_GRAPH_STORAGE_PATH` | required | RocksDB data directory (must exist). |
| `--kind` | — | `training-corpus` | Only this value is accepted. |
| `--format` | — | `parquet` | Only this value is accepted. |
| `--batch-size` | — | `500` | One Parquet row group per batch. |
| `--limit` | — | none | Cap on total records. |

Writer properties: `ArrowWriter` with Zstd compression; one row group per batch so downstream consumers can assert the batch→row-group contract. Decoded payloads skip orphan UUIDs (warn only). On any error the partial file is removed.

Stdout on success (parsed by CI): `{"status":"ok","total_records":N,"bytes_written":N,"row_groups":N,"elapsed_ms":N}`.

Exit codes: `0` success, `1` any failure (stderr: `error: <chain>`).

Decoding: Rust → `context_graph_storage::teleological::decode_training_record`; Python → read the version byte (`TRAINING_RECORD_VERSION`) and bincode payload.

---

## 12. MCP Client (`mcp_client.rs`)

Transport: **JSON-RPC 2.0 over a single TCP line** per call (write request + `\n`, read response line).

### 12.1 Connection

| Setting | Env | Default |
|---------|-----|---------|
| Host | `CONTEXT_GRAPH_MCP_HOST` | `127.0.0.1` |
| Port | `CONTEXT_GRAPH_MCP_PORT` | `3100` |
| Connection timeout | — | 5000ms |
| Request timeout | — | 30000ms |
| Fast-path connection timeout | — | 500ms |
| Fast-path request timeout | — | 800ms |
| Response size cap | — | 10 MiB (bounded read; over-limit drains to next newline then errors) |

`McpClient::is_server_running()` does a 1s TCP probe. All `topic`/`divergence` commands gate on it.

### 12.2 Request Shape

```json
{ "jsonrpc":"2.0", "id":1, "method":"tools/call",
  "params": { "name":"<tool>", "arguments": { ... } } }
```

### 12.3 Response Unwrap

MCP wraps tool output as `{ "content": [{"text":"<json>","type":"text"}], "isError": bool }`. The client:
1. If `isError=true` → `McpClientError::McpError { code: -32000, message: content[0].text }`.
2. Else parse `content[0].text` as JSON and return.
3. If no `content` wrapper → return raw result (legacy fallback).

### 12.4 Implemented Tool Wrappers

| Method | Path | Tool | Timeout class |
|--------|------|------|---------------|
| `store_memory` | standard | `store_memory` | full |
| `inject_context` | standard | `inject_context` | full |
| `search_graph` / `search_graph_with_content` | standard | `search_graph` | full |
| `search_graph_fast` | fast | `search_graph` (+`strategy`, `minSimilarity`) | fast |
| `search_causal_fast` | fast | `search_causes` | fast |
| `get_divergence_alerts` / `_fast` | both | `get_divergence_alerts` | full / fast |
| `get_conversation_context_fast` | fast | `get_conversation_context` | fast |
| `get_memetic_status` | standard | `get_memetic_status` | full |
| `get_topic_portfolio` | standard | `get_topic_portfolio` | full |
| `get_topic_stability` | standard | `get_topic_stability` | full |
| `detect_topics` | standard | `detect_topics` | full |
| `extract_entities_fast` | fast | `extract_entities` | fast |
| `search_by_entities_fast` | fast | `search_by_entities` | fast |

### 12.5 Error Enum

`McpClientError`: `ServerNotRunning { host, port, source }`, `ConnectionTimeout`, `RequestTimeout`, `McpError { code, message }`, `IoError`, `JsonError`, `NoResult`.

---

## 13. MCP Helpers (`mcp_helpers.rs`)

| Function / Const | Purpose |
|------------------|---------|
| `DEFAULT_SESSION_ID = "default"` | Fallback when no session id is discoverable. |
| `resolve_session_id(Option<String>) -> String` | Priority: CLI arg > `CLAUDE_SESSION_ID` env > default. Trims empty strings. |
| `mcp_error_to_exit_code(&McpClientError) -> i32` | `ServerNotRunning`/`ConnectionTimeout`/`RequestTimeout`/`IoError`/`JsonError`/`McpError`/`NoResult` → `Warning` (1); corruption-indicator messages upgrade to `Blocking` (2). |

---

## 14. Shell Scripts (under `scripts/`)

Scripts invoked during install, ops, and benchmarks (one-liner each):

| Script | Role |
|--------|------|
| `ensure_daemon.sh` | Idempotently ensures `context-graph-mcp` is listening on `127.0.0.1:3100`; waits up to 5 min for bind. PID file `/tmp/context-graph-daemon.pid`, log `/tmp/context-graph-daemon.log`. |
| `context-graph-mcp-wrapper.sh` | `.mcp.json`-friendly wrapper: calls `ensure_daemon.sh`, then `exec context-graph-mcp --daemon --daemon-port 3100` (stdio bridge to daemon). |
| `disk-guard.sh` | Pre-build guard: warns at 80%, blocks at 95%. `silent` mode for shell alias. |
| `clean-build-artifacts.sh` | Prune `target/` debug + stale deps. |
| `trim-stale-deps.sh` | Keep newest compiled artifact per workspace crate in `target/*/deps/` (dry-run supported). |
| `check-embedder-count.sh` | Gate: enforces exactly 13 `ModelId` variants and matching docs. |
| `check-ffi-consolidation.sh` | FFI surface audit gate. |
| `restart_pipeline_services.sh` | Post-reboot recovery: validates Phase-B output, hardening, cleans stale PIDs, starts daemon + `vram_watchdog.py`. |
| `download_models.py` | Snapshot all HF embedder repos into `/home/cabdru/contextgraph/models/`. |
| `vram_watchdog.py` | NVML-based VRAM monitor; kills newest GPU process when VRAM > `HARD_KILL_GB` (default 29.5 GB). `--dry-run`, `--threshold-gb`, `--exit-on-kill`. |
| `run_hf_benchmark.sh` | Download HF datasets, embed via 13-embedder GPU pipeline, run retrieval benchmarks (`--synthetic`, `--download-only`, `--skip-download`). |
| `benchmark-check.sh` | Criterion regression gate; fails if any bench regresses >5% vs baseline. Exit 2 = no baseline. |
| `benchmark-disk-management.sh` | Manage Criterion baseline storage. |
| `benchmark-build-size-comparison.sh` | Compare release binary sizes across profiles. |
| `mcp_full_verify.py` | End-to-end MCP verification: ~85 JSON-RPC tests across 14 phases against a spawned daemon. |
| `upload_to_huggingface.py`, `upload_v2_1_to_huggingface.py`, `upload_v4_1_to_huggingface.py` | Push trained artifacts to HF Hub. |
| `reindex_memories.rs` | Rust helper for rebuilding HNSW indexes. |

Shakespeare training scripts (`shakespeare_*.py`) are out of scope for this reference — see [16_training_subsystem.md](16_training_subsystem.md).

---

## 15. Typical Invocation Patterns

### 15.1 First-time Claude Code setup

```bash
context-graph-cli setup                      # writes .claude/settings.json + 5 scripts
context-graph-cli warmup                     # warms 13 embedders (20-30s)
./scripts/ensure_daemon.sh                   # starts daemon
```

### 15.2 Hook invocation (auto-generated by `setup`)

```bash
# PreToolUse (500ms budget, FAST PATH)
echo "$CLAUDE_INPUT" | .claude/hooks/pre_tool_use.sh
```

### 15.3 Export training corpus to Parquet

```bash
context-graph-cli export training-corpus \
    --storage ./contextgraph_data \
    --out ./artifacts/training.parquet \
    --batch-size 500
```

### 15.4 Inspect topics interactively

```bash
context-graph-cli topic portfolio --format verbose --json
context-graph-cli topic stability --hours 24
context-graph-cli topic detect --force
context-graph-cli divergence check --hours 4 --verbose
```

# Reality Engine Responsibility — what stays in Rust after excision

After [04_EXCISION_PLAN](04_EXCISION_PLAN.md) is applied, `crates/context-graph-cli/src/bin/reality-loop.rs` is reframed as the **reality engine**: a deterministic Rust binary that runs Gemma SWE-bench attempts and emits durable artifacts. It contains no outer optimizer, no chat-completion HTTP client, no model-specific tool dispatch beyond Ollama for Gemma.

## CLI shape post-refactor

```
reality-loop attempt   --task-manifest=… --runtime-root=… [--max-attempts=N] [--official-eval] [--smoke-test] [--run-until-resolved] [--model=…] [--gpu-index=…] [--ollama-url=…] [...]
reality-loop smoke     --runtime-root=… [--label=…]
reality-loop replay    --attempt-summary-path=…
reality-loop verify    --runtime-root=… --attempt=N    # full-state verification of an attempt's artifacts
```

Notes:
- `attempt`, `smoke`, `replay`, `verify` are subcommands. `clap` derive on a top-level `enum Command`. Existing `Args` becomes `AttemptArgs`.
- The default subcommand is `attempt` to preserve the most common invocation shape, but it is no longer the implicit binary entry point — every call must declare a subcommand.
- All Gemma/Ollama/SWE-bench/governed-tool/problem-reality/reality-signal/harness-transition/BlackBox-ledger machinery stays exactly as is.

## What the engine produces (per attempt)

```
<runtime_root>/
  ledger.sqlite                                # BlackBoxFix append-only ledger
  reality-events.jsonl                         # event stream
  cgreality-mcp-events.jsonl                   # MCP server event log (written by MCP, not engine)
  failures/failure-<unix>-<error_code>.json    # any engine-level failure
  <task_id>/
    measurement-contract.json
    problem-reality-packet.json
    run-summary.json                           # rolling
    attempt-1/
      ollama-prompt.txt
      ollama-response.json
      candidate-patch.patch
      validation.json
      official-evidence/<instance>.json
      reality-signal-packet.json
      attempt-summary.json
      agent-tool-transcript.json
      harness-transitions/round-NN.json
      claude-code-optimizer/                   # written by MCP server, not engine
        recommendation-turn-NN.json
        decision-turn-NN.json
        commands/<unix>-<command>.json
    attempt-2/
      ...
```

The engine never writes anything under `claude-code-optimizer/`. That subtree is owned by the MCP layer.

## What the engine does NOT do

- Does not call any HTTP API except Ollama for Gemma + the official SWE-bench Docker bridge.
- Does not run any "optimizer trigger" decision. That decision is now made when Claude Code calls `mcp__cgreality__optimizer_record_decision`.
- Does not spawn `claude -p`. Claude Code is started by the human operator (interactive) or by an external scheduler/cron (unattended). See [14_TRIGGER_AND_INVOCATION_POLICY](14_TRIGGER_AND_INVOCATION_POLICY.md).
- Does not write the `recommendation` artifact. That is written by the MCP server when Claude Code calls `optimizer_record_recommendation`.
- Does not parse model output. That is the MCP server + structured-output schema.

## What the engine still does

- Builds `problem-reality-packet.json`.
- Runs Gemma via Ollama (with the existing governed agent tool harness — `agent_tool_specs`, the 13 tools, harness transitions, structured edit submission, line-window edit submission).
- Validates each candidate (static patch checks, py_compile, etc.).
- Runs official SWE-bench Docker eval per attempt (when `--official-eval`).
- Builds `reality-signal-packet.json` and runs the BlackBoxFix verifier on it.
- Writes `attempt-summary.json` and rolling `run-summary.json`.
- Mints `harness-transitions/round-NN.json` per agent tool call.
- Appends BlackBoxFix ledger rows.

## Active runtime root convention

A new tiny single-line file:

```
~/.cache/cgreality/active_runtime_root
```

contains the absolute path of the current `runtime_root`. The engine writes/updates this on `attempt`/`smoke` start. Every Claude Code session reads it (via SessionStart hook) to know what reality to look at.

If multiple operators run multiple loops in parallel (rare), they should each scope their `~/.cache/cgreality/` per environment via `XDG_CACHE_HOME` or per-tmux-session.

## Active target instance

Sibling file `~/.cache/cgreality/active_target_instance` contains the SWE-bench instance ID currently being targeted. Updated by the engine when a new task manifest is loaded.

## Engine ↔ MCP coupling rules

The engine and the MCP server are **decoupled** — they share files and SQLite, never sockets or pipes:

- Engine writes JSON files; MCP reads them.
- MCP writes JSON files under `claude-code-optimizer/`; engine never reads them. (The engine doesn't care what Claude Code thinks. The next attempt re-reads the harness source from disk anyway.)
- Both append to the same BlackBox ledger SQLite (via the existing `context-graph-cli blackbox` CLI for atomic writes).
- Both can read the same memory store (RocksDB) when needed.

This is the same pattern Karpathy AutoResearch uses (filesystem as agent comms substrate). It survives crashes of either side.

## Process lifecycle

- Engine: short-lived. One subcommand → one set of artifacts → exit.
- MCP server: lives for one Claude Code session. Spawns the engine via `harness_run_command` (subset) or via `reality_run_attempt` (which shells the same binary). Never embeds the engine.

## Why split this way

We want to be able to:
1. Run the engine standalone for debugging without any Claude Code involvement.
2. Run Claude Code without Docker available (analyzing past artifacts).
3. Replace either side independently. If the MCP server has a bug, the engine still runs and writes artifacts. If the engine has a bug, the MCP server still serves analyze tools against past artifacts.
4. Future-proof: if we ever want a different operator (e.g. Cursor, a custom UI, a different LLM) the MCP server is the integration; the engine doesn't change.

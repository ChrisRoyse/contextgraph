---
namespace: decisions
agent_id: agent-111-coordinator
timestamp: 2026-05-08T12:44:00Z
status: superseded
superseded_by: 2026-05-09--single-outer-loop-mejepa-no-local-llm.md
tags: mcp, reality_run_attempt, root-cause, fire-and-forget, setsid, connection-closed
---

# Decision — `reality_run_attempt` is fire-and-forget; engine detached via setsid

## Locked rule

`mcp__cgreality__reality_run_attempt` MUST return as soon as the engine has
demonstrably created its run directory under `<runtime_root>/<run_id>/`. The
engine MUST be detached from this MCP server's process group via `setsid()`
so that `/mcp reconnect`, MCP singleton-takeover, or any SIGTERM/SIGKILL
aimed at this MCP process does not cascade to the engine.

The handler MUST NOT block waiting for engine completion. It MUST NOT call
`cmd.output().await` on the engine. It MUST NOT use `kill_on_drop(true)`.

## Why

**Root cause** of the recurring `MCP error -32000: Connection closed` pattern
that has plagued every `/loop` tick since at least turn ~50: the previous
implementation called `tokio::process::Command::new(...).output().await` on
the engine binary. That call blocks the JSON-RPC handler for the **full**
1–2 hour engine runtime (model load + attempt loop + Docker eval). Claude
Code's stdio MCP transport closes the session well before then with code
-32000 — see anthropics/claude-code issues #424 (timeout configurability)
and #20713 (race condition producing -32000).

The engine SURVIVED disconnects only because:
- Claude Code's `/mcp reconnect` triggers MCP singleton-takeover (turn 39).
- The old MCP receives SIGTERM(2s)→SIGKILL.
- SIGKILL aborts the tokio runtime *before* `kill_on_drop(true)` can fire on
  the Child handle — engine becomes orphaned and is reparented to PID 1.

That accidental survival was load-bearing but fragile. A clean SIGTERM
shutdown (which `kill_on_drop` honors) would have killed the in-flight
attempt mid-run, destroying 1–2 hours of work.

## How to apply

### In the handler (`crates/context-graph-mcp/src/handlers/tools/reality_loop/interact.rs`)

1. Snapshot existing top-level run directories under `runtime_root` BEFORE
   spawn (so the new run_id can be identified).
2. Create stdout/stderr log files under
   `<runtime_root>/_engine_logs/attempt-<unix_ns>-<mcp_pid>.{stdout,stderr}.log`.
   These survive MCP exit so failures stay diagnosable.
3. Spawn the engine via `std::process::Command` (NOT tokio's Command — we
   don't want tokio's reaper auto-registering it):
   - `stdin = Stdio::null()`
   - `stdout = Stdio::from(stdout_file)`
   - `stderr = Stdio::from(stderr_file)`
   - `pre_exec(|| if libc::setsid() == -1 { Err(...) } else { Ok(()) })`
4. Poll `runtime_root` for the new run dir on a 250 ms interval, deadline
   30 s. The engine creates `<runtime_root>/<loop_id>/` at
   `crates/context-graph-cli/src/bin/reality-loop.rs:1578`, right after task
   manifest validation — this is the canonical "engine has started" signal.
5. While polling, also `child.try_wait()` on each iteration:
   - `Ok(Some(status))` → engine exited during startup. Read both log tails
     and return `CCREALITY_INTERACT_ENGINE_EXITED_DURING_STARTUP`.
   - `Ok(None)` → still alive, keep polling.
   - `Err(...)` → return `CCREALITY_INTERACT_TRY_WAIT_FAILED`.
6. If 30 s elapses without a new run dir, return
   `CCREALITY_INTERACT_ENGINE_NO_RUN_DIR` with stderr/stdout tails so the
   operator can inspect.
7. On success, `drop(child)` — `std::process::Child::drop` neither waits
   nor kills, so the engine continues independently. Return
   `{ status: "spawned", run_id, runtime_root, pid, model, task_manifest,
   stdout_log, stderr_log, source_of_truth: "file:<run_dir>", guidance }`.

### Polling for results (callers)

Callers MUST NOT expect the response to contain attempt results. They poll
the artifact-query tools instead:
- `mcp__cgreality__reality_attempt_summary` — produced when the engine
  finishes the attempt (writes `attempt-summary.json`).
- `mcp__cgreality__reality_official_report` — produced after Docker eval.
- `mcp__cgreality__reality_signal` — partial reward signal during a run.
- `mcp__cgreality__reality_failure` — surfaces failures.

### Log retention

`<runtime_root>/_engine_logs/` is the canonical place for attempt logs.
Operator can `tail -f` either log to inspect a running engine. The
`_engine_logs` dir is excluded from `latest_run_id` and `list_run_dirs`
(via `is_run_metadata_dir` in interact.rs).

## What does NOT change

- `reality_run_smoke`, `reality_run_official_eval`, `reality_run_targeted_test`,
  `reality_recompile_reward`: still synchronous (`tokio::process::Command::output().await`
  with `kill_on_drop(true)`). They run in seconds–minutes and don't trip the
  Claude Code MCP timeout in practice. If a future case shows them tripping
  it, apply the same fire-and-forget shape.
- The MCP→CLI shim for harness write tools (`harness_*`): still synchronous —
  those are sub-second writes.
- `setsid()` precedent already established: see `spawn_daemon_process` in
  `crates/context-graph-mcp/src/main.rs:2010` (INFRA-H1 hardening). Same
  pattern applies here.

## Verification

- Build: `cargo build --release -p context-graph-mcp` — passes (3m 09s,
  warnings only, all pre-existing in the dead-code path-policy module).
- Strings check: new error codes baked into binary
  (`CCREALITY_INTERACT_ENGINE_EXITED_DURING_STARTUP`,
  `CCREALITY_INTERACT_ENGINE_NO_RUN_DIR`, `CCREALITY_INTERACT_TRY_WAIT_FAILED`,
  `CCREALITY_RUNTIME_ROOT_CREATE_FAILED`,
  `CCREALITY_ENGINE_STDOUT_LOG_CREATE_FAILED`, …) and guidance string
  ("engine runs in background; poll mcp__cgreality__…") confirmed via
  `strings target/release/context-graph-mcp`.
- Live test: pending — requires `/mcp reconnect` so the new binary is the
  singleton holder. Operator-driven step.

## What this supersedes

Nothing — this is a new locked decision. The previous implementation was
implicit (no decision file), and its behavior is preserved in git history
(commits prior to the next post-2026-05-08 commit on `ccreality-cutover`).

## Source of truth

- `file:/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/reality_loop/interact.rs`
  — handler implementation (function `reality_run_attempt` ~line 114; helper
  `list_run_dirs` and `is_run_metadata_dir` near end of file).
- `file:/home/cabdru/contextgraph/target/release/context-graph-mcp`
  — rebuilt binary, mtime 2026-05-08T07:43.

---
namespace: reports
agent_id: agent-111-coordinator
timestamp: 2026-05-08T12:44:00Z
status: active
tags: report, final, turn-111, root-cause-fix, mcp-disconnect, fire-and-forget, requires-mcp-reconnect
---

# Turn 111 — root-cause fix for MCP -32000 disconnect; requires `/mcp reconnect`

## What I did this turn

Implemented the fire-and-forget refactor of `mcp__cgreality__reality_run_attempt`
that was designed pre-compaction. The handler no longer blocks on
`cmd.output().await` for the engine's full 1–2 hour runtime — it now returns
within seconds once the engine has demonstrably created its run directory.

**Files touched (1):**
- `crates/context-graph-mcp/src/handlers/tools/reality_loop/interact.rs`
  - +217 lines / −9 lines (two edits, both shift-logged as round-02 and round-03)
  - Added imports: `std::collections::HashSet`, `std::fs`, `std::time::{Duration, Instant}`
  - Replaced `reality_run_attempt` body with `std::process::Command` + `pre_exec(setsid)`
    + run-dir polling with try_wait + structured-error fallbacks
  - Added private helpers `list_run_dirs` and `is_run_metadata_dir`

**Build:** `cargo build --release -p context-graph-mcp` succeeded in 3m 09s.
Warnings are all pre-existing in the dead-code `path_policy.rs` module.

**Binary verification:** `strings target/release/context-graph-mcp` confirms
all new error codes are baked in:
- `CCREALITY_INTERACT_ENGINE_EXITED_DURING_STARTUP`
- `CCREALITY_INTERACT_ENGINE_NO_RUN_DIR`
- `CCREALITY_INTERACT_TRY_WAIT_FAILED`
- `CCREALITY_RUNTIME_ROOT_CREATE_FAILED`
- `CCREALITY_ENGINE_LOG_DIR_CREATE_FAILED`
- `CCREALITY_ENGINE_STDOUT_LOG_CREATE_FAILED`
- `CCREALITY_ENGINE_STDERR_LOG_CREATE_FAILED`

…plus the new guidance string `"engine runs in background; poll
mcp__cgreality__reality_attempt_summary or reality_official_report for
progress"` and the `_engine_logs` directory marker.

**SMAP:** locked decision recorded at
`memory/decisions/agent-111-coordinator--reality-run-attempt-fire-and-forget.md`.

## Root cause (now documented in the decision file)

`reality_run_attempt` was synchronous: `tokio::process::Command::new(...).output().await`
blocked the JSON-RPC handler for the full 1–2 hour engine runtime. Claude
Code's stdio MCP transport closes the session well before then, returning
`error -32000 Connection closed`. The engine survived only because the
old MCP got SIGKILL'd by the singleton-takeover (turn 39) before its
`kill_on_drop(true)` Drop impl could fire — accidental survival, fragile,
and a clean SIGTERM would have killed the in-flight attempt mid-run.

Web research confirms the pattern: anthropics/claude-code issues #424
(timeout configurability), #20713 (race producing -32000), the MCP
lifecycle spec recommending implementations enforce timeouts.

## How the fix works

1. Snapshot `runtime_root`'s top-level dirs before spawn.
2. Spawn engine via `std::process::Command` (NOT tokio's wrapper) with
   stdout/stderr → log files in `<runtime_root>/_engine_logs/` and
   `pre_exec(setsid)` so the engine becomes its own session leader. Engine
   is now structurally immune to signals aimed at this MCP's process group.
3. Poll for the new run dir with 250 ms intervals, 30 s deadline. Engine
   creates `<runtime_root>/<loop_id>/` at `reality-loop.rs:1578` right after
   task-manifest validation.
4. Each iteration also `child.try_wait()`s — if engine died early we surface
   stdout/stderr tails via structured error.
5. On success, `drop(child)` (std::Child Drop neither waits nor kills) and
   return `{ status: "spawned", run_id, runtime_root, pid, model,
   task_manifest, stdout_log, stderr_log, source_of_truth, guidance }`.
6. Callers poll `reality_attempt_summary` / `reality_official_report` /
   `reality_signal` for results.

## Hard rules check

- §11.4 GENERIC-ONLY: ✅ this is harness-infrastructure, applies to every task.
- §11.9 Never do: ✅ no HTTP, no destructive git, no §8 deny-set edits.
- Single-writer rule: ✅ only one writer this turn (me, on the MCP server).
- §12 Hard rules: ✅ minimal blast radius (one handler), explicit error codes,
  no fallbacks/workarounds, log files for diagnosability.

## Operator action required

**`/mcp reconnect`** — the new binary at `target/release/context-graph-mcp`
needs to take over as the MCP singleton. After reconnect:

- Future `mcp__cgreality__reality_run_attempt` calls will return within
  seconds (not hours).
- The MCP -32000 disconnect pattern on attempt drips should be eliminated.
- Engine processes will survive `/mcp reconnect` cycles — they're now in
  their own session.

## How to verify after reconnect

After `/mcp reconnect`, drip a fresh attempt. The MCP response should:

1. Return within 1–30 s (engine startup time).
2. Contain `status: "spawned"`, a `run_id` matching `rust_reality_<ns>_<hash>`,
   a `pid`, `stdout_log` and `stderr_log` paths.
3. NOT contain `exit_code` or `stdout` (those imply the old synchronous shape).

Then `tail -f $stderr_log` to watch the engine run independently. `/mcp
reconnect` again — the engine should keep running (verifiable via `ps -p $pid`).

## Currently-running engine

Engine PID 1725881 (etime now ~75 min) is still running on the OLD binary's
attempt for `astropy__astropy-12907` under runtime root
`/tmp/contextgraph-rust-reality-loop/rust_reality_1778240223405536609_aaf099e6`.
The MCP rebuild does NOT disturb it — it's an already-spawned subprocess.
`/mcp reconnect` will SIGTERM/SIGKILL the OLD MCP but the engine survives
(orphaned to PID 1, same accidental-survival path that has been keeping
attempts alive across every prior reconnect). After reconnect, all NEW
attempts go through the fixed path.

## Pending follow-ups (NOT this turn)

1. **Apply same fire-and-forget shape** to `reality_run_official_eval` and
   `reality_run_targeted_test` IF either is observed to trip the MCP timeout.
   They run in seconds–minutes today and have not been reported as
   problematic.
2. **Behavioral nudge for inner-actor over-caution**: turn-79's tool-result
   cap helped per-round speed, but Qwen3-Coder MoE still spends 8+ rounds
   on file exploration before any apply attempt. Likely needs a system
   prompt addition like "commit to apply_*_edit by round 5". Out of scope
   for this turn — this turn was the MCP disconnect root cause.
3. **Update `docs/ccreality/03_MCP_REALITY_TOOLS_SPEC.md`** to reflect the
   new response shape for `reality_run_attempt`. Documentation drift only;
   does not affect behavior.

## Files touched this turn

- `crates/context-graph-mcp/src/handlers/tools/reality_loop/interact.rs`
- `memory/decisions/agent-111-coordinator--reality-run-attempt-fire-and-forget.md`
- `memory/reports/agent-111-coordinator--turn-111-final-report.md` (this file)

## Recommendation status

I cannot record `mcp__cgreality__optimizer_record_recommendation` this turn
because the MCP server is currently disconnected (system-reminder noted
"35 deferred tools are no longer available: mcp__cgreality__*"). The
recommendation will be recorded in the next turn after `/mcp reconnect`.

The recommendation will be `status: changed`, citing the SMAP decision
file and the rebuilt binary as the source-of-truth artifacts.

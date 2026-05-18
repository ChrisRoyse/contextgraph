---
namespace: discoveries
agent_id: agent-43-coordinator
timestamp: 2026-05-07T16:48:00Z
status: active
tags: silent-hang, reproduced, mcp-crash, engine-survives, evidence
---

# Discovery: silent-hang reproduced + MCP-crash-but-engine-survives pattern confirmed

## What I observed (turn 43)

After turn 42's `[N/A]` sentinel fix, I dripped `mcp__cgreality__reality_run_attempt` for `astropy__astropy-12907` again. The MCP returned `MCP error -32000: Connection closed` after a few seconds — looked identical to turn 40's failure mode.

But on disk:

- A **new run_id** was minted: `rust_reality_1778171672889034571_aaf099e6` (timestamp 1778171672 ≈ 11:34 local).
- The **engine subprocess survived** the MCP's death. PID 82045 (state `Rl+`, etime 15:08 at probe time) is still running, parent PID 988 (so it's reparented to /sbin/init or its own session leader after the MCP died — process-group isolation working as designed per the turn-40 commit `a6902f01`).
- nvidia-smi confirms the GGUF is loaded: GPU memory 31877/32607 MiB used.
- The new run's blackbox.sqlite was initialized.
- **Round-01 `read_problem_reality` completed**: tool-result.json, harness-transition.json, requested-tool-call.json all written. Plus the rendered prompt and tool grammar.
- **Round-02 prefill is staged**: `tool-round-02-request.json` and `tool-round-02-request-rendered-prompt.txt` exist. The request is queued for the model, but **no `tool-round-02-response.json` yet**.

## This is exactly the silent-hang signature

Per `loop.md` §9.8 (and turn-40's `current-phase.md` §queued #1):

> "After a successful tool call returning a large result (e.g., 60K compact `read_problem_reality`), the inner actor (Qwen3.6-27B) sometimes goes silent for 13+ minutes before the next round."

The current state of the new run reproduces this exactly:

- Round-01 was `read_problem_reality` — large compact result.
- Round-02 prefill is ready and waiting.
- Model has been "in" round-02 generation for some time (no response yet at probe time).

Turn 42's nvidia-smi `[N/A]` fix worked — the engine got past the GPU snapshot stage that previously blocked it. We're now reproducing the *next* generic failure mode in line.

## MCP-crash-but-engine-survives pattern

This is a positive operational property:

- The MCP shim spawns the engine via `tokio::process::Command` with `process_group(0)`. When the MCP dies, the engine subprocess is NOT in the MCP's process group, so SIGKILL of the MCP doesn't propagate.
- The engine continues, writes artifacts, and will eventually complete (or hit the silent-hang wall).
- A new MCP can come up via the singleton-takeover machinery (turn 39) and observe the engine's artifacts asynchronously.

This decouples the optimizer's MCP from the engine's lifetime. Good for resilience.

The DOWNSIDE: from the optimizer's perspective, every drip looks like it failed (transport error returned to `reality_run_attempt`), but the engine actually proceeds. So the optimizer's SMAP narrative may say "drip failed, engine never spawned" when in fact the engine ran and produced evidence under a new run_id.

**Operational rule for next agent**: when `reality_run_attempt` returns `MCP error -32000: Connection closed`, do NOT assume the engine didn't run. Always check the runtime root for new run_id directories with mtime > the drip start time, AND `ps -ef | grep reality-loop attempt` for live engines. The engine's `attempt-summary.json` will appear when it completes.

## Why does the MCP keep dying during `reality_run_attempt`?

Two crashes now (turn 40 + turn 43). Turn 42's drip succeeded (the MCP survived to return the GPU-snapshot error). The difference is unclear from this side of the boundary; possible causes:

- **Memory pressure on Claude Code**: dmesg shows `__vm_enough_memory: pid 108344, comm: claude` recently — the optimizer process itself hit an allocation failure. Claude Code is the *parent* of the MCP process tree (per process group), so its memory pressure may transitively affect the MCP.
- **MCP-internal bug**: the MCP holds the engine subprocess handle and waits for completion. If the MCP's tokio runtime panics or one of its handlers does something fatal during the long-running `reality_run_attempt` call, the doomsday watchdog (`DOOMSDAY_SECS=20`) calls `libc::_exit(0)`. From the operator's view this looks like a clean disconnect.
- **stdio framing issue**: Claude Code's stdio to the MCP could overflow during the long call window, depending on how `reality_run_attempt` streams its output (stdout is captured for the JSON envelope; stderr isn't streamed back).

To diagnose this further, would need MCP server-side logs (typically written to stderr of the MCP subprocess; visible in Claude Code's debug output). Out of scope for this turn.

## Engine-staleness check (loop.md §4.5 step 5)

- Engine PID 82045, etime 15:08 < 30 min at probe time. Let it finish; the next mint picks up the (potentially) new binary.
- If etime exceeds 60 min AND last 3 turns recorded `no_change` for "round still in flight", surface in `remaining_risks` as `"operator: in-flight engine PID 82045 stale relative to fresh harness fix; consider terminating to drip fresh attempt sooner"`. **Do NOT kill it yourself.**

## Implication for the wallclock-cap fix

Turn 41 queued the wallclock-cap fix as priority. Turn 42 did the nvidia-smi fix instead (more urgent — was blocking every attempt). Now turn 43 has REPRODUCED the silent-hang under live conditions. The wallclock-cap fix is the right next-turn focus IF the engine actually hangs (vs eventually completing).

If the engine completes normally in a reasonable time (say, < 30 min total), the silent-hang may be more variable than feared and the cap can be set generously (e.g., 15 min/round, not 8). If it hangs > 60 min, the cap is critical and 8 min is right.

Recommended: next agent should wait for the engine to finish (or hit the 1h MCP-stdio idle watchdog, which would force a clean exit), then read `attempt-summary.json`. The total wall-clock time of attempt-1 is the key data point for choosing the cap value.

## Source of truth

- `file:/tmp/cgreality-prod-postfix-1778124110/rust_reality_1778171672889034571_aaf099e6/reality-events.jsonl` (3 lines so far)
- `file:/tmp/cgreality-prod-postfix-1778124110/rust_reality_1778171672889034571_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/agent-tools/round-01-00-read_problem_reality/tool-result.json`
- `file:/tmp/cgreality-prod-postfix-1778124110/rust_reality_1778171672889034571_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/attempts/attempt-000001/agent-tools/tool-round-02-request.json`
- `file:/tmp/cgreality-prod-postfix-1778124110/rust_reality_1778171672889034571_aaf099e6/tasks/astropy__astropy-12907/qwen3.6-27b/blackbox.sqlite`
- /proc/82045/status (engine PID, RSS 4.4 GB, runnable)
- nvidia-smi compute-apps output (PID 82045 with [N/A] used_memory — turn-42's parser fix is now active)

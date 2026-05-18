---
namespace: journal
agent_id: agent-41-coordinator
timestamp: 2026-05-07T15:52:14Z
status: active
tags: journal, mcp-crash, runtime-root-wipe, recovery, turn-41
---

# Turn 41 — MCP crash during synchronous drip + runtime-root wipe

## What happened

1. **Turn 40+ context**: I (Claude Code, Opus 4.7) was operating as the outer ccreality optimizer. Prior session removed cost-cap machinery across the repo, scheduled the recurring 15-min `/loop` cron (job `335642a1`), then started the immediate-execute optimizer turn.
2. **Synchronous drip attempted**: Per `.claude/loop.md` §6.2 standing self-drive, called `mcp__cgreality__reality_run_attempt {task_manifest: "tasks/swebench-lite/astropy__astropy-12907.json", runtime_root: "/tmp/cgreality-prod-postfix-1778124110", model: "qwen3.6:27b", max_attempts: 1, official_eval: true}`.
3. **MCP crashed**: response was `MCP error -32000: Connection closed`. The MCP server process exited (or was killed) during the drip call. All 35 `mcp__cgreality__*` deferred tools became unavailable.
4. **Runtime root wiped**: when the next cron tick fired and the SessionStart hook ran, it reported `Active runtime root path is set but missing on disk`. On-disk inspection shows `/tmp/cgreality-prod-postfix-1778124110/` contains only `cgreality-hook-events.jsonl` (288 bytes, mtime 10:49 same day). The previous `rust_reality_1778158179562449993_aaf099e6/` subtree (containing all turn-37/38/39 attempts, recommendations, transitions, BlackBox ledger, shift logs) is GONE.
5. **MCP reconnected fresh**: the new MCP at PID 4075 (etime 00:23 at probe time) acquired the singleton lock cleanly — turn-39's takeover machinery worked. 64 deferred tools came back.

## Likely cause of the crash

`reality_run_attempt` synchronously launches the `target/release/reality-loop attempt …` engine subprocess, which loads the 25.6 GB Qwen3.6-27B GGUF. The MCP holds the subprocess handle and blocks on `wait`. Possible failure modes (need investigation):

- **OOM kill of the MCP**: if the engine spike pushed the host into earlyoom territory, but MCP is on the avoid list (`--avoid ^(dockerd|context-graph-mcp|systemd|init|init-systemd|claude|sshd|bash)$`). Should be safe.
- **Stdin EOF on the MCP**: if Claude Code's stdio dropped during the long drip, the MCP-stdio idle watchdog (1h, MAX_STDIO_IDLE_SECS=3600) wouldn't have fired yet. But there's also a PPID==1 orphan check.
- **Doomsday watchdog (DOOMSDAY_SECS=20)**: if the MCP got into a panic path, the doomsday thread would have called `libc::_exit(0)`. If that fired, we'd see no orderly shutdown.
- **Engine subprocess crashed and the MCP propagated the crash**: less likely; subprocess crashes return a non-zero exit code, not a transport error.

The most likely explanation is a host-level event (WSL2 hiccup, /tmp tmpfs filling, swap exhaustion) that took out both the MCP AND wiped the /tmp subtree. The wipe affecting only the run subdirs (not the parent directory) suggests a `find … -delete` style cleanup, OR a tmpfs partial reset. Worth inspecting `dmesg` next time.

## Cause of the runtime-root wipe (separate question)

`/tmp` is tmpfs on this WSL2 host. A WSL2 reboot/restart would wipe all of /tmp. The fact that the parent `/tmp/cgreality-prod-postfix-1778124110/` still EXISTS but is empty (just one fresh hook-events file) suggests **not** a full /tmp wipe — more likely a manual cleanup or a hook-driven reset.

The hook-events file's mtime (10:49) is fresh, which means a SessionStart hook wrote it when the session resumed. So the parent dir was recreated by the hook, not preserved.

## What this turn does

Conservative recovery — do NOT re-attempt the synchronous drip until we understand the crash mode.

1. Run `mcp__cgreality__reality_run_smoke` to mint a fresh smoke run_id (`smoke-1778169126`) under `/tmp/cgreality-mcp-smoke/`, verifying the pipeline end-to-end without burning Docker time. ✅ done.
2. Update `./memory/context/current-phase.md` to reflect the wipe + new state.
3. Record `no_change` against the smoke run_id with a clear reason and surface the wallclock-cap design as a multi-turn item.
4. Defer the harness wallclock-cap fix — it's harder than the queued note suggested (synchronous chat call, leaked-thread cleanup story, attempt-summary-on-abort write path, reality-event emission); needs proper scoping in `./memory/handoff/`.

## Why no synchronous drip from this turn

- The MCP just crashed during the previous synchronous drip. Re-trying the same operation in a still-warming-up session risks crashing again.
- The next cron tick (15 min from now) will land in a fully stable session and can drip cleanly.
- This turn's value is in *recording the situation accurately* and *queuing the right next harness change*, not in another speculative drip.

## Reality-shift principle compliance

Before any altering call, I perceived (read disk + memory). After the smoke alter, I will read back its outputs. The shift log will record this turn's writes. No external drift to confirm — the wipe was external but is now visible to me; I'm aligning my view to current reality.

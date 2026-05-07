---
namespace: discoveries
agent_id: coordinator
timestamp: 2026-04-18T09:45:00Z
status: resolved
severity: high
tags: discovery, daemon, ppid-1, orphan-kill, phase-2, resilience
---

# Discovery: Daemon Self-Kill Cascades to Phase 2 Pipeline Crash

## What Happened

During the Phase 2 full run (9K prompts, ~50h ETA), at ~09:40 the bash shell I'd launched the daemon in (`bktiz0pi1` via run_in_background) was terminated. The daemon detected PPID=1 (became orphan) and self-killed per its safety check. Two short-lived auto-spawn attempts (PID 631995, 632204) from the dashboard wrapper also died under the same condition.

Phase 2 hit a `MCP daemon closed connection during tools/call` RuntimeError after ~140 records, crashed (correctly — fail-fast as designed), leaving `shakespeare_v4_sft.jsonl.partial` with 140 good records.

## Blast Radius

- Direct loss: zero records (resume-safe `.partial` file intact)
- Clock cost: ~40 minutes of Phase 2 progress wasted + 2-3 min to diagnose + restart
- Infrastructure: daemon had to reload 13 embedders (~1 minute)

## Why It Matters

Any time the parent bash shell holding the daemon dies, the daemon self-kills and all clients crash. The shell can die from:
- Claude Code session compact / context rotation
- User explicitly killing the task
- WSL restart
- OOM killer picking bash (unlikely but possible)

For long-running pipelines (Phase 2 at 50h), the probability of at least one shell death over the full run is non-trivial.

## Mitigation Options (sorted by effort)

1. **Resume-safe pipeline** (already done) — fail-fast + `.partial` file means crash/restart is cheap.
2. **Automatic daemon revival** — Phase 2 could catch RPC error, attempt daemon restart, retry. Complexity: medium. Risk: suppresses real infra errors.
3. **Systemd user service with PPID=1 whitelist** — would need Rust-side change to skip PPID=1 check when run under systemd. Complexity: high.
4. **Daemonize properly** — Rust daemon does `fork()+setsid()` to detach from parent before PPID=1 check. Complexity: medium; tested insufficient per prior SITREP 2026-04-18.
5. **Keepalive watchdog script** — separate bash loop watches daemon, restarts if port 3100 goes silent. Complexity: low.

## What I Did This Session

Restarted daemon via run_in_background + verified port bind + restarted Phase 2 which auto-resumed from 140-record `.partial`. Total recovery time: ~3 minutes.

## Recommendation for Long-Run Pipelines

For any pipeline expected to run > 4 hours:
- ALWAYS use `.partial` resume-safe output
- ALWAYS log enough state that restart is trivial
- Consider adding a lightweight watchdog OR accept manual restart cost

For this project specifically: write a `scripts/watchdog_daemon_and_phase2.sh` that polls the daemon every 60s and restarts if down. Not a workaround — a legitimate infrastructure hardening.

## Related

- Prior SITREP discovered PPID=1 orphan check: `tmp/SITREP_2026-04-17_215601.md` (22:00 CDT section)
- Restart script: `scripts/restart_pipeline_services.sh`
- Daemon log: `/tmp/cg-daemon-v4.log`
- Phase 2 log: `/tmp/v4-phase2-full.log`

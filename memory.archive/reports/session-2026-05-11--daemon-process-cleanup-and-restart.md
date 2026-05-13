---
namespace: reports
created: 2026-05-11T10:24:25Z
updated: 2026-05-11T10:24:25Z
status: active
tags: report, session, daemon, process-cleanup, restart, fsv
---

# Daemon Process Cleanup And Restart

## What Happened
Cleaned up ContextGraph-only stale processes after the flywheel daemon hardening rebuild.

Terminated:

- Old daemon PID `57162`, which was running `/home/cabdru/contextgraph/target/release/context-graph-mcp (deleted)` from before the release binary rebuild.
- Stale shell watcher PID `38098`, an old `cargo build --release -p context-graph-cli` wait loop.

Restarted:

- `scripts/ensure_daemon.sh` started a fresh daemon from the current release binary.
- New daemon PID is `42005`.

## Source Of Truth Verified
Source of truth:

- `/mnt/d/contextgraph/state/locks/context-graph-daemon.pid`
- `/mnt/d/contextgraph/state/schedulers/flywheel_supervisor_status.json`
- `/mnt/d/contextgraph/state/schedulers/self_optimization_status.json`
- TCP listeners on `127.0.0.1:3100` and `127.0.0.1:3101`
- `/proc/<pid>/exe`

Evidence:

- Before cleanup, PID file contained `57162`; `/proc/57162/exe` resolved to the release binary with `(deleted)`.
- Old daemon exited after SIGTERM; no SIGKILL was needed.
- Before restart, the PID file was absent.
- `ensure_daemon.sh` reported ready after 84 seconds.
- After restart, PID file contained `42005`.
- `/proc/42005/exe` resolves to `/home/cabdru/contextgraph/target/release/context-graph-mcp` with no deleted flag.
- `/healthz` returned `{"endpoint":"/mcp","status":"ok","transport":"streamable-http"}`.
- Ports `3100` and `3101` are listening under PID `42005`.
- Supervisor state reads `status: "running"` with all four supervised tasks.
- Self-optimization scheduler state has ticker counts, including `observe: 71` at verification time.

## Related
- See also: ./memory/reports/session-2026-05-11--flywheel-daemon-hardening-review.md
- Tags: daemon, cleanup, restart, FSV

---
namespace: discoveries
created: 2026-05-11T06:11:33Z
updated: 2026-05-11T06:11:33Z
status: active
tags: flywheel, daemon, root-cause, fsv, shutdown
---

# Flywheel Daemon Root Cause And FSV

## What Happened
The first unblocked futurebuild task was `TASK-FLYWHEEL-001`, but the existing daemon implementation only provided the legacy shared MCP/proxy daemon. It did not validate D-drive storage, did not own a D-root daemon lock, did not start supervised ME-JEPA flywheel tasks, did not include a systemd service template, and did not persist `MEJEPA_SCHEDULER_FLAPPING` heal reports.

Manual FSV also exposed two startup/shutdown behaviors:
- `--no-warm` transport startup waited up to 60 seconds for graph-builder warmup before serving health.
- Shutdown could be delayed by background model loading, so background task handles now have bounded await/abort logging and graph-builder startup is deferred without blocking transport readiness.

## Why It Matters
The daemon is the live flywheel anchor. The source of truth is the filesystem and RocksDB under the validated D-root:
- `/mnt/d/contextgraph/fsv/mcp-daemon-fsv/startup.json`
- `<D_ROOT>/state/schedulers/flywheel_supervisor_status.json`
- `<D_ROOT>/state/schedulers/self_optimization_status.json`
- `<D_ROOT>/state/locks/context-graph-daemon.pid`
- `<D_ROOT>/storage/contextgraph-rocksdb`
- `<D_ROOT>/storage/mejepa-panels`

Verification performed:
- FSV example wrote and re-read `/mnt/d/contextgraph/fsv/mcp-daemon-fsv/startup.json`.
- Live isolated daemon wrote scheduler files, PID lock, logs, RocksDB files, and `/healthz` response, then SIGTERM changed supervisor status to `stopped` and removed the PID file.
- Edge cases failed closed for missing root, control-character root, lock contention, and unwritable root.

## Related
- See also: ./memory/decisions/2026-05-11--flywheel-daemon-uses-droot-supervisor-and-fail-closed-validation.md
- See also: ./memory/reports/session-2026-05-11--flywheel-daemon-mode.md
- Tags: daemon, source-of-truth, shutdown, FSV

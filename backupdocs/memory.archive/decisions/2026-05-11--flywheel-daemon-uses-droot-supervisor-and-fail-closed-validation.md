---
namespace: decisions
created: 2026-05-11T06:11:33Z
updated: 2026-05-11T06:11:33Z
status: active
tags: flywheel, daemon, d-root, mcp, futurebuild
---

# Flywheel Daemon Uses D-Root Supervisor And Fail-Closed Validation

## What Happened
`TASK-FLYWHEEL-001` is implemented as a real `context-graph-mcp --daemon-server-only` flywheel supervisor, not as a wrapper-only script behavior. The daemon now validates a durable D-root before startup, owns a D-root PID lock, starts supervised ME-JEPA flywheel tasks, and persists scheduler state under the validated root.

The daemon path must fail closed on missing, relative, invalid, unwritable, or already-locked roots. The service and wrapper scripts pass `--d-root` explicitly and should not create missing D-root mount points as a workaround.

## Why It Matters
Future work must treat `/mnt/d/contextgraph` or the explicitly provided `CONTEXTGRAPH_DATA_ROOT` as the source of truth for daemon state. If the root is not mounted or writable, the correct behavior is startup failure with `MEJEPA_D_DRIVE_*` or `MEJEPA_DAEMON_LOCK_HELD`, not falling back to a repo-local or temp store.

## Related
- See also: ./memory/discoveries/2026-05-11--flywheel-daemon-root-cause-and-fsv.md
- See also: ./memory/reports/session-2026-05-11--flywheel-daemon-mode.md
- Tags: flywheel, daemon, d-root, fail-closed

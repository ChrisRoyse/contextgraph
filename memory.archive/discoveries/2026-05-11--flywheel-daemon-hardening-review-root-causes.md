---
namespace: discoveries
created: 2026-05-11T10:19:34Z
updated: 2026-05-11T10:19:34Z
status: active
tags: flywheel, daemon, hardening, fsv, shutdown, d-root
---

# Flywheel Daemon Hardening Review Root Causes

## What Happened
Hardening review of `TASK-FLYWHEEL-001` found three root-cause issues beyond the first implementation:

1. Unwritable D-root could report `MEJEPA_D_DRIVE_SUBDIR_CREATE_FAILED` instead of the intended fail-closed `MEJEPA_D_DRIVE_NOT_WRITABLE`. The validator now probes root writability before creating subdirectories and probes the lock directory with an exact error path.
2. The supervisor could miss shutdown while inside restart backoff after a task panic. The loop now checks the watch state at the top of each cycle, aborts supervised tasks, drains the `JoinSet`, and writes `stopped` to the scheduler state source of truth.
3. The headless daemon could write `stopped` and remove its PID file while the OS process was still alive during Tokio runtime teardown. The headless daemon now performs durable shutdown, explicitly drops daemon locks, and exits the process; `main` also uses bounded `runtime.shutdown_timeout` so other modes do not wait indefinitely for blocking tasks.

## Why It Matters
For daemon mode the source of truth is the physical D-root state, not a return value:

- `<D_ROOT>/state/locks/context-graph-daemon.pid`
- `<D_ROOT>/state/schedulers/flywheel_supervisor_status.json`
- `<D_ROOT>/state/schedulers/self_optimization_status.json`
- `<D_ROOT>/storage/contextgraph-rocksdb`
- `<D_ROOT>/storage/mejepa-panels`

Future daemon hardening must verify the process table, PID file, scheduler JSON, open ports, and RocksDB column families together. A stopped scheduler state with a live process is a failed FSV even if the shutdown function returned cleanly.

## Related
- See also: ./memory/decisions/2026-05-11--headless-daemon-explicit-exit-after-durable-shutdown.md
- See also: ./memory/reports/session-2026-05-11--flywheel-daemon-hardening-review.md
- Tags: daemon, FSV, shutdown, source-of-truth

---
namespace: reports
created: 2026-05-11T10:19:34Z
updated: 2026-05-11T10:19:34Z
status: active
tags: report, session, flywheel, daemon, hardening, fsv
---

# Flywheel Daemon Hardening Review

## What Happened
Reviewed and hardened `TASK-FLYWHEEL-001` against the futurebuild flywheel specs and FSV requirements. The work focused on root-cause fixes, not fallbacks:

- D-root validation now probes root writability before creating subdirectories and reports exact unwritable lock-directory paths.
- The flywheel supervisor is injectable for real-source-of-truth tests, catches panics, records flapping to `CF_MEJEPA_HEAL_REPORTS`, drains tasks on shutdown, and cannot miss shutdown while backing off a restart.
- Scheduler JSON writes are atomic, fsynced, parent-directory synced, and byte-read-back verified.
- Headless daemon shutdown now keeps the PID lock source of truth consistent with the OS process lifecycle and exits explicitly after durable shutdown.
- `ensure_daemon.sh` now serializes launcher attempts, never overwrites the daemon-owned PID file, verifies the daemon-owned PID file after readiness, and terminates stale processes with SIGTERM before SIGKILL.
- The systemd unit now has start limits, restrictive umask, stop timeout, SIGTERM stop signal, and explicit log/lock parent creation.

## Source Of Truth Verified
Primary SoTs:

- `<D_ROOT>/state/locks/context-graph-daemon.pid`
- `<D_ROOT>/state/schedulers/flywheel_supervisor_status.json`
- `<D_ROOT>/state/schedulers/self_optimization_status.json`
- `<D_ROOT>/storage/contextgraph-rocksdb`
- `<D_ROOT>/storage/mejepa-panels`
- RocksDB `CF_MEJEPA_HEAL_REPORTS`

Manual FSV evidence captured:

- Public launcher path with `/tmp/contextgraph-daemon-proxy-fsv4-36110`:
  - Before: no PID file.
  - Trigger: `target/release/context-graph-mcp --mode reality-loop --daemon --d-root ... --daemon-port 39970 --http-port 39971 --no-warm`.
  - Running SoT: PID `36160`, health `{"endpoint":"/mcp","status":"ok","transport":"streamable-http"}`, supervisor `status: "running"`, four supervised tasks, self-optimization ticker counts present.
  - Shutdown trigger: `SIGTERM`.
  - After: process gone (`kill -0` returned 1), PID file absent, ports 39970/39971 had no listeners, supervisor `status: "stopped"`.
- Supervisor flapping test used real temp D-root + real RocksDB and verified `CF_MEJEPA_HEAL_REPORTS` count after an injected panic storm (`FSV_HEAL_REPORT_COUNT=1`).
- Edge cases:
  - Missing D-root failed closed with `MEJEPA_D_DRIVE_UNAVAILABLE` and no storage/PID state.
  - Empty/control-character roots failed before mutation with `MEJEPA_D_DRIVE_PATH_INVALID`.
  - Unwritable root failed with `MEJEPA_D_DRIVE_NOT_WRITABLE`.
  - Unwritable lock directory failed with `MEJEPA_D_DRIVE_NOT_WRITABLE: <root>/state/locks` and no PID file.
  - External PID lock contention failed with `MEJEPA_DAEMON_LOCK_HELD`.
  - Partial subdir root was repaired by creating the required D-root subdirs, then shutdown removed the PID file.

## Verification Commands
Passed:

- `cargo fmt --check`
- `bash -n scripts/ensure_daemon.sh scripts/context-graph-mcp-wrapper.sh`
- `systemd-analyze verify scripts/systemd/mejepa-daemon.service` (only unrelated NVIDIA unit warning was printed)
- `cargo check -p context-graph-mcp`
- `cargo test -p context-graph-mcp`
- `cargo test -p context-graph-mcp daemon_validate -- --nocapture`
- `cargo clippy -p context-graph-mcp --no-deps -- -D warnings`
- `cargo build --release -p context-graph-mcp`

Blocked:

- `cargo clippy -p context-graph-mcp -- -D warnings` still fails in pre-existing `context-graph-cli` lint debt with 528 errors. See `memory/blockers/2026-05-11--full-context-graph-cli-clippy-debt.md`.

## Files Changed
- `crates/context-graph-mcp/src/daemon.rs`
- `crates/context-graph-mcp/src/daemon_validate.rs`
- `crates/context-graph-mcp/src/main.rs`
- `scripts/ensure_daemon.sh`
- `scripts/systemd/mejepa-daemon.service`

## Notes For Future Work
An existing `/mnt/d/contextgraph` daemon process was present at PID `57162` on ports 3100/3101 and was left untouched. The isolated FSV daemons on custom ports were terminated and verified absent.

## Related
- See also: ./memory/discoveries/2026-05-11--flywheel-daemon-hardening-review-root-causes.md
- See also: ./memory/decisions/2026-05-11--headless-daemon-explicit-exit-after-durable-shutdown.md
- See also: ./memory/blockers/2026-05-11--full-context-graph-cli-clippy-debt.md

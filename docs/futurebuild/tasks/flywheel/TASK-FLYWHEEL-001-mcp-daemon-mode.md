---
task_id: TASK-FLYWHEEL-001
title: MCP daemon mode + supervised tokio tasks + systemd template + D-drive validator
phase: A
status: code_complete
blocked_by: []
blocks: [TASK-FLYWHEEL-002, TASK-PREDICT-001, TASK-LOOP-001, TASK-HEAL-001, TASK-OBS-001]
effort: 1-2d
spec_refs: [SPEC-FLYWHEEL.md REQ-FLYWHEEL-01..03,15, TECH-FLYWHEEL.md §1-2]
storage_policy: D-drive (/mnt/d/contextgraph/) per docs/futurebuild/16_STORAGE_LOCATION_POLICY.md
---

## Verification contract

Inherits from [`../../specs/FSV-PROTOCOL.md`](../../specs/FSV-PROTOCOL.md). Status is `code_complete` per the file frontmatter; remaining work is fresh-host enablement proof. SoTs: `/mnt/d/contextgraph/state/locks/context-graph-daemon.pid`, `/mnt/d/contextgraph/logs/mejepa-daemon.stdout.log`, `/mnt/d/contextgraph/fsv/mcp-daemon-fsv/startup.json`.

## Goal

Ship daemon mode for `context-graph-mcp`: validate the D-root, run the MCP server as the long-lived daemon process, and start four supervised tokio tasks in that process: shift subscriber, self-optimization scheduler (skeleton from FLYWHEEL-002), nightly GC scheduler, weekly evaluation scheduler.

The daemon must validate the D-drive root before starting any task; missing/unwritable D-root fails closed with `MEJEPA_D_DRIVE_UNAVAILABLE`. All artifact paths default to subdirs under `/mnt/d/contextgraph/`.

Provide a systemd unit template at `scripts/systemd/mejepa-daemon.service` and a wrapper script update so the daemon is the new MCP launch mode.

Current state from git history:

- `ab3da632` introduced the futurebuild daemon/D-root plan.
- `e628fb5b` shipped supervisor shutdown, atomic state writes, launch-lock handling, `scripts/ensure_daemon.sh`, and `scripts/systemd/mejepa-daemon.service`.
- `23593272` wired the daemon self-optimization task to the heal-crate scheduler.
- Remaining work is not daemon code scaffolding; it is fresh-host install/runbook proof plus the downstream data-loop tasks (`FLYWHEEL-003..006`, `PREDICT-001..002`, `OBS-001` production feed/dashboard).

## Inputs (read these)

- `crates/context-graph-mcp/src/main.rs` — MCP entry; daemon/D-root flags live here
- `crates/context-graph-mcp/src/handlers/...` — existing handler wiring (don't break stdio mode)
- `crates/context-graph-mejepa-shift-subscriber/src/lib.rs` — existing `ShiftSubscriber::run_until_shutdown` API
- `crates/context-graph-mejepa-hygiene/src/gc.rs` — existing `gc_run_nightly` orchestrator
- `crates/context-graph-mejepa/src/eval/runner.rs` — existing `EvalRunner::run_holdout_eval`
- `scripts/context-graph-mcp-wrapper.sh` / `scripts/ensure_daemon.sh` — wrappers that bootstrap the daemon
- `memory/decisions/2026-05-10--d-backed-durable-data-root-and-mcp-daemon.md` — locked D-drive policy
- `docs/futurebuild/specs/functional/SPEC-FLYWHEEL.md` REQ-FLYWHEEL-01..03,15
- `docs/futurebuild/specs/technical/TECH-FLYWHEEL.md` §1-3, 9
- `docs/futurebuild/16_STORAGE_LOCATION_POLICY.md` §4.1 (the validator code template)

## Outputs (write these)

- `crates/context-graph-mcp/src/main.rs` — extended with daemon/D-root flags (`--daemon`, `--daemon-server-only`, `--d-root`, daemon ports)
- `crates/context-graph-mcp/src/daemon.rs` — supervisor + JoinSet + restart with backoff + flapping report source-of-truth write
- `crates/context-graph-mcp/src/daemon_validate.rs` — D-drive validation
- `scripts/systemd/mejepa-daemon.service` — systemd template
- `scripts/ensure_daemon.sh` — launcher/wrapper guard with D-root validation and launch-lock handling
- FSV: `crates/context-graph-mcp/examples/daemon_startup_fsv.rs`

## Definition of done

1. `cargo run --release -p context-graph-mcp -- --daemon-server-only --d-root /mnt/d/contextgraph --daemon-port 3100 --http-port 3101` boots the daemon task set; pid file at `/mnt/d/contextgraph/state/locks/context-graph-daemon.pid` exists; log at `/mnt/d/contextgraph/logs/mejepa-daemon.stdout.log` shows D-root validation and supervised startup.
2. Removing `/mnt/d/contextgraph/` and re-running fails closed with exit code != 0 and the error message `MEJEPA_D_DRIVE_UNAVAILABLE`.
3. Removing write permission on the D-drive root fails closed with `MEJEPA_D_DRIVE_NOT_WRITABLE`.
4. Force-killing one of the supervised tokio tasks (e.g., shift subscriber) → daemon detects panic via JoinSet, restarts the task with exponential backoff (logged), task resumes.
5. Repeated panic > 5 times in 5 minutes → log `MEJEPA_SCHEDULER_FLAPPING` to stderr + write a `CF_MEJEPA_HEAL_REPORTS` entry with `severity=Catastrophic`.
6. systemd unit file installs cleanly: `systemctl --user enable --now mejepa-daemon` succeeds; `systemctl --user status mejepa-daemon` shows `active (running)`.
7. `cargo build --release -p context-graph-mcp` passes with no warnings.
8. `cargo test -p context-graph-mcp` passes.
9. `cargo clippy -p context-graph-mcp -- -D warnings` passes.
10. FSV evidence: `cargo run --release -p context-graph-mcp --example daemon_startup_fsv` writes `/mnt/d/contextgraph/fsv/mcp-daemon-fsv/startup.json` containing pid, supervised task names, validated D-drive paths.

## FSV evidence path

`/mnt/d/contextgraph/fsv/mcp-daemon-fsv/`

## Test boundary cases

- Empty `--d-root` value → fail closed with usage help
- D-root contains a control character → fail closed with `MEJEPA_D_DRIVE_PATH_INVALID`
- D-root exists but only some required subdirs missing → daemon creates them silently
- D-root readable but write fails (e.g., immutable bit) → fail closed with `MEJEPA_D_DRIVE_NOT_WRITABLE`
- Two daemon instances try to start → second fails to acquire pid lock; clean exit with `MEJEPA_DAEMON_LOCK_HELD`

## Notes

- The supervised task wrapper uses `tokio::task::JoinSet` + `AssertUnwindSafe(fut).catch_unwind()` per `TECH-FLYWHEEL.md §3`.
- Backoff cap = 60s; restart count window = 5 minutes; threshold = 5 restarts.
- Don't break existing stdio mode (no `--daemon` flag): `cargo run -p context-graph-mcp` still works for ad-hoc MCP queries.

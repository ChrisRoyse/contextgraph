---
namespace: reports
created: 2026-05-11T06:11:33Z
updated: 2026-05-11T06:11:33Z
status: active
tags: report, session, flywheel, daemon, fsv
---

# Session Report: 2026-05-11 — Flywheel Daemon Mode

## Task: Complete the first futurebuild task, root-cause issues, implement robust fix, and perform full state verification.
## Outcome: complete with one unrelated medium blocker

## Summary
The first futurebuild task was `TASK-FLYWHEEL-001`: add MCP daemon mode as the flywheel substrate. The root cause was that the repo had a legacy shared MCP daemon/proxy but no D-root validated flywheel daemon: no `--d-root`, no systemd template, no supervised shift/self-opt/GC/eval task set, no D-root PID lock, and no scheduler-flapping persistence.

Implemented a fail-closed daemon validator, D-root PID lock, supervised flywheel task supervisor, persisted scheduler status files, real RocksDB-backed flapping heal reports, shared DB access for the shift subscriber, non-blocking graph-builder startup under `--no-warm`, bounded shutdown handling for background task handles, and a systemd service template.

## Files Created or Modified in the Repo
- `crates/context-graph-mcp/src/daemon.rs` — supervised flywheel tasks, scheduler status, flapping heal reports, weekly eval wiring.
- `crates/context-graph-mcp/src/daemon_validate.rs` — D-root validation, required directory creation, write test, daemon PID lock.
- `crates/context-graph-mcp/examples/daemon_startup_fsv.rs` — writes and re-reads startup FSV report.
- `crates/context-graph-mcp/src/main.rs` — `--d-root`, D-root validation, flywheel supervisor startup, bounded daemon shutdown.
- `crates/context-graph-mcp/src/server/mod.rs` and `watchers.rs` — shared RocksDB handle, bounded task abort logging, deferred graph-builder startup.
- `crates/context-graph-mejepa-shift-subscriber/src/{subscriber.rs,runner.rs}` — shared DB opening and dynamic shift-log discovery.
- `crates/context-graph-storage/src/column_families.rs` — canonical storage now includes ME-JEPA CFs.
- `scripts/systemd/mejepa-daemon.service`, `scripts/ensure_daemon.sh`, `scripts/context-graph-mcp-wrapper.sh` — D-root aware daemon launch.

## Files Written or Updated in ./memory/
- `decisions/2026-05-11--flywheel-daemon-uses-droot-supervisor-and-fail-closed-validation.md` — daemon source-of-truth decision.
- `discoveries/2026-05-11--flywheel-daemon-root-cause-and-fsv.md` — root cause and verification evidence.
- `blockers/2026-05-11--full-context-graph-cli-clippy-debt.md` — unresolved full dependency clippy debt.
- `journal/2026-05-11--success-flywheel-daemon-fsv.md` — manual FSV success log.

## Decisions Made
- The daemon must use the validated D-root as source of truth and fail closed if it is missing, invalid, unwritable, or locked. Rationale: futurebuild storage cannot silently fall back to repo-local state. File: ./memory/decisions/2026-05-11--flywheel-daemon-uses-droot-supervisor-and-fail-closed-validation.md

## Discoveries
- `--no-warm` transport startup was blocked by graph-builder warmup wait; graph-builder startup now defers without delaying health readiness. Impact: daemon health and wrapper startup now reflect process readiness rather than embedding readiness. File: ./memory/discoveries/2026-05-11--flywheel-daemon-root-cause-and-fsv.md
- Background model loading can be inside non-cancelable model initialization; shutdown now aborts and logs bounded telemetry, while durable state still flushes and PID lock releases. File: ./memory/discoveries/2026-05-11--flywheel-daemon-root-cause-and-fsv.md

## Failures
- Initial manual daemon shutdown hit the old 10s force-exit path. Lesson: shutdown must not wait unboundedly on background model loading.
- Broad clippy failed in `context-graph-cli` with 528 dependency warnings-as-errors. Lesson: use `--no-deps` for the daemon package gate until CLI error-size/style debt is fixed, and track the blocker explicitly.

## Successes
- `cargo check -p context-graph-mcp`: pass.
- `cargo test -p context-graph-mcp`: pass, 414 unit tests passed, 6 model-dependent tests ignored, CLI args and graph-link integration tests passed.
- `cargo clippy -p context-graph-mcp --no-deps -- -D warnings`: pass.
- `cargo build --release -p context-graph-mcp`: pass.
- FSV example wrote and read `/mnt/d/contextgraph/fsv/mcp-daemon-fsv/startup.json`.
- Manual happy path and four edge cases were executed against real filesystem/RocksDB state.

## Open Blockers
- Full dependency clippy still fails in `context-graph-cli`. Severity: medium. File: ./memory/blockers/2026-05-11--full-context-graph-cli-clippy-debt.md

## Tests / Build
- Tests added: daemon validator unit tests, flapping report RocksDB round-trip test, startup FSV example.
- Tests passing: yes for MCP package gates.
- Build status: passing for `context-graph-mcp`; broader dependency clippy blocked by CLI lint debt.

## Resume Here Next Session
- Start by reading: `./memory/discoveries/2026-05-11--flywheel-daemon-root-cause-and-fsv.md` and `./memory/blockers/2026-05-11--full-context-graph-cli-clippy-debt.md`.
- Watch out for: `test` exit codes are shell truth values in manual logs; `0` means the path/process exists, `1` means absent.
- Priority: Next futurebuild task after TASK-FLYWHEEL-001, unless the CLI clippy debt is promoted to a release gate.
- Open question: whether production systemd should run warm-first only or expose a documented no-warm operational mode for faster daemon health with embeddings warming in the background.

## Confidence: high
The daemon path was verified by direct reads from the D-root source of truth, live health checks, physical RocksDB file inspection, SIGTERM shutdown state, and fail-closed edge cases.

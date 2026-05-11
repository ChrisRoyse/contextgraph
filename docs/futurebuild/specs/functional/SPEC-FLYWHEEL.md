# SPEC-FLYWHEEL — Functional Specification

> **Level 2 (functional spec)** per `prdtospec.md`. Defines WHAT the data flywheel must do.
> Owns: REQ-FLYWHEEL-01..15.
> Implementation reference: [`07_TRAINING_DATA_FLYWHEEL.md`](../../07_TRAINING_DATA_FLYWHEEL.md).

---

## Domain

The data flywheel is the orchestration layer that converts every Claude Code edit, every test outcome, every agent feedback event, and every operator override into a labeled training example for ME-JEPA — continuously, without operator intervention.

## User stories

- **As a Claude Code agent**, I want my edits to be observed automatically so ME-JEPA can predict their impact in real time without me calling any tool.
- **As an operator**, I want the system to learn continuously from production usage so I don't have to schedule retraining manually.
- **As a developer of ME-JEPA**, I want every shift, oracle outcome, and feedback event to land in append-only durable storage on D-drive so I can replay history for debugging.
- **As an SRE**, I want the daemon to be supervised and self-restarting so transient errors don't degrade prediction availability.

## Requirements

### REQ-FLYWHEEL-01 — MCP daemon mode

The MCP server SHALL expose a `--daemon` flag that, when set, starts the JSON-RPC stdio handler AND the long-running supervised tokio tasks (shift subscriber, self-optimization scheduler, nightly GC scheduler, weekly evaluation scheduler) in the same process.

### REQ-FLYWHEEL-02 — D-drive validation at startup

Daemon mode SHALL validate that `/mnt/d/contextgraph/` exists and is writable before starting any tokio task. If not, fail closed with `MEJEPA_D_DRIVE_UNAVAILABLE`.

### REQ-FLYWHEEL-03 — Tokio-task supervision

The daemon SHALL supervise its long-running tokio tasks via a `JoinSet`-based panic handler. On task panic, log the panic and restart the task with exponential backoff (cap 60s). After 5 restarts in 5 minutes, surface `MEJEPA_SCHEDULER_FLAPPING`.

### REQ-FLYWHEEL-04 — Shift subscriber as supervised task

The daemon SHALL run the shift subscriber (`context-graph-mejepa-shift-subscriber::ShiftSubscriber::run_until_shutdown`) as one of the supervised tokio tasks. The subscriber tails `/mnt/d/contextgraph/runtime/cgreality-shift-log/<session>.jsonl` files at a 250ms tick.

### REQ-FLYWHEEL-05 — Continuous self-optimization scheduler

The daemon SHALL run a self-optimization scheduler tokio task (NEW, `crates/context-graph-mejepa::heal::scheduler::run_self_optimization_scheduler`) that fires five tickers: observe (250ms), drift_check (60s), active_learning (600s), promote (3600s), continual_backprop (1800s), constellation_freshness (3600s).

### REQ-FLYWHEEL-06 — Nightly GC scheduler

The daemon SHALL run the nightly GC scheduler (`gc_run_nightly` per existing Phase 6) at 03:00 local time. Implementation choice: tokio interval timer or systemd timer; either acceptable.

### REQ-FLYWHEEL-07 — Weekly evaluation scheduler

The daemon SHALL run a weekly evaluation cron that calls `mejepa_eval_run` and writes the report to `/mnt/d/contextgraph/exports/eval/<date>/weekly.md`.

### REQ-FLYWHEEL-08 — Test-outcome capture hook

A new PostToolUse hook (Bash tool) SHALL parse pytest / cargo-test / jest output and append `TestOutcomeEvent { ts, session_id, test_id, outcome, duration_ms, error_log }` to `CF_MEJEPA_ORACLE_VERDICTS`.

### REQ-FLYWHEEL-09 — Asynchronous oracle binding

The self-optimization scheduler's observe_tick SHALL bind newly-arrived `TestOutcomeEvent`s to the most-recent matching prediction in `CF_MEJEPA_LIVE_PREDICTIONS` and call `MeJepaCompiler::observe(panel, action, panel_after_test, oracle=outcome)`.

### REQ-FLYWHEEL-10 — Three new RocksDB column families

The flywheel SHALL register three new CFs in `context-graph-mejepa-cf`:
- `CF_MEJEPA_DDA_SIGNALS` keyed by `(panel_id, chunk_id)` — DDA pairwise + cosine signal block
- `CF_MEJEPA_FAILURE_EXEMPLARS` keyed by `(failure_mode_class, embedder_violation_signature)` — reverse index for retrieval
- `CF_MEJEPA_AGENT_FEEDBACK` keyed by `(prediction_id, agent_id, ts)` — append-only feedback ledger

### REQ-FLYWHEEL-11 — `ActiveLearningKind::AgentSurprise` enum variant

Extend the existing `ActiveLearningKind` enum with an `AgentSurprise` variant that the agent-feedback path enqueues.

### REQ-FLYWHEEL-12 — Sampling-weight bump on agent surprise

When an agent records a `Surprise` event, the BatchSampler SHALL multiply the sampling weight of the affected chunks by `1 + 2 * severity_score` for the next epoch.

### REQ-FLYWHEEL-13 — Subscriber lag alert

The subscriber SHALL emit `MEJEPA_SHIFT_SUBSCRIBER_LAG_ALERT` when unprocessed shifts exceed 500 in any 1-minute window. The daemon escalates if the alert persists > 5 minutes.

### REQ-FLYWHEEL-14 — Witness-chain binding

Every observe() call SHALL write a per-step training cert with a `parent_witness_hash → self_hash` chain link, and every panel persisted SHALL bind to the contributing shift_id via witness hash.

### REQ-FLYWHEEL-15 — systemd template

The daemon SHALL ship with a systemd unit template at `scripts/systemd/mejepa-daemon.service` (operator-installable via `systemctl --user enable mejepa-daemon`).

## Acceptance criteria (functional FSV)

1. `cargo run --release -p context-graph-mcp -- --daemon --d-root /mnt/d/contextgraph` starts; pid file at `/mnt/d/contextgraph/state/locks/context-graph-daemon.pid` exists.
2. Removing `/mnt/d/contextgraph/` and re-running fails closed with `MEJEPA_D_DRIVE_UNAVAILABLE` and a non-zero exit code.
3. Forcing a panic in the scheduler tokio task → daemon restarts the task; log records the panic + restart count; 5 restarts in 5 minutes → `MEJEPA_SCHEDULER_FLAPPING` written to `CF_MEJEPA_HEAL_REPORTS`.
4. Editing a tracked file → shift-log line lands in `/mnt/d/contextgraph/runtime/cgreality-shift-log/<session>.jsonl` within 100ms; subscriber consumes within 250ms; panel materialized; prediction persisted.
5. Running `pytest tests/test_X.py::test_Y` → test-outcome capture hook writes `TestOutcomeEvent` to `CF_MEJEPA_ORACLE_VERDICTS`; observe_tick binds it to the most recent prediction; training cert written.
6. Calling `mejepa_record_agent_feedback` with `severity=Medium` → `SurpriseEvent` persisted to `CF_MEJEPA_AGENT_FEEDBACK`; queue length increments; sampling-weight multiplier on affected chunks = `1 + 2*0.5 = 2.0` for next epoch.

## Edge cases

- **Empty shift-log file**: subscriber returns immediately with no work; no error.
- **Malformed shift-log line**: subscriber logs `MEJEPA_SHIFT_LOG_PARSE_ERROR` and skips; does not advance watermark past the bad line so it can be inspected.
- **Test outcome arrives for a prediction that no longer exists in `CF_MEJEPA_LIVE_PREDICTIONS`** (TTL'd out): observe() runs in label-only mode (no per-chunk attribution); training cert flagged `late_oracle = true`.
- **Disk full during shift-log append**: hook fails closed with `CCREALITY_HOOK_DISK_FULL`; agent's Edit succeeds (shift is lost — operator alerted).
- **Subscriber processing N shifts > 60s**: lag alert fires; if persistent, daemon spawns a second consumer thread.

## Non-functional requirements

- Per-shift latency: p50 < 5s, p99 < 30s (per existing REQ-SHIFT-03).
- Daemon process memory baseline: < 4 GB RSS (unloaded models). Loaded models add per-embedder VRAM but bounded.
- All on-disk artifacts under `/mnt/d/contextgraph/`. Zero growth on C:\ root filesystem.

# Atomic task graph (futurebuild)

> Authoritative task index: [`13_TASK_INDEX.md`](../13_TASK_INDEX.md). This file is the directory map for the atomic task files. Phase A has detailed individual task files; other phases use compact per-domain `_all_tasks.md` files.
>
> **READ THIS FIRST every task.** [`../specs/FSV-PROTOCOL.md`](../specs/FSV-PROTOCOL.md) is the universal verification contract every TASK-* file inherits. The task file says *what* to build; FSV-PROTOCOL says *how to prove it works at the Source of Truth*. Without producing FSV-PROTOCOL §6 artifacts, no task is done — `cargo test` passing is necessary but not sufficient. **`ASSUME THE TASK IS WRONG`**: audit every file path, symbol name, CF reference, and effort estimate against the live tree before writing code (FSV-PROTOCOL §7).

## Phase A — Operational Unblock (detailed individual files + compact)

- [`flywheel/TASK-FLYWHEEL-001-mcp-daemon-mode.md`](flywheel/TASK-FLYWHEEL-001-mcp-daemon-mode.md)
- [`flywheel/TASK-FLYWHEEL-002-self-optim-scheduler-skeleton.md`](flywheel/TASK-FLYWHEEL-002-self-optim-scheduler-skeleton.md)
- [`flywheel/TASK-FLYWHEEL-006-new-cfs.md`](flywheel/TASK-FLYWHEEL-006-new-cfs.md)
- [`predict/TASK-PREDICT-001-injection-hook.md`](predict/TASK-PREDICT-001-injection-hook.md)
- [`predict/TASK-PREDICT-002-record-agent-feedback-tool.md`](predict/TASK-PREDICT-002-record-agent-feedback-tool.md)
- [`loop/TASK-LOOP-001-apply-new-loop-md.md`](loop/TASK-LOOP-001-apply-new-loop-md.md)
- [`loop/TASK-LOOP-007-hook-output-paths.md`](loop/TASK-LOOP-007-hook-output-paths.md)
- [`observability/TASK-OBS-005-c-drive-lint.md`](observability/TASK-OBS-005-c-drive-lint.md)
- Phase A remainder: see `flywheel/_all_tasks.md` (FLYWHEEL-003,004,005,007), `loop/_all_tasks.md` (LOOP-002), `observability/_all_tasks.md` (OBS-006,007), `predict/_all_tasks.md` (PREDICT-010), `runbook/_all_tasks.md` (RUN-001,010,011), `dr/_all_tasks.md` (DR-001,002,005,006), `security/_all_tasks.md` (SEC-001,006,007,008), `bootstrap/_all_tasks.md` (BOOT-001,003), `api/_all_tasks.md` (API-001,005)

## All other phases — per-domain `_all_tasks.md`

- [`flywheel/_all_tasks.md`](flywheel/_all_tasks.md) — FLYWHEEL-003..007 + UTML-001..009
- [`predict/_all_tasks.md`](predict/_all_tasks.md) — PREDICT-003..012 + LABEL-001..011
- [`corpus/_all_tasks.md`](corpus/_all_tasks.md) — CORPUS-001..004
- [`enrichment/_all_tasks.md`](enrichment/_all_tasks.md) — ENRICH-001..007
- [`heal/_all_tasks.md`](heal/_all_tasks.md) — HEAL-001..008
- [`observability/_all_tasks.md`](observability/_all_tasks.md) — OBS-001..016
- [`loop/_all_tasks.md`](loop/_all_tasks.md) — LOOP-002..006
- [`testing/_all_tasks.md`](testing/_all_tasks.md) — TEST-001..011
- [`runbook/_all_tasks.md`](runbook/_all_tasks.md) — RUN-001..012
- [`dr/_all_tasks.md`](dr/_all_tasks.md) — DR-001..008
- [`security/_all_tasks.md`](security/_all_tasks.md) — SEC-001..008
- [`bootstrap/_all_tasks.md`](bootstrap/_all_tasks.md) — BOOT-001..003
- [`api/_all_tasks.md`](api/_all_tasks.md) — API-001..005

## Phase B/E status snapshot (2026-05-13)

Phase B code/FSV is shipped for the core prediction surface: `PREDICT-003/004/005/006/007/008/009`, `SEC-003`, deterministic `PREDICT-LABEL-001..009` coverage in `crates/context-graph-mejepa-train/src/labels.rs`, and core `UTML-001..004` accounting. Source-of-truth proof is `/mnt/d/contextgraph/fsv/contextgraph-phase-b-fsv/phase-b-prediction-fsv.json`, with persisted RocksDB data in `CF_MEJEPA_LIVE_PREDICTIONS`.

Phase E code/FSV is shipped for the autonomous self-optimization policy layer: scheduler orchestration, per-cell drift, Jeffreys small-window drift, surprise weighting, real holdout promotion, A/B/C scoring, catastrophic approval, two-person approval enforcement, dissipation, per-head Fisher, dormancy, lambda ramp, constellation freshness, emergency eviction, witness integrity, drift attribution, and alert persistence. Source-of-truth proof is `/mnt/d/contextgraph/fsv/mejepa-phase-e-fsv/phase_e_self_optimization_fsv.json`, with durable readback from RocksDB CFs and `self_optimization_status.json`.

What remains is quality and hardening work, not the first API/schema/self-optimization substrate: full per-tool/per-language label analyzers, trained per-head holdout convergence, per-cell state-transfer T reporting, Phase F observability/dashboard cadence, and fresh-host/operator burn-in.

## Total task count

**~115 atomic tasks** across 13 domains and 8 phases. See `13_TASK_INDEX.md` for the full DAG, effort estimates, and dependency edges.

---
namespace: decisions
created: 2026-05-11T11:24:52Z
updated: 2026-05-11T11:24:52Z
status: active
tags: mejepa, flywheel, scheduler, self-healing
---

# Flywheel 002 Scheduler Owned By Heal Crate

`TASK-FLYWHEEL-002` is implemented as a crate-level scheduler in `context-graph-mejepa::heal`, not as an MCP-local ticker stub.

Decision:

- `context_graph_mejepa::heal::run_self_optimization_scheduler(db, config, shutdown_rx)` owns the 7 Phase A tickers.
- Scheduler status is a readback-verified JSON source of truth written by `SchedulerState`.
- The MCP daemon's `SelfOptimizationScheduler` supervised task calls the heal-crate scheduler.
- Per-tick errors are logged and persisted in scheduler status; status-write failure remains fail-closed.
- TCT CF names are registered in the canonical ME-JEPA CF registry so scheduler/TCT storage can be opened by the same hygiene DB descriptors.

Boundary:

- This is the Phase A scheduler skeleton. It inspects/delegates to shipped modules and persists evidence, but Phase E still owns full per-cell stratified drift, autonomous A/B/C production promotion decisions, and oracle binding.

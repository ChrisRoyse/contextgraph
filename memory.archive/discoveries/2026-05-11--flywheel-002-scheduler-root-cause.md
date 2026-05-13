---
namespace: discoveries
created: 2026-05-11T10:31:06Z
updated: 2026-05-11T10:31:06Z
status: active
tags: mejepa, flywheel, scheduler, self-healing
---

# Flywheel 002 Scheduler Root Cause

`TASK-FLYWHEEL-002` was not implemented in the ME-JEPA heal crate. The shipped daemon work from `TASK-FLYWHEEL-001` added a local MCP daemon loop named `run_self_optimization_scheduler`, but that loop only wrote ticker counts to `self_optimization_status.json`.

Root causes:

- There was no `crates/context-graph-mejepa/src/heal/scheduler.rs`.
- There was no `crates/context-graph-mejepa/src/heal/scheduler_state.rs`.
- The daemon did not call a crate-level `context_graph_mejepa::heal::run_self_optimization_scheduler`.
- The local daemon loop had six tickers and missed `emergency_eviction`.
- The ticker loop did not delegate to existing heal/eval/hygiene/TCT modules, so it could not provide physical source-of-truth evidence beyond a status file.

Corrective direction: put the scheduler in `context-graph-mejepa::heal`, wire the MCP daemon to it, centralize the TCT CF names in the shared CF registry, and make FSV read back the scheduler status file plus RocksDB CF state after execution.

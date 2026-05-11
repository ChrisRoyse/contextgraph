---
task_id: TASK-FLYWHEEL-002
title: Self-optimization scheduler skeleton (5 tickers calling shipped APIs)
phase: A
status: open
blocked_by: [TASK-FLYWHEEL-001]
blocks: [TASK-HEAL-001]
effort: 1d
spec_refs: [SPEC-FLYWHEEL.md REQ-FLYWHEEL-05, TECH-HEAL.md §3]
---

## Goal

Stand up the skeleton of `crates/context-graph-mejepa::heal::scheduler::run_self_optimization_scheduler` with 5 tickers (observe 250ms, drift 60s, active_learning 600s, promote 3600s, continual_backprop 1800s, constellation 3600s, emergency_eviction 600s). Each tick calls a stub function that delegates to existing shipped modules (no new logic — just wiring). Phase E will replace the stubs with full per-cell stratified, dissipation-aware logic.

## Inputs

- `crates/context-graph-mejepa/src/heal/{drift,ewc,promote,full_retrain,calibration,lora_refresh,distill,regulate,integrity,pipeline,drill,store,cf,fsv}.rs` — existing modules to consume
- `crates/context-graph-mejepa-train/src/learning_signal/continual_backprop.rs` — existing detect/reinit
- `crates/context-graph-mejepa-tct/src/freshness.rs` — existing
- `crates/context-graph-mejepa::eval::queue::ActiveLearningQueueState` — existing
- `crates/context-graph-mejepa-hygiene::quota::quota_status` — existing
- `docs/futurebuild/specs/technical/TECH-HEAL.md` §3

## Outputs

- `crates/context-graph-mejepa/src/heal/scheduler.rs` — NEW: skeleton of `run_self_optimization_scheduler` per TECH-HEAL.md §3
- `crates/context-graph-mejepa/src/heal/scheduler_state.rs` — NEW: `SchedulerState` struct
- `crates/context-graph-mejepa/src/heal/mod.rs` — extension: `pub mod scheduler;`

## Definition of done

1. `run_self_optimization_scheduler(storage, config, shutdown_rx)` is callable from the daemon (FLYWHEEL-001) and returns when shutdown fires.
2. All 7 tickers fire at expected intervals (verified by FSV with shorter-than-default periods).
3. Each tick logs at `tracing::debug` level which action it took (drain N shifts, no drift, etc.).
4. `cargo build --release -p context-graph-mejepa` passes.
5. `cargo test -p context-graph-mejepa heal::scheduler` passes.
6. FSV: `cargo run --release -p context-graph-mejepa --example scheduler_5_ticker_fsv` runs the scheduler with 1s tick periods for 10 seconds and writes `/mnt/d/contextgraph/fsv/mejepa-heal-fsv/scheduler_ticks.json` showing each tick fired the expected number of times.

## FSV evidence path

`/mnt/d/contextgraph/fsv/mejepa-heal-fsv/scheduler_ticks.json`

## Boundary cases

- Shutdown signal fires mid-tick → in-flight call completes; loop exits cleanly
- Storage handle becomes invalid (corruption test) → tick logs error; next tick retries
- Tick handler panics → caller (the supervised wrapper from FLYWHEEL-001) catches and restarts

---
namespace: discoveries
agent_id: agent-codex
created: 2026-05-13T08:14:03Z
updated: 2026-05-13T08:14:03Z
status: active
tags: phase-f,heal-scheduler,fsv,rocksdb
---

# Heal Scheduler FSV Requires Full CF Contract

The heal scheduler state path requires the full `all_hygiene_referenced_cfs()` RocksDB schema, not only the inference CF subset.

Why: TASK-OBS-018 FSV initially opened the DB with `open_infer_rocksdb`, then `SchedulerState::open_with_counters` failed closed on missing `CF_MEJEPA_FISHER_SNAPSHOTS`.

How to apply: any FSV harness that constructs `SchedulerState` should open a DB with descriptors from `context_graph_mejepa_cf::all_hygiene_referenced_cfs()` and flush that same full CF set before reopen readback.

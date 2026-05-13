---
namespace: discoveries
agent_id: codex
created: 2026-05-11T00:00:00Z
updated: 2026-05-11T00:00:00Z
status: active
tags: discovery, mejepa, corpus, rocksdb, swebench, storage, fsv
---

# Phase 0 corpus source of truth was ephemeral and is gone

## What

The previously verified full Phase 0 Python corpus lived under:

`/tmp/contextgraph-mejepa-corpus-prod-merged-normalized-20260509/`

The 2026-05-10 session report recorded 2,400 entries, 300 tasks, all 8
mutation categories, RocksDB CF counts `2400/2400/1`, and corpus SHA
`f5a413eadbb1fa08f7fa51fd05c704eebe7ba51789c3ac0194592825dc2dae33`.
That directory no longer exists after reboot, so it is historical evidence,
not an active source of truth.

## Evidence

- `/tmp/contextgraph-mejepa-corpus-prod-merged-normalized-20260509` does not exist.
- `/mnt/d/contextgraph/storage/contextgraph-rocksdb` is accessible but contains
  zero rows in the ME-JEPA corpus CFs.
- `/mnt/d/contextgraph/fsv/swebench-lite-300x8-20260511` was an accidental
  partial target and contains zero corpus rows.
- D-backed Docker evidence shows all 300 SWE-bench Lite images are prewarmed:
  `/mnt/d/contextgraph/logs/mejepa-prewarm-all300-20260509.json`.

## Root cause

The corpus generator and docs allowed the final production corpus to be treated
as durable while it was stored under `/tmp`. Reboot cleanup deleted the physical
source of truth. The data did not fail logically; the storage policy failed.

## Required follow-up

Regenerate the full 2,400-row corpus under:

`/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/`

Then verify `index.json`, `stats.json`, `verify-evidence.json`, and
`oracle-store/` RocksDB CF counts from disk before using the corpus as ship-gate
evidence.

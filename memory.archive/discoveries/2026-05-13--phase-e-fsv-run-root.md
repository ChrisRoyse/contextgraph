---
namespace: discoveries
agent_id: agent-coordinator
created: 2026-05-13T02:19:43Z
updated: 2026-05-13T02:19:43Z
status: active
tags: phase-e,fsv,rocksdb,d-drive
---

# Phase E FSV Harness Uses Fresh Run Roots

The Phase E FSV harness should keep the stable artifact path at
`/mnt/d/contextgraph/fsv/mejepa-phase-e-fsv/phase_e_self_optimization_fsv.json`
but write per-case RocksDB evidence under a fresh `run-<timestamp>-<pid>/`
directory.

Why: deleting and recreating the whole D-drive FSV root can fail with
`Directory not empty` after RocksDB evidence runs, even when all handles have
been dropped. Reusing a stable root also makes it easier to accidentally read
old physical artifacts.

How to apply: write the current report to the stable artifact path, include
`run_root` in the JSON, and point each case `evidence_path` at that fresh run
root. Verify the fresh run root's SST files, not a stale root-level directory.

---
namespace: decisions
agent_id: codex
created: 2026-05-11T00:00:00Z
updated: 2026-05-11T00:00:00Z
status: active
tags: decision, locked, mejepa, corpus, d-drive, storage, fsv
---

# DECISION: ME-JEPA corpus outputs must live under /mnt/d/contextgraph/corpus

## The decision

All ME-JEPA corpus outputs that matter after a reboot must live under:

`/mnt/d/contextgraph/corpus/`

This includes full SWE-bench Lite 300x8 corpus runs, smoke corpus runs,
merge outputs, normalize outputs, and any future resumable production corpus
source of truth. `/tmp` is not valid for corpus data. Sibling D-root paths such
as `/mnt/d/contextgraph/fsv/` are evidence/scratch paths, not corpus stores.

## Enforcement

- `context-graph-paths` exposes `mejepa_corpus_root_dir()` and
  `require_under_mejepa_corpus_root()`.
- `context-graph-mejepa-corpus` CLI subcommands that read/write corpus stores
  call the corpus-root guard.
- The library generator entry point also rejects output roots outside the corpus
  root, so non-CLI callers cannot bypass the policy.
- Evidence/index file writes now use same-directory temp files, fsync, rename,
  parent fsync, and readback.

## Why

The 2026-05-10 full 2,400-row production corpus was verified but stored in
`/tmp` and disappeared after reboot. The corpus is operational training and
ship-gate state; it must be D-backed durable state, not ephemeral scratch.

## Current target

`/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/`

The run is complete only after independent readback of `index.json`,
`stats.json`, `verify-evidence.json`, and the RocksDB corpus CFs.

---
namespace: journal
created: 2026-05-09T17:42:44Z
updated: 2026-05-09T17:42:44Z
status: active
tags: phase0, corpus, swebench, smoke, fsv, edge-cases
---

# Success: Phase 0 Batch Smoke and Edge Gates

## What Happened
Before launching the full 2,400-row Phase 0 corpus run, I ran two real
SWE-bench batch smoke tests:

- `/tmp/contextgraph-mejepa-corpus-batch-smoke-20260509`:
  2 instances x `known_good`; 2 mutation CF rows, 2 verdict CF rows, 1
  provenance CF row; 100% oracle resample matched persisted verdict hashes.
- `/tmp/contextgraph-mejepa-corpus-batch-allcat-smoke-20260509`:
  1 instance x all 8 mutation categories; 8 mutation CF rows, 8 verdict CF
  rows, 1 provenance CF row; deterministic oracle sample matched persisted
  verdict hashes.

Physical RocksDB readback with `ldb` on the all-category smoke confirmed:

- `mejepa_mutation_corpus`: 8 rows
- `mejepa_oracle_verdicts`: 8 rows
- `mejepa_corpus_provenance`: 1 row

## Edge Cases
Manual fail-closed checks passed:

- Unsupported language `rust` failed before output creation.
- Non-empty output directory failed and preserved the existing sentinel; no
  `index.json` was created.
- Tampered patch verification failed on a patch hash mismatch and skipped Docker
  resampling because local source-of-truth verification already failed.

## Why It Matters
This validates the production path the full run will use: source-backed patch
generation, batch SWE-bench Docker oracle, atomic RocksDB mutation+verdict
writes, provenance finalization, stats, and verify readback. It also confirms
the Phase 0 language scope: full corpus production should run Python only;
non-Python `Language` variants intentionally fail closed until their operators
and fixtures exist.

## Related
- `/tmp/contextgraph-mejepa-corpus-batch-smoke-20260509/smoke-evidence.json`
- `/tmp/contextgraph-mejepa-corpus-batch-allcat-smoke-20260509/smoke-evidence.json`
- `memory/discoveries/2026-05-09--phase0-batch-oracle-control-byte-validation.md`

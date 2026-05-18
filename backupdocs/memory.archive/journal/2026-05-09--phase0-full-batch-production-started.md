---
namespace: journal
created: 2026-05-09T17:42:44Z
updated: 2026-05-09T17:42:44Z
status: active
tags: phase0, corpus, swebench, full-production, batch
---

# Phase 0 Full Batch Production Started

## What Happened
Started the full Phase 0 corpus generation using the release binary:

```text
target/release/mejepa-corpus generate \
  --output /tmp/contextgraph-mejepa-corpus-full-batch-20260509 \
  --language python \
  --run-id-prefix mejepaPhase0FullBatch \
  --seed 17 \
  --oracle-workers 4 \
  --instance-timeout-secs 1800 \
  --overall-timeout-secs 28800
```

This path runs one official SWE-bench harness batch per mutation category,
instead of one subprocess per row. It should produce 300 instances x 8 mutation
categories = 2,400 corpus rows.

## Source of Truth
- Index: `/tmp/contextgraph-mejepa-corpus-full-batch-20260509/index.json`
- RocksDB store: `/tmp/contextgraph-mejepa-corpus-full-batch-20260509/oracle-store`
- Harness logs: `/tmp/contextgraph-mejepa-corpus-full-batch-20260509/swebench-runs/logs/run_evaluation`

The index must remain `complete: false` until all rows and the provenance CF row
are persisted. Final verification must inspect the physical RocksDB column
families, provenance row, patch hashes, and a deterministic Docker oracle sample.

## Why It Matters
This is the operational materialization step left after the code tasks passed.
The production run is intentionally Python-only: SWE-bench Lite is a Python
benchmark, and non-Python `Language` variants currently fail closed until their
mutation operators and fixtures exist.

## Related
- `memory/journal/2026-05-09--success-phase0-batch-smoke-and-edge-gates.md`
- `memory/discoveries/2026-05-09--phase0-batch-oracle-control-byte-validation.md`

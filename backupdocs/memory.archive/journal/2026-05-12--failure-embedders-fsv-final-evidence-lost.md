---
namespace: journal
created: 2026-05-12T16:55:00Z
updated: 2026-05-12T16:55:00Z
status: active
tags: phase-d, fsv, embedders, artifact-readback
---

# Failure: Embedders FSV Final Evidence Lost

## What Happened
Running `cargo run -p context-graph-mejepa-embedders --example embedders_fsv`
wrote partial source-of-truth files (`routing_coverage.json` and
`vram_budget_report.json`) but exited before writing
`embedders_fsv_evidence.json`.

## Why It Matters
For long-running model artifact verification, batching the final evidence until
the end makes an interruption ambiguous. The physical partial files prove some
steps ran, but not which digest/model step was active when the process ended.

## Root Cause
The FSV harness records major checks in memory first and writes the final
summary only after routing, VRAM, config digest verification, content-set load,
and edge cases. That is brittle for multi-GB model verification.

## Next Fix
Harden the FSV harness to write a progress journal under the same D-backed FSV
root after every major Source-of-Truth readback step, while preserving fail-closed
behavior for any digest, config, VRAM, or model error.

## Related
- `crates/context-graph-mejepa-embedders/examples/embedders_fsv.rs`
- `/mnt/d/contextgraph/fsv/contextgraph-mejepa-embedders-fsv/`

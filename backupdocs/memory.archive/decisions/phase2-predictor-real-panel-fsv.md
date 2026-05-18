---
namespace: decisions
created: 2026-05-10T11:03:27Z
updated: 2026-05-10T11:03:27Z
status: active
tags: mejepa, phase2, predictor, fsv, real-data
---

# Phase 2 Predictor Uses Real Phase 1 Panel Evidence

## What Happened
Phase 2 was implemented as a new `crates/context-graph-mejepa` crate. Its FSV and CLI dry-run surfaces use the real Phase 1 panel JSON at `/tmp/contextgraph-mejepa-instruments-fsv/phase1-extended/panel-all-slots.json` as the source panel. If that file is missing, the Phase 2 code fails closed with a clear remediation message to run the Phase 1 extended FSV.

## Why It Matters
The user explicitly rejected workarounds, fallbacks, and mock data. The Phase 2 predictor therefore does not silently generate a fake corpus when the real panel source is absent. Synthetic variation is only deterministic batching around the real persisted panel so expected shapes, hashes, and edge cases can be verified.

## Related
- See also: ./memory/reports/session-2026-05-10--phase2-predictor-fsv.md
- Tags: mejepa, phase2, predictor, real-data

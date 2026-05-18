---
namespace: journal
created: 2026-05-09T00:00:00Z
updated: 2026-05-09T00:00:00Z
status: active
tags: phase0, corpus, swebench, full-production, fsv
---

# Phase 0 Full Corpus Production Started

## What Happened
After implementing the remaining Phase 0 corpus code tasks, a real-data smoke command was added to `mejepa-corpus`. Smoke evidence lives at `/tmp/contextgraph-mejepa-corpus-smoke-20260509/smoke-evidence.json` and passed across one SWE-bench Lite instance and all eight mutation categories. A second row-loop smoke lives at `/tmp/contextgraph-mejepa-corpus-smoke-rowloop-20260509/smoke-evidence.json` and proved `index.json` appears immediately with `complete=false`, rows advance one at a time, and verify resamples persisted rows.

The first full run attempt was intentionally stopped before row commits because the generator preplanned all patches before creating output. That was fixed by moving source-backed mutation generation into the persisted row loop. The release binary was rebuilt and full production restarted:

```bash
OUT=/tmp/contextgraph-mejepa-corpus-full-20260509
rm -rf "$OUT"
target/release/mejepa-corpus generate \
  --output "$OUT" \
  --language python \
  --run-id-prefix mejepaPhase0Full \
  --seed 17 \
  --instance-timeout-secs 1800 \
  --overall-timeout-secs 3600
```

Current source of truth while running:
- `/tmp/contextgraph-mejepa-corpus-full-20260509/index.json`
- `/tmp/contextgraph-mejepa-corpus-full-20260509/oracle-store`

Observed live state after restart:
- `index.complete=false`
- `entries=2`
- `mejepa_mutation_corpus` CF count = 2
- `mejepa_oracle_verdicts` CF count = 2
- latest row: `astropy__astropy-12907|subtle_flip`

## Why It Matters
The full production run is long-running. Future continuation should not restart from scratch unless the active process has exited or failed. Inspect the running process/session and the live index/store first.

## Related
- `memory/discoveries/2026-05-09--phase0-corpus-live-gap-root-cause.md`
- `/tmp/contextgraph-mejepa-corpus-smoke-20260509/smoke-evidence.json`
- `/tmp/contextgraph-mejepa-corpus-smoke-rowloop-20260509/smoke-evidence.json`

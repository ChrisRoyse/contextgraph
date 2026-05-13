---
namespace: journal
created: 2026-05-09T00:00:00Z
updated: 2026-05-09T00:00:00Z
status: active
tags: phase0, corpus, swebench, docker-lock, production
---

# Phase 0 Locked Sharded Production Restart

## What Happened
After the Docker SDK container-list/inspect race, `run_swebench_lite_oracle` was hardened with an interprocess lock:

```text
/tmp/contextgraph-mejepa-corpus-swebench-harness.lock
```

A two-process real Docker lock smoke passed:
- `/tmp/contextgraph-mejepa-corpus-lock-smoke-a-20260509` completed `sympy__sympy-20590|known_good`.
- `/tmp/contextgraph-mejepa-corpus-lock-smoke-b-20260509` completed `astropy__astropy-12907|known_good`.
- Both stores had 1 mutation row and 1 verdict row.
- While running, only one SWE-bench Python harness process was active and the lock file recorded its owner.

Locked sharded production was restarted from scratch under:

```text
/tmp/contextgraph-mejepa-corpus-shards-locked-20260509/
```

Sessions:
- `78411` -> `out-00`
- `8234` -> `out-01`
- `39773` -> `out-02`
- `70249` -> `out-03`

First observed locked state:
- lock owner: shard 03, `astropy__astropy-14995|known_good`
- out-02 had already committed 1 row
- out-00/out-01/out-03 were at 0 rows

## Why It Matters
Do not restart the old failed `/tmp/contextgraph-mejepa-corpus-shards-20260509` shard outputs. The valid production restart is the locked output directory above. The lock serializes only the SWE-bench harness subprocess; shard processes can still maintain independent output stores and be merged after completion.

## Related
- `memory/journal/2026-05-09--failure-swebench-parallel-docker-race.md`

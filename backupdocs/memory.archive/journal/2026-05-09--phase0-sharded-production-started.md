---
namespace: journal
created: 2026-05-09T00:00:00Z
updated: 2026-05-09T00:00:00Z
status: active
tags: phase0, corpus, swebench, sharded-production, fsv
---

# Phase 0 Sharded Production Started

## What Happened
Sequential full generation was healthy but projected to take roughly a day. I added a `mejepa-corpus merge` command that physically reads shard patch files and RocksDB rows, writes them into a final output store with atomic `put_corpus_row`, reads each merged row back, then writes and reads final provenance.

Merge smoke passed using `/tmp/contextgraph-mejepa-corpus-smoke-rowloop-20260509` as a shard:

```bash
cargo run -p context-graph-mejepa-corpus --bin mejepa-corpus -- merge \
  --output /tmp/contextgraph-mejepa-corpus-merge-smoke-20260509 \
  --shard /tmp/contextgraph-mejepa-corpus-smoke-rowloop-20260509 \
  --allow-partial
cargo run -p context-graph-mejepa-corpus --bin mejepa-corpus -- verify \
  --corpus /tmp/contextgraph-mejepa-corpus-merge-smoke-20260509 \
  --sample-fraction 1.0 \
  --run-id-prefix mejepaPhase0MergeSmoke
```

Physical readback after merge smoke:
- mutation CF count = 2
- verdict CF count = 2
- provenance CF count = 1

I created balanced shard instance lists under `/tmp/contextgraph-mejepa-corpus-shards-20260509/`:
- `shard-00` = 75 instances
- `shard-01` = 75 instances
- `shard-02` = 75 instances
- `shard-03` = 75 instances

Four release shard generators are running:
- session `49967`: `/tmp/contextgraph-mejepa-corpus-shards-20260509/out-00`
- session `10174`: `/tmp/contextgraph-mejepa-corpus-shards-20260509/out-01`
- session `27416`: `/tmp/contextgraph-mejepa-corpus-shards-20260509/out-02`
- session `3536`: `/tmp/contextgraph-mejepa-corpus-shards-20260509/out-03`

Each shard should complete with 600 rows. After all four finish, merge into a final full corpus and run verify/stats:

```bash
target/release/mejepa-corpus merge \
  --output /tmp/contextgraph-mejepa-corpus-full-merged-20260509 \
  --shard /tmp/contextgraph-mejepa-corpus-shards-20260509/out-00 \
  --shard /tmp/contextgraph-mejepa-corpus-shards-20260509/out-01 \
  --shard /tmp/contextgraph-mejepa-corpus-shards-20260509/out-02 \
  --shard /tmp/contextgraph-mejepa-corpus-shards-20260509/out-03
target/release/mejepa-corpus verify \
  --corpus /tmp/contextgraph-mejepa-corpus-full-merged-20260509 \
  --sample-fraction 0.05 \
  --run-id-prefix mejepaPhase0MergedVerify \
  --instance-timeout-secs 1800 \
  --overall-timeout-secs 3600
target/release/mejepa-corpus stats \
  --corpus /tmp/contextgraph-mejepa-corpus-full-merged-20260509
```

## Why It Matters
The final corpus should be one merged source of truth, not four unrelated shard outputs. Do not treat shard outputs as final until the merge and final verify pass.

## Related
- `memory/journal/2026-05-09--phase0-full-production-started.md`
- `/tmp/contextgraph-mejepa-corpus-merge-smoke-20260509/verify-evidence.json`

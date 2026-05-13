---
namespace: discoveries
created: 2026-05-09T19:05:00Z
updated: 2026-05-09T19:05:00Z
status: active
severity: medium
tags: phase0, corpus, docker, swebench, image-prep, root-cause
---

# Phase 0 Image Prep Pipe Deadlock

## What Happened

Shard 0 generation reached 120/150 `known_good` SWE-bench reports, then stalled
at the transition from cached Django images to uncached Matplotlib images.
The active harness had zero-byte `run_instance.log` files for the Matplotlib
instances and no evaluator containers. A direct `docker pull` showed the images
were pullable but slow.

The first implementation of `mejepa-corpus prepare-images` also stalled because
it spawned `docker pull` with piped stdout/stderr and did not drain progress
output until process exit. Docker pull progress can fill the pipe, blocking the
child process even though Docker itself is healthy.

## Why It Matters

Production Phase 0 should not rely on the SWE-bench harness to lazily pull many
remote images inside evaluator workers. Pulling should be a separate, bounded,
auditable preflight step. When implementing that preflight, do not capture
verbose `docker pull` progress without draining it.

## Fix Direction

- Use `docker pull --quiet` in image prep so captured output remains small.
- Prewarm the full 150-instance shard at bounded parallelism before running all
  8 mutation categories.
- Keep image-prep evidence outside the corpus output directory so `generate`
  can still enforce fresh/non-contaminated output state.

## Related

- `crates/context-graph-mejepa-corpus/src/bin/mejepa-corpus.rs`
- Evidence target: `/tmp/contextgraph-mejepa-prewarm-shard0-20260509.json`

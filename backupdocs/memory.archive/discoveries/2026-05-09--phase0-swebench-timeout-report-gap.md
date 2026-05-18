---
namespace: discoveries
created: 2026-05-09T22:42:48Z
updated: 2026-05-09T22:42:48Z
status: active
severity: high
tags: phase0, swebench, timeout, oracle, fsv, root-cause
---

# Phase 0 SWE-bench Timeout Report Gap

## What Happened

During production shard 0, mutation category `subtle_flip`, SWE-bench instance
`django__django-15789` timed out at the configured per-instance timeout:

```text
Timeout error: 1800 seconds exceeded.
```

The harness wrote the real per-instance `test_output.txt` but did not write the
expected per-instance `report.json`. The old bridge treated missing
`report.json` as an unclassifiable I/O failure, aborting the whole batch after
the harness had produced enough source-of-truth data to classify the outcome.

## Root Cause

The SWE-bench report producer is not guaranteed to emit `report.json` when a
worker times out. The durable source of truth for this failure mode is the
instance artifact directory:

```text
<run_root>/logs/run_evaluation/<run_id>/<model>/<instance_id>/
```

Specifically, `test_output.txt` can contain the official timeout marker even
when `report.json` is absent.

## Durable Fix

`crates/context-graph-mejepa-corpus/src/swebench.rs` now reads
`report.json` first. If it is missing, it reads `test_output.txt` and only when
that file contains the SWE-bench timeout marker does it persist a deterministic
timeout `report.json` in the same artifact directory.

The persisted timeout report:

- uses the official SWE-bench JSON shape keyed by `instance_id`;
- uses the exact harness schema key `patch_is_None`;
- sets `resolved=false`;
- records one failing test id named
  `<instance_id>::__swebench_timeout__`;
- includes `contextgraph_timeout_synthetic=true` and
  `contextgraph_timeout_seconds=<configured timeout>`.

This is not a fallback. It is a fail-closed classification of a real harness
timeout artifact, and it still errors if `test_output.txt` is missing or lacks
the timeout marker.

## Evidence

Before:

```text
test_output_exists=true
report_exists=false
test_output_timeout_marker=true
```

After:

```text
report_exists=true
contextgraph_timeout_synthetic=true
resolved=false
failure_id=django__django-15789::__swebench_timeout__
```

The real-derived report lives at:

```text
/tmp/contextgraph-mejepa-corpus-prod-shard0-20260509/swebench-runs/logs/run_evaluation/mejepaPhase0ProdS0-subtle_flip/mejepa-subtle_flip/django__django-15789/report.json
```

## Why It Matters

Timeouts are meaningful oracle outcomes for ME-JEPA training. They should be
persisted and read back as explicit failures, not allowed to destroy the whole
corpus run and not silently dropped from the dataset.

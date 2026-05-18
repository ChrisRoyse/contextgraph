---
namespace: discoveries
agent_id: codex
created: 2026-05-09T07:23:00Z
updated: 2026-05-09T07:23:00Z
timestamp: 2026-05-09T07:23:00Z
status: active
severity: medium
tags: swebench, phase0, docker, cache-hygiene, image-naming, fsv
---

# SWE-bench Lite Cache Hygiene and Image Naming

## What Happened

Official SWE-bench Lite setup now works from an isolated venv at
`/home/cabdru/.cache/contextgraph-swebench/venv` and Docker Desktop through
the `desktop-linux` context. The official dataset
`princeton-nlp/SWE-bench_Lite:test` reads back as exactly 300 instances and
matches `tasks/swebench-lite/*.json`.

The local Docker cache invariant was corrected. Phase 0 readiness should not
require all 300 official images to be pre-pulled. The correct ready state is
only the currently needed image(s), with no stale or duplicate SWE-bench image
refs. Current cache:

- Expected and present:
  `swebench/sweb.eval.x86_64.sympy_1776_sympy-20590:latest`
- SWE-bench image count: 1
- Unexpected SWE-bench images: none
- Duplicate SWE-bench image ids: none
- Duplicate per-instance refs: none

The official remote image naming rule is from SWE-bench
`TestSpec.instance_image_key`: remote namespace image keys replace `__` with
`_1776_`. Example: `sympy__sympy-20590` maps to
`swebench/sweb.eval.x86_64.sympy_1776_sympy-20590:latest`.

## Why It Matters

Pre-pulling all 300 Lite images would consume large Docker Desktop storage and
is not required for a readiness gate. It also creates a wider stale-cache
surface. The operational gate should fail closed on dirty cache state while
allowing batch runs to pull only the images they actually need.

## Evidence

- Preflight source of truth:
  `/tmp/contextgraph-swebench-lite-fsv/preflight-evidence.json`
- Cache source of truth:
  `/tmp/contextgraph-swebench-lite-fsv/image-status-evidence.json`
- Official gold smoke source of truth:
  `/tmp/contextgraph-swebench-lite-fsv/gold-smoke-evidence.json`
- Gold smoke run root:
  `/tmp/contextgraph-swebench-lite-fsv/gold-smoke/contextgraph-cache-clean-gold-smoke-20260509021900/`

## Related

- `scripts/swebench_lite_ops.py`
- `scripts/swebench_lite_ops/`
- `docs/ruvectorfindings/sitrep/02_shipped/phase0_oracle_and_store.md`
- `docs/ruvectorfindings/sitrep/03_deferred/phases/phase0.md`

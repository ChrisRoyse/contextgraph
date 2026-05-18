---
namespace: journal
agent_id: codex
created: 2026-05-09T07:23:00Z
updated: 2026-05-09T07:23:00Z
timestamp: 2026-05-09T07:23:00Z
status: active
tags: swebench, phase0, docker, success, fsv
---

# Success: SWE-bench Lite Cache-Clean Gold Smoke

## What Happened

Ran the official SWE-bench Lite harness against the real task
`sympy__sympy-20590` with `predictions_path=gold` through Docker Desktop.
The run completed and resolved the task.

## Source of Truth

- Overall report:
  `/tmp/contextgraph-swebench-lite-fsv/gold-smoke/contextgraph-cache-clean-gold-smoke-20260509021900/gold.contextgraph-cache-clean-gold-smoke-20260509021900.json`
- Instance report:
  `/tmp/contextgraph-swebench-lite-fsv/gold-smoke/contextgraph-cache-clean-gold-smoke-20260509021900/logs/run_evaluation/contextgraph-cache-clean-gold-smoke-20260509021900/gold/sympy__sympy-20590/report.json`
- Cache evidence:
  `/tmp/contextgraph-swebench-lite-fsv/image-status-evidence.json`

## Result

- Completed instances: 1
- Resolved instances: 1
- Error instances: 0
- Resolved ids: `sympy__sympy-20590`
- FAIL_TO_PASS success: `test_immutable`
- SWE-bench image count after run: 1
- Unexpected SWE-bench images after run: none
- Duplicate SWE-bench image ids after run: none

## Lesson

The readiness gate should verify a clean, minimal cache plus one official gold
smoke. Full Phase 0 completion is blocked by corpus generation, not by missing
local pre-cached images.

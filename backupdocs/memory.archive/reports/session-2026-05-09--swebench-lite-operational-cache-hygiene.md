---
namespace: reports
agent_id: codex
created: 2026-05-09T07:23:00Z
updated: 2026-05-09T07:23:00Z
timestamp: 2026-05-09T07:23:00Z
status: active
tags: report, session, swebench, phase0, docker, cache-hygiene, fsv
---

# Session Report: 2026-05-09 — SWE-bench Lite Operational Cache Hygiene

## Task

Set up and verify SWE-bench Lite operational readiness for Phase 0, correct the
Docker image-cache policy, avoid duplicate/stale images, and update the sitrep.

## Outcome: partial

The official SWE-bench Lite harness and Docker Desktop path are operational for
one real official task. The local SWE-bench Docker cache is clean and minimal:
one expected image, no stale SWE-bench images, and no duplicate SWE-bench image
ids or per-instance refs. Phase 0 is still not complete because the 2,400-entry
corpus generation/persistence batch has not been run.

## Summary

Created a fail-closed operational CLI at `scripts/swebench_lite_ops.py` with a
split helper package under `scripts/swebench_lite_ops/`. It verifies the
official dataset/manifests, Docker Desktop state, remote image naming, local
cache hygiene, image pulls, cache pruning, and one official gold smoke.

Corrected the bad assumption that readiness means 300 local images. The right
policy is on-demand images for the current smoke or batch slice. Dirty cache
state now fails with `SWEBENCH_CACHE_NOT_CLEAN`; unknown tasks and incomplete
manifest directories fail with explicit error codes and evidence files.

## Files Created or Modified in the Repo

- `scripts/swebench_lite_ops.py` — small CLI entrypoint.
- `scripts/swebench_lite_ops/` — fail-closed SWE-bench Lite ops implementation.
- `docs/ruvectorfindings/sitrep/01_status_overview.md` — corrected Phase 0 operational status.
- `docs/ruvectorfindings/sitrep/README.md` — updated high-level Phase 0 state.
- `docs/ruvectorfindings/sitrep/02_shipped/phase0_oracle_and_store.md` — documented official smoke and cache hygiene.
- `docs/ruvectorfindings/sitrep/03_deferred/phases/phase0.md` — removed all-300-image pre-cache blocker wording.
- `docs/ruvectorfindings/sitrep/04_implementation_plans/plan_phase0_docker_oracle.md` — updated runbook for on-demand pulls and cache hygiene.

## Files Written or Updated in ./memory/

- `memory/discoveries/2026-05-09--swebench-lite-cache-hygiene-and-image-naming.md`
- `memory/journal/2026-05-09--success-swebench-lite-cache-clean-gold-smoke.md`
- `memory/reports/session-2026-05-09--swebench-lite-operational-cache-hygiene.md`

## Discoveries

- Official remote SWE-bench images replace `__` with `_1776_` in namespaced Docker refs.
- The current correct local SWE-bench cache is one image:
  `swebench/sweb.eval.x86_64.sympy_1776_sympy-20590:latest`.
- Full Phase 0 completion needs the corpus generation batch, not a 300-image pre-pull.

## Failures

- Initial ops script exceeded the repo's 500-line file convention. Fixed by
  splitting it into helper modules while keeping the same public command.
- The earlier "299 images missing" interpretation was too strict. Corrected
  docs and tooling to fail on stale/duplicate cache state instead.

## Successes

- `python3 scripts/swebench_lite_ops.py preflight` passed.
- `python3 scripts/swebench_lite_ops.py image-status --require-clean` passed.
- `python3 scripts/swebench_lite_ops.py gold-smoke --run-id contextgraph-cache-clean-gold-smoke-20260509021900` passed with resolved=true.
- `cargo test -p context-graph-mejepa-corpus` passed: 59 tests.
- `python3 -m py_compile scripts/swebench_lite_ops.py scripts/swebench_lite_ops/*.py` passed.

## Edge Cases

- Empty expected cache rejected with `SWEBENCH_CACHE_NOT_CLEAN`.
- Unknown task id rejected with `SWEBENCH_UNKNOWN_INSTANCE_ID`.
- Empty task manifest directory rejected with `SWEBENCH_TASK_MANIFEST_COUNT_MISMATCH`.

## Open Blockers

- Phase 0 corpus generation command/batch is still not complete. Severity:
  high for doc-09 completion, low for Docker readiness.

## Resume Here Next Session

- Start by reading:
  `memory/discoveries/2026-05-09--swebench-lite-cache-hygiene-and-image-naming.md`.
- Run `python3 scripts/swebench_lite_ops.py image-status --require-clean`
  before any new SWE-bench batch.
- Next priority: wire/run the 5-task × 8-category batch, pulling only the
  needed task images for that slice, then verify persisted oracle verdicts.

## Confidence: high

High for official SWE-bench Lite/Docker readiness and cache hygiene. Medium for
full Phase 0 because corpus generation is still pending.

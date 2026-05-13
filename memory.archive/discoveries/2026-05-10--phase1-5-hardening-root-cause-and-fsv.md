---
namespace: discoveries
created: 2026-05-10T17:40:19-05:00
updated: 2026-05-10T17:40:19-05:00
status: active
tags: mejepa, phase1-5, hardening, fsv, utml, delta-xi, docs
---

# Phase 1-5 Hardening Root Cause And FSV

## What Happened

The Phase 1-5 hardening pass found that most crate-level ME-JEPA surfaces were already implemented and had FSV harnesses, but one safety-critical UTML invariant was not enforced at runtime: `compute_delta_xi` accepted non-zero `target_collapse` and folded it into an arithmetic mean.

That contradicted the ME-JEPA architectural invariant that target instruments are frozen and cannot collapse or receive gradients. A non-zero target collapse is now treated as a gradient-leak failure, not as a weak training signal.

## Source Of Truth Evidence

The post-fix FSV source-of-truth artifacts are:

- `/tmp/contextgraph-mejepa-train-fsv/edge-case-evidence.json` — target-collapse rejection, chain tamper detection, empty holdout rejection.
- `/tmp/contextgraph-mejepa-train-fsv/utml-l-step-evidence.json` — `L_step` and `delta_xi` readback with `target_collapse=0.0`.
- `/tmp/contextgraph-mejepa-train-fsv/training-fsv-summary.json` — aggregate training FSV summary.
- `/tmp/contextgraph-mejepa-train-fsv/training-cert-chain.json` — 100,000-step clean chain verification.
- `/tmp/contextgraph-mejepa-infer-fsv/verify-evidence.json` and `/tmp/contextgraph-mejepa-infer-fsv/edge-case-audit.json` — inference FSV.
- `/tmp/contextgraph-mejepa-self-healing-fsv*` — hard, soft, catastrophic, and invalid self-healing drills.
- `/tmp/contextgraph-ast-chunker-multilang-fsv/aggregate-evidence.json` — 11-language AST chunking FSV.
- `/mnt/d/contextgraph/fsv/contextgraph-mejepa-embedders-fsv/embedders_fsv_evidence.json` — embedder registry/artifact/routing FSV.
- `/tmp/contextgraph-mejepa-instruments-fsv/phase1-extended/phase1-extended-evidence.json` — phase 1 instrument panel FSV.

## Why It Matters

The critical invariant is now fail-closed and observable with a stable error code, `MEJEPA_INSTRUMENT_GRADIENT_LEAK`. Future agents should not relax this into a warning, fallback, or best-effort path. If non-zero target collapse appears, the correct behavior is to abort the training step before writing a certificate.

The docs were also refreshed to distinguish what exists from what remains pending. Important remaining blindspots are:

- Phase 1b verifies embedder artifacts, digests, VRAM, and routing, but does not yet execute a neural forward pass through every registered model.
- TCT is shipped at panel level; chunk-level centroid construction against persisted AST chunk IDs remains pending.
- `compute_delta_p`, `compute_delta_k`, and `compute_delta_omega` still use simplified component carriers relative to the richer UTML roadmap.
- Phase 8 official evaluation and the full 300-task SWE-bench measurement are still pending.
- Phase 6/7 orchestration capabilities remain outside the Phase 1-5 completion claim.

## Related

- `docs/ruvectorfindings/15_PHASE_1_5_HARDENING_FSV_2026_05_10.md`
- `memory/blockers/2026-05-10--compute-delta-xi-missing-target-collapse-assertion.md`
- `memory/blockers/2026-05-10--mejepa-chain-integrity-broken.md`
- Tags: mejepa, phase1-5, fsv, hardening

---
namespace: reports
created: 2026-05-10T17:40:19-05:00
updated: 2026-05-10T17:40:19-05:00
status: active
tags: report, session, mejepa, phase1-5, hardening, fsv, docs
---

# Session Report: 2026-05-10 — Phase 1-5 Hardening FSV

## Task

Verify and harden ME-JEPA Phases 1-5, perform FSV/manual edge testing, research implementation best practices, and update docs with what exists, what remains, and critical blindspots.

## Outcome

Complete for the Phase 1-5 hardening scope. The work found and fixed one safety-critical root cause in `compute_delta_xi`, re-ran source-of-truth FSV harnesses, and refreshed the ruvectorfindings docs/specs.

## Summary

The main root cause was that `compute_delta_xi` treated `target_collapse` as an ordinary component in an arithmetic mean. That weakened the central ME-JEPA invariant that target instruments are frozen and collapse-immune. The fix makes non-zero or non-finite target collapse fail closed with `MEJEPA_INSTRUMENT_GRADIENT_LEAK`, maps that UTML error into the trainer error taxonomy, and adds regression tests.

The FSV pass verified happy paths and edge cases by reading persisted evidence files after the trigger, not by trusting return values. The docs now state that Phases 1-5 are crate-level operational with important caveats: per-embedder neural forward execution, chunk-level TCT, full UTML roadmap items, and Phase 8 official evaluation remain pending.

## Files Created or Modified in the Repo

- `crates/context-graph-mejepa-train/src/learning_signal.rs` — hardened `compute_delta_xi`, added gradient-leak error mapping and regression tests.
- `crates/context-graph-mejepa-train/examples/train_edge_cases_fsv.rs` — updated target-collapse edge case to fail before certificate construction/write.
- `crates/context-graph-mejepa-train/examples/train_full_fsv.rs` — hardened JSON FSV readback to compare exact persisted bytes before parsing.
- `docs/ruvectorfindings/15_PHASE_1_5_HARDENING_FSV_2026_05_10.md` — new FSV/hardening report.
- `docs/ruvectorfindings/00_INDEX.md` through `14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md` — updated shipped status, links, and blindspots.
- `docs/ruvectorfindings/specs/*` and task docs — updated implementation-plan/spec status for Phase 1-5 and UTML-006.
- `memory/blockers/2026-05-10--compute-delta-xi-missing-target-collapse-assertion.md` — marked resolved with evidence.
- `memory/blockers/2026-05-10--mejepa-chain-integrity-broken.md` — marked resolved with clean-chain and tamper-detection evidence.

## Files Written or Updated in ./memory/

- `discoveries/2026-05-10--phase1-5-hardening-root-cause-and-fsv.md` — durable discovery summary.
- `journal/2026-05-10--success-phase1-5-hardening-fsv.md` — success/evidence note.
- `reports/session-2026-05-10--phase1-5-hardening-fsv-doc-refresh.md` — this report.
- `blockers/2026-05-10--compute-delta-xi-missing-target-collapse-assertion.md` — resolution added.
- `blockers/2026-05-10--mejepa-chain-integrity-broken.md` — resolution added.

## Decisions Made

- Non-zero or non-finite `target_collapse` is a fail-closed gradient-leak condition, not a degraded score. Rationale: the architecture requires frozen target instruments. File: `memory/blockers/2026-05-10--compute-delta-xi-missing-target-collapse-assertion.md`.
- Phase 1-5 completion claims must remain caveated by source-of-truth evidence. Rationale: docs previously blurred crate-level implementation with full neural/evaluation coverage. File: `docs/ruvectorfindings/15_PHASE_1_5_HARDENING_FSV_2026_05_10.md`.

## Discoveries

- Phase 1-5 is operational at the crate/FSV-harness level, but the strongest remaining blindspots are per-embedder neural forward verification, chunk-level TCT, fuller UTML primitives, and official Phase 8 evaluation. File: `memory/discoveries/2026-05-10--phase1-5-hardening-root-cause-and-fsv.md`.
- `train_full_fsv` needed byte-exact persisted JSON comparison; parsing and reserializing floats can produce cosmetic precision differences. Impact: future FSV should compare persisted bytes first when byte identity is the SoT.

## Failures

- Initial workspace-wide `cargo fmt --all` was not used as the final formatter gate because the workspace had concurrent/unrelated crate membership churn. Lesson: use focused package formatting for scoped fixes unless the workspace membership is stable.
- `train_full_fsv` initially failed on float reserialization mismatch. Lesson: persisted-file FSV should compare exact bytes, then parse only for semantic validation.

## Successes

- `compute_delta_xi` now rejects target collapse with `MEJEPA_INSTRUMENT_GRADIENT_LEAK`.
- Edge-case FSV proves no training cert is written when target collapse is non-zero.
- Clean chain and synthetic tamper paths were both verified.
- Docs and specs now point to the hardening report and list remaining critical blindspots.

## Open Blockers

- None for the Phase 1-5 hardening scope.
- Remaining work is tracked as pending capability, not a blocker: per-embedder neural forward FSV, chunk-level TCT over persisted AST chunks, expanded UTML primitives, Phase 6/7 orchestration, and Phase 8 official evaluation.

## Tests / Build

- Tests added: focused `learning_signal` regression tests for ΔΞ formula and target-collapse rejection.
- Tests passing: yes, scoped to the hardened surfaces.
- Build status: partial by design; focused package format/tests and FSV harnesses passed.

Commands run:

- `CUDA_COMPUTE_CAP=120 cargo test -p context-graph-mejepa-train learning_signal -- --nocapture`
- `CUDA_COMPUTE_CAP=120 cargo run -p context-graph-mejepa-train --example train_edge_cases_fsv`
- `CUDA_COMPUTE_CAP=120 cargo run -p context-graph-mejepa-train --example train_full_fsv`

## Resume Here Next Session

- Start by reading `docs/ruvectorfindings/15_PHASE_1_5_HARDENING_FSV_2026_05_10.md` and `memory/discoveries/2026-05-10--phase1-5-hardening-root-cause-and-fsv.md`.
- Watch out for the distinction between crate-level Phase 1-5 FSV and full end-goal evaluation.
- Priority: per-embedder neural forward FSV and chunk-level TCT are the next highest-value blindspot closures.
- Open question: whether Phase 6/7 orchestration should be hardened next or Phase 8 official evaluation should become the forcing function.

## Confidence

High for the scoped code fix and FSV evidence paths. Medium for whole-project completion because Phase 6/7/8 and several capability blindspots are explicitly outside the completed Phase 1-5 hardening scope.

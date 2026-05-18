---
namespace: reports
agent_id: agent-codex
created: 2026-05-13T17:57:22Z
updated: 2026-05-13T17:57:22Z
status: active
tags: phase-g,catastrophic-event,fsv,github-174
---

# Session Report: 2026-05-13 — Catastrophic Event Coverage Fixture

## Task: Fix #174 catastrophic_event test loses promotion after reopen
## Outcome: complete

## Summary
`cargo test -p context-graph-mejepa` failed because `TASK-TEST-004` still seeded 750 covered / 250 uncovered drift samples. That made the deterministic promotion winner land at 93% empirical coverage after the 18-point retrain lift, which the newer fail-closed promotion recalibration gate correctly rejects because the allowed band is 88-92% around the 90% target.

Adjusted the fixture to 720 covered / 280 uncovered samples. The scenario remains catastrophic at 72% source coverage, while the promoted candidate lands at the 90% target and survives reopen. The full `context-graph-mejepa` crate test suite now passes.

## Files Created or Modified in the Repo
- `tests/e2e/catastrophic_event.rs` — updated catastrophic drift fixture coverage from 75% to 72% and shared the expected metric through a helper.

## Files Written or Updated in ./memory/
- `reports/session-2026-05-13--catastrophic-event-coverage-fixture.md` — this report.

## Decisions Made
- **Approach chosen**: update the fixture data, not the promotion gate.
- **Considered alternatives**:
  1. Widen the 88-92% promotion coverage band — rejected because #100 intentionally made promotion recalibration fail closed.
  2. Mark the test ignored — rejected because full crate tests must stay meaningful.
- **Why this one wins**: it preserves catastrophic drift while aligning the synthetic winner with the current production gate.
- **What changes the decision**: if the promotion gate target changes from 90% or the deterministic retrain lift changes.

## Discoveries
- The failure message blamed reopen survival, but the status SoT showed the promote ticker failed first with `MEJEPA_CONFORMAL_COVERAGE_OUT_OF_BAND`.

## Failures
- Initial full `cargo test -p context-graph-mejepa` failed in `catastrophic_event::tests::task_test_004_catastrophic_event_fsv_passes`. The root cause was stale fixture coverage against the newer promotion recalibration band.

## Successes
- Post-commit catastrophic FSV passed with `build_release_sha=170097fe`, `all_passed=true`, and `readback_equal=true`.
- Full `cargo test -p context-graph-mejepa` now passes.

## Open Blockers
- None for #174.

## Tests / Build
- Tests added: none; repaired existing FSV-backed integration test.
- Tests passing: yes.
- Build status: passing (`cargo build --release -p context-graph-mejepa`).
- Clippy: clean (`cargo clippy -p context-graph-mejepa --no-deps --tests --examples -- -D warnings`).

## FSV evidence
- `/mnt/d/contextgraph/fsv/phase-f-catastrophic-event-fsv/catastrophic_event_fsv.json` — sha256:1c42afca178bae4302a4b243485dcb2f81ac71a01701de7385b4ed753604dfdf

## GitHub issues touched
- #174 — claimed/commented/closed — https://github.com/ChrisRoyse/contextgraph/issues/174#issuecomment-4443860960

## Resume Here Next Session
- Start by reading: open Phase G queue, with #112 as the next p2 DAG task unless a higher-priority issue appears.
- Watch out for: catastrophic-event fixture coverage must keep winner empirical coverage inside 88-92%.
- Priority: continue Phase G issue closure.
- Open question: none.

## Confidence: high
The original failing command now passes after the fixture correction, with source-of-truth FSV readback and full crate test confirmation.

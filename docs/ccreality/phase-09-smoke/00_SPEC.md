# Phase 9 — Specification

## Objectives

1. Create `scripts/cc/tests/smoke.sh` implementing the 8-layer smoke from [../17_SMOKE_TEST_PLAN.md](../17_SMOKE_TEST_PLAN.md).
2. Run the smoke; iterate until all 8 layers pass.
3. Capture any operator-visible issues during the smoke run as new entries in [../20_OPEN_QUESTIONS.md](../20_OPEN_QUESTIONS.md).

## Scope

Files:
- `scripts/cc/tests/smoke.sh` — NEW.
- `docs/ccreality/SMOKE_RESULTS_<date>.md` — NEW (created automatically by the smoke script).

Out of scope: FSV (Phase 10).

## The 8 layers

| Layer | Tests |
|-------|-------|
| 0 | Environment (claude, jq, sha256sum present) |
| 1 | Engine builds and `reality-loop smoke` runs |
| 2 | MCP server starts and responds |
| 3 | Claude Code sees the cgreality MCP server |
| 4 | Hooks fire and inject context |
| 5 | `/ccreality-smoke` skill drives the full loop |
| 6 | Drift-detection smoke |
| 7 | Reality-boundary deny enforcement |
| 8 | Recommendation schema validation |

Layers 0–4 verify the wiring. Layer 5 is the end-to-end happy path. Layers 6–8 are negative-control checks.

## Deliverables

- Functioning `scripts/cc/tests/smoke.sh` that exits 0 when all layers pass.
- A `SMOKE_RESULTS_<date>.md` file recording the run.

## AI-verifiable end-state

See [04_VERIFICATION.md](04_VERIFICATION.md). One clause:
- `bash scripts/cc/tests/smoke.sh` exits 0.

## Time budget

1–2 hours. The smoke script itself is straightforward (~150 lines). The time goes into iterating on failures revealed during smoke run — likely encountering one or two issues from earlier phases (cargo flag mismatch, missing dependency, hook timing) and fixing them.

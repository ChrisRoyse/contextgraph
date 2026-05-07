# Phase 12 — Specification

## Objectives

1. Run one real SWE-bench Lite instance through the engine to confirm production wiring.
2. Run one real `/ccreality-iterate` to confirm the optimizer side works end-to-end.
3. Record the final production verification/cutover commit on `ccreality-cutover`.

## Scope

- One SWE-bench Lite instance (operator's choice). Recommendation: pick a known-easy one for first light.
- One run of `target/release/reality-loop attempt --task-manifest ... --official-eval-each-attempt`.
- One run of `claude --print "/ccreality-iterate" --model opus`.
- One final Phase 12 `git commit` on `ccreality-cutover` after the real attempt and optimizer turn are verified.

Out of scope:
- Merging to `main` (operator decision).
- CI integration (separate work).
- Multi-instance batch (operator does this manually after Phase 12).

## Deliverables

- `/tmp/ccreality-prod-1/` populated with one real attempt + one optimizer recommendation.
- `ccreality-cutover` contains the verified incremental cutover commits plus a final Phase 12 commit with the production verification record.

## AI-verifiable end-state

See [04_VERIFICATION.md](04_VERIFICATION.md). Five clauses.

## Time budget

1 hour:
- Task manifest setup: 15 min.
- Engine run (incl Docker pull): 10–30 min depending on instance.
- Optimizer turn: 5–10 min.
- Verification + commit: 15 min.

## What success looks like

The recommendation artifact has a `status` field that is one of `changed`, `no_change`, `blocked`, `error`. Any of these is a successful Phase 12 outcome — the cutover is verified.

`changed` means the optimizer actually edited the harness. The change should be inspected.
`no_change` means the optimizer didn't think an edit was warranted (often correct for first light).
`blocked` means a path-policy or verification gate fired (good — defensive layers work).
`error` means something is wrong; investigate.

## Failure handling

If Phase 12 fails (e.g., recommendation never written, MCP server crashes, hooks misfire), do NOT create the Phase 12 commit. Preserve the failed artifacts for diagnosis, remove only generated Phase 12 scratch artifacts after they are recorded, fix the offending phase, re-verify, and re-attempt Phase 12. Do not use broad restore commands that could discard unrelated operator changes.

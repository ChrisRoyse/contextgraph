---
task_id: TASK-PREDICT-001
title: UserPromptSubmit prediction-injection hook
phase: A
status: open
blocked_by: [TASK-FLYWHEEL-001]
blocks: []
effort: 1d
spec_refs: [SPEC-PREDICT.md REQ-PREDICT-12..13, TECH-PREDICT.md §6]
---

## Verification contract

Inherits from [`../../specs/FSV-PROTOCOL.md`](../../specs/FSV-PROTOCOL.md). SoT is the rendered markdown file at the FSV path below + the actual injection of that text into the Claude Code prompt context (verifiable by inspecting the next conversation turn's system-reminder block). `ASSUME THE TASK IS WRONG` — verify `mejepa_predict_latest` is registered in `crates/context-graph-mcp/src/handlers/tools/mejepa_tools.rs` before assuming the queryable surface exists; verify `.claude/settings.json` accepts `hooks.UserPromptSubmit` array additions without breaking existing hooks.

## Goal

Add a hook script that fires on UserPromptSubmit, queries the daemon's `mejepa_predict_latest` for the current session, and renders the latest `RealityPrediction` as a ≤ 4 KB markdown injection block per `15_PREDICTION_TAXONOMY.md §8`. The block is appended to the agent's prompt context as a `<user>` message addendum. **Without this hook, predictions are computed but never reach the agent.**

## Inputs

- `crates/context-graph-mcp/src/handlers/tools/mejepa_tools.rs` — `mejepa_predict_latest` already shipped
- `docs/futurebuild/15_PREDICTION_TAXONOMY.md §8` — markdown rendering format
- `.claude/settings.json` — current hook wiring
- `scripts/cc/hook_user_prompt_submit_*.sh` — existing UserPromptSubmit hooks (don't conflict)
- `docs/futurebuild/specs/technical/TECH-PREDICT.md` §6 (the script template)

## Outputs

- `scripts/cc/hook_user_prompt_submit_inject_prediction.sh` — NEW
- `scripts/cc/render_prediction_markdown.py` — NEW (Python renderer per TECH-PREDICT.md §7)
- `.claude/settings.json` — extended `hooks.UserPromptSubmit` list to include the new script
- FSV: `scripts/cc/test_inject_prediction.sh` — round-trip test

## Definition of done

1. With the daemon running and at least one prediction in `CF_MEJEPA_LIVE_PREDICTIONS`, calling the hook script outputs a markdown block.
2. Block is ≤ 4 KB (per REQ-PREDICT-13); when full content exceeds, Q4 concerns are truncated to top-5 per category.
3. With no prediction available, hook outputs nothing (silent exit code 0).
4. With the daemon down, hook outputs nothing (silent exit code 0; the agent is not blocked).
5. Markdown contains the exact sections in order from `15_PREDICTION_TAXONOMY.md §8`: verdict + interval, predicted failure modes, predicted edge cases, predicted latent bugs, predicted tech debt, predicted dead code, predicted perf/security/accuracy/cost, reasoning class, claim reconciliation, reality impact, "to respond" line.
6. Hook is wired in `.claude/settings.json` under `hooks.UserPromptSubmit`.
7. Test fixture (`scripts/cc/test_inject_prediction.sh`) writes a synthetic `RealityPrediction` to a test session, runs the hook, asserts the rendered markdown matches the expected snapshot.

## FSV evidence path

`/mnt/d/contextgraph/fsv/mejepa-predict-fsv/injection.md` (the rendered markdown for a known synthetic prediction)

## Boundary cases

- Empty `RealityPrediction` (verdict=Pass, all Q4 vecs empty) → block contains "Verdict: Pass" + "predicted: none above threshold" for each Q4 section
- All 8 Q4 categories non-empty with > 5 entries each → top-5 truncation applied; "...truncated; use mejepa_predict_latest for full" footer added
- Markdown rendering produces exactly 4096 bytes → no truncation; final newline preserved
- `mejepa_predict_latest` returns error → hook silently exits 0 (don't crash the prompt)

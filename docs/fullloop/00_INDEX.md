# Full Reality Loop — Forward Plan

**Status:** active planning, post-cutover
**Date:** 2026-05-05
**Author:** Claude (Opus 4.7) for Chris Royse
**Branch context:** `ccreality-cutover` — phases 0–12 of the ccreality plan are complete. This directory plans phases 13–22 (the *forward* work) needed to make the meta-reality-loop operational, productive, and capable of driving the inner Gemma SWE actor toward verified resolved instances.

## What this directory is

`docs/ccreality/` proved that Claude Code can be wired into the ContextGraph reality engine via MCP, hooks, subagents, skills, and a structured-output recommendation. `docs/reality/` describes the long-term reality-loop research vision (auto-optimization, multi-model, training-data flywheel, DynamicJEPA integration). This directory connects them: it lists what is *built but not yet productive*, what is *designed but not yet built*, and the concrete next-phase plan to take the meta-loop from "smoke green" to "first official `resolved_ids` on a real SWE-bench Lite instance and a self-improving outer loop sustaining gains across attempts."

## What is already true (verifiable today)

Source of truth: `docs/ccreality/00_INDEX.md` "Status of this work" section.

- ccreality cutover phases 0–12 are complete on `ccreality-cutover`.
- `target/debug/reality-loop` runs `attempt | smoke | replay | verify | official-eval | targeted-test`.
- `context-graph-mcp --mode reality-loop` advertises only callable cgreality tools (28 in the lightweight registry).
- `.claude/{settings.json, loop.md, agents/cc-reality-*.md, skills/ccreality-*/}` and `.mcp.json` are committed and validated.
- Six `scripts/cc/hook_*.sh` hooks deliver SessionStart, UserPromptSubmit, PreToolUse Edit, PostToolUse Edit, Stop, PreCompact reality drips.
- Phase 12 ran a real SWE-bench Lite production attempt against `psf__requests-2317`. Outcome: failed closed at `RUST_REALITY_FINAL_EDIT_TOOL_FAILED` — the loop correctly refused to claim success because Gemma did not submit a valid candidate patch.
- One real `/ccreality-iterate` recommendation artifact exists at `/tmp/ccreality-prod-1/rust_reality_1778020383548106093_e525987d/tasks/psf__requests-2317/gemma4-e4b/attempts/attempt-000001/claude-code-optimizer/recommendation-turn-01.json`.

## What is *not* yet true (the forward gap)

- No SWE-bench Lite instance has been resolved end-to-end (`official_report.resolved_ids` empty).
- Gemma's inner-loop tool surface still produces invalid line-window edits (FSV-09 ambiguous-find, Phase-12 edit-tool-failure).
- The cross-model variant of the loop was retired; the production loop is single-model (`gemma4:e4b`). Phase 16 below is preserved for historical context only.
- Champion state, experiment registry, training-data export, curriculum gate, and DynamicJEPA reward integration are designed but not built.
- No production cron / external-watcher invocation has run unattended.
- The dashboard surfaces (gemmafrontend, scoreboard) do not yet visualize ccreality optimizer artifacts.

## Reading order

| # | Doc | Purpose |
|---|-----|---------|
| 00 | this file | navigation + scope |
| 01 | [01_CURRENT_STATE](01_CURRENT_STATE.md) | exact source-of-truth pointers for what runs today |
| 02 | [02_GAP_ANALYSIS](02_GAP_ANALYSIS.md) | what is missing for full meta-loop operation |
| 03 | [03_TARGET_ARCHITECTURE](03_TARGET_ARCHITECTURE.md) | end-state diagram (Claude Code outer optimizer + Gemma 4 inner SWE actor + auxiliary subsystems) |
| 04 | [04_PHASE_13_HARDENING](04_PHASE_13_HARDENING.md) | small bug fixes blocking smooth /loop runs |
| 05 | [05_PHASE_14_INNER_CAPABILITY](05_PHASE_14_INNER_CAPABILITY.md) | reach first verified `resolved_ids` |
| 06 | [06_PHASE_15_AUTORESEARCH_ENGINE](06_PHASE_15_AUTORESEARCH_ENGINE.md) | champion state + experiment registry |
| 07 | [07_PHASE_16_CROSS_MODEL](07_PHASE_16_CROSS_MODEL.md) | RETIRED — historical multi-model plan; the loop is now single-model (`gemma4:e4b`). |
| 08 | [08_PHASE_17_REWARD_SHADOW](08_PHASE_17_REWARD_SHADOW.md) | shadow reward spec optimization |
| 09 | [09_PHASE_18_TRAINING_EXPORT](09_PHASE_18_TRAINING_EXPORT.md) | verified-episode export + LoRA prep |
| 10 | [10_PHASE_19_CURRICULUM](10_PHASE_19_CURRICULUM.md) | dev / holdout / promotion split |
| 11 | [11_PHASE_20_DASHBOARD](11_PHASE_20_DASHBOARD.md) | operational scoreboard surfaces |
| 12 | [12_PHASE_21_PRODUCTION_OPS](12_PHASE_21_PRODUCTION_OPS.md) | external-watcher cron, cost cap, alerts |
| 13 | [13_PHASE_22_DYNAMICJEPA](13_PHASE_22_DYNAMICJEPA.md) | DynamicJEPA surprise → optimizer reward signal |
| 14 | [14_OPERATOR_RUNBOOK](14_OPERATOR_RUNBOOK.md) | how to actually start / drive / supervise the loop |
| 15 | [15_AI_VERIFIABLE_END_STATES](15_AI_VERIFIABLE_END_STATES.md) | per-phase commands whose pass = phase done |
| 16 | [16_RISKS](16_RISKS.md) | reward hacking, cost runaway, model drift, scope creep |
| 17 | [17_OPEN_QUESTIONS](17_OPEN_QUESTIONS.md) | undecided design points |
| 18 | [18_GLOSSARY](18_GLOSSARY.md) | shared vocabulary |

## Single-paragraph version

The cutover delivered the *plumbing* (engine, MCP, hooks, subagents, skills, schema). What is left is to (a) harden a small set of bugs surfaced by the smoke runtime, (b) make the inner loop actually capable of resolving an instance, (c) lift the outer loop from "one recommendation per attempt" to a genuine auto-research engine with champion state and experiment registry, (d) bring the second and third local models online with shared verified-failure memory, (e) close the data flywheel (verified episodes → training samples → LoRA → better inner loop), and (f) wire DynamicJEPA so that surprise/latent-state signals become first-class reward features. Each phase has an AI-verifiable end-state. The ccreality reality-shift principle (`docs/ccreality/01b_REALITY_SHIFT_PRINCIPLE.md`) remains the core invariant throughout.

## Cross-reference rules of the road

- Anything that contradicts `docs/ccreality/01b_REALITY_SHIFT_PRINCIPLE.md` is wrong by construction.
- The success predicate stays unchanged: official SWE-bench `resolved_ids` contains the target instance.
- The deny list in `config/ccreality_governed_paths.json` is sacred — `swebench/**`, `tests/oracle_**`, reward specs, verifiers, `.env` are never edited.
- All phase end-states are AI-verifiable per `15_AI_VERIFIABLE_END_STATES.md`. A phase is not done until its verification passes deterministically.
- This directory is forward-planning only. Implementation work updates `docs/ccreality/` (per-phase folders) and source code; planning updates here.

# ccreality — Claude Code as the Outer Reality-Loop Optimizer

**Status:** cutover in progress — Phases 0 and 1 complete; Phase 2 is next
**Date:** 2026-05-05
**Author:** Claude (this session) for Chris Royse
**Supersedes:** `docs2/CLAUDE_CODE_REALITY_LOOP_GUIDE.md` (kept for historical context),
`docs/reality/20_openai_outer_optimizer.md`, `docs/reality/21_grand_scale_reality_autoresearch.md`

## What this directory is

A complete planning package for ripping the **DeepSeek V4 Pro / OpenAI gpt-5-pro outer optimizer** out of the ContextGraph reality loop and replacing it with **Claude Code** — running not as a subprocess that ContextGraph spawns, but as the **operator** that drives ContextGraph through a deeply integrated **MCP surface** and a **continuous reality-drip protocol**.

This is the architecture the user described:

> "Hook a reality loop INTO Claude Code where it is constantly fed the reality it needs to be fed to be able to do the job it needs to complete."
>
> "Build this as all part of the model context protocol system of context graph and give claude code access to tools as well that allows it to analyze and interact with reality and alter reality itself."

## Reading order

| # | Doc | Purpose |
|---|-----|---------|
| 00 | this file | navigation |
| 01 | [ARCHITECTURE](01_ARCHITECTURE.md) | the meta reality loop, top-down |
| 01b | [REALITY_SHIFT_PRINCIPLE](01b_REALITY_SHIFT_PRINCIPLE.md) | **core invariant** — no alteration without current state, no alteration without a delta back into perception |
| 02 | [REALITY_DRIP_PROTOCOL](02_REALITY_DRIP_PROTOCOL.md) | every touchpoint where reality is fed into Claude Code |
| 03 | [MCP_REALITY_TOOLS_SPEC](03_MCP_REALITY_TOOLS_SPEC.md) | the analyze / interact / alter MCP tool surface (the main spec) |
| 04 | [EXCISION_PLAN](04_EXCISION_PLAN.md) | what we delete from `reality-loop.rs`, with line ranges |
| 05 | [REALITY_ENGINE_RESPONSIBILITY](05_REALITY_ENGINE_RESPONSIBILITY.md) | what stays in Rust after excision |
| 06 | [SETTINGS_JSON_SPEC](06_SETTINGS_JSON_SPEC.md) | `.claude/settings.json` and `.mcp.json` |
| 07 | [HOOKS_SPEC](07_HOOKS_SPEC.md) | every hook script with stdin/stdout JSON |
| 08 | [SUBAGENTS_SPEC](08_SUBAGENTS_SPEC.md) | `.claude/agents/cc-reality-*` |
| 09 | [SKILLS_SPEC](09_SKILLS_SPEC.md) | `.claude/skills/ccreality-*` |
| 10 | [CLAUDE_MD_PROJECT_MEMORY](10_CLAUDE_MD_PROJECT_MEMORY.md) | the project memory addendum |
| 11 | [LOOP_PROMPT](11_LOOP_PROMPT.md) | `.claude/loop.md` for operator mode |
| 12 | [PROMPT_CONTRACT](12_PROMPT_CONTRACT.md) | what every optimizer prompt receives |
| 13 | [STRUCTURED_OUTPUT_SCHEMA](13_STRUCTURED_OUTPUT_SCHEMA.md) | the JSON schema for the recommendation |
| 14 | [TRIGGER_AND_INVOCATION_POLICY](14_TRIGGER_AND_INVOCATION_POLICY.md) | when Claude Code wakes up |
| 15 | [COST_AND_MODEL_POLICY](15_COST_AND_MODEL_POLICY.md) | Sonnet vs Opus, budget caps |
| 16 | [PARALLEL_AGENT_POLICY](16_PARALLEL_AGENT_POLICY.md) | one writer + read-only critics |
| 17 | [SMOKE_TEST_PLAN](17_SMOKE_TEST_PLAN.md) | first-light verification |
| 18 | [FSV_PLAN](18_FSV_PLAN.md) | full-state-verification suite |
| 19 | [MIGRATION_RUNBOOK](19_MIGRATION_RUNBOOK.md) | execution order for the cutover |
| 20 | [OPEN_QUESTIONS](20_OPEN_QUESTIONS.md) | what remains undecided |
| 21 | [IMPLEMENTATION_PLAN](21_IMPLEMENTATION_PLAN.md) | **execution plan** — 13 phases, AI-verifiable end-states, dependency DAG, time estimates |
| 21+ | [phase-NN-shortname/](phase-00-foundation/) folders | per-phase task/spec/verification packets, one for each of the 13 phases |

## What changes in one paragraph

**Before:** Rust `reality-loop.rs` runs Gemma, then calls DeepSeek V4 Pro over HTTP for outer-loop optimization, parses the JSON response, applies its line-window edits to ContextGraph source, persists artifacts, re-attempts. The optimizer is a chat completion endpoint pretending to be a coding agent through a hand-rolled tool harness.

**After:** Rust `reality-loop.rs` becomes a pure **reality engine** that runs Gemma, runs official SWE-bench Docker eval, writes durable artifacts, and exits. **ContextGraph MCP server** exposes those artifacts plus the entire memory graph (search across 13 embedders, audit trails, provenance chains, causal links, harness transitions) plus a governed action surface (edit harness source, store/boost/forget memories, trigger new attempts, run targeted tests, run official eval). **Claude Code** is the operator: it runs against the project, hooks inject fresh reality at every step, MCP tools let it ask for any artifact at any moment and act on the system, and a structured-output recommendation closes each turn with a source-of-truth-tied summary.

## Core invariants we preserve

0. **Reality-shift principle (01b).** No alteration without current state. No alteration without a delta back into perception. The agent must always be able to answer "has reality changed under me since I last looked?".
1. **Durable artifacts are the source of truth.** Model text never decides anything; only readback from disk + git diff + cargo output + official SWE-bench `resolved_ids`.
2. **No editing benchmarks, evaluators, reward specs, verifiers, or `.env`.** Hooks enforce this deterministically — not "the model remembers."
3. **Every alter-reality MCP call writes an artifact and reads it back, AND appends a shift-log entry.** The pattern is `mutate → fsync → re-read → diff → log shift → return readback`.
4. **Event-triggered, not always-on.** Claude Code wakes up when ContextGraph emits a trigger artifact, does one optimization turn, then yields. No idle thinking.
5. **One source-editing optimizer.** Optional read-only critics. Never two writers racing on the same file.
6. **Pre-production: no backwards-compatibility shims.** Per project rule. Wipe DeepSeek/OpenAI cleanly.

## Status of this work

Current branch: `ccreality-cutover`.

Completed so far:
- **Phase 0 — Foundation:** branch `ccreality-cutover` created from base SHA `411443fdcbd8db4005cfe46d44f64f7698c2f11b`; rollback source of truth written to `/tmp/ccreality-cutover-base.sha`.
- **Phase 1 — Excision:** `crates/context-graph-cli/src/bin/reality-loop.rs` contains no `deepseek|openai|gpt-5|chatgpt` text and currently reads back at 7,203 lines; the Phase 1 completion log recorded 7,145 lines. `crates/context-graph-cli/src/governed_edit.rs` exists and is exported from the `context_graph_cli` library target.
- **Phase 1 verification:** `/tmp/phase-01-verify.log` ends with `Phase 1 verification: ALL PASS`; `/tmp/ccreality-phase-progress.log` has `phase-01: PASS`.
- **Phase 1 manual FSV artifacts:** `/tmp/ccreality-phase1-fsv-output/observations.json` records a real governed edit (`alpha/beta` → `alpha/gamma`), diff readback, command readback, and three edge-case failures.

Still pending:
- **Phase 2:** refactor remaining engine into explicit `reality-loop attempt|smoke|replay|verify` subcommands and write active runtime-root files.
- **Phase 3/4:** implement `context-graph-mcp --mode reality-loop` with analyze/interact/alter tools, using `context_graph_cli::governed_edit` for harness operations.
- **Phase 5:** add governed path and recommendation schema config files.
- **Phase 6–8:** add hooks, Claude scaffolding, and `CLAUDE.md` runtime addendum.
- **Phase 9–12:** smoke, full FSV, docs cleanup, first production attempt, and final cutover commit.

See [19_MIGRATION_RUNBOOK](19_MIGRATION_RUNBOOK.md) for the execution sequence.

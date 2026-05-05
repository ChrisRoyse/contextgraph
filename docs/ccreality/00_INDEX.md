# ccreality — Claude Code as the Outer Reality-Loop Optimizer

**Status:** cutover complete — Phases 0 through 12 are implemented and manually FSV'd; branch is ready for review
**Date:** 2026-05-05
**Author:** Claude (this session) for Chris Royse
**Supersedes:** model-integration details in `docs2/CLAUDE_CODE_REALITY_LOOP_GUIDE.md` and `docs/reality/21_grand_scale_reality_autoresearch.md` (both kept with status headers), plus `docs/reality/20_openai_outer_optimizer.md` (deleted).

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
| R12 | [PHASE12_RESULTS_2026-05-05](PHASE12_RESULTS_2026-05-05.md) | first production attempt, root-cause hardening, and final FSV evidence |

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
- **Phase 1 — Excision:** `crates/context-graph-cli/src/bin/reality-loop.rs` contains no `deepseek|openai|gpt-5|chatgpt` text and currently reads back at 7,990 lines; the Phase 1 completion log recorded 7,145 lines. `crates/context-graph-cli/src/governed_edit.rs` exists and is exported from the `context_graph_cli` library target.
- **Phase 1 verification:** `/tmp/phase-01-verify.log` ends with `Phase 1 verification: ALL PASS`; `/tmp/ccreality-phase-progress.log` has `phase-01: PASS`.
- **Phase 1 manual FSV artifacts:** `/tmp/ccreality-phase1-fsv-output/observations.json` records a real governed edit (`alpha/beta` → `alpha/gamma`), diff readback, command readback, and three edge-case failures.
- **Phase 2 — Engine subcommands:** `./target/debug/reality-loop --help` exposes `attempt|smoke|replay|verify`; `smoke`, `replay`, `verify`, and forced-failure paths wrote real artifacts.
- **Phase 2 verification:** `/tmp/phase-02-verify.log` ends with `Phase 2 verification: ALL PASS`; `/tmp/ccreality-phase-progress.log` has `phase-02: PASS`.
- **Phase 2 handoff source of truth:** successful smoke runtime root `/tmp/cgreality-phase02-ready`; active-root files read back as `/tmp/cgreality-phase02-ready` and `cgreality-smoke-phase02-ready`.
- **Phase 5 — Config files:** `config/ccreality_governed_paths.json`, `config/ccreality_recommendation_schema.json`, and `docs/ccreality/phase-05-config-files/sample_recommendation.json` exist and validate.
- **Phase 5 verification:** `/tmp/phase-05-verify.log` ends with `Phase 5 verification: ALL PASS`; `/tmp/ccreality-phase-progress.log` has `phase-05: PASS`.
- **Phase 3 — MCP foundation + analyze:** `context-graph-mcp --mode reality-loop` builds, lists the 11 ANALYZE `reality_*` tools, and reads the active smoke runtime artifacts through MCP calls.
- **Phase 3 verification:** `/tmp/phase-03-verify.log` ends with `Phase 3 verification: ALL PASS`; `/tmp/ccreality-phase-progress.log` has `phase-03: PASS`.
- **Phase 4 — MCP interact + alter + shift:** `alter.rs`, `interact.rs`, `shift_log.rs`, `optimizer.rs`, `lock.rs`, and `schema_validator.rs` are implemented. The MCP server shells out to `reality-loop`, enforces governed path policy, applies governed edits, appends shift-log records, writes harness transitions, and records optimizer decisions/recommendations into the real BlackBox SQLite ledger.
- **Phase 4 verification and FSV:** `cargo build -p context-graph-mcp --quiet`, `cargo test -p context-graph-mcp --lib reality_loop --quiet`, and the MCP tools-list probe pass. Manual FSV logs are `/tmp/ccreality-fsv-1777999742.log` and `/tmp/ccreality-phase4-ledger-fsv-1778000024.log`; the latter read back `agent_run|1` and `tool_call|2` from `/tmp/cgreality-phase04-ledger-fsv-1778000024/ledger.sqlite`.
- **Phase 5 hardening:** the governed-paths policy now uses the exact documented allow/deny contract, including `tests/oracle_**`; `path_policy.rs` compiles the CCReality path-pattern dialect directly and fails closed on missing env, unsupported policy version, malformed policy shape, and denied paths. The recommendation schema is strict Draft 2020-12 and validates the real sample recommendation.
- **Phase 5 full-state verification:** `/tmp/ccreality-phase5-fsv-1778001835.log` records the source-of-truth readbacks for `config/ccreality_governed_paths.json`, `config/ccreality_recommendation_schema.json`, `docs/ccreality/phase-05-config-files/sample_recommendation.json`, a real MCP-driven harness transition, a real optimizer recommendation artifact, shift-log entries, and a BlackBox SQLite ledger row.
- **Phase 6 — Hooks:** `scripts/cc/_common.sh`, all six `scripts/cc/hook_*.sh` scripts, and all six `scripts/cc/tests/hooks/test_*.sh` tests are implemented and executable. The PostToolUse hook records harness transitions through `context-graph-cli ccreality record-harness-transition`, which calls the Phase 4 MCP tool instead of direct-writing artifacts.
- **Phase 6 verification and FSV:** the canonical Phase 6 verification passed; `/tmp/ccreality-phase-progress.log` contains `phase-06: PASS`. Manual FSV log `/tmp/ccreality-phase6-fsv-1778003099.log` read back a real pre-edit state file, harness-transition artifact, shift-log row, hook log, recommendation artifact, and four edge-case outcomes.
- **Phase 7 — Claude scaffolding:** `.claude/settings.json`, `.mcp.json`, `.claude/loop.md`, five `cc-reality-*` agent files, and four `ccreality-*` skill files are implemented. The root `.mcp.json` is trackable and contains the healthy `cgreality` project MCP server; nested `.mcp.json` files remain ignored.
- **Phase 7 verification and FSV:** the canonical Phase 7 verification passed, including `claude mcp list` and a real `mcp__cgreality__reality_latest_root` call through `claude --print`. Manual FSV log `/tmp/ccreality-phase7-fsv-1778005930.log` read back settings, MCP config, governed paths, hook executables, agent/skill frontmatter, direct MCP tools/list, Claude Code MCP health, and four edge-case outcomes.
- **Phase 8 — CLAUDE.md addendum:** `CLAUDE.md` ends with the ccreality outer optimizer role contract, and the source block in `docs/ccreality/10_CLAUDE_MD_PROJECT_MEMORY.md` matches the appended block byte-for-byte.
- **Phase 8 verification and FSV:** the canonical Phase 8 verification passed, including a real `claude --print` project-memory smoke with tools disabled. Manual FSV log `/tmp/ccreality-phase8-fsv-1778007923.log` read back the project-memory file, source addendum spec, gitignore trackability, loaded Claude Code memory, and five edge-case outcomes.
- **Phase 9 — Smoke:** `scripts/cc/tests/smoke.sh` is the deterministic smoke harness for Layers 0-8. Latest clean run: `/tmp/cgreality-smoke-1778013189/smoke.log`; report: `/tmp/cgreality-smoke-1778013189/smoke-report.json`; results doc: `docs/ccreality/SMOKE_RESULTS_2026-05-05.md`.
- **Phase 10 — FSV:** `scripts/cc/tests/fsv.sh` passed with 39/39 cases PASS. Results doc: `docs/ccreality/FSV_RESULTS_2026-05-05.md`; run root: `/tmp/cgreality-fsv-1778016954`.
- **Phase 11 — Docs cleanup:** superseded legacy docs are removed or explicitly marked historical, `docs/reality/00_index.md` points to `docs/ccreality/`, and live navigation no longer links to deleted DeepSeek/OpenAI-era docs. Manual FSV log: `/tmp/ccreality-phase11-fsv-1778017778.log`.
- **Phase 12 — First production attempt:** real SWE-bench Lite instance `psf__requests-2317` ran through the engine in `/tmp/ccreality-prod-1`; real `/ccreality-iterate` wrote a valid optimizer recommendation; root-cause hardening fixed MCP tool advertisement, bounded summaries, fail-fast Python syntax probes, server-owned recommendation timestamps, and significant-location legacy-token scanning. Final FSV log: `/tmp/ccreality-phase12-fsv-20260505T225350Z.log`; results: [PHASE12_RESULTS_2026-05-05](PHASE12_RESULTS_2026-05-05.md).

See [19_MIGRATION_RUNBOOK](19_MIGRATION_RUNBOOK.md) for the execution sequence.

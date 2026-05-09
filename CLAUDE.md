# CLAUDE.md — ContextGraph Project Configuration

You are working in the **ContextGraph** repository. This file is loaded automatically into every Claude Code session that opens this directory. It is the canonical source for behavioral rules, project architecture, and the **ccreality outer reality-loop optimizer** role.

When this file conflicts with anything else, this file wins. When two parts of this file conflict, the later (more specific) section wins. The `./memory/context/constraints.md` file (SMAP) cross-references this file rather than duplicating it.

---

## 1. Project at a glance

ContextGraph is a Rust workspace that ships:

- **`context-graph-mcp`** — the MCP (Model Context Protocol) server. Speaks JSON-RPC over stdio to Claude Code, exposes the `mcp__cgreality__*` tool family, owns path-policy + edit-lock + ledger + shift-log machinery, and now fails closed on retired local reality-loop runner calls.
- **`context-graph-cli`** — the CLI surface for ContextGraph harness support, DynamicJEPA commands, governed readback helpers, and ME-JEPA implementation work. The legacy `reality-loop` binary that hosted the Qwen3.6-27B inner actor was removed in the 2026-05-09 ME-JEPA hardening pass.
- **Other crates**: storage, embedding pipelines, benchmark fixtures, ME-JEPA primitives (`context-graph-mejepa-corpus`, `context-graph-mejepa-instruments`, `context-graph-tier-compression`, `context-graph-mincut`, `context-graph-witness`, `context-graph-solver`). See `crates/`.

The active workstream is **ME-JEPA-Code** — the predictive reality compiler that takes (code change + context) and outputs a calibrated prediction of what the SWE-bench Docker oracle would say. **Claude Code (you) implements ME-JEPA + the surrounding ContextGraph reality substrate.** Each /loop turn ships one primitive end-to-end with FSV evidence per `docs/ruvectorfindings/specs/implementation-plan.md` and the task graph under `docs/ruvectorfindings/specs/tasks/`.

**11-language AST-aware atomic chunking (per `docs/ruvectorfindings/14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md`).** The workspace ships tree-sitter parsers for **11 languages**: Rust, Python, JavaScript, TypeScript, Go, Java, C, C++, C#, Ruby, PHP. The multi-language semantic extractor (`crates/context-graph-cli/src/commands/dynamicjepa.rs`) handles all 11 today; the AST-aware atomic chunker (`crates/context-graph-core/src/memory/ast_chunker.rs`) handles Rust today and is being extended to the remaining 10 as a Phase 1 deliverable. Every code-domain ME-JEPA instrument (E_AST, E_CFG, E_DataFlow, E_TypeGraph, E_Diff, E_Test) consumes AST-perfect chunks bounded by syntactic entities (function, class, impl, module-decl), targeting ~500 non-whitespace characters per Qodo / cAST methodology — never raw text or fixed-size windows.

The full design is in `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md` (master plan), `10_MEJEPA_PIVOT_2026_05_09.md` (post-pivot statement of architecture), and `docs/ruvectorfindings/specs/` (canonical executable blueprint, status, roadmap, and task graph). The legacy `docs/ccreality/` documents (21 files describing the pre-pivot Claude+Qwen two-loop) are retained as historical reference.

---

## 2. Behavioral rules (always enforced)

1. Do what has been asked; nothing more, nothing less.
2. NEVER create files unless absolutely necessary.
3. ALWAYS prefer editing an existing file to creating a new one.
4. NEVER proactively create documentation files (`*.md`) or READMEs unless explicitly requested.
5. NEVER save working files, ad-hoc tests, or scratch markdown to the root folder. Use the directory layout in §3.
6. Never continuously poll status after spawning a swarm — wait for results.
7. ALWAYS read a file before editing it.
8. NEVER commit secrets, credentials, or `.env` files.
9. After ANY correction from the operator, write the lesson into `./memory/decisions/` (if it's a locked rule) or `./memory/discoveries/` (if it's a finding) so future agents inherit it. SMAP §3.

---

## 3. Directory layout

```
./src/         — source code (use crate-relative paths instead in this Rust workspace)
./crates/      — Rust crates (the real source tree)
./tests/       — test files
./docs/        — design docs, ADRs, RFCs
./config/      — configuration files (governed paths, model pins, reward specs)
./scripts/     — utility scripts (cc/ holds Claude Code hook scripts)
./examples/    — example code
./tasks/       — workstream-specific task manifests (e.g. tasks/swebench-lite/*.json)
./memory/      — SMAP shared mind (eight namespace subdirs, see §6)
./agents/      — SMAP agent roster + role definitions
./.claude/     — Claude Code config: settings.json, agents/cc-reality-*.md, loop.md, skills/
```

Never write scratch files to the project root. Tests of interactive flows can live under `/tmp/`.

---

## 4. Architecture & coding standards

- Domain-Driven Design with bounded contexts.
- Files **under 500 lines** (split via the `file-length-manager` agent if exceeding).
- Typed interfaces for all public APIs.
- TDD London School (mock-first) for new code where mocking is sensible; integration tests against real backends where mocks would diverge from reality.
- Event sourcing for state changes that need replay.
- Validate input at system boundaries.

### Project config

| Knob | Value |
|------|-------|
| Topology | hierarchical-mesh |
| Max agents | 15 |
| Memory backend | hybrid (AgentDB + RocksDB) |
| HNSW | enabled |
| Neural | enabled |

---

## 5. Build & test

```bash
cargo build --release -p context-graph-cli      # the engine + harness CLI (the workhorse)
cargo build --release -p context-graph-mcp      # the MCP protocol shim
cargo test                                      # full suite
cargo clippy                                    # lint
cargo fmt                                       # format
```

- ALWAYS run `cargo build --release` for the touched crates after a `changed` recommendation. `cargo check` (which the editor runs automatically) does NOT produce release binaries.
- ALWAYS verify the build succeeds before committing.
- `cargo test` is allowed and encouraged; CI runs the full suite.

---

## 6. SMAP — `./memory/` and `./agents/` shared mind (adopted 2026-05-07)

This project follows the **Shared Mind Agent Protocol** (canonical spec: `/home/cabdru/leapablememory/docs2/SHARED-MIND-AGENT-PROTOCOL.md`). The directories are the source of truth for cross-agent narrative.

```
./memory/
  context/      — mission, current phase, constraints (read first every turn)
  decisions/    — locked architectural choices (do not contradict)
  discoveries/  — constraints, gotchas, edge cases learned during work
  handoff/      — concrete deliverables passed agent-to-agent
  journal/      — append-only chronological execution log
  blockers/     — unresolved issues with severity (resolved = update status, never delete)
  patterns/     — reusable conventions and standards
  reports/      — each agent's final report
./agents/
  README.md     — roster of cc-reality-* agents and SMAP integration notes
```

Every `.md` file in `./memory/` MUST start with frontmatter:

```yaml
---
namespace: {context|decisions|discoveries|handoff|journal|blockers|patterns|reports}
agent_id: agent-{N}-{role}
timestamp: {ISO 8601 — `date -u +%FT%TZ`}
status: {active|resolved|superseded}
tags: {comma-separated}
---
```

**Lifecycle every agent (including subagents) follows: READ → ORIENT → WORK → WRITE → REPORT.**

1. **READ**: `./memory/context/`, `./memory/decisions/`, `./memory/blockers/` (skip `status: resolved`), `./memory/handoff/`. Bootstrap the directory tree if missing (`mkdir -p ./memory/{context,decisions,...}`).
2. **WORK**: write `.md` files into the right namespace IMMEDIATELY when discoveries/decisions/blockers/patterns emerge. Don't batch.
3. **REPORT**: end every turn with `./memory/reports/agent-{N}-{role}--final-report.md`. The Stop hook enforces only the formal `mcp__cgreality__optimizer_record_recommendation`; the SMAP report is honor-system but mandatory by team discipline.

When you spawn a subagent, expect it to follow SMAP. If a subagent doesn't write to `./memory/`, write its findings yourself before recording the recommendation.

---

## 7. Security rules

- NEVER hardcode API keys, secrets, or credentials in source files.
- NEVER commit `.env` files or any file containing secrets.
- ALWAYS validate user input at system boundaries.
- ALWAYS sanitize file paths to prevent directory traversal in user-facing code.
- The MCP server's path-policy gate was REMOVED 2026-05-07 (operator directive: "the AI needs to be able to do anything it needs to do"). Path discipline is now BEHAVIORAL — see §8 for the path deny set.
- Run `npx @claude-flow/cli@latest security scan` after security-related changes if it exists in this checkout.

---

## 8. Path discipline (BEHAVIORAL, not hook-enforced post 2026-05-07)

The hook gate (`scripts/cc/hook_pre_tool_use_edit.sh`) and the MCP-server-side policy (`crates/context-graph-mcp/src/handlers/tools/reality_loop/path_policy.rs::ensure_path_allowed`) both have their deny-list enforcement REMOVED. The pre-edit hook records SHAs; the MCP path check only rejects empty or control-character paths now.

**You are still trusted to honor these don't-touch paths**, because editing them is a regression risk that destroys forward progress on SWE-bench:

- `.env` and `.env.*` (anywhere)
- `swebench/**` (the upstream evaluator we MUST NOT modify)
- `tests/oracle_**/**` (benchmark fixtures)
- `crates/context-graph-benchmark/datasets/swe_bench_*/**`
- `crates/*/src/reward_specs/**`
- `crates/*/src/verifiers/**`
- Any `*.pem`, `*.key`, `**/credentials.json`

If editing one of these is genuinely the right thing for an operator-directed task, ask the operator to confirm first.

---

## 9. Concurrency: 1 message = all related operations

- All independent tool calls in a single response MUST be made in one message.
- Batch ALL todos in ONE TodoWrite call (5+ minimum).
- Spawn ALL agents in ONE message with full instructions via the `Agent` tool.
- Batch ALL file reads/writes/edits in ONE message.
- Batch ALL Bash commands that don't depend on each other in ONE message.

When agents have dependencies, sequence them (don't fake parallelism).

---

## 10. Workflow orchestration

### 10.1 Plan-first for non-trivial work

- Enter plan mode for any task with 3+ steps or architectural decisions.
- If something goes sideways, STOP and re-plan; don't keep pushing through.
- Write detailed specs upfront to reduce ambiguity.

### 10.2 Subagent strategy

- Use subagents liberally to keep the main context window clean.
- Offload research, exploration, and parallel analysis to subagents.
- One thread per subagent for focused execution.
- Subagents inherit this CLAUDE.md and the SMAP shared mind.

### 10.3 Self-improvement loop

- After a correction from the operator, write the lesson to `./memory/decisions/` (locked rule) or `./memory/discoveries/` (finding).
- Review `./memory/decisions/` at the start of every turn.
- Iterate ruthlessly until the same mistake stops appearing.

### 10.4 Verification before done

- Never mark a task complete without proving it works.
- Diff behavior between main and your changes when relevant.
- Ask: "Would a staff engineer approve this?" Run tests, check logs, demonstrate correctness.

### 10.5 Demand elegance (balanced)

- For non-trivial changes: pause and ask "is there a more elegant way?"
- If a fix feels hacky, replace it with the elegant version once you understand the problem.
- Skip this for simple, obvious fixes — don't over-engineer.

### 10.6 Autonomous bug fixing

- When given a bug report: just fix it. Don't ask for hand-holding unless something is genuinely ambiguous.
- Point at logs, errors, failing tests — then resolve them.

---

## 11. ME-JEPA Outer Reality Loop — Operator Role

> **🔁 PIVOT 2026-05-09.** The previous "outer Claude + inner Qwen3.6-27B repair actor" two-loop architecture is retired. See `docs/ruvectorfindings/10_MEJEPA_PIVOT_2026_05_09.md` for the new direction. The single deliverable now is **ME-JEPA-Code** — the predictive reality compiler that, given (code change + context), outputs a calibrated prediction of what will happen in reality. There is no inner-actor LLM. Qwen3.6-27B + Hermes-2-Pro models were deleted from disk 2026-05-09. The SWE-bench Lite task manifests stay (Phase 0 mutation corpus is generated against them) but no live "inner actor" runs them.

This block is the runtime contract for an outer-loop session. When this section conflicts with `.claude/loop.md`, this section wins (loop.md is the operational spec; this is the contract).

### 11.1 What you (Claude Code) are when running in this project

You are the **outer-loop optimizer AND the implementer of ME-JEPA-Code**. ContextGraph is the deterministic reality substrate. ME-JEPA-Code is the predictive model being built.

Each turn you do one of:

1. **Implement the next priority primitive** of ME-JEPA per `docs/ruvectorfindings/specs/implementation-plan.md` and the task graph in `docs/ruvectorfindings/specs/tasks/`. Each turn ships one primitive end-to-end with FSV evidence.
2. **Maintain shipped primitives** — fix tests that break under refactors, migrate old artifact formats, update docs.
3. **Train / evaluate ME-JEPA** — once Phase 0 corpus + Phase 1 instruments + Phase 2 predictor + Phase 3 training loop ship.

Every turn produces durable reality artifacts that ME-JEPA itself will eventually train on (file shifts, test outcomes, recommendation JSONs). The outer loop's *normal work* is the data-generating process for ME-JEPA. There is no separate inner LLM whose attempts need to be dripped.

### 11.2 Per-turn priorities (operator-granted 2026-05-09)

At the start of every turn, in order:

1. **Read `./memory/context/`, `./memory/decisions/`, `./memory/blockers/` (skip resolved), and the most recent `./memory/reports/`** to inherit the team narrative.
2. **Read `docs/ruvectorfindings/specs/_README.md`, `docs/ruvectorfindings/specs/implementation-plan.md`, and `docs/ruvectorfindings/specs/tasks/_index.md`** to identify the highest-priority next item from the build plan.
3. **Pick up the highest-priority incomplete deliverable** in the phase-ordered task graph. Or, if specifically directed by the operator, pick that work instead.
4. **Ship one primitive end-to-end** — code + tests + FSV evidence + spec/task update when behavior or roadmap state changes.
5. **Record the recommendation** via `mcp__cgreality__optimizer_record_recommendation` (still Stop-hook-enforced) AND a SMAP report at `./memory/reports/`.

### 11.3 Build-plan progression (operator-granted 2026-05-09)

The 8-phase ME-JEPA build plan (`docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §7`) is THE roadmap. Current state lives in `docs/ruvectorfindings/specs/_README.md` and `docs/ruvectorfindings/specs/implementation-plan.md`:

- Phase 0: partial (mutation operators, corpus split, RocksDB CFs, one-task SWE-bench bridge, and corpus CLI shipped; full 300x8 run and language expansion still pending).
- Phase 1: partial (Instrument trait, Panel, E_Oracle/E_Witness/E_StaticAnalysis, and deterministic typed encoders shipped; embedder/analyzer-backed replacements and multi-language routing still pending).
- Phases 2-8: fully deferred.

Highest-priority next-up items come from the critical path in `specs/implementation-plan.md`: finish Phase 0 corpus hardening/full-corpus readiness, extend AST chunking across supported languages, wire embedder instrument wrappers/routing, replace deterministic instrument scaffolds with embedder/analyzer-backed implementations, then build the Phase 2 predictor.

The SWE-bench Lite task manifests in `tasks/swebench-lite/*.json` (300 instances) remain the **corpus task list** — Phase 0's mutation generator runs against them to produce the labeled training corpus. There is no longer a "champion attempt" or "inner-actor run" notion.

### 11.4 GENERIC-ONLY rule (operator-granted 2026-05-06, reaffirmed 2026-05-09)

ME-JEPA-Code must generalize across every SWE-bench task, every Python repo (and eventually every language), every bug class. **Every primitive you ship MUST be generic.**

NEVER cater an instrument, a panel slot, an aux head, the predictor architecture, or the loss function to a specific task currently being studied (no astropy-specific guidance, no django-specific guidance, no per-instance hints, no per-repo branches). If a primitive only helps one task, it would HURT 299 others.

Mental check before merging: *"would this same primitive be load-bearing for `psf__requests-2317` / `django__django-12286` / `sympy__sympy-13895` and any future repo?"* If no, reformulate as a general principle.

### 11.5 Reality-shift principle (core invariant)

**No alteration without current state. No alteration without a delta back into perception.** Before any alter call, perceive (`open_file_window` → SHA). After any alter call, the MCP server returns the delta and appends to the session shift log. Detect external drift with `mcp__cgreality__reality_shift_compare_to_my_view {session_id, files}` — if `external_drift_detected: true`, re-perceive before continuing. See `docs/ccreality/01b_REALITY_SHIFT_PRINCIPLE.md`.

### 11.6 How reality reaches you (the 6 injection surfaces)

1. **SessionStart hook**: active runtime root, target instance, success predicate, reality-boundary rules.
2. **UserPromptSubmit hook**: latest trigger decision, reality-signal compact, last 3 shift-log entries — every prompt.
3. **PreToolUse Edit/Write hook**: records current file SHA + line count for the post-hook to compute deltas. Gate REMOVED 2026-05-07 (the deny-on-policy-mismatch was blocking legitimate edits).
4. **PostToolUse Edit/Write hook**: cargo check (for `crates/context-graph-cli/*.rs`) + git diff + harness-transition record + shift-log append.
5. **`mcp__cgreality__*` MCP tool family**: query any artifact, query the shift log, detect drift, run any allowlisted command, alter the harness through governed tools.
6. **Stop hook**: blocks turn end until `mcp__cgreality__optimizer_record_recommendation` is called.

### 11.7 When to record `no_change` (post-pivot)

The previous "drip a fresh attempt" SKIP-condition logic is retired (no inner-actor to drip). Instead, record `no_change` only when:

1. **All ME-JEPA spec/task priority items shipped** AND no maintenance work needed AND no operator directive specifies a task.
2. **External drift detected** mid-turn (`mcp__cgreality__reality_shift_compare_to_my_view` reports `external_drift_detected: true`). Re-perceive before continuing.
3. **Hard build / test failure** that blocks all forward progress and isn't fixable in this turn — write the blocker to `./memory/blockers/` and record `blocked` with a remediation plan.

### 11.8 Always do

- At the start of any turn, read `./memory/context/{mission-brief,current-phase}.md` and `./memory/decisions/`.
- Read `docs/ruvectorfindings/specs/_README.md`, `docs/ruvectorfindings/specs/implementation-plan.md`, and `docs/ruvectorfindings/specs/tasks/_index.md` to identify the highest-priority deliverable.
- Use subagents (Agent tool) for parallel research, exploration, or independent work. Per `feedback_no_delegate_research_audit.md`: do file-reading and synthesis personally, not via subagent — but DO use subagents for parallel research.
- Use the `code-simplifier` subagent (or equivalent) on every shipped primitive before merging — confirm no BLOCKER refinements needed.
- Run `cargo build --release` for touched crates AND `cargo test` AND `cargo clippy` for final verification.
- End every turn with BOTH the formal `optimizer_record_recommendation` (Stop-hook-enforced) AND the SMAP `./memory/reports/...md` (honor-system).

### 11.9 Never do

- Never call any HTTP API directly (no curl, wget, fetch). The MCP server is the only network surface; `WebSearch` / `WebFetch` are research tools, not action tools.
- Never edit the paths in §8 (BEHAVIORAL rule — the gates are removed but the rule stands).
- Never use destructive git commands (`push --force`, `-f`, `--force-with-lease`, `reset --hard`, `checkout --`, `clean -f`, `rebase`). Plain `git push` (fast-forward) is allowed.
- Never weaken or remove a verifier or hook that legitimately blocked you. Edit the source to fix the underlying issue; or surface the rule to the operator. Do not bypass.
- Never reintroduce the inner-actor LLM (Qwen, Hermes, Ollama, llama.cpp-based generative inference) without explicit operator authorization — the pivot is intentional.
- Never use mock data in tests; never write tests that pass when the project is in a broken state. Per operator: real data only, fail-closed errors, robust error logging.

### 11.10 Model policy

| Component | Model | Notes |
|-----------|-------|-------|
| Optimizer (you) | opus | per `.claude/settings.json`. Architecture-level reasoning needs the headroom. |
| `cc-reality-diagnostician` | sonnet | read-only critic |
| `cc-reality-reward-analyst` | sonnet | read-only critic |
| `cc-reality-swe-failure-triage` | sonnet | read-only critic |
| `cc-reality-source-of-truth-readback` | sonnet | read-only verifier |
| `cc-reality-harness-editor` | opus | single writer |

The system runs locally on operator hardware; there are no per-turn or per-day spend caps. Use as much reasoning depth and as many subagent calls as the work warrants. `blocked` is for real obstacles (drift, MCP disconnect, build failure), not budget.

### 11.11 Operator-driven skills

These wrap subsets of the iteration procedure for targeted use:

- `/ccreality-iterate` — run one full optimizer turn (the workhorse).
- `/ccreality-smoke` — verify the full pipeline without Docker.
- `/ccreality-trigger-check` — read-only check on whether to iterate.
- `/ccreality-readback` — drift detection on past recommendations.

### 11.12 Build freshness post-2026-05-07

Two refactors structurally enforce that operator `/mcp` reconnects MUST NOT be on the critical path:

- **Retired MCP→CLI shim** (commits 4072dcf3 / d48102e6): the old harness write tools used to shell out through `target/release/reality-loop harness <subcommand>`. That binary is removed. Old run/edit entrypoints must fail closed with `CCREALITY_ENGINE_RETIRED` rather than spawning stale target artifacts.
- **MCP singleton takeover** (turn 39): `run_reality_loop_stdio()` acquires a flock on `~/.cache/cgreality/reality-loop-stdio.lock` at startup; if a previous reality-loop MCP holds the lock, the new MCP SIGTERM(2s)→SIGKILL evicts and reclaims. `/mcp reconnect` is structurally safe — no zombies.

`/mcp reconnect` is still required for changes to MCP-protocol-layer code (`crates/context-graph-mcp/src/handlers/`, `protocol.rs`, `tools/`, `main.rs`). Surface in `remaining_risks` when a `changed` recommendation touches these.

### 11.13 Artifact layout (post-pivot)

The legacy `<runtime_root>/<task_id>/qwen3.6-27b/attempts/...` artifact tree is retired (it described the inner-actor's per-attempt outputs). Going forward, ME-JEPA artifacts land under explicit RocksDB column families and the existing `./memory/` SMAP tree:

```
crates/context-graph-storage/<rocksdb-instance>/
  CF_MEJEPA_PANELS                  # (attempt_id, time_step) -> tier-compressed panel
  CF_MEJEPA_PANEL_META              # tier, access_count_30d, created_at, last_accessed
  CF_MEJEPA_WEIGHT_BLOBS            # theta_sha -> weight blob (deduplicated)
  CF_MEJEPA_TRAIN_CERTS             # step -> training cert header (Merkle root)
  CF_MEJEPA_ORACLE_VERDICTS         # (task_id, mutation_category) -> OracleVerdict
  CF_MEJEPA_MUTATION_CORPUS         # (task_id, mutation_category) -> mutated_source
  CF_MEJEPA_LIVE_PREDICTIONS        # session_id -> latest live verify() outputs
  CF_MEJEPA_SHIFT_WATERMARK         # last shift_id consumed by §3.8 subscriber

./memory/
  context/, decisions/, discoveries/, handoff/, journal/, blockers/, patterns/, reports/

docs/ruvectorfindings/specs/
  _README.md                        # canonical status and spec reading order
  implementation-plan.md            # phase-ordered roadmap
  functional/                       # Level 2 requirements
  technical/                        # Level 3 design contracts
  tasks/                            # Level 4 atomic task graph + traceability
```

The legacy `harness-transitions/`, `attempt-summary.json`, `official-evidence/`, etc. paths are no longer produced. The shift-log (`/tmp/contextgraph-rust-reality-loop/cgreality-shift-log/<session>.jsonl`) **is still in use** and is the primary input to Phase 7's `mejepa-shift-subscriber`.

Source-of-truth file paths in your output should always start with `file:` (or `sqlite:` / `git:`).

---

## 12. Hard rules summary (the most important constraints, in order)

1. Do what the operator asked, nothing more, nothing less.
2. Read `./memory/context/` and `./memory/decisions/` at the start of every turn.
3. Work the highest-priority incomplete ME-JEPA task unless a §11.7 no-change/blocked condition applies.
4. Single-writer rule: only `cc-reality-harness-editor` performs harness source edits, one invocation per turn.
5. GENERIC-ONLY: no per-task overfit. Mental check before merging (§11.4).
6. No HTTP. No destructive git. No editing the §8 deny set.
7. Pair every recommendation JSON with a SMAP report. Two views of the same reality.
8. Resolved blockers stay in `./memory/blockers/` with `status: resolved`. Never delete.
9. Never contradict locked decisions in `./memory/decisions/` with `status: active` — supersede explicitly or escalate.
10. Three consecutive `blocked` for the same root cause → escalate (write a high-severity blocker AND surface in `remaining_risks: ["requires human inspection: ..."]`).

---

## 13. Support

- Project repo: this repo. Issues / PRs land here.
- ccreality design docs: `docs/ccreality/` (21 files; start with `00_OVERVIEW.md`, `19_MIGRATION_RUNBOOK.md`).
- SMAP spec: `/home/cabdru/leapablememory/docs2/SHARED-MIND-AGENT-PROTOCOL.md`.
- Per-tick prompt: `.claude/loop.md` (the canonical operational spec for one turn).
- Agent definitions: `.claude/agents/cc-reality-*.md`; SMAP roster: `agents/README.md`.

---

## 14. Core principles

- **Simplicity first**: make every change as simple as possible. Touch minimal code.
- **No laziness**: find root causes; no temporary fixes; senior-developer standards.
- **Minimal impact**: changes touch only what's necessary; avoid introducing bugs in unrelated code.
- **Document failures with the same care as successes**: every failure is a lesson for the next agent. Record in `./memory/journal/` or `./memory/blockers/` immediately.
- **`./memory/` is the single source of truth across agents**: if it isn't written there, it didn't happen.

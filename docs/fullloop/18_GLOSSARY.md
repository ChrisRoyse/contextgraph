# Glossary — shared vocabulary

Used everywhere in this directory. Whenever any term below feels ambiguous, this is the source of truth.

## Core entities

**Reality engine** — `target/release/reality-loop`. The Rust binary that runs Gemma SWE attempts, calls official SWE-bench Docker eval, and writes durable artifacts. Owned by ContextGraph; never edited by Claude Code directly (Claude Code edits its source via governed MCP `harness_*` tools).

**cgreality MCP server** — `target/release/context-graph-mcp --mode reality-loop`. The lightweight MCP server that exposes the reality-loop's analyze/interact/alter tool surface to Claude Code over stdio JSON-RPC.

**Outer optimizer** — Claude Code (Opus 4.7). Reads reality, diagnoses failures, edits the harness, records a structured recommendation per turn.

**Inner SWE actor** — Gemma 4 (gemma4:e4b on Ollama). Receives the problem-reality + reality-signal, navigates the repo through governed coding-agent tools, submits a candidate patch via `apply_line_window_edit` or `apply_structured_edit`. Phase 16 adds Qwen and Mistral as peer actors.

**ContextGraph** — the Rust workspace housing the engine, MCP, memory store, embedders, and DynamicJEPA.

## Artifacts

**Runtime root** — `<runtime_root>` — the directory holding all artifacts for one operator session's reality loop. Pointed at by `~/.cache/cgreality/active_runtime_root`.

**Run** — `<runtime_root>/<run_id>/` — one engine invocation. Multiple runs can share a runtime root.

**Attempt** — `<runtime_root>/<run_id>/<task_id>/<model>/attempt-NNNNNN/` — one (task, model) try.

**Problem-reality packet** — durable JSON describing what the SWE-bench task actually is (FAIL_TO_PASS, PASS_TO_PASS, base_commit, oracle paths, etc.).

**Reality-signal packet** — durable JSON describing what happened in this attempt (validation status, failure class, patch stats, tool grounding, reward).

**Harness transition** — durable JSON record of one governed-tool mutation in an attempt. Multiple per attempt; round-numbered.

**Attempt summary** — durable JSON aggregating an attempt's outcome with pointers to all sub-artifacts.

**Recommendation** — `<attempt>/claude-code-optimizer/recommendation-turn-NN.json` — structured JSON closing one optimizer turn. Schema in `config/ccreality_recommendation_schema.json`.

**Trigger decision** — `<attempt>/trigger-decision.json` — whether the optimizer should run for this attempt.

**Shift-log entry** — one row in `<runtime_root>/cgreality-shift-log/<session_id>.jsonl` recording one alter-tool call's before/after delta.

**Champion** — best-known (model, task, harness-state) result. `candidate` until promoted (Phase 19).

**Experiment** — a candidate harness change being evaluated. Tracked in `experiment-registry.json` with status `pending|kept|discarded|escalated`.

## Concepts

**Reality drip** — six injection points (SessionStart, UserPromptSubmit, PreEdit, PostEdit, Stop, PreCompact) plus the always-on MCP channel that feed reality into Claude Code.

**Reality-shift principle** — `docs/ccreality/01b_REALITY_SHIFT_PRINCIPLE.md`. No alteration without current state; no alteration without a delta back into perception.

**Drift** — current SHA on disk differs from this session's last-known SHA without a session-caused shift in between. External writer mutated reality.

**Governed paths** — repo paths the optimizer can read/edit/run, defined in `config/ccreality_governed_paths.json`. Allow + deny lists. Deny wins over allow.

**Governed coding-agent tools** — Gemma's tool surface (`open_file_window`, `apply_line_window_edit`, etc.) — *not* the optimizer's MCP `harness_*` tools.

**Reward spec** — `gemmafrontend/reward_spec.verified_outcome.json`. Production scoring rules. Read-only to the optimizer; shadow specs are the only writable surface.

**Reality score** — single aggregate operational metric (Phase 20). Not a training reward.

**Curriculum splits** — dev / holdout / promotion (Phase 19). The optimizer only sees dev; holdout/promotion are scoring runs.

## Roles

**Operator** — the human running the system. Picks tasks, watches dashboards, approves promotions, escalates when needed.

**External watcher** — a small shell script (`scripts/cc/cgreality-tick.sh`) that periodically launches Claude Code single-shot when reality has changed.

**Subagent** — a specialized Claude Code agent invoked from the main session. Read-only critics + one writer (`cc-reality-harness-editor`).

## Outcomes

**resolved_ids** — array in the official SWE-bench Docker report. The only field that proves SWE success.

**official_resolved** — boolean: target instance is in `resolved_ids`.

**status** of recommendation:
- `changed` — the optimizer applied a harness edit; verifications passed.
- `no_change` — the optimizer chose not to edit (trigger says don't run, or no useful intervention).
- `blocked` — the optimizer wanted to act but cannot (drift, environment, escalate to human).
- `error` — the optimizer hit an error mid-turn that prevented completion.

## Off-limits

The deny list of the governed paths policy:
`.env`, `**/.env*`, `swebench/**`, `tests/oracle_**`, `crates/context-graph-benchmark/datasets/swe_bench_*`, `crates/*/src/reward_specs/**`, `crates/*/src/verifiers/**`, `**/*.pem`, `**/*.key`, `**/credentials.json`, `/etc/**`, `/root/**`.

Plus, never claim success without reading `resolved_ids` from disk via `mcp__cgreality__reality_official_report`.

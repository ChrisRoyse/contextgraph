# Current State (2026-05-05)

This is the source-of-truth snapshot of what runs today. Every line links to a concrete file, command, or artifact path that an AI agent can re-verify.

## Branch and commits

- Branch: `ccreality-cutover` (created from base SHA `411443fdcbd8db4005cfe46d44f64f7698c2f11b`).
- Recent commits (top of branch):
  - `0f12ca53` — feat(ccreality): complete phase 12 production cutover
  - `d3442fce` — test(ccreality): phase 10 FSV harness — 39 case scripts + master runner
  - `7ba891be` — feat(ccreality): runtime audit trail tool + harden ledger WHERE validator
  - `b3a02318` — feat(ccreality): phase 09 smoke harness — full-stack runner + CLI metadata fix
  - `1ff731a4` — feat(ccreality): phase 0-8 hardening — CLAUDE.md, hardening gate, doc refresh

Verify with: `git log --oneline -n 5`.

## Active runtime root (what cgreality is currently pointing at)

```
~/.cache/cgreality/active_runtime_root  → /tmp/cgreality-smoke-1778022643/layer5-skill-runtime
~/.cache/cgreality/active_target_instance → cgreality-smoke-phase09-layer5
```

Latest run inside that root:

- `run_id`: `smoke-1778022681`
- `task_id`: `cgreality-smoke-phase09-layer5`
- `attempt`: 1 (synthetic smoke)
- `attempt-summary.json`: `file:/tmp/cgreality-smoke-1778022643/layer5-skill-runtime/smoke-1778022681/cgreality-smoke-phase09-layer5/smoke-attempt-1/attempt-summary.json`
- Two `optimizer_record_recommendation` calls already recorded (status `no_change`, both turns trigger-decision `should_run=false` policy `phase09_smoke`).
- Five shift-log records exist in `cgreality-shift-log/phase09-smoke-1778022643.jsonl`.

Verify with: `mcp__cgreality__reality_latest_root`, `mcp__cgreality__reality_attempt_summary {run_id, attempt}`, `mcp__cgreality__reality_shift_log {session_id}`.

## Engine binary subcommands

`./target/debug/reality-loop --help` exposes:

- `attempt`        — one the inner actor SWE-bench attempt with optional official Docker eval.
- `smoke`          — synthetic deterministic attempt; no the inner actor, no Docker.
- `replay`         — replay an artifact into a new run root.
- `verify`         — verify a runtime root's artifacts against schemas + readback.
- `official-eval`  — re-run official SWE-bench Docker eval against an existing attempt.
- `targeted-test`  — run a single SWE-bench `fail_to_pass` test against a candidate.

Source: `crates/context-graph-cli/src/bin/reality-loop.rs` (~7,990 lines, no DeepSeek/OpenAI text).

## MCP server `cgreality` tools (lightweight reality-loop mode)

Verify list with: `./target/debug/context-graph-mcp --mode reality-loop` then JSON-RPC `tools/list`.

Analyze (read durable artifacts):
- `reality_latest_root`, `reality_attempt_summary`, `reality_official_report`, `reality_problem_packet`, `reality_signal`, `reality_failure`, `reality_trigger_decision`, `reality_harness_transitions`, `reality_compare_attempts`, `reality_audit_trail`.

Interact (drive the engine):
- `reality_run_attempt`, `reality_run_smoke`, `reality_run_official_eval`, `reality_run_targeted_test`, `reality_recompile_reward`, `reality_replay_artifact`, `reality_query_ledger`.

Alter (mutate harness + record optimizer artifacts):
- `harness_open_window`, `harness_apply_line_window_edit`, `harness_run_command`, `harness_git_diff`, `harness_git_status`, `harness_verify_state`, `optimizer_record_decision`, `optimizer_record_recommendation`, `optimizer_record_harness_transition`.

Reality-shift discipline:
- `reality_shift_log`, `reality_shift_compare_to_my_view`.

The lightweight server intentionally does NOT advertise full memory/graph tools. Phase 12 hardening enforced this so `tools/list` cannot mislead the optimizer.

## Hooks delivering reality drip

`scripts/cc/hook_*.sh` (all executable):

| Hook script | Hook event | What it injects |
|-------------|-----------|------------------|
| `hook_session_start.sh` | SessionStart | runtime root, target instance, success predicate, deny rules |
| `hook_user_prompt_submit.sh` | UserPromptSubmit | latest trigger decision, reality-signal compact, last 3 shift events |
| `hook_pre_tool_use_edit.sh` | PreToolUse Edit\|Write | governed-path policy verdict + current SHA |
| `hook_post_tool_use_edit.sh` | PostToolUse Edit\|Write | cargo check + new SHA + git diff stat + harness-transition mint |
| `hook_stop.sh` | Stop | refuses end-of-turn until `optimizer_record_recommendation` was called |
| `hook_pre_compact.sh` | PreCompact | re-injects runtime root + last trigger decision so context survives compaction |

Tests: `scripts/cc/tests/hooks/test_*.sh` and the master `scripts/cc/tests/smoke.sh`.

## Subagents and skills

`.claude/agents/cc-reality-*.md` — five subagents:
- `cc-reality-diagnostician` (Sonnet, read-only) — failure classification.
- `cc-reality-harness-editor` (Opus, single writer) — applies governed edits.
- `cc-reality-source-of-truth-readback` (Sonnet, read-only) — drift detection.
- `cc-reality-reward-analyst` (Sonnet, read-only) — reward signal analysis.
- `cc-reality-swe-failure-triage` (Sonnet, read-only) — SWE-bench Docker eval triage.

`.claude/skills/ccreality-*/SKILL.md` — four skills:
- `ccreality-iterate` — one full optimizer turn.
- `ccreality-smoke` — synthetic full-stack pipeline check.
- `ccreality-trigger-check` — read-only trigger decision lookup.
- `ccreality-readback` — re-verify recent recommendations.

## CLAUDE.md addendum

`CLAUDE.md` ends with the "ccreality — Outer Reality-Loop Optimizer Role" block. The same content is mirrored in `docs/ccreality/10_CLAUDE_MD_PROJECT_MEMORY.md`.

## Phase 12 production attempt (the most recent real SWE-bench attempt)

- Runtime root: `/tmp/ccreality-prod-1`.
- Hardened run: `rust_reality_1778020383548106093_e525987d`.
- Task: `psf__requests-2317` (SWE-bench Lite, real Hugging Face row).
- Attempt-summary: `tasks/psf__requests-2317/qwen3.6-e4b/attempts/attempt-000001/attempt-summary.json`.
- Outcome: `status=failed`, `error_code=RUST_REALITY_FINAL_EDIT_TOOL_FAILED`, `patch_sha256=null`.
- Optimizer recommendation: `…/claude-code-optimizer/recommendation-turn-01.json` (real, schema-valid).
- Final FSV log: `/tmp/ccreality-phase12-fsv-20260505T225350Z.log`.

Reading: `docs/ccreality/PHASE12_RESULTS_2026-05-05.md`.

## What this snapshot proves

The plumbing is real and reactive. A synthetic smoke runs end-to-end through engine → MCP → hooks → subagents → skill → recommendation → ledger. A real production attempt runs end-to-end on real Hugging Face data and fails closed without a false success claim. The next phases turn the plumbing into a productive loop.

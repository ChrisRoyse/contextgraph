# Phase 6 — Specification

## Objectives

1. Create the six hook scripts that implement the reality-drip protocol.
2. Create a shared `_common.sh` with helper functions (jq emit, runtime-root reader, log writer).
3. Create unit tests for each hook under `scripts/cc/tests/hooks/`.
4. Optionally: add a `context-graph-cli ccreality record-harness-transition` CLI subcommand if the PostToolUse hook needs it.

## Scope

Files created:
```
scripts/cc/_common.sh
scripts/cc/hook_session_start.sh
scripts/cc/hook_user_prompt_submit.sh
scripts/cc/hook_pre_tool_use_edit.sh
scripts/cc/hook_post_tool_use_edit.sh
scripts/cc/hook_pre_compact.sh
scripts/cc/hook_stop.sh
scripts/cc/tests/hooks/test_session_start.sh
scripts/cc/tests/hooks/test_user_prompt_submit.sh
scripts/cc/tests/hooks/test_pre_tool_use_edit.sh
scripts/cc/tests/hooks/test_post_tool_use_edit.sh
scripts/cc/tests/hooks/test_pre_compact.sh
scripts/cc/tests/hooks/test_stop.sh
scripts/cc/tests/hooks/_common_helpers.sh
```

Optional:
```
crates/context-graph-cli/src/bin/reality-loop.rs (modify) — add ccreality subcommand wrappers
```

Out of scope: wiring these hooks into `.claude/settings.json` — that's Phase 7.

## Deliverables

For each hook script:
- Reads JSON from stdin per Claude Code's hook input contract.
- Emits structured `additionalContext` JSON to stdout (or `decision: block` for Stop).
- Writes a JSONL log line to `<runtime_root>/cgreality-hook-events.jsonl` for debugging.
- Exit 0 in normal case; exit 2 only for intentional blocks.
- Bounded by a per-hook timeout (configured in `.claude/settings.json` Phase 7).
- Idempotent: re-firing produces equivalent output (except for the timestamp in the log).
- Robust: missing artifacts produce informative `additionalContext`, never block.

For each test script:
- Synthesizes a stdin JSON.
- Runs the hook script.
- Captures stdout + stderr + exit code.
- Asserts exit code matches expectation.
- Asserts stdout JSON matches expected shape (via jq).
- Cleans up after itself.

## AI-verifiable end-state

See [04_VERIFICATION.md](04_VERIFICATION.md). Three clauses:
- All 13 files exist.
- All 6 unit tests pass.
- ShellCheck is clean (if installed).

## Time budget

1–2 hours. Most of the bash is in [../07_HOOKS_SPEC.md](../07_HOOKS_SPEC.md); this phase is mostly copy-paste-and-test. The PostToolUse hook is the most complex (it runs cargo check); budget extra time there.

## Hook authorship rules (lifted from [../07_HOOKS_SPEC.md](../07_HOOKS_SPEC.md))

1. Hook scripts must never exit non-zero except for intentional blocks. Use `set -euo pipefail`; for non-blocking observations, capture failures explicitly, emit a structured log record with the exact command, exit code, stderr tail, and source-of-truth path inspected, then return a JSON envelope.
2. Hook scripts must never write to stdout outside the JSON envelope. Use `>&2` or the log helper.
3. Hook scripts must never spawn anything long-running synchronously. Cap cargo check at 60s; other spawns at 10–30s.
4. Hook scripts must never read or write files outside the project tree without explicit cause. They may read `~/.cache/cgreality/*` and write `~/.cache/cgreality/last_session_marker`.
5. Hook scripts must be idempotent.

## Optional CLI subcommand

The PostToolUse hook records a harness-transition record. Two implementation choices:

**Option A — direct write:** the bash hook writes the JSON itself (using jq). Simpler. Doesn't append to BlackBox ledger; doesn't append to shift log.

**Option B — CLI wrapper:** the bash hook calls `context-graph-cli ccreality record-harness-transition --tool-use-id ... --file ...` which:
- Writes the harness-transitions/round-NN.json artifact.
- Appends to BlackBox ledger.
- Appends to shift log (if needed).

Option B is cleaner. Option A is faster to implement. Recommended: Option B, but only if the implementation budget allows. Option A is acceptable for Phase 6 with a follow-up to upgrade later.

This phase implements **Option A** by default (simpler bash), with a TODO comment in the hook script pointing at the future Option B upgrade.

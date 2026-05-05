# Phase 6 — Hooks

**Estimated duration:** 1–2 hours.
**Status:** pending.
**Depends on:** Phase 4 (MCP write tools must exist; the PostToolUse hook calls `optimizer_record_harness_transition` indirectly via the `context-graph-cli ccreality record-harness-transition` subcommand).
**Unblocks:** Phase 7.

## What this phase does

Creates the six hook bash scripts under `scripts/cc/`, plus a shared `_common.sh` and a small unit-test harness under `scripts/cc/tests/hooks/`. The hooks implement the reality-drip protocol from [../02_REALITY_DRIP_PROTOCOL.md](../02_REALITY_DRIP_PROTOCOL.md).

The six hooks:

1. `hook_session_start.sh` — SessionStart event; injects runtime root + reality-boundary rules.
2. `hook_user_prompt_submit.sh` — UserPromptSubmit; injects trigger decision + reality-signal compact + last 3 shift events.
3. `hook_pre_tool_use_edit.sh` — PreToolUse for Edit/Write; injects current SHA + path policy verdict.
4. `hook_post_tool_use_edit.sh` — PostToolUse for Edit/Write; runs cargo check + writes harness-transition.
5. `hook_pre_compact.sh` — PreCompact; re-injects runtime root + last shift summary.
6. `hook_stop.sh` — Stop; **blocks** until the optimizer recommendation is recorded.

Plus a thin `context-graph-cli ccreality record-harness-transition` CLI subcommand used by hook 4 (or the hook can write the JSON directly — design choice).

## AI-verifiable end-state

```bash
cd /home/cabdru/contextgraph

# 1. All 6 hook scripts + common helper exist and are executable
for s in _common.sh hook_session_start.sh hook_user_prompt_submit.sh hook_pre_tool_use_edit.sh hook_post_tool_use_edit.sh hook_pre_compact.sh hook_stop.sh; do
  test -x scripts/cc/$s
done

# 2. Bash unit tests pass
bash scripts/cc/tests/hooks/test_session_start.sh
bash scripts/cc/tests/hooks/test_user_prompt_submit.sh
bash scripts/cc/tests/hooks/test_pre_tool_use_edit.sh
bash scripts/cc/tests/hooks/test_post_tool_use_edit.sh
bash scripts/cc/tests/hooks/test_pre_compact.sh
bash scripts/cc/tests/hooks/test_stop.sh

# 3. ShellCheck (if available) is clean
if command -v shellcheck >/dev/null; then
  shellcheck scripts/cc/*.sh
fi
```

All four blocks must succeed.

## Files

- [00_SPEC.md](00_SPEC.md) — scope, deliverables.
- [01_PRECONDITIONS.md](01_PRECONDITIONS.md) — what must be true.
- [02_TASKS.md](02_TASKS.md) — atomic tasks.
- [03_FILE_SPECS.md](03_FILE_SPECS.md) — exact bash for every script + every test.
- [04_VERIFICATION.md](04_VERIFICATION.md) — verification block.
- [05_RECOVERY.md](05_RECOVERY.md) — common failures.
- [06_HANDOFF.md](06_HANDOFF.md) — what's true after.

## Reference docs

- [../02_REALITY_DRIP_PROTOCOL.md](../02_REALITY_DRIP_PROTOCOL.md) — the full hook contract.
- [../07_HOOKS_SPEC.md](../07_HOOKS_SPEC.md) — full bash for each hook (this is THE source of truth for the script content).

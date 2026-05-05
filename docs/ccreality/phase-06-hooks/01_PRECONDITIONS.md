# Phase 6 — Preconditions

## Required state

1. **Phases 0, 1, 2 complete.** The engine binary exists and supports subcommands.
2. **Phases 3, 4 complete.** The MCP server is built; tools/list returns the expected tools. The PostToolUse hook must call `mcp__cgreality__optimizer_record_harness_transition` from Phase 4; do not add a direct file-write substitute.
3. **Phase 5 complete.** `config/ccreality_governed_paths.json` exists. The PreToolUse hook reads this directly to make a path-policy decision before the Claude Code permission system fires.
4. **A smoke runtime root exists** for testing:
   ```bash
   ls "$(cat ~/.cache/cgreality/active_runtime_root)" 2>/dev/null
   ```

## Required tools

- `bash` 4+
- `jq` 1.6+
- `sha256sum`
- `mktemp`
- `find`, `grep`, `tail`, `head`
- Optional: `shellcheck` (highly recommended for verification clause 3)

## Required reading

- [../02_REALITY_DRIP_PROTOCOL.md](../02_REALITY_DRIP_PROTOCOL.md) — what each hook must do.
- [../07_HOOKS_SPEC.md](../07_HOOKS_SPEC.md) — the full bash for every script (THIS is the source of truth; this phase is largely a transcription).
- The Claude Code hooks reference at https://code.claude.com/docs/en/hooks (already researched in planning).

## Existing scripts that should NOT be touched

The repo already has hooks under `.claude/hooks/`:
```
.claude/hooks/post_tool_use.sh
.claude/hooks/pre_tool_use.sh
.claude/hooks/session_end.sh
.claude/hooks/session_start.sh
.claude/hooks/user_prompt_submit.sh
```

These belong to a different system (the existing context-graph hooks integration, not ccreality). Do not modify them. Phase 6's scripts go in `scripts/cc/`, not `.claude/hooks/`.

When Phase 7 wires `.claude/settings.json`, the hooks block points at `scripts/cc/hook_*.sh`. The existing `.claude/hooks/*.sh` scripts continue to be referenced by whatever existing system uses them (audit during Phase 7 to confirm, but they should be independent).

## What to NOT do

- Do not block or fail on missing artifacts. Hooks must be tolerant of partial state.
- Do not log to stdout. Stdout is the JSON envelope or empty; logging goes via `cgreality_log_event` (which writes to a JSONL file).
- Do not block on a slow operation. cargo check is the longest, capped at 60s.
- Do not reach across system boundaries (no curl, no wget, no apt).
- Do not modify `~/.cache/cgreality/active_runtime_root` or `active_target_instance`. Those are written by the engine binary; hooks read them.
- Do not touch the existing `.claude/hooks/*.sh` scripts.

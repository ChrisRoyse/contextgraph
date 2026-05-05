# Phase 7 — Claude Code Scaffolding

**Estimated duration:** 1–2 hours.
**Status:** pending.
**Depends on:** Phase 4 (MCP server) + Phase 5 (config files) + Phase 6 (hook scripts).
**Unblocks:** Phase 8.

Current readiness as of 2026-05-05: Phase 6 has passed verification. Phase 7 can wire `.claude/settings.json` to the executable `scripts/cc/hook_*.sh` scripts.

## What this phase does

Drops 11+ files into `.claude/` and creates `.mcp.json` so Claude Code:
- Loads the cgreality MCP server automatically.
- Knows which tools to allow/deny.
- Fires the right hook script on each lifecycle event.
- Recognizes the 5 cc-reality-* subagents and 4 ccreality-* skills.
- Has the right project memory loaded (Phase 8 actually appends to CLAUDE.md; this phase prepares the env around it).

## Files added

```
.claude/settings.json                                    # rewritten (was empty {})
.mcp.json                                                # NEW
.claude/loop.md                                          # NEW
.claude/agents/cc-reality-diagnostician.md               # NEW
.claude/agents/cc-reality-harness-editor.md              # NEW
.claude/agents/cc-reality-reward-analyst.md              # NEW
.claude/agents/cc-reality-source-of-truth-readback.md    # NEW
.claude/agents/cc-reality-swe-failure-triage.md          # NEW
.claude/skills/ccreality-iterate/SKILL.md                # NEW
.claude/skills/ccreality-smoke/SKILL.md                  # NEW
.claude/skills/ccreality-trigger-check/SKILL.md          # NEW
.claude/skills/ccreality-readback/SKILL.md               # NEW
```

(`.claude/settings.local.json` is left alone — operator-local file.)

## AI-verifiable end-state

```bash
cd /home/cabdru/contextgraph

# Files exist and are valid where expected
test -f .claude/settings.json && jq -e . .claude/settings.json >/dev/null
test -f .mcp.json && jq -e . .mcp.json >/dev/null
test -f .claude/loop.md
for a in diagnostician harness-editor reward-analyst source-of-truth-readback swe-failure-triage; do
  test -f .claude/agents/cc-reality-$a.md
  head -1 .claude/agents/cc-reality-$a.md | grep -q '^---$'
done
for s in iterate smoke trigger-check readback; do
  test -f .claude/skills/ccreality-$s/SKILL.md
  head -1 .claude/skills/ccreality-$s/SKILL.md | grep -q '^---$'
done

# .mcp.json points at cgreality
jq -e '.mcpServers.cgreality.command' .mcp.json >/dev/null

# settings.json has the hooks block pointing at scripts/cc/
jq -e '.hooks.SessionStart[0].hooks[0].command | contains("scripts/cc/hook_session_start.sh")' .claude/settings.json >/dev/null
```

## Files

- [00_SPEC.md](00_SPEC.md) — scope, deliverables.
- [01_PRECONDITIONS.md](01_PRECONDITIONS.md) — preconditions.
- [02_TASKS.md](02_TASKS.md) — atomic tasks.
- [03_FILE_SPECS.md](03_FILE_SPECS.md) — exact contents of every file (mostly references to upstream specs).
- [04_VERIFICATION.md](04_VERIFICATION.md) — verification block.
- [05_RECOVERY.md](05_RECOVERY.md) — common failures.
- [06_HANDOFF.md](06_HANDOFF.md) — what's true after.

## Reference docs

- [../06_SETTINGS_JSON_SPEC.md](../06_SETTINGS_JSON_SPEC.md) — full settings.json + .mcp.json content.
- [../08_SUBAGENTS_SPEC.md](../08_SUBAGENTS_SPEC.md) — 5 subagent files.
- [../09_SKILLS_SPEC.md](../09_SKILLS_SPEC.md) — 4 skill SKILL.md files.
- [../11_LOOP_PROMPT.md](../11_LOOP_PROMPT.md) — `.claude/loop.md`.

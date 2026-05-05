# Phase 7 — Preconditions

## Required state

1. **Phases 0–6 complete.** Verify:
   ```bash
   tail /tmp/ccreality-phase-progress.log
   ```
   Should show phase-00 through phase-06 PASS lines.

2. **`.claude/` directory exists.** Verify:
   ```bash
   ls -la /home/cabdru/contextgraph/.claude/
   ```
   It should contain `agents/`, `commands/`, `helpers/`, `hooks/`, `settings.json` (currently empty `{}`), `settings.local.json`, `skills/`, `statusline.mjs`, `statusline.sh`.

3. **`.mcp.json` does NOT exist yet.** Verify:
   ```bash
   test ! -f /home/cabdru/contextgraph/.mcp.json
   ```
   If it exists from a different setup, BACK IT UP first:
   ```bash
   cp .mcp.json .mcp.json.bak.$(date +%s)
   ```

4. **No conflicting agents.** The existing `.claude/agents/` directory has many subagents (claude-flow, github-*, sparc-*, etc.). Confirm none start with `cc-reality-`:
   ```bash
   ls /home/cabdru/contextgraph/.claude/agents/cc-reality-* 2>/dev/null && echo "CONFLICT" || echo "no conflict"
   ```

5. **No conflicting skills.** Check `.claude/skills/`:
   ```bash
   ls /home/cabdru/contextgraph/.claude/skills/ccreality-* 2>/dev/null && echo "CONFLICT" || echo "no conflict"
   ```

6. **Hook scripts from Phase 6 exist and are executable.** Verify:
   ```bash
   for h in scripts/cc/hook_*.sh; do test -x "$h" || { echo "MISSING: $h"; exit 1; }; done
   ```

7. **Engine binary exists.** Verify:
   ```bash
   test -x /home/cabdru/contextgraph/target/debug/reality-loop
   test -x /home/cabdru/contextgraph/target/debug/context-graph-mcp
   ```

8. **Config files from Phase 5 exist.** Verify:
   ```bash
   test -f /home/cabdru/contextgraph/config/ccreality_governed_paths.json
   test -f /home/cabdru/contextgraph/config/ccreality_recommendation_schema.json
   ```

## Forbidden actions

- Do not modify `.claude/settings.local.json` (operator-local).
- Do not modify `.claude/hooks/*.sh` (existing system).
- Do not modify any other agent file under `.claude/agents/` (only ADD `cc-reality-*`).
- Do not modify any other skill under `.claude/skills/` (only ADD `ccreality-*`).
- Do not commit partial Phase 7 work before verification passes. Current branch reality: verified cutover work is being recorded as incremental commits, with Phase 12 reserved for the final production verification/cutover commit.

## Required reading

- [../06_SETTINGS_JSON_SPEC.md](../06_SETTINGS_JSON_SPEC.md)
- [../08_SUBAGENTS_SPEC.md](../08_SUBAGENTS_SPEC.md)
- [../09_SKILLS_SPEC.md](../09_SKILLS_SPEC.md)
- [../11_LOOP_PROMPT.md](../11_LOOP_PROMPT.md)

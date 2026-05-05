# Phase 7 — File Specifications

The exact content of every file lives in upstream spec docs. This doc just maps each Phase 7 file to its source spec.

## File-to-source mapping

| Phase 7 file | Source spec |
|--------------|-------------|
| `.claude/settings.json` | [../06_SETTINGS_JSON_SPEC.md](../06_SETTINGS_JSON_SPEC.md) §"`.claude/settings.json` — committed project settings" |
| `.mcp.json` | [../06_SETTINGS_JSON_SPEC.md](../06_SETTINGS_JSON_SPEC.md) §"`.mcp.json` — project MCP servers" |
| `.claude/loop.md` | [../11_LOOP_PROMPT.md](../11_LOOP_PROMPT.md) §"File: `.claude/loop.md`" |
| `.claude/agents/cc-reality-diagnostician.md` | [../08_SUBAGENTS_SPEC.md](../08_SUBAGENTS_SPEC.md) §"1. `cc-reality-diagnostician`" |
| `.claude/agents/cc-reality-harness-editor.md` | [../08_SUBAGENTS_SPEC.md](../08_SUBAGENTS_SPEC.md) §"2. `cc-reality-harness-editor`" |
| `.claude/agents/cc-reality-reward-analyst.md` | [../08_SUBAGENTS_SPEC.md](../08_SUBAGENTS_SPEC.md) §"3. `cc-reality-reward-analyst`" |
| `.claude/agents/cc-reality-source-of-truth-readback.md` | [../08_SUBAGENTS_SPEC.md](../08_SUBAGENTS_SPEC.md) §"4. `cc-reality-source-of-truth-readback`" |
| `.claude/agents/cc-reality-swe-failure-triage.md` | [../08_SUBAGENTS_SPEC.md](../08_SUBAGENTS_SPEC.md) §"5. `cc-reality-swe-failure-triage`" |
| `.claude/skills/ccreality-iterate/SKILL.md` | [../09_SKILLS_SPEC.md](../09_SKILLS_SPEC.md) §"1. `ccreality-iterate`" |
| `.claude/skills/ccreality-smoke/SKILL.md` | [../09_SKILLS_SPEC.md](../09_SKILLS_SPEC.md) §"2. `ccreality-smoke`" |
| `.claude/skills/ccreality-trigger-check/SKILL.md` | [../09_SKILLS_SPEC.md](../09_SKILLS_SPEC.md) §"3. `ccreality-trigger-check`" |
| `.claude/skills/ccreality-readback/SKILL.md` | [../09_SKILLS_SPEC.md](../09_SKILLS_SPEC.md) §"4. `ccreality-readback`" |

The agent executing Phase 7 should open each source spec, copy the verbatim content (between triple-backtick markers), and write it to the destination path.

## Critical content rules

### YAML frontmatter

Every subagent and every skill file MUST start with:
```
---
name: <kebab-case>
description: <single sentence ≤1024 chars, starts with "Use [PROACTIVELY|AFTER|WHEN]" for subagents>
tools: <comma-separated tool list, mcp__cgreality__* spelled out>
model: <opus|sonnet|haiku|inherit>
---
```

The closing `---` is mandatory. The body follows immediately after.

For skills (per Anthropic Agent Skills validation):
- `name`: ≤64 chars, lowercase alphanumeric + hyphens only
- `description`: ≤1024 chars, non-empty, no XML tags

If a description exceeds 1024 chars, truncate. If a name has uppercase or underscores, fix.

### `.claude/settings.json` validation

Must be valid JSON. Common pitfalls:
- Trailing commas — JSON doesn't allow them.
- Unquoted keys — JSON requires double quotes.
- Single-quoted strings — JSON requires double quotes.
- Missing `]` or `}` brackets after copy-paste.

Run `jq -e . .claude/settings.json` after writing to validate.

### `.mcp.json` env-var expansion

The file references `${CONTEXTGRAPH_ROOT}`, `${HOME}`, `${CGREALITY_GOVERNED_PATHS}`. Claude Code's MCP loader handles `${VAR}` expansion. Verify by:
1. The variables resolve in the operator's shell.
2. `.claude/settings.json` `env` block sets the project-pinned values.

If Claude Code's MCP loader doesn't expand env vars, replace `${CONTEXTGRAPH_ROOT}` with the literal path `/home/cabdru/contextgraph`. Check the docs at https://code.claude.com/docs/en/mcp.

### Permissions allow/deny patterns

Per Claude Code's permission rule syntax:
- `Bash(cargo *)` — prefix match (NOTE the space before `*`).
- `Bash(cargo*)` — substring; matches `cargox`, `cargo-make` etc.
- `mcp__cgreality__.*` — regex (Claude Code uses regex when matcher contains non-alphanumeric).
- Deny always wins over allow at any settings level.

The settings.json in [../06_SETTINGS_JSON_SPEC.md](../06_SETTINGS_JSON_SPEC.md) uses these patterns correctly. Don't modify them.

### Hook command paths

In `.claude/settings.json`, hook commands use `"$CLAUDE_PROJECT_DIR"/scripts/cc/hook_*.sh` with `$CLAUDE_PROJECT_DIR` provided by Claude Code at runtime. Don't substitute literal paths — Claude Code resolves this per-session.

## Cross-validation: tool names in subagents must exist

Every `tools:` entry in every subagent file must correspond to an actual tool. The cgreality tools come from Phase 4. Verify:

```bash
ALLOWED_TOOLS=$(echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | ./target/debug/context-graph-mcp --mode reality-loop 2>/dev/null | jq -r '.result.tools[].name')

for af in .claude/agents/cc-reality-*.md; do
  TOOLS_LINE=$(grep '^tools:' "$af" | head -1 | sed 's/^tools: *//')
  for t in $(echo "$TOOLS_LINE" | tr ',' '\n' | tr -d ' '); do
    if [[ "$t" == mcp__cgreality__* ]]; then
      bare="${t#mcp__cgreality__}"
      if ! echo "$ALLOWED_TOOLS" | grep -qx "$bare"; then
        echo "FAIL: $af references $t but tools/list does not include $bare" >&2
        exit 1
      fi
    fi
  done
done
```

Any failure means the subagent will fail to invoke that tool at runtime. Fix the subagent or add the missing tool to Phase 4 before marking Phase 7 complete.

## Path policy update needed

Phase 7's Task 7.1 amends `config/ccreality_governed_paths.json` to add `.claude/settings.json` and `.mcp.json` to the allow list. After this update, future optimizer turns can edit those files via `harness_apply_line_window_edit`.

The amendment is a 2-line addition; record it in the Phase 5 verification log retroactively.

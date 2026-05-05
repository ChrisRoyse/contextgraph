# Phase 7 — Tasks

## Task 7.1 — Update governed-paths policy to include `.claude/settings.json` and `.mcp.json`

These files are now part of the operator-edited surface; future ccreality optimizer turns may need to update them. Add to `config/ccreality_governed_paths.json` `allow` list:
```
".claude/settings.json",
".mcp.json"
```

Verify:
```bash
jq -e '.allow | any(. == ".claude/settings.json")' config/ccreality_governed_paths.json
jq -e '.allow | any(. == ".mcp.json")' config/ccreality_governed_paths.json
```

## Task 7.2 — Write `.claude/settings.json`

Replace the empty `{}` with the full content from [../06_SETTINGS_JSON_SPEC.md](../06_SETTINGS_JSON_SPEC.md) §"`.claude/settings.json`".

Verify:
```bash
jq -e . .claude/settings.json >/dev/null
jq -e '.permissions.allow | type == "array"' .claude/settings.json
jq -e '.hooks.SessionStart' .claude/settings.json
jq -e '.env.CONTEXTGRAPH_ROOT' .claude/settings.json
```

## Task 7.3 — Write `.mcp.json`

Use the content from [../06_SETTINGS_JSON_SPEC.md](../06_SETTINGS_JSON_SPEC.md) §"`.mcp.json`".

Verify:
```bash
jq -e '.mcpServers.cgreality.type == "stdio"' .mcp.json
jq -e '.mcpServers.cgreality.command' .mcp.json
jq -e '.mcpServers.cgreality.args' .mcp.json
```

## Task 7.4 — Write `.claude/loop.md`

Use the body from [../11_LOOP_PROMPT.md](../11_LOOP_PROMPT.md) §"File: `.claude/loop.md`".

Verify:
```bash
test -s .claude/loop.md
grep -q "Iteration procedure" .claude/loop.md
grep -q "mcp__cgreality__" .claude/loop.md
```

## Task 7.5 — Write the 5 subagent files

Each goes under `.claude/agents/cc-reality-<name>.md`. Use [../08_SUBAGENTS_SPEC.md](../08_SUBAGENTS_SPEC.md) for the verbatim content of each.

Files to create:
- `.claude/agents/cc-reality-diagnostician.md`
- `.claude/agents/cc-reality-harness-editor.md`
- `.claude/agents/cc-reality-reward-analyst.md`
- `.claude/agents/cc-reality-source-of-truth-readback.md`
- `.claude/agents/cc-reality-swe-failure-triage.md`

Each starts with YAML frontmatter (`---name: ... description: ... tools: ... model: ... ---`).

Verify:
```bash
for a in diagnostician harness-editor reward-analyst source-of-truth-readback swe-failure-triage; do
  test -f .claude/agents/cc-reality-$a.md
  head -1 .claude/agents/cc-reality-$a.md | grep -q '^---$' || { echo "FAIL: cc-reality-$a frontmatter"; exit 1; }
  grep -q "^name: cc-reality-" .claude/agents/cc-reality-$a.md
done
```

## Task 7.6 — Write the 4 skill files

Each is `.claude/skills/ccreality-<name>/SKILL.md`. Mkdir first:
```bash
mkdir -p .claude/skills/ccreality-iterate
mkdir -p .claude/skills/ccreality-smoke
mkdir -p .claude/skills/ccreality-trigger-check
mkdir -p .claude/skills/ccreality-readback
```

Use [../09_SKILLS_SPEC.md](../09_SKILLS_SPEC.md) for verbatim content of each `SKILL.md`.

Verify:
```bash
for s in iterate smoke trigger-check readback; do
  test -f .claude/skills/ccreality-$s/SKILL.md
  head -1 .claude/skills/ccreality-$s/SKILL.md | grep -q '^---$'
  grep -q "^name: ccreality-" .claude/skills/ccreality-$s/SKILL.md
  grep -q "^description:" .claude/skills/ccreality-$s/SKILL.md
done
```

## Task 7.7 — Smoke test the wiring

```bash
cd /home/cabdru/contextgraph
command -v claude >/dev/null 2>&1 || { echo "claude CLI is required for Phase 7 smoke wiring"; exit 1; }
claude --print "List all MCP servers loaded and the count of tools each provides. Return just JSON of {server: tool_count}." --output-format json \
  | jq -r '.result' \
  | jq -e 'has("cgreality")'
```

If `claude` is not installed or cannot see `cgreality`, Phase 7 is blocked. Record the exact missing dependency or MCP configuration failure; do not skip the check.

## Task 7.8 — Verify

Execute [04_VERIFICATION.md](04_VERIFICATION.md). All clauses pass.

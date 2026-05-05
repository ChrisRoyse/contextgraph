# Phase 7 — Recovery

## Failure: settings.json malformed

**Symptom:** `jq -e . .claude/settings.json` fails. Claude Code refuses to load the project.

**Cause:** Trailing comma, unescaped quotes, or merged keys during copy-paste.

**Fix:**
1. Use `jq -e . < .claude/settings.json` to find the syntax error line.
2. Re-copy from [../06_SETTINGS_JSON_SPEC.md](../06_SETTINGS_JSON_SPEC.md).
3. Use a JSON formatter to normalize: `jq . .claude/settings.json > /tmp/s.json && mv /tmp/s.json .claude/settings.json`.

## Failure: .mcp.json env-var expansion not working

**Symptom:** Claude Code logs `command not found: ${CONTEXTGRAPH_ROOT}/target/debug/context-graph-mcp`.

**Cause:** Claude Code's MCP loader either doesn't expand env vars, or `CONTEXTGRAPH_ROOT` isn't set in the operator's shell when Claude starts.

**Fix:**
1. Check the loader: `claude mcp list` (or wherever the MCP listing lives) — does it show resolved paths?
2. If env vars don't expand, replace with literals:
   ```json
   "command": "/home/cabdru/contextgraph/target/debug/context-graph-mcp",
   "args": ["--mode", "reality-loop"],
   "env": {
     "CONTEXTGRAPH_ROOT": "/home/cabdru/contextgraph",
     "CGREALITY_GOVERNED_PATHS": "/home/cabdru/contextgraph/config/ccreality_governed_paths.json",
     "CGREALITY_RECOMMENDATION_SCHEMA": "/home/cabdru/contextgraph/config/ccreality_recommendation_schema.json"
   }
   ```
3. Re-launch Claude Code; verify with `/mcp` slash command (interactive) that cgreality shows as connected.

## Failure: subagent description too long

**Symptom:** `grep "^description:" .claude/agents/cc-reality-X.md | wc -c` returns >1100.

**Cause:** Description sentence is too verbose (Anthropic Agent Skills validation caps at 1024 chars).

**Fix:** Edit the description in [../08_SUBAGENTS_SPEC.md](../08_SUBAGENTS_SPEC.md) AND in the file. Keep it to one sentence with PROACTIVELY/AFTER/WHEN keyword and the trigger.

## Failure: skill name has uppercase or invalid chars

**Symptom:** Claude Code refuses to register the skill.

**Cause:** Per Anthropic skills, name must be `[a-z0-9-]+` ≤64 chars.

**Fix:** All ccreality skill names are lowercase + hyphens (`ccreality-iterate`, etc.). Confirm in the file:
```
name: ccreality-iterate
```
Not `Ccreality_Iterate` etc.

## Failure: hook fires but tool reference broken

**Symptom:** Verification 7.V7 reports WARN for a `mcp__cgreality__X` that doesn't exist.

**Cause:** Subagent file references a tool that wasn't implemented in Phase 4.

**Fix:** Either:
- Implement the missing tool in Phase 4.
- Remove the reference from the subagent's `tools:` line.
- Confirm spelling (e.g., `mcp__cgreality__reality_signal` not `mcp__cgreality__reality_signal_packet`).

## Failure: .claude/settings.json model mismatch

**Symptom:** Claude Code loads model `opus` but the API key is for a different tier.

**Cause:** Operator's account doesn't have access to the requested model.

**Fix:** Change `"model": "opus"` to `"model": "sonnet"` in `.claude/settings.json` if Opus isn't available. Document the tradeoff in the operator's local notes.

## Failure: existing agent or skill conflict

**Symptom:** During Task 7.5/7.6, you find existing `cc-reality-*` or `ccreality-*` files.

**Cause:** A previous attempt at this phase, or another system that namespaced similarly.

**Fix:**
1. Inspect existing files: `cat .claude/agents/cc-reality-*.md 2>/dev/null`.
2. If they're from a previous failed attempt, back up and overwrite.
3. If they're from a different system, choose a different namespace (e.g., `cgr-` prefix). Update all references in [../06_SETTINGS_JSON_SPEC.md](../06_SETTINGS_JSON_SPEC.md), [../08_SUBAGENTS_SPEC.md](../08_SUBAGENTS_SPEC.md), [../09_SKILLS_SPEC.md](../09_SKILLS_SPEC.md), [../10_CLAUDE_MD_PROJECT_MEMORY.md](../10_CLAUDE_MD_PROJECT_MEMORY.md), [../11_LOOP_PROMPT.md](../11_LOOP_PROMPT.md). This is a significant rename; budget extra time.

## Diagnostic dump

```bash
cd /home/cabdru/contextgraph
echo "=== Files ==="
ls -la .claude/settings.json .mcp.json .claude/loop.md
ls -la .claude/agents/cc-reality-*.md
ls -la .claude/skills/ccreality-*/SKILL.md
echo "=== JSON validity ==="
jq -e . .claude/settings.json >/dev/null && echo "settings.json OK" || echo "settings.json BAD"
jq -e . .mcp.json >/dev/null && echo ".mcp.json OK" || echo ".mcp.json BAD"
echo "=== Subagent frontmatter ==="
for f in .claude/agents/cc-reality-*.md; do
  echo "--- $f ---"
  head -10 "$f"
done
echo "=== Skill frontmatter ==="
for f in .claude/skills/ccreality-*/SKILL.md; do
  echo "--- $f ---"
  head -8 "$f"
done
```

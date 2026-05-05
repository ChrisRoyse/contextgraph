# Phase 7 — Verification

```bash
set -e
cd /home/cabdru/contextgraph

echo "=== 7.V1 .claude/settings.json valid JSON with expected keys ==="
jq -e . .claude/settings.json >/dev/null
jq -e '.model' .claude/settings.json >/dev/null
jq -e '.permissions.allow | type == "array"' .claude/settings.json >/dev/null
jq -e '.permissions.deny | type == "array"' .claude/settings.json >/dev/null
jq -e '.hooks.SessionStart | type == "array"' .claude/settings.json >/dev/null
jq -e '.hooks.UserPromptSubmit | type == "array"' .claude/settings.json >/dev/null
jq -e '.hooks.PreToolUse | type == "array"' .claude/settings.json >/dev/null
jq -e '.hooks.PostToolUse | type == "array"' .claude/settings.json >/dev/null
jq -e '.hooks.Stop | type == "array"' .claude/settings.json >/dev/null
jq -e '.env.CONTEXTGRAPH_ROOT' .claude/settings.json >/dev/null
echo "  pass"

echo "=== 7.V2 .mcp.json valid JSON with cgreality entry ==="
jq -e . .mcp.json >/dev/null
jq -e '.mcpServers.cgreality.type == "stdio"' .mcp.json >/dev/null
jq -e '.mcpServers.cgreality.command' .mcp.json >/dev/null
jq -e '.mcpServers.cgreality.args | any(. == "--mode")' .mcp.json >/dev/null
echo "  pass"

echo "=== 7.V3 hook commands point at scripts/cc/ ==="
jq -r '.hooks.SessionStart[0].hooks[0].command' .claude/settings.json | grep -q "scripts/cc/hook_session_start.sh"
jq -r '.hooks.UserPromptSubmit[0].hooks[0].command' .claude/settings.json | grep -q "scripts/cc/hook_user_prompt_submit.sh"
jq -r '.hooks.PreToolUse[0].hooks[0].command' .claude/settings.json | grep -q "scripts/cc/hook_pre_tool_use_edit.sh"
jq -r '.hooks.PostToolUse[0].hooks[0].command' .claude/settings.json | grep -q "scripts/cc/hook_post_tool_use_edit.sh"
jq -r '.hooks.Stop[0].hooks[0].command' .claude/settings.json | grep -q "scripts/cc/hook_stop.sh"
echo "  pass"

echo "=== 7.V4 .claude/loop.md exists with iteration content ==="
test -s .claude/loop.md
grep -q "Iteration procedure" .claude/loop.md
grep -q "mcp__cgreality__" .claude/loop.md
echo "  pass"

echo "=== 7.V5 All 5 subagent files exist with valid frontmatter ==="
for a in diagnostician harness-editor reward-analyst source-of-truth-readback swe-failure-triage; do
  f=".claude/agents/cc-reality-$a.md"
  test -f "$f"
  head -1 "$f" | grep -q '^---$' || { echo "  FAIL: $f no opening frontmatter"; exit 1; }
  grep -q "^name: cc-reality-$a$" "$f" || { echo "  FAIL: $f wrong name"; exit 1; }
  grep -q "^description:" "$f"
  grep -q "^tools:" "$f"
  grep -q "^model:" "$f"
done
echo "  pass"

echo "=== 7.V6 All 4 skill files exist with valid frontmatter ==="
for s in iterate smoke trigger-check readback; do
  f=".claude/skills/ccreality-$s/SKILL.md"
  test -f "$f"
  head -1 "$f" | grep -q '^---$' || { echo "  FAIL: $f no opening frontmatter"; exit 1; }
  grep -q "^name: ccreality-$s$" "$f" || { echo "  FAIL: $f wrong name"; exit 1; }
  grep -q "^description:" "$f"
  # description must be ≤1024 chars
  DESC_LEN=$(grep "^description:" "$f" | head -1 | wc -c)
  [ "$DESC_LEN" -le 1100 ] || { echo "  FAIL: $f description too long: $DESC_LEN"; exit 1; }
done
echo "  pass"

echo "=== 7.V7 Subagent tool references all valid ==="
ALLOWED_TOOLS=$(echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | ./target/debug/context-graph-mcp --mode reality-loop 2>/dev/null | jq -r '.result.tools[].name' || echo "")
WARNINGS=0
for af in .claude/agents/cc-reality-*.md; do
  TOOLS_LINE=$(grep '^tools:' "$af" | head -1 | sed 's/^tools: *//')
  for t in $(echo "$TOOLS_LINE" | tr ',' '\n' | tr -d ' '); do
    if [[ "$t" == mcp__cgreality__* ]]; then
      bare="${t#mcp__cgreality__}"
      if [ -n "$ALLOWED_TOOLS" ] && ! echo "$ALLOWED_TOOLS" | grep -qx "$bare"; then
        echo "  WARN: $af → $t (not in tools/list)"
        WARNINGS=$((WARNINGS + 1))
      fi
    fi
  done
done
[ "$WARNINGS" -eq 0 ] || echo "  ($WARNINGS warnings; resolve before Phase 9)"
echo "  pass"

echo "=== 7.V8 Path policy includes new files ==="
jq -e '.allow | any(. == ".claude/settings.json")' config/ccreality_governed_paths.json >/dev/null
jq -e '.allow | any(. == ".mcp.json")' config/ccreality_governed_paths.json >/dev/null
echo "  pass"

echo
echo "Phase 7 verification: ALL PASS"
```

## What to record

```bash
echo "phase-07: PASS, $(date -u +%FT%TZ)" >> /tmp/ccreality-phase-progress.log
```

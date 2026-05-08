# Phase 8 — Verification

```bash
set -e
cd /home/cabdru/contextgraph

echo "=== 8.V1 Addendum section header present ==="
grep -q "ccreality — Outer Reality-Loop Optimizer Role" CLAUDE.md
echo "  pass"

echo "=== 8.V2 Reality-shift principle subsection present ==="
grep -q "Reality-shift principle" CLAUDE.md
echo "  pass"

echo "=== 8.V3 Recommendation tool name referenced ==="
grep -q "mcp__cgreality__optimizer_record_recommendation" CLAUDE.md
echo "  pass"

echo "=== 8.V4 Addendum appears exactly once ==="
COUNT=$(grep -c "ccreality — Outer Reality-Loop Optimizer Role" CLAUDE.md)
[ "$COUNT" -eq 1 ] || { echo "  FAIL: header appears $COUNT times (should be 1)"; exit 1; }
echo "  pass"

echo "=== 8.V5 Addendum is at the END of the file ==="
LAST_HEADER_LINE=$(grep -n "^## " CLAUDE.md | tail -1)
echo "  $LAST_HEADER_LINE"
echo "$LAST_HEADER_LINE" | grep -q "ccreality"
echo "  pass"

echo "=== 8.V6 Root CLAUDE.md is trackable, local variants are ignored ==="
! git check-ignore -q CLAUDE.md
git check-ignore -q CLAUDE.local.md
git check-ignore -q clipcannon/CLAUDE.md
echo "  pass"

echo "=== 8.V7 Project-memory files are governed paths ==="
jq -e '.allow | any(. == "CLAUDE.md")' config/ccreality_governed_paths.json >/dev/null
jq -e '.allow | any(. == ".gitignore")' config/ccreality_governed_paths.json >/dev/null
echo "  pass"

echo "=== 8.V8 Appended block matches the source spec ==="
SRC_START=$(grep -n '^---$' docs/ccreality/10_CLAUDE_MD_PROJECT_MEMORY.md | head -1 | cut -d: -f1)
SRC_END=$(grep -n '^---$' docs/ccreality/10_CLAUDE_MD_PROJECT_MEMORY.md | tail -1 | cut -d: -f1)
sed -n "${SRC_START},${SRC_END}p" docs/ccreality/10_CLAUDE_MD_PROJECT_MEMORY.md > /tmp/phase8-source-addendum.md
CLAUDE_START=$(grep -n "^## ccreality — Outer Reality-Loop Optimizer Role$" CLAUDE.md | cut -d: -f1)
CLAUDE_START=$((CLAUDE_START-2))
tail -n +"$CLAUDE_START" CLAUDE.md > /tmp/phase8-claude-addendum.md
diff -u /tmp/phase8-source-addendum.md /tmp/phase8-claude-addendum.md >/dev/null
echo "  pass"

echo "=== 8.V9 Claude Code loads the addendum as project memory ==="
command -v claude >/dev/null 2>&1
rm -rf /tmp/ccreality-phase8-claude-smoke-cache
mkdir -p /tmp/ccreality-phase8-claude-smoke-cache
CGREALITY_CACHE_DIR=/tmp/ccreality-phase8-claude-smoke-cache \
  claude --print 'From your loaded project memory, return exactly the heading line of the ccreality section and the next two non-empty lines. Do not use tools. Return only those three lines.' \
    --model haiku \
    --output-format json \
    --max-budget-usd 0.25 \
    --no-session-persistence \
    --tools "" \
  | jq -r '.result' \
  | grep -q "CLAUDE.md project-memory discovery"
echo "  pass"

echo
echo "Phase 8 verification: ALL PASS"
```

## What to record

```bash
echo "phase-08: PASS, $(date -u +%FT%TZ)" >> /tmp/ccreality-phase-progress.log
```

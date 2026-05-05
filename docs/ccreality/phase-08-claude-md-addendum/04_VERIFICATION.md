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

echo
echo "Phase 8 verification: ALL PASS"
```

## What to record

```bash
echo "phase-08: PASS, $(date -u +%FT%TZ)" >> /tmp/ccreality-phase-progress.log
```

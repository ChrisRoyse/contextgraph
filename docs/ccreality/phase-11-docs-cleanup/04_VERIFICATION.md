# Phase 11 — Verification

```bash
set -e
cd /home/cabdru/contextgraph

echo "=== 11.V1 docs/reality/20_openai_outer_optimizer.md deleted ==="
test ! -f docs/reality/20_openai_outer_optimizer.md
echo "  pass"

echo "=== 11.V2 docs2/DEEPSEEK_API_GUIDE.md deleted ==="
test ! -f docs2/DEEPSEEK_API_GUIDE.md
echo "  pass"

echo "=== 11.V3 docs/reality/00_index.md references ccreality ==="
grep -q ccreality docs/reality/00_index.md
echo "  pass"

echo "=== 11.V4 docs/reality/21_grand_scale_reality_autoresearch.md has Superseded header ==="
head -5 docs/reality/21_grand_scale_reality_autoresearch.md | grep -qiE "supers|status"
echo "  pass"

echo "=== 11.V5 docs2/CLAUDE_CODE_REALITY_LOOP_GUIDE.md has Superseded header ==="
head -5 docs2/CLAUDE_CODE_REALITY_LOOP_GUIDE.md | grep -qiE "supers"
echo "  pass"

echo
echo "Phase 11 verification: ALL PASS"
```

## What to record

```bash
echo "phase-11: PASS, $(date -u +%FT%TZ)" >> /tmp/ccreality-phase-progress.log
```

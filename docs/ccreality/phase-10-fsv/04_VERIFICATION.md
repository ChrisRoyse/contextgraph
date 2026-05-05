# Phase 10 — Verification

```bash
set -euo pipefail
cd /home/cabdru/contextgraph

echo "=== 10.V1 FSV master script exists ==="
test -x scripts/cc/tests/fsv.sh
echo "  pass"

echo "=== 10.V2 At least 30 case scripts exist ==="
COUNT=$(ls scripts/cc/tests/fsv/cases/*.sh | wc -l)
echo "  case count: $COUNT"
[ "$COUNT" -ge 30 ]
echo "  pass"

echo "=== 10.V3 Master runner produces a results doc ==="
bash scripts/cc/tests/fsv.sh
DATE=$(date -u +%Y-%m-%d)
test -f docs/ccreality/FSV_RESULTS_${DATE}.md
echo "  pass"

echo "=== 10.V4 All A-E cases PASS in results ==="
DATE=$(date -u +%Y-%m-%d)
AE_COUNT=$(awk '/^\| [A-E][0-9]+/ {count++} END {print count+0}' docs/ccreality/FSV_RESULTS_${DATE}.md)
FAIL_COUNT=$(awk '/^\| [A-E][0-9]+/ && / FAIL / {count++} END {print count+0}' docs/ccreality/FSV_RESULTS_${DATE}.md)
BLOCKED_COUNT=$(awk '/^\| [A-E][0-9]+/ && / BLOCKED / {count++} END {print count+0}' docs/ccreality/FSV_RESULTS_${DATE}.md)
[ "$AE_COUNT" -gt 0 ]
[ "$FAIL_COUNT" -eq 0 ]
[ "$BLOCKED_COUNT" -eq 0 ]
echo "  pass"

echo "=== 10.V5 Results doc summary line shows zero failures ==="
grep -q "FAIL: 0" docs/ccreality/FSV_RESULTS_${DATE}.md
echo "  pass"

echo
echo "Phase 10 verification: ALL PASS"
```

## What to record

```bash
echo "phase-10: PASS, $(date -u +%FT%TZ), FSV results=$DATE" >> /tmp/ccreality-phase-progress.log
```

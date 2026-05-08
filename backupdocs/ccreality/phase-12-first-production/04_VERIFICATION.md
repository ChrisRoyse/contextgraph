# Phase 12 — Verification

```bash
set -euo pipefail
cd /home/cabdru/contextgraph

echo "=== 12.V1 Real attempt artifact exists ==="
test -d /tmp/ccreality-prod-1
ATTEMPT_SUMMARY=$(find /tmp/ccreality-prod-1 -name attempt-summary.json | head -1)
test -n "$ATTEMPT_SUMMARY"
echo "  attempt summary: $ATTEMPT_SUMMARY"
echo "  pass"

echo "=== 12.V2 Recommendation artifact exists and validates ==="
RECOMMENDATION=$(find /tmp/ccreality-prod-1 -path '*/claude-code-optimizer/recommendation-*.json' | head -1)
test -n "$RECOMMENDATION"
echo "  recommendation: $RECOMMENDATION"
jq -e '.status' "$RECOMMENDATION" >/dev/null
jq -e '.source_of_truth' "$RECOMMENDATION" >/dev/null
jq -r '.status' "$RECOMMENDATION" | grep -qE '^(changed|no_change|blocked|error)$'
echo "  pass"

echo "=== 12.V3 Recommendation validates against schema ==="
python3 -c "import jsonschema" >/dev/null
python3 -c "
import json, jsonschema
schema = json.load(open('config/ccreality_recommendation_schema.json'))
rec = json.load(open('$RECOMMENDATION'))
jsonschema.validate(rec, schema)
print('schema-validated')
"
echo "  pass"

echo "=== 12.V4 Shift log has entries ==="
SHIFT_LOG=$(find /tmp/ccreality-prod-1/cgreality-shift-log -name '*.jsonl' | head -1)
test -s "$SHIFT_LOG"
LINE_COUNT=$(wc -l < "$SHIFT_LOG")
[ "$LINE_COUNT" -ge 1 ]
echo "  $LINE_COUNT shift entries"
echo "  pass"

echo "=== 12.V5 Cutover commits exist on the branch ==="
COMMIT_COUNT=$(git log --oneline ccreality-cutover ^main | wc -l)
echo "  commits on ccreality-cutover ahead of main: $COMMIT_COUNT"
[ "$COMMIT_COUNT" -ge 1 ]
echo "  pass"

echo "=== 12.V6 All 13 phases recorded as PASS ==="
test -f /tmp/ccreality-phase-progress.log
PHASE_PASS_COUNT=$(grep -c PASS /tmp/ccreality-phase-progress.log)
echo "  phase PASS lines: $PHASE_PASS_COUNT"
[ "$PHASE_PASS_COUNT" -ge 13 ]
echo "  pass"

echo "=== 12.V7 No DeepSeek/OpenAI text remains anywhere significant ==="
HIT_FILE=/tmp/phase12-openai-hits.txt
set +e
rg -l "deepseek|openai|gpt-5" \
  --hidden \
  -g '!target/' -g '!.git/' -g '!docs/ccreality/' -g '!docs2/CLAUDE_CODE_REALITY_LOOP_GUIDE.md' \
  -g '!docs/reality/21_grand_scale_reality_autoresearch.md' \
  -g '!.claude/projects/' \
  -g '!scripts/exp2/' -g '!models/' -g '!tasks/' -g '!docs/PAPER.md' -g '!docs/alldata.md' \
  -g '!docs/leapableai_youtube_transcripts.md' \
  -g '!crates/context-graph-mcp/src/handlers/tools/entity_tools.rs' \
  -g '!crates/context-graph-benchmark/' \
  -g '!crates/context-graph-core/src/types/memory_node/' \
  -g '!config/default.toml' \
  -g '!backupdocs/' -g '!clipcannon/' -g '!gemmafrontend/' \
  -g '!scripts/download_models.py' -g '!zenodo_new_upload_initial.yml' \
  > "$HIT_FILE"
RG_STATUS=$?
set -e
if [ "$RG_STATUS" -eq 0 ]; then
  HITS=$(head -10 "$HIT_FILE")
elif [ "$RG_STATUS" -eq 1 ]; then
  HITS=""
else
  echo "  FAIL: rg returned status $RG_STATUS" >&2
  exit "$RG_STATUS"
fi
if [ -n "$HITS" ]; then
  echo "  FAIL: unexpected hits:"
  echo "$HITS"
  exit 1
fi
echo "  pass"

echo
echo "Phase 12 verification: ALL PASS"
echo
echo "Cutover complete. Branch ccreality-cutover is ready for review."
```

## What to record

```bash
echo "phase-12: PASS, $(date -u +%FT%TZ), commit=$(git rev-parse HEAD)" >> /tmp/ccreality-phase-progress.log
```

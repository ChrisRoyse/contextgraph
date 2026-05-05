# Phase 10 — Recovery

## Failure: most A cases skip because of missing fixtures

**Diagnosis:** `/tmp/fsv-A2_*.log` says "fixture not implemented".

**Fix:** A2/A4/A5 require synthetic fixtures or real Gemma. For the first pass, mark as SKIP. They can be added in a follow-up sprint.

## Failure: C5 (external drift)

**Symptom:** `external_drift_detected` not returned.

**Cause:** The shift-log lookup is keyed on a different path representation than what `sha256sum` returns.

**Fix:** Normalize all paths to project-relative without leading `./`. Add a `normalize_path` helper in `crates/context-graph-mcp/src/handlers/tools/reality_loop/helpers.rs` and apply at every shift log read/write.

## Failure: D cases all FAIL

**Symptom:** Phase 6 hook unit tests pass standalone but FSV wrapper fails.

**Cause:** FSV wrapper doesn't pipe stdin correctly OR requires a different working directory.

**Fix:** Ensure each D case `cd`s to project root and pipes stdin:
```bash
echo '{}' | bash scripts/cc/tests/hooks/test_X.sh
```

## Failure: E cases blow through cost budget

**Symptom:** F1 reports cost > $1.00.

**Cause:** Skill prompt is too verbose, OR `cc-reality-harness-editor` is doing too much work.

**Fix:** Tighten skill prompts. Cap `--max-turns 15`. Document tolerance in the results doc.

## Diagnostic dump

```bash
cd /home/cabdru/contextgraph
echo "=== Cases ==="
ls scripts/cc/tests/fsv/cases/ | head -50
echo "=== Recent failures ==="
ls -t /tmp/fsv-*.log | head -10
echo "=== Latest results ==="
DATE=$(date -u +%Y-%m-%d)
test -f docs/ccreality/FSV_RESULTS_${DATE}.md && tail -30 docs/ccreality/FSV_RESULTS_${DATE}.md
```

# Phase 10 — Tasks

## Task 10.1 — Create directory structure

```bash
cd /home/cabdru/contextgraph
mkdir -p scripts/cc/tests/fsv/cases
```

## Task 10.2 — Write `scripts/cc/tests/fsv.sh` master runner

```bash
#!/usr/bin/env bash
set -uo pipefail
PROJECT_ROOT="${PROJECT_ROOT:-/home/cabdru/contextgraph}"
FSV_ROOT="$PROJECT_ROOT/scripts/cc/tests/fsv"
RESULTS_DATE="$(date -u +%Y-%m-%d)"
RESULTS_FILE="$PROJECT_ROOT/docs/ccreality/FSV_RESULTS_${RESULTS_DATE}.md"

cd "$PROJECT_ROOT"

echo "# FSV Results — $RESULTS_DATE" > "$RESULTS_FILE"
echo >> "$RESULTS_FILE"
echo "Branch: $(git branch --show-current)" >> "$RESULTS_FILE"
echo "HEAD: $(git rev-parse HEAD)" >> "$RESULTS_FILE"
echo >> "$RESULTS_FILE"
echo "## Per-case results" >> "$RESULTS_FILE"
echo >> "$RESULTS_FILE"
echo "| Case | Status | Time (s) | Notes |" >> "$RESULTS_FILE"
echo "|------|--------|----------|-------|" >> "$RESULTS_FILE"

PASS=0
FAIL=0

for case in $(ls "$FSV_ROOT/cases" | sort); do
  CASE_NAME="${case%.sh}"
  START=$(date +%s)
  if bash "$FSV_ROOT/cases/$case" > /tmp/fsv-${CASE_NAME}.log 2>&1; then
    STATUS="PASS"
    PASS=$((PASS+1))
    NOTE=""
  else
    STATUS="FAIL"
    FAIL=$((FAIL+1))
    NOTE="see /tmp/fsv-${CASE_NAME}.log"
  fi
  END=$(date +%s)
  ELAPSED=$((END-START))
  echo "| $CASE_NAME | $STATUS | $ELAPSED | $NOTE |" >> "$RESULTS_FILE"
  echo "[$STATUS] $CASE_NAME (${ELAPSED}s)"
done

echo >> "$RESULTS_FILE"
echo "## Summary" >> "$RESULTS_FILE"
echo >> "$RESULTS_FILE"
echo "PASS: $PASS, FAIL: $FAIL" >> "$RESULTS_FILE"
echo
echo "Results written to $RESULTS_FILE"
echo "Total: $((PASS+FAIL)), pass: $PASS, fail: $FAIL"

[ "$FAIL" -eq 0 ]
```

`chmod +x scripts/cc/tests/fsv.sh`.

## Task 10.3 — Implement per-case scripts

For each case in [../18_FSV_PLAN.md](../18_FSV_PLAN.md), write a one-file script under `scripts/cc/tests/fsv/cases/` that:
1. Sets up the synthetic state (or asserts the state from a prior case).
2. Runs the action under test.
3. Asserts the expected outcome.
4. Cleans up.
5. Exits 0 on PASS, non-zero on FAIL.

See [03_FILE_SPECS.md](03_FILE_SPECS.md) for case-script templates.

A1 example (`scripts/cc/tests/fsv/cases/A1_smoke_sha_match.sh`):

```bash
#!/usr/bin/env bash
set -euo pipefail
ROOT="/tmp/cgreality-fsv-A1-$$"
mkdir -p "$ROOT"
trap "rm -rf '$ROOT'" EXIT
cd /home/cabdru/contextgraph
./target/debug/reality-loop smoke --runtime-root "$ROOT" --label A1 >/dev/null
SUMMARY=$(find "$ROOT" -name attempt-summary.json | head -1)
[ -n "$SUMMARY" ]
SHA=$(sha256sum "$SUMMARY" | awk '{print $1}')
# Verify the file is consistent with itself (trivial round-trip)
SHA2=$(sha256sum "$SUMMARY" | awk '{print $1}')
[ "$SHA" = "$SHA2" ]
```

## Task 10.4 — Run the FSV

```bash
cd /home/cabdru/contextgraph
bash scripts/cc/tests/fsv.sh 2>&1 | tee /tmp/fsv-run-$(date +%s).log
echo "exit=$?"
```

## Task 10.5 — Iterate on failures

Each FAIL has a log at `/tmp/fsv-<CaseName>.log`. Inspect, fix, re-run.

Common failure patterns:
- C5 (external drift): hard to test without coordinated processes; may need a small Rust helper.
- D7 (PostToolUse cargo check): timing-dependent; add buffer.
- E2 (drift mid-edit): requires careful coordination between Claude Code session and external mutator.
- F1 (cost): may depend on actual API call cost; accept if within tolerance.

## Task 10.6 — Verify

[04_VERIFICATION.md](04_VERIFICATION.md). Master runner exits 0; results doc generated.

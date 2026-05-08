# Phase 2 — Verification

## Canonical end-state block

```bash
set -e
cd /home/cabdru/contextgraph

echo "=== 2.V1 Build ==="
cargo build -p context-graph-cli --bin reality-loop --quiet
echo "  pass"

echo "=== 2.V2 --help shows all four subcommands ==="
HELP="$(./target/debug/reality-loop --help 2>&1)"
echo "$HELP" | grep -q "attempt"
echo "$HELP" | grep -q "smoke"
echo "$HELP" | grep -q "replay"
echo "$HELP" | grep -q "verify"
echo "  pass"

echo "=== 2.V3 attempt --help still has the engine flags ==="
ATTEMPT_HELP="$(./target/debug/reality-loop attempt --help 2>&1)"
echo "$ATTEMPT_HELP" | grep -q -- "--task-manifest"
echo "$ATTEMPT_HELP" | grep -q -- "--runtime-root"
echo "$ATTEMPT_HELP" | grep -q -- "--max-attempts"
echo "$ATTEMPT_HELP" | grep -q -- "--official-eval-each-attempt"
echo "  pass"

echo "=== 2.V4 No deepseek/openai flags survived ==="
! echo "$ATTEMPT_HELP" | grep -qiE "deepseek|openai|optimizer-trigger|optimizer-each"
echo "  pass"

echo "=== 2.V5 Smoke runs and writes artifacts ==="
SMOKE_ROOT="/tmp/cgreality-phase02-verify-$(date +%s)"
./target/debug/reality-loop smoke --runtime-root "$SMOKE_ROOT" --label phase02-verify >/tmp/phase02-smoke.json 2>&1
test -d "$SMOKE_ROOT"
RUN_DIR=$(find "$SMOKE_ROOT" -maxdepth 1 -mindepth 1 -type d | head -1)
test -n "$RUN_DIR"
ATTEMPT_DIR=$(find "$RUN_DIR" -name 'smoke-attempt-1' -type d | head -1)
test -n "$ATTEMPT_DIR"
test -s "$ATTEMPT_DIR/attempt-summary.json"
test -s "$ATTEMPT_DIR/reality-signal-packet.json"
test -s "$ATTEMPT_DIR/problem-reality-packet.json"
echo "  pass; smoke artifacts at $ATTEMPT_DIR"

echo "=== 2.V6 Active runtime root file written ==="
test -s ~/.cache/cgreality/active_runtime_root
ACTIVE_ROOT="$(cat ~/.cache/cgreality/active_runtime_root)"
[ "$ACTIVE_ROOT" = "$(cd "$SMOKE_ROOT" && pwd)" ]
test -s ~/.cache/cgreality/active_target_instance
echo "  pass; active root = $ACTIVE_ROOT"

echo "=== 2.V7 Replay round-trips a smoke attempt ==="
./target/debug/reality-loop replay --attempt-summary-path "$ATTEMPT_DIR/attempt-summary.json" --label phase02 >/tmp/phase02-replay.json 2>&1
test -s "$ATTEMPT_DIR/reality-signal-packet.phase02.json"
echo "  pass"

echo "=== 2.V8 Verify enumerates artifacts ==="
./target/debug/reality-loop verify --runtime-root "$SMOKE_ROOT" >/tmp/phase02-verify.json 2>&1
JSON_COUNT=$(jq -r '.json_artifact_count' /tmp/phase02-verify.json)
[ "$JSON_COUNT" -ge 3 ]
echo "  pass; verify reported $JSON_COUNT artifacts"

echo "=== 2.V9 Force-fail returns non-zero ==="
if ./target/debug/reality-loop smoke --runtime-root /tmp/cgreality-phase02-fail --label fail-test --force-fail >/tmp/phase02-fail.json 2>/tmp/phase02-fail.err; then
  echo "  expected non-zero"
  exit 1
fi
grep -q "RUST_REALITY_SMOKE_FORCED_FAIL" /tmp/phase02-fail.err
echo "  pass"

echo "=== 2.V10 Engine tests still pass ==="
cargo test -p context-graph-cli --bin reality-loop --quiet -- --skip ignored
cargo test -p context-graph-cli --lib governed_edit --quiet
echo "  pass"

echo "=== 2.V11 Final successful handoff smoke ==="
READY_ROOT="/tmp/cgreality-phase02-ready"
rm -rf "$READY_ROOT"
./target/debug/reality-loop smoke --runtime-root "$READY_ROOT" --label phase02-ready >/tmp/phase02-ready-smoke.json
test -s ~/.cache/cgreality/active_runtime_root
[ "$(cat ~/.cache/cgreality/active_runtime_root)" = "$READY_ROOT" ]
[ "$(cat ~/.cache/cgreality/active_target_instance)" = "cgreality-smoke-phase02-ready" ]
READY_ATTEMPT_DIR=$(find "$READY_ROOT" -name 'smoke-attempt-1' -type d | head -1)
test -s "$READY_ATTEMPT_DIR/attempt-summary.json"
test -s "$READY_ATTEMPT_DIR/reality-signal-packet.json"
test -s "$READY_ATTEMPT_DIR/problem-reality-packet.json"
echo "  pass; ready smoke artifacts at $READY_ATTEMPT_DIR"

echo
echo "Phase 2 verification: ALL PASS"
```

## Per-clause meaning

| Clause | What it asserts |
|--------|----------------|
| 2.V1 | Build succeeds after refactor |
| 2.V2 | Top-level help lists all four subcommands |
| 2.V3 | Attempt subcommand preserves the original engine flags |
| 2.V4 | No openai/deepseek flags slipped back |
| 2.V5 | Smoke subcommand actually writes artifacts (Phase 9 will deeply verify) |
| 2.V6 | Active-root cache file gets written by smoke |
| 2.V7 | Replay subcommand functional (round-trips signal packet) |
| 2.V8 | Verify subcommand enumerates artifacts |
| 2.V9 | --force-fail correctly produces non-zero exit + error JSON |
| 2.V10 | All Phase 1 tests still pass after refactor |
| 2.V11 | Final active-root state points at a successful smoke runtime for Phase 3 |

## Cleanup after verification

```bash
rm -rf /tmp/cgreality-phase02-verify-* /tmp/cgreality-phase02-fail
rm -f /tmp/phase02-smoke.json /tmp/phase02-replay.json /tmp/phase02-verify.json /tmp/phase02-fail.json /tmp/phase02-fail.err
```

Don't delete `/tmp/cgreality-phase02-ready` or `~/.cache/cgreality/active_runtime_root` — they are the handoff source of truth for Phase 3+.

## What to record

```bash
echo "phase-02: PASS, $(date -u +%FT%TZ), smoke_root=/tmp/cgreality-phase02-ready, verify_log=/tmp/phase-02-verify.log" >> /tmp/ccreality-phase-progress.log
```

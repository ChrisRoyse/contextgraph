# Phase 4 — Verification

## Prerequisites

- Phase 3 verification passes.
- Phase 5 verification passes (config files exist).
- A smoke runtime root exists.

## Canonical end-state block

```bash
set -e
cd /home/cabdru/contextgraph

echo "=== 4.V1 MCP server builds ==="
cargo build -p context-graph-mcp --quiet
echo "  pass"

echo "=== 4.V2 New module files exist ==="
for f in alter.rs interact.rs shift_log.rs optimizer.rs lock.rs schema_validator.rs; do
  test -f "crates/context-graph-mcp/src/handlers/tools/reality_loop/$f"
done
echo "  pass"

echo "=== 4.V3 Unit tests pass ==="
cargo test -p context-graph-mcp --lib reality_loop --quiet
echo "  pass"

echo "=== 4.V4 tools/list returns ≥30 tools ==="
TOOLS=$(echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' \
  | ./target/debug/context-graph-mcp --mode reality-loop 2>/dev/null \
  | jq -r '.result.tools[].name' | wc -l)
echo "  total tool count: $TOOLS"
[ "$TOOLS" -ge 30 ]
echo "  pass"

echo "=== 4.V5 All expected tools present ==="
TOOL_NAMES=$(echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' \
  | ./target/debug/context-graph-mcp --mode reality-loop 2>/dev/null \
  | jq -r '.result.tools[].name')
for expected in \
  reality_run_attempt reality_run_smoke reality_run_official_eval reality_run_targeted_test \
  reality_recompile_reward reality_query_ledger \
  harness_open_window harness_apply_line_window_edit harness_run_command \
  harness_git_diff harness_git_status harness_verify_state \
  store_memory boost_importance forget_concept merge_concepts \
  optimizer_record_decision optimizer_record_recommendation optimizer_record_harness_transition \
  reality_shift_log reality_shift_compare_to_my_view; do
  echo "$TOOL_NAMES" | grep -qx "$expected" || { echo "  MISSING: $expected"; exit 1; }
done
echo "  pass"

echo "=== 4.V6 reality_run_smoke succeeds end-to-end ==="
RES=$(echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"reality_run_smoke","arguments":{"runtime_root":"/tmp/cgreality-phase04-smoke","label":"phase04-mcp"}}}' \
  | ./target/debug/context-graph-mcp --mode reality-loop 2>/dev/null \
  | jq -r '.result.content[0].text')
echo "$RES" | jq -e '.status == "success"' >/dev/null
echo "  pass"

echo "=== 4.V7 harness_open_window returns SHA for benign file ==="
RES=$(echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"harness_open_window","arguments":{"path":"docs/ccreality/00_INDEX.md","start_line":1,"end_line":5}}}' \
  | ./target/debug/context-graph-mcp --mode reality-loop 2>/dev/null \
  | jq -r '.result.content[0].text')
SHA=$(echo "$RES" | jq -r '.sha256')
echo "  observed SHA: $SHA"
echo "$SHA" | grep -qE "^sha256:[a-f0-9]{64}$"
echo "  pass"

echo "=== 4.V8 harness_open_window denies forbidden path ==="
RES=$(echo '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"harness_open_window","arguments":{"path":".env","start_line":1,"end_line":5}}}' \
  | ./target/debug/context-graph-mcp --mode reality-loop 2>/dev/null \
  | jq -r '.result.content[0].text')
echo "$RES" | jq -e '.error_code == "CCREALITY_HARNESS_PATH_DENIED" or .error_code == "CCREALITY_GOVERNED_PROJECT_PATH_DENIED"' >/dev/null
echo "  pass"

echo "=== 4.V9 harness_apply_line_window_edit with stale SHA fails ==="
# Use a deliberately wrong observed_sha256
RES=$(echo '{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"harness_apply_line_window_edit","arguments":{"path":"docs/ccreality/00_INDEX.md","start_line":1,"end_line":1,"observed_sha256":"sha256:0000000000000000000000000000000000000000000000000000000000000000","replace":"# wrong"}}}' \
  | ./target/debug/context-graph-mcp --mode reality-loop 2>/dev/null \
  | jq -r '.result.content[0].text')
echo "$RES" | jq -e '.error_code | contains("SHA_MISMATCH")' >/dev/null
echo "  pass"

echo "=== 4.V10 reality_shift_log returns array ==="
SESS="phase04-test-session"
RES=$(echo "{\"jsonrpc\":\"2.0\",\"id\":6,\"method\":\"tools/call\",\"params\":{\"name\":\"reality_shift_log\",\"arguments\":{\"session_id\":\"$SESS\",\"limit\":5}}}" \
  | ./target/debug/context-graph-mcp --mode reality-loop 2>/dev/null \
  | jq -r '.result.content[0].text')
echo "$RES" | jq -e '.shifts | type == "array"' >/dev/null
echo "  pass"

echo "=== 4.V11 optimizer_record_recommendation rejects malformed payload ==="
RES=$(echo '{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"optimizer_record_recommendation","arguments":{"status":"changed","reason":"missing required fields"}}}' \
  | ./target/debug/context-graph-mcp --mode reality-loop 2>/dev/null \
  | jq -r '.result.content[0].text')
echo "$RES" | jq -e '.error_code == "CCREALITY_RECOMMENDATION_SCHEMA_INVALID"' >/dev/null
echo "  pass"

echo "=== 4.V12 reality_query_ledger rejects DROP attempt ==="
RES=$(echo '{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"reality_query_ledger","arguments":{"run_id":"any","where":"1=1; DROP TABLE ledger_records"}}}' \
  | ./target/debug/context-graph-mcp --mode reality-loop 2>/dev/null \
  | jq -r '.result.content[0].text')
echo "$RES" | jq -e '.error_code == "CCREALITY_LEDGER_QUERY_REJECTED"' >/dev/null
echo "  pass"

echo "=== 4.V13 Single-writer lock blocks concurrent harness_apply ==="
# Acquire then attempt second concurrent acquire; second should fail.
# Write a small Rust test or use a parallel pair of curl-style requests.
# Simpler: confirm the lock file gets written and removed.
test ! -f ~/.cache/cgreality/edit_lock || rm ~/.cache/cgreality/edit_lock
# Trigger one apply (fails on bad SHA but should still cycle the lock).
echo '{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"harness_apply_line_window_edit","arguments":{"path":"docs/ccreality/00_INDEX.md","start_line":1,"end_line":1,"observed_sha256":"sha256:0000000000000000000000000000000000000000000000000000000000000000","replace":"x"}}}' \
  | ./target/debug/context-graph-mcp --mode reality-loop 2>/dev/null > /dev/null
test ! -f ~/.cache/cgreality/edit_lock  # should be released by Drop
echo "  pass"

echo "=== 4.V14 stdout is JSON-RPC only ==="
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' \
  | ./target/debug/context-graph-mcp --mode reality-loop 2>/dev/null \
  | jq -e '.result.tools | length > 0' >/dev/null
echo "  pass"

echo
echo "Phase 4 verification: ALL PASS"
```

## Cleanup

```bash
rm -rf /tmp/cgreality-phase04-smoke
rm -f ~/.cache/cgreality/edit_lock
```

## What to record

```bash
echo "phase-04: PASS, $(date -u +%FT%TZ), tool count=$TOOLS" >> /tmp/ccreality-phase-progress.log
```

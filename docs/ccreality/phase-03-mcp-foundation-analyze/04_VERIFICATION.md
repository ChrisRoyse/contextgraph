# Phase 3 — Verification

## Prerequisites for verification

A smoke runtime root from Phase 2 must exist. Re-run if not:
```bash
./target/debug/reality-loop smoke --runtime-root /tmp/cgreality-phase03-smoke --label phase03-verify
```

## Canonical end-state block

```bash
set -euo pipefail
cd /home/cabdru/contextgraph
export CGREALITY_GOVERNED_PATHS=/home/cabdru/contextgraph/config/ccreality_governed_paths.json
export CGREALITY_RECOMMENDATION_SCHEMA=/home/cabdru/contextgraph/config/ccreality_recommendation_schema.json

echo "=== 3.V1 MCP server builds ==="
cargo build -p context-graph-mcp --quiet
echo "  pass"

echo "=== 3.V2 reality_loop module files exist ==="
test -f crates/context-graph-mcp/src/handlers/tools/reality_loop/mod.rs
test -f crates/context-graph-mcp/src/handlers/tools/reality_loop/analyze.rs
test -f crates/context-graph-mcp/src/handlers/tools/reality_loop/path_policy.rs
test -f crates/context-graph-mcp/src/handlers/tools/reality_loop/errors.rs
test -f crates/context-graph-mcp/src/handlers/tools/reality_loop/helpers.rs
echo "  pass"

echo "=== 3.V3 --mode flag is recognized ==="
./target/debug/context-graph-mcp --help 2>&1 | grep -q -- "--mode"
echo "  pass"

echo "=== 3.V4 path_policy unit test passes ==="
cargo test -p context-graph-mcp --lib reality_loop::path_policy --quiet
echo "  pass"

echo "=== 3.V5 tools/list returns 11 reality_* tool names ==="
TOOLS_LIST=$(echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' \
  | timeout 10s ./target/debug/context-graph-mcp --mode reality-loop 2>/tmp/phase03-tools-list.err)
COUNT=$(echo "$TOOLS_LIST" | jq -r '.result.tools[].name' | grep -cE '^reality_(latest_root|attempt_summary|official_report|problem_packet|signal|failure|trigger_decision|harness_transitions|compare_attempts|audit_trail|replay_artifact)$')
echo "  reality_* tool count: $COUNT"
[ "$COUNT" -ge 11 ]
echo "  pass"

echo "=== 3.V6 reality_latest_root returns valid payload ==="
RES=$(echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"reality_latest_root","arguments":{}}}' \
  | timeout 10s ./target/debug/context-graph-mcp --mode reality-loop 2>/tmp/phase03-latest-root.err)
PAYLOAD=$(echo "$RES" | jq -r '.result.content[0].text')
echo "  payload: $(echo "$PAYLOAD" | head -c 200)"
echo "$PAYLOAD" | jq -e '.runtime_root' >/dev/null
echo "$PAYLOAD" | jq -e '.success_predicate' >/dev/null
echo "  pass"

echo "=== 3.V7 reality_attempt_summary returns the smoke attempt summary ==="
RUN_ID=$(echo "$PAYLOAD" | jq -r '.active_run_id')
TASK_ID=$(echo "$PAYLOAD" | jq -r '.active_task_id')
[ "$RUN_ID" != "null" ] && [ "$TASK_ID" != "null" ] || { echo "  FAIL: smoke not active"; exit 1; }
RES=$(echo "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{\"name\":\"reality_attempt_summary\",\"arguments\":{\"run_id\":\"$RUN_ID\",\"attempt\":1}}}" \
  | timeout 10s ./target/debug/context-graph-mcp --mode reality-loop 2>/tmp/phase03-attempt-summary.err)
SUMMARY=$(echo "$RES" | jq -r '.result.content[0].text')
echo "$SUMMARY" | jq -e '.source_of_truth' >/dev/null
echo "$SUMMARY" | jq -e '.attempt' >/dev/null
echo "  pass"

echo "=== 3.V8 reality_problem_packet (compact) returns shape ==="
RES=$(echo "{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"tools/call\",\"params\":{\"name\":\"reality_problem_packet\",\"arguments\":{\"run_id\":\"$RUN_ID\",\"compact\":true}}}" \
  | timeout 10s ./target/debug/context-graph-mcp --mode reality-loop 2>/tmp/phase03-problem-packet.err)
echo "$RES" | jq -r '.result.content[0].text' | jq -e '.source_of_truth' >/dev/null
echo "  pass"

echo "=== 3.V9 reality_replay_artifact returns SHA + content for small file ==="
SUMMARY_PATH="$(cat ~/.cache/cgreality/active_runtime_root)/$RUN_ID/$TASK_ID/smoke-attempt-1/attempt-summary.json"
RES=$(echo "{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"tools/call\",\"params\":{\"name\":\"reality_replay_artifact\",\"arguments\":{\"path\":\"$SUMMARY_PATH\"}}}" \
  | timeout 10s ./target/debug/context-graph-mcp --mode reality-loop 2>/tmp/phase03-replay-artifact.err)
REPLAY=$(echo "$RES" | jq -r '.result.content[0].text')
echo "$REPLAY" | jq -e '.exists == true' >/dev/null
echo "$REPLAY" | jq -e '.sha256 | startswith("sha256:")' >/dev/null
echo "  pass"

echo "=== 3.V10 stdout is JSON-RPC only (no log corruption) ==="
# Capture only stdout while sending a request; if the server logs to stdout, JSON parsing will break.
RES=$(echo '{"jsonrpc":"2.0","id":6,"method":"tools/list","params":{}}' \
  | timeout 10s ./target/debug/context-graph-mcp --mode reality-loop 2>/tmp/phase03-json-only.err \
  | jq -e '.result.tools | length > 0' 2>&1)
[ "$RES" = "true" ]
echo "  pass"

echo
echo "Phase 3 verification: ALL PASS"
```

## Per-clause meaning

| Clause | What it asserts |
|--------|----------------|
| 3.V1 | Build succeeds |
| 3.V2 | All five module files exist |
| 3.V3 | --mode flag is recognized |
| 3.V4 | Path policy passes its unit test |
| 3.V5 | All 11 reality_* tool names appear in tools/list |
| 3.V6 | reality_latest_root returns a parseable payload with required fields |
| 3.V7 | reality_attempt_summary actually reads from disk and returns the smoke attempt |
| 3.V8 | reality_problem_packet (compact) returns valid JSON |
| 3.V9 | reality_replay_artifact correctly SHAs and inlines a small file |
| 3.V10 | Stdout has nothing but valid JSON-RPC; logging didn't leak |

## Multi-request verification helper

Some MCP servers expect persistent stdio sessions. If single-request invocations don't work, use a small helper:

```bash
cat > /tmp/mcp-multi-req.sh <<'EOF'
#!/usr/bin/env bash
set -e
{
  echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{}}}'
  sleep 0.2
  echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
} | "$1" --mode reality-loop 2>/dev/null
EOF
chmod +x /tmp/mcp-multi-req.sh
/tmp/mcp-multi-req.sh ./target/debug/context-graph-mcp | tail -1 | jq '.result.tools | length'
```

Adjust per the server's actual JSON-RPC handshake requirements (look at how Claude Code calls it).

## Cleanup

```bash
rm -rf /tmp/cgreality-phase03-smoke /tmp/mcp-multi-req.sh
```

## What to record

```bash
echo "phase-03: PASS, $(date -u +%FT%TZ), reality_* tool count=$COUNT" >> /tmp/ccreality-phase-progress.log
```

# Demo Runbook

This is the operator demo for the ccreality stack. It uses real binaries, real artifacts, real SQLite ledgers, and actual hook/MCP behavior.

## Demo 1 - Show The Hardened MCP Surface

```bash
cd /home/cabdru/contextgraph
printf '%s\n' '{"jsonrpc":"2.0","id":"tools","method":"tools/list","params":{}}' \
  | ./target/release/context-graph-mcp --mode reality-loop \
  | jq -c '[.result.tools[].name] as $n | {tool_count:($n|length), has_search_graph:($n|index("search_graph")!=null), has_reality_attempt_summary:($n|index("reality_attempt_summary")!=null)}'
```

Expected:

```json
{"tool_count":28,"has_search_graph":false,"has_reality_attempt_summary":true}
```

## Demo 2 - Run The Full Smoke

```bash
cd /home/cabdru/contextgraph
CGREALITY_SMOKE_RUN_STAMP=$(date +%s) bash scripts/cc/tests/smoke.sh
```

Fresh passing reference:

```text
smoke_root=/tmp/cgreality-smoke-1778022643
runtime_root=/tmp/cgreality-smoke-1778022643/layer5-skill-runtime
recommendation=/tmp/cgreality-smoke-1778022643/layer5-skill-runtime/smoke-1778022681/cgreality-smoke-phase09-layer5/smoke-attempt-1/claude-code-optimizer/recommendation-turn-01.json
ledger=/tmp/cgreality-smoke-1778022643/layer5-skill-runtime/ledger.sqlite
```

Physical readback:

```bash
sqlite3 /tmp/cgreality-smoke-1778022643/layer5-skill-runtime/ledger.sqlite \
  'select record_kind, count(*) from ledger_records group by record_kind order by record_kind;'
jq '{status, reason, created_at_unix, source_of_truth}' \
  /tmp/cgreality-smoke-1778022643/layer5-skill-runtime/smoke-1778022681/cgreality-smoke-phase09-layer5/smoke-attempt-1/claude-code-optimizer/recommendation-turn-01.json
```

Expected ledger readback from the fresh run:

```text
agent_run|1
tool_call|3
```

## Demo 3 - Run Full State Verification

```bash
cd /home/cabdru/contextgraph
FSV_RUN_ROOT=/tmp/cgreality-fullstack-fsv-$(date +%s) bash scripts/cc/tests/fsv.sh
```

Fresh passing reference:

```text
Run root: /tmp/cgreality-fullstack-fsv-1778022699
Total: 39, PASS: 39, FAIL: 0
```

Inspect edge-case proof:

```bash
cat /tmp/cgreality-fullstack-fsv-1778022699/cases/C2_stale_sha_fails/state.log
cat /tmp/cgreality-fullstack-fsv-1778022699/cases/C3_denied_path_fails/state.log
cat /tmp/cgreality-fullstack-fsv-1778022699/cases/F3_large_artifact_pointer/source-of-truth.log
```

## Demo 4 - Show Real Production Attempt Evidence

```bash
cd /home/cabdru/contextgraph
jq '{status:.attempt.status,error_code:.attempt.error_code,message:.attempt.message,ledger_counts}' \
  /tmp/ccreality-prod-1/rust_reality_1778020383548106093_e525987d/tasks/psf__requests-2317/gemma4-e4b/attempts/attempt-000001/attempt-summary.json

jq '{turn_number,status,reason,source_of_truth_readbacks}' \
  /tmp/ccreality-prod-1/rust_reality_1778020383548106093_e525987d/tasks/psf__requests-2317/gemma4-e4b/attempts/attempt-000001/claude-code-optimizer/recommendation-turn-01.json
```

Expected: the attempt failed closed because the inner model produced invalid edits; the recommendation explains why no harness edit was warranted.

## Demo 5 - Show Hook Context Is Bounded

```bash
printf '%s\n' '{"session_id":"demo","source":"startup","model":"opus"}' \
  | scripts/cc/hook_session_start.sh \
  | jq '.hookSpecificOutput.additionalContext | {chars:length, has_compact:contains("contextgraph_attempt_summary_hook_compact")}'
```

Expected:

```json
{"chars":1929,"has_compact":true}
```


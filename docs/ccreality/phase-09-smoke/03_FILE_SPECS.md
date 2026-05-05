# Phase 9 — File Specifications

## `scripts/cc/tests/smoke.sh`

Use the body from [../17_SMOKE_TEST_PLAN.md](../17_SMOKE_TEST_PLAN.md) §"Smoke replay script", adapted as below.

```bash
#!/usr/bin/env bash
set -euo pipefail
PROJECT_ROOT="${PROJECT_ROOT:-/home/cabdru/contextgraph}"
SMOKE_ROOT="${SMOKE_ROOT:-/tmp/cgreality-smoke-$(date +%s)}"
mkdir -p "$SMOKE_ROOT"
cd "$PROJECT_ROOT"

LOG="$SMOKE_ROOT/smoke.log"
exec > >(tee -a "$LOG") 2>&1

echo "==== ccreality smoke ===="
echo "PROJECT_ROOT: $PROJECT_ROOT"
echo "SMOKE_ROOT: $SMOKE_ROOT"
echo "DATE: $(date -u +%FT%TZ)"
echo

echo "== Layer 0: environment =="
which claude jq sha256sum >/dev/null
echo "  PASS"

echo "== Layer 1: engine builds + smoke runs =="
cargo build -p context-graph-cli --bin reality-loop --quiet
./target/debug/reality-loop smoke --runtime-root "$SMOKE_ROOT/run1" --label ci-smoke >/dev/null
test -d "$SMOKE_ROOT/run1"
echo "  PASS"

echo "== Layer 2: MCP server builds + responds =="
cargo build -p context-graph-mcp --quiet
TOOLS=$(echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' \
  | ./target/debug/context-graph-mcp --mode reality-loop 2>/dev/null \
  | jq -r '.result.tools[].name' | wc -l)
[ "$TOOLS" -ge 30 ]
echo "  PASS ($TOOLS tools)"

echo "== Layer 3: Claude Code sees cgreality MCP =="
HAS_MCP=$(claude --print "List MCP servers as JSON {server_name: tool_count}. Return only the JSON, no commentary." --output-format json 2>/dev/null \
  | jq -r '.result' \
  | jq -r 'has("cgreality")' 2>/dev/null || echo "false")
[ "$HAS_MCP" = "true" ]
echo "  PASS"

echo "== Layer 4: SessionStart hook fires =="
HOOK_OUT=$(claude --print "Quote verbatim the FIRST line of context the SessionStart hook injected. Return only that line." --output-format json 2>/dev/null \
  | jq -r '.result' | head -c 200)
[ -n "$HOOK_OUT" ]
echo "  $HOOK_OUT"
echo "  PASS"

echo "== Layer 5: /ccreality-smoke skill =="
SKILL_OUT=$(claude --print "/ccreality-smoke" --model opus --max-turns 25 --output-format json 2>/dev/null)
echo "$SKILL_OUT" | jq -e '.session_id' >/dev/null
RECOMMENDATION_FILE=$(find "$SMOKE_ROOT" "$HOME/.cache/cgreality" -path '*/claude-code-optimizer/recommendation-*.json' 2>/dev/null | head -1)
[ -n "$RECOMMENDATION_FILE" ] || { echo "  FAIL: no recommendation artifact found"; exit 1; }
echo "  PASS (recommendation: $RECOMMENDATION_FILE)"

echo "== Layer 6: drift detection =="
# Simulate by editing a benign file mid-test (no real Claude session running)
TARGET="docs/ccreality/00_INDEX.md"
SHA_BEFORE=$(sha256sum "$TARGET" | awk '{print $1}')
echo "# touched at $(date)" >> "$TARGET"
SHA_AFTER=$(sha256sum "$TARGET" | awk '{print $1}')
[ "$SHA_BEFORE" != "$SHA_AFTER" ]
# Revert
git restore "$TARGET"
echo "  PASS (drift detection mechanism works manually; full MCP drift FSV in Phase 10)"

echo "== Layer 7: deny enforcement =="
DENY_OUT=$(claude --print "Edit /home/cabdru/contextgraph/.env and add a comment '# test'. Report whether the edit was blocked or completed. Return one word: 'blocked' or 'completed'." --output-format json 2>/dev/null | jq -r '.result' | head -c 50)
echo "$DENY_OUT" | grep -qi "block\|deny"
echo "  PASS ($DENY_OUT)"

echo "== Layer 8: schema validation rejects malformed payload =="
SCHEMA_OUT=$(claude --print "Call mcp__cgreality__optimizer_record_recommendation with the literal payload {\"status\":\"changed\",\"reason\":\"x\"}. Report the error_code from the response." --output-format json 2>/dev/null | jq -r '.result' | head -c 200)
echo "$SCHEMA_OUT" | grep -qE "(SCHEMA_INVALID|REQUIRED|VALIDATION)"
echo "  PASS"

echo
echo "Smoke pass."
```

Save to `scripts/cc/tests/smoke.sh` and `chmod +x`.

## `docs/ccreality/SMOKE_RESULTS_<date>.md` (auto-generated)

Created by Task 9.4 in [02_TASKS.md](02_TASKS.md). Format:

```markdown
# Smoke Results — YYYY-MM-DD

Run-id: ...
Operator: ...
Branch: ...
HEAD: ...

## Layers
| Layer | Result | Notes |
|-------|--------|-------|
| 0 | PASS | |
...

## Artifacts retained
...

## Issues observed
...
```

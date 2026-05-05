# Smoke Test Plan — first-light verification

Before letting the loop touch real SWE-bench attempts, smoke must pass end-to-end. The smoke is designed to verify every layer **without** burning Docker time or Gemma inference.

## Smoke layers (verify in order)

### Layer 0 — environment
```bash
which claude && claude --version
which jq && jq --version
which sha256sum
ls "$HOME/.cache/cgreality/" 2>/dev/null || mkdir -p "$HOME/.cache/cgreality/"
```

### Layer 1 — engine builds and runs `smoke` subcommand
```bash
cd /home/cabdru/contextgraph
cargo build -p context-graph-cli --bin reality-loop
./target/debug/context-graph-cli reality-loop smoke --runtime-root /tmp/cgreality-smoke --label first-light
```

Expected: exit 0, `/tmp/cgreality-smoke/run-…/smoke-attempt-1/` written with `attempt-summary.json`, `reality-signal-packet.json`, `problem-reality-packet.json`. The active runtime root file `~/.cache/cgreality/active_runtime_root` is updated.

### Layer 2 — MCP server starts and responds
```bash
cargo build -p context-graph-mcp
./target/debug/context-graph-mcp --mode reality-loop &
MCP_PID=$!
# Send a simple JSON-RPC initialize over stdio (use an MCP client harness)
# Expected: server replies with capabilities; tools listed include mcp__cgreality__reality_latest_root
kill $MCP_PID
```

### Layer 3 — Claude Code sees the MCP server
```bash
claude --version
# Confirm .claude/settings.json + .mcp.json are present
ls -la /home/cabdru/contextgraph/.claude/settings.json /home/cabdru/contextgraph/.mcp.json
# Headless one-shot to list MCP tools
claude -p "List the tools available under the cgreality MCP server. Return only a comma-separated list of tool names." --output-format json | jq -r '.result'
```

Expected: result string includes `mcp__cgreality__reality_latest_root`, `mcp__cgreality__harness_open_window`, `mcp__cgreality__optimizer_record_recommendation`.

### Layer 4 — hooks fire and inject context
```bash
claude -p "Tell me what reality the SessionStart hook injected verbatim, with no commentary." --output-format json | jq -r '.result'
```

Expected: result starts with "Active reality-loop runtime root: /tmp/cgreality-smoke/…" or "ContextGraph reality loop is not currently running" depending on whether layer 1 ran. Either way, the SessionStart hook stdout/`additionalContext` must be visible in the response.

### Layer 5 — `/ccreality-smoke` skill drives the full loop
```bash
claude -p "/ccreality-smoke" --model opus --max-turns 25
```

Expected:
- A trigger-decision artifact is written via `mcp__cgreality__optimizer_record_decision`.
- `mcp__cgreality__reality_run_smoke` is called and returns success.
- `mcp__cgreality__harness_open_window` returns SHA for `docs/ccreality/00_INDEX.md`.
- `mcp__cgreality__harness_run_command` returns `cargo --version` exit 0.
- `mcp__cgreality__harness_verify_state` returns all PASS.
- `mcp__cgreality__optimizer_record_recommendation` is called with status=`no_change`.
- The Stop hook allows the session to end.
- A recommendation artifact exists at `<runtime_root>/cgreality-smoke-attempt/claude-code-optimizer/recommendation-turn-01.json`.
- The shift log at `<runtime_root>/cgreality-shift-log/<session>.jsonl` has at least one entry (the recommendation itself counts as a shift).

### Layer 6 — drift-detection smoke
```bash
# Open a known file via the MCP harness_open_window in a Claude Code session.
# Then while that session reasons, mutate the file from outside:
echo "# touched at $(date)" >> /home/cabdru/contextgraph/docs/ccreality/00_INDEX.md
# In the Claude Code session, call mcp__cgreality__reality_shift_compare_to_my_view
# Expected: drift = "external_drift_detected"
```

This proves the shift principle's drift detection is wired correctly. Revert the test edit afterward.

### Layer 7 — reality-boundary deny enforcement
```bash
claude -p "Edit /home/cabdru/contextgraph/.env and add a comment '# test'. After the edit, report what happened." --output-format json
```

Expected: the edit is denied. Both layers (settings.json deny + PreToolUse hook policy check) should fire. Result includes the deny reason; .env is unmodified.

### Layer 8 — recommendation schema validation
```bash
# Force a malformed recommendation and verify it is rejected.
claude -p "Call mcp__cgreality__optimizer_record_recommendation with this payload: {\"status\":\"changed\",\"reason\":\"broken test\"}. Report the error verbatim." --output-format json
```

Expected: tool returns `CCREALITY_RECOMMENDATION_SCHEMA_INVALID` because required fields (run_id, attempt, source_files_changed, etc.) are missing. The Stop hook still blocks because no valid recommendation was recorded.

## Smoke pass criteria

All eight layers must pass on a single run before the loop is declared "first light." The smoke artifacts are themselves recorded in BlackBox ledger (`record_kind = "ccreality_smoke_run"`) for replay.

## Smoke artifacts to retain

```
/tmp/cgreality-smoke/
  run-<id>/
    smoke-attempt-1/
      attempt-summary.json
      problem-reality-packet.json
      reality-signal-packet.json
      claude-code-optimizer/
        decision-turn-01.json
        recommendation-turn-01.json
        commands/<unix>-cargo-version.json
    cgreality-shift-log/<session>.jsonl
    cgreality-hook-events.jsonl
    cgreality-mcp-events.jsonl
```

If layer 5 fails, inspect `cgreality-mcp-events.jsonl` for the failing tool call, and the cargo build output for any `reality-loop` or `context-graph-mcp` compilation errors.

## Smoke replay script

`scripts/cc/tests/smoke.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail
SMOKE_ROOT="${SMOKE_ROOT:-/tmp/cgreality-smoke-$(date +%s)}"
mkdir -p "$SMOKE_ROOT"
cd /home/cabdru/contextgraph

echo "== Layer 0 =="
which claude jq sha256sum

echo "== Layer 1 =="
cargo build -p context-graph-cli --bin reality-loop
./target/debug/context-graph-cli reality-loop smoke --runtime-root "$SMOKE_ROOT" --label ci-smoke

echo "== Layer 2 =="
cargo build -p context-graph-mcp

echo "== Layer 3, 4, 5 =="
claude -p "/ccreality-smoke" --model opus --max-turns 25 --output-format json > "$SMOKE_ROOT/claude-out.json"
test -f "$(jq -r '.session_id' "$SMOKE_ROOT/claude-out.json")"

echo "== Layer 6 (manual; skipped in CI) =="

echo "== Layer 7 =="
DENY_OUT="$(claude -p "Edit /home/cabdru/contextgraph/.env and add a comment '# test'. Report whether the edit was allowed." --output-format json | jq -r '.result')"
echo "$DENY_OUT" | grep -qi "deny\|denied\|blocked"

echo "== Layer 8 =="
SCHEMA_OUT="$(claude -p "Call mcp__cgreality__optimizer_record_recommendation with this minimal payload: {\"status\":\"changed\",\"reason\":\"x\"}. Report the error verbatim." --output-format json | jq -r '.result')"
echo "$SCHEMA_OUT" | grep -q CCREALITY_RECOMMENDATION_SCHEMA_INVALID

echo "Smoke pass."
```

Run before any production attempt:
```bash
bash scripts/cc/tests/smoke.sh
```

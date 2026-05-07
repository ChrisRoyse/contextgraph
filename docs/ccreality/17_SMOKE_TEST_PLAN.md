# Smoke Test Plan — first-light verification

Before letting the loop touch real SWE-bench attempts, smoke must pass end-to-end. The smoke is designed to verify every layer **without** burning Docker time or the inner actor inference.

## Current State

As of 2026-05-05, Phases 0–9 are complete and manually FSV'd. The deterministic Phase 9 harness is `scripts/cc/tests/smoke.sh`. Latest pass: `/tmp/cgreality-smoke-1778013189/smoke-report.json` and `docs/ccreality/SMOKE_RESULTS_2026-05-05.md`.

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
./target/debug/reality-loop smoke --runtime-root /tmp/cgreality-smoke --label first-light
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
- `mcp__cgreality__harness_run_command` returns `git status --short` exit 0; this command is in the governed allowlist.
- `mcp__cgreality__harness_verify_state` returns all PASS.
- `mcp__cgreality__optimizer_record_recommendation` is called with status=`no_change`.
- The Stop hook allows the session to end.
- A recommendation artifact exists under `<runtime_root>/smoke-*/cgreality-smoke-*/smoke-attempt-1/claude-code-optimizer/recommendation-turn-01.json`.
- The shift log at `<runtime_root>/cgreality-shift-log/<session>.jsonl` has at least one entry (the recommendation itself counts as a shift).

### Layer 6 — drift-detection smoke

Use `scripts/cc/tests/hooks/fixtures/post_tool_use_target.txt` as the tracked probe. Open it with `harness_open_window`, apply a governed edit with `harness_apply_line_window_edit` using the observed SHA, mutate the file externally, then call `reality_shift_compare_to_my_view`.

Expected: `drift = "external_drift_detected"` and the fixture is restored byte-for-byte before the script exits.

### Layer 7 — reality-boundary deny enforcement
Expected: `.claude/settings.json` denies `.env`, the PreToolUse hook returns `permissionDecision: "deny"` under `hookSpecificOutput`, the hook event log grows, and the `.env` SHA is unchanged. The smoke harness verifies this by invoking the hook script directly with real stdin; it does not attempt to modify `.env`.

### Layer 8 — recommendation schema validation
Expected: a malformed `optimizer_record_recommendation` call returns `status: "error"` and `error_code: "CCREALITY_RECOMMENDATION_SCHEMA_INVALID"`. The harness reads ledger row count, recommendation file count, and shift-log line count before and after to prove the invalid write did not mutate durable state.

## Smoke pass criteria

All eight layers must pass on a single run before the loop is declared "first light." The smoke artifacts include a real BlackBox ledger with one `agent_run` row and two `tool_call` rows for the optimizer decision/recommendation writes.

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
        commands/project-command-git-status---short.json
    cgreality-shift-log/<session>.jsonl
    cgreality-hook-events.jsonl
    cgreality-mcp-events.jsonl
```

If layer 5 fails, inspect `/tmp/cgreality-smoke-<stamp>/mcp-*.raw.json`, the attempt `claude-code-optimizer/` artifacts, and the BlackBox `ledger_records` rows.

## Smoke replay script

Run before any production attempt:
```bash
bash scripts/cc/tests/smoke.sh
```

The script auto-generates `docs/ccreality/SMOKE_RESULTS_<date>.md` and `/tmp/cgreality-smoke-<stamp>/smoke-report.json`.

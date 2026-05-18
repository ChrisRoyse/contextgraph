---
namespace: discoveries
agent_id: agent-41-coordinator
timestamp: 2026-05-07T15:55:42Z
status: active
tags: discovery, smoke, blackbox-ledger, gap, recommendation-record
---

# Discovery: `mcp__cgreality__reality_run_smoke` does not initialize a BlackBox ledger

## What happened

When recording turn-41's `no_change` recommendation against the smoke run_id `smoke-1778169126`, `mcp__cgreality__optimizer_record_recommendation` returned:

```json
{
  "details": {"runtime_root": "/tmp/cgreality-mcp-smoke"},
  "error_code": "CCREALITY_OPTIMIZER_LEDGER_MISSING",
  "field_path": "optimizer.ledger.path",
  "message": "no ledger.sqlite or blackbox.sqlite was found under active runtime root",
  "remediation": "initialize a real BlackBox ledger before recording optimizer artifacts",
  "source_of_truth": "file:/tmp/cgreality-mcp-smoke",
  "status": "error"
}
```

But despite the error response, the recommendation JSON file WAS written to disk at `/tmp/cgreality-mcp-smoke/smoke-1778169126/cgreality-smoke-turn-41-recovery-smoke/smoke-attempt-1/claude-code-optimizer/recommendation-turn-41.json` (server-stamped `created_at_unix` and `source_of_truth` fields populated). So the recommendation file IS the source of truth; only the BlackBox ledger row failed.

## Why this happened

`mcp__cgreality__reality_run_smoke` mints a fresh runtime root (`/tmp/cgreality-mcp-smoke/`) with attempt artifacts (problem-reality-packet, reality-signal-packet, attempt-summary), but does NOT initialize a `blackbox.sqlite` or `ledger.sqlite` there. `.claude/skills/ccreality-smoke/SKILL.md` step 6 explicitly notes this: *"Before recording, ensure the runtime root has a real BlackBox ledger initialized and an `agent_run` row for the smoke task. The BlackBox CLI mints its own ledger `run_id`..."* — but the user-facing CLI `target/release/context-graph-cli ccreality` only exposes `record-harness-transition`, not a `blackbox-init` or `blackbox-start-run` command.

The functions exist internally — `crates/context-graph-cli/src/commands/blackbox.rs:17830 fn mcp_start_run(...)` and a tool-name `blackbox_start_run` are wired in the BlackBox MCP server. But the BlackBox MCP tool surface is not part of the `mcp__cgreality__*` MCP server we use for ccreality. They appear to be a separate MCP family that's not currently loaded as deferred tools in this session.

## Severity

LOW for the immediate turn (recommendation file is on disk; Stop hook satisfied; SMAP report written), but MEDIUM as a gap in the smoke pipeline because:

- Future smoke-based recovery turns hit the same wall.
- The skill doc step 6 warns about this but provides no operative path to resolve it.
- The deviation between spec ("recommendation write must fail") and actual behavior (file written, error returned) is itself worth documenting — the server logs the error but doesn't cleanly roll back.

## Recommended fixes (multi-turn)

1. **Expose a `blackbox-init` subcommand** on `target/release/context-graph-cli ccreality`, wrapping the internal `mcp_start_run` function or equivalent. Then `reality_run_smoke` can shell out to it before completing.
2. **OR** `mcp__cgreality__reality_run_smoke` should initialize the ledger inline as part of the smoke setup. Same code path, different invocation point.
3. **OR** make `optimizer_record_recommendation` more graceful when the ledger is missing: log a warning, still write the file, return success with `ledger_status: missing` in the response (so the caller knows the recommendation is on disk but un-indexed).

Option 3 is the smallest change and matches the actual observed behavior (file written despite error). Worth pursuing.

## What this means for THIS turn

- The recommendation IS recorded for reality-shift purposes (file on disk, server-stamped).
- The BlackBox ledger has no row for this turn — anyone querying via `mcp__cgreality__reality_query_ledger` for `run_id: smoke-1778169126` will see nothing.
- The Stop hook is satisfied (the tool was called).
- Next turn, when dripping a real attempt, the engine will mint a real ledger.

---
namespace: discoveries
created: 2026-05-11T02:09:55Z
updated: 2026-05-11T02:09:55Z
status: active
severity: high
tags: ccreality, hooks, cli, mcp, stale-binary, fsv
---

# CcReality Stale Binary Selection

## What Happened

The normal `scripts/cc/tests/hooks/test_post_tool_use_edit.sh` path failed after the hook began passing `--preedit-state-path` and `--after-source-path`. The helper built a fresh debug CLI, but `scripts/cc/_common.sh` preferred an older release `context-graph-cli`, so the hook executed a binary whose Clap schema did not know the new flags.

After forcing `CGREALITY_CONTEXT_GRAPH_CLI=target/debug/context-graph-cli` and `CGREALITY_MCP_BIN=target/debug/context-graph-mcp`, the same hook test passed. That proved the root cause was stale binary resolution, not the new hook payload.

The fix now chooses the newest local CLI executable when no explicit override is set. The CLI also handles HTTP-style `.mcp.json` configs by selecting the newest local MCP binary while preserving fail-closed behavior for explicit command configs.

## Why It Matters

Hook/CLI/MCP schema changes are safety-critical because they produce durable reality shifts. If the hook silently executes a stale binary, tests can fail for the wrong reason and real sessions can stop recording transitions. Future hook hardening should always include an end-to-end shell smoke test without environment overrides.

## Related

- See also: ./memory/reports/session-2026-05-11--phase7-producer-source-backed-hardening.md
- Tags: ccreality, binary-resolution, hooks

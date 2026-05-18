---
namespace: discoveries
agent_id: codex
timestamp: 2026-05-09T02:04:08Z
created: 2026-05-09T02:04:08Z
updated: 2026-05-09T02:04:08Z
status: active
tags: ccreality,mcp,stale-binary,witness-chain,fail-closed
---

# CcReality Stale Debug MCP Binary

## What Happened

The ccreality hook CLI resolved its one-shot MCP subprocess to
`target/debug/context-graph-mcp` by default, while `.mcp.json` runs
`target/release/context-graph-mcp`. On this host the release MCP binary was
newer than the debug binary, so release-side witness-chain writes could contain
operation types that the stale debug subprocess did not understand.

## Why It Matters

This created recurring `CCREALITY_WITNESS_OP_TYPE_UNKNOWN` hook failures even
when the actual active MCP server was correct. Falling back to debug also
hid the real operational error: the release source-of-truth binary was missing
or not executable.

## Fix Direction

The CLI must use `.mcp.json` as the default command source, which currently
selects the release MCP binary. Local debug use must be explicit through
`CGREALITY_MCP_BIN` or an intentional `.mcp.json` change; there is no inferred
debug fallback. The resolver should log whether the configured path exists and
is executable so failures point directly at the physical cause.

## Related

- `crates/context-graph-cli/src/commands/ccreality/mod.rs`
- `.mcp.json`

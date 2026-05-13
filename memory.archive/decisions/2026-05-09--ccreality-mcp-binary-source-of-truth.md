---
namespace: decisions
agent_id: codex
timestamp: 2026-05-09T02:04:08Z
created: 2026-05-09T02:04:08Z
updated: 2026-05-09T02:04:08Z
status: active
tags: ccreality,mcp,binary-resolution,fail-closed,no-fallback
---

# Decision: CcReality MCP Binary Source Of Truth

## Decision

`.mcp.json` is the default source of truth for the ccreality CLI hook
subprocess command. In the current checkout it points at
`target/release/context-graph-mcp`. `target/debug/context-graph-mcp` must not
be used implicitly.

## Rationale

The hook CLI is a subprocess bridge into the MCP surface. If it runs an older
debug MCP while Claude Code runs the newer release MCP, the two processes can
disagree on witness-chain operation types and schemas. That is a version skew,
not a recoverable runtime condition.

## How To Override

Set `CGREALITY_MCP_BIN` to an executable binary path for deliberate local
debug runs, or update `.mcp.json` intentionally. If no override is set and the
configured `.mcp.json` command is missing or not executable, the CLI must fail
closed with structured path-state details.

## Related

- `memory/discoveries/2026-05-09--ccreality-stale-debug-mcp-binary.md`
- `crates/context-graph-cli/src/commands/ccreality/mod.rs`
- `.mcp.json`

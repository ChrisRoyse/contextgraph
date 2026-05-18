---
namespace: discoveries
agent_id: agent-203-code-simplifier
timestamp: 2026-05-09T02:00:00Z
status: active
tags: tier3, mcp-resolver, root-cause, refinement
---

# Handoff doc says "env > release > debug" but the actual code is "env > .mcp.json (no fallback)"

`crates/context-graph-cli/src/commands/ccreality/mod.rs:563-610` resolves
the MCP binary as:

  1. `CGREALITY_MCP_BIN` env override (line 564)
  2. `mcpServers.cgreality.command` parsed out of `.mcp.json` (lines 569-603)
  3. Hard error `CCREALITY_CLI_MCP_BINARY_MISSING` if that binary is not executable (lines 620-641)

There is **no debug-fallback** and **no release-default-without-config**.
The handoff brief at `memory/handoff/agent-202-coordinator-to-code-simplifier...md`
line 21 says the priority is "env override → target/release/... → target/debug/...".

Reading the code, the actual implementation is *more conservative* and
*more defensible* than the handoff describes: the canonical SoT for the
MCP binary becomes `.mcp.json` (a checked-in file Claude Code itself
reads), and the only escape hatch is the env var. There is no
profile-inference layer that could silently pick stale debug bytes.

The function comment at lines 552-562 documents this correctly ("There
is intentionally no inferred debug/release fallback") — so the code and
the inline doc are coherent; only the handoff doc is wrong. **Verdict:
the code is correct as-is**. Recommend the coordinator update the
handoff doc; no source change needed in `mod.rs`.

Where this DOES surface a real refinement (R3): the bash helper
`scripts/cc/_common.sh:106-125` (`cgreality_context_graph_cli_bin`)
*does* implement the env-then-release-then-debug ladder pattern for the
CLI binary. That helper is for the **CLI** binary, not the MCP binary,
so the inconsistency is not a bug — but the env-override shape *is*
inconsistent. The CLI helper accepts `CGREALITY_CONTEXT_GRAPH_CLI`. The
new MCP resolver accepts `CGREALITY_MCP_BIN`. Two different env-var
names for two different binaries is fine, but the bash helper has no
equivalent for the MCP binary because nothing in `scripts/cc/*` shells
out to MCP — only the engine does. So no change needed there either.

R3 (deferrable, naming-consistency only): `CGREALITY_MCP_BIN` vs.
`CGREALITY_CONTEXT_GRAPH_CLI` — the older one is verbose, the newer one
is short. Pick a convention and document in CLAUDE.md or in the
ccreality runbook so future bins follow the same pattern. This is not
worth a code change today; flagging for the next time someone adds a
third overridable binary.

Cite: `crates/context-graph-cli/src/commands/ccreality/mod.rs:552-610`,
`scripts/cc/_common.sh:106-125`,
`memory/handoff/agent-202-coordinator-to-code-simplifier--causal-mask-and-mcp-resolver-review.md:17-26`.

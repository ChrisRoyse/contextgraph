---
namespace: discoveries
agent_id: codex
timestamp: 2026-05-09T02:27:42Z
status: active
tags: tier1,tier2,fsv,mcp,fail-closed,source-of-truth
---

# Tier-1/2 FSV and MCP Binary Source Hardening

## What Happened

Review of the last two hours of history found two recurring hardening risks:

- FSV examples could write JSON evidence with `expected_code` and `actual_code`
  fields but not always fail the process when those fields diverged.
- The ccreality hook path could execute a stale or unintended
  `context-graph-mcp` binary. Before the release rebuild, the physical
  `.mcp.json` command at `target/release/context-graph-mcp` still ignored an
  unknown argument and continued toward RocksDB startup.

## Why It Matters

FSV evidence must be a source of truth, not a report that can quietly encode a
failed expectation. Every claim in an FSV JSON should be asserted before write,
then read back from disk and compared to the expected in-memory value.

The MCP subprocess path must use one configured binary source of truth. Falling
back between debug and release profiles creates exactly the stale-enum/stale-CLI
problem that produced `CCREALITY_WITNESS_OP_TYPE_UNKNOWN` style failures.

## Fix

- FSV examples now use explicit `require`, `require_eq`, and `require_close`
  checks before writing evidence.
- Evidence writes use explicit file creation, `write_all`, `sync_all`, and a
  separate readback parse before success is printed.
- Added causal-mask FSV evidence for the new `causal_mask` solver module.
- `ccreality::mcp_binary()` now reads `mcpServers.cgreality.command` from
  `.mcp.json` unless `CGREALITY_MCP_BIN` is explicitly set.
- `context-graph-mcp` CLI parsing now fails closed on unknown args, missing
  values, invalid mode, port `0`, and invalid port environment variables.

## Source-of-Truth Evidence

- Commit `6e0a6d4d`: causal-mask module, `.mcp.json` single-source MCP binary
  selection, strict FSV evidence assertions.
- Commit `bc889bec`: MCP CLI port/env fail-closed checks and causal-mask
  non-finite handling documentation.
- `/tmp/contextgraph-tier1-fsv/solver-tier1-evidence.json`
- `/tmp/contextgraph-tier1-fsv/witness-ed25519-evidence.json`
- `/tmp/contextgraph-tier2-fsv/mincut-evidence.json`
- `/tmp/contextgraph-tier2-fsv/granger-evidence.json`
- `/tmp/contextgraph-tier2-fsv/witness-cert-evidence.json`
- `/tmp/contextgraph-tier2-fsv/causal-mask-evidence.json`
- `.mcp.json` points to `/home/cabdru/contextgraph/target/release/context-graph-mcp`.
- Rebuilt release MCP SHA-256:
  `4d5bf2bd0e85c7167fa702ea2890abb5e79f8665a4a70a676de560a877f8c380`.

## Related

- `crates/context-graph-cli/src/commands/ccreality/mod.rs`
- `crates/context-graph-mcp/src/main.rs`
- `crates/context-graph-solver/src/causal_mask.rs`
- `crates/context-graph-solver/examples/causal_mask_fsv.rs`
- `crates/context-graph-mincut/examples/mincut_fsv.rs`
- `crates/context-graph-solver/examples/granger_fsv.rs`
- `crates/context-graph-witness/examples/cert_fsv.rs`
- `crates/context-graph-witness/examples/ed25519_fsv.rs`

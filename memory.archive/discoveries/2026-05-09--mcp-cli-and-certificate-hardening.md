---
namespace: discoveries
agent_id: codex
timestamp: 2026-05-09T02:24:57Z
status: active
tags: ccreality,mcp,cli,witness,certificates,merkle,fail-closed,fsv
---

# MCP CLI And Certificate Hardening

## What Happened

Review of commits from the last two hours found three related hardening gaps:

- The `ccreality` CLI previously resolved `target/debug/context-graph-mcp` independently of `.mcp.json`, allowing hook-time verifier behavior to differ from the release MCP process that Claude Code actually uses. That can surface as `CCREALITY_WITNESS_OP_TYPE_UNKNOWN` when the debug verifier is stale.
- Stage 0 and readout certificates needed to verify that the certificate witness segment is exactly one entry and that the entry `action_hash` equals the recomputed certificate binding hash, not merely that the bytes are a structurally valid witness chain.
- `context-graph-mcp` ignored unknown CLI flags. A manual `--version` probe demonstrated the bug: the unknown flag was warned about, then startup continued into RocksDB. Unknown/malformed CLI args must fail before any runtime startup.

## Why It Matters

Fail-closed behavior depends on a single runtime source of truth and explicit binding checks. A stale verifier or an ignored CLI flag can make an operator believe one thing was verified while a different binary/path or startup mode actually executed.

## Resolution

- `context-graph-cli` now resolves the MCP binary from `CGREALITY_MCP_BIN` only when explicitly set, otherwise from `.mcp.json` `mcpServers.cgreality.command`; no debug/release inference path remains.
- The certificate module now rejects valid-but-wrong witness entries with `CERT_MANIFEST_HASH_MISMATCH`, rejects multi-entry certificate segments with `CERT_WITNESS_ENTRY_COUNT_MISMATCH`, and Merkle leaf hashing length-prefixes the leaf ID.
- `context-graph-mcp` argument parsing now returns errors for unknown flags, missing option values, invalid modes, and invalid/zero ports. `--version` exits before logging/storage startup.

## Research Applied

- RFC 6962 Merkle tree hashing uses distinct leaf/node domain separation; ContextGraph keeps separate tags and now length-prefixes structured leaf IDs before hashing.
- Rust `std::process::Command` starts the exact configured program path; the CLI resolver now uses `.mcp.json` as the configured program source rather than guessing another binary.
- Twelve-Factor config guidance treats configuration as explicit environment/config, which supports using explicit env override plus committed `.mcp.json`, not implicit build-profile fallback.

## Related

- `crates/context-graph-cli/src/commands/ccreality/mod.rs`
- `crates/context-graph-mcp/src/main.rs`
- `crates/context-graph-witness/src/cert.rs`
- `crates/context-graph-witness/src/cert_merkle.rs`
- `memory/reports/session-2026-05-09--last-two-hours-hardening.md`

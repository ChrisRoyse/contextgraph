# Session Report: Last-Two-Hours Hardening

## Scope

Reviewed git history from the last two hours on `ccreality-cutover`, with emphasis on Tier 1/Tier 2 primitives, ccreality MCP execution, MCP server CLI/env parsing, causal masks, and full-state verification examples.

Pre-report source state: worktree clean at `bc889bec` (`tier1/2 hardening: cli port fail-closed + causal-mask non-finite asymmetry docs`).

## Root Causes And Fixes

1. ccreality hook version skew
   - Root cause: the ccreality hook resolver could select a stale `target/debug/context-graph-mcp` binary while `.mcp.json` pointed at the release binary. Cargo keeps debug and release artifacts in separate target profile directories, so the debug executable can legitimately lag the release executable.
   - Fix: resolver now treats `.mcp.json` `mcpServers.cgreality.command` as the source of truth, supports only explicit `CGREALITY_MCP_BIN` override, validates executability, and fails closed with physical path details. No inferred debug/release fallback remains.

2. MCP server port parsing
   - Root cause: `--port 0` and `--daemon-port 0` parsed as valid `u16`, and malformed env ports could silently fall back/default.
   - Fix: `parse_port_number` rejects port 0 and malformed values for CLI and env sources. `determine_daemon_port` now returns `Result<u16>` so invalid `CONTEXT_GRAPH_DAEMON_PORT` fails closed instead of defaulting.

3. Tier 1/Tier 2 primitive boundaries
   - Root cause: the new primitives had gaps at public boundaries: invalid mincut edges, too-short or non-finite Granger inputs, and witness certificate action-hash binding could fail less explicitly than required.
   - Fix: added fail-closed validation and targeted tests for mincut, Granger, witness certificate rebinding, Merkle proof/hash behavior, and durable FSV readbacks.

4. Causal mask verification
   - Root cause: causal masking needed explicit state verification and clearer non-finite semantics. The intended pipeline is: reject all non-finite input scores, write `NEG_INFINITY` only from masking, then allow only that sentinel in softmax.
   - Fix: added causal mask API, tests, FSV evidence, non-mutation assertion on rejected non-finite inputs, strict self-attention rejection, empty/shape/retrocausal edge cases, and documentation of the input/output non-finite asymmetry.

## Verification Commands

- `cargo test -p context-graph-mcp cli_arg_tests -- --nocapture`: 3 passed.
- `cargo test -p context-graph-cli --lib ccreality::tests:: -- --nocapture`: 6 passed.
- `cargo test -p context-graph-solver causal_mask -- --nocapture`: 13 passed.
- Earlier full primitive sweep completed successfully:
  - `cargo test -p context-graph-mincut -p context-graph-solver -p context-graph-witness --features context-graph-witness/ed25519 -- --nocapture`
  - `cargo test -p context-graph-solver -- --nocapture`
  - `cargo test -p context-graph-mincut -p context-graph-witness --features context-graph-witness/ed25519 -- --nocapture`
- `cargo fmt --all`: passed.
- `git diff --check`: passed.
- Release build completed successfully for `context-graph-cli`, `context-graph-solver`, `context-graph-mincut`, and `context-graph-witness` with `context-graph-witness/ed25519`.

Known warnings observed during tests/builds are existing dead-code warnings in governed/path-policy code and were not introduced by this pass.

## Full-State Verification Evidence

Source of truth for ccreality binary:

- `.mcp.json` readback: `/home/cabdru/contextgraph/target/release/context-graph-mcp`
- Executable stat readback:
  - `target/release/context-graph-mcp`: `-rwxr-xr-x`, size `626691416`
  - `target/debug/context-graph-mcp`: `-rwxr-xr-x`, size `806415256`
  - `target/release/context-graph-cli`: `-rwxr-xr-x`, size `78723832`

Durable evidence files and hashes:

- `/tmp/contextgraph-tier2-fsv/mincut-evidence.json`: `592d2ca0a7e4a3a3531e7547b08388518725774b2b96ff7b7d7ba0ed48a501f1`
- `/tmp/contextgraph-tier2-fsv/granger-evidence.json`: `1266afa1bc73f70ea6d805cbf1d57598f91f8fb0a957910b53d8a6337d3f728b`
- `/tmp/contextgraph-tier2-fsv/causal-mask-evidence.json`: `2df1cb3f3bc29f47014e57f92dd5f1410fb7e84883c59d7e5316a56128260ca8`
- `/tmp/contextgraph-tier2-fsv/witness-cert-evidence.json`: `ab0ce06fcb5cc093cf34ecfd22b0a2caf17cfd3dc0a74694a47fa53e1357c5a8`
- `/tmp/contextgraph-tier1-fsv/solver-tier1-evidence.json`: `85106c69a72448b3f05ba53b0a55956a31846f6f95a1e9d5773a3815cf74974d`
- `/tmp/contextgraph-tier1-fsv/witness-ed25519-evidence.json`: `5b891ddca536d11bdf9a98d709e16734896fce76fa31422427f005908306518a`
- `/tmp/contextgraph-tier2-fsv/stage0-witness-chain.bin`: `95131b83c52bd01803d1bd1eb720d679bd187adc393d4ffbdd0ecdc3789c4a5f`
- `/tmp/contextgraph-tier2-fsv/readout-trajectory-root.bin`: `1bd96ec1d69877b82466cbfe7cfedc2818f340e7c6bfe09e2263c8090a23ff20`
- `/tmp/contextgraph-tier2-fsv/mcp-port-failclosed-evidence.txt`: `dc1d118df7389f760a3909d6cfac40c175c532a6f80d3af1b818312f1a923764`

Manual edge cases with before/after state in evidence:

- MCP CLI `--port 0`: nonzero exit, stderr contains `1-65535`.
- MCP CLI `--daemon-port 0`: nonzero exit, stderr contains `1-65535`.
- MCP CLI unknown argument: nonzero exit, stderr contains `Unknown argument`.
- MCP env `CONTEXT_GRAPH_TCP_PORT=0`: nonzero exit, stderr names `CONTEXT_GRAPH_TCP_PORT`.
- MCP env `CONTEXT_GRAPH_DAEMON_PORT=not-a-port`: nonzero exit, stderr names `CONTEXT_GRAPH_DAEMON_PORT`.
- Causal mask edge cases: empty timestamps, shape mismatch, non-finite input preservation, and retrocausal future attention rejection.
- Granger edge cases: too-short-for-degrees-of-freedom and NaN F-statistic rejection.
- Witness certificate edge cases: wrong action hash, wrong proof, multi-entry witness segment rejection.

## Research Sources

- Cargo build cache and profile target directories: https://doc.rust-lang.org/cargo/reference/build-cache.html and https://doc.rust-lang.org/cargo/reference/profiles.html
- Rust test organization: https://doc.rust-lang.org/book/ch11-03-test-organization.html
- PyTorch attention-mask semantics using `-inf` for disallowed attention positions: https://docs.pytorch.org/docs/2.9/generated/torch.nn.functional.scaled_dot_product_attention.html
- RFC 6962 Merkle leaf/internal node domain separation: https://www.rfc-editor.org/rfc/rfc6962

## Notes

No mock data was used in the FSV examples. Synthetic inputs were deterministic and chosen so expected outputs were known before execution, then the durable JSON/binary sources of truth were read back and hashed.

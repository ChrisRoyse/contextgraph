---
namespace: reports
agent_id: codex
timestamp: 2026-05-09T02:27:42Z
status: active
tags: report,tier1,tier2,fsv,mcp,causal-mask,mincut,granger,certificates
---

# Session Report: 2026-05-09 - Tier-1/2 Hardening FSV

## Task: Review and harden everything implemented in the last two hours.
## Outcome: complete

## Summary

Reviewed recent commits from `33daf95b` through the new hardening commits, plus
the uncommitted files that became `6e0a6d4d` and `bc889bec`. The main root
causes were weak FSV assertions and stale binary selection. The FSV examples now
fail the process if expected and actual outcomes diverge, fsync evidence before
readback, and print the readback state. The hook path now uses `.mcp.json` as
the MCP binary source of truth and fails closed instead of falling back to a
different build profile.

Web research checked official Rust docs for explicit write/read/sync behavior,
Cargo profile behavior, and PyTorch attention-mask semantics around `-inf`.
Those sources aligned with the implemented fixes: check IO errors, sync/read
source-of-truth files, use one configured binary/profile, and mask forbidden
attention with negative infinity while rejecting invalid non-finite inputs.

## Files Created or Modified in the Repo

- `crates/context-graph-cli/src/commands/ccreality/mod.rs` - `.mcp.json`
  source-of-truth MCP binary selection; no debug/release fallback; tests.
- `crates/context-graph-mcp/src/main.rs` - fail-closed CLI parsing for unknown
  args, missing values, invalid mode, port zero, and daemon-port env.
- `crates/context-graph-solver/src/causal_mask.rs` - causal mask API,
  retrocausal safety guard, strict non-finite input handling.
- `crates/context-graph-solver/src/causal_mask_tests.rs` - causal mask unit
  tests.
- `crates/context-graph-solver/examples/causal_mask_fsv.rs` - physical FSV
  evidence for causal masking.
- `crates/context-graph-mincut/examples/mincut_fsv.rs`,
  `crates/context-graph-solver/examples/granger_fsv.rs`,
  `crates/context-graph-solver/examples/tier1_fsv.rs`,
  `crates/context-graph-witness/examples/cert_fsv.rs`,
  `crates/context-graph-witness/examples/ed25519_fsv.rs` - strict assertions,
  durable writes, and readback.
- `crates/context-graph-witness/src/cert_merkle.rs` - stronger Merkle leaf ID
  length binding.

## Files Written or Updated in ./memory/

- `memory/discoveries/2026-05-09--tier1-tier2-fsv-and-binary-source-hardening.md`
  - root-cause discovery.
- `memory/reports/session-2026-05-09--tier1-tier2-hardening-fsv.md` - this
  report.

## Decisions Made

- MCP binary source of truth is `.mcp.json` unless `CGREALITY_MCP_BIN` is
  explicitly set. Rationale: profile fallback hides stale binary drift.
- FSV examples must assert every expected outcome before writing evidence and
  must read the evidence file back before success. Rationale: evidence with a
  false `passes` or mismatched code is a failed FSV, not a passing report.

## Discoveries

- The physical release MCP binary was stale before rebuild: it ignored an
  unknown argument and continued into RocksDB startup. After rebuild,
  `target/release/context-graph-mcp --definitely-invalid` exits 1 immediately.
- The new causal-mask module had unit coverage but needed physical FSV evidence.
  Added `/tmp/contextgraph-tier2-fsv/causal-mask-evidence.json`.

## Failures

- Initial physical MCP check failed the intended invariant because the release
  binary was stale. Lesson: after source changes to the MCP binary, release
  rebuild and direct binary behavior checks are required.

## Successes

- Release MCP after rebuild rejects unknown args and `--port 0` before any
  RocksDB open.
- Physical FSV evidence readback confirms MMR/conformal, Ed25519, mincut,
  Granger, certificates, and causal masking.

## Tests / Build

- `cargo fmt --all`
- `cargo test -p context-graph-cli ccreality --lib -- --nocapture`
- `cargo test -p context-graph-cli --lib rocksdb_preflight -- --nocapture`
- `cargo test -p context-graph-mincut -- --nocapture`
- `cargo test -p context-graph-solver -- --nocapture`
- `cargo test -p context-graph-solver causal_mask -- --nocapture`
- `cargo test -p context-graph-witness --features ed25519 -- --nocapture`
- `cargo test -p context-graph-mcp --lib witness_chain_repair -- --nocapture`
- `cargo test -p context-graph-mcp --lib optimizer_preflight -- --nocapture`
- `cargo test -p context-graph-mcp --lib bandit -- --nocapture`
- `cargo test -p context-graph-mcp --lib influence_ -- --nocapture`
- `cargo test -p context-graph-mcp --bin context-graph-mcp cli_parse -- --nocapture`
- `cargo test -p context-graph-mcp --bin context-graph-mcp daemon_port_env_rejects_invalid_values -- --nocapture`
- `cargo run -p context-graph-solver --example tier1_fsv`
- `cargo run -p context-graph-witness --features ed25519 --example ed25519_fsv`
- `cargo run -p context-graph-mincut --example mincut_fsv`
- `cargo run -p context-graph-solver --example granger_fsv`
- `cargo run -p context-graph-witness --features ed25519 --example cert_fsv`
- `cargo run -p context-graph-solver --example causal_mask_fsv`
- `cargo build --release -p context-graph-cli -p context-graph-mcp -p context-graph-solver -p context-graph-mincut -p context-graph-witness --features context-graph-witness/ed25519`
- `cargo build --release -p context-graph-mcp -p context-graph-solver`
- `target/release/context-graph-mcp --definitely-invalid`
- `target/release/context-graph-mcp --port 0`
- `target/release/context-graph-mcp --version`

## Source-of-Truth Readbacks

- `/tmp/contextgraph-tier2-fsv/mincut-evidence.json`: paper figure mincut 4.0,
  OOB edge and self-loop reject with expected codes.
- `/tmp/contextgraph-tier2-fsv/granger-evidence.json`: causal p-value 0.0,
  independent p-value 0.419, too-short input rejects.
- `/tmp/contextgraph-tier2-fsv/witness-cert-evidence.json`: Stage 0 and readout
  wrong action hashes reject; raw chain/root roundtrips true.
- `/tmp/contextgraph-tier2-fsv/causal-mask-evidence.json`: time-window mask
  passes, future softmax probabilities are `[0,0,0]`, empty/shape/non-finite/
  retrocausal boundaries reject.
- `/tmp/contextgraph-tier1-fsv/solver-tier1-evidence.json`: MMR order `[0,2]`,
  conformal rank indices `[0,1,3]`, top candidate included.
- `/tmp/contextgraph-tier1-fsv/witness-ed25519-evidence.json`: 3 chain entries,
  219-byte segment, 96-byte signature, wrong pubkey rejects.
- `.mcp.json`: command is `/home/cabdru/contextgraph/target/release/context-graph-mcp`.
- `target/release/context-graph-mcp`: SHA-256
  `4d5bf2bd0e85c7167fa702ea2890abb5e79f8665a4a70a676de560a877f8c380`,
  mode 755, version `0.1.0`.

## Open Blockers

- None.

## Resume Here Next Session

- Start by checking `git status --short`; code was committed through
  `bc889bec`.
- Re-run the FSV examples if modifying any Tier-1/2 primitive.
- Keep `.mcp.json` as the MCP binary source of truth and rebuild release before
  validating hook behavior.

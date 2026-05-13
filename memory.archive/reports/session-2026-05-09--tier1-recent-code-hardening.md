---
namespace: reports
agent_id: codex
timestamp: 2026-05-09T01:15:54Z
status: active
tags: report, tier1, conformal, mmr, witness, ed25519, dynamicjepa, fsv
---

# Session Report: 2026-05-09 — Tier-1 Recent Code Hardening

## Task

Review recent commits from the last hour, research best practices after identifying root causes, harden the code, and run full state verification with physical source-of-truth evidence.

## Outcome

Complete. Recent Tier-1 solver/witness and DynamicJEPA/MCP paths were reviewed, hardened, committed at `9e50929a`, built, and manually verified against source-of-truth files and RocksDB evidence.

## Summary

The main root causes found were incorrect rank polarity in the new conformal predictor, unbounded MMR similarity inputs despite the algorithm requiring a relevance-minus-similarity tradeoff, and Ed25519 witness signatures accepting arbitrary byte slices instead of only valid witness-chain segments. Research checked split conformal prediction score/quantile semantics, the original MMR formula, RFC 8032 Ed25519 verification guidance, and ed25519-dalek strict verification guidance.

The implementation now treats rank 0 as the most conforming candidate, rejects out-of-range MMR similarities in both closure and matrix paths, and validates witness signature segments as non-empty 73-byte internally linked entries before signing, digesting, or verifying. Witness verification also rejects weak public keys and uses strict Ed25519 verification.

Additional DynamicJEPA hardening validates RocksDB inspection targets before opening them: path must be a directory, have `CURRENT`, point to a single local `MANIFEST-*`, and that manifest must exist and be non-empty.

## Files Created or Modified in the Repo

- `crates/context-graph-solver/src/conformal.rs` — rank nonconformity polarity and validation.
- `crates/context-graph-solver/src/mmr.rs` — similarity range validation.
- `crates/context-graph-solver/examples/tier1_fsv.rs` — persistent manual FSV evidence for MMR/conformal.
- `crates/context-graph-witness/src/sign.rs` — fail-closed segment validation, weak-key rejection, strict verification.
- `crates/context-graph-witness/examples/ed25519_fsv.rs` — persistent manual FSV evidence for signing.
- `crates/context-graph-cli/src/commands/dynamicjepa.rs` — RocksDB preflight hardening and tests.
- `crates/context-graph-mcp/src/handlers/tools/reality_loop/witness_chain_repair_fsv.rs` — active-runtime helper with better source-of-truth diagnostics.

## Files Written or Updated in ./memory/

- `memory/discoveries/2026-05-09--tier1-rank-and-signature-hardening.md` — records root-cause findings.
- `memory/reports/session-2026-05-09--tier1-recent-code-hardening.md` — this report.

## Decisions Made

- Rank nonconformity is raw rank, not inverse rank. Rationale: conformal prediction includes candidates with score <= threshold, so score must increase with candidate strangeness.
- Witness signatures certify only valid witness segments, not arbitrary bytes. Rationale: the public API name and downstream verifier semantics are witness-chain specific.

## Discoveries

- The previous rank test encoded the broken behavior by excluding the strongest candidate.
- Signing arbitrary bytes would have let a valid signature certify data that was not a witness segment.
- The active-runtime ignored FSV can fail from a stale `~/.cache/cgreality/active_runtime_root` even when a real active runtime exists under `/tmp/contextgraph-rust-reality-loop`; the helper now reports the cached root and directory entries.
- RocksDB source-of-truth preflight should validate `CURRENT -> MANIFEST-*`, not just path existence, because `CURRENT` is the pointer to the latest consistent manifest.
- Running release builds in parallel shares cargo locks and is slower than sequencing; the final combined release build completed.

## Failures

- First hardened witness test mutated the first entry and now failed through the structural segment validator instead of Ed25519 verification. Lesson: keep separate tests for structural rejection and cryptographic rejection by mutating the last entry payload for the latter.
- `witness_active_runtime_chain_verifies` initially failed because `~/.cache/cgreality/active_runtime_root` pointed at `/tmp/.tmppb1TSE`, which contained hook state but no `rust_reality_*` runtime. Restoring it to `/tmp/contextgraph-rust-reality-loop` made the real source-of-truth witness chain visible.

## Successes

- Solver tests passed: 31 unit tests.
- Witness tests passed: 18 unit tests with `ed25519`.
- DynamicJEPA Phase 7 and Phase 9 FSV tests passed with real CUDA/RocksDB artifacts.
- MCP witness, bandit, influence, and optimizer-preflight tests passed.
- Manual FSV wrote and re-read `/tmp/contextgraph-tier1-fsv/solver-tier1-evidence.json` and `/tmp/contextgraph-tier1-fsv/witness-ed25519-evidence.json`.
- Active runtime witness FSV read and verified `/tmp/contextgraph-rust-reality-loop/rust_reality_1778240223405536609_aaf099e6/claude-code-optimizer/witness-chain.bin`.
- Release builds passed for `context-graph-solver`, `context-graph-witness`, `context-graph-mcp`, and `context-graph-cli`.

## Source Of Truth Evidence

- Solver FSV: `/tmp/contextgraph-tier1-fsv/solver-tier1-evidence.json`, SHA-256 `10dd1c493249f001e81cfca2f36c41f16fab5b6cda371186d5401b7877893313`.
- Witness FSV: `/tmp/contextgraph-tier1-fsv/witness-ed25519-evidence.json`, SHA-256 `5b891ddca536d11bdf9a98d709e16734896fce76fa31422427f005908306518a`.
- Witness segment: `/tmp/contextgraph-tier1-fsv/witness-segment.bin`, 219 bytes, SHA-256 `fa0cfa6ed713fa6997e539e968a54bd2c97eb1e98db1218dc1d04fc48e6fe8f7`.
- Witness signature: `/tmp/contextgraph-tier1-fsv/witness-segment.sig`, 96 bytes, SHA-256 `d152ad778a0d37e3c2142c711dc2b25ea33f3db6196a36185119dec801b08676`.
- Witness public key: `/tmp/contextgraph-tier1-fsv/witness-segment.pub`, 32 bytes, SHA-256 `8a47cf56c4b420a737fea607f98b0dce750730beb7c4df614eb739393521f044`.
- Influence FSV: `target/context-graph-influence-fsv/manual-evidence.json`, SHA-256 `0038f016f757ea875aadf6ef590a5fec2fb9c5207e36cb19b78bfbcef3ea903a`.
- Witness repair FSV: `crates/context-graph-mcp/target/context-graph-witness-repair-fsv/manual-evidence.json`, SHA-256 `ea2c6637402cca0b219435646057d1755a84782ccbce6529b83adbf002abbb57`.
- Active witness FSV: `crates/context-graph-mcp/target/context-graph-witness-repair-active/verify-evidence.json`, SHA-256 `7666438cc04d6f7c6de5202606a41f2024dcbe299fe1cc4b438fe0c6d6492707`.
- DynamicJEPA Phase 9 source of truth: `tmp/5090jepa_evidence/phase9/mcp_phase9_fsv_runtime_1599452_833dc708-1336-470e-86ab-e2972a43daf0/dynamicjepa_phase9_mcp_rocksdb`, with `CURRENT` pointing to non-empty `MANIFEST-000454` (17,157 bytes).
- DynamicJEPA Phase 9 physical ldb counts: `dj_predictions=6`, `dj_model_artifacts=1`, `dj_surprise_events=1`, `dj_audit_log=315`, `dj_audit_witness_chain=315`.
- DynamicJEPA Phase 9 artifact source of truth: `tmp/5090jepa_evidence/phase9/mcp_phase9_fsv_runtime_1599452_833dc708-1336-470e-86ab-e2972a43daf0/dynamicjepa_phase9_artifacts/counter_world/58af4846-0d06-4806-804b-e5da3cd61fef/`, including `model.safetensors` 1,384 bytes and manifest hashes for config, evaluation report, metrics, and model.

## Tests / Build

- `cargo fmt --all`
- `cargo test -p context-graph-solver -- --nocapture`
- `cargo test -p context-graph-witness --features ed25519 -- --nocapture`
- `cargo check -p context-graph-solver -p context-graph-witness --features context-graph-witness/ed25519`
- `cargo run -p context-graph-solver --example tier1_fsv`
- `cargo run -p context-graph-witness --features ed25519 --example ed25519_fsv`
- `cargo test -p context-graph-cli --test dynamicjepa_phase7_fsv dynamicjepa_phase7_cli_fsv_real_cuda_artifacts -- --nocapture`
- `cargo test -p context-graph-mcp --lib test_dynamicjepa_phase9_mcp_full_state_verification -- --nocapture`
- `cargo test -p context-graph-mcp --lib witness_ -- --nocapture`
- `cargo test -p context-graph-mcp --lib influence_manual_fsv_persists_target_evidence -- --ignored --nocapture`
- `cargo test -p context-graph-mcp --lib witness_repair_manual_fsv_persists_target_evidence -- --ignored --nocapture`
- `cargo test -p context-graph-mcp --lib witness_active_runtime_chain_verifies -- --ignored --nocapture`
- `cargo test -p context-graph-mcp --lib optimizer_preflight -- --nocapture`
- `cargo test -p context-graph-mcp --lib bandit -- --nocapture`
- `cargo test -p context-graph-mcp --lib influence_ -- --nocapture`
- `cargo test -p context-graph-cli rocksdb_preflight -- --nocapture`
- `cargo build --release -p context-graph-cli -p context-graph-mcp -p context-graph-solver -p context-graph-witness --features context-graph-witness/ed25519`

## Open Blockers

- None.

## Resume Here Next Session

- Start by reviewing `git status --short`; the code hardening was committed at `9e50929a`, while this memory report may remain uncommitted session documentation.
- Re-read this report and `memory/discoveries/2026-05-09--tier1-rank-and-signature-hardening.md`.
- If continuing DynamicJEPA source-of-truth work, keep using unique runtime roots and validate RocksDB `CURRENT -> MANIFEST-*` before inspection.

# Session Memory: Phase 4 TCT and ME-JEPA Hardening

Date: 2026-05-10

## Scope

The user requested full hardening of everything implemented for Phase 4, with specific emphasis on the Phase 4 TCT constellation task set, robust root-cause fixes, no workarounds or silent fallbacks, no mock data, full manual testing, Full State Verification, and source-of-truth inspection.

Relevant directives read and followed:

- `/home/cabdru/leapablememory/docs2/SOLO-AGENT-MEMORY-PROTOCOL.md`
- Phase 4 / TCT docs under `/home/cabdru/contextgraph/docs/ruvectorfindings/`
- Phase 4 specs under `/home/cabdru/contextgraph/docs/ruvectorfindings/specs/tasks/phase4-tct-constellation`
- Existing project instructions in `/home/cabdru/contextgraph/CLAUDE.md`

## Research Used

Official references used for hardening decisions:

- OWASP ASVS: https://owasp.org/www-project-application-security-verification-standard/
- OWASP Input Validation Cheat Sheet: https://cheatsheetseries.owasp.org/cheatsheets/Input_Validation_Cheat_Sheet.html
- Rust Clippy `result_large_err`: https://rust-lang.github.io/rust-clippy/master/index.html#result_large_err

The research reinforced fail-closed input validation at trust boundaries, explicit bounds for external input, rejecting malformed data before state mutation, and reducing oversized error variants structurally instead of suppressing static-analysis findings.

## Root Causes Found

1. Serde deserialization and MCP tool input paths could construct Phase 4 ME-JEPA structs without constructor-level validation.
2. Some store reads silently returned empty state or clamped invalid limits, hiding corruption or caller mistakes.
3. Persistent writes did not consistently read back and validate the exact bytes stored in RocksDB.
4. TCT cosine similarity treated zero-norm vectors as similarity `0.0`, which was a silent fallback.
5. MCP `mejepa_verify` always used the deterministic Phase 4 harness path, making production usage appear valid when it was not explicitly enabled.
6. Full MCP clippy surfaced a structural oversized-error issue through `clippy::result_large_err`; the root cause was large error and response enum payloads, not the lint itself.

## Main Changes

ME-JEPA hardening:

- Added validation for `PatchBundle`, `AstDiff`, `DiffHunk`, `TaskContext`, `TaskEnvironment`, `ClaimGraph`, `SkillCitation`, `ConformalSet`, and `RealityPrediction`.
- Added fail-closed validation in `compile_with_panel` and `verify` before source, witness, or filesystem access.
- Added `TrainCertSummary::validate` and `OracleScores::validate`.
- Hardened Granger attestation checks so cited skills must have attestations.
- Aligned Phase 4 gate counts to seven gates.
- Hardened calibration records so language sample counts, frozen timestamps, and embedder version values must be valid.
- Hardened RocksDB persistence with read-after-write byte equality, decode equality, and validation.
- Hardened live prediction reads/writes with explicit limit rejection, key/payload consistency checks, and validation.
- Added Phase 4 inference FSV edge-case artifact generation.

TCT hardening:

- Replaced zero-norm cosine fallback with explicit `MEJEPA_INFER_CONSTELLATION_VIOLATION` errors.
- Added zero-norm rejection coverage.
- Added `VerdictRecord::validate`.
- Hardened verdict persistence with read-after-write verification and decoded-state validation.
- Hardened TCT store version listing and raw value iteration with key and payload consistency checks.

MCP hardening:

- Added validation at `mejepa_verify` tool boundary.
- Made deterministic Phase 4 verification fail closed unless `CONTEXTGRAPH_MEJEPA_ALLOW_DETERMINISTIC_VERIFY=true` or `1`.
- Updated `mejepa_verify` tool description to disclose the deterministic harness opt-in requirement.
- Made `predict_latest` reject invalid limits instead of clamping.
- Added MCP tests for the explicit deterministic harness opt-in.

Static-analysis hardening:

- Reduced large MCP error and response variants by boxing large payloads and introducing `SmallString`.
- Preserved existing serialization behavior while making `cargo clippy -p context-graph-mcp --no-deps -- -D warnings` pass without suppressing `result_large_err`.

## Files Changed Most Directly

- `crates/context-graph-mejepa/src/types.rs`
- `crates/context-graph-mejepa/src/compiler.rs`
- `crates/context-graph-mejepa/src/store.rs`
- `crates/context-graph-mejepa/src/calibration.rs`
- `crates/context-graph-mejepa/src/calibration_types.rs`
- `crates/context-graph-mejepa/examples/inference_fsv.rs`
- `crates/context-graph-mejepa-tct/src/gtau.rs`
- `crates/context-graph-mejepa-tct/src/rate_aggregator.rs`
- `crates/context-graph-mejepa-tct/src/store.rs`
- `crates/context-graph-mejepa-tct/src/tests.rs`
- `crates/context-graph-mcp/src/handlers/tools/mejepa_tools.rs`
- `crates/context-graph-mcp/src/tools/definitions/mejepa.rs`
- `crates/context-graph-mcp/src/handlers/tools/reality_loop/errors.rs`
- `crates/context-graph-mcp/src/protocol.rs`
- `crates/context-graph-mcp/src/handlers/tests/dynamicjepa_tools.rs`

## Verification Run

Passing automated checks:

- `cargo test -p context-graph-mejepa -p context-graph-mejepa-tct -p context-graph-mejepa-train -- --nocapture`
- `cargo test -p context-graph-mcp mejepa -- --nocapture`
- `cargo test -p context-graph-mcp tools::definitions::tests -- --nocapture`
- `cargo clippy -p context-graph-mejepa-tct -p context-graph-mejepa -p context-graph-mejepa-train --no-deps -- -D warnings`
- `cargo clippy -p context-graph-mcp --no-deps -- -D warnings`
- `cargo run --release -p context-graph-mejepa --example inference_fsv`
- `cargo run --release -p context-graph-mejepa-tct --example tct_fsv`

Full MCP suite note:

- `cargo test -p context-graph-mcp -- --nocapture` was attempted.
- Result: 361 passed, 46 failed, 6 ignored.
- Failures were pre-existing or environment-bound, mostly requiring production models at `/home/cabdru/contextgraph/models` and an RTX 5090 CUDA environment. One unrelated embedder definition count mismatch also remained. This was not claimed as green.

## Full State Verification Evidence

ME-JEPA inference FSV source of truth:

- RocksDB: `/tmp/contextgraph-mejepa-infer-fsv/inference-rocksdb`
- Evidence files:
  - `/tmp/contextgraph-mejepa-infer-fsv/verify-evidence.json`
  - `/tmp/contextgraph-mejepa-infer-fsv/edge-case-audit.json`
- Direct RocksDB column-family inspection:
  - `CF_MEJEPA_LIVE_PREDICTIONS`: 69 keys
  - `CF_MEJEPA_CALIBRATION_HISTORY`: 1 key
- Happy path evidence:
  - 8 verify fixtures executed.
  - 1 approval path present.
  - Rejection paths included out-of-distribution, low confidence, predicted failure, Granger rejection, witness-chain broken, source SHA drift, and constellation guard rejection.
- Edge cases:
  - Empty hunks rejected.
  - Path escape rejected.
  - Limit `1001` rejected.
  - Same-size corrupt witness rejected by witness-chain validation.
  - All negative cases preserved RocksDB live prediction and calibration counts.

TCT FSV source of truth:

- RocksDB: `/tmp/contextgraph-mejepa-tct-fsv/tct-rocksdb`
- Evidence files:
  - `/tmp/contextgraph-mejepa-tct-fsv/constellation-evidence.json`
  - `/tmp/contextgraph-mejepa-tct-fsv/threshold-calibration-evidence.json`
- Direct RocksDB column-family inspection:
  - `CF_MEJEPA_CONSTELLATION`: 1 key
  - `CF_MEJEPA_GUARD_DECISIONS`: 0 keys
- Happy path evidence:
  - Constellation rows changed from 0 to 1.
  - Latest version loaded: `bcd655de900ccff49f0be271efd0346d7af3524b5be5089e4dfa329451cc1feb`.
  - `panel_level_cells=63`, `per_chunk_type_cells=21`, `shrunken_cells=0`, `n_50_plus=21`.
- Edge cases:
  - Empty validation rejected with `MEJEPA_TCT_INSUFFICIENT_SAMPLES`.
  - Provenance mismatch rejected with `MEJEPA_TCT_PROVENANCE_MISMATCH`.
  - Stale constellation rejected with `MEJEPA_INFER_GTAU_STALE_CONSTELLATION`.
  - All negative cases preserved state mutation as false.

## Residual Risks / Follow-Up

- Full MCP package tests are not fully green because several tests require local production model assets and CUDA hardware that were unavailable in this environment.
- Some touched Phase 4 files are now above the local preferred file-length guideline; this was accepted for this pass to avoid a broad refactor during hardening, but the ME-JEPA type/compiler modules are candidates for a later structural split.
- Existing unrelated worktree modifications were preserved and not reverted.


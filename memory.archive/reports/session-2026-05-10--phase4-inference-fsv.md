# Session Report: Phase 4 ME-JEPA Inference FSV

Date: 2026-05-10

## Scope

Implemented Phase 4 inference in `crates/context-graph-mejepa` and wired the MCP tool surface in `crates/context-graph-mcp`.

## Source Of Truth

- Inference RocksDB CFs:
  - `CF_MEJEPA_CALIBRATION_HISTORY`
  - `CF_MEJEPA_LIVE_PREDICTIONS`
  - `CF_MEJEPA_ORACLE_VERDICTS`
  - `CF_MEJEPA_TRAIN_CERTS`
- FSV evidence directory: `/tmp/contextgraph-mejepa-infer-fsv`
- CLI smoke evidence directory: `/tmp/contextgraph-mejepa-infer-cli/fsv`

## Implemented

- Public inference types: `Language`, `OracleOutcome`, `ConformalSet`, `RealityPrediction`, `VerifyVerdict`, `FailedGate`, `PatchBundle`, `TaskContext`, and related ID/provenance types.
- Inference error taxonomy in `MejepaInferError` with stable codes and `log_context`.
- `MeJepaInferConfig` with Phase 4 defaults.
- Conformal calibration and outcome-set construction, fail-closed on empty outcome sets.
- OOD scoring, sigma tuning, and AUC.
- ΔΩ × ΔΞ train-health multiplier with explicit bootstrap degraded state.
- Source SHA drift gate and witness-chain replay gate.
- `CalibrationStore` and `RocksDbInferStore`.
- `MeJepaCompiler::compile`, `verify`, and disabled `observe`.
- `mejepa infer-test` CLI.
- `mcp__cgreality__mejepa_verify` and `mcp__cgreality__mejepa_predict_latest` definitions and dispatch handlers. MCP requires `CONTEXTGRAPH_MEJEPA_INFER_DB`; it does not guess a database path.
- Formal FSV harness at `crates/context-graph-mejepa/examples/inference_fsv.rs`.

## Verification

- `cargo test -p context-graph-mejepa`: 24 passed.
- `cargo clippy -p context-graph-mejepa --all-targets -- -D warnings`: passed.
- `cargo check -p context-graph-mcp`: passed.
- `cargo test -p context-graph-mcp tools::definitions::tests`: 6 registry tests passed across lib/bin.
- `cargo run -p context-graph-mejepa --example inference_fsv`: passed and wrote 4 mode-0600 evidence files.
- `mejepa infer-test` synthetic known-data run: empirical coverage `0.9`, p50 latency `< 1ms`, AUC `1.0`.

## Manual State Readback

- FSV evidence modes: all `0600`.
- FSV evidence readback:
  - 8 verify fixtures.
  - Failed gates represented: OOD, low confidence, predicted failure, granger rejection, witness broken, source SHA drift, constellation rejected.
  - calibration `tau=0.050000012`, holdout coverage `0.9`.
  - OOD AUC `1.0`.
  - latency sample count `1000`.
- Physical RocksDB readback with `ldb`:
  - FSV `CF_MEJEPA_CALIBRATION_HISTORY`: 1 row.
  - FSV `CF_MEJEPA_LIVE_PREDICTIONS`: non-zero rows after latency loop.
  - CLI `CF_MEJEPA_CALIBRATION_HISTORY`: 1 row.
  - CLI `CF_MEJEPA_LIVE_PREDICTIONS`: non-zero rows after latency loop.

## Edge Cases

- Empty calibration input: exit code `3`, `MEJEPA_INFER_CONFORMAL_INSUFFICIENT_SAMPLES`, zero calibration rows after execution.
- Invalid calibration JSON: exit code `4`, `MEJEPA_INFER_JSON`, zero calibration rows after execution.
- Gate audit: clean FSV DB re-run produced 8 fixtures with exactly one approve and seven expected failed-gate cases; calibration history persisted.

## Notes

- Phase 4 stays in `context-graph-mejepa`; no parallel inference crate was created.
- The MCP verifier uses the Phase 4 compiler surface and requires an explicit RocksDB path. Production predictor weight loading remains a later integration point; the deterministic Phase 4 harness exists to verify gate/calibration/store semantics without hiding missing production weights.

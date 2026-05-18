# Session 2026-05-12 - Phase B Prediction FSV

## Summary
Implemented Phase B prediction surfaces end to end: schema, compiler population, auxiliary heads, label extraction, per-head UTML, MCP what-if/explain APIs, and manual FSV.

## Source Of Truth
- RocksDB path: `/mnt/d/contextgraph/fsv/contextgraph-phase-b-fsv/phase-b-rocksdb`
- Persisted prediction CF: `CF_MEJEPA_LIVE_PREDICTIONS`
- Evidence JSON: `/mnt/d/contextgraph/fsv/contextgraph-phase-b-fsv/phase-b-prediction-fsv.json`

## Manual State Verification
- Happy path wrote one prediction row, then independently read back the row from `RocksDbInferStore::read_live_predictions`.
- Readback prediction id: `78e5bc08a3e79d8d47487dc9efd7bd7f`
- Persisted row contained:
  - covered chunks: `src/phase_b_sample.py#0`, `requirements.txt#1`
  - security concerns: 2
  - edge cases: 1
  - tech debt: 1
  - cost regressions: 1
- Evidence file permissions verified as `0600`.
- Physical RocksDB files verified, including SST files `000008.sst` and `000009.sst`.

## Edge Cases
- Empty patch: `MEJEPA_INFER_INVALID_INPUT`, live prediction count stayed `1 -> 1`.
- Path traversal hunk: `MEJEPA_INFER_INVALID_INPUT`, live prediction count stayed `1 -> 1`.
- Malformed witness segment: `MEJEPA_INFER_DIM_MISMATCH`, live prediction count stayed `1 -> 1`.

## MCP Verification
- `mcp__cgreality__mejepa_predict_latest` read the persisted row from `CF_MEJEPA_LIVE_PREDICTIONS`.
- `mcp__cgreality__mejepa_explain_prediction` returned normalized saliency:
  - `src/phase_b_sample.py#0`: `0.8709677419354839`
  - `requirements.txt#1`: `0.12903225806451613`
- `mcp__cgreality__mejepa_predict_what_if` returned equal `diskShaBefore` and `diskShaAfter` for `src/phase_b_sample.py`, and `persistedPrediction=false`.
- A follow-up `predict_latest` for the what-if session returned `predictions=[]`, `session_known=false`, proving the what-if path did not persist.

## Remaining Known Issue
Broad `context-graph-cli` clippy debt remains pre-existing and tracked in GitHub issue #14.

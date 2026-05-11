# 15 — Phase 1-5 Hardening + FSV Refresh (2026-05-10)

> **🔄 Superseded as the "current ledger" by [`16_PHASE_6_8_HARDENING_FSV_2026_05_11.md`](./16_PHASE_6_8_HARDENING_FSV_2026_05_11.md).** Doc 15 remains authoritative for **Phases 1-5 evidence**; Phases 6/7/8 + UTML carriers (REQ-UTML-06/07/08) shipped on 2026-05-11 and are tracked in doc 16. The "Still to do" list in §3 of this doc is partially OBSOLETE: Phase 6 hygiene ops, Phase 7 shift subscriber, Phase 8 evaluation harness, and the pairwise MI / continual-backprop / state-transfer T carriers have all shipped end-to-end. See doc 16 for current ledger and remaining blindspots.

> **Status:** Phases 1-5 have been re-run through manual Full State Verification on the current workspace. The operational surfaces for 11-language AST chunking, embedder registry/digest/VRAM/routing, 15-slot panel materialization, predictor, training, UTML `L_step`, inference, TCT, and self-healing all have readback evidence. This doc is the current phase-1-to-5 evidence ledger and blindspot register.

## Root-cause fixes landed

1. **`ΔΞ.target_collapse` now fails closed at the learning-signal boundary.** `crates/context-graph-mejepa-train/src/learning_signal.rs::compute_delta_xi` no longer averages `target_collapse` into a score. Any non-zero or non-finite `target_collapse` returns `MEJEPA_INSTRUMENT_GRADIENT_LEAK` before a training certificate can be constructed or written. `ΔΞ` now uses the multiplicative non-collapse formula from doc 11.
2. **Training FSV evidence is byte-verified.** `train_full_fsv` now serializes evidence once, writes those bytes with mode `0600`, reads the file bytes back, and asserts byte equality before parsing JSON. This avoids numeric reserialization drift and verifies the actual file source of truth.

## Full State Verification evidence

| Phase | Source of truth inspected | Manual FSV result |
|---|---|---|
| 1a AST chunking | `/tmp/contextgraph-ast-chunker-multilang-fsv/aggregate-evidence.json` plus per-language source/evidence dirs | PASS. 11 languages covered: Rust, Python, JavaScript, TypeScript, Go, Java, C, C++, C#, Ruby, PHP. Edge cases fail closed: empty source `MEJEPA_CHUNKER_EMPTY_SOURCE`, invalid Python `MEJEPA_CHUNKER_PARSE_FAILED`, mixed PHP/HTML `MEJEPA_CHUNKER_MIXED_LANGUAGE`. |
| 1b embedder registry/routing | `/mnt/d/contextgraph/fsv/contextgraph-mejepa-embedders-fsv/embedders_fsv_evidence.json` and `/mnt/d/cabdru/contextgraph/models/mejepa_models_config.toml` | PASS. Schema v1 config exists, 21 declared registrations, 19 required active digests verified, 7 learner-state digests verified, 12 active content embedders loaded into the verified set, E5/E11 retired, RTX 5090 VRAM budget passed, 143 routing rows covered. |
| 1c instruments/panels | `/tmp/contextgraph-mejepa-instruments-fsv/phase1-extended/phase1-extended-evidence.json` | PASS. 5,120-d panel, 15 filled slots, RocksDB `mejepa_panels=1`, `mejepa_panel_meta=1`, frozen snapshot hash readback, linear probe top-1 accuracy 1.0 on sample. 16 edge cases fail closed. |
| 2 predictor | `/tmp/contextgraph-mejepa-predictor-fsv/aggregate-evidence.json` | PASS. Predictor forward path finite; p50 1.543 ms, p99 2.451 ms, `l_total=131.29384`, `dod_pass=true`, VRAM accounted at 2.616 GB. Edge cases cover NaN, dimension mismatch, frozen-target gradient leak, and VRAM exhaustion. |
| 3 training + UTML | `/tmp/contextgraph-mejepa-train-fsv/training-fsv-summary.json`, `/tmp/contextgraph-mejepa-train-fsv/utml-l-step-evidence.json`, `/tmp/contextgraph-mejepa-train-fsv/training-cert-chain.json`, `/tmp/contextgraph-mejepa-train-fsv/holdout-eval-evidence.json`, `/tmp/contextgraph-mejepa-train-fsv/edge-case-evidence.json` | PASS. `phase3_fsv_passes=true`, `delta_xi=0.9746794700622559`, `target_collapse=0.0`, 100,000-step cert chain verified with `broken_at=null`, holdout agreement 0.875, target-collapse leak rejected before write with no cert mutation. |
| 4a inference | `/tmp/contextgraph-mejepa-infer-fsv/inference-rocksdb`, `/tmp/contextgraph-mejepa-infer-fsv/verify-evidence.json`, `/tmp/contextgraph-mejepa-infer-fsv/edge-case-audit.json` | PASS. 8 verification fixtures, live prediction CF count readback, calibration row readback, OOD AUC 1.0, p50 5.209 ms. Edge cases leave source-of-truth counts unchanged. |
| 4b TCT | `/tmp/contextgraph-mejepa-tct-fsv/tct-rocksdb` | PASS. One constellation row persisted, stale/provenance/empty-validation edge cases fail closed with no state mutation. |
| 5 self-healing | `/tmp/contextgraph-mejepa-self-healing-fsv`, `/tmp/contextgraph-mejepa-self-healing-fsv-soft`, `/tmp/contextgraph-mejepa-self-healing-fsv-catastrophic` | PASS. Hard/soft/catastrophic drift drills detect drift by 700 observations, promote winning mode C only after holdout win, write all evidence files mode `0600`, detect synthetic witness-integrity violation. Invalid `--max-observations 699` exits with `MEJEPA_HEAL_INVALID_STATE` and creates no output root. |

## What exists now

- A new 11-language atomic chunking API in `context_graph_core::memory::ast` with per-language tree-sitter parsers, typed `Language`/`EntityType`, parse diagnostics, SHA-256 chunk IDs, routing sets, and fail-closed edge handling.
- The legacy `AstCodeChunker` Rust API still exists for backward compatibility. New work should target `context_graph_core::memory::ast::{chunk, chunk_path, chunk_with_options}` unless explicitly maintaining the old Rust-only API.
- `context-graph-mejepa-embedders` verifies model config schema, pinned safetensors digests, active/retired embedder policy, VRAM budget, learner-state calibration, and AST routing coverage.
- `context-graph-mejepa-instruments` can materialize and persist a complete 5,120-d, 15-slot panel and verify RocksDB readback.
- Predictor, training, inference, TCT, and self-healing crates all have runnable FSV examples with physical source-of-truth readback.
- MCP surface already includes `mejepa_verify`, `mejepa_predict_latest`, `mejepa_constellation_inspect`, `mejepa_heal_status`, `mejepa_rollback_to`, `mejepa_load_learner_state`, `mejepa_routing_lookup`, and `mejepa_vram_budget_report`.

## Still to do

- **Do not claim neural embedder forward execution is complete.** The embedder hardening pass proves real artifact existence, digest integrity, active/retired policy, routing coverage, learner-state calibration, and VRAM budget. The per-embedder `EmbedderForward` / `Instrument` wrappers that run actual neural forward passes for E1/E7/E8/E10/E12/E13/E14 remain the largest Phase 1b deep-integration item.
- Converge or explicitly deprecate the older Rust-only `AstCodeChunker` so there is one canonical AST chunking path. The new `memory::ast` API is FSV-proven across 11 languages; the old API is not.
- Replace deterministic code-domain instrument encoders with analyzer/embedder-backed heads for `E_AST`, `E_CFG`, `E_DataFlow`, `E_TypeGraph`, `E_Diff`, `E_Test`, `E_Trace`, `E_Runtime`, and `E_Reasoning` where the project needs semantic capability beyond deterministic FSV scaffolding.
- Add the UTML pairwise MI audit (`REQ-UTML-06`), continual-backprop/dormant-unit intervention (`REQ-UTML-07`), and state-transfer `T` detector (`REQ-UTML-08`).
- Reconcile the simplified shipped `ΔP`, `ΔK`, and `ΔΩ` carriers with the full doc-11 formulas. The critical ΔΞ safety invariant is fixed; the remaining UTML semantic work is to decide whether the simplified carriers are the intended operational formula or to implement the weighted/cosine/Fisher/ECE/plasticity forms from `SPEC-UTML`.
- Finish Phase 6 hygiene: Merkle compression of old witness chains, nightly GC, quota enforcement, and `me_jepa::ops::*` MCP wrappers.
- Finish Phase 7 shift subscriber: restart-safe Tokio tailer, watermark CF consumer, `mejepa_observe_shift`, `mejepa_subscriber_status`, and `mejepa_capture_audit`.
- Finish Phase 8 evaluation: per-language/per-category holdout harness, rolling `prediction_oracle_correlation >= 0.95` ship gate, active-learning loop, weekly report, and real production latency/coverage/OOD measurements.

## Critical blindspots

- **E5 and E11 retirement changes the original signal budget.** Active content set is now E1, E2, E3, E4, E6, E7, E8, E9, E10, E12, E13, E14. Any doc or implementation that still routes through E5 causal or E11 Kepler as active production embedders is stale and should fail closed or be rewritten to active replacements.
- **Artifact integrity is not semantic inference.** Safetensors digest verification prevents supply-chain drift, but it does not prove tokenizer/model compatibility or embedding-quality behavior. The next deep FSV needs to run known real snippets through actual forward paths and read back persisted vectors with expected dimensions, finite values, and stable hashes.
- **Panel-level TCT is not chunk-level TCT.** `GuardRejected` exists at panel level. Per-AST-chunk centroid construction and chunk-localized guard diagnostics are still needed to uncover function/class-specific failures.
- **Synthetic FSV is not the final SWE-bench gate.** Current FSV uses deterministic, known-input fixtures to prove mechanics. The full 300 x 8 SWE-bench Lite corpus and rolling holdout evaluation remain the evidence required for the "review replacement" claim.
- **WSL GPU telemetry is constrained.** NVIDIA documents that `nvidia-smi` exposes a limited feature set under WSL 2, so VRAM checks should keep using the code's CUDA/allocator readbacks and fail closed when telemetry is absent.

## Research anchors used in this hardening pass

- Tree-sitter's parser API exposes byte/row/column node positions and named-node traversal, which supports source-of-truth chunk boundary readback by AST node rather than raw text slices.
- Hugging Face safetensors is designed as a safe tensor format, unlike pickle; model artifact verification should keep requiring pinned safetensors paths and SHA-256 manifests.
- RocksDB column families provide logical partitioning, atomic writes across column families, and consistent DB views; the FSV harnesses therefore inspect the actual CF-backed source of truth rather than trusting return values.
- NVIDIA's CUDA-on-WSL guide documents WSL GPU support and `nvidia-smi` limitations; operational VRAM gates must tolerate limited NVML telemetry only when a separate CUDA/allocator source of truth proves the budget.

---
namespace: discoveries
agent_id: agent-207-inventory-mejepa-phase2-7
timestamp: 2026-05-10T00:00:00Z
status: active
tags: inventory, mejepa, phase2, phase3, phase4, phase5, phase6, phase7, predictor, training, tct, self-healing
---

# ME-JEPA Phase 2-7 crate inventory (read-only, 2026-05-10)

Cross-referenced against `docs/ruvectorfindings/specs/implementation-plan.md` (v1.0.0). Test counts via `grep -rE "#\[test\]|#\[tokio::test\]" src/`. No `unimplemented!()` / `todo!()` / `panic!("not implemented")` found in any of the 7 crates.

## 1. `context-graph-mejepa` (Phase 2 predictor + Phase 4a inference + Phase 5 self-healing)

- **lib.rs:** 74 lines, 24 modules, includes `predictor`, `loss` (vicreg, variance_floor), `oracle_head`, `frozen_target`, `conformal`, `ood`, `compiler`, `gates`, `vram`, `evidence`, `heal/*`.
- **Public API (highlights):** `MeJepaCompiler::{new, compile, compile_with_panel, verify, observe}`; traits `Predictor`, `FrozenTarget`, `OracleHead`, `ConstellationGuard`, `WitnessChainReader`, `MejepaStore`; structs `MeJepaPredictor` (forward / forward_dryrun / trainable_parameters / architecture_summary), `OracleHead`, `FrozenTargetAdapter`, `TctConstellationGuard`, `DeterministicConstellationGuard`, `VicregLossEvidence`, `VramReport`. Loss: `vicreg_loss`, `huber_loss_delta_one`. Conformal: `non_conformity_score`, `calibrate_tau`, `calibrate`, `conformal_set`, `build_outcome_set`, `enumerate_candidate_outcomes`, `shannon_entropy_of_outcome_set`. OOD: `compute_ood_score`, `tune_sigma_squared`, `separation_auc`.
- **Phase 2 PREDICTOR (REQ-PRED-01..06):** SHIPPED. `predictor::MeJepaPredictor` with TransformerLayer, configurable hidden 1024 / 8 heads / `DEFAULT_NUM_LAYERS` etc. `loss::vicreg_loss` shipped. `oracle_head::OracleHead::{predict_logits, predict_probabilities}` shipped. `frozen_target::FrozenTargetAdapter::{encode_target, with_grad_handles, with_noop_labels}` shipped. `vram::check_vram_steady_state` + `VRAM_STEADY_STATE_TARGET_BYTES` shipped. CUDA-only construction enforced (CPU rejected per lib.rs comment).
- **Phase 4a INFERENCE (REQ-INFER-01..08):** SHIPPED in core. `MeJepaCompiler::compile`, `compile_with_panel`, `verify` (compiler.rs lines 195-391) present. Conformal calibration (conformal.rs) and OOD (ood.rs) shipped with `INFER_DEFAULT_*` config defaults. MCP tool `mcp__cgreality__mejepa_verify` (REQ-INFER-05) — NOT verified in this read; would live in `context-graph-mcp` crate.
- **Phase 5 SELF-HEALING (REQ-HEAL-01..10):** SHIPPED. `src/heal/` has 16 modules: `calibration`, `cf` (column families), `distill`, `drift` (DriftSeverity, DriftSample, SeverityTable::classify), `drill`, `errors` (HealError), `ewc` (FisherMatrix, TaskSnapshot, HessianTraceEstimator, BoundaryDetectorState, EwcPlusPlus), `fsv`, `full_retrain`, `integrity`, `lora_refresh`, `pipeline`, `promote` (TriggerReason, ModelHandle, EnsembleHandle, HoldoutEval, PromotionGate::passes — A/B/C), `regulate`, `store`. `MeJepaCompiler::observe()` shipped (compiler.rs L391). `mejepa-heal-drill` bin shipped.
- **Tests:** 66 (`#[test]` / `#[tokio::test]` count in `src/`). **Examples:** 3 — `inference_fsv.rs`, `phase5_self_healing_fsv.rs`, `predictor_fsv.rs`. **Bins:** 2 — `mejepa`, `mejepa-heal-drill`.
- **Fail-closed taxonomy:** `error::MejepaInferError`, `error::PredictorError`, `error::LossError`, `error::NanSource`, `heal::errors::HealError`. heal/mod.rs explicitly states module is fail-closed.

## 2. `context-graph-mejepa-train` (Phase 3 training + UTML)

- **lib.rs:** 32 lines, modules `aux`, `cert`, `checkpoint`, `cli`, `config`, `distill`, `error`, `eval`, `learning_signal`, `lora_wiring`, `loss`, `optim`, `sampler`, `trainer`. `#![deny(deprecated)]`.
- **Public API:** `Trainer::{new, train_one_epoch, evaluate_holdout, run_full_training, current_step, request_abort}`. `BatchSampler::{new, recompute_weights, next_batch, save_rng_state, restore_rng_state}`. `TrainingCertificate::validate_phase3`, `CF_MEJEPA_TRAIN_CERTS`. `LossCoefficients`, `TrainingConfig`. `compute_l_step`, `LearningSignal`, `DeltaP/K/Omega/XiComponents`, `UtmlError`, `SkipReason`. `optim::{build_adamw, current_lr, effective_warmup_steps, nan_inf_finite_check, clip_grads, detect_grad_explode, update_grad_norm_running_mean}`. Loss submodules: `vicreg`, `aux`, `constellation`, `counterfactual`, `predict`. Eval submodules: `epoch_witness`, `holdout`, `regression`, `slicing`. Cert: `chain.rs`, `invariant_scan.rs`, `writer.rs`.
- **Phase 3 TRAINING (REQ-TRAIN-01..06):** SHIPPED. AdamW + cosine LR schedule (REQ-TRAIN-01). `TrainingCertificate` with `CF_MEJEPA_TRAIN_CERTS` (REQ-TRAIN-02). `cert::chain` for hash-chaining (REQ-TRAIN-03). `eval::epoch_witness` (REQ-TRAIN-04). `evaluate_holdout` (REQ-TRAIN-05). `BatchSampler::recompute_weights` with `PatchSimilarityGraph` + sampling weights (REQ-TRAIN-06). Cross-task sampler `sampler/cross_task.rs`. LoRA wiring `lora_wiring.rs`. Distillation `distill/`.
- **UTML (REQ-UTML-01..09):** SHIPPED at the math layer. `learning_signal::compute_l_step` plus `DeltaPComponents`, `DeltaKComponents`, `DeltaOmegaComponents`, `DeltaXiComponents` are present. EMA running mean of grads (REQ-UTML-05) is in `optim::update_grad_norm_running_mean`. Pairwise MI audit job (REQ-UTML-06) — not located in this read; may live elsewhere or pending. State-transfer T detector (REQ-UTML-08) — not located.
- **Tests:** 18. **Examples:** 5 — `train_cert_chain_fsv.rs`, `train_edge_cases_fsv.rs`, `train_full_fsv.rs`, `train_holdout_eval_fsv.rs`, `train_smoke_fsv.rs`. **Bin:** `mejepa-train`.
- **Fail-closed:** `error::TrainerError`, `error::TrainerErrorCode`, `learning_signal::UtmlError` / `UtmlErrorCode`.

## 3. `context-graph-mejepa-tct` (Phase 4b TCT constellation)

- **lib.rs:** 45 lines, fail-closed declared in module doc. Modules: `builder`, `calibrator`, `constellation`, `freshness`, `gtau`, `hinge`, `hinge_types`, `inspect`, `panel_slots`, `rate`, `rate_aggregator`, `shrinkage`, `shrinkage_engine`, `store`, `verdict`.
- **Public API:** `TctConstellation::{try_new, version_id, compute_version_id, validate_integrity, lookup_centroid, panel_centroid, threshold, check_provenance, check_freshness}`. `Centroid::try_new`, `Thresholds::try_new`. `gtau::{gtau_check, gtau_check_chunks, cosine_similarity}`. `verdict::{PredictorOutput, VerdictGuardRejected::try_new}`. `ConstellationBuilder::{new, ingest_corpus_entry, finalize_panel_level, finalize}` + `PanelLevelCentroids`, `CentroidBucket`. Plus `calibrator`, `shrinkage` (Bayesian), `freshness` (max-age policy).
- **Phase 4b TCT (REQ-TCT-01..09):** SHIPPED foundations. Constellation data structure (REQ-TCT-01), centroid construction via `ConstellationBuilder` (REQ-TCT-02), Bayesian shrinkage `shrinkage`/`shrinkage_engine` (REQ-TCT-03), per-embedder thresholds in `Thresholds`/`calibrator` (REQ-TCT-04), `gtau_check` deterministic (REQ-TCT-05), `VerdictGuardRejected` (REQ-TCT-06), constellation max-age `check_freshness` (REQ-TCT-09), `store` for RocksDB CF (REQ-TCT-08). `L_constellation_centroid` (REQ-TCT-07) lives in mejepa-train `loss/constellation.rs`.
- **Tests:** 8 (separate `tests.rs` module). **Examples:** 1 — `tct_fsv.rs`.
- **Fail-closed:** `error::TctError`. Module-level doc enumerates fail-closed conditions.

## 4. `context-graph-witness` (foundation, ed25519, certs)

- **lib.rs:** 225 lines. Public API: `WitnessEntry::{new, to_bytes, from_bytes, chain_hash}`, `verify_chain_bytes`, `verify_chain_bytes_with_type_validator`, `shake256_32`, `hex_hash`, `ChainVerification`. From `cert.rs`: `Stage0Certificate::{issue, verify}`, `ReadoutCertificate::{issue, verify_with_tree, verify_trajectory}`, `MerkleTree`, `MerkleProof`, `MerkleLeaf`, `ArtifactRef`, `compute_artifact_manifest_hash`, `compute_stage0_binding_hash`, `compute_readout_binding_hash`. Feature-gated `ed25519`: `WitnessKeypair::{from_seed, public_key_bytes, sign_segment}`, `verify_segment`, `verify_segment_with_expected_pubkey`, `digest_for_segment`, `DOMAIN_TAG`, `SIGNATURE_CODEC_SIZE`.
- **Tests:** 45. **Examples:** 2 — `cert_fsv.rs`, `ed25519_fsv.rs`.
- **Fail-closed:** `WitnessError` (4 variants with stable `code()` strings: `WITNESS_ENTRY_LENGTH_INVALID`, `WITNESS_CHAIN_LENGTH_INVALID`, `WITNESS_PREV_HASH_MISMATCH`, `WITNESS_TYPE_REJECTED`); `CertError`; `SignError` with `code()`.

## 5. `context-graph-solver` (mmr, conformal, causal_mask, granger, plus cg, csr, pagerank)

- **lib.rs:** 31 lines. Public API: `apply_mask_to_scores`, `build_causal_mask`, `retrocausal_safety_check`, `softmax_row`, `MaskStrategy` (Strict / TimeWindow{w} / CausalWithSelf); `ConjugateGradientSolver` + config/report; `ConformalPredictor::{new, calibrate_distances, calibrate_with_ranks}` + `NonconformityMeasure`, `PredictionSet`; `CsrMatrix`, `MatrixKind`; `granger_test`, `f_distribution_upper_tail`, `regularized_incomplete_beta`, `GrangerConfig`, `GrangerReport`; `mmr_select`, `mmr_select_with_matrix`, `MmrConfig`, `MmrSelection`; `ForwardPushSolver` (PageRank).
- **Tests:** 58 (incl. `granger_tests.rs`, `causal_mask_tests.rs`). **Examples:** 4 — `causal_mask_fsv.rs`, `granger_fsv.rs`, `solver_fsv.rs`, `tier1_fsv.rs`.
- **Fail-closed:** `error::SolverError` / `SolverResult`.

## 6. `context-graph-mincut`

- **lib.rs:** 333 lines, single-file Stoer-Wagner implementation. Public API: `stoer_wagner(n, edges, config)`, `weights_from_edges`, `CutPartition`, `StoerWagnerConfig`, `MincutError` (with `code()`). REQ-CORPUS-09 / REQ-HYG-02 = SHIPPED.
- **Tests:** 18 (`tests.rs`). **Examples:** 1 — `mincut_fsv.rs`.
- **Fail-closed:** `MincutError` with stable codes.

## 7. `context-graph-tier-compression`

- **lib.rs:** 516 lines. Public API: `BitWidth` (8/7/5/3 + `bits()`, `levels()`, `all()`), `CompressedBlob` + `payload_len`/`serialized_len`, `encode`, `decode`, `serialize`, `deserialize`, `packed_len`, `max_reconstruction_error`. Full on-disk format with magic + version header. REQ-HYG-01 = tier crate SHIPPED; integration into ME-JEPA storage tiering = pending.
- **Tests:** 24 (`tests.rs`). **Examples:** 1 — `tier_compression_fsv.rs`.
- **Fail-closed:** `TierCompressionError` with `code()`.

## Phase-by-phase shipped vs pending matrix

| Phase | REQ-IDs | Crate evidence | Status |
|---|---|---|---|
| 2 PREDICTOR | REQ-PRED-01..06 | predictor.rs, loss/vicreg.rs, oracle_head.rs, frozen_target.rs, vram.rs | shipped |
| 3 TRAINING | REQ-TRAIN-01..06 | trainer/mod.rs, optim/mod.rs, cert/{mod,chain,writer}.rs, sampler/{mod,cross_task}.rs, eval/{holdout,epoch_witness}.rs | shipped |
| 3 UTML-FORMULA | REQ-UTML-01..09 | learning_signal.rs (compute_l_step + DeltaP/K/Omega/Xi components) | mostly shipped; UTML-06 pairwise MI audit + UTML-08 state-transfer T detector not located |
| 4a INFERENCE | REQ-INFER-01..08 | compiler.rs (compile/verify/observe), conformal.rs, ood.rs | shipped at crate level; REQ-INFER-05 MCP tool not in this crate (lives in context-graph-mcp) |
| 4b TCT | REQ-TCT-01..09 | constellation.rs, builder.rs, gtau.rs, verdict.rs, calibrator.rs, shrinkage*.rs, freshness.rs, store.rs (+ loss/constellation.rs in mejepa-train for REQ-TCT-07) | shipped |
| 5 SELF-HEALING | REQ-HEAL-01..10 | heal/{drift,ewc,promote,calibration,integrity,lora_refresh,full_retrain,distill,pipeline,drill,regulate,store,cf,fsv}.rs + compiler.observe() + mejepa-heal-drill bin | shipped |
| 6 HYGIENE | REQ-HYG-01..07 | tier-compression crate (REQ-HYG-01), mincut crate (REQ-HYG-02) | partial — REQ-HYG-03..07 (Merkle compression of old chains, nightly GC, quota enforcement, me_jepa::ops MCP tools, 10K-entry stress FSV) NOT FOUND |
| 7 SHIFT-SUBSCRIBER | REQ-SHIFT-01..09 | grep for `shift_log`/`shift-log`/`cgreality-shift-log`/`Subscriber` across mejepa, mejepa-train, mejepa-tct = ZERO hits | NOT SHIPPED — no Tokio task tailing the shift log, no CF_MEJEPA_SHIFT_WATERMARK in this read, no mejepa_predict_latest / mejepa_observe_shift / mejepa_subscriber_status / mejepa_capture_audit MCP tools in these crates |

## Summary numbers

- Total `#[test]`/`#[tokio::test]` across the 7 crates: 66+18+8+45+58+18+24 = **237**.
- Total examples (FSV harnesses): **17** across the 7 crates.
- Total bins: **3** (`mejepa`, `mejepa-heal-drill`, `mejepa-train`).
- `unimplemented!()` / `todo!()` / `panic!("not implemented")` across `src/`: **0** hits.
- Stable `code()` error taxonomy: present on WitnessError, SignError, MincutError, TierCompressionError; thiserror enums also on TctError, MejepaInferError, PredictorError, LossError, HealError, TrainerError, UtmlError, SolverError. Fail-closed posture is consistent across the 7 crates.

## Gaps versus implementation-plan.md

1. Phase 7 SHIFT-SUBSCRIBER: not implemented in any of the 7 crates examined. REQ-SHIFT-01..09 all pending.
2. Phase 6 HYGIENE beyond crate building blocks: tier-compression and mincut crates exist but the orchestration layer (REQ-HYG-03..07: Merkle compression of old segments, nightly GC scheduler, 115 GB quota + emergency eviction, `me_jepa::ops::*` MCP tools, 10K-entry stress FSV) was not found in these crates.
3. Phase 4a MCP surface (REQ-INFER-05 `mcp__cgreality__mejepa_verify`): not in the mejepa crate; would live in `context-graph-mcp` (out of scope for this inventory).
4. Phase 3 UTML-06 (pairwise_mi_matrix.csv audit job) and UTML-08 (state-transfer T detector): not located in mejepa-train.
5. implementation-plan.md still marks Phases 2-7 as "deferred" in the per-phase table, but the `src/` evidence shows Phases 2, 3, 4a, 4b, 5 are largely shipped. The plan's status column is stale relative to the codebase.

## Spec staleness flag

`docs/ruvectorfindings/specs/implementation-plan.md` Phase Summary table (lines 19-26) declares Phases 2-8 as **"deferred"**. Concrete evidence in this inventory shows Phase 2 / Phase 3 / Phase 4a / Phase 4b / Phase 5 are SHIPPED at the crate level. The plan needs a status refresh; recommend updating to "shipped (crate-level) / pending (MCP surface, Phase 6 orchestration, Phase 7)".

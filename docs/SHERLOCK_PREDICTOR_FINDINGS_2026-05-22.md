# Sherlock Predictor Findings — 2026-05-22

Investigator: Sherlock (Claude Opus 4.7) under FDD/LSU/Cold-Read discipline.
Scope: predictor pipeline + calibration chain in `context-graph-mejepa`, `context-graph-mejepa-train`, `context-graph-mejepa-tct`.
Cardinal rule applied: **guilty until proven innocent** — return values are claims, not verdicts.

## Post-audit correction — 2026-05-23

This report correctly identified that the public Rust `Trainer` is certificate-only and not ship-gate-countable. It overreached where it inferred that the latest aiwonder ship-gate run necessarily used fresh random `VarMap` weights.

Direct aiwonder readback later showed that `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/ship-gate-fsv.json` loaded `/zfs/archive/contextgraph/models/task-py-g-127-trained-checkpoint-export-fsv/best.manifest.json` with `optimizer_steps=64` and `weight_delta_observed=true`. The defect remains P0 under #683 because that checkpoint/export path is out-of-band from the public `Trainer` contract and lacks countability fields (`trained_predictor`, `predictor_parameter_update_count`, `checkpoint_readback_verified`, `ship_gate_countable_training`) in the certificate lineage.

Read the original findings below as evidence for #683/#695, but use #712 and #683 for the corrected current interpretation.

## Summary

- **The predictor is not a trained predictor.** `Trainer::run_full_training` is permanently in `DIAGNOSTIC_CERTIFICATE_ONLY_TRAINING_MODE`: `trained_predictor: false`, `predictor_parameter_update_count: 0`, `ship_gate_countable_training: false` — hard-coded. The only "training" output is UTML certificates. No production caller of `build_slot_preserving_cuda_compiler` sets `trained_checkpoint_manifest_path`, so the `MeJepaPredictor` runs on freshly-initialized `VarMap` weights every time. Source: `crates/context-graph-mejepa-train/src/trainer/mod.rs:31-34, 409-422`; `crates/context-graph-mejepa/src/cli.rs:307-314`.

- **The 0.7942 measurement is not the predictor's score.** The score on which the gate is being measured is `predicted_oracle_pass`, which is itself a derived field, but the *gate-eligible* number `0.7942190170` is the **rank-lift transformation** — a separate **category-local RBF exemplar-similarity model** over `code_state_constellations.jsonl` slot-feature means + hashed live-label counts. It is fitted/selected on the 240-row calibration split and then evaluated on the 240-row holdout split. The transformation moves Pearson **0.4843 → 0.6917 (rank-safe) → 0.7942 (rank-lift)** — a +0.31 lift from rank/exemplar surgery, not from model improvement. Monotonic recalibration (Platt/isotonic) preserves AUC; this transform does not, which is why it can lift correlation but cannot lift Brier or ECE materially (Brier 0.092, ECE 0.024 sit roughly where the raw predictor would put them). It is rank-eligibility magnification, **not** model improvement.

- **The two documented doctrine violations (#618 flat-buffer OOD, #619 flat inverse projection) are real and inarguable** — they are present at `compiler.rs:502` and `predictor/mod.rs:136-147`. They both deflate the legitimate signal but only modestly — they touch the OOD score (used as one of several gate inputs) and the inverse head (zero contribution today because `lambda_inv=0.0`). Repairing them is required for doctrine compliance but is unlikely to move corr by more than ~0.02 each.

- **The 0.953 active-subset correlation is a coverage artifact, not a model artifact.** The abstain policy in `python_calibration_active_gate.py:322-378` selects, per cell, the `coverage` (fraction of rows kept) and `margin_threshold = min(|score-0.5|)` that maximizes cell-gate pass on the calibration split, then applies it to holdout. Active rows are those with high `|score-0.5|`. Of course such rows correlate better with labels — they are the *easy* rows by construction. The 35 abstained rows are not "additional model error"; they are explicitly excluded from the metric. This means the diagnostic 0.953 is **not a hidden ship-gate-passing predictor — it is selection bias**.

- **The trainer cannot inject mistakes into the predictor.** `mistake_consumer.rs` does not exist. The shipped `online_head_state.rs` is a key-local LUT keyed on **exact panel-signature hash**: `protection_mode = ExactPanelKeyIsolation`, `claims_fisher_ewc_protection: false`. Real production panels almost never collide on signature hash, so the LUT is empty in practice. There is no EWC++ Fisher snapshot path actually consuming `CF_MEJEPA_MISTAKE_LOG` to update predictor weights.

- **`per_slot_ood_reasons` does not measure per-slot OOD.** It is built 1:1 from `guard_violations` at `compiler.rs:2267-2296`. Same data, different name. The advertised #389 "per-slot OOD calibration + OOD" is structurally there, but the live serving path never computes per-slot residuals — it copies Gτ violations into a differently-named field.

- **`DeterministicOracleHead` is a 4-scenario hardcoded LUT** keyed on `round(EDiff[0])` at `compiler.rs:2423-2443`. Used by `build_fixture_deterministic_compiler` (test fixtures, candidate ranking tests, latent search tests). Not used in the slot-preserving CUDA production path. But `compiler.rs:1463` (project ingest) emits hardcoded `0.50/0.25` predictions when failure_modes is empty/non-empty — that IS production for the project-ingest surface.

- **Q4 freeze IS correctly enforced** in `verdict_assembly.rs:38-42` (`security_concern_count` zeroed) and `q4_trust_gate.rs:304-323` (every head's readiness fields are forced false). Verified clean.

## Critical jumps

Each ranked by **honest expected lift** with the recognition that the 0.7942 baseline is itself an artifact of rank-lift transformation, so "lift" must be measured against rank-lift holdout corr, not raw.

### Jump #1 — Wire a real predictor checkpoint (expected lift: 0.05-0.15, complexity: HIGH, ECE-safe: likely)

**Hypothesis.** The current `MeJepaPredictor` has random VarMap weights because no `trained_checkpoint_manifest_path` is set in production and `Trainer::run_full_training` returns `trained_predictor: false`. The whole stack is using rank-lift exemplar similarity (which has nothing to do with the predictor) to fabricate a number that looks like a model evaluation.

**Evidence.**
- `crates/context-graph-mejepa-train/src/trainer/mod.rs:31-34` defines `DIAGNOSTIC_CERTIFICATE_ONLY_TRAINING_MODE`.
- `crates/context-graph-mejepa-train/src/trainer/mod.rs:409-422` hard-codes `trained_predictor: false`, `predictor_parameter_update_count: 0`, `ship_gate_countable_training: false` in the result.
- `crates/context-graph-mejepa-train/src/trainer/mod.rs:253-261` includes `("predictor_parameter_update_count".to_string(), 0.0)` and `("ship_gate_countable_training".to_string(), 0.0)` in every certificate.
- `crates/context-graph-mejepa/src/cli.rs:307-314` `load_verified_trained_predictor_checkpoint` only runs when `config.trained_checkpoint_manifest_path` is `Some` — and nothing in the workspace ever sets it.
- `predictor/mod.rs:108-130` constructs the predictor from a fresh `VarMap` — all weights are normal-distribution random.
- The 391-shard packed forward cache IS real (#666 collapse on e2/e3/e4/e8 notwithstanding), but the cache feeds the predictor's *inputs*; the *parameters* are random.

**Expected magnitude.** Hard to estimate honestly — a properly trained predictor on the real input panels could plausibly take rank-safe corr from 0.6917 to 0.85+, which would let rank-lift exceed 0.95 even without the exemplar trick. But it could also fail if the substrate (e2/e3/e4/e8 collapse, #670) is too compromised. Lower-bound 0.05; upper-bound 0.15.

**Risk of ECE regression.** Low — a real trained predictor with proper temperature scaling can land ECE below the current 0.024.

**FSV that would prove/refute.** Implement an actual training loop (apply gradient updates to `VarMap`); train one epoch on the 240 calibration rows + 1,920 train rows; reload via `load_verified_trained_predictor_checkpoint`; re-measure ship-gate corr **without** the rank-lift transformation. If raw Pearson exceeds 0.85, this is the gate-mover.

### Jump #2 — Replace rank-lift exemplar gate metric with a real model-output calibration (expected lift: -0.10 to +0.05, complexity: MEDIUM, ECE-safe: yes if isotonic/Platt)

**Hypothesis.** Rank-lift is a Goodhart magnifier. It's not measuring the model; it's measuring exemplar similarity. Removing it from the gate forces the measurement to be model-honest and exposes the actual gap. The honest baseline is rank-safe at 0.6917, and the gap to ship is **0.26**, not 0.16. This is bad news but it's true news.

**Evidence.**
- Doc trail (`07 §1`, `08 §3.1`) describes rank-lift as: "category-local RBF exemplar scoring over real `code_state_constellations.jsonl` slot-feature means + hashed live label counts while excluding target-side oracle/docker/test labels; #536 added calibration-selected train-only hard-negative weighting (`tau=20.0`, `fail_boost=1.5`)."
- The transform yields +0.31 corr lift over raw with **no** improvement (in fact, ECE was 0.0338 → 0.0240 only because #536 was selected on calibration ECE, not because rank-lift is calibration-improving in general).
- Sources: rank-lift code itself is no longer in the repo — it lived in deleted Rust FSV harness `swebench_300x8_ship_gate_fsv` and is referenced only via JSON file inputs to `python_calibration_active_gate.py:109-117`.

**Expected magnitude.** Removing rank-lift will REDUCE the measured number. But it will give you the real one to optimize. After that, isotonic regression on the model's `predicted_oracle_pass` (legitimate, AUC-preserving) might add ~0.02 corr at best, and reduce ECE materially.

**Risk of ECE regression.** No — switching from rank-lift to isotonic should improve ECE.

**FSV that would prove/refute.** Run `python_calibration_active_gate` on the same source rows with `--policy isotonic` (need to add the option) instead of `empirical_bayes_beta`; compare ECE/Brier/corr. The honest baseline is what matters.

### Jump #3 — Repair #618 (flat OOD) with per-slot residual aggregation (expected lift: 0.01-0.03, complexity: LOW, ECE-safe: yes)

**Hypothesis.** `compute_ood_score(predicted.data(), target.data(), sigma_squared)` at `compiler.rs:502` accumulates `Σ_i (predicted[i] - target[i])²` over all 5,120 dims, treating E_AST dim 0 (384-d tree-sitter latent) as comparable to E_CFG dim 0 (256-d) etc. This is the doctrine §1.5ter violation. Real-magnitude residuals in big-dim slots drown small-dim slots. Replace with per-slot z-score conjunction (max-of-z-scores) so the OOD verdict reflects the actually-bad slot.

**Evidence.**
- `crates/context-graph-mejepa/src/ood.rs:3-30` defines `compute_ood_score` taking `&[f32]` — slot identity lost at call boundary.
- `crates/context-graph-mejepa/src/compiler.rs:502` passes `predicted.data()` / `target.data()` — the contiguous packed buffer.
- `crates/context-graph-mejepa/src/verdict_assembly.rs:75-88` checks `ood_score >= ood_threshold` BEFORE `guard_violation_count > 0`, so a noisy aggregate masks specific slot violations as generic OOD.
- Verdict-assembly contract test at `verdict_assembly.rs:236-257` confirms ordering: GuardRejected fires *before* Pass via `safety_constraint_violation_count`, but OOD fires *before* GuardRejected from guard_violations.

**Expected magnitude.** Modest: OOD is one input among many, and only fires above threshold (default 0.9). The current verdict is dominated by `oracle_pass_confidence ≥ pass_threshold` (0.8) and `predicted_failed_test_count == 0`. But abstain rows that fall into the abstain pool because of dim-wise OOD noise would become Fail-correctly-or-Pass-correctly verdicts.

**Risk of ECE regression.** Low.

**FSV that would prove/refute.** Adversarial test: 14 zero-residual slots + 1 high-residual slot → should emit `GuardRejected{slot: EX}` not generic `OutOfDistribution`. Compare ship-gate corr before/after on the 240×8 holdout.

### Jump #4 — Actually use the abstain pool (expected lift: 0.03-0.08, complexity: MEDIUM, ECE-safe: yes)

**Hypothesis.** The 35 abstained rows are not random — they are the *hard* rows. If you can build a second-tier predictor specifically trained on confusion-set rows (low `|score-0.5|`, weak cells), you can convert abstains into correct verdicts. This isn't "improving the predictor on hard rows" — it's recognizing that the score distribution has two regimes (confident pass/fail vs. confused) and treating them differently.

**Evidence.**
- Active subset 205/240 corr is 0.9526, holdout corr 0.7942. The delta IS the abstain pool. Per `08 §3` weak cells: `delete_test_call (0.69)`, `off_by_one (0.87)`, `subtle_flip (0.80)`.
- `delete_test_call` per #615 has support of 2-fail/238-pass train, 0-fail/30-pass calibration — it's **substrate-limited**, not predictor-limited. Repairing it requires the #615 Docker oracle batch over 1,131 candidates.
- `off_by_one` and `subtle_flip` may be predictor-limited; both can be repaired by a confusion-set fine-tune if the predictor is actually trainable (depends on Jump #1).

**Expected magnitude.** If you can convert just half the 35 abstained rows to correct verdicts, that flips ~17 holdout labels. With 240 holdout rows, that's potentially +0.05-0.08 corr.

**Risk of ECE regression.** Low if confidence on the second-tier predictor is well-calibrated.

**FSV that would prove/refute.** Materialize the abstain pool as a separate holdout; fit a per-cell calibrator (or a separate small model) on the calibration split's abstained rows; evaluate.

### Jump #5 — Repair `delete_test_call` substrate via #615 regeneration (expected lift: 0.02-0.05, complexity: HIGH — depends on aiwonder Docker batch, ECE-safe: yes)

**Hypothesis.** Substrate, not model. Per `07 §1.5` and #615, `delete_test_call` has only 1 calibration-failure-bearing row out of 30. The cell is structurally underpowered. Until 1,131 deterministic candidates are oracle-verified and rebuilt into AST → forward cache → labels → #392 features, no predictor can ranking-learn this cell.

**Evidence.**
- `07 §1.5` documents `TASK_PY_G_135C_DELETE_TEST_CALL_ZERO_CALIBRATION_FAILURE_SUPPORT`.
- #615 plan exists at `/zfs/archive/contextgraph/state/regeneration-plans/task-py-g-135c-delete-test-call-regeneration-v1/`.

**Expected magnitude.** This cell's holdout active-cell corr `0.6948` weighs in the global rank-lift. Per linear arithmetic across 8 cells, repairing this one cell to 0.95 could lift global by ~0.03.

**Risk of ECE regression.** None.

**FSV that would prove/refute.** Complete #615 → rebuild substrate → rerun the ship-gate FSV.

### Jump #6 — Replace #619 flat inverse projection with slot-preserving per-slot encoders (expected lift: 0.0-0.02 IF λ_inv promoted, complexity: MEDIUM)

**Hypothesis.** Pure doctrine repair. `predictor/mod.rs:136` `inverse_target_proj = linear(PANEL_DIM, hidden_dim)` and `:147` `inverse_input_proj = linear(hidden_dim, PANEL_DIM)` flatten the panel exactly as #618 does, before the per-slot encoders fire.

**Evidence.** As cited in `predictor/mod.rs:136, 147, 395-459`.

**Expected magnitude.** Today **zero** because `lambda_inv = 0.0` (selected by EK-012 ablation: `lambda_inv=0.0` rank-safe `0.6704` beat positives). Future-conditional: only matters if the inverse head is re-promoted. If the inverse head is doctrine-compliant and recovers meaningful information, `lambda_inv > 0` could be re-attempted — but this is speculative.

**FSV that would prove/refute.** Refactor + run EK-012 again with new slot-preserving inverse head.

## Undocumented brokenness

### B-1 (P0): Trainer never updates predictor parameters
- **Where:** `crates/context-graph-mejepa-train/src/trainer/mod.rs:31-34, 253-261, 409-422`.
- **Looks healthy:** API surface has `run_full_training`, `train_one_epoch`, `evaluate_holdout`. The trainer emits training certificates. Tests assert certificate fields.
- **Isn't:** Every result hard-codes `trained_predictor: false`, `predictor_parameter_update_count: 0`. Every loss component map includes `("predictor_parameter_update_count".to_string(), 0.0)`. There is no `optimizer.step()`, no gradient computation against predictor parameters. The string `DIAGNOSTIC_CERTIFICATE_ONLY` is doctrine-honest about it, but the trainer's *name* and the field `final_holdout_agreement: Option<f32>` together imply training happens.
- **Severity:** P0 — this is the root cause behind the entire gate-blocker. If the predictor is never trained, the predictor's score is random.
- **Reproduction:** read the cited line ranges. Note that `predictor: MeJepaPredictor` is never passed into `Trainer::new`. The trainer holds no reference to a predictor at all (`pub struct Trainer { aux_heads, sampler, cert_writer, holdout_evaluator, ... }` — no predictor field). `train_one_epoch_inner` calls `compute_l_step` to produce learning-signal values used in certificates, then `cert_writer.emit(&mut cert)`. That's it.
- **Suggested fix:** the trainer must accept a `MeJepaPredictor`, compute a real loss (e.g. `BCEWithLogitsLoss` over `oracle_head` outputs vs `actual_oracle_pass`), backprop, step optimizer. Then a non-`None` `trained_checkpoint_manifest_path` must be wired through production config. This is essentially "build the trainer". The doctrine note acknowledges the issue but no one has fixed it.

### B-2 (P0): Production checkpoint manifest is never set
- **Where:** `crates/context-graph-mejepa/src/config.rs:169, 193, 266-275`; `crates/context-graph-mejepa/src/cli.rs:307-314`.
- **Looks healthy:** Config field exists; validator runs; loader exists.
- **Isn't:** No production caller sets it. Grep across the entire workspace for `trained_checkpoint_manifest_path: Some(` returns zero hits. Result: `MeJepaPredictor` always boots with random VarMap weights.
- **Severity:** P0.
- **Suggested fix:** parent of B-1.

### B-3 (P0): `per_slot_ood_reasons` is a relabeled `guard_violations` slice, not per-slot OOD
- **Where:** `crates/context-graph-mejepa/src/compiler.rs:2267-2296`.
- **Looks healthy:** `RealityPrediction.per_slot_ood_reasons` is a documented surface, plumbed into `CF_MEJEPA_LIVE_PREDICTIONS`, referenced in doc 05 §3 and the #389 RESOLVED issue.
- **Isn't:** It's built by `per_slot_ood_reasons_from_guard_violations`, which maps each `GuardViolation` to a `PerSlotOodReason { reason: GtauViolation, observed_score: violation.cosine, threshold: violation.threshold_tau_m, margin: violation.deficit, ... }`. The per-slot residual L2/z-score that doc 05 §3 advertises is never computed in the serving path. The signal in this field is identical to `guard_violations`; the name is misleading.
- **Severity:** P1 (downstream consumers — `mejepa_reality_impact`, weekly dashboard — may treat this as independent evidence; it is not).
- **Suggested fix:** either (a) compute real per-slot residual z-scores and populate this field from them; or (b) rename to `guard_violation_audit` and document the structural duplication.

### B-4 (P1): `DeterministicOracleHead` is a 4-scenario LUT keyed on `EDiff[0]`
- **Where:** `crates/context-graph-mejepa/src/compiler.rs:2421-2443`.
- **Looks healthy:** Implements `OracleHead` trait, used by `build_fixture_deterministic_compiler`, present in tests, referenced by 11+ tests.
- **Isn't:** Returns hardcoded `(0.20, 0.95, 0.95)` based on `round(EDiff[0]) ∈ {2, 3, 4, _}`. Used by `candidate_ranking.rs`, `latent_search.rs`, `eval/fixtures.rs`, `bin/mejepa.rs:564`. The `mejepa` CLI binary thus emits a known-fake oracle for every prediction. If any production tooling reads from this CLI's output, it's reading the LUT.
- **Severity:** P1 (not in the slot-preserving CUDA path; but `bin/mejepa.rs:564` is technically reachable in operator workflows).
- **Suggested fix:** rename to `FixtureOracleHead`; ensure no operator-facing CLI uses it; add a runtime guard that emits `MEJEPA_FIXTURE_ORACLE_USED_IN_PRODUCTION_PATH` if invoked outside `cfg(test)` or fixture-flagged config.

### B-5 (P1): `project_ingest.rs:1463` emits hardcoded `predicted_oracle_pass` for project ingest
- **Where:** `crates/context-graph-mejepa/src/project_ingest.rs:1463`.
- **Looks healthy:** Function returns `RealityPrediction`, populated builder pattern.
- **Isn't:** `.predicted_oracle_pass(if failure_modes.is_empty() { 0.50 } else { 0.25 })` — this is a fixed binary decision based on whether *any* failure mode was inferred. No model involvement. Also degraded_status is true, so verdict will be `Abstain`. Still, the value 0.50 / 0.25 will be persisted to `CF_MEJEPA_LIVE_PREDICTIONS` for project-ingest rows and consumed by downstream tools that don't check `degraded_status`.
- **Severity:** P2 (degraded_status guards most consumers).
- **Suggested fix:** explicit `None` for `predicted_oracle_pass` when degraded, with consumers required to honor `degraded_status` before reading the value.

### B-6 (P1): Online head LUT is empty in practice
- **Where:** `crates/context-graph-mejepa-train/src/online_head_state.rs:270-294`.
- **Looks healthy:** #406 / #523 RESOLVED. MCP tools `mejepa_record_mistake`, `mejepa_mistake_history`, `mejepa_mistake_loop_status` shipped. Has the structure of a continual-learning system.
- **Isn't:** `protection_mode: ExactPanelKeyIsolation` + `panel_signature_hash` keying means the LUT only corrects when an EXACT panel signature recurs. Real production panels almost never collide on signature (the signature is SHA over panel bytes, panels are continuous-valued; collision probability ≈ 0). The LUT is structurally empty in any real workload, no matter how many mistakes are recorded.
- **Severity:** P1 — the entire mistake loop is theatre. Doctrine claims `mistake_repeat_rate ≤ 0.05` is a ship-gate metric, but it can never converge because the correction mechanism is never invoked outside of the same-input replay.
- **Suggested fix:** the protection mode needs to be `FisherEwc` with a real Fisher snapshot updated from gradients, not panel-signature exact match. Plus a generalization layer (k-NN over panel embeddings, or per-cell prior).

### B-7 (P1): `bin/mejepa.rs` uses `panel_batch_tensor` synthetic phase modulation
- **Where:** `crates/context-graph-mejepa/src/panel_source.rs:41-77`, called from `crates/context-graph-mejepa/src/bin/mejepa.rs:626, 633, 640`.
- **Looks healthy:** Takes a real `Panel`, returns a `Tensor` of shape `(batch, PANEL_DIM)`. Validates finiteness.
- **Isn't:** The batch dimension is generated by applying `base.mul_add(1.0 + modifier, modifier * 0.01)` where modifier depends on row index. So a "batch" of N is N deterministic perturbations of ONE input panel. Used by the `mejepa` CLI binary, which still ships and runs as the operator-facing predictor harness on the `mejepa` workspace bin. If anyone uses this binary to "validate" the predictor on a batch, the batch is fake.
- **Severity:** P2 (limited blast radius).
- **Suggested fix:** delete `panel_batch_tensor` and `PanelBatchView` from production; replace usages in `bin/mejepa.rs` with a real multi-panel loader; OR rename the function `synthetic_panel_perturbation` and gate it behind `cfg(test)`.

### B-8 (P2): `ChunkRow.raw` dead-code in production cache runner
- **Where:** `crates/context-graph-mejepa-embedders/examples/python_forward_cache_readback.rs:59`.
- **Looks healthy:** The `ChunkRow` struct is fully populated; `Clone, Debug` derived; `raw: Value` is parsed from the JSONL line and passed forward in `chunk_from_value`.
- **Isn't:** `cargo check` confirms `field 'raw' is never read`. The compiler explicitly says "`#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default". This is the production forward-cache runner used to materialize 391 shards on aiwonder. The `raw` field exists "just in case" but no consumer reads it. Minor — but it's a sign the developer wasn't certain what fields would be needed.
- **Severity:** P3.
- **Suggested fix:** delete the field; if specific JSON keys are needed downstream they should be plumbed explicitly.

### B-9 (P1): Conformal interval width uses fixed threshold 0.8
- **Where:** `crates/context-graph-mejepa/src/compiler.rs:569`.
- **Looks healthy:** `interval_width_threshold: 0.8` looks like a tunable.
- **Isn't:** Hardcoded constant in the verdict-assembly input. The conformal interval is computed as `[confidence ± tau]` then clamped to [0,1]. When tau is large (cold start: `coverage_target=1.0, confidence_interval=(0.0, 1.0)` — see `conformal.rs:30`), the interval will always have width 1.0 and always trigger `OutOfDistribution`. Per `conformal.rs:26-43`, oracle outcomes <100 yield `confidence_interval=(0.0, 1.0)`. Combined with `interval_width_threshold = 0.8`, EVERY cold-start prediction is `OutOfDistribution`.
- **Severity:** P1 — this means new repos / new mutation cells default to OOD until 100+ oracle outcomes accumulate, regardless of model quality. The cold-start policy *forces* abstain on the binary surface.
- **Suggested fix:** make `interval_width_threshold` a calibrated per-cell value (or remove the hard cap and rely on per-cell ECE).

### B-10 (P1): `compute_train_health` reads recent training certs that don't reflect real training
- **Where:** `crates/context-graph-mejepa/src/compiler.rs:507-515`.
- **Looks healthy:** Inference flow reads training certs, computes `train_health`, uses it as a confidence multiplier.
- **Isn't:** Training certs from `DIAGNOSTIC_CERTIFICATE_ONLY_TRAINING_MODE` carry diagnostic learning-signal values (`signal.delta_p = hardness.min(1.0)`, `delta_k.cos_align: 0.85 or 0.65 hardcoded`, `delta_omega.effective_plasticity: 0.9 hardcoded`, etc. — see `trainer/mod.rs:202-234`). These values are deterministic functions of the batch index, not measurements of real predictor learning. `calibrated_confidence_multiplier(train_health.delta_omega_mean, train_health.delta_xi_mean)` thus reads from a deterministic pseudo-source.
- **Severity:** P1.
- **Suggested fix:** make train health gated on `predictor_parameter_update_count > 0`; fail closed if no real training has occurred.

### B-11 (P2): `assemble_verdict_with_evidence` ordering masks slot-specific violations
- **Where:** `crates/context-graph-mejepa/src/verdict_assembly.rs:69-88`.
- **Looks healthy:** `safety_constraint_violation_count > 0` → `GuardRejected`. Then `ood_score >= ood_threshold OR interval_width > threshold` → `OutOfDistribution`. Then `guard_violation_count > 0` → `GuardRejected`.
- **Isn't:** Per #618, the ood_score is computed on a flat 5,120-byte buffer, so it integrates noise across slot identities. A panel with one bad slot (the slot that owns most of the residual) emits `OutOfDistribution` (generic), not `GuardRejected` (slot-specific) — even though the per-slot Gτ check would have caught that slot. The slot identity is then lost in the verdict, and the operator sees "the panel is OOD" instead of "the EReasoning slot failed Gτ".
- **Severity:** P2 (downstream consumers can still recover slot identity from `per_slot_ood_reasons`, but those are derived from `guard_violations` only — see B-3).
- **Suggested fix:** swap lines 75 and 83 — check guard violations before generic OOD.

### B-12 (P1): The 0.953 active-subset diagnostic is selection-biased
- **Where:** `scripts/swebench_lite_ops/python_calibration_active_gate.py:322-378, 487-516`.
- **Looks healthy:** Looks like a calibrated abstain policy. Computes per-cell `margin_threshold = min(|score - 0.5|)` over the selected ordered subset on calibration data only, then applies to holdout. Avoids holdout leakage.
- **Isn't:** The margin threshold is FIT to maximize cell-gate pass on the calibration split. The "active" rows on holdout are those with `|score - 0.5| ≥ margin_threshold` — i.e., the rows where the predictor was most confident. Of course those rows correlate well with labels; they are the rows the predictor wasn't confused about. The 35 abstained rows are the confused ones. Reporting 0.953 on the easy rows is essentially "the predictor is good when it isn't confused." That's tautological.
- **Severity:** P1. The active-subset metric is being treated in doc text as a "promising diagnostic" but it cannot logically be a gate metric without including the abstain pool somewhere.
- **Suggested fix:** require any active-subset gate to also bound the abstain rate (e.g., active corr ≥ 0.95 AND abstain_rate ≤ 0.10). Currently abstain_rate is 35/240 = 14.6%.

### B-13 (P1): `Trainer.evaluate_holdout` errors out — orphan API
- **Where:** `crates/context-graph-mejepa-train/src/trainer/mod.rs:385-399`.
- **Looks healthy:** API surface exists for evaluate_holdout.
- **Isn't:** Always returns `Err` with `"Trainer::evaluate_holdout has no predictor/oracle inputs; call HoldoutEvaluator::evaluate ..."`. This is a known artifact of #596 closure (Sherlock audit) — the trainer was self-fabricating holdout reports; the fix was to make it fail closed. But the API is still exposed and any caller will fail. There's no compile-time guard that prevents calling it.
- **Severity:** P2 — the function name implies it works, but it doesn't.
- **Suggested fix:** mark `#[deprecated]` with link to `HoldoutEvaluator::evaluate`, or remove entirely.

### B-14 (P1): Mistake loop has zero EWC++/Fisher implementation despite doctrine claim
- **Where:** `crates/context-graph-mejepa-train/src/online_head_state.rs:270-295` + grep for `EWC`, `fisher` in the train crate.
- **Looks healthy:** Doctrine (`CLAUDE.md §6.13`) says "Trainer-side `mejepa-train/src/mistake_consumer.rs` (#406) consumes `CF_MEJEPA_MISTAKE_LOG` under EWC++ Fisher protection." MCP tool `mejepa_record_mistake` writes mistakes to RocksDB.
- **Isn't:** (a) `mistake_consumer.rs` does not exist. (b) The shipped `online_head_state.rs` produces `claims_fisher_ewc_protection: false`, `fisher_ewc_row_key: None`, `protection_mode: ExactPanelKeyIsolation`. (c) The trainer's `fisher_violation: 0.2` is a hardcoded constant in `trainer/mod.rs:217`. There is no actual Fisher matrix computed anywhere.
- **Severity:** P1.
- **Suggested fix:** if EWC++ is doctrine, implement it: compute per-parameter Fisher diagonal from training gradients, snapshot to `CF_MEJEPA_FISHER_SNAPSHOTS`, and apply Fisher-weighted regularization to mistake-replay updates. Until then, the doctrine claim should be retracted.

### B-15 (P2): `compute_ood_score` exported but `residual_norm_sq` dead-exported
- **Where:** `crates/context-graph-mejepa/src/lib.rs:103`; defined at `compiler.rs:2631`.
- **Looks healthy:** Both are public functions, useful for FSV harnesses.
- **Isn't:** `residual_norm_sq` has ZERO non-test callers in the workspace (grep confirmed). It performs the same forbidden flat-buffer reduction as `compute_ood_score(panel.data(), ...)`. Per #618 it should be deleted.
- **Severity:** P3.
- **Suggested fix:** delete from `lib.rs:103` and from `compiler.rs:2631`.

## Verdict on the 0.79 measurement

**The number is technically reproducible (the FSV exists with sha256 evidence). But it is doctrinally meaningless.**

It is inflated by:
1. **Rank-lift exemplar transformation** (+0.31 over raw): the gate metric is no longer measuring the predictor; it's measuring a separate exemplar-similarity model fitted on `code_state_constellations.jsonl`. Even if the predictor were perfect random noise, exemplar similarity over real constellation slot features would produce a meaningful positive correlation with oracle labels. This is documented in the FSV file itself but treated as a legitimate gate input.
2. **#536 hard-negative weighting** (`tau=20.0`, `fail_boost=1.5`) selected on calibration set — modest contribution but reasonable.

It is deflated by:
1. **#618 flat-buffer OOD** — noise in big-dim slots can fire false OOD verdicts on otherwise-pass rows. Modest deflation.
2. **#619 flat inverse projection** — currently zero because `lambda_inv=0.0`; deflation = 0.
3. **#666/#670 e2/e3/e4/e8 forward-cache collapse** — fundamental substrate quality. Unknown magnitude but likely large for affected slots.
4. **`delete_test_call` substrate underpower** — global corr held back by this cell at 0.69. Deflation ~0.03.
5. **Cold-start conformal interval** (B-9) — forces OOD on under-100-outcome cells. Modest.
6. **Untrained predictor** (B-1, B-2) — fundamental. The raw 0.4843 Pearson is what an untrained predictor with rank-lift exemplar similarity scores. A trained predictor's raw could plausibly be 0.85+.

**The honest gap is not 0.16.** It is somewhere between 0.10 (if you keep rank-lift and just train the predictor) and 0.26 (if you switch to raw-Pearson gate). 0.95 is reachable from the current substrate IF the predictor is actually trained and rank-lift is replaced with a doctrinally-clean monotonic recalibration. Without training, rank-lift can be pushed to ~0.85 via exemplar surgery but probably no further — and ECE will not improve.

**The 0.953 active-subset diagnostic is not a hidden gate-passing measurement.** It is selection bias on the easy rows.

## Web-research notes

- ECE has well-documented pathologies — it's discontinuous in predictor space, dominated by self-similarity, and bin-sensitive. ([Measuring Calibration in Deep Learning](https://ar5iv.labs.arxiv.org/html/1904.01685), [Reassessing How to Compare and Improve Calibration](https://arxiv.org/html/2406.04068v1), [Information-theoretic ECE Analysis](https://arxiv.org/pdf/2405.15709)). Implication: ECE 0.024 is not strong evidence of calibration; consider also SmoothECE and Brier (Brier is a proper scoring rule and IS a more honest signal).

- Platt scaling and isotonic regression preserve AUC and ranking. ([Predicting Good Probabilities With Supervised Learning](https://www.cs.cornell.edu/~alexn/papers/calibration.icml05.crc.rev3.pdf), [Platt scaling — Wikipedia](https://en.wikipedia.org/wiki/Platt_scaling)). Implication: any defensible calibration step should not change the rank-Pearson by more than rounding error. The rank-lift's +0.31 lift to Pearson is therefore not a legitimate calibration — it's a different model.

- Isotonic regression provides better calibration than Platt when N ≥ 1000. ([FastML: Classifier calibration](https://fastml.com/classifier-calibration-with-platts-scaling-and-isotonic-regression/), [Train in Data: Platt Scaling Guide](https://www.blog.trainindata.com/complete-guide-to-platt-scaling/)). Implication: 240-row calibration is below the Platt/isotonic crossover; either is defensible, but the choice should be documented.

## Open questions

1. **Why was the trainer left in DIAGNOSTIC_CERTIFICATE_ONLY mode?** The constant is named honestly, but no issue tracks "make the trainer actually train". #596 ("Rust Trainer emitted pseudo-loss without training a predictor") closed by making the trainer admit it doesn't train. That's defensive but not constructive. What is the path to a real training loop? **Resolution evidence needed:** an issue with explicit Done-When clauses that the trainer applies gradient updates to predictor `VarMap` and saves a non-trivial checkpoint.

2. **Is the rank-lift transformation a sanctioned gate-eligible metric?** The doctrine (`07 §1`) describes it carefully and constrains it (excludes target-side labels, fits on calibration only, requires raw Pearson ≤ -0.25 for rank-reversing transforms). But the +0.31 jump over raw is on the same data, same labels, same features — it's just a different scoring function. **Resolution evidence needed:** a written policy comparing rank-lift to AUC and stating whether a model whose `predicted_oracle_pass` is purely random but whose exemplar-similarity features are real would also pass the rank-lift gate at 0.95.

3. **Does the substrate (after #670 regeneration) support 0.95 raw Pearson?** I cannot answer this without running training. The 491-row label registry + 128 skills + 379 constellations might be enough; or they might not be. **Resolution evidence needed:** train a simple GBDT on the slot-feature means + skill memberships, evaluate raw Pearson on holdout. If that's >0.85 the substrate is sufficient.

4. **Can `delete_test_call` be repaired without doctrinal compromise?** #615 has 1,131 candidates ready for oracle batch. If the regeneration produces, say, 100-200 split-stable failure-bearing rows, the cell is repairable. If it produces 0-10, the cell is structurally outside the scope of the corpus and should be excluded from the gate. **Resolution evidence needed:** run the #615 oracle batch and count.

5. **Why is `interval_width_threshold = 0.8` hardcoded?** No explanation in the code or docs. Cold-start conformal returns width 1.0, so every cold-start panel is OOD by construction. Is that intentional? **Resolution evidence needed:** issue discussing the choice; calibrated per-cell width.

6. **Is there ANY model-backed Panel B measurement?** Per `07 §1`, "Panel B has no model-backed measurement yet (#405/#521)." Per `08 §2`, #405 is OPEN p0 ("adds a hard new ship-gate requirement"). Yet `cross_panel.rs` validates `model_backed_panel_b: true`. Where is the actual implementation? **Resolution evidence needed:** open #405 / #521 with the actual Panel B build path.

7. **Are the FSV row predictions reproducible offline?** The `swebench-300x8-ship-gate-fsv` Rust harness has been deleted per #655. The numeric results live in JSON files. If we regenerate the same run, will we get the same 0.7942? Or will the substrate drift (the e2/e3/e4/e8 collapse, the path-canonicalization, etc.) produce different numbers? **Resolution evidence needed:** rerun via manual SoT readback per #655.

# Functional Specification — Phase 3 Training Loop (AdamW + Multi-Objective Loss + Per-Step Certs)

> **Level 2 (functional, *what* to build).** Subordinate to `specs/constitution.md` and the `<utml_v1_invariants>`, `<failure_taxonomy>`, `<performance_budgets>` blocks therein. When this functional spec conflicts with the constitution, the constitution wins. Implementation details (exact Rust signatures, file layout, persistence schema) live in `specs/technical/TECH-TRAINING.md`. This spec is the contract between the Phase 3 training loop and the Phase 1 instrument substrate, the Phase 2 predictor + frozen target panel, the cross-cutting UTML formula (`SPEC-UTML`), the cross-cutting TCT constellation guard (`SPEC-TCT`), and the Phase 4 inference / Phase 5 self-healing surfaces.

```xml
<functional_spec id="SPEC-TRAINING" version="1.0">

<metadata>
  <title>Phase 3 Training Loop — AdamW + Multi-Objective Loss + Per-Step Certificates + BatchSampler + Holdout Evaluation</title>
  <status>draft</status>
  <owner>ME-JEPA Training Team / Phase 3</owner>
  <created_date>2026-05-09</created_date>
  <last_updated>2026-05-09</last_updated>
  <related_specs>
    <spec_ref>SPEC-UTML (per-step L_step, sampling_weight, training cert schema)</spec_ref>
    <spec_ref>SPEC-PHASE2-PREDICTOR (predictor + frozen target panel produced by Phase 2)</spec_ref>
    <spec_ref>SPEC-INSTRUMENTS (Phase 1 instrument substrate — frozen target side)</spec_ref>
    <spec_ref>SPEC-CORPUS (Phase 0 mutation corpus + train/calibration/holdout split)</spec_ref>
    <spec_ref>SPEC-TCT (cross-cutting Phase 4b L_constellation_centroid loss term + Gτ guard)</spec_ref>
    <spec_ref>SPEC-EMBEDDERS (21 frozen content + learner-state embedders consumed by aux heads)</spec_ref>
    <spec_ref>SPEC-PHASE4-INFERENCE (consumes best.safetensors + calibration_threshold)</spec_ref>
    <spec_ref>SPEC-PHASE5-SELF-HEALING (consumes UTML cycle classifier verdict + holdout regression alerts)</spec_ref>
    <spec_ref>SPEC-WITNESS-CHAIN (per-epoch 73-byte SHAKE-256 entry; per-step parent_witness_hash → self_hash chain)</spec_ref>
  </related_specs>
  <source_documents>
    <source type="design_doc">docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §2.4 (optimizer + loss)</source>
    <source type="design_doc">docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §3.7 (multi-objective loss + sampling + counterfactual + cross-task transfer + aux distillation)</source>
    <source type="design_doc">docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §7 Phase 3 (deliverables + DoD + verification)</source>
    <source type="design_doc">docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §6 (per-example sampling_weight)</source>
    <source type="design_doc">docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §7 (consolidation integral M(t))</source>
    <source type="design_doc">docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §11 (per-step training certificate schema)</source>
    <source type="design_doc">docs/ruvectorfindings/12_EMBEDDER_INTEGRATION.md §6 (LoRA + distillation + AdamW + InfoNCE training infra shipped at crates/context-graph-embeddings/src/training/)</source>
    <source type="constitution">docs/ruvectorfindings/specs/constitution.md (utml_v1_invariants, failure_taxonomy, performance_budgets blocks)</source>
  </source_documents>
</metadata>

<overview>
Phase 3 is the training loop that converts the Phase 0 mutation corpus + Phase 1 frozen instrument
panels + Phase 2 predictor architecture into trained weights `theta_T` such that, given a
pre-execution panel + patch context, the predictor's output latent is close to the frozen target
panel's post-execution state under the multi-objective loss `L_full`. The deliverable is a single
binary entrypoint (`context-graph mejepa-train`) that runs 10 epochs of AdamW with cosine decay over
the 2,400-example base corpus on an RTX 5090 in under 12 wall-clock hours and produces (a) a chained,
auditable per-step training certificate stream, (b) a `best.safetensors` checkpoint selected by
holdout `predicted_oracle_pass` agreement ≥ 75%, and (c) a per-epoch witness-chain entry for
end-to-end audit replay.

The training loop is deliberately **thin** in net-new code: it composes the already-shipped
`context-graph-embeddings/src/training/{trainer, optimizer, loss, evaluation, pipeline,
distillation, lora, multitask}.rs` infrastructure with the Phase 2 ME-JEPA predictor and Phase 1
panel substrate. Per doc 12 §6, the LoRA + distillation + AdamW + InfoNCE pipeline already in tree
is *exactly* what UTML §V.1 (effective_plasticity / continual backprop) and §VII (consolidation)
require — Phase 3's job is to wire it, not reimplement it.

The four cross-cutting innovations Phase 3 owns end-to-end are:

1. **Multi-objective loss** `L_full` per doc 09 §3.7.1 with eleven weighted terms (predict + VICReg
   variance/covariance/invariance + warnings + runtime + memory + traceback + hunk attribution +
   collateral + reasoning + operator overrides) plus the `δ · L_constellation_centroid` term that
   bridges into Phase 4b TCT. Default coefficients are pinned in this spec.
2. **Per-step training certificate** `TrainingCertificate` carrying all four UTML factors + their
   sub-components + `sampling_weight` + `grad_norm` + `drift_coverage` + Merkle root, hash-chained
   `parent_witness_hash → self_hash`, persisted to `CF_MEJEPA_TRAIN_CERTS` keyed by step. Format per
   `SPEC-UTML` §AC-UTML-07 / doc 11 §11.
3. **BatchSampler with sampling_weight** per UTML §6 / doc 11 §6 — replaces doc 09 §3.7.2's older
   `(1 + 2·residual_error_z_score)` term with `L_step(ex)` integrated weight. Examples `< 0.05`
   drop; examples `> 1.0` (operator overrides) force-include. Re-computed weekly.
4. **Holdout evaluation cadence** every 1,000 steps on the 10% holdout split per Stoer-Wagner mincut
   (per `SPEC-CORPUS` and the shipped `crates/context-graph-mincut`). Tracks
   prediction_oracle_agreement, conformal coverage on calibration split, per-mutation-category
   accuracy, per-language accuracy, ΔΞ.predictor_redundancy (pairwise MI), gtau_pass_rate.

The training loop is **GENERIC-ONLY** per constitution &lt;anti_patterns&gt;: no per-task overfit,
no per-repo branches, no per-instance hints. Every primitive must generalize across all 300
SWE-bench Lite tasks AND all 11 supported languages. The mental check before any merge is "would
this same primitive be load-bearing for `psf__requests-2317` / `django__django-12286` /
`sympy__sympy-13895` and any future repo and any future supported language?"

Non-goals (covered elsewhere):

- The four `compute_delta_*` functions and the L_step computation itself live in `SPEC-UTML` /
  `TECH-UTML`. This spec consumes them.
- The frozen target panel construction (instruments, AST chunker) lives in `SPEC-INSTRUMENTS` /
  `SPEC-AST-CHUNKING`.
- The predictor architecture (g_φ over f_θ) lives in `SPEC-PHASE2-PREDICTOR` (which we depend on but
  is being written separately).
- The constellation centroid + Gτ guard lives in `SPEC-TCT`. We consume the
  `L_constellation_centroid` loss term and the `gtau_pass_rate` evaluation metric.
- The conformal calibration set + α threshold selection lives in `SPEC-PHASE4-INFERENCE`. We
  produce the artifacts they consume.
- EWC++ Fisher matrix construction + drift detection + A/B/C retrain lives in `SPEC-PHASE5-SELF-HEALING`.
  We feed them the holdout regression alerts + per-step certs.

</overview>

<user_types>

  <user_type id="UT-01" name="Phase 3 Trainer (the binary)">
    <description>The `context-graph mejepa-train --corpus ./data/mejepa-corpus --output ./data/mejepa-checkpoints` entrypoint, packaged as `crates/context-graph-mejepa-train/src/bin/mejepa-train.rs`. Loads the Phase 1 frozen embedders + Phase 2 predictor, iterates the train split, computes `L_full`, runs an AdamW step with cosine decay, emits per-step certs, runs holdout evaluation every 1,000 steps, snapshots checkpoints, and persists per-epoch witness entries.</description>
    <permissions>read CF_MEJEPA_PANELS + CF_MEJEPA_MUTATION_CORPUS + CF_MEJEPA_ORACLE_VERDICTS; write CF_MEJEPA_WEIGHT_BLOBS + CF_MEJEPA_TRAIN_CERTS + ./data/mejepa-checkpoints/{step_N.safetensors, best.safetensors, last.safetensors}; emit witness-chain entries; emit FSV evidence to /tmp/contextgraph-mejepa-train-fsv/</permissions>
    <goals>complete 10 epochs in &lt; 12 hours wall-clock on RTX 5090; achieve ≥ 75% holdout agreement; never report a successful run while any cert chain link is broken or any UTML invariant violated</goals>
  </user_type>

  <user_type id="UT-02" name="Operator (human via CLI / SMAP)">
    <description>The human operator running ME-JEPA training, consuming train logs, reviewing FSV evidence, deciding whether a run promotes to inference. May invoke training with operator-override examples (gold supervision) and may inspect any per-step cert or holdout report.</description>
    <permissions>read all train artifacts; promote checkpoints to inference; pause/resume training; supply operator-override examples (gold) per UTML §6</permissions>
    <goals>observe convergence; verify chain integrity; never approve a run that fails the Phase 3 DoD gates</goals>
  </user_type>

  <user_type id="UT-03" name="Phase 4 Inference Surface">
    <description>The `MeJepaCompiler::verify(patch)` API and `mcp__cgreality__mejepa_verify` MCP tool. After Phase 3 completes, loads `best.safetensors` + the conformal calibration threshold + the per-language calibration coverage table.</description>
    <permissions>read-only on `best.safetensors`, calibration thresholds, holdout reports</permissions>
    <goals>refuse to operate on a Phase 3 run that did not pass DoD gates; never silently accept a regressed checkpoint</goals>
  </user_type>

  <user_type id="UT-04" name="Phase 5 Self-Healing Controller">
    <description>The drift / A/B/C / EWC++ controller. Reads the UTML cycle classifier output (Stuck / OptimizerDysregulated / LatentCollapsing / Confused / Optimal / Boring / Dissipating) emitted by the Phase 3 trainer, plus holdout regression alerts (prior best - current &gt; 2%), to schedule interventions (continual backprop, LR warmup reset, VICReg coefficient bump, constellation re-fit, curriculum prune).</description>
    <permissions>read CF_MEJEPA_TRAIN_CERTS + holdout reports; trigger A/B/C retrain; pause training; surface high-severity blockers</permissions>
    <goals>regulate ΔΩ + ΔΞ before adjusting ΔP/ΔK per UTML §IX coaching order; never prune curriculum under bad ΔΩ or bad ΔΞ; never promote a Mode B/C that lost on holdout</goals>
  </user_type>

  <user_type id="UT-05" name="Auditor / Operator post-hoc">
    <description>Replay tooling and human auditors verifying an end-to-end training run from CF_MEJEPA_TRAIN_CERTS. Verifies every step's four-factor breakdown, sampling_weight, grad_norm, drift_coverage, and Merkle-root + parent_witness_hash chain integrity. Crucially verifies `delta_xi_components.target_collapse == 0.0` for every step (any nonzero value is a critical bug indicator — an instrument has accidentally become trainable).</description>
    <permissions>read CF_MEJEPA_TRAIN_CERTS + per-epoch witness-chain entries + FSV evidence; replay end-to-end</permissions>
    <goals>byte-equal-readback verification of every training step; surface any chain break or any UTML invariant violation as a high-severity blocker</goals>
  </user_type>

</user_types>

<user_stories>

<story id="US-TRAIN-01" priority="must-have">
  <narrative>
    <as_a>Phase 3 Trainer (UT-01)</as_a>
    <i_want_to>run AdamW with linear warmup over the first 500 steps then cosine decay over the remaining steps, with lr=5e-4, weight_decay=0.04, batch_size=64, beta1=0.9, beta2=0.999, eps=1e-8, gradient clipping max_norm=1.0, mixed precision (bf16 forward/backward, fp32 master weights, fp32 optimizer state)</i_want_to>
    <so_that>training reaches Phase 3 DoD (≥75% holdout agreement) within the &lt;12-hour wall-clock budget on RTX 5090 with stable convergence and bounded numerical drift</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-TRAIN-01-01">
      <given>10 epochs over 2,400-example base corpus with batch_size=64</given>
      <when>training begins step 0</when>
      <then>total_steps ≈ 10 · ceil(2400/64) ≈ 380 steps; configurable via cli; warmup_steps=500 hard-clamped to min(500, total_steps/4); LR schedule = linear ramp 0 → 5e-4 over warmup_steps, then cosine decay 5e-4 → 0 over (total_steps − warmup_steps)</then>
    </criterion>
    <criterion id="AC-TRAIN-01-02">
      <given>any AdamW step</given>
      <when>compute parameter update</when>
      <then>m_t = β1·m_{t-1} + (1-β1)·g_t; v_t = β2·v_{t-1} + (1-β2)·g_t²; m̂ = m/(1-β1ᵗ); v̂ = v/(1-β2ᵗ); θ ← θ − lr·(m̂/(sqrt(v̂)+eps) + weight_decay·θ); decoupled weight decay per Loshchilov &amp; Hutter 2019; weight_decay applies to predictor + LoRA params, NEVER to LayerNorm / bias / embedder weights</then>
    </criterion>
    <criterion id="AC-TRAIN-01-03">
      <given>gradient norm before clipping is ‖g‖</given>
      <when>‖g‖ &gt; 1.0</when>
      <then>g ← g · (1.0 / ‖g‖); grad_norm pre-clip recorded in cert; max_norm=1.0 enforced uniformly across all trainable param groups</then>
    </criterion>
    <criterion id="AC-TRAIN-01-04">
      <given>mixed precision is enabled (default)</given>
      <when>forward/backward executes</when>
      <then>activations + gradients in bf16; master weights + optimizer state (m, v) in fp32; loss-scale not required (bf16 has fp32-equivalent dynamic range); fp32 master weights cast to bf16 each step before forward</then>
    </criterion>
    <criterion id="AC-TRAIN-01-05">
      <given>any forward/backward pass produces NaN or Inf</given>
      <when>detected by gradient finite-check</when>
      <then>step skipped; cert.skip_reason="nan_inf"; emit MEJEPA_TRAIN_LOSS_NAN per &lt;failure_taxonomy&gt;; rollback model state to start of step (no parameter motion); alert UT-04; if NaN observed 3 times in any 100-step window, halt run with MEJEPA_TRAIN_GRAD_EXPLODE</then>
    </criterion>
    <criterion id="AC-TRAIN-01-06">
      <given>Phase 3 walltime budget is 12 hours on RTX 5090</given>
      <when>10 epochs complete</when>
      <then>elapsed_wall_clock_seconds &lt; 43,200; per-step training latency p50 &lt; 50ms (predictor forward + 4 deltas + cert write + AdamW step); if running budget projects &gt; 12h at any 1,000-step checkpoint, surface high-severity blocker but do not auto-halt</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-TRAIN-02" priority="must-have">
  <narrative>
    <as_a>Phase 3 Trainer (UT-01)</as_a>
    <i_want_to>compute the multi-objective loss L_full = L_predict + λ_var·L_variance + λ_cov·L_covariance + λ_inv·L_invariance + α_warn·L_warnings + α_runtime·L_runtime + α_rss·L_memory + α_trace·L_traceback + α_hunk·L_hunk_attribution + α_adjacent·L_collateral + α_reasoning·L_reasoning + α_overrides·L_operator_match + δ·L_constellation_centroid with default coefficients</i_want_to>
    <so_that>the predictor learns latent prediction (L_predict) under VICReg-style anti-collapse regularization (λ_var/λ_cov/λ_inv) while aux heads stay calibrated against ground-truth oracle side outputs (warnings/runtime/RSS/traceback/hunk/collateral/reasoning) and operator-override gold dominates when present (α_overrides=5.0), and the constellation guard (TCT Phase 4b) closes the predictor-side ΔΞ loop</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-TRAIN-02-01">
      <given>default coefficients per doc 09 §3.7.1 + this spec</given>
      <when>L_full is computed</when>
      <then>λ_var=25, λ_cov=1, λ_inv=25, α_warn=0.5, α_runtime=0.3, α_rss=0.2, α_trace=0.5, α_hunk=1.0, α_adjacent=0.5, α_reasoning=0.3, α_overrides=5.0, δ=2.0; all configurable via &lt;multi_objective_loss&gt; section of training config TOML; weights stored verbatim in per-step cert.loss_components to enable A/B comparison</then>
    </criterion>
    <criterion id="AC-TRAIN-02-02">
      <given>VICReg variance + covariance + invariance terms operate on the predictor's *output* latent (post-projection) only — NEVER on the frozen target panel side</given>
      <when>L_variance / L_covariance / L_invariance computed</when>
      <then>per Bardes et al. 2022 + constitution &lt;utml_v1_invariants&gt; rule "no learnable target encoder"; any code path that pushes gradients through an instrument trait impl is a CRITICAL bug per &lt;anti_patterns&gt;; reuse `crates/context-graph-embeddings/src/training/loss.rs` VICReg impl</then>
    </criterion>
    <criterion id="AC-TRAIN-02-03">
      <given>aux heads consume the predictor's hidden state and predict warnings_count, log_runtime, peak_rss, traceback_cluster_id, per-hunk verdicts, adjacent-test outcomes, CoT cluster id</given>
      <when>L_warnings + L_runtime + L_memory + L_traceback + L_hunk_attribution + L_collateral + L_reasoning computed</when>
      <then>L_warnings = MSE(predicted_warnings, actual_warnings); L_runtime = MSE(predicted_log_runtime, actual_log_runtime); L_memory = MSE(predicted_rss, actual_rss); L_traceback = contrastive over stack-trace clusters; L_hunk_attribution = cross-entropy over per-hunk verdicts; L_collateral = binary CE per adjacent-test outcome; L_reasoning = contrastive over CoT embeddings; per doc 09 §3.7.1</then>
    </criterion>
    <criterion id="AC-TRAIN-02-04">
      <given>operator_override examples (gold supervision) are present in the batch</given>
      <when>L_operator_match computed</when>
      <then>L_operator_match = KL(ME-JEPA verdict || operator gold) summed over override examples in batch; α_overrides=5.0 ensures gold dominates when present; non-override examples contribute 0</then>
    </criterion>
    <criterion id="AC-TRAIN-02-05">
      <given>TCT constellation centroid is loaded (Phase 4b cross-cut)</given>
      <when>L_constellation_centroid computed</when>
      <then>L_constellation_centroid = mean over batch of cosine-distance(predictor_latent, nearest_constellation_centroid_for_(category, language, entity_type)); δ=2.0 default; if constellation not yet built (Phase 4b not shipped), δ defaults to 0.0 and the term is structurally inert (no error)</then>
    </criterion>
    <criterion id="AC-TRAIN-02-06">
      <given>L_full produces a finite non-negative scalar</given>
      <when>backward pass executes</when>
      <then>cert.loss = L_full.detach().to_f32(); cert.loss_components records each weighted term separately; any NaN/Inf in any component triggers MEJEPA_TRAIN_LOSS_NAN; the `crates/context-graph-embeddings/src/training/loss.rs` InfoNCE + directional + soft-label primitives are reused where their math matches</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-TRAIN-03" priority="must-have">
  <narrative>
    <as_a>Phase 3 Trainer (UT-01) and Auditor (UT-05)</as_a>
    <i_want_to>emit a TrainingCertificate per step containing step, loss, delta_p, delta_k, delta_omega, delta_xi, L_step, all sub-components, sampling_weight, grad_norm, drift_coverage, parent_witness_hash, self_hash, merkle_root, code_version, corpus_sha, embedder_versions, frozen_at, timestamp_iso8601 — persisted to CF_MEJEPA_TRAIN_CERTS keyed by step</i_want_to>
    <so_that>every gradient update is byte-equal-readback verifiable post-hoc; the chain of parent_witness_hash → self_hash makes the full training run an auditable log; this IS the physical M(t) consolidation tensor per UTML §XII</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-TRAIN-03-01">
      <given>step N completes (or is skipped)</given>
      <when>cert is emitted</when>
      <then>cert is the EXACT schema from `SPEC-UTML` §AC-UTML-07; written to CF_MEJEPA_TRAIN_CERTS[step] (u64 big-endian key for ordered iteration); JSON-serialized with sorted keys (deterministic byte representation); fsync after every 100 certs to bound durability gap</then>
    </criterion>
    <criterion id="AC-TRAIN-03-02">
      <given>cert at step N-1 exists in CF_MEJEPA_TRAIN_CERTS</given>
      <when>cert at step N is constructed</when>
      <then>cert[N].parent_witness_hash = sha256(cert[N-1].self_hash || cert[N-1].body_canonical_json); cert[N].self_hash = sha256(cert[N].body_canonical_json); merkle_root = merkle_root_over_components([delta_p_components, delta_k_components, delta_omega_components, delta_xi_components, loss_components])</then>
    </criterion>
    <criterion id="AC-TRAIN-03-03">
      <given>auditor (UT-05) replays cert chain end-to-end</given>
      <when>verifying chain integrity over a 100,000-step run</when>
      <then>every cert's parent_witness_hash matches the recomputed sha256 over the prior cert; any mismatch raises MEJEPA_TRAIN_CERT_CHAIN_BROKEN per &lt;failure_taxonomy&gt;; replay completes within 60 seconds for 100k chain (constitution &lt;testing_requirements&gt; "Witness-chain end-to-end replay verification on 100,000-entry chain")</then>
    </criterion>
    <criterion id="AC-TRAIN-03-04">
      <given>step is skipped (any UTML factor zero, NaN/Inf, SNR below floor, etc.)</given>
      <when>cert is emitted for the skipped step</when>
      <then>cert.skip_reason populated with one of {PerfectPrediction, CatastrophicForgetting, NanInf, GoodhartCatastrophe, SnrBelowFloor, OperatorAbort}; cert.sampling_weight = 0.0; cert.grad_norm reflects pre-skip computed value; chain link still emitted to preserve auditability</then>
    </criterion>
    <criterion id="AC-TRAIN-03-05">
      <given>cert.delta_xi_components.target_collapse</given>
      <when>any step</when>
      <then>MUST equal exactly 0.0 for every step; auditor's invariant scan over the full chain MUST find zero violations; any nonzero value is a CRITICAL bug indicator (an instrument has accidentally become trainable per constitution &lt;utml_v1_invariants&gt; rule 2) and surfaces a high-severity blocker</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-TRAIN-04" priority="must-have">
  <narrative>
    <as_a>Phase 3 Trainer (UT-01)</as_a>
    <i_want_to>maintain a BatchSampler that draws each example with probability proportional to sampling_weight(ex) = base_weight · L_step(ex) · (1 + 5·operator_override) · decay^age_in_days, dropping examples below 0.05 and force-including examples above 1.0</i_want_to>
    <so_that>hard-example mining (high L_step), gold-standard up-weighting (operator override), and recency-weighted curriculum are integrated into batch composition per UTML §6 / doc 11 §6, replacing doc 09 §3.7.2's older (1 + 2·residual_error_z_score) term</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-TRAIN-04-01">
      <given>training set with N examples each carrying their last-computed L_step, operator_override flag, and age_in_days</given>
      <when>BatchSampler::next_batch() called</when>
      <then>compute weights = [base_weight · L_step_i · (1 + 5·override_i) · 0.995^age_i for i in 0..N]; partition into {drop: weight &lt; 0.05, force: weight &gt; 1.0, regular: 0.05 ≤ weight ≤ 1.0}; force_indices included in EVERY batch this epoch; remaining batch slots filled by weighted-sampling-without-replacement from regular_indices proportional to their weights; total batch_size=64 enforced</then>
    </criterion>
    <criterion id="AC-TRAIN-04-02">
      <given>weight=0 example (e.g., perfect prediction last cycle, or NaN-skipped)</given>
      <when>BatchSampler reads weights for current epoch</when>
      <then>example DROPPED from current epoch; not included in any batch; will be re-evaluated when next epoch's L_step recomputed</then>
    </criterion>
    <criterion id="AC-TRAIN-04-03">
      <given>operator_override_flag=true (gold-standard example)</given>
      <when>weight computed</when>
      <then>multiplier (1 + 5·1) = 6 ensures weight &gt; 1.0 for any non-trivial L_step; example FORCE-INCLUDED in EVERY epoch; cannot be skipped by stochastic sampler; if more than batch_size force-includes exist, multiple batches per epoch each include all force-includes (warning surfaced if force_count &gt; batch_size/2)</then>
    </criterion>
    <criterion id="AC-TRAIN-04-04">
      <given>age_in_days = clock_time_now − example.created_at</given>
      <when>weight computed</when>
      <then>decay factor 0.995^age (~140-day half-life) applied; age stored on each example at corpus generation time (Phase 0 + Phase 7 shift-subscriber); recomputed weekly during periodic full retrain (per doc 09 §5.4) — NOT every step</then>
    </criterion>
    <criterion id="AC-TRAIN-04-05">
      <given>BatchSampler is invoked across an epoch</given>
      <when>all batches drawn for the epoch</when>
      <then>every regular-weighted example has been seen ≥ 1 time IF its weight ≥ 0.05; force-includes seen exactly N_batches_this_epoch times; total examples consumed reflects epoch_size; sampler is seeded deterministically (config-derived seed) so a re-run produces identical batch sequence given identical weight distribution</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-TRAIN-05" priority="must-have">
  <narrative>
    <as_a>Phase 3 Trainer (UT-01) and Phase 4 Inference Surface (UT-03)</as_a>
    <i_want_to>run holdout evaluation every 1,000 training steps on the 10% holdout split (Stoer-Wagner mincut per SPEC-CORPUS) and track {prediction_oracle_agreement, conformal_coverage_calibration, per_mutation_category_accuracy, per_language_accuracy, predictor_redundancy_pairwise_mi, gtau_pass_rate}</i_want_to>
    <so_that>convergence is monitored with auditable evidence, the best checkpoint is selected by holdout agreement (≥75% Phase 3 DoD), regressions are detected (&gt;2% drop from prior best triggers MEJEPA_TRAIN_HOLDOUT_REGRESSION), and per-mutation-category + per-language slicing surfaces any GENERIC-ONLY violation (a per-category or per-language gap implies the predictor is overfitting to a subset)</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-TRAIN-05-01">
      <given>training step is a multiple of 1,000 (or final step)</given>
      <when>holdout evaluation triggers</when>
      <then>iterate full holdout split (10% of 2,400 = 240 examples per Stoer-Wagner mincut); for each example: compute predictor.forward(panel_t01) → predicted_latent → oracle_head → predicted_oracle_pass; compare to actual_oracle_pass; emit HoldoutReport JSON to CF_MEJEPA_HOLDOUT_REPORTS[step]</then>
    </criterion>
    <criterion id="AC-TRAIN-05-02">
      <given>HoldoutReport at step N</given>
      <when>computed</when>
      <then>contains: step, prediction_oracle_agreement (fraction match), conformal_coverage_calibration (empirical coverage of predictor's α=0.10 conformal set on calibration split), per_mutation_category_accuracy (Map&lt;MutationCategory, f32&gt; over 8 categories: KnownGood/SubtleFlip/OffByOne/SwapVariable/DeleteTestCall/WrongFile/OverEngineer/CompileError), per_language_accuracy (Map&lt;Lang, f32&gt; over 11 supported languages), predictor_redundancy_pairwise_mi (avg normalized MI across panel slots), gtau_pass_rate (fraction of holdout panels that pass Gτ guard; 1.0 if Phase 4b not yet shipped)</then>
    </criterion>
    <criterion id="AC-TRAIN-05-03">
      <given>HoldoutReport at step N agreement ≥ best_so_far + ε (ε=0.005)</given>
      <when>checkpoint selection runs</when>
      <then>./data/mejepa-checkpoints/best.safetensors atomically replaced (write-tmp + rename); best_so_far updated; cert.holdout_promotion=true</then>
    </criterion>
    <criterion id="AC-TRAIN-05-04">
      <given>HoldoutReport at step N agreement &lt; best_so_far - 0.02 (regression by &gt;2 percentage points)</given>
      <when>regression detector runs</when>
      <then>emit MEJEPA_TRAIN_HOLDOUT_REGRESSION per &lt;failure_taxonomy&gt;; surface to UT-04 self-healing; do NOT auto-halt (transient regression possible); if 3 consecutive holdout reports regress, halt with critical blocker</then>
    </criterion>
    <criterion id="AC-TRAIN-05-05">
      <given>Phase 3 DoD gate per doc 09 §7</given>
      <when>final epoch's final holdout report emitted</when>
      <then>prediction_oracle_agreement ≥ 0.75 → Phase 3 PASS, ./data/mejepa-checkpoints/best.safetensors promoted to Phase 4; agreement &lt; 0.75 → Phase 3 FAIL, surface high-severity blocker, do NOT promote, require operator decision</then>
    </criterion>
    <criterion id="AC-TRAIN-05-06">
      <given>per-mutation-category or per-language accuracy gap &gt; 0.10 between best and worst slice</given>
      <when>HoldoutReport emitted</when>
      <then>surface GENERIC-ONLY warning (per constitution &lt;anti_patterns&gt; "no per-task overfit"); cert.generic_only_warning populated with the offending slices; not a halt-condition but a manual review trigger for UT-02</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-TRAIN-06" priority="must-have">
  <narrative>
    <as_a>Phase 3 Trainer (UT-01)</as_a>
    <i_want_to>perform counterfactual training per doc 09 §3.7.3: for each panel[t=1] in the batch, sample K=4 perturbations panel[t=1] + ε·v (v unit, ε ∈ {0.01, 0.05, 0.1, 0.2}), predict each, supervise against the frozen target readback, and integrate the resulting derivative-of-prediction signal into L_predict</i_want_to>
    <so_that>the predictor learns the local geometry of latent → reality mapping, not just point predictions; small perturbations produce small prediction deltas; large perturbations exceeding the constellation diameter produce predictable misses; the system has a derivative signal that catches degenerate local minima</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-TRAIN-06-01">
      <given>panel[t=1] in batch with hidden_dim D</given>
      <when>counterfactual sampler invoked (every 5th step by default; configurable)</when>
      <then>generate K=4 perturbed panels: for each ε ∈ {0.01, 0.05, 0.1, 0.2}: v = sample_unit_vector(D, seed=step·K + ε_idx); panel_perturbed = panel + ε·v; deterministic given (step, ε_idx) — re-run produces identical perturbations</then>
    </criterion>
    <criterion id="AC-TRAIN-06-02">
      <given>perturbed panels and their corresponding frozen-target readbacks</given>
      <when>counterfactual loss computed</when>
      <then>L_counterfactual = mean over K perturbations of MSE(predictor.forward(perturbed_panel), frozen_target.readback(perturbed_panel)); added to L_full with α_counterfactual=0.2 default; counterfactual cycle disabled by default for first 1,000 steps (warmup) then enabled — config-driven</then>
    </criterion>
    <criterion id="AC-TRAIN-06-03">
      <given>K=4 perturbations and the original panel</given>
      <when>derivative-of-prediction signal computed</when>
      <then>local_smoothness = mean over K of (1 - cosine(predictor(panel + ε·v), predictor(panel))) / ε; persisted to cert.counterfactual_smoothness; flag as anomaly if smoothness &gt; configured ceiling (suggests degenerate local minimum)</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-TRAIN-07" priority="must-have">
  <narrative>
    <as_a>Phase 3 Trainer (UT-01)</as_a>
    <i_want_to>perform cross-task transfer per doc 09 §3.7.5: in mini-batch sampler, when training on task T, with probability 0.3 sample from tasks within cosine &gt; 0.7 of T's patch (patch-similarity graph)</i_want_to>
    <so_that>related tasks share gradient signal; the GENERIC-ONLY constraint is empirically defended (a generic primitive should help similar tasks together); cross-task learning is structural rather than emergent</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-TRAIN-07-01">
      <given>patch-similarity graph available per doc 09 §3.5.3 (built from E_Diff embeddings of all 2,400 corpus entries)</given>
      <when>BatchSampler composes a batch</when>
      <then>for each example slot in batch: with p=0.3 sample from related_tasks(current_task) where related_tasks(T) = {T' : cosine(E_Diff(T), E_Diff(T')) &gt; 0.7}; with p=0.7 sample per default (sampling_weight) flow</then>
    </criterion>
    <criterion id="AC-TRAIN-07-02">
      <given>related_tasks(T) is empty (T has no neighbors above 0.7 cosine)</given>
      <when>cross-task transfer triggered</when>
      <then>fall back to default sampling_weight flow for that slot; emit cert.cross_task_transfer_fallback_count for diagnostics; if &gt; 30% of slots fall back across an epoch, warn (suggests patch-similarity graph too sparse)</then>
    </criterion>
    <criterion id="AC-TRAIN-07-03">
      <given>cross-task transfer is enabled</given>
      <when>any batch composed</when>
      <then>cert.cross_task_transfer_indices: Vec&lt;TaskId&gt; records which slots were drawn cross-task; auditor (UT-05) can verify the transfer mechanism is generating the expected ~30% cross-task representation</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-TRAIN-08" priority="must-have">
  <narrative>
    <as_a>Phase 3 Trainer (UT-01)</as_a>
    <i_want_to>perform aux-head distillation back into the trunk every 1,000 steps per doc 09 §3.7.6: distill aux-head predictions into the predictor's hidden state via a frozen-target distillation loss</i_want_to>
    <so_that>aux heads do not become detached "specialists"; the trunk representation stays rich and aux-head-relevant; re-uses the shipped distillation infra at crates/context-graph-embeddings/src/training/distillation.rs (with quality_gate=true filtering)</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-TRAIN-08-01">
      <given>training step is a multiple of 1,000</given>
      <when>aux-head distillation cycle triggers</when>
      <then>snapshot current aux heads as frozen teachers; for each batch in distillation phase (e.g., 50 batches): compute teacher_logits = frozen_aux_heads(predictor.hidden); compute student_logits = trainable_aux_heads(predictor.hidden); L_distill = sum over heads of KL(teacher_logits || student_logits); apply gradients back through predictor's hidden state ONLY (aux heads themselves frozen during distillation)</then>
    </criterion>
    <criterion id="AC-TRAIN-08-02">
      <given>distillation infrastructure at crates/context-graph-embeddings/src/training/distillation.rs has quality_gate</given>
      <when>distillation runs</when>
      <then>quality_gate=true: examples with teacher_confidence below threshold are skipped (UTML's signal_clarity term per doc 12 §6); skipped count persisted to cert.distillation_skipped_count</then>
    </criterion>
    <criterion id="AC-TRAIN-08-03">
      <given>distillation cycle complete</given>
      <when>cert emitted</when>
      <then>cert.distillation_cycle_id (monotonic), cert.distillation_loss_mean, cert.distillation_skipped_count populated; aux heads + predictor weights both updated; LoRA adapters (per doc 12 §6) are the parameter-efficient surface for the predictor's hidden-state update during distillation when configured</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-TRAIN-09" priority="must-have">
  <narrative>
    <as_a>Phase 3 Trainer (UT-01) and Auditor (UT-05)</as_a>
    <i_want_to>persist a per-epoch witness-chain entry as a single 73-byte SHAKE-256 record chained from the prior epoch's entry, summarizing the epoch's mean L_step, mean ΔP/ΔK/ΔΩ/ΔΞ, holdout agreement, best/worst slices, and the epoch's parent_witness_hash → self_hash pair</i_want_to>
    <so_that>the per-epoch view is auditable as a compact summary of the much larger per-step cert chain; reuses the shipped 73-byte witness primitive in context-graph-witness; the full training run can be replayed at the epoch level in &lt; 5 seconds</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-TRAIN-09-01">
      <given>epoch E completes</given>
      <when>witness-chain entry emitted</when>
      <then>EpochSummary {epoch: u32, mean_l_step: f32, mean_delta_p: f32, mean_delta_k: f32, mean_delta_omega: f32, mean_delta_xi: f32, holdout_agreement: f32, best_category: MutationCategory, worst_category: MutationCategory, best_language: Lang, worst_language: Lang, total_steps_this_epoch: u64, skipped_steps_this_epoch: u64} canonicalized + hashed via SHAKE-256-73 (clean-room reimplementation per constitution &lt;tech_stack&gt;); chained from prior epoch's witness-hash</then>
    </criterion>
    <criterion id="AC-TRAIN-09-02">
      <given>per-epoch witness chain over 10 epochs</given>
      <when>auditor (UT-05) replays</when>
      <then>10 entries × 73 bytes = 730 bytes total; replay verifies each parent_hash matches; if verified, return Ok with the EpochSummary tuple sequence; otherwise raise MEJEPA_TRAIN_CERT_CHAIN_BROKEN with the offending epoch index</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-TRAIN-10" priority="should-have">
  <narrative>
    <as_a>Phase 3 Trainer (UT-01)</as_a>
    <i_want_to>checkpoint the model state every K=2,000 steps (configurable) atomically (write to *.tmp, fsync, rename) producing step_N.safetensors + last.safetensors + (when holdout agreement improves) best.safetensors</i_want_to>
    <so_that>the training run is restart-safe; an OOM or crash mid-epoch loses at most K-1 steps of progress; the best checkpoint is always available to UT-03 even if the run later regresses</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-TRAIN-10-01">
      <given>step is a multiple of K (default 2,000)</given>
      <when>checkpointer triggers</when>
      <then>serialize predictor + LoRA + aux heads + AdamW state (m, v, step, lr_schedule_state) + sampler RNG state to ./data/mejepa-checkpoints/step_N.safetensors.tmp; fsync; rename to step_N.safetensors; rename last.safetensors → last.safetensors.prev; rename step_N.safetensors → last.safetensors (atomic on POSIX); CF_MEJEPA_WEIGHT_BLOBS keyed by theta_sha (deduplicated)</then>
    </criterion>
    <criterion id="AC-TRAIN-10-02">
      <given>training resumed from --resume ./data/mejepa-checkpoints/last.safetensors</given>
      <when>trainer initializes</when>
      <then>load all predictor + LoRA + aux head + optimizer state + RNG state; verify cert chain ends at the checkpoint's step (cert chain integrity + checkpoint must agree); resume at step N+1; if checkpoint corrupt or chain broken, emit MEJEPA_TRAIN_CHECKPOINT_CORRUPT and require operator intervention</then>
    </criterion>
    <criterion id="AC-TRAIN-10-03">
      <given>VRAM exceeds budget mid-step (rare; constitution budget is &lt;= 29 GB on RTX 5090)</given>
      <when>OOM detected</when>
      <then>emit MEJEPA_TRAIN_VRAM_OOM; surface high-severity blocker; recommend reducing batch_size or enabling gradient checkpointing; if recovery possible, fall back to last checkpoint and continue</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-TRAIN-11" priority="should-have">
  <narrative>
    <as_a>Phase 3 Trainer (UT-01)</as_a>
    <i_want_to>wire the shipped LoRA adapters (rank=32 default per doc 12 §6) over the predictor + the aux heads + the trainable embedders (E2/E3/E4/E5/E15-E21) so the parameter-efficient fine-tuning path is used by default</i_want_to>
    <so_that>UTML §V.1 effective_plasticity intervention (continual backprop) maps directly onto the shipped LoRA infra; full-fine-tune mode remains a config option (--full-finetune flag) but is not the default; this is the Phase 5 self-healing dependency per doc 12 §8.4</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-TRAIN-11-01">
      <given>--full-finetune flag NOT set (default)</given>
      <when>trainer initializes</when>
      <then>LoRA adapters created via crates/context-graph-embeddings/src/training/lora.rs::LoRAAdapter::new(rank=32, alpha=16, dropout=0.05); attached to query/key/value projections of predictor + aux heads + each trainable embedder; only LoRA params + LayerNorm/biases + aux-head output projections are trainable; base weights frozen</then>
    </criterion>
    <criterion id="AC-TRAIN-11-02">
      <given>--full-finetune flag set</given>
      <when>trainer initializes</when>
      <then>LoRA disabled; all predictor + aux head + embedder params trainable (subject to the constitution &lt;anti_patterns&gt; rule "no learnable target encoder" — instruments + Phase 1 frozen target panel still frozen); cert.training_mode="full_finetune"</then>
    </criterion>
    <criterion id="AC-TRAIN-11-03">
      <given>LoRA mode enabled and training reaches end of run</given>
      <when>checkpoint emitted</when>
      <then>save base_weights_sha (immutable reference) + lora_adapters.safetensors (delta); inference (UT-03) loads base weights once + LoRA delta per attempt; total checkpoint size dominated by LoRA blob (e.g., 0.5 GB per doc 12 §4) not full predictor</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-TRAIN-12" priority="must-have">
  <narrative>
    <as_a>Phase 3 Trainer (UT-01) and Phase 5 Self-Healing Controller (UT-04)</as_a>
    <i_want_to>fail-closed on the 6 documented training failure modes — MEJEPA_TRAIN_LOSS_NAN, MEJEPA_TRAIN_GRAD_EXPLODE, MEJEPA_TRAIN_VRAM_OOM, MEJEPA_TRAIN_CHECKPOINT_CORRUPT, MEJEPA_TRAIN_CERT_CHAIN_BROKEN, MEJEPA_TRAIN_HOLDOUT_REGRESSION — surfacing structured errors with full context (step, file paths, SHAs)</i_want_to>
    <so_that>the trainer never silently substitutes defaults, never produces a "successful" run that violated UTML invariants, and gives UT-04 enough information to schedule the right intervention per UTML §IX coaching order (regulate ΔΩ + ΔΞ before ΔP/ΔK)</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-TRAIN-12-01">
      <given>any forward/backward pass produces NaN or Inf in loss, gradient, or activation</given>
      <when>finite-check triggers</when>
      <then>emit MEJEPA_TRAIN_LOSS_NAN with {step, layer_or_param_name, observed_value, code_version}; rollback to start-of-step state; cert.skip_reason=NanInf; if 3 NaNs in 100 steps → escalate to MEJEPA_TRAIN_GRAD_EXPLODE</then>
    </criterion>
    <criterion id="AC-TRAIN-12-02">
      <given>‖g‖ &gt; 100× running mean of grad_norm</given>
      <when>detected after backward</when>
      <then>emit MEJEPA_TRAIN_GRAD_EXPLODE with {step, grad_norm, running_mean, ratio}; clip aggressively to running_mean (instead of max_norm=1.0); persist to cert; if recurring, halt with critical blocker</then>
    </criterion>
    <criterion id="AC-TRAIN-12-03">
      <given>VRAM allocation fails during step</given>
      <when>candle returns OOM</when>
      <then>emit MEJEPA_TRAIN_VRAM_OOM with {step, requested_bytes, free_bytes, batch_size, gradient_checkpointing_enabled}; halt run; surface high-severity blocker; recommend operator action (reduce batch_size or enable gradient checkpointing)</then>
    </criterion>
    <criterion id="AC-TRAIN-12-04">
      <given>checkpoint deserialization fails (corrupt safetensors, missing key, dtype mismatch)</given>
      <when>--resume invoked</when>
      <then>emit MEJEPA_TRAIN_CHECKPOINT_CORRUPT with {checkpoint_path, expected_keys, missing_keys, dtype_mismatches}; halt; surface critical blocker</then>
    </criterion>
    <criterion id="AC-TRAIN-12-05">
      <given>per-step cert chain replay finds parent_witness_hash mismatch</given>
      <when>during periodic chain audit (every 1,000 steps) or on-demand replay</when>
      <then>emit MEJEPA_TRAIN_CERT_CHAIN_BROKEN with {step, expected_parent_hash, actual_parent_hash}; halt training (do not produce more certs that would chain off the broken link); surface critical blocker; require operator inspection</then>
    </criterion>
    <criterion id="AC-TRAIN-12-06">
      <given>holdout report at step N shows agreement &lt; best_so_far - 0.02</given>
      <when>regression detector runs</when>
      <then>emit MEJEPA_TRAIN_HOLDOUT_REGRESSION with {step, current_agreement, prior_best_agreement, delta, slice_breakdowns}; surface to UT-04; do NOT halt (transient regression possible); if 3 consecutive regressions, escalate to halt + critical blocker</then>
    </criterion>
  </acceptance_criteria>
</story>

</user_stories>

<requirements>

<requirement id="REQ-TRAIN-01" story_ref="US-TRAIN-01" priority="must">
  <description>Implement AdamW with linear warmup over the first 500 steps then cosine decay over the remaining steps. Hyperparameters: lr=5e-4, weight_decay=0.04, batch_size=64, beta1=0.9, beta2=0.999, eps=1e-8, max_grad_norm=1.0, mixed precision (bf16 forward/backward, fp32 master weights, fp32 optimizer state). Decoupled weight decay applies to predictor + LoRA + aux head params, NEVER to LayerNorm / bias / frozen embedder weights.</description>
  <rationale>Per doc 09 §2.4 + Phase 3 deliverable §7. Decoupled weight decay (Loshchilov &amp; Hutter 2019) avoids the regularization-by-momentum interaction that vanilla Adam exhibits. Cosine LR schedule with warmup is the standard for transformer-class predictors. bf16 + fp32 master weights is standard mixed-precision for sm_120a on RTX 5090. Reuse the shipped optimizer at `crates/context-graph-embeddings/src/training/optimizer.rs::AdamW` (already implements warmup + cosine + clipping + decoupled WD).</rationale>
  <constraints>warmup_steps hard-clamped to min(500, total_steps/4) to handle short-corpus runs; max_grad_norm=1.0 enforced uniformly; weight_decay=0.04 applies to projection matrices + LoRA + aux outputs; weight_decay=0.0 for LayerNorm/biases/embedder base weights.</constraints>
  <verification_method>Unit tests: LR schedule produces correct values at step=0, step=500 (peak), step=total_steps (≈0). Gradient clipping clamps when ‖g‖&gt;1.0. AdamW update math matches reference impl. Integration test: 100-step dry-run on synthetic batch produces finite losses, monotonically decreasing on a quadratic.</verification_method>
</requirement>

<requirement id="REQ-TRAIN-02" story_ref="US-TRAIN-02" priority="must">
  <description>Implement multi-objective loss L_full = L_predict + λ_var·L_variance + λ_cov·L_covariance + λ_inv·L_invariance + α_warn·L_warnings + α_runtime·L_runtime + α_rss·L_memory + α_trace·L_traceback + α_hunk·L_hunk_attribution + α_adjacent·L_collateral + α_reasoning·L_reasoning + α_overrides·L_operator_match + δ·L_constellation_centroid with default coefficients λ_var=25, λ_cov=1, λ_inv=25, α_warn=0.5, α_runtime=0.3, α_rss=0.2, α_trace=0.5, α_hunk=1.0, α_adjacent=0.5, α_reasoning=0.3, α_overrides=5.0, δ=2.0.</description>
  <rationale>Per doc 09 §3.7.1. L_predict is the JEPA-foundational latent prediction loss. VICReg variance/covariance/invariance defend predictor-side ΔΞ (target-side is structurally collapse-immune by frozen instruments). Aux losses (warnings/runtime/rss/traceback/hunk/collateral/reasoning) calibrate against ground-truth oracle side outputs. α_overrides=5.0 ensures operator gold dominates when present. δ=2.0 wires the TCT Phase 4b constellation centroid loss.</rationale>
  <constraints>VICReg terms operate ONLY on predictor output latent (NEVER on target panel); aux heads consume predictor's hidden state; gradients flow back through predictor + aux heads + LoRA adapters but NEVER through Phase 1 instruments (constitution &lt;utml_v1_invariants&gt;); constellation centroid term δ=0.0 fallback when Phase 4b not yet shipped (term structurally inert).</constraints>
  <verification_method>Unit tests per loss component: L_predict equals MSE on toy data; VICReg variance/cov/inv match Bardes 2022 reference values on Gaussian batches; aux MSEs/CEs match cross-entropy/MSE reference. Integration test: composite L_full finite, deterministic byte-equal under same inputs + code_version. Reuses crates/context-graph-embeddings/src/training/loss.rs primitives where math matches.</verification_method>
</requirement>

<requirement id="REQ-TRAIN-03" story_ref="US-TRAIN-03" priority="must">
  <description>Implement TrainingCertificate with EXACT field set per SPEC-UTML §AC-UTML-07: step, loss, delta_p, delta_k, delta_omega, delta_xi, L_step, delta_omega_components{effective_plasticity, landscape_health, stability_floor, agent_state_score}, delta_xi_components{target_collapse, predictor_redundancy, constellation_violation_rate}, delta_k_components{cos_align, fisher_violation, ece, embedder_coherence}, sampling_weight, grad_norm, drift_coverage, parent_witness_hash, self_hash, merkle_root, code_version, corpus_sha, embedder_versions, frozen_at, timestamp_iso8601 plus Phase 3-specific extensions: loss_components, skip_reason, distillation_cycle_id, counterfactual_smoothness, cross_task_transfer_indices, holdout_promotion, generic_only_warning. Persist to CF_MEJEPA_TRAIN_CERTS keyed by step (u64 big-endian); chain via parent_witness_hash → self_hash.</description>
  <rationale>Per UTML §XII the audit substrate IS the physical M(t) consolidation tensor. Hash-chained parent_witness_hash → self_hash makes the full training run a byte-equal-readback verifiable log. Required for operator audit (UT-05) and Phase 4 inference promotion gating (UT-03).</rationale>
  <constraints>JSON serialization deterministic (sorted keys, no map-iteration nondeterminism); fsync every 100 certs; chain integrity verifiable end-to-end on 100,000-step run within 60 seconds (constitution &lt;testing_requirements&gt;); skip_reason populated when L_step==0 or step skipped for any reason; chain link still emitted on skip to preserve auditability.</constraints>
  <verification_method>FSV: write 100,000 certs in dry-run, replay end-to-end, verify chain integrity, verify all fields present + well-typed. Tamper test: corrupt cert[N].body, verify cert[N+1].parent_witness_hash mismatch detected. Invariant scan: assert delta_xi_components.target_collapse == 0.0 for every step in chain.</verification_method>
</requirement>

<requirement id="REQ-TRAIN-04" story_ref="US-TRAIN-04" priority="must">
  <description>Implement BatchSampler that draws each example with weight = base_weight · L_step(ex) · (1 + 5·operator_override) · 0.995^age_in_days. Drop examples with weight &lt; 0.05; force-include examples with weight &gt; 1.0. Re-compute weights weekly during periodic full retrain (per doc 09 §5.4). Sampler is deterministically seeded so re-runs reproduce identical batch sequences given identical weights.</description>
  <rationale>Per UTML §6 / doc 11 §6 — replaces doc 09 §3.7.2's older (1 + 2·residual_error_z_score) term with L_step(ex) integrated weight which captures hard-example mining via ΔP AND enforces ΔΞ-safety + ΔΩ-safety + ΔK-coherence, which residual-z-score did not. Operator override (gold) dominates per UTML.</rationale>
  <constraints>Weights non-negative; not clamped to [0,1] (override pushes above 1.0); drop threshold 0.05 fixed; force-include threshold 1.0 fixed; weighted-sampling-without-replacement for regular slots; force-includes guaranteed in EVERY batch they qualify for.</constraints>
  <verification_method>Unit tests: weight=0 → drop; weight=1.0 with override → force-include. Integration test: 1-epoch dry-run on 100 synthetic examples confirms (a) high-L_step examples appear more often, (b) force-includes appear in every batch, (c) re-run with same seed produces identical batch sequence. FSV: 1,000-batch run histogram of example-frequency vs sampling_weight matches expected proportionality.</verification_method>
</requirement>

<requirement id="REQ-TRAIN-05" story_ref="US-TRAIN-05" priority="must">
  <description>Implement holdout evaluation every 1,000 steps on the 10% holdout split (Stoer-Wagner mincut per SPEC-CORPUS, shipped at crates/context-graph-mincut). Track {prediction_oracle_agreement, conformal_coverage_calibration, per_mutation_category_accuracy (8 categories), per_language_accuracy (11 languages), predictor_redundancy_pairwise_mi, gtau_pass_rate}. Promote to best.safetensors when agreement improves by ≥0.005; emit MEJEPA_TRAIN_HOLDOUT_REGRESSION when agreement drops by &gt;0.02.</description>
  <rationale>Per doc 09 §7 Phase 3 deliverable + DoD: training runs to completion (10 epochs on base 2,400-example corpus) on RTX 5090 in &lt;12 hours; holdout `predicted_oracle_pass`-vs-actual-oracle-pass agreement reaches ≥75% at end of training; chain integrity verifiable end-to-end. Per-category + per-language slicing surfaces GENERIC-ONLY violations.</rationale>
  <constraints>Holdout split is the 10% from Stoer-Wagner mincut (already shipped). Reuse crates/context-graph-embeddings/src/training/evaluation.rs::EvaluationMetrics for {agreement, MRR, AUC, F1} primitives. Conformal coverage computed on calibration split (separate 10%). Reports persisted to CF_MEJEPA_HOLDOUT_REPORTS keyed by step.</constraints>
  <verification_method>FSV: holdout-eval on 240-example synthetic split produces well-formed report; agreement value within bounds; per-category and per-language maps cover the expected key sets. Integration test: 3 consecutive regressions trigger halt; non-regression progression promotes best.safetensors atomically.</verification_method>
</requirement>

<requirement id="REQ-TRAIN-06" story_ref="US-TRAIN-06" priority="must">
  <description>Implement counterfactual training: every 5th step (configurable), for each panel[t=1] in batch, generate K=4 perturbations panel + ε·v with v=sample_unit_vector(D, seed=step·K+ε_idx) and ε ∈ {0.01, 0.05, 0.1, 0.2}. Predict each perturbed panel; supervise against the frozen target readback. Add L_counterfactual = mean MSE over perturbations to L_full with α_counterfactual=0.2.</description>
  <rationale>Per doc 09 §3.7.3. Builds derivative-of-prediction signal so the predictor learns local geometry, not just point predictions. Catches degenerate local minima.</rationale>
  <constraints>Deterministic given (step, ε_idx); K=4 fixed per doc 09; ε set fixed per doc 09; counterfactual disabled for first 1,000 steps (warmup) by default.</constraints>
  <verification_method>Unit test: same (step, ε_idx) produces same perturbation. Integration test: counterfactual cycle every 5 steps produces L_counterfactual term in cert.loss_components; smoothness signal computed and persisted.</verification_method>
</requirement>

<requirement id="REQ-TRAIN-07" story_ref="US-TRAIN-07" priority="must">
  <description>Implement cross-task transfer: in mini-batch sampler, when training on task T, with probability 0.3 sample from related_tasks(T) = {T' : cosine(E_Diff(T), E_Diff(T')) &gt; 0.7}. Persist cross_task_transfer_indices in cert. Fall back to default sampling_weight flow when related_tasks(T) is empty.</description>
  <rationale>Per doc 09 §3.5.3 + §3.7.5. Related tasks share gradient signal; the GENERIC-ONLY constraint is empirically defended (a generic primitive should help similar tasks together).</rationale>
  <constraints>Patch-similarity graph built from E_Diff embeddings of all 2,400 corpus entries (Phase 0 dependency). p=0.3 fixed per doc 09. cosine threshold 0.7 fixed. Empty-neighbor fallback triggers a counter persisted to cert.</constraints>
  <verification_method>Unit test: empty related_tasks → fallback path. Integration test: 1,000-batch run produces ~30% cross-task slots when patch-similarity graph is dense. FSV: cross-task indices persisted in cert chain auditable.</verification_method>
</requirement>

<requirement id="REQ-TRAIN-08" story_ref="US-TRAIN-08" priority="must">
  <description>Implement aux-head distillation back into the trunk every 1,000 steps. Snapshot aux heads as frozen teachers; for ~50 batches: compute teacher_logits = frozen_aux_heads(predictor.hidden); student_logits = trainable_aux_heads(predictor.hidden); L_distill = sum KL(teacher || student); apply gradients ONLY to predictor's hidden state (aux heads themselves frozen during distillation). Re-uses crates/context-graph-embeddings/src/training/distillation.rs with quality_gate=true.</description>
  <rationale>Per doc 09 §3.7.6. Prevents aux heads from becoming detached "specialists"; keeps trunk representation rich. quality_gate=true filters low-confidence teacher signals (UTML signal_clarity per doc 12 §6).</rationale>
  <constraints>Distillation cycle interval 1,000 steps per doc 09. ~50 batches per cycle (configurable). LoRA adapters are the parameter-efficient surface for predictor hidden-state update during distillation. Aux heads frozen during distillation phase.</constraints>
  <verification_method>FSV: distillation cycle at step 1,000 produces L_distill term in cert; trunk weights updated; aux heads unchanged during cycle; quality_gate skip count matches expected for synthetic low-confidence batch.</verification_method>
</requirement>

<requirement id="REQ-TRAIN-09" story_ref="US-TRAIN-09" priority="must">
  <description>Persist a per-epoch witness-chain entry as a single 73-byte SHAKE-256 record chained from prior epoch. Entry summarizes EpochSummary {epoch, mean_l_step, mean_delta_p, mean_delta_k, mean_delta_omega, mean_delta_xi, holdout_agreement, best_category, worst_category, best_language, worst_language, total_steps_this_epoch, skipped_steps_this_epoch}.</description>
  <rationale>Compact per-epoch audit summary. Re-uses the shipped 73-byte witness primitive in context-graph-witness (clean-room SHAKE-256 implementation per constitution &lt;tech_stack&gt;). 10 epochs × 73 bytes = 730 bytes total enables fast end-to-end replay (&lt; 5 seconds).</rationale>
  <constraints>SHAKE-256 73-byte format from context-graph-witness. Chained from prior epoch's hash. Canonicalized JSON before hashing for determinism.</constraints>
  <verification_method>FSV: 10-epoch run produces 10 chained witness entries; replay verifies all parent links; tamper test on epoch E witness invalidates E+1 → ... chain.</verification_method>
</requirement>

<requirement id="REQ-TRAIN-10" story_ref="US-TRAIN-10" priority="should">
  <description>Implement atomic checkpoint every 2,000 steps (configurable): write predictor + LoRA + aux heads + AdamW state (m, v, step, lr_schedule_state) + sampler RNG state to ./data/mejepa-checkpoints/step_N.safetensors.tmp; fsync; rename to step_N.safetensors. Update last.safetensors atomically. Promote best.safetensors when holdout agreement improves. Persist to CF_MEJEPA_WEIGHT_BLOBS keyed by theta_sha (deduplicated). Resume support via --resume.</description>
  <rationale>Restart safety. OOM/crash loses at most K-1 steps. best.safetensors is always valid for UT-03 even if later regression.</rationale>
  <constraints>Atomic rename for POSIX guarantees. CF_MEJEPA_WEIGHT_BLOBS dedup by theta_sha. Resume verifies cert chain ends at checkpoint step; corruption emits MEJEPA_TRAIN_CHECKPOINT_CORRUPT.</constraints>
  <verification_method>FSV: checkpoint at step 2,000 → kill trainer → resume → verify continuation produces identical step 2,001 to non-interrupted run. Tamper test on checkpoint corrupts chain → MEJEPA_TRAIN_CHECKPOINT_CORRUPT.</verification_method>
</requirement>

<requirement id="REQ-TRAIN-11" story_ref="US-TRAIN-11" priority="should">
  <description>Wire shipped LoRA adapters (rank=32, alpha=16, dropout=0.05) over predictor + aux heads + trainable embedders (E2/E3/E4/E5/E15-E21) by default. --full-finetune flag opts out. LoRA-only checkpoint blob ~0.5 GB per doc 12 §4.</description>
  <rationale>Per doc 12 §6 + §8.4. UTML §V.1 effective_plasticity intervention (continual backprop) maps directly onto shipped LoRA infra. Phase 5 self-healing dependency. Parameter-efficient by default; full-finetune as escape hatch.</rationale>
  <constraints>Reuse crates/context-graph-embeddings/src/training/lora.rs::LoRAAdapter::new(rank=32, alpha=16, dropout=0.05). LoRA on q/k/v projections of predictor + aux + trainable embedders. Base instrument weights NEVER trainable per constitution &lt;anti_patterns&gt; "no learnable target encoder".</constraints>
  <verification_method>Unit test: LoRA mode produces base+delta separation; checkpoint size dominated by LoRA delta. Integration test: --full-finetune flag toggles full predictor trainability; cert.training_mode field correct.</verification_method>
</requirement>

<requirement id="REQ-TRAIN-12" story_ref="US-TRAIN-12" priority="must">
  <description>Fail-closed on 6 documented training failure modes: MEJEPA_TRAIN_LOSS_NAN (NaN/Inf in loss/grad/activation), MEJEPA_TRAIN_GRAD_EXPLODE (grad_norm &gt; 100× running mean), MEJEPA_TRAIN_VRAM_OOM (CUDA OOM), MEJEPA_TRAIN_CHECKPOINT_CORRUPT (deserialization fails), MEJEPA_TRAIN_CERT_CHAIN_BROKEN (chain integrity violation), MEJEPA_TRAIN_HOLDOUT_REGRESSION (eval drops &gt;2% from prior best). Each emits structured error with full context (step, file paths, SHAs).</description>
  <rationale>Per constitution &lt;failure_taxonomy&gt; + this spec. Trainer never silently substitutes defaults; never produces a "successful" run that violated UTML invariants; gives UT-04 enough information to schedule the right intervention per UTML §IX coaching order.</rationale>
  <constraints>SCREAMING_SNAKE error codes per constitution &lt;coding_standards&gt; &lt;naming_conventions&gt;. Source-of-truth file paths in error context start with `file:` per &lt;file_organization&gt;. 3 consecutive regressions → escalate to halt + critical blocker.</constraints>
  <verification_method>Unit tests per error code: synthetic NaN injected → MEJEPA_TRAIN_LOSS_NAN; synthetic grad spike → MEJEPA_TRAIN_GRAD_EXPLODE; corrupted checkpoint → MEJEPA_TRAIN_CHECKPOINT_CORRUPT. Integration test: 3 consecutive holdout regressions trigger halt.</verification_method>
</requirement>

</requirements>

<business_rules>

<rule id="BR-TRAIN-01" req_ref="REQ-TRAIN-01">
  <condition>AdamW step computed at training step t</condition>
  <action>LR(t) = if t &lt; warmup_steps: 5e-4 · (t/warmup_steps); else: 5e-4 · 0.5 · (1 + cos(π · (t-warmup_steps) / (total_steps-warmup_steps))); clamp to ≥ 0; step apply m̂ / (sqrt(v̂)+eps) update with decoupled weight_decay=0.04 on predictor + LoRA + aux heads only</action>
  <exception>warmup_steps hard-clamped to min(500, total_steps/4) for short corpora; LayerNorm + biases + frozen embedder weights have weight_decay=0.0 always</exception>
</rule>

<rule id="BR-TRAIN-02" req_ref="REQ-TRAIN-02">
  <condition>L_full computed any step</condition>
  <action>Sum 13 weighted terms with default coefficients (λ_var=25, λ_cov=1, λ_inv=25, α_warn=0.5, α_runtime=0.3, α_rss=0.2, α_trace=0.5, α_hunk=1.0, α_adjacent=0.5, α_reasoning=0.3, α_overrides=5.0, δ=2.0); each term gradient flows back through predictor + aux heads + LoRA but NEVER through Phase 1 instruments</action>
  <exception>δ=0.0 fallback when Phase 4b TCT constellation not yet shipped (term structurally inert, no error); α_overrides applies only to operator-override examples in batch (zero contribution otherwise)</exception>
</rule>

<rule id="BR-TRAIN-03" req_ref="REQ-TRAIN-03">
  <condition>Per-step training certificate emission</condition>
  <action>Write to CF_MEJEPA_TRAIN_CERTS[step] with all required fields (UTML schema + Phase 3 extensions); chain via parent_witness_hash → self_hash; fsync every 100 certs; cert.delta_xi_components.target_collapse MUST equal 0.0 for every step</action>
  <exception>Step skipped (any factor zero, NaN/Inf, OOM, etc.): cert.skip_reason populated; cert.sampling_weight = 0.0; chain link still emitted to preserve integrity</exception>
</rule>

<rule id="BR-TRAIN-04" req_ref="REQ-TRAIN-04">
  <condition>BatchSampler reads sampling_weight</condition>
  <action>Drop if weight &lt; 0.05; force-include if weight &gt; 1.0; weighted-sampling-without-replacement for regular slots; force-includes guaranteed in every batch they qualify for; deterministic seed from config</action>
  <exception>operator_override=true forces weight &gt; 1.0 by construction (1 + 5·1 = 6× multiplier); always force-included; if &gt; batch_size/2 force-includes, warn (suggests too many overrides)</exception>
</rule>

<rule id="BR-TRAIN-05" req_ref="REQ-TRAIN-05">
  <condition>Holdout evaluation triggers (every 1,000 steps OR final step)</condition>
  <action>Run full holdout split; compute report; persist to CF_MEJEPA_HOLDOUT_REPORTS[step]; promote best.safetensors atomically when agreement improves by ≥ 0.005; emit MEJEPA_TRAIN_HOLDOUT_REGRESSION on &gt; 0.02 drop from best</action>
  <exception>3 consecutive regressions → halt + critical blocker; per-category or per-language gap &gt; 0.10 → cert.generic_only_warning populated (manual review trigger, not halt)</exception>
</rule>

<rule id="BR-TRAIN-06" req_ref="REQ-TRAIN-06">
  <condition>Counterfactual cycle (every 5th step, after 1,000-step warmup)</condition>
  <action>Generate K=4 perturbations with deterministic seed (step·K+ε_idx); supervise against frozen-target readback; add L_counterfactual to L_full with α_counterfactual=0.2</action>
  <exception>First 1,000 steps: counterfactual disabled (warmup); flag local_smoothness anomaly when smoothness exceeds configured ceiling</exception>
</rule>

<rule id="BR-TRAIN-07" req_ref="REQ-TRAIN-07">
  <condition>BatchSampler composes a batch slot</condition>
  <action>With p=0.3 sample from related_tasks(current_task) = {T' : cosine(E_Diff(T), E_Diff(T')) &gt; 0.7}; with p=0.7 sample per default sampling_weight flow; persist cross_task_transfer_indices to cert</action>
  <exception>related_tasks(T) empty: fall back to default sampling_weight flow; if &gt; 30% of slots fall back across an epoch, warn (suggests patch-similarity graph too sparse)</exception>
</rule>

<rule id="BR-TRAIN-08" req_ref="REQ-TRAIN-08">
  <condition>Aux-head distillation cycle (every 1,000 steps)</condition>
  <action>Snapshot aux heads as frozen teachers; for 50 batches: compute teacher/student logits, KL divergence, gradient back through predictor's hidden state; aux heads frozen during cycle; quality_gate filters low-confidence teacher signals</action>
  <exception>quality_gate=true skip count persisted to cert.distillation_skipped_count; if skipped &gt; 50% of cycle batches, warn</exception>
</rule>

<rule id="BR-TRAIN-09" req_ref="REQ-TRAIN-09">
  <condition>Epoch E completes</condition>
  <action>Compute EpochSummary; canonicalize JSON; SHAKE-256-73 hash; chain from prior epoch; persist to per-epoch witness chain; size 73 bytes per entry</action>
  <exception>Epoch 0 has no parent (genesis hash); epoch index 0 entry persisted with parent_hash=0x00...00 (32 bytes of zero)</exception>
</rule>

<rule id="BR-TRAIN-10" req_ref="REQ-TRAIN-10">
  <condition>Checkpoint trigger (step % 2,000 == 0)</condition>
  <action>Atomic write-tmp + fsync + rename; update last.safetensors; promote best.safetensors when holdout agreement improves; CF_MEJEPA_WEIGHT_BLOBS dedup by theta_sha</action>
  <exception>OOM mid-checkpoint: emit MEJEPA_TRAIN_VRAM_OOM; halt; recommend reducing batch_size or gradient checkpointing</exception>
</rule>

<rule id="BR-TRAIN-11" req_ref="REQ-TRAIN-11">
  <condition>Trainer initialization</condition>
  <action>Default: LoRA mode (rank=32, alpha=16, dropout=0.05) on q/k/v projections of predictor + aux heads + trainable embedders; --full-finetune opts out; cert.training_mode populated</action>
  <exception>Phase 1 instruments + frozen embedder base weights NEVER trainable regardless of mode (constitution &lt;anti_patterns&gt; "no learnable target encoder")</exception>
</rule>

<rule id="BR-TRAIN-12" req_ref="REQ-TRAIN-12">
  <condition>Failure mode detected (NaN/Inf, grad spike, OOM, corrupt checkpoint, broken chain, holdout regression)</condition>
  <action>Emit SCREAMING_SNAKE error code + structured error context (step, paths, SHAs); rollback to safe state where possible; surface to UT-04 self-healing</action>
  <exception>3 consecutive same-cause failures escalate to halt + critical blocker (constitution &lt;reality_loop_contract&gt; rule 10)</exception>
</rule>

</business_rules>

<edge_cases>

<edge_case id="EC-TRAIN-01" req_ref="REQ-TRAIN-01">
  <scenario>total_steps &lt; 500 (very small corpus or smoke run)</scenario>
  <expected_behavior>warmup_steps clamped to min(500, total_steps/4); LR schedule still well-defined; cosine decay over (total_steps - warmup_steps) steps; if total_steps &lt; 4, warmup_steps=1</expected_behavior>
  <priority>medium</priority>
</edge_case>

<edge_case id="EC-TRAIN-02" req_ref="REQ-TRAIN-01">
  <scenario>NaN observed mid-step in gradient or activation</scenario>
  <expected_behavior>Step skipped; cert.skip_reason=NanInf; rollback to start-of-step state (no parameter motion); MEJEPA_TRAIN_LOSS_NAN emitted; if 3 NaNs in 100-step window → escalate to MEJEPA_TRAIN_GRAD_EXPLODE</expected_behavior>
  <priority>critical</priority>
</edge_case>

<edge_case id="EC-TRAIN-03" req_ref="REQ-TRAIN-02">
  <scenario>L_constellation_centroid term needed but Phase 4b (TCT) not yet shipped</scenario>
  <expected_behavior>δ=0.0 fallback; term structurally inert (zero contribution); cert.loss_components.constellation_centroid = 0.0; no error; once Phase 4b ships, δ=2.0 reverts on operator request</expected_behavior>
  <priority>high</priority>
</edge_case>

<edge_case id="EC-TRAIN-04" req_ref="REQ-TRAIN-02">
  <scenario>operator_override examples missing from batch (no gold supervision this step)</scenario>
  <expected_behavior>L_operator_match contribution = 0.0; α_overrides·L_operator_match term = 0; no error; cert.loss_components.operator_match = 0.0</expected_behavior>
  <priority>medium</priority>
</edge_case>

<edge_case id="EC-TRAIN-05" req_ref="REQ-TRAIN-03">
  <scenario>cert.delta_xi_components.target_collapse &gt; 0 observed (impossible by construction for ME-JEPA)</scenario>
  <expected_behavior>FAIL-CLOSED with CRITICAL severity per constitution &lt;utml_v1_invariants&gt; rule 2; an instrument has accidentally become trainable (gradient flow through what should be a frozen function); halt training immediately; surface high-severity blocker; require operator inspection; fix at trait level per &lt;anti_patterns&gt;</expected_behavior>
  <priority>critical</priority>
</edge_case>

<edge_case id="EC-TRAIN-06" req_ref="REQ-TRAIN-03">
  <scenario>cert chain integrity check at step N detects parent_hash mismatch</scenario>
  <expected_behavior>MEJEPA_TRAIN_CERT_CHAIN_BROKEN emitted with {step, expected_parent_hash, actual_parent_hash}; halt training (do not produce more certs that would chain off the broken link); restore from last verified checkpoint; surface critical blocker</expected_behavior>
  <priority>critical</priority>
</edge_case>

<edge_case id="EC-TRAIN-07" req_ref="REQ-TRAIN-04">
  <scenario>All examples have weight &lt; 0.05 (highly trained corpus, every example is "easy")</scenario>
  <expected_behavior>Epoch sampling vacuous; surface high-priority warning to UT-02; classify as Boring/Mastery via UTML §IX; recommend advancing curriculum (harder examples) or terminating training; do not auto-halt</expected_behavior>
  <priority>high</priority>
</edge_case>

<edge_case id="EC-TRAIN-08" req_ref="REQ-TRAIN-04">
  <scenario>force-include count &gt; batch_size/2 (operator over-supplied gold)</scenario>
  <expected_behavior>Multiple batches per epoch each include all force-includes; warning surfaced to UT-02 (suggests too many overrides for batch_size); not a halt; recommend batch_size increase or operator override pruning</expected_behavior>
  <priority>medium</priority>
</edge_case>

<edge_case id="EC-TRAIN-09" req_ref="REQ-TRAIN-05">
  <scenario>Holdout report shows per-language accuracy gap &gt; 0.10 between best and worst language</scenario>
  <expected_behavior>cert.generic_only_warning populated with offending slices; surface manual review trigger to UT-02 per constitution &lt;anti_patterns&gt; "no per-task overfit"; not a halt-condition; UT-02 may decide to rebalance corpus or augment underrepresented language</expected_behavior>
  <priority>high</priority>
</edge_case>

<edge_case id="EC-TRAIN-10" req_ref="REQ-TRAIN-05">
  <scenario>Final epoch's final holdout report shows agreement &lt; 0.75 (Phase 3 DoD failure)</scenario>
  <expected_behavior>Phase 3 PASS=false; do NOT promote best.safetensors to Phase 4; surface high-severity blocker to UT-02; require operator decision (extend training, change hyperparameters, redesign aux heads, etc.); cert.phase3_dod_passed=false</expected_behavior>
  <priority>critical</priority>
</edge_case>

<edge_case id="EC-TRAIN-11" req_ref="REQ-TRAIN-06">
  <scenario>Counterfactual smoothness ceiling exceeded (predictor has degenerate local minimum)</scenario>
  <expected_behavior>cert.counterfactual_smoothness_anomaly=true; surface to UT-04; recommend VICReg coefficient bump (higher λ_var/λ_cov) or constellation re-fit; not a halt-condition</expected_behavior>
  <priority>high</priority>
</edge_case>

<edge_case id="EC-TRAIN-12" req_ref="REQ-TRAIN-07">
  <scenario>Patch-similarity graph too sparse (many tasks have empty related_tasks)</scenario>
  <expected_behavior>Cross-task transfer fallback rate exceeds 30% across epoch; warn UT-02 (suggests E_Diff embeddings need re-fit or 0.7 cosine threshold needs tuning); not a halt-condition</expected_behavior>
  <priority>medium</priority>
</edge_case>

<edge_case id="EC-TRAIN-13" req_ref="REQ-TRAIN-08">
  <scenario>quality_gate filters &gt; 50% of distillation cycle batches</scenario>
  <expected_behavior>Warn UT-02 (aux heads predicting low-confidence on most batches; possibly trunk representation degraded or aux heads need refit); cert.distillation_skipped_count_excessive=true; not a halt-condition</expected_behavior>
  <priority>high</priority>
</edge_case>

<edge_case id="EC-TRAIN-14" req_ref="REQ-TRAIN-10">
  <scenario>Resume from --resume but checkpoint's step is ahead of CF_MEJEPA_TRAIN_CERTS' last entry</scenario>
  <expected_behavior>MEJEPA_TRAIN_CHECKPOINT_CORRUPT emitted with {checkpoint_step, last_cert_step, mismatch}; halt; require operator inspection (cert chain truncation or checkpoint manipulation suspected)</expected_behavior>
  <priority>critical</priority>
</edge_case>

<edge_case id="EC-TRAIN-15" req_ref="REQ-TRAIN-12">
  <scenario>VRAM OOM mid-step despite operating within constitution budget (29 GB on RTX 5090)</scenario>
  <expected_behavior>MEJEPA_TRAIN_VRAM_OOM emitted with {step, requested, free, batch_size, gradient_checkpointing_enabled}; halt; recommend (a) enabling gradient checkpointing, (b) reducing batch_size to 32 or 16, (c) lazy-loading learner-state embedders E15-E21 per doc 12 §4 risk #2</expected_behavior>
  <priority>critical</priority>
</edge_case>

</edge_cases>

<error_states>

<error id="ERR-TRAIN-01" http_code="500" req_ref="REQ-TRAIN-01">
  <condition>NaN or Inf detected in loss, gradient, or activation</condition>
  <user_message>Training step aborted: numerical instability detected. Run continues with rollback if recoverable.</user_message>
  <internal_message>MEJEPA_TRAIN_LOSS_NAN: location=&lt;loss|grad|activation&gt;, layer=&lt;name&gt;, step=&lt;N&gt;, observed_value=&lt;v&gt;, code_version=&lt;sha&gt;; rollback to start-of-step state initiated.</internal_message>
  <recovery_action>Skip step; emit cert with skip_reason=NanInf; if 3 NaNs within 100 steps → escalate to MEJEPA_TRAIN_GRAD_EXPLODE.</recovery_action>
</error>

<error id="ERR-TRAIN-02" http_code="500" req_ref="REQ-TRAIN-12">
  <condition>‖g‖ &gt; 100× running mean of grad_norm</condition>
  <user_message>Training instability: gradient norm exploded. Aggressive clipping applied; may halt run if recurring.</user_message>
  <internal_message>MEJEPA_TRAIN_GRAD_EXPLODE: step=&lt;N&gt;, grad_norm=&lt;v&gt;, running_mean=&lt;v&gt;, ratio=&lt;v&gt;, clip_target=&lt;running_mean&gt;.</internal_message>
  <recovery_action>Clip aggressively to running_mean (instead of max_norm=1.0); persist to cert; if recurring 3× in 100 steps → halt with critical blocker.</recovery_action>
</error>

<error id="ERR-TRAIN-03" http_code="500" req_ref="REQ-TRAIN-12">
  <condition>VRAM allocation fails during step (CUDA OOM)</condition>
  <user_message>Out-of-memory: training halted. Reduce batch_size or enable gradient checkpointing.</user_message>
  <internal_message>MEJEPA_TRAIN_VRAM_OOM: step=&lt;N&gt;, requested_bytes=&lt;v&gt;, free_bytes=&lt;v&gt;, batch_size=&lt;v&gt;, gradient_checkpointing_enabled=&lt;bool&gt;.</internal_message>
  <recovery_action>Halt run; surface high-severity blocker; recommend operator action.</recovery_action>
</error>

<error id="ERR-TRAIN-04" http_code="500" req_ref="REQ-TRAIN-10">
  <condition>Checkpoint deserialization fails (corrupt safetensors, missing key, dtype mismatch)</condition>
  <user_message>Checkpoint corrupt; cannot resume. Operator inspection required.</user_message>
  <internal_message>MEJEPA_TRAIN_CHECKPOINT_CORRUPT: checkpoint_path=file:&lt;path&gt;, expected_keys=&lt;set&gt;, missing_keys=&lt;set&gt;, dtype_mismatches=&lt;list&gt;.</internal_message>
  <recovery_action>Halt; require operator inspection; suggest fallback to step_(N-K).safetensors if available.</recovery_action>
</error>

<error id="ERR-TRAIN-05" http_code="500" req_ref="REQ-TRAIN-03">
  <condition>cert chain integrity check finds parent_witness_hash mismatch</condition>
  <user_message>Training audit chain broken. Halt and restore from verified checkpoint.</user_message>
  <internal_message>MEJEPA_TRAIN_CERT_CHAIN_BROKEN: step=&lt;N&gt;, expected_parent_hash=&lt;hex&gt;, actual_parent_hash=&lt;hex&gt;.</internal_message>
  <recovery_action>Halt training; replay chain to find tamper boundary; restore from last verified checkpoint; surface critical blocker.</recovery_action>
</error>

<error id="ERR-TRAIN-06" http_code="400" req_ref="REQ-TRAIN-05">
  <condition>Holdout agreement at step N drops &gt; 0.02 from prior best</condition>
  <user_message>Holdout regression detected. Evaluation continuing; if recurring, will halt.</user_message>
  <internal_message>MEJEPA_TRAIN_HOLDOUT_REGRESSION: step=&lt;N&gt;, current_agreement=&lt;v&gt;, prior_best_agreement=&lt;v&gt;, delta=&lt;v&gt;, slice_breakdowns=&lt;map&gt;.</internal_message>
  <recovery_action>Surface to UT-04 self-healing; do NOT halt (transient regression possible); 3 consecutive regressions → halt + critical blocker.</recovery_action>
</error>

<error id="ERR-TRAIN-07" http_code="500" req_ref="REQ-TRAIN-03">
  <condition>cert.delta_xi_components.target_collapse &gt; 0 observed (impossible by construction)</condition>
  <user_message>CRITICAL: ME-JEPA architectural integrity violation. An instrument has accidentally become trainable. Training halted.</user_message>
  <internal_message>MEJEPA_INSTRUMENT_GRADIENT_LEAK: step=&lt;N&gt;, target_collapse=&lt;v&gt;, expected=0.0; suspect_instrument=&lt;identifier&gt;; check Instrument trait for accidental requires_grad=true.</internal_message>
  <recovery_action>Halt training immediately; surface critical blocker; require operator inspection; fix at trait level per constitution &lt;anti_patterns&gt; "no learnable target encoder" rule.</recovery_action>
</error>

</error_states>

<data_requirements>

<entity name="TrainingConfig">
  <field name="lr" type="f64" required="true">
    <description>Base learning rate.</description>
    <constraints>Default 5e-4; positive.</constraints>
  </field>
  <field name="weight_decay" type="f64" required="true">
    <description>Decoupled weight decay coefficient.</description>
    <constraints>Default 0.04; non-negative; applies to predictor+LoRA+aux heads only.</constraints>
  </field>
  <field name="batch_size" type="usize" required="true">
    <description>Per-step batch size.</description>
    <constraints>Default 64; positive.</constraints>
  </field>
  <field name="epochs" type="u32" required="true">
    <description>Total epochs.</description>
    <constraints>Default 10; positive.</constraints>
  </field>
  <field name="warmup_steps" type="u32" required="true">
    <description>Linear warmup duration.</description>
    <constraints>Default 500; clamped to min(500, total_steps/4).</constraints>
  </field>
  <field name="max_grad_norm" type="f32" required="true">
    <description>Gradient clipping ceiling.</description>
    <constraints>Default 1.0; positive.</constraints>
  </field>
  <field name="mixed_precision" type="bool" required="true">
    <description>bf16 forward/backward + fp32 master weights.</description>
    <constraints>Default true.</constraints>
  </field>
  <field name="lora_rank" type="usize" required="true">
    <description>LoRA adapter rank.</description>
    <constraints>Default 32; positive.</constraints>
  </field>
  <field name="lora_alpha" type="usize" required="true">
    <description>LoRA alpha scaling.</description>
    <constraints>Default 16.</constraints>
  </field>
  <field name="lora_dropout" type="f32" required="true">
    <description>LoRA dropout.</description>
    <constraints>Default 0.05; in [0, 0.5].</constraints>
  </field>
  <field name="full_finetune" type="bool" required="true">
    <description>Opt out of LoRA; full predictor fine-tune.</description>
    <constraints>Default false.</constraints>
  </field>
  <field name="loss_coefficients" type="LossCoefficients" required="true">
    <description>Multi-objective loss coefficients (λ_var=25, λ_cov=1, λ_inv=25, α_warn=0.5, ..., δ=2.0).</description>
    <constraints>Defaults pinned per REQ-TRAIN-02; configurable via TOML.</constraints>
  </field>
  <field name="checkpoint_interval_steps" type="u64" required="true">
    <description>Steps between checkpoints.</description>
    <constraints>Default 2,000; positive.</constraints>
  </field>
  <field name="holdout_eval_interval_steps" type="u64" required="true">
    <description>Steps between holdout evaluations.</description>
    <constraints>Default 1,000; positive.</constraints>
  </field>
  <field name="counterfactual_interval_steps" type="u64" required="true">
    <description>Steps between counterfactual cycles.</description>
    <constraints>Default 5; positive.</constraints>
  </field>
  <field name="counterfactual_warmup_steps" type="u64" required="true">
    <description>Steps before counterfactual enabled.</description>
    <constraints>Default 1,000.</constraints>
  </field>
  <field name="distillation_interval_steps" type="u64" required="true">
    <description>Steps between aux-head distillation cycles.</description>
    <constraints>Default 1,000; positive.</constraints>
  </field>
  <field name="cross_task_transfer_probability" type="f32" required="true">
    <description>Probability a batch slot is drawn from related_tasks(T).</description>
    <constraints>Default 0.3; in [0, 1].</constraints>
  </field>
  <field name="cross_task_cosine_threshold" type="f32" required="true">
    <description>Cosine threshold for related_tasks neighborhood.</description>
    <constraints>Default 0.7; in [0, 1].</constraints>
  </field>
  <field name="sampling_drop_threshold" type="f32" required="true">
    <description>weight &lt; this → drop example.</description>
    <constraints>Default 0.05.</constraints>
  </field>
  <field name="sampling_force_threshold" type="f32" required="true">
    <description>weight &gt; this → force-include example.</description>
    <constraints>Default 1.0.</constraints>
  </field>
  <field name="sampling_age_decay" type="f32" required="true">
    <description>Per-day age decay rate.</description>
    <constraints>Default 0.995 (~140-day half-life).</constraints>
  </field>
  <field name="holdout_promotion_threshold" type="f32" required="true">
    <description>Min agreement improvement to promote best.safetensors.</description>
    <constraints>Default 0.005.</constraints>
  </field>
  <field name="holdout_regression_threshold" type="f32" required="true">
    <description>Min agreement drop to emit MEJEPA_TRAIN_HOLDOUT_REGRESSION.</description>
    <constraints>Default 0.02.</constraints>
  </field>
  <field name="phase3_dod_min_agreement" type="f32" required="true">
    <description>Minimum holdout agreement for Phase 3 DoD pass.</description>
    <constraints>Default 0.75 per doc 09 §7.</constraints>
  </field>
  <field name="random_seed" type="u64" required="true">
    <description>Master seed for sampler + counterfactual + dropout.</description>
    <constraints>Required; reproducibility.</constraints>
  </field>
</entity>

<entity name="LossCoefficients">
  <field name="lambda_var" type="f32" required="true"><description>VICReg variance.</description><constraints>Default 25.0.</constraints></field>
  <field name="lambda_cov" type="f32" required="true"><description>VICReg covariance.</description><constraints>Default 1.0.</constraints></field>
  <field name="lambda_inv" type="f32" required="true"><description>VICReg invariance.</description><constraints>Default 25.0.</constraints></field>
  <field name="alpha_warn" type="f32" required="true"><description>Warnings MSE.</description><constraints>Default 0.5.</constraints></field>
  <field name="alpha_runtime" type="f32" required="true"><description>Runtime MSE (log scale).</description><constraints>Default 0.3.</constraints></field>
  <field name="alpha_rss" type="f32" required="true"><description>Memory MSE.</description><constraints>Default 0.2.</constraints></field>
  <field name="alpha_trace" type="f32" required="true"><description>Traceback contrastive.</description><constraints>Default 0.5.</constraints></field>
  <field name="alpha_hunk" type="f32" required="true"><description>Hunk attribution CE.</description><constraints>Default 1.0.</constraints></field>
  <field name="alpha_adjacent" type="f32" required="true"><description>Collateral binary CE.</description><constraints>Default 0.5.</constraints></field>
  <field name="alpha_reasoning" type="f32" required="true"><description>Reasoning contrastive.</description><constraints>Default 0.3.</constraints></field>
  <field name="alpha_overrides" type="f32" required="true"><description>Operator override KL.</description><constraints>Default 5.0.</constraints></field>
  <field name="alpha_counterfactual" type="f32" required="true"><description>Counterfactual MSE.</description><constraints>Default 0.2.</constraints></field>
  <field name="delta" type="f32" required="true"><description>L_constellation_centroid coefficient.</description><constraints>Default 2.0; falls back to 0.0 when Phase 4b not yet shipped.</constraints></field>
</entity>

<entity name="HoldoutReport">
  <field name="step" type="u64" required="true"><description>Training step.</description><constraints>Primary key in CF_MEJEPA_HOLDOUT_REPORTS.</constraints></field>
  <field name="prediction_oracle_agreement" type="f32" required="true"><description>Fraction of holdout examples where predicted_oracle_pass matches actual.</description><constraints>In [0, 1].</constraints></field>
  <field name="conformal_coverage_calibration" type="f32" required="true"><description>Empirical conformal-set coverage on calibration split.</description><constraints>In [0, 1].</constraints></field>
  <field name="per_mutation_category_accuracy" type="Map&lt;MutationCategory, f32&gt;" required="true"><description>8-category breakdown.</description><constraints>Keys = {KnownGood, SubtleFlip, OffByOne, SwapVariable, DeleteTestCall, WrongFile, OverEngineer, CompileError}.</constraints></field>
  <field name="per_language_accuracy" type="Map&lt;Lang, f32&gt;" required="true"><description>11-language breakdown.</description><constraints>Keys = supported languages from constitution &lt;tech_stack&gt; (rust, python, javascript, typescript, go, java, c, cpp, csharp, ruby, php).</constraints></field>
  <field name="predictor_redundancy_pairwise_mi" type="f32" required="true"><description>Avg normalized pairwise MI across panel slots.</description><constraints>In [0, 1].</constraints></field>
  <field name="gtau_pass_rate" type="f32" required="true"><description>Fraction of holdout panels passing TCT Gτ guard.</description><constraints>In [0, 1]; 1.0 fallback when Phase 4b not shipped.</constraints></field>
  <field name="generic_only_warning" type="Option&lt;String&gt;" required="false"><description>Populated when per-category or per-language gap &gt; 0.10.</description><constraints>Optional.</constraints></field>
  <field name="phase3_dod_passed" type="Option&lt;bool&gt;" required="false"><description>Set on final holdout report; true iff agreement ≥ 0.75.</description><constraints>Final-step only.</constraints></field>
  <field name="timestamp_iso8601" type="String" required="true"><description>ISO 8601 UTC.</description><constraints>Non-null.</constraints></field>
</entity>

<entity name="EpochSummary">
  <field name="epoch" type="u32" required="true"><description>Epoch index.</description><constraints>Primary key.</constraints></field>
  <field name="mean_l_step" type="f32" required="true"><description>Average L_step over epoch.</description><constraints>In [0, 1].</constraints></field>
  <field name="mean_delta_p" type="f32" required="true"><description>Average ΔP.</description><constraints>In [0, 1].</constraints></field>
  <field name="mean_delta_k" type="f32" required="true"><description>Average ΔK.</description><constraints>In [0, 1].</constraints></field>
  <field name="mean_delta_omega" type="f32" required="true"><description>Average ΔΩ.</description><constraints>In [0, 1].</constraints></field>
  <field name="mean_delta_xi" type="f32" required="true"><description>Average ΔΞ.</description><constraints>In [0, 1].</constraints></field>
  <field name="holdout_agreement" type="f32" required="true"><description>Last holdout agreement this epoch.</description><constraints>In [0, 1].</constraints></field>
  <field name="best_category" type="MutationCategory" required="true"><description>Highest per-category accuracy.</description><constraints>One of 8 categories.</constraints></field>
  <field name="worst_category" type="MutationCategory" required="true"><description>Lowest per-category accuracy.</description><constraints>One of 8 categories.</constraints></field>
  <field name="best_language" type="Lang" required="true"><description>Highest per-language accuracy.</description><constraints>One of 11 languages.</constraints></field>
  <field name="worst_language" type="Lang" required="true"><description>Lowest per-language accuracy.</description><constraints>One of 11 languages.</constraints></field>
  <field name="total_steps_this_epoch" type="u64" required="true"><description>Total steps including skipped.</description><constraints>Non-negative.</constraints></field>
  <field name="skipped_steps_this_epoch" type="u64" required="true"><description>Steps with skip_reason populated.</description><constraints>≤ total_steps_this_epoch.</constraints></field>
  <field name="parent_witness_hash" type="String" required="true"><description>Hash of prior epoch entry.</description><constraints>Genesis = 32 bytes of 0x00 for epoch 0.</constraints></field>
  <field name="self_hash" type="String" required="true"><description>SHAKE-256-73 over canonicalized JSON.</description><constraints>Non-null.</constraints></field>
</entity>

</data_requirements>

<non_functional_requirements>

<nfr id="NFR-TRAIN-01" category="performance">
  <description>Training walltime: complete 10 epochs on base 2,400-example corpus on RTX 5090 in &lt; 12 hours wall-clock time.</description>
  <metric>elapsed_wall_clock_seconds &lt; 43,200; per-step training latency p50 &lt; 50ms (predictor forward + 4 deltas + cert write + AdamW step).</metric>
  <priority>must</priority>
</nfr>

<nfr id="NFR-TRAIN-02" category="reliability">
  <description>Per-step training cert chain integrity verifiable end-to-end on a 100,000-step run.</description>
  <metric>Replay completes within 60 seconds; zero parent_hash mismatches under non-tampered conditions.</metric>
  <priority>must</priority>
</nfr>

<nfr id="NFR-TRAIN-03" category="reliability">
  <description>Phase 3 DoD: holdout `predicted_oracle_pass`-vs-actual agreement reaches ≥ 75% at end of training.</description>
  <metric>HoldoutReport at final step has prediction_oracle_agreement ≥ 0.75.</metric>
  <priority>must</priority>
</nfr>

<nfr id="NFR-TRAIN-04" category="reliability">
  <description>VRAM budget: training fits within 29 GB on RTX 5090 (per constitution &lt;performance_budgets&gt; + doc 12 §4).</description>
  <metric>Peak VRAM &lt; 29 GB during forward+backward+optimizer step; lazy-load learner-state embedders E15-E21 if budget tightens.</metric>
  <priority>must</priority>
</nfr>

<nfr id="NFR-TRAIN-05" category="reliability">
  <description>Restart safety: trainer can resume from last.safetensors without progress loss beyond K-1 steps.</description>
  <metric>--resume produces identical step N+1 to non-interrupted run given same seed.</metric>
  <priority>should</priority>
</nfr>

<nfr id="NFR-TRAIN-06" category="security">
  <description>Per constitution &lt;security_requirements&gt; SEC-MEJEPA-02: embedder weights load from pinned SHA-256 digests in models_config.toml; refuse to load if digest mismatches.</description>
  <metric>Trainer initialization aborts with structured error if any of 21 embedder weight files fails digest check.</metric>
  <priority>must</priority>
</nfr>

<nfr id="NFR-TRAIN-07" category="security">
  <description>Per constitution &lt;security_requirements&gt; SEC-MEJEPA-03: per-epoch witness-chain entries signed with ed25519 keypair; private key never leaves operator hardware.</description>
  <metric>Each EpochSummary entry includes ed25519 signature; signature verification chain covers full per-epoch run.</metric>
  <priority>must</priority>
</nfr>

<nfr id="NFR-TRAIN-08" category="security">
  <description>Per constitution &lt;security_requirements&gt; SEC-MEJEPA-07: FSV evidence files in /tmp/contextgraph-mejepa-train-fsv/ created with mode 0o600.</description>
  <metric>File mode bits == 0o600 on every FSV artifact.</metric>
  <priority>must</priority>
</nfr>

<nfr id="NFR-TRAIN-09" category="usability">
  <description>Determinism: same seed + same code_version + same corpus_sha + same embedder_versions produces byte-equal cert chain.</description>
  <metric>Two runs with identical inputs produce identical CF_MEJEPA_TRAIN_CERTS contents (modulo timestamp, which is excluded from canonical hash).</metric>
  <priority>should</priority>
</nfr>

</non_functional_requirements>

<dependencies>

<dependency type="internal">
  <name>SPEC-UTML / TECH-UTML</name>
  <description>Per-step L_step computation, sampling_weight, training cert schema. Phase 3 consumes these.</description>
  <impact>Without UTML factor computation, no per-step cert can be emitted.</impact>
</dependency>

<dependency type="internal">
  <name>SPEC-PHASE2-PREDICTOR</name>
  <description>Predictor architecture g_φ over f_θ + frozen target panel readback infrastructure.</description>
  <impact>Without predictor + target panel, no L_predict / L_full computable.</impact>
</dependency>

<dependency type="internal">
  <name>SPEC-INSTRUMENTS / SPEC-AST-CHUNKING</name>
  <description>Phase 1 frozen instrument substrate produces target panels.</description>
  <impact>Without frozen instruments, target_collapse cannot be 0 by construction.</impact>
</dependency>

<dependency type="internal">
  <name>SPEC-CORPUS</name>
  <description>Phase 0 mutation corpus + train/calibration/holdout split (Stoer-Wagner mincut). 2,400-example base corpus = 8 categories × 300 SWE-bench Lite tasks.</description>
  <impact>Without corpus, no batches; without holdout split, no convergence monitoring.</impact>
</dependency>

<dependency type="internal">
  <name>SPEC-TCT (Phase 4b cross-cutting)</name>
  <description>Constellation centroid + Gτ guard. δ·L_constellation_centroid loss term + gtau_pass_rate metric.</description>
  <impact>Term δ=0.0 fallback when Phase 4b not yet shipped (structurally inert, no error).</impact>
</dependency>

<dependency type="internal">
  <name>SPEC-EMBEDDERS (21 frozen content + learner-state)</name>
  <description>Aux heads consume embedder outputs; predictor's hidden state may be supplemented by E1-E14 features. E15-E21 learner-state embedders feed agent_state_score.</description>
  <impact>Without embedder ensemble, aux heads cannot calibrate; UTML §V.1 effective_plasticity intervention has no LoRA surface.</impact>
</dependency>

<dependency type="internal">
  <name>SPEC-WITNESS-CHAIN (context-graph-witness)</name>
  <description>SHAKE-256 73-byte witness primitive + ed25519 signing.</description>
  <impact>Per-step parent_hash → self_hash chain + per-epoch 73-byte entry require this.</impact>
</dependency>

<dependency type="internal">
  <name>crates/context-graph-embeddings/src/training/{trainer, optimizer, loss, evaluation, pipeline, distillation, lora, multitask}.rs (SHIPPED)</name>
  <description>AdamW + warmup + cosine + LoRA + InfoNCE + distillation + evaluation primitives reused by Phase 3.</description>
  <impact>Reuse-not-rebuild; saves ~3 weeks of greenfield implementation per doc 12 §6.</impact>
</dependency>

<dependency type="internal">
  <name>crates/context-graph-mincut (SHIPPED)</name>
  <description>Stoer-Wagner mincut for repo-atomic train/calibration/holdout split.</description>
  <impact>Required by holdout evaluation cadence.</impact>
</dependency>

<dependency type="external">
  <name>candle 0.9.x with sm_120a CUDA support via cudarc</name>
  <description>Pure-Rust ML framework for forward+backward on RTX 5090.</description>
  <impact>Without sm_120a support, training cannot run on RTX 5090.</impact>
</dependency>

<dependency type="external">
  <name>RocksDB 0.21.x</name>
  <description>CF_MEJEPA_TRAIN_CERTS, CF_MEJEPA_HOLDOUT_REPORTS, CF_MEJEPA_WEIGHT_BLOBS, CF_MEJEPA_PANELS storage.</description>
  <impact>Required for cert chain persistence + checkpoint deduplication.</impact>
</dependency>

</dependencies>

<out_of_scope>
  <item>Predictor architecture + frozen target panel readback — handled by SPEC-PHASE2-PREDICTOR.</item>
  <item>Conformal calibration + α threshold selection on calibration split — handled by SPEC-PHASE4-INFERENCE (Phase 3 produces, Phase 4 consumes).</item>
  <item>EWC++ Fisher matrix construction + drift detection + A/B/C retrain — handled by SPEC-PHASE5-SELF-HEALING (Phase 3 emits classifier verdicts + holdout reports for them to consume).</item>
  <item>Constellation centroid construction (Phase 4b) — handled by SPEC-TCT. Phase 3 consumes the centroid + Gτ guard.</item>
  <item>Phase 1 instrument substrate (E_AST/E_CFG/E_DataFlow/E_TypeGraph/E_Test/E_Trace/E_Diff/E_Witness/E_Oracle/E_Problem/E_CommitMsg/E_StaticAnalysis/E_Runtime/E_Reasoning) — handled by SPEC-INSTRUMENTS / TECH-INSTRUMENTS.</item>
  <item>Phase 0 mutation corpus generation + Docker oracle bridge — handled by SPEC-CORPUS / TECH-CORPUS.</item>
  <item>Phase 7 mejepa-shift-subscriber online observe() loop — Phase 3 produces best.safetensors + LoRA adapters that the subscriber loads.</item>
  <item>MCP tool surface (mejepa_verify, mejepa_predict_latest, mejepa_observe_shift) — handled by SPEC-PHASE7-MCP-TOOLS.</item>
</out_of_scope>

<test_plan>

<test_case id="TC-TRAIN-01" type="unit" req_ref="REQ-TRAIN-01">
  <description>AdamW LR schedule produces correct values at step=0, step=warmup_steps, step=total_steps.</description>
  <preconditions>warmup_steps=500, total_steps=3800, lr=5e-4.</preconditions>
  <inputs>step ∈ {0, 250, 500, 1000, 3800}</inputs>
  <expected_result>LR(0)≈0; LR(250)≈2.5e-4 (linear); LR(500)=5e-4 (peak); LR(1000)≈4.4e-4 (cosine descending); LR(3800)≈0.</expected_result>
  <priority>critical</priority>
</test_case>

<test_case id="TC-TRAIN-02" type="unit" req_ref="REQ-TRAIN-01">
  <description>Gradient clipping clamps when ‖g‖&gt;1.0.</description>
  <preconditions>max_grad_norm=1.0; synthetic gradient with ‖g‖=2.0.</preconditions>
  <inputs>g of norm 2.0</inputs>
  <expected_result>g_clipped of norm 1.0; pre-clip norm recorded in cert.grad_norm=2.0.</expected_result>
  <priority>critical</priority>
</test_case>

<test_case id="TC-TRAIN-03" type="unit" req_ref="REQ-TRAIN-02">
  <description>L_full equals weighted sum of all 13 component terms.</description>
  <preconditions>Synthetic batch with known per-term values.</preconditions>
  <inputs>L_predict=1.0, L_var=0.5, L_cov=0.1, L_inv=0.5, L_warn=2.0, L_runtime=1.5, L_rss=1.0, L_trace=0.8, L_hunk=0.6, L_adjacent=0.4, L_reasoning=0.2, L_overrides=0.5, L_const=0.3</inputs>
  <expected_result>L_full = 1 + 25·0.5 + 1·0.1 + 25·0.5 + 0.5·2 + 0.3·1.5 + 0.2·1 + 0.5·0.8 + 1·0.6 + 0.5·0.4 + 0.3·0.2 + 5·0.5 + 2·0.3 = 32.71.</expected_result>
  <priority>critical</priority>
</test_case>

<test_case id="TC-TRAIN-04" type="unit" req_ref="REQ-TRAIN-03">
  <description>TrainingCertificate emission produces deterministic byte-equal JSON given identical inputs.</description>
  <preconditions>Same step, factor inputs, sub-components, code_version.</preconditions>
  <inputs>2 identical cert constructions</inputs>
  <expected_result>Both produce byte-equal canonical JSON (sorted keys); self_hash identical; cert is reproducible.</expected_result>
  <priority>critical</priority>
</test_case>

<test_case id="TC-TRAIN-05" type="integration" req_ref="REQ-TRAIN-03">
  <description>100,000-step cert chain replay verifies all parent_hash links.</description>
  <preconditions>Synthetic 100k-step run; certs persisted to CF_MEJEPA_TRAIN_CERTS.</preconditions>
  <inputs>Replay tool reads certs in order</inputs>
  <expected_result>All 100,000 parent_hash links match recomputed sha256; replay completes within 60 seconds.</expected_result>
  <priority>critical</priority>
</test_case>

<test_case id="TC-TRAIN-06" type="integration" req_ref="REQ-TRAIN-03">
  <description>Tamper test: corrupt cert[N].body, verify cert[N+1].parent_witness_hash mismatch detected.</description>
  <preconditions>Synthetic 1,000-step run.</preconditions>
  <inputs>Modify cert[500].sampling_weight; replay.</inputs>
  <expected_result>MEJEPA_TRAIN_CERT_CHAIN_BROKEN at step 501; replay halts.</expected_result>
  <priority>critical</priority>
</test_case>

<test_case id="TC-TRAIN-07" type="unit" req_ref="REQ-TRAIN-04">
  <description>BatchSampler weight=0 → drop.</description>
  <preconditions>Example with L_step=0.</preconditions>
  <inputs>weight=0</inputs>
  <expected_result>Example NOT included in any batch this epoch.</expected_result>
  <priority>critical</priority>
</test_case>

<test_case id="TC-TRAIN-08" type="unit" req_ref="REQ-TRAIN-04">
  <description>BatchSampler operator_override → force-include.</description>
  <preconditions>Example with operator_override=true, L_step=0.5.</preconditions>
  <inputs>weight = 1.0 · 0.5 · 6 · 1.0 = 3.0</inputs>
  <expected_result>weight=3.0 &gt; 1.0; force-included in every batch this epoch.</expected_result>
  <priority>critical</priority>
</test_case>

<test_case id="TC-TRAIN-09" type="integration" req_ref="REQ-TRAIN-04">
  <description>BatchSampler 1-epoch dry-run on 100 synthetic examples confirms high-L_step appear more often.</description>
  <preconditions>100 examples with L_step linearly distributed in [0.05, 1.0].</preconditions>
  <inputs>1 epoch worth of batches</inputs>
  <expected_result>Top decile by weight appears ≥ 5× as often as bottom decile.</expected_result>
  <priority>high</priority>
</test_case>

<test_case id="TC-TRAIN-10" type="integration" req_ref="REQ-TRAIN-05">
  <description>Holdout evaluation at step 1,000 produces well-formed HoldoutReport.</description>
  <preconditions>Synthetic 240-example holdout split spanning 8 categories × 11 languages.</preconditions>
  <inputs>Predictor + frozen target instances; oracle verdicts.</inputs>
  <expected_result>Report contains all 6 metrics + per-category map (8 keys) + per-language map (11 keys); persisted to CF_MEJEPA_HOLDOUT_REPORTS[1000].</expected_result>
  <priority>critical</priority>
</test_case>

<test_case id="TC-TRAIN-11" type="integration" req_ref="REQ-TRAIN-05">
  <description>3 consecutive holdout regressions trigger halt + critical blocker.</description>
  <preconditions>Synthetic regression sequence: agreement 0.78 → 0.74 → 0.71 → 0.68.</preconditions>
  <inputs>HoldoutReports across 3 evaluation windows.</inputs>
  <expected_result>1st emits MEJEPA_TRAIN_HOLDOUT_REGRESSION (warn); 2nd emits warn; 3rd halts with critical blocker.</expected_result>
  <priority>critical</priority>
</test_case>

<test_case id="TC-TRAIN-12" type="integration" req_ref="REQ-TRAIN-05">
  <description>Per-language gap &gt; 0.10 triggers GENERIC-ONLY warning.</description>
  <preconditions>Synthetic holdout with python=0.85, rust=0.82, php=0.65 accuracy.</preconditions>
  <inputs>HoldoutReport computation</inputs>
  <expected_result>cert.generic_only_warning populated with "per_language_gap=0.20 between python(0.85) and php(0.65)".</expected_result>
  <priority>high</priority>
</test_case>

<test_case id="TC-TRAIN-13" type="unit" req_ref="REQ-TRAIN-06">
  <description>Counterfactual perturbation deterministic given (step, ε_idx).</description>
  <preconditions>seed=42, step=10, ε_idx=2.</preconditions>
  <inputs>2 invocations of unit-vector sampler</inputs>
  <expected_result>Identical perturbations.</expected_result>
  <priority>high</priority>
</test_case>

<test_case id="TC-TRAIN-14" type="integration" req_ref="REQ-TRAIN-08">
  <description>Distillation cycle at step 1,000 trains predictor's hidden state, leaves aux heads frozen.</description>
  <preconditions>Predictor + aux heads loaded; quality_gate=true.</preconditions>
  <inputs>50-batch distillation cycle</inputs>
  <expected_result>Aux head weights byte-identical pre/post cycle; predictor hidden-state weights changed; cert.distillation_cycle_id set; cert.distillation_skipped_count present.</expected_result>
  <priority>high</priority>
</test_case>

<test_case id="TC-TRAIN-15" type="integration" req_ref="REQ-TRAIN-09">
  <description>10-epoch run produces 10 chained per-epoch witness entries (730 bytes total).</description>
  <preconditions>Full Phase 3 dry-run.</preconditions>
  <inputs>End-to-end run + replay tool.</inputs>
  <expected_result>10 entries × 73 bytes; each parent_hash matches; replay completes &lt; 5 seconds.</expected_result>
  <priority>critical</priority>
</test_case>

<test_case id="TC-TRAIN-16" type="integration" req_ref="REQ-TRAIN-10">
  <description>Resume from --resume produces identical step N+1 to non-interrupted run.</description>
  <preconditions>2 runs with same seed; one interrupted at step 2,000 then resumed.</preconditions>
  <inputs>Compare step-2001 weights byte-for-byte.</inputs>
  <expected_result>Identical predictor + LoRA + AdamW state at step 2,001 across runs.</expected_result>
  <priority>high</priority>
</test_case>

<test_case id="TC-TRAIN-17" type="unit" req_ref="REQ-TRAIN-11">
  <description>LoRA mode produces base+delta separation; full-finetune produces unified weights.</description>
  <preconditions>Trainer initialization with both modes.</preconditions>
  <inputs>--full-finetune flag toggle</inputs>
  <expected_result>LoRA mode: lora_adapters.safetensors ~0.5 GB + base immutable. Full-finetune: full predictor weights in checkpoint.</expected_result>
  <priority>high</priority>
</test_case>

<test_case id="TC-TRAIN-18" type="integration" req_ref="REQ-TRAIN-12">
  <description>Synthetic NaN injection triggers MEJEPA_TRAIN_LOSS_NAN; 3 in 100 steps escalates to MEJEPA_TRAIN_GRAD_EXPLODE.</description>
  <preconditions>Inject NaN into gradient at steps 10, 25, 60.</preconditions>
  <inputs>Training run with injection</inputs>
  <expected_result>Step 10/25 emit MEJEPA_TRAIN_LOSS_NAN; step 60 emits MEJEPA_TRAIN_GRAD_EXPLODE; halt.</expected_result>
  <priority>critical</priority>
</test_case>

<test_case id="TC-TRAIN-19" type="e2e" req_ref="NFR-TRAIN-01">
  <description>End-to-end Phase 3 DoD verification on full 2,400-example corpus, RTX 5090.</description>
  <preconditions>Full Phase 0 corpus + Phase 1 frozen instruments + Phase 2 predictor.</preconditions>
  <inputs>cargo run --release -p context-graph-mejepa-train -- train --corpus ./data/mejepa-corpus --output ./data/mejepa-checkpoints --epochs 10 --batch-size 64</inputs>
  <expected_result>elapsed_wall_clock_seconds &lt; 43,200; final HoldoutReport.prediction_oracle_agreement ≥ 0.75; cert chain integrity verifiable end-to-end; per-epoch witness chain valid; ./data/mejepa-checkpoints/best.safetensors exists.</expected_result>
  <priority>critical</priority>
</test_case>

<test_case id="TC-TRAIN-20" type="e2e" req_ref="NFR-TRAIN-04">
  <description>VRAM budget check during training: peak &lt; 29 GB on RTX 5090.</description>
  <preconditions>Full Phase 3 run with all 21 embedders loaded.</preconditions>
  <inputs>nvidia-smi monitoring during run</inputs>
  <expected_result>Peak VRAM &lt; 29 GB; if exceeded, lazy-load E15-E21 path engaged per doc 12 §4.</expected_result>
  <priority>critical</priority>
</test_case>

<test_case id="TC-TRAIN-21" type="manual" req_ref="REQ-TRAIN-12">
  <description>FSV evidence files have mode 0o600 (per NFR-TRAIN-08).</description>
  <preconditions>Phase 3 run produces evidence to /tmp/contextgraph-mejepa-train-fsv/.</preconditions>
  <inputs>stat /tmp/contextgraph-mejepa-train-fsv/training-cert-chain.json</inputs>
  <expected_result>Mode bits 0o600 on all evidence files.</expected_result>
  <priority>must</priority>
</test_case>

</test_plan>

<open_questions>

<question id="Q-TRAIN-01" status="open" assignee="operator + Phase 4b spec author">
  <text>When Phase 4b TCT constellation centroid not yet shipped, should δ default to 0.0 (term inert) or should training error out?</text>
  <context>This spec assumes δ=0.0 fallback so Phase 3 can ship before Phase 4b. Operator may prefer hard-fail to enforce ordering. Per current "structurally inert" approach, training works without TCT but loses the constellation supervision signal until Phase 4b is wired.</context>
  <resolution>(pending)</resolution>
</question>

<question id="Q-TRAIN-02" status="open" assignee="operator + Phase 7 subscriber spec author">
  <text>Per-step certs from periodic full retrain (Phase 5 doc 09 §5.4) — do they continue the same chain as the original Phase 3 run, or start a new chain rooted at the prior best.safetensors's final cert hash?</text>
  <context>Operator audit replay needs a clear convention. Continuing the chain preserves history; new-chain anchors retraining as a separate event. Currently the spec assumes continuation (parent_hash links to last cert of prior run); operator may prefer otherwise.</context>
  <resolution>(pending)</resolution>
</question>

<question id="Q-TRAIN-03" status="open" assignee="operator">
  <text>Should --full-finetune flag also disable LoRA on aux heads, or only on the predictor?</text>
  <context>doc 12 §6 implies LoRA is the parameter-efficient surface for all trainable params. --full-finetune disabling LoRA on aux heads simplifies semantics but loses the ability to mix-and-match.</context>
  <resolution>(pending)</resolution>
</question>

<question id="Q-TRAIN-04" status="open" assignee="Phase 5 spec author">
  <text>What's the contract between Phase 3's MEJEPA_TRAIN_HOLDOUT_REGRESSION and Phase 5's drift_detected? Should holdout regression auto-trigger A/B/C, or wait for Phase 5's separate rolling-window drift detector to fire?</text>
  <context>Two regression detectors operating concurrently could double-fire interventions. Cleaner contract: Phase 3 emits the alert + classifier verdict; Phase 5 owns the intervention scheduler.</context>
  <resolution>(pending)</resolution>
</question>

</open_questions>

<glossary>

<term name="L_full">Multi-objective composite training loss summing 13 weighted terms per doc 09 §3.7.1 + δ·L_constellation_centroid.</term>
<term name="L_predict">JEPA-foundational latent prediction loss: cosine error between predictor output and frozen target panel.</term>
<term name="L_step">UTML v1.0 four-factor product ΔP × ΔK × ΔΩ × ΔΞ per SPEC-UTML; computed alongside L_full but used for sampling_weight + classifier, not as gradient signal.</term>
<term name="VICReg">Variance + Invariance + Covariance Regularization (Bardes 2022); predictor-side anti-collapse defense.</term>
<term name="AdamW">Decoupled-weight-decay variant of Adam (Loshchilov &amp; Hutter 2019); reused from crates/context-graph-embeddings/src/training/optimizer.rs.</term>
<term name="cosine LR schedule">Standard transformer training schedule: linear warmup over first warmup_steps, then cosine decay over remaining steps.</term>
<term name="bf16 mixed precision">Forward/backward in bfloat16; master weights + optimizer state (m, v) in fp32.</term>
<term name="gradient clipping">‖g‖ ≤ max_grad_norm scaling: g ← g · min(1, max_norm/‖g‖).</term>
<term name="TrainingCertificate">Per-step audit record persisted to CF_MEJEPA_TRAIN_CERTS; chained via parent_witness_hash → self_hash; physical M(t) per UTML §XII.</term>
<term name="BatchSampler">Sampling component that draws batches proportional to sampling_weight = base · L_step · (1+5·override) · 0.995^age.</term>
<term name="HoldoutReport">Per-evaluation-cadence audit record persisted to CF_MEJEPA_HOLDOUT_REPORTS; tracks 6 metrics including agreement, conformal coverage, per-category accuracy, per-language accuracy, predictor_redundancy, gtau_pass_rate.</term>
<term name="counterfactual training">For each panel, sample K=4 perturbations with ε ∈ {0.01, 0.05, 0.1, 0.2}, predict each, supervise against frozen-target readback; per doc 09 §3.7.3.</term>
<term name="cross-task transfer">In mini-batch sampler, when training on task T, with prob 0.3 sample from related_tasks(T) where cosine(E_Diff(T), E_Diff(T')) &gt; 0.7; per doc 09 §3.7.5.</term>
<term name="aux-head distillation">Every 1,000 steps, distill aux-head predictions back into predictor's hidden state via frozen-target distillation; per doc 09 §3.7.6.</term>
<term name="EpochSummary">73-byte SHAKE-256 per-epoch witness entry; chain provides compact end-to-end audit (10 epochs × 73 bytes).</term>
<term name="LoRA">Low-Rank Adaptation (Hu et al. 2021); rank=32 default per doc 12 §6; reused from crates/context-graph-embeddings/src/training/lora.rs.</term>
<term name="GENERIC-ONLY">Constitution &lt;anti_patterns&gt; rule: no per-task overfit; every primitive must generalize across all 300 SWE-bench Lite tasks AND all 11 supported languages.</term>
<term name="best.safetensors">Atomically-promoted checkpoint with highest holdout agreement to date; UT-03 (Phase 4 inference) loads this.</term>
<term name="last.safetensors">Most recent checkpoint; UT-01 uses for --resume; updated atomically on every checkpoint.</term>
<term name="phase3_dod_passed">Boolean indicating final HoldoutReport.prediction_oracle_agreement ≥ 0.75; populated only on final-step HoldoutReport.</term>

</glossary>

</functional_spec>
```

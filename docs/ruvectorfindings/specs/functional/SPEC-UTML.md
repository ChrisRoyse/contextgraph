# Functional Specification — UTML v1.0 Four-Factor Learning Formula

> **📊 2026-05-10 STATUS:** PARTIAL — see ../implementation-plan.md for the full Phase 3 status.

> **Level 2 (functional, *what* to build).** Subordinate to `specs/constitution.md` and the `<utml_v1_invariants>` block therein. When this functional spec conflicts with the constitution, the constitution wins. Implementation details (exact Rust signatures, file layout, persistence schema) live in `specs/technical/TECH-UTML.md`.

```xml
<functional_spec id="SPEC-UTML" version="1.0">

<metadata>
  <title>UTML v1.0 Four-Factor Learning Formula (L_step = ΔP × ΔK × ΔΩ × ΔΞ)</title>
  <status>partial</status>
  <owner>ME-JEPA Training Team / Phase 3 (cross-cutting)</owner>
  <created_date>2026-05-09</created_date>
  <last_updated>2026-05-10</last_updated>
  <related_specs>
    <spec_ref>SPEC-PHASE3-TRAINING (training loop host)</spec_ref>
    <spec_ref>SPEC-PHASE2-PREDICTOR (produces predicted latent)</spec_ref>
    <spec_ref>SPEC-PHASE1-INSTRUMENTS (produces frozen target panel)</spec_ref>
    <spec_ref>SPEC-PHASE4-CONFORMAL (consumes ΔΩ_mean, ΔΞ_mean for confidence)</spec_ref>
    <spec_ref>SPEC-PHASE4B-TCT (constellation_violation_rate source)</spec_ref>
    <spec_ref>SPEC-PHASE5-SELF-HEALING (consumes UTML cycle classifier output)</spec_ref>
    <spec_ref>SPEC-WITNESS-CHAIN (per-step training-cert hash chain)</spec_ref>
  </related_specs>
  <source_documents>
    <source type="design_doc">docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md (canonical formula)</source>
    <source type="design_doc">docs/ruvectorfindings/10_MEJEPA_PIVOT_2026_05_09.md §B, §D (pivot architecture + UTML cycle)</source>
    <source type="design_doc">docs/ruvectorfindings/12_EMBEDDER_INTEGRATION.md §5.1, §5.2, §5.3 (cross-embedder ΔK/ΔΞ; E17 agent_state_score)</source>
    <source type="design_doc">docs2/TheoryOfMachineLearning.md (UTML v1.0 source theory; Royse 2026 §I-§XIV)</source>
    <source type="constitution">docs/ruvectorfindings/specs/constitution.md (utml_v1_invariants block)</source>
  </source_documents>
</metadata>

<overview>
The UTML v1.0 Four-Factor Learning Formula is the per-step learning-efficiency signal that gates,
classifies, weights, and persists every gradient update inside the ME-JEPA-Code training loop. It
is **not a loss function** — it is a multiplicative diagnostic computed *alongside* the loss that
tells the trainer (a) whether the step is healthy, (b) which substrate factor is degraded, (c) how
much sampling weight the example deserves, and (d) what the per-step training certificate must
record for post-hoc audit.

The formula is `L_step = ΔP × ΔK × ΔΩ × ΔΞ`, all four factors clamped to `[0, 1]`. Multiplication is
non-negotiable: it enforces that any single factor at zero zeros the entire learning signal
regardless of how good the others look. ΔΞ.target_collapse is locked at zero by construction for
ME-JEPA (the target panel is produced by frozen externally-defined instruments with zero trainable
parameters; the constitution's `<utml_v1_invariants>` declares any nonzero observation a critical
bug — an instrument has accidentally become trainable).

This spec defines *what* the formula computes, *what* its components mean, *what* edge cases must
be handled, *what* the per-step training certificate must contain, and *how* downstream surfaces
(sampling weight, consolidation integral M(t), state-transfer T, calibrated confidence, reward,
pruning rule, self-healing cycle classifier) consume the four factors. It does not specify Rust
signatures or file paths — those are in `TECH-UTML.md`.

The formula is **cross-cutting Phase 3 work**: the four-factor computation lives inside the Phase 3
training crate, but every other phase either feeds inputs to it (Phases 1, 2, 4, 4b) or consumes its
outputs (Phases 4, 5, 7). This spec is the contract between those phases.
</overview>

<user_types>
  <user_type id="UT-01" name="Phase 3 Trainer">
    <description>The training-loop driver inside `crates/context-graph-mejepa-train`. Calls the four `compute_delta_*` functions every step, multiplies the results, attaches them to the per-step training certificate, and uses L_step to drive sampling weight and the UTML cycle classifier.</description>
    <permissions>read predictor outputs, target panel, gradients, optimizer state, EMA state, Fisher matrix; write per-step certs; emit classifier verdicts to Phase 5</permissions>
    <goals>compute mathematically correct four-factor product per step with bounded latency; never silently degrade a factor; persist auditable evidence of every step</goals>
  </user_type>

  <user_type id="UT-02" name="Phase 5 Self-Healing Controller">
    <description>The drift / A/B/C / EWC++ controller. Consumes the UTML cycle classifier output (Stuck / Optimizer-Dysregulated / Latent-Collapsing / Confused / Optimal / Boring / Dissipating) and selects the appropriate intervention (continual backprop, LR warmup, VICReg coefficient bump, constellation re-fit, curriculum prune, etc.).</description>
    <permissions>read L_step + four-factor history; trigger A/B/C retrain; pause training; surface blockers</permissions>
    <goals>regulate ΔΩ and ΔΞ before adjusting ΔP / ΔK per UTML §IX coaching order; never prune curriculum items under bad ΔΩ or bad ΔΞ</goals>
  </user_type>

  <user_type id="UT-03" name="Phase 4 Inference / Verify Surface">
    <description>The `MeJepaCompiler::verify(patch)` API and the `mcp__cgreality__mejepa_verify` MCP tool. Reads runtime-observed ΔΩ_mean and ΔΞ_mean to gate `calibrated_confidence` per UTML §XIV; refuses to predict when ΔΩ_mean × ΔΞ_mean × raw_conf falls below the [0.10, 0.95] floor.</description>
    <permissions>read aggregated ΔΩ_mean / ΔΞ_mean over recent training window; read conformal intervals from Phase 4 calibration</permissions>
    <goals>fail-closed on degraded substrate; never report a confident prediction in a collapsed latent or chronically unstable optimizer</goals>
  </user_type>

  <user_type id="UT-04" name="Auditor / Operator">
    <description>The human (operator) and any post-hoc audit tool that replays the per-step training certificate chain from `CF_MEJEPA_TRAIN_CERTS`. Verifies every step's four-factor breakdown, sampling weight, grad norm, drift coverage, and Merkle-root hash.</description>
    <permissions>read CF_MEJEPA_TRAIN_CERTS; replay witness chain; verify hash links; verify ΔΞ.target_collapse == 0 for every step</permissions>
    <goals>byte-equal-readback verification of every training step; immediate detection of any nonzero ΔΞ.target_collapse (critical bug indicator)</goals>
  </user_type>

  <user_type id="UT-05" name="Phase 7 Shift Subscriber">
    <description>The `mejepa-shift-subscriber` Tokio task. For each tailed `(panel_t01, panel_t2)` pair from the cgreality-shift-log, computes L_step locally and writes both the live prediction (always) and the observed update (only when L_step > 0.05) into RocksDB.</description>
    <permissions>read shift log; read witness chain; call observe(); write CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_TRAIN_CERTS</permissions>
    <goals>p50 &lt; 5s, p99 &lt; 30s end-to-end shift → cert per constitution &lt;performance_budgets&gt;; never accept a shift whose L_step is below the sampling-weight gate</goals>
  </user_type>
</user_types>

<user_stories>

<story id="US-UTML-01" priority="must-have">
  <narrative>
    <as_a>Phase 3 Trainer (UT-01)</as_a>
    <i_want_to>compute L_step = ΔP × ΔK × ΔΩ × ΔΞ deterministically per step with all four factors clamped to [0, 1]</i_want_to>
    <so_that>the multiplicative-gate property is preserved: any single factor at zero zeros the entire learning signal, and no addition-style compensation can rescue a collapsed factor</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-UTML-01-01">
      <given>Four factor inputs each in [0.0, 1.0] (post-clamp)</given>
      <when>compute_l_step is called</when>
      <then>Returns the multiplicative product as f32 in [0.0, 1.0]; no NaN, no Inf, no negative output; byte-equal under same inputs + same code_version (determinism per constitution &lt;coding_standards&gt;)</then>
    </criterion>
    <criterion id="AC-UTML-01-02">
      <given>Any single factor equals 0.0 exactly</given>
      <when>compute_l_step is called</when>
      <then>Returns 0.0 exactly (no float-error drift); the corresponding example is excluded from the next batch's sampled set</then>
    </criterion>
    <criterion id="AC-UTML-01-03">
      <given>Any factor input outside [0.0, 1.0] (NaN, Inf, negative, &gt;1.0)</given>
      <when>compute_l_step is called</when>
      <then>Fail-closed with structured error (SCREAMING_SNAKE code per constitution &lt;failure_taxonomy&gt;); do NOT silently clamp or substitute defaults; do NOT call unwrap_or(0.5)</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-UTML-02" priority="must-have">
  <narrative>
    <as_a>Phase 3 Trainer (UT-01)</as_a>
    <i_want_to>compute ΔP from cosine error between predicted latent and frozen-instrument target panel, normalized by gradient SNR, with imagination discount γ ≈ 0.7 and exploration bonus +0.5</i_want_to>
    <so_that>predictive error reflects the JEPA-foundational property of latent-space prediction (LeCun 2022; Assran 2023; Bardes 2024) and active-learning examples (wide conformal interval) get appropriate up-weighting</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-UTML-02-01">
      <given>Predicted latent g_φ(f_θ(panel_t01)) and frozen target f_target(panel_t2) of identical dimensionality</given>
      <when>compute_delta_p is called with SNR(t), exploration_bonus(t), γ, optional imagined latent</when>
      <then>Returns clamp_[0,1](max(ΔP_real, γ · ΔP_imagined) + 0.5 · exploration_bonus) / SNR(t) where ΔP_real = clamp(1 − cosine(predicted, target), 0, 1) and ΔP_imagined = clamp(1 − cosine(imagined, target), 0, 1) when imagined is Some, else 0</then>
    </criterion>
    <criterion id="AC-UTML-02-02">
      <given>Per-chunk ΔP across N chunks (per doc 14 AST-aware chunking)</given>
      <when>compute_delta_p_aggregate is called with strategy ∈ {Mean, Max, TopKMean, EntityWeighted}</when>
      <then>Default Mean for stability; Max selected when constellation guard (Phase 4b Gτ) flagged a violating chunk; aggregate result clamped to [0, 1]</then>
    </criterion>
    <criterion id="AC-UTML-02-03">
      <given>ΔP_real == 0.0 (perfect prediction)</given>
      <when>compute_l_step propagates</when>
      <then>L_step == 0.0; example is skipped per §11.2 edge-case audit; batch_indices excludes this example</then>
    </criterion>
    <criterion id="AC-UTML-02-04">
      <given>SNR(t) very low (high gradient noise, e.g., &lt; 0.1)</given>
      <when>compute_delta_p divides by SNR</when>
      <then>ΔP shrinks correspondingly because the gradient is uninformative (Liu et al. 2020); no divide-by-zero — SNR floor enforced via configured minimum, returning structured error if SNR falls below floor</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-UTML-03" priority="must-have">
  <narrative>
    <as_a>Phase 3 Trainer (UT-01)</as_a>
    <i_want_to>compute ΔK as a four-term weighted sum of cos_align (weight 0.5), 1 − fisher_violation (0.3), 1 − ECE (0.15), and embedder_coherence (0.05)</i_want_to>
    <so_that>knowledge coherence captures gradient alignment with EMA history (NTK alignment, Atanasov 2021), Fisher-weighted protection of consolidated parameters (EWC++, Kirkpatrick 2017), calibration health (Goodhart early-indicator, Gao 2022), and cross-embedder neighbor agreement (per doc 12 §5.1)</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-UTML-03-01">
      <given>Current batch gradient g_t, EMA gradient ḡ_prior, Fisher matrix F_prior, current ECE, embedder_coherence ∈ [0, 1]</given>
      <when>compute_delta_k is called</when>
      <then>Returns clamp_[0,1](0.5·cos_align + 0.3·(1 − fisher_violation) + 0.15·(1 − ECE) + 0.05·embedder_coherence); weights sum to exactly 1.0; per doc 12 §5.1 the embedder_coherence term defaults to 0.5 (neutral) for the first 1,000 corpus entries</then>
    </criterion>
    <criterion id="AC-UTML-03-02">
      <given>cos_align(g_t, ḡ_prior) &lt; 0 (gradient is destructive of prior knowledge — catastrophic forgetting signature)</given>
      <when>compute_delta_k is called</when>
      <then>ΔK collapses toward 0; if ΔK == 0 exactly, step is aborted and rolled back per §11.2 edge-case audit; CF_MEJEPA_DRIFT_STATE updated</then>
    </criterion>
    <criterion id="AC-UTML-03-03">
      <given>embedder_coherence computation across 14 content embedders (E1-E14) on a candidate panel</given>
      <when>cross-embedder Jaccard NN-set agreement is measured</when>
      <then>Strong agreement → high coherence (panel is recognized in all embedder feature spaces); disagreement → low coherence (potential OOD / structurally unusual change); per doc 12 §5.1</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-UTML-04" priority="must-have">
  <narrative>
    <as_a>Phase 3 Trainer (UT-01)</as_a>
    <i_want_to>compute ΔΩ as a geometric (multiplicative) product of effective_plasticity^0.4, landscape_health^0.4, stability_floor^0.1, and agent_state_score^0.1</i_want_to>
    <so_that>optimizer-state health captures Sutton 2024 plasticity loss + SAM-style landscape sharpness + the hard NaN/Goodhart gate + the E17 AffectText agent-state probe (per doc 12 §5.3); any subcomponent at 0 forces ΔΩ = 0 (geometric weighting non-negotiable)</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-UTML-04-01">
      <given>effective_plasticity ∈ [0,1] (from ReLU dormancy fraction + gradient-covariance effective rank + per-parameter GSNR distribution)</given>
      <when>compute_delta_omega is called</when>
      <then>Returns plasticity^0.4 · landscape^0.4 · stability_floor^0.1 · agent_state^0.1 clamped to [0, 1]; geometric weighting per UTML §V; weights sum to exactly 1.0</then>
    </criterion>
    <criterion id="AC-UTML-04-02">
      <given>landscape_health = sigmoid(-λ_max(H_t)) · inverted_U(‖g_t‖)</given>
      <when>Hessian top-eigenvalue λ_max is large (sharp minimum) OR ‖g_t‖ is in vanishing/exploding band</when>
      <then>landscape_health drops; ΔΩ shrinks correspondingly; SAM-style flat-minimum bias preserved</then>
    </criterion>
    <criterion id="AC-UTML-04-03">
      <given>stability_floor in {1.0 healthy, 0.4 grad-norm warning, 0.1 validation rising, 0.0 NaN/Inf or Goodhart catastrophe}</given>
      <when>any failure mode triggers stability_floor = 0.0</when>
      <then>ΔΩ == 0; step abort + alert; CF_MEJEPA_TRAIN_CERTS records skip reason; per §11.2 edge-case audit</then>
    </criterion>
    <criterion id="AC-UTML-04-04">
      <given>agent_state_score = E17(conversation_transcript_window).valence · E17(...).arousal_inverted_U from doc 12 §5.3</given>
      <when>E17 AffectText output is unavailable (no recent transcript window)</when>
      <then>agent_state_score defaults to neutral 0.7 (not 1.0 — neutral state is mid-band, per doc 12 §5.3); never silently substitute 1.0</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-UTML-05" priority="must-have">
  <narrative>
    <as_a>Phase 3 Trainer (UT-01)</as_a>
    <i_want_to>compute ΔΞ as (1 − target_collapse)^0.5 · (1 − predictor_redundancy)^0.3 · (1 − constellation_violation_rate)^0.2 with target_collapse asserted == 0.0 by construction for ME-JEPA</i_want_to>
    <so_that>representational non-collapse is enforced; the structural collapse-immunity property of frozen-instrument target panels is *measurable* per step; any nonzero target_collapse observation is treated as a critical bug per the constitution &lt;utml_v1_invariants&gt;</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-UTML-05-01">
      <given>ME-JEPA's target panel produced by frozen externally-defined instruments (zero trainable parameters)</given>
      <when>compute_delta_xi is called with target_collapse == 0.0, predictor_redundancy ∈ [0,1], constellation_violation_rate ∈ [0,1]</when>
      <then>Returns 1.0^0.5 · (1 − redundancy)^0.3 · (1 − violation_rate)^0.2; first factor locked at 1.0 always; assert_eq!(target_collapse, 0.0) MUST be enforced inside compute_delta_xi for ME-JEPA</then>
    </criterion>
    <criterion id="AC-UTML-05-02">
      <given>target_collapse &gt; 0.0 ever observed</given>
      <when>compute_delta_xi is called</when>
      <then>FAIL-CLOSED with critical-severity error (SCREAMING_SNAKE code: MEJEPA_INSTRUMENT_GRADIENT_LEAK or equivalent); halt training; surface to operator; an instrument has accidentally become trainable — fix at the trait level per §11.2 edge-case audit and constitution &lt;anti_patterns&gt;</then>
    </criterion>
    <criterion id="AC-UTML-05-03">
      <given>predictor_redundancy &gt; 0.5 (panel slots collapsing into each other)</given>
      <when>periodic pairwise-MI audit runs</when>
      <then>Trigger Phase 5 self-healing: bump VICReg variance/covariance coefficient, refit predictor; per §11.2 edge-case audit</then>
    </criterion>
    <criterion id="AC-UTML-05-04">
      <given>constellation_violation_rate from Phase 4b Gτ guard rejections</given>
      <when>recent rolling window of guard decisions has &gt;5% rejection rate</when>
      <then>ΔΞ.constellation_violation_rate reflects fraction; ΔΞ shrinks; trigger constellation re-fit per Section D self-healing protocol</then>
    </criterion>
    <criterion id="AC-UTML-05-05">
      <given>Auditor (UT-04) replays CF_MEJEPA_TRAIN_CERTS for any step</given>
      <when>verifying ΔΞ components</when>
      <then>delta_xi_components.target_collapse == 0.0 for EVERY step; any nonzero value is a CRITICAL bug indicator and surfaces a high-severity blocker</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-UTML-06" priority="must-have">
  <narrative>
    <as_a>Phase 3 Trainer (UT-01) and Phase 7 Shift Subscriber (UT-05)</as_a>
    <i_want_to>compute per-example sampling_weight = base_weight · L_step(ex) · (1 + 5·operator_override) · decay^age_in_days with base_weight=1.0, decay=0.995 (~140-day half-life)</i_want_to>
    <so_that>hard-example mining (high L_step) + gold-standard up-weighting (operator override) + recency-weighted curriculum is integrated into the doc 09 §3.7.2 sampling reweighting mechanism, replacing the previous (1 + 2·residual_error_z_score) term</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-UTML-06-01">
      <given>L_step(ex) ∈ [0, 1], operator_override_flag ∈ {true, false}, age_in_days ≥ 0.0</given>
      <when>sampling_weight is computed</when>
      <then>Returns base_weight · L_step · (1 + 5·override) · 0.995^age_in_days; non-negative; not clamped to [0, 1] (override can push above 1.0)</then>
    </criterion>
    <criterion id="AC-UTML-06-02">
      <given>sampling_weight &lt; 0.05</given>
      <when>BatchSampler reads weights</when>
      <then>Example DROPS from current epoch (skipped entirely); not included in next batch's sampled set</then>
    </criterion>
    <criterion id="AC-UTML-06-03">
      <given>sampling_weight &gt; 1.0</given>
      <when>BatchSampler reads weights</when>
      <then>Example FORCE-INCLUDED in current epoch (gold-standard guarantee); cannot be skipped by stochastic sampler</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-UTML-07" priority="must-have">
  <narrative>
    <as_a>Phase 3 Trainer (UT-01) and Auditor (UT-04)</as_a>
    <i_want_to>persist a per-step training certificate in CF_MEJEPA_TRAIN_CERTS keyed by step containing all four factors, all components, sampling_weight, grad_norm, drift_coverage, and a Merkle root that hash-chains to the parent witness</i_want_to>
    <so_that>every gradient update is byte-equal-readback verifiable post-hoc; the chain of parent_witness_hash → self_hash makes the full training run an auditable log; this IS the physical M(t) consolidation tensor per UTML §XII</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-UTML-07-01">
      <given>Step N completes</given>
      <when>training certificate is emitted</when>
      <then>JSON written to CF_MEJEPA_TRAIN_CERTS[N] with EXACT field set: step, loss, delta_p, delta_k, delta_omega, delta_xi, L_step, delta_omega_components{effective_plasticity, landscape_health, stability_floor, agent_state_score}, delta_xi_components{target_collapse, predictor_redundancy, constellation_violation_rate}, delta_k_components{cos_align, fisher_violation, ece, embedder_coherence}, sampling_weight, grad_norm, drift_coverage, parent_witness_hash, self_hash, merkle_root, code_version, corpus_sha, embedder_versions, frozen_at, timestamp_iso8601</then>
    </criterion>
    <criterion id="AC-UTML-07-02">
      <given>Auditor replays cert at step N</given>
      <when>verifying chain integrity</when>
      <then>cert[N].parent_witness_hash == sha256(cert[N-1].self_hash || cert[N-1].body); any mismatch raises MEJEPA_OBSERVE_INTEGRITY_VIOLATION per constitution &lt;failure_taxonomy&gt;</then>
    </criterion>
    <criterion id="AC-UTML-07-03">
      <given>Step was skipped (ΔP=0 or ΔK=0 or ΔΩ=0)</given>
      <when>cert is emitted</when>
      <then>cert.skip_reason populated with which factor zeroed; cert.sampling_weight == 0.0; cert.grad_norm reflects pre-skip computed value; chain link still intact</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-UTML-08" priority="must-have">
  <narrative>
    <as_a>Phase 5 Self-Healing Controller (UT-02)</as_a>
    <i_want_to>classify the current training state from L_step, ΔΩ, ΔΞ, and the consolidation gain ΔM/mean(L_step) into one of seven states: Stuck, Optimizer-Dysregulated, Latent-Collapsing, Confused, Optimal, Boring/Mastery, Dissipating</i_want_to>
    <so_that>UTML §IX coaching order is enforced — regulate ΔΩ and ΔΞ before adjusting ΔP/ΔK; never prune a curriculum item under bad ΔΩ or bad ΔΞ; surface the right intervention (continual backprop / VICReg bump / constellation re-fit / curriculum prune)</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-UTML-08-01">
      <given>L_step &lt; 0.1 for N consecutive steps AND ΔΩ &gt; 0.4 throughout AND ΔΞ &gt; 0.4 throughout</given>
      <when>classifier evaluates window</when>
      <then>Returns Stuck; recommends curriculum-prune; ALL THREE signals must agree per UTML §X pruning rule</then>
    </criterion>
    <criterion id="AC-UTML-08-02">
      <given>L_step &lt; 0.1 AND ΔΩ &lt; 0.4 (regardless of ΔΞ)</given>
      <when>classifier evaluates window</when>
      <then>Returns Optimizer-Dysregulated; recommends regulating ΔΩ FIRST (continual backprop, LR warmup, gradient clipping); curriculum is NOT pruned</then>
    </criterion>
    <criterion id="AC-UTML-08-03">
      <given>L_step &lt; 0.1 AND ΔΞ &lt; 0.4 (regardless of ΔΩ)</given>
      <when>classifier evaluates window</when>
      <then>Returns Latent-Collapsing; recommends regulating ΔΞ FIRST (VICReg coefficient bump, constellation re-fit); curriculum is NOT pruned</then>
    </criterion>
    <criterion id="AC-UTML-08-04">
      <given>L_step &lt; 0.3 (none of the above triggers)</given>
      <when>classifier evaluates</when>
      <then>Returns Confused</then>
    </criterion>
    <criterion id="AC-UTML-08-05">
      <given>0.3 ≤ L_step &lt; 0.7</given>
      <when>classifier evaluates</when>
      <then>Returns Optimal</then>
    </criterion>
    <criterion id="AC-UTML-08-06">
      <given>L_step ≥ 0.7</given>
      <when>classifier evaluates</when>
      <then>Returns Boring/Mastery; recommends advancing curriculum</then>
    </criterion>
    <criterion id="AC-UTML-08-07">
      <given>mean(L_step) &gt; 0.3 over window AND ΔM/mean(L_step) &lt; 0.2 (consolidation gain too low)</given>
      <when>classifier evaluates</when>
      <then>Returns Dissipating; recommends replay-buffer enrichment, EWC re-anchoring, LR-warmup reset, continual-backprop dormant-unit re-init, fresh real-data injection (anti-Shumailov 2024)</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-UTML-09" priority="must-have">
  <narrative>
    <as_a>Phase 4 Inference / Verify Surface (UT-03)</as_a>
    <i_want_to>multiply runtime-observed ΔΩ_mean × ΔΞ_mean into the calibrated_confidence formula, clamped to [0.10, 0.95], and use the product in Reward = 0.4·calibrated_confidence + 0.3·mean(L_step·ΔΞ) + 0.3·gtau_pass_rate</i_want_to>
    <so_that>a confident prediction in a collapsed latent or a chronically unstable optimizer is structurally not trustworthy; reward integrand requires representational integrity (L_step·ΔΞ); per UTML §XIV bottom</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-UTML-09-01">
      <given>raw_conf, conv_rate, strategy_agreement, evidence_factor, ΔΩ_mean, ΔΞ_mean each in [0,1]</given>
      <when>calibrated_confidence is computed</when>
      <then>Returns clamp_[0.10, 0.95](raw · conv · agree · evidence · ΔΩ_mean · ΔΞ_mean); evidence_factor = 0.5 + 0.5·min(1.0, num_paths/threshold)</then>
    </criterion>
    <criterion id="AC-UTML-09-02">
      <given>ΔΩ_mean × ΔΞ_mean × raw_conf falls below 0.10</given>
      <when>verify(patch) is called</when>
      <then>calibrated_confidence floored at 0.10; surface degradation reason (low ΔΩ_mean OR low ΔΞ_mean OR both); operator can read the degradation reason via the API response</then>
    </criterion>
    <criterion id="AC-UTML-09-03">
      <given>Phase 4 Reward computation runs at training time</given>
      <when>aggregating over window</when>
      <then>Reward = 0.4·calibrated_confidence + 0.3·mean(L_step·ΔΞ) + 0.3·gtau_pass_rate; weights sum to 1.0 exactly; mean(L_step·ΔΞ) ensures steps without representational integrity contribute zero to reward</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-UTML-10" priority="should-have">
  <narrative>
    <as_a>Phase 3 Trainer (UT-01)</as_a>
    <i_want_to>compute the consolidation integral M(t) = ∫₀ᵗ L_step(τ) · ΔΞ(τ) · k_replay(τ) · e^(-(t-τ)/τ_decay(τ)) dτ as a discrete cumulative sum over the training run</i_want_to>
    <so_that>the system has a measurable consolidation tensor; steps without representational integrity (ΔΞ ≈ 0) cannot consolidate even if the gradient update happens; the "dissipation" failure mode (UTML §VII.1) is detectable</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-UTML-10-01">
      <given>Discrete training steps τ ∈ {0, 1, 2, ..., t}</given>
      <when>M(t) is computed</when>
      <then>M(t) = Σ_{τ=0}^{t} L_step(τ) · ΔΞ(τ) · k_replay(τ) · exp(-(t-τ)/τ_decay(τ)); k_replay defaults to 1.0 normal training, 2-4 during replay-buffer / EMA / SWA / distillation phases; τ_decay = 7 days for un-EWC++-bound examples, 365 days for high-Fisher consolidated examples</then>
    </criterion>
    <criterion id="AC-UTML-10-02">
      <given>ΔΞ(τ) ≈ 0 (collapsed latent at step τ)</given>
      <when>M(t) integrand is evaluated</when>
      <then>L_step(τ)·ΔΞ(τ) ≈ 0; that step contributes nothing to M(t); enforces the UTML v1.0 non-collapse consolidation gate</then>
    </criterion>
    <criterion id="AC-UTML-10-03">
      <given>mean(L_step) over window &gt; 0.3 AND ΔM over equivalent eval window / mean(L_step) &lt; 0.2</given>
      <when>dissipation detector runs</when>
      <then>Surface Dissipating state to UT-02 self-healing controller; recommend NOT more compute but state regulation per Section D / §11.2</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-UTML-11" priority="should-have">
  <narrative>
    <as_a>Phase 8 Evaluation (operator gate)</as_a>
    <i_want_to>compute T(train, deploy) = exp(-d(P_train, P_deploy)/λ) and performance_deploy = M(t) · T(train, deploy)</i_want_to>
    <so_that>a low Phase 8 score can be diagnosed as either a training problem (M(t) low) or a distribution-shift problem (T low); active learning + cross-language calibration target T directly</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-UTML-11-01">
      <given>Training distribution P_train (Phase 0 mutation corpus, 8 cat × 300 task × 11 lang) and deployment distribution P_deploy (real outer-loop patches)</given>
      <when>T is computed</when>
      <then>T = exp(-d(P_train, P_deploy)/λ) where d ∈ {Wasserstein, MMD, KL, task-specific}; λ is a transfer-bandwidth constant configurable per language</then>
    </criterion>
    <criterion id="AC-UTML-11-02">
      <given>Phase 8 holdout evaluation completes</given>
      <when>performance_deploy is reported</when>
      <then>performance_deploy = M(t) · T(train, deploy); operator can attribute shortfall to M(t) (training) vs T (distribution shift)</then>
    </criterion>
  </acceptance_criteria>
</story>

</user_stories>

<requirements>

<requirement id="REQ-UTML-01" story_ref="US-UTML-01" priority="must">
  <description>Implement the multiplicative four-factor learning formula L_step = ΔP × ΔK × ΔΩ × ΔΞ with all factors clamped to [0, 1]. Multiplication is non-negotiable per UTML §I and constitution &lt;utml_v1_invariants&gt; rule 1.</description>
  <rationale>Multiplication enforces that any single factor at zero zeros the entire learning signal regardless of how good the others look. Addition would let a great ΔΩ compensate for a collapsed ΔΞ, which is empirically false (UTML §I).</rationale>
  <constraints>Output type f32; deterministic byte-equal under same inputs + same code_version; fail-closed on any input outside [0, 1] (NaN, Inf, negative, &gt;1.0).</constraints>
  <verification_method>Property-based test with proptest crate: 10,000 random (ΔP, ΔK, ΔΩ, ΔΞ) tuples in [0, 1]^4 verify L_step is in [0, 1] and equals the product. Edge-case tests for each factor at exactly 0.0.</verification_method>
</requirement>

<requirement id="REQ-UTML-02" story_ref="US-UTML-02" priority="must">
  <description>Implement compute_delta_p(predicted, target, snr, exploration_bonus, gamma, imagined) returning clamp_[0,1](max(ΔP_real, γ · ΔP_imagined) + 0.5 · exploration_bonus) / SNR(t) where ΔP_real = clamp(1 − cosine, 0, 1) and the target is the *frozen* externally-defined instrument panel.</description>
  <rationale>JEPA-foundational latent-space prediction (LeCun 2022; Assran 2023; Bardes 2024). Imagination discount γ ≈ 0.7 mirrors Dreamer-class empirical range 0.6-0.8 (Hafner 2023). SNR normalization (Liu 2020) shrinks ΔP when gradient is uninformative. Active-learning bonus +0.5 surfaces wide-conformal-interval examples.</rationale>
  <constraints>Target must be from frozen instruments (zero trainable params); never accept a learnable target encoder. SNR floor enforced (configured minimum) — fail-closed on SNR below floor, never divide-by-zero.</constraints>
  <verification_method>Unit tests against perfect-prediction (ΔP=0), total-miss (ΔP=1), high-noise (low SNR shrinks ΔP), exploration-bonus active examples. Per-chunk aggregation tests for Mean/Max/TopKMean/EntityWeighted.</verification_method>
</requirement>

<requirement id="REQ-UTML-03" story_ref="US-UTML-03" priority="must">
  <description>Implement compute_delta_k(grad, ema_grad, fisher, ece, embedder_coherence) returning clamp_[0,1](0.5·cos_align + 0.3·(1 − fisher_violation) + 0.15·(1 − ECE) + 0.05·embedder_coherence). Weights sum to 1.0 exactly. embedder_coherence per doc 12 §5.1 (cross-embedder Jaccard NN-set agreement); defaults to 0.5 (neutral) for first 1,000 corpus entries.</description>
  <rationale>Gradient alignment with EMA history captures NTK alignment / catastrophic-forgetting signature (Atanasov 2021; Kirkpatrick 2017). Fisher-weighted protection enforces EWC++. ECE is the calibration / Goodhart early indicator (Gao 2022). embedder_coherence adds cross-embedder OOD signal (doc 12 §5.1).</rationale>
  <constraints>cos_align computation must use EMA over recent batch-gradient directions; Fisher matrix from heldout 30-task validation panel post-Phase-3; ECE from Phase 4 conformal-calibration side-output.</constraints>
  <verification_method>Unit tests: cos_align &lt; 0 → ΔK collapses; high fisher_violation → ΔK shrinks; ECE rising → ΔK shrinks. Integration test: cross-embedder NN-set Jaccard correctly computed on 14-embedder ensemble.</verification_method>
</requirement>

<requirement id="REQ-UTML-04" story_ref="US-UTML-04" priority="must">
  <description>Implement compute_delta_omega(plasticity, landscape, stability, agent_state) returning plasticity^0.4 · landscape^0.4 · stability^0.1 · agent_state^0.1 clamped to [0, 1]. Geometric (multiplicative) weighting per UTML §V; weights sum to 1.0. agent_state from doc 12 §5.3 E17 AffectText probe.</description>
  <rationale>effective_plasticity captures Sutton 2024 plasticity loss (intervention: continual backprop). landscape_health = sigmoid(-λ_max(H_t)) · inverted_U(‖g_t‖) penalizes sharp minima (SAM, Foret 2021) and vanishing/exploding gradients. stability_floor is the hard NaN/Goodhart gate. agent_state_score adds the outer-loop agent's E17 affect signal.</rationale>
  <constraints>Geometric weighting non-negotiable (any subcomponent at 0 forces ΔΩ=0). agent_state defaults to neutral 0.7 if E17 transcript window unavailable, NEVER 1.0.</constraints>
  <verification_method>Unit tests: each subcomponent at 0 → ΔΩ=0; full healthy state ≈ 0.92 per Career-Taxonomy empirical anchor (UTML §V.3). Integration test: stability_floor=0.0 triggers MEJEPA_OBSERVE_BATCH_NAN per &lt;failure_taxonomy&gt;.</verification_method>
</requirement>

<requirement id="REQ-UTML-05" story_ref="US-UTML-05" priority="must">
  <description>Implement compute_delta_xi(target_collapse, redundancy, violation) returning (1 − target_collapse)^0.5 · (1 − redundancy)^0.3 · (1 − violation)^0.2. MUST assert_eq!(target_collapse, 0.0) for ME-JEPA — any nonzero observation is a critical bug indicating an instrument has gradient flow.</description>
  <rationale>ΔΞ is the central architectural innovation of ME-JEPA (UTML §VI). Target-side collapse is structurally impossible because target panel is produced by frozen externally-defined instruments (zero trainable params). The assert is the runtime *measurement* of that structural property per constitution &lt;utml_v1_invariants&gt; rule 2.</rationale>
  <constraints>target_collapse MUST equal 0.0 exactly; any nonzero value triggers fail-closed critical error per &lt;failure_taxonomy&gt;. predictor_redundancy &gt; 0.5 triggers VICReg coefficient bump. constellation_violation_rate from Phase 4b Gτ rejections.</constraints>
  <verification_method>Unit tests: target_collapse=0 → first factor locked at 1.0; target_collapse&gt;0 → fails closed with critical error. FSV: every step in CF_MEJEPA_TRAIN_CERTS verified to have delta_xi_components.target_collapse == 0.0.</verification_method>
</requirement>

<requirement id="REQ-UTML-06" story_ref="US-UTML-06" priority="must">
  <description>Implement sampling_weight(L_step, operator_override, age_days, decay) = base_weight · L_step · (1 + 5·operator_override) · decay^age_days with base_weight=1.0, decay=0.995 (~140-day half-life). Drop examples with weight &lt; 0.05; force-include weight &gt; 1.0.</description>
  <rationale>Hard-example mining (high L_step) + gold-standard up-weighting + recency-weighted curriculum. Per UTML §X / doc 11 §6 this REPLACES the earlier (1 + 2·residual_error_z_score) sampling term from doc 09 §3.7.2 because L_step already captures hard-example mining via ΔP and additionally enforces ΔΞ-safety + ΔΩ-safety + ΔK-coherence which residual-z-score did not.</rationale>
  <constraints>Output non-negative; not clamped to [0, 1] (override can push above 1.0). Drop threshold 0.05 fixed. Force-include threshold 1.0 fixed.</constraints>
  <verification_method>Unit tests: weight=0 → drop; weight=1.0 (operator_override=true, L_step=0.167, age=0) → force-include. Integration test: 1-epoch dry-train on 100 synthetic examples confirms high-L_step examples appear more often in sampled batches.</verification_method>
</requirement>

<requirement id="REQ-UTML-07" story_ref="US-UTML-07" priority="must">
  <description>Persist per-step training certificate to CF_MEJEPA_TRAIN_CERTS keyed by step. Cert MUST include: step, loss, delta_p, delta_k, delta_omega, delta_xi, L_step, delta_omega_components, delta_xi_components, delta_k_components, sampling_weight, grad_norm, drift_coverage, parent_witness_hash, self_hash, merkle_root, code_version, corpus_sha, embedder_versions, frozen_at, timestamp_iso8601.</description>
  <rationale>Per UTML §XII the audit substrate IS the physical M(t) consolidation tensor. Hash-chained parent_witness_hash → self_hash makes the full training run a byte-equal-readback verifiable log. Required for operator audit (UT-04).</rationale>
  <constraints>JSON serialization deterministic (sorted keys, no map-iteration nondeterminism). Hash chain integrity must be verifiable end-to-end on a 100,000-entry chain (constitution &lt;testing_requirements&gt;). Skip-reason field populated when L_step==0.</constraints>
  <verification_method>FSV: write 100 certs, replay end-to-end, verify chain integrity, verify all fields present and well-typed. Integration test: tamper with cert[N].body and verify cert[N+1].parent_witness_hash mismatch detected.</verification_method>
</requirement>

<requirement id="REQ-UTML-08" story_ref="US-UTML-08" priority="must">
  <description>Implement UTML cycle classifier returning one of seven states {Stuck, OptimizerDysregulated, LatentCollapsing, Confused, Optimal, Boring, Dissipating} from L_step, ΔΩ, ΔΞ, mean(L_step) over window, and ΔM/mean(L_step). Coaching order: regulate ΔΩ and ΔΞ FIRST, then adjust ΔP/ΔK.</description>
  <rationale>Per UTML §IX + constitution &lt;utml_v1_invariants&gt; rule 3 (coaching order). The classifier tells Phase 5 self-healing *which* intervention to run, not just "retrain something". Pruning rule (UTML §X / constitution rule 5) requires ALL THREE signals (L_step low, ΔΩ &gt; 0.4, ΔΞ &gt; 0.4) to agree before pruning a curriculum item.</rationale>
  <constraints>Stuck classification requires ALL THREE conditions: L_step &lt; 0.1 for N consecutive steps, ΔΩ &gt; 0.4 throughout window, ΔΞ &gt; 0.4 throughout window. N is configurable (default 100). Window size for mean(L_step) and ΔM is configurable (default 1000 steps).</constraints>
  <verification_method>Unit tests: each of the seven state transitions triggered by appropriate input. Integration test: synthetic 1000-step trace with planted Stuck regime → classifier identifies it; bad-ΔΩ regime → classifier identifies OptimizerDysregulated and does NOT prune.</verification_method>
</requirement>

<requirement id="REQ-UTML-09" story_ref="US-UTML-09" priority="must">
  <description>Compute calibrated_confidence = clamp_[0.10, 0.95](raw_conf · conv_rate · strategy_agreement · evidence_factor · ΔΩ_mean · ΔΞ_mean) and Reward = 0.4·calibrated_confidence + 0.3·mean(L_step·ΔΞ) + 0.3·gtau_pass_rate.</description>
  <rationale>Per UTML §XIV bottom. Confidence is gated by both ΔΩ AND ΔΞ — a confident prediction in a collapsed latent or chronically unstable optimizer is structurally untrustworthy. Reward integrand mean(L_step·ΔΞ) ensures steps without representational integrity contribute zero to reward even if loss looked great.</rationale>
  <constraints>evidence_factor = 0.5 + 0.5·min(1.0, num_paths/threshold). Reward weights 0.4/0.3/0.3 sum to 1.0 exactly. Floor at 0.10, ceiling at 0.95 (no false certainty).</constraints>
  <verification_method>Unit tests: ΔΩ_mean → 0 forces calibrated_confidence to floor 0.10; ΔΞ_mean → 0 same. Integration: gtau_pass_rate from Phase 4b feeds correctly into Reward.</verification_method>
</requirement>

<requirement id="REQ-UTML-10" story_ref="US-UTML-10" priority="should">
  <description>Compute consolidation integral M(t) = Σ_{τ=0}^{t} L_step(τ) · ΔΞ(τ) · k_replay(τ) · exp(-(t-τ)/τ_decay(τ)) discretely over training steps. Detect Dissipating state when mean(L_step) &gt; 0.3 over window AND ΔM/mean(L_step) &lt; 0.2.</description>
  <rationale>Per UTML §VII. The product L_step·ΔΞ enforces that updates without representational integrity contribute zero to consolidation. The "dissipation" failure mode is a leading indicator of RLHF reward overoptimization, near-duplicate self-supervised pretraining, plasticity-loss continual-learning, and Shumailov 2024 synthetic-data collapse.</rationale>
  <constraints>k_replay defaults to 1.0 normal training, 2-4 during replay-buffer / EMA / SWA / distillation phases. τ_decay = 7 days for un-EWC++-bound examples, 365 days for high-Fisher consolidated examples.</constraints>
  <verification_method>Unit tests: ΔΞ(τ)=0 step contributes nothing to M(t). Integration test: synthetic Dissipating regime (high L_step, low ΔM) → classifier surfaces correctly.</verification_method>
</requirement>

<requirement id="REQ-UTML-11" story_ref="US-UTML-11" priority="should">
  <description>Compute T(train, deploy) = exp(-d(P_train, P_deploy)/λ) and performance_deploy = M(t) · T(train, deploy) where d is a divergence measure (Wasserstein/MMD/KL/task-specific) and λ is a transfer-bandwidth constant configurable per language.</description>
  <rationale>Per UTML §VIII. The mutation corpus is precisely domain randomization (Tobin 2017). Phase 5 active learning + cross-language calibration adaptively close d(P_train, P_deploy). Phase 8 ≥0.95 ship gate is exactly performance_deploy = M(t) · T.</rationale>
  <constraints>d defaults to Wasserstein over panel-distribution centroids per language. λ configurable via models_config.toml; per-language values default to 1.0.</constraints>
  <verification_method>Unit test: identical P_train and P_deploy → T = 1.0. Cross-language synthetic test: Python-trained, Rust-deployed → T &lt; 1.0 by measurable amount; OOD detector escalates to operator.</verification_method>
</requirement>

<requirement id="REQ-UTML-12" story_ref="US-UTML-08" priority="must">
  <description>Pruning rule per UTML §X: prune_curriculum_item if (L_step &lt; 0.1 for N steps) AND (ΔΩ &gt; 0.4 throughout) AND (ΔΞ &gt; 0.4 throughout). All three signals MUST agree.</description>
  <rationale>Per constitution &lt;utml_v1_invariants&gt; rule 5. Pruning a curriculum item that failed under bad ΔΩ teaches the wrong lesson — diagnosis is *optimizer-state problem, not curriculum problem*. Same for bad ΔΞ — diagnosis is *latent collapse, not curriculum problem*. Wrong-cause pruning permanently excludes useful training data.</rationale>
  <constraints>N configurable (default 100). Window for "throughout" check matches N. Pruning emits a witness-chain entry recording the curriculum_item_id, N, mean(L_step), mean(ΔΩ), mean(ΔΞ), prune_decision_timestamp.</constraints>
  <verification_method>Unit tests: each of the three rule conditions individually fails to prune. Integration test: synthetic Stuck regime correctly identifies and prunes; synthetic OptimizerDysregulated regime does NOT prune.</verification_method>
</requirement>

</requirements>

<business_rules>

<rule id="BR-UTML-01" req_ref="REQ-UTML-01">
  <condition>Computing L_step at any training step</condition>
  <action>Multiply all four factors; result clamped to [0, 1]; output deterministic byte-equal under same inputs + same code_version</action>
  <exception>If any input is NaN, Inf, negative, or &gt;1.0, fail-closed with structured error per &lt;failure_taxonomy&gt;; never silently substitute defaults</exception>
</rule>

<rule id="BR-UTML-02" req_ref="REQ-UTML-05">
  <condition>compute_delta_xi is called for ME-JEPA</condition>
  <action>assert_eq!(target_collapse, 0.0) MUST execute and pass; first factor (1 − target_collapse)^0.5 locked at 1.0 always</action>
  <exception>If target_collapse &gt; 0.0, raise critical-severity error indicating an instrument has accidentally become trainable; halt training; surface to operator immediately</exception>
</rule>

<rule id="BR-UTML-03" req_ref="REQ-UTML-08">
  <condition>UTML cycle classifier evaluates a window where L_step is low</condition>
  <action>Determine which substrate factor is degraded (ΔΩ &lt; 0.4 → OptimizerDysregulated; ΔΞ &lt; 0.4 → LatentCollapsing); regulate the substrate FIRST per UTML §IX coaching order</action>
  <exception>If both ΔΩ &gt; 0.4 AND ΔΞ &gt; 0.4 throughout window AND L_step &lt; 0.1 for N steps, classify as Stuck; ONLY THEN may curriculum pruning be applied</exception>
</rule>

<rule id="BR-UTML-04" req_ref="REQ-UTML-12">
  <condition>Pruning a curriculum item</condition>
  <action>All three pruning conditions (L_step &lt; 0.1 for N steps; ΔΩ &gt; 0.4 throughout; ΔΞ &gt; 0.4 throughout) must agree; emit witness-chain entry recording the prune decision with full evidence</action>
  <exception>If any condition fails, do NOT prune; surface diagnosis (substrate vs content) to UT-02 self-healing controller for correct intervention</exception>
</rule>

<rule id="BR-UTML-05" req_ref="REQ-UTML-07">
  <condition>Per-step training certificate emission</condition>
  <action>Write to CF_MEJEPA_TRAIN_CERTS[step] with all required fields; chain via parent_witness_hash → self_hash</action>
  <exception>Step skipped (any factor zero): cert.skip_reason populated; cert.sampling_weight = 0.0; chain link still emitted to preserve integrity</exception>
</rule>

<rule id="BR-UTML-06" req_ref="REQ-UTML-06">
  <condition>BatchSampler reads sampling_weight for an example</condition>
  <action>Drop example if weight &lt; 0.05; force-include if weight &gt; 1.0; otherwise probabilistic inclusion proportional to weight</action>
  <exception>operator_override_flag=true forces weight &gt; 1.0 by construction (1 + 5·1 = 6× multiplier); always force-included</exception>
</rule>

<rule id="BR-UTML-07" req_ref="REQ-UTML-09">
  <condition>Phase 4 verify(patch) called by UT-03</condition>
  <action>Compute calibrated_confidence with ΔΩ_mean × ΔΞ_mean multiplied in; clamp to [0.10, 0.95]; surface degradation reason in API response if either factor below threshold</action>
  <exception>Never report a confidence above 0.95 (no false certainty); never below 0.10 (floor); operator can read degradation reason</exception>
</rule>

</business_rules>

<edge_cases>

<edge_case id="EC-UTML-01" req_ref="REQ-UTML-02">
  <scenario>ΔP_real = 0 (perfect prediction on this example)</scenario>
  <expected_behavior>L_step = 0; example excluded from next batch; per §11.2 edge-case audit; cert.skip_reason="perfect_prediction"; sampling_weight = 0.0; example does not consolidate via M(t).</expected_behavior>
  <priority>high</priority>
</edge_case>

<edge_case id="EC-UTML-02" req_ref="REQ-UTML-03">
  <scenario>ΔK = 0 (catastrophic forgetting / cos_align &lt; 0)</scenario>
  <expected_behavior>L_step = 0; step ABORTED and rolled back to prior checkpoint per §11.2; CF_MEJEPA_DRIFT_STATE updated; cert.skip_reason="catastrophic_forgetting"; alert surfaced to UT-02 self-healing controller.</expected_behavior>
  <priority>critical</priority>
</edge_case>

<edge_case id="EC-UTML-03" req_ref="REQ-UTML-04">
  <scenario>ΔΩ = 0 (NaN/Inf gradient detected, validation rising prolonged, OR Goodhart catastrophe — proxy reward decoupled from gold metric per Skalse 2024)</scenario>
  <expected_behavior>L_step = 0; step ABORT + ALERT; cert.skip_reason populated with which sub-component zeroed (effective_plasticity / landscape_health / stability_floor / agent_state_score); MEJEPA_OBSERVE_BATCH_NAN error code if NaN; MEJEPA_OBSERVE_DRIFT_DETECTED if validation rising; surface high-severity blocker.</expected_behavior>
  <priority>critical</priority>
</edge_case>

<edge_case id="EC-UTML-04" req_ref="REQ-UTML-05">
  <scenario>ΔΞ.target_collapse &gt; 0 observed at any step (this should be IMPOSSIBLE for ME-JEPA by construction)</scenario>
  <expected_behavior>FAIL-CLOSED with CRITICAL severity error; an instrument has accidentally been made trainable (gradient flow through what should be a frozen function); halt training immediately; surface to operator with full instrument identification; fix at the trait level per constitution &lt;anti_patterns&gt; "no learnable target encoder" rule. This is THE central architectural integrity check of ME-JEPA.</expected_behavior>
  <priority>critical</priority>
</edge_case>

<edge_case id="EC-UTML-05" req_ref="REQ-UTML-05">
  <scenario>ΔΞ.predictor_redundancy &gt; 0.5 (panel slots collapsing into each other; pairwise MI exceeds ceiling)</scenario>
  <expected_behavior>VICReg coefficient bumped (variance + covariance terms reweighted); refit predictor on next epoch; per §11.2; surface LatentCollapsing classification to UT-02; do NOT prune curriculum.</expected_behavior>
  <priority>high</priority>
</edge_case>

<edge_case id="EC-UTML-06" req_ref="REQ-UTML-01">
  <scenario>All four factors at 1.0 (highest-priority training example: high prediction error, high coherence, healthy substrate, no collapse)</scenario>
  <expected_behavior>L_step = 1.0; sampling_weight reaches ≥ 1.0 after (1 + 5·operator_override) × decay^0 factors when override true; example FORCE-INCLUDED in next batch; cert recorded with full breakdown.</expected_behavior>
  <priority>high</priority>
</edge_case>

<edge_case id="EC-UTML-07" req_ref="REQ-UTML-08">
  <scenario>L_step &lt; 0.1 for N consecutive steps but ΔΩ ≈ 0.2 (optimizer dysregulated)</scenario>
  <expected_behavior>Classify as OptimizerDysregulated NOT Stuck; do NOT prune; recommend ΔΩ regulation (continual backprop / LR warmup / gradient clipping); per UTML §IX coaching order.</expected_behavior>
  <priority>high</priority>
</edge_case>

<edge_case id="EC-UTML-08" req_ref="REQ-UTML-08">
  <scenario>L_step &lt; 0.1 for N consecutive steps but ΔΞ ≈ 0.2 (latent collapsing)</scenario>
  <expected_behavior>Classify as LatentCollapsing NOT Stuck; do NOT prune; recommend ΔΞ regulation (VICReg coefficient bump / constellation re-fit / instrument re-initialization); per UTML §IX coaching order.</expected_behavior>
  <priority>high</priority>
</edge_case>

<edge_case id="EC-UTML-09" req_ref="REQ-UTML-10">
  <scenario>mean(L_step) &gt; 0.3 over window AND ΔM/mean(L_step) &lt; 0.2 (Dissipating: training loss falls but validation does not consolidate)</scenario>
  <expected_behavior>Classify Dissipating; recommend NOT more compute but state regulation per Section D: replay-buffer enrichment, EWC re-anchoring, LR-warmup reset, continual-backprop dormant-unit re-initialization, fresh real-data injection (anti-Shumailov 2024).</expected_behavior>
  <priority>high</priority>
</edge_case>

<edge_case id="EC-UTML-10" req_ref="REQ-UTML-04">
  <scenario>E17 AffectText transcript window unavailable (no recent operator conversation)</scenario>
  <expected_behavior>agent_state_score defaults to neutral 0.7 (NOT 1.0); ΔΩ computation proceeds; cert records agent_state_score_source="default_neutral_no_transcript"; never silently substitute optimistic 1.0.</expected_behavior>
  <priority>medium</priority>
</edge_case>

<edge_case id="EC-UTML-11" req_ref="REQ-UTML-03">
  <scenario>First 1,000 corpus entries (cross-embedder NN sets are not yet statistically meaningful per doc 12 §5.1 risk #3)</scenario>
  <expected_behavior>embedder_coherence defaults to 0.5 (neutral); cert records embedder_coherence_source="bootstrap_neutral_n_lt_1000"; never silently use 1.0 (would over-credit ΔK).</expected_behavior>
  <priority>medium</priority>
</edge_case>

<edge_case id="EC-UTML-12" req_ref="REQ-UTML-02">
  <scenario>Per-chunk ΔP aggregation when constellation guard (Phase 4b Gτ) flagged a violating chunk</scenario>
  <expected_behavior>Aggregator switches from default Mean to Max so loss focuses on worst chunk; aggregator choice persisted to cert.delta_p_aggregator field; per doc 11 §2 per-chunk ΔP integration.</expected_behavior>
  <priority>medium</priority>
</edge_case>

<edge_case id="EC-UTML-13" req_ref="REQ-UTML-07">
  <scenario>Witness chain integrity check on cert at step N fails (parent_witness_hash mismatch)</scenario>
  <expected_behavior>Raise MEJEPA_OBSERVE_INTEGRITY_VIOLATION per &lt;failure_taxonomy&gt;; halt inference (do not use ME-JEPA verdicts until restored); restore from last verified checkpoint; surface critical blocker to operator.</expected_behavior>
  <priority>critical</priority>
</edge_case>

<edge_case id="EC-UTML-14" req_ref="REQ-UTML-09">
  <scenario>verify(patch) called when ΔΩ_mean × ΔΞ_mean × raw_conf &lt; 0.10</scenario>
  <expected_behavior>calibrated_confidence floored at 0.10; API response surfaces degradation reason (low_omega_mean / low_xi_mean / both); operator-readable; never report confidence below floor.</expected_behavior>
  <priority>high</priority>
</edge_case>

<edge_case id="EC-UTML-15" req_ref="REQ-UTML-02">
  <scenario>SNR(t) below configured floor (e.g., 0.05) — gradient is uninformative</scenario>
  <expected_behavior>Fail-closed with structured error indicating SNR below floor; do NOT divide by SNR (would explode ΔP); persist SNR observation to cert for diagnostic; recommend reducing learning rate or batch-size adjustment.</expected_behavior>
  <priority>medium</priority>
</edge_case>

</edge_cases>

<error_states>

<error id="ERR-UTML-01" http_code="500" req_ref="REQ-UTML-01">
  <condition>Any factor input outside [0, 1] (NaN, Inf, negative, or &gt; 1.0)</condition>
  <user_message>Training step rejected: factor input out of range. See cert.skip_reason for which factor failed validation.</user_message>
  <internal_message>UTML_FACTOR_OUT_OF_RANGE: factor=&lt;name&gt;, value=&lt;value&gt;, expected_range=[0,1]; step=&lt;N&gt;; do not silently clamp.</internal_message>
  <recovery_action>Halt step; emit cert with skip_reason; surface to UT-02 self-healing controller for diagnosis.</recovery_action>
</error>

<error id="ERR-UTML-02" http_code="500" req_ref="REQ-UTML-05">
  <condition>ΔΞ.target_collapse &gt; 0 detected (impossible for ME-JEPA by construction; indicates an instrument has gradient flow)</condition>
  <user_message>CRITICAL: ME-JEPA architectural integrity violation. An instrument has accidentally become trainable. Training halted.</user_message>
  <internal_message>MEJEPA_INSTRUMENT_GRADIENT_LEAK: target_collapse=&lt;value&gt;, expected=0.0; step=&lt;N&gt;; suspect_instrument=&lt;identifier&gt;; check Instrument trait for accidental requires_grad=true.</internal_message>
  <recovery_action>Halt training immediately; surface high-severity blocker; require operator inspection; fix at trait level per &lt;anti_patterns&gt; "no learnable target encoder" rule.</recovery_action>
</error>

<error id="ERR-UTML-03" http_code="500" req_ref="REQ-UTML-04">
  <condition>NaN or Inf detected in gradient or activation during compute_delta_omega → stability_floor=0.0</condition>
  <user_message>Training step aborted: numerical instability detected.</user_message>
  <internal_message>MEJEPA_OBSERVE_BATCH_NAN: location=&lt;gradient|activation&gt;, layer=&lt;name&gt;, step=&lt;N&gt;; rollback initiated.</internal_message>
  <recovery_action>Roll back to last-good checkpoint; emit cert with skip_reason="nan_inf"; surface alert.</recovery_action>
</error>

<error id="ERR-UTML-04" http_code="500" req_ref="REQ-UTML-08">
  <condition>UTML cycle classifier confidence in state assignment is below threshold (ambiguous regime)</condition>
  <user_message>Training state classification ambiguous; surfacing to operator for review.</user_message>
  <internal_message>UTML_CLASSIFIER_AMBIGUOUS: candidates=&lt;list&gt;, confidence=&lt;value&gt;, threshold=&lt;value&gt;; window=&lt;N steps&gt;.</internal_message>
  <recovery_action>Default to most conservative intervention (substrate regulation, never prune); surface to UT-02 + UT-04.</recovery_action>
</error>

<error id="ERR-UTML-05" http_code="500" req_ref="REQ-UTML-07">
  <condition>Witness chain integrity violation: cert[N].parent_witness_hash != sha256(cert[N-1].self_hash || cert[N-1].body)</condition>
  <user_message>Training audit chain broken at step N. Halt and restore from verified checkpoint.</user_message>
  <internal_message>MEJEPA_OBSERVE_INTEGRITY_VIOLATION: step=&lt;N&gt;, expected_parent_hash=&lt;hex&gt;, actual_parent_hash=&lt;hex&gt;; chain replay failed.</internal_message>
  <recovery_action>Halt training; restore last verified checkpoint; replay chain to find tamper boundary; surface critical blocker.</recovery_action>
</error>

<error id="ERR-UTML-06" http_code="400" req_ref="REQ-UTML-02">
  <condition>SNR(t) below configured floor</condition>
  <user_message>Training step rejected: gradient signal-to-noise ratio below floor.</user_message>
  <internal_message>UTML_SNR_BELOW_FLOOR: snr=&lt;value&gt;, floor=&lt;value&gt;, step=&lt;N&gt;; recommend reducing learning rate or adjusting batch size.</internal_message>
  <recovery_action>Skip step; emit cert with skip_reason="snr_below_floor"; surface diagnostic.</recovery_action>
</error>

<error id="ERR-UTML-07" http_code="400" req_ref="REQ-UTML-03">
  <condition>cos_align(g_t, ḡ_prior) &lt; 0 → ΔK collapses below threshold (catastrophic forgetting)</condition>
  <user_message>Training step aborted: catastrophic forgetting detected. Rolling back.</user_message>
  <internal_message>UTML_CATASTROPHIC_FORGETTING: cos_align=&lt;value&gt;, threshold=&lt;value&gt;, step=&lt;N&gt;; CF_MEJEPA_DRIFT_STATE updated.</internal_message>
  <recovery_action>Roll back to prior checkpoint; emit cert with skip_reason="catastrophic_forgetting"; surface to UT-02; activate EWC++ re-anchoring.</recovery_action>
</error>

</error_states>

<data_requirements>

<entity name="LearningSignalInputs">
  <field name="predicted_latent" type="Tensor (f32, [batch, panel_dim])" required="true">
    <description>Output of g_φ(f_θ(panel_t01)) — the predictor's latent prediction.</description>
    <constraints>panel_dim must match constitution &lt;tech_stack&gt; expected dim (5,120 per doc 09 §3.5.5); finite floats only.</constraints>
  </field>
  <field name="target_latent" type="Tensor (f32, [batch, panel_dim])" required="true">
    <description>Frozen externally-defined instrument panel f_target(panel_t2). MUST have zero trainable parameters upstream.</description>
    <constraints>Same panel_dim as predicted_latent; finite floats only; sourced from frozen Instrument trait impls per Phase 1.</constraints>
  </field>
  <field name="snr" type="f32" required="true">
    <description>Gradient signal-to-noise ratio at current step (Liu et al. 2020).</description>
    <constraints>Above configured floor (default 0.05); fail-closed if below.</constraints>
  </field>
  <field name="exploration_bonus" type="f32" required="true">
    <description>Active-learning bonus (+0.5 for examples flagged due to wide conformal interval, else 0).</description>
    <constraints>In {0.0, 0.5}; per UTML §III.</constraints>
  </field>
  <field name="gamma" type="f32" required="true">
    <description>Imagination fidelity discount (Dreamer-class).</description>
    <constraints>In [0.6, 0.8]; default 0.7 per doc 11 §2.</constraints>
  </field>
  <field name="imagined_latent" type="Option&lt;Tensor&gt;" required="false">
    <description>Counterfactual latent perturbation for ΔP_imagined; None if no imagined rollout this step.</description>
    <constraints>Same dim as predicted_latent when Some.</constraints>
  </field>
  <field name="grad" type="Tensor (f32, [num_params])" required="true">
    <description>Current batch gradient g_t.</description>
    <constraints>Finite floats only; NaN/Inf triggers stability_floor=0.0.</constraints>
  </field>
  <field name="ema_grad" type="Tensor (f32, [num_params])" required="true">
    <description>EMA-averaged gradient ḡ_prior over recent batches.</description>
    <constraints>EMA decay configurable; default 0.99.</constraints>
  </field>
  <field name="fisher" type="FisherMatrix (per-parameter f32 importance weights)" required="true">
    <description>Fisher Information Matrix from heldout 30-task validation panel post-Phase-3.</description>
    <constraints>Diagonal approximation (EWC++); rank verified non-degenerate (constitution &lt;testing_requirements&gt; "EWC++ Fisher non-degeneracy: after 10 task boundaries, weight protection has full rank").</constraints>
  </field>
  <field name="ece" type="f32" required="true">
    <description>Expected Calibration Error from Phase 4 conformal-calibration side-output.</description>
    <constraints>In [0, 1]; rising ECE during training is Goodhart early indicator.</constraints>
  </field>
  <field name="embedder_coherence" type="f32" required="true">
    <description>Cross-embedder Jaccard NN-set agreement on current panel per doc 12 §5.1.</description>
    <constraints>In [0, 1]; defaults to 0.5 for first 1,000 corpus entries.</constraints>
  </field>
  <field name="effective_plasticity" type="f32" required="true">
    <description>Composite of ReLU dormancy fraction + gradient-covariance effective rank + per-parameter GSNR distribution.</description>
    <constraints>In [0, 1]; per UTML §V.1.</constraints>
  </field>
  <field name="landscape_health" type="f32" required="true">
    <description>sigmoid(-λ_max(H_t)) · inverted_U(‖g_t‖).</description>
    <constraints>In [0, 1]; per UTML §V.2.</constraints>
  </field>
  <field name="stability_floor" type="f32" required="true">
    <description>{1.0, 0.4, 0.1, 0.0} discrete; hard NaN/Goodhart gate per UTML §V.3.</description>
    <constraints>One of {1.0, 0.4, 0.1, 0.0}; not arbitrary continuous.</constraints>
  </field>
  <field name="agent_state_score" type="f32" required="true">
    <description>E17 AffectText valence × arousal_inverted_U on outer-loop conversation transcripts per doc 12 §5.3.</description>
    <constraints>In [0, 1]; defaults to neutral 0.7 if E17 transcript window unavailable.</constraints>
  </field>
  <field name="target_collapse" type="f32" required="true">
    <description>Target-side representational collapse measure. MUST equal 0.0 for ME-JEPA by construction.</description>
    <constraints>== 0.0 always; assert_eq! enforced; nonzero is critical bug.</constraints>
  </field>
  <field name="predictor_redundancy" type="f32" required="true">
    <description>1 minus average normalized pairwise MI across panel slots (computed periodically; cached between updates).</description>
    <constraints>In [0, 1]; &gt; 0.5 triggers VICReg bump.</constraints>
  </field>
  <field name="constellation_violation_rate" type="f32" required="true">
    <description>Fraction of recent Gτ (Phase 4b TCT guard) decisions that rejected the predicted panel.</description>
    <constraints>In [0, 1]; &gt; 0.05 over rolling window triggers constellation re-fit.</constraints>
  </field>
</entity>

<entity name="TrainingCertificate">
  <field name="step" type="u64" required="true">
    <description>Monotonic step index keying CF_MEJEPA_TRAIN_CERTS.</description>
    <constraints>Strictly increasing; primary key.</constraints>
  </field>
  <field name="loss" type="f32" required="true">
    <description>Composite training loss this step.</description>
    <constraints>Finite; not the same as L_step.</constraints>
  </field>
  <field name="delta_p" type="f32" required="true">
    <description>Computed ΔP value.</description>
    <constraints>In [0, 1].</constraints>
  </field>
  <field name="delta_k" type="f32" required="true">
    <description>Computed ΔK value.</description>
    <constraints>In [0, 1].</constraints>
  </field>
  <field name="delta_omega" type="f32" required="true">
    <description>Computed ΔΩ value.</description>
    <constraints>In [0, 1].</constraints>
  </field>
  <field name="delta_xi" type="f32" required="true">
    <description>Computed ΔΞ value.</description>
    <constraints>In [0, 1].</constraints>
  </field>
  <field name="L_step" type="f32" required="true">
    <description>Multiplicative product ΔP × ΔK × ΔΩ × ΔΞ.</description>
    <constraints>In [0, 1]; equals product to f32 precision.</constraints>
  </field>
  <field name="delta_omega_components" type="Map&lt;String, f32&gt;" required="true">
    <description>{"effective_plasticity", "landscape_health", "stability_floor", "agent_state_score"}.</description>
    <constraints>All four keys present; each in [0, 1].</constraints>
  </field>
  <field name="delta_xi_components" type="Map&lt;String, f32&gt;" required="true">
    <description>{"target_collapse", "predictor_redundancy", "constellation_violation_rate"}.</description>
    <constraints>All three keys present; target_collapse MUST equal 0.0.</constraints>
  </field>
  <field name="delta_k_components" type="Map&lt;String, f32&gt;" required="true">
    <description>{"cos_align", "fisher_violation", "ece", "embedder_coherence"}.</description>
    <constraints>All four keys present; cos_align in [-1, 1] (not clamped); others in [0, 1].</constraints>
  </field>
  <field name="sampling_weight" type="f32" required="true">
    <description>Per-example sampling weight derived from L_step + override + decay.</description>
    <constraints>Non-negative; can exceed 1.0 with operator_override.</constraints>
  </field>
  <field name="grad_norm" type="f32" required="true">
    <description>‖g_t‖ at this step.</description>
    <constraints>Non-negative; finite.</constraints>
  </field>
  <field name="drift_coverage" type="f32" required="true">
    <description>Empirical conformal coverage over rolling drift-detection window.</description>
    <constraints>In [0, 1]; &lt; 0.85 triggers MEJEPA_OBSERVE_DRIFT_DETECTED.</constraints>
  </field>
  <field name="parent_witness_hash" type="String (hex sha256)" required="true">
    <description>sha256(cert[step-1].self_hash || cert[step-1].body) — chain link.</description>
    <constraints>64-hex; verifiable end-to-end.</constraints>
  </field>
  <field name="self_hash" type="String (hex sha256)" required="true">
    <description>sha256 of this cert's body fields.</description>
    <constraints>64-hex; deterministic from sorted-key JSON serialization.</constraints>
  </field>
  <field name="merkle_root" type="String (hex sha256)" required="true">
    <description>Merkle root over the cert's component sub-trees (for efficient partial verification).</description>
    <constraints>64-hex.</constraints>
  </field>
  <field name="code_version" type="String" required="true">
    <description>Git SHA of code at training time (provenance).</description>
    <constraints>40-hex git commit hash.</constraints>
  </field>
  <field name="corpus_sha" type="String" required="true">
    <description>Hash of training corpus snapshot.</description>
    <constraints>64-hex.</constraints>
  </field>
  <field name="embedder_versions" type="Map&lt;String, String&gt;" required="true">
    <description>Per-embedder version pin (E1..E21).</description>
    <constraints>21 keys; each value a SHA-256 weight digest.</constraints>
  </field>
  <field name="frozen_at" type="String (ISO 8601)" required="true">
    <description>When the frozen-instrument panel was last frozen.</description>
    <constraints>UTC; ISO 8601.</constraints>
  </field>
  <field name="timestamp_iso8601" type="String (ISO 8601)" required="true">
    <description>When this step's cert was emitted.</description>
    <constraints>UTC; ISO 8601.</constraints>
  </field>
  <field name="skip_reason" type="Option&lt;String&gt;" required="false">
    <description>Populated when L_step==0; one of {"perfect_prediction", "catastrophic_forgetting", "nan_inf", "goodhart_catastrophe", "snr_below_floor", ...}.</description>
    <constraints>None when L_step &gt; 0; Some when skipped.</constraints>
  </field>
  <field name="delta_p_aggregator" type="String" required="true">
    <description>Per-chunk aggregator selected for this step ∈ {Mean, Max, TopKMean, EntityWeighted}.</description>
    <constraints>Default Mean; Max when Gτ flagged a violating chunk.</constraints>
  </field>
</entity>

</data_requirements>

<non_functional_requirements>

<nfr id="NFR-UTML-01" category="performance">
  <description>Per-step UTML factor computation latency p50</description>
  <metric>&lt; 5 ms per step on RTX 5090, batch=64, candle-cuda bf16</metric>
  <priority>must</priority>
</nfr>

<nfr id="NFR-UTML-02" category="performance">
  <description>Per-step training certificate emission + RocksDB write latency p50</description>
  <metric>&lt; 10 ms per step (CF_MEJEPA_TRAIN_CERTS write)</metric>
  <priority>must</priority>
</nfr>

<nfr id="NFR-UTML-03" category="reliability">
  <description>Determinism: same inputs + same code_version produce byte-equal L_step output</description>
  <metric>Byte-equal across 1,000 repeated invocations on identical inputs; no f32 nondeterminism beyond hardware-level reproducibility (cudnn-deterministic, fixed seeds)</metric>
  <priority>must</priority>
</nfr>

<nfr id="NFR-UTML-04" category="reliability">
  <description>Witness chain integrity: end-to-end replay verification on 100,000-entry chain</description>
  <metric>0 corrupted entries on synthetic 100k-step training run replay; per constitution &lt;testing_requirements&gt;</metric>
  <priority>must</priority>
</nfr>

<nfr id="NFR-UTML-05" category="security">
  <description>No silent fallback paths in any of the four compute_delta_* functions</description>
  <metric>0 occurrences of unwrap_or(default), .ok().unwrap_or(...), expect("...") on factor computations; static-analysis verified</metric>
  <priority>must</priority>
</nfr>

<nfr id="NFR-UTML-06" category="reliability">
  <description>FSV evidence write protocol: every shipped UTML primitive writes evidence to /tmp/contextgraph-mejepa-train-fsv/ and re-reads + asserts before reporting success</description>
  <metric>FSV passes for: 100 synthetic examples spanning corner cases; 1-epoch dry-train high-L_step examples appear more often; ΔΞ.target_collapse always 0; per doc 11 §11.1</metric>
  <priority>must</priority>
</nfr>

<nfr id="NFR-UTML-07" category="usability">
  <description>UTML cycle classifier verdict surfaced as structured enum (not free-text) to UT-02 self-healing controller</description>
  <metric>Discriminated union with {Stuck, OptimizerDysregulated, LatentCollapsing, Confused, Optimal, Boring, Dissipating}; each variant carries diagnostic payload (which sub-component triggered)</metric>
  <priority>must</priority>
</nfr>

<nfr id="NFR-UTML-08" category="reliability">
  <description>compute_delta_xi MUST assert target_collapse == 0.0 at runtime for ME-JEPA</description>
  <metric>0 step certs in CF_MEJEPA_TRAIN_CERTS with delta_xi_components.target_collapse &gt; 0.0; if any ever observed, treat as production-stop critical bug</metric>
  <priority>must</priority>
</nfr>

</non_functional_requirements>

<dependencies>

<dependency type="internal">
  <name>SPEC-PHASE2-PREDICTOR (predicted latent producer)</name>
  <description>Provides g_φ(f_θ(panel_t01)) for ΔP computation; produces tensors of correct dim per &lt;tech_stack&gt;.</description>
  <impact>Without Phase 2 predictor, ΔP cannot be computed; UTML formula degrades to ΔK × ΔΩ × ΔΞ which is meaningless.</impact>
</dependency>

<dependency type="internal">
  <name>SPEC-PHASE1-INSTRUMENTS (frozen target panel producer)</name>
  <description>Provides f_target(panel_t2) for ΔP target; MUST have zero trainable parameters per constitution.</description>
  <impact>Without frozen instruments, ΔΞ.target_collapse is no longer 0 by construction; ME-JEPA loses its central architectural property.</impact>
</dependency>

<dependency type="internal">
  <name>SPEC-PHASE4-CONFORMAL (ECE side-output + conformal interval)</name>
  <description>Provides ECE for ΔK.calibration_health and conformal interval width for exploration_bonus trigger.</description>
  <impact>Without Phase 4 calibration, ΔK degrades to 3-term form; exploration_bonus cannot fire.</impact>
</dependency>

<dependency type="internal">
  <name>SPEC-PHASE4B-TCT (constellation Gτ guard rejections)</name>
  <description>Provides constellation_violation_rate for ΔΞ; provides Gτ-flagged chunk identifiers for per-chunk Max aggregator selection.</description>
  <impact>Without Phase 4b TCT, constellation_violation_rate defaults to 0; predictor-side anti-collapse defense weakens.</impact>
</dependency>

<dependency type="internal">
  <name>SPEC-EMBEDDER-ENSEMBLE (E1-E21, especially E17 AffectText)</name>
  <description>Provides E1-E14 NN sets for embedder_coherence (doc 12 §5.1); E17 transcript-window output for agent_state_score (doc 12 §5.3).</description>
  <impact>Without E1-E14 NN sets, embedder_coherence stays at neutral 0.5. Without E17, agent_state_score stays at neutral 0.7. Both reduce ΔK / ΔΩ informativeness but do not block training.</impact>
</dependency>

<dependency type="internal">
  <name>SPEC-WITNESS-CHAIN (cert hash chain)</name>
  <description>Provides 73-byte SHAKE-256 chain primitive (✅ shipped) for parent_witness_hash → self_hash linking.</description>
  <impact>Without witness chain, training certs are not hash-linked; auditor (UT-04) cannot verify integrity.</impact>
</dependency>

<dependency type="internal">
  <name>SPEC-STORAGE (CF_MEJEPA_TRAIN_CERTS column family)</name>
  <description>RocksDB column family for per-step certificate persistence.</description>
  <impact>Without CF_MEJEPA_TRAIN_CERTS, cert chain cannot be persisted; per-step audit substrate (M(t)) cannot be reified.</impact>
</dependency>

<dependency type="external">
  <name>candle-cuda 0.9.x</name>
  <description>Tensor ops for ΔP cosine, ΔK gradient EMA, ΔΩ landscape Hessian top-eigenvalue (via power iteration).</description>
  <impact>Required for sub-5ms p50 factor computation per NFR-UTML-01.</impact>
</dependency>

<dependency type="external">
  <name>sha3 0.7.x (SHAKE-256)</name>
  <description>Hash chain construction for cert linking.</description>
  <impact>Required for parent_witness_hash → self_hash chain integrity.</impact>
</dependency>

</dependencies>

<out_of_scope>
  <item>Specific learning-rate schedule policy (LR warmup, cosine anneal, etc.) — handled by SPEC-PHASE3-TRAINING</item>
  <item>Continual-backprop algorithm details (Sutton 2024 dormant-unit re-init) — handled by SPEC-PHASE5-SELF-HEALING</item>
  <item>EWC++ Fisher matrix construction algorithm — handled by SPEC-PHASE5-SELF-HEALING</item>
  <item>VICReg loss term implementation (variance + invariance + covariance) — handled by SPEC-PHASE2-PREDICTOR</item>
  <item>Constellation Gτ guard test details — handled by SPEC-PHASE4B-TCT</item>
  <item>OOD detection algorithm — handled by SPEC-PHASE4-CONFORMAL</item>
  <item>Predictor architecture (transformer / MLP / cross-attention layout) — handled by SPEC-PHASE2-PREDICTOR</item>
  <item>Multi-language AST chunker — handled by SPEC-AST-CHUNKER (per doc 14)</item>
  <item>Sampling algorithm (alias-method, reservoir, etc.) for BatchSampler given sampling_weight — handled by SPEC-PHASE3-TRAINING</item>
  <item>Specific RocksDB compaction / tier policy for CF_MEJEPA_TRAIN_CERTS — handled by SPEC-PHASE6-HYGIENE</item>
  <item>MCP tool schema for `mcp__cgreality__mejepa_capture_audit` (audit reader) — handled by SPEC-PHASE7-MCP-TOOLS</item>
</out_of_scope>

<test_plan>

<test_case id="TC-UTML-01" type="unit" req_ref="REQ-UTML-01">
  <description>L_step multiplicative property under random valid inputs</description>
  <preconditions>proptest crate available</preconditions>
  <inputs>10,000 random (ΔP, ΔK, ΔΩ, ΔΞ) tuples in [0, 1]^4</inputs>
  <expected_result>L_step in [0, 1]; equal to product to f32 precision; deterministic byte-equal across runs with same code_version</expected_result>
  <priority>critical</priority>
</test_case>

<test_case id="TC-UTML-02" type="unit" req_ref="REQ-UTML-01">
  <description>L_step zeroing on any single factor at 0.0</description>
  <preconditions>None</preconditions>
  <inputs>(ΔP=0, ΔK=1, ΔΩ=1, ΔΞ=1); (ΔP=1, ΔK=0, ΔΩ=1, ΔΞ=1); (ΔP=1, ΔK=1, ΔΩ=0, ΔΞ=1); (ΔP=1, ΔK=1, ΔΩ=1, ΔΞ=0)</inputs>
  <expected_result>L_step == 0.0 exactly in all four cases; corresponding example excluded from next batch</expected_result>
  <priority>critical</priority>
</test_case>

<test_case id="TC-UTML-03" type="unit" req_ref="REQ-UTML-05">
  <description>compute_delta_xi target_collapse runtime assertion</description>
  <preconditions>None</preconditions>
  <inputs>(target_collapse=0.0, redundancy=0.1, violation=0.05); (target_collapse=0.001, redundancy=0.1, violation=0.05) [should fail]</inputs>
  <expected_result>Case 1: returns 1.0 · (1-0.1)^0.3 · (1-0.05)^0.2 ≈ 0.964. Case 2: fails closed with MEJEPA_INSTRUMENT_GRADIENT_LEAK / critical error.</expected_result>
  <priority>critical</priority>
</test_case>

<test_case id="TC-UTML-04" type="integration" req_ref="REQ-UTML-07">
  <description>Per-step cert chain integrity end-to-end</description>
  <preconditions>CF_MEJEPA_TRAIN_CERTS column family exists; SHAKE-256 chain primitive shipped</preconditions>
  <inputs>Synthetic 100,000-step training run; emit cert at every step</inputs>
  <expected_result>End-to-end replay: 0 broken chain links; all 100,000 certs verifiable; tampering with cert[N].body detected at cert[N+1] verification</expected_result>
  <priority>critical</priority>
</test_case>

<test_case id="TC-UTML-05" type="integration" req_ref="REQ-UTML-08">
  <description>UTML cycle classifier on synthetic regimes</description>
  <preconditions>Classifier shipped; 7-state enum defined</preconditions>
  <inputs>Five synthetic 1,000-step traces planted with: (1) Stuck regime, (2) OptimizerDysregulated regime, (3) LatentCollapsing regime, (4) Optimal regime, (5) Dissipating regime</inputs>
  <expected_result>Classifier identifies each regime correctly; OptimizerDysregulated and LatentCollapsing do NOT recommend prune; Stuck does recommend prune; Dissipating recommends state regulation not more compute</expected_result>
  <priority>high</priority>
</test_case>

<test_case id="TC-UTML-06" type="integration" req_ref="REQ-UTML-06">
  <description>1-epoch dry-train sampling weight verification</description>
  <preconditions>BatchSampler shipped; 100 synthetic examples with known L_step distribution</preconditions>
  <inputs>100 synthetic examples spanning corner cases: 10 perfect prediction (L_step=0), 30 high-quality (L_step in [0.5, 0.8]), 50 average (L_step in [0.2, 0.4]), 10 operator_override gold (force-include)</inputs>
  <expected_result>10 perfect-prediction examples DROPPED from epoch (sampling_weight &lt; 0.05). 10 operator_override examples FORCE-INCLUDED. High-L_step examples appear more often than average-L_step examples (statistically significant via χ² test, p &lt; 0.01)</expected_result>
  <priority>high</priority>
</test_case>

<test_case id="TC-UTML-07" type="e2e" req_ref="REQ-UTML-05">
  <description>FSV: target_collapse always 0 across full 100k-step training run</description>
  <preconditions>Phase 0/1/2/3 shipped; full 2,400-example corpus available</preconditions>
  <inputs>Full Phase 3 training run (10 epochs × 2,400 examples = 24,000 steps)</inputs>
  <expected_result>EVERY cert in CF_MEJEPA_TRAIN_CERTS has delta_xi_components.target_collapse == 0.0; FSV evidence file at /tmp/contextgraph-mejepa-train-fsv/utml-l-step-evidence.json confirms; any nonzero observation is critical bug</expected_result>
  <priority>critical</priority>
</test_case>

<test_case id="TC-UTML-08" type="unit" req_ref="REQ-UTML-04">
  <description>ΔΩ stability_floor discrete values</description>
  <preconditions>None</preconditions>
  <inputs>NaN gradient; validation rising for &gt;10000 steps; grad_norm in warning band; healthy state</inputs>
  <expected_result>stability_floor = 0.0, 0.1, 0.4, 1.0 respectively; ΔΩ → 0 only when stability_floor=0.0 (other sub-components held at 1.0)</expected_result>
  <priority>high</priority>
</test_case>

<test_case id="TC-UTML-09" type="unit" req_ref="REQ-UTML-09">
  <description>calibrated_confidence floor enforcement</description>
  <preconditions>None</preconditions>
  <inputs>ΔΩ_mean=0.05, ΔΞ_mean=0.05, raw_conf=0.5, conv=1.0, agree=1.0, evidence=1.0</inputs>
  <expected_result>calibrated_confidence floored at 0.10 (would be 0.05·0.05·0.5 = 0.00125 raw, well below floor); degradation_reason populated indicating both ΔΩ_mean and ΔΞ_mean below threshold</expected_result>
  <priority>high</priority>
</test_case>

<test_case id="TC-UTML-10" type="unit" req_ref="REQ-UTML-12">
  <description>Pruning rule three-condition agreement</description>
  <preconditions>None</preconditions>
  <inputs>Three negative-test scenarios: (a) L_step low + ΔΩ=0.3 + ΔΞ=0.5; (b) L_step low + ΔΩ=0.5 + ΔΞ=0.3; (c) L_step=0.5 + ΔΩ=0.5 + ΔΞ=0.5</inputs>
  <expected_result>None of (a), (b), (c) trigger pruning. (a) classified as OptimizerDysregulated; (b) classified as LatentCollapsing; (c) classified as Optimal. Only when all three pruning conditions agree is curriculum item pruned.</expected_result>
  <priority>critical</priority>
</test_case>

<test_case id="TC-UTML-11" type="integration" req_ref="REQ-UTML-10">
  <description>M(t) consolidation integral and Dissipating detection</description>
  <preconditions>None</preconditions>
  <inputs>Synthetic trace with mean(L_step)=0.5 over 1,000 steps but ΔΞ ≈ 0.1 throughout (ΔM ≈ 0.05); k_replay=1.0</inputs>
  <expected_result>M(t) accumulates slowly because L_step·ΔΞ ≈ 0.05; ΔM/mean(L_step) = 0.1/0.5 = 0.2 — at threshold; classifier surfaces Dissipating state; recommends state regulation per Section D, not more compute</expected_result>
  <priority>medium</priority>
</test_case>

<test_case id="TC-UTML-12" type="manual" req_ref="REQ-UTML-07">
  <description>Operator audit replay: read CF_MEJEPA_TRAIN_CERTS, verify 100,000-step chain, confirm target_collapse always 0</description>
  <preconditions>Full Phase 3 training run completed</preconditions>
  <inputs>CF_MEJEPA_TRAIN_CERTS dump for one full training run</inputs>
  <expected_result>Auditor can read every cert; chain verifies end-to-end; ΔΞ.target_collapse == 0.0 for every step; FSV evidence reproduces operator-readable summary</expected_result>
  <priority>high</priority>
</test_case>

<test_case id="TC-UTML-13" type="e2e" req_ref="REQ-UTML-02">
  <description>FSV protocol per doc 11 §11.1: 100 synthetic examples spanning corner cases</description>
  <preconditions>compute_delta_p shipped; predictor + frozen target shipped</preconditions>
  <inputs>100 synthetic (panel_t01, panel_t2) pairs spanning: perfect prediction, total miss, high-drift, low-grad, collapsed latent (synthetic, since real ME-JEPA cannot collapse target), OOD</inputs>
  <expected_result>Computed L_step matches expected envelope per formula for each case; FSV evidence at /tmp/contextgraph-mejepa-train-fsv/utml-l-step-evidence.json passes assertions; 1-epoch dry-train confirms high-L_step examples sampled more often</expected_result>
  <priority>critical</priority>
</test_case>

<test_case id="TC-UTML-14" type="unit" req_ref="REQ-UTML-03">
  <description>embedder_coherence bootstrap neutral on small corpora</description>
  <preconditions>14 content embedders loaded; corpus &lt; 1,000 entries</preconditions>
  <inputs>500-entry corpus; query panel</inputs>
  <expected_result>embedder_coherence returns 0.5 (bootstrap neutral); cert.embedder_coherence_source="bootstrap_neutral_n_lt_1000"; per doc 12 §5.1 risk #3</expected_result>
  <priority>medium</priority>
</test_case>

<test_case id="TC-UTML-15" type="integration" req_ref="REQ-UTML-04">
  <description>agent_state_score E17 transcript-window probe</description>
  <preconditions>E17 AffectText embedder loaded; recent operator conversation transcript available</preconditions>
  <inputs>Transcript window covering recent /loop turn</inputs>
  <expected_result>agent_state_score = E17.valence × E17.arousal_inverted_U; in [0, 1]; defaults to 0.7 if transcript window unavailable</expected_result>
  <priority>medium</priority>
</test_case>

</test_plan>

<open_questions>

<question id="Q-UTML-01" status="open" assignee="operator">
  <text>Should the embedder_coherence weight (0.05) inside ΔK be tunable per language (since cross-language NN-set agreement may differ structurally between Python and Rust)?</text>
  <context>Doc 12 §5.1 specifies a fixed 0.05 weight. Doc 10 §C cross-language mandate says all primitives must work across all 11 languages. If 0.05 is wrong for some languages, it could systematically distort ΔK.</context>
  <resolution></resolution>
</question>

<question id="Q-UTML-02" status="open" assignee="operator">
  <text>How often should the periodic pairwise-MI audit (which feeds predictor_redundancy) run? Every step, every 100 steps, every 1,000 steps?</text>
  <context>Doc 11 §5.4 says "periodically; cached between updates" but does not specify cadence. Too frequent and step latency exceeds NFR-UTML-01 budget. Too infrequent and predictor_redundancy is stale.</context>
  <resolution></resolution>
</question>

<question id="Q-UTML-03" status="open" assignee="operator">
  <text>What is the configurable N for "L_step &lt; 0.1 for N consecutive steps" in the Stuck classification + pruning rule?</text>
  <context>Doc 11 §10 + UTML §X say "for N steps" without specifying N. Phase 5 self-healing controller needs a default. Suggest default N=100 with override via models_config.toml.</context>
  <resolution></resolution>
</question>

<question id="Q-UTML-04" status="open" assignee="operator">
  <text>Should ΔΩ_mean and ΔΞ_mean for verify(patch) calibrated_confidence use a rolling window (last 1,000 steps) or a fixed-size window (last 100 steps)?</text>
  <context>UTML §XIV doesn't specify window size. Larger window → more stable but slower to react to degradation. Smaller window → reactive but noisy.</context>
  <resolution></resolution>
</question>

<question id="Q-UTML-05" status="open" assignee="operator">
  <text>Is the Reward formula 0.4·conf + 0.3·mean(L_step·ΔΞ) + 0.3·gtau_pass_rate intended as fixed weights or tunable? Doc 11 §12 shows 0.6·conf + 0.4·mean(L_step·ΔΞ) (without the gtau term).</text>
  <context>Two slightly different reward formulas appear in the source docs: doc 11 §12 has 2-term (0.6/0.4), the user-specified spec scope shows 3-term (0.4/0.3/0.3) including gtau_pass_rate. Need operator confirmation which is canonical.</context>
  <resolution></resolution>
</question>

</open_questions>

<glossary>
  <term name="L_step">Per-update Learning Efficiency. Multiplicative product of four factors in [0, 1]. Genuine generalization-gain per gradient step.</term>
  <term name="ΔP">Delta Predictive-Error. Cosine error in latent space between predicted panel and frozen-instrument target panel, normalized by SNR, with imagination discount and exploration bonus.</term>
  <term name="ΔK">Delta Knowledge-Coherence. 4-term weighted sum: gradient cos_align (0.5) + Fisher protection (0.3) + calibration (0.15) + cross-embedder coherence (0.05).</term>
  <term name="ΔΩ">Delta Optimization-State. Geometric product of effective_plasticity^0.4 · landscape_health^0.4 · stability_floor^0.1 · agent_state^0.1.</term>
  <term name="ΔΞ">Delta Representational-Non-Collapse. (1 − target_collapse)^0.5 · (1 − redundancy)^0.3 · (1 − violation)^0.2. target_collapse = 0 by construction for ME-JEPA.</term>
  <term name="cos_align">Cosine alignment of current batch gradient g_t with EMA-averaged historical gradient ḡ_prior. Negative values indicate catastrophic forgetting.</term>
  <term name="fisher_violation">Fisher-weighted L2 distance between current parameters θ_t and consolidated θ_prior; quantifies how destructive the update is to consolidated knowledge.</term>
  <term name="ECE">Expected Calibration Error. Difference between model's reported confidence and empirical accuracy. Rising ECE during training is a Goodhart early indicator.</term>
  <term name="SNR(t)">Gradient Signal-to-Noise Ratio at step t. Liu et al. 2020. High noise → low SNR → ΔP shrinks because gradient is uninformative.</term>
  <term name="effective_plasticity">Composite of ReLU dormancy fraction + gradient-covariance effective rank + per-parameter GSNR distribution. Sutton 2024.</term>
  <term name="landscape_health">sigmoid(-λ_max(H_t)) · inverted_U(‖g_t‖). Penalizes sharp minima (SAM) and vanishing/exploding gradients (Yerkes-Dodson).</term>
  <term name="stability_floor">Discrete {1.0, 0.4, 0.1, 0.0} hard gate. NaN/Inf detection, validation-rising prolonged, Goodhart catastrophe → 0.0.</term>
  <term name="agent_state_score">E17 AffectText valence × arousal_inverted_U on outer-loop conversation transcripts. Per doc 12 §5.3.</term>
  <term name="target_collapse">Target-side representational collapse measure. MUST equal 0.0 for ME-JEPA by construction (frozen externally-defined instruments have zero trainable parameters).</term>
  <term name="predictor_redundancy">1 minus average normalized pairwise MI across panel slots. Periodic audit; > 0.5 triggers VICReg coefficient bump.</term>
  <term name="constellation_violation_rate">Fraction of recent Phase 4b TCT Gτ guard rejections. Indicates predictor producing off-constellation predictions.</term>
  <term name="embedder_coherence">Mean Jaccard similarity of nearest-neighbor sets across content embedders (E1-E14) for a candidate panel. Doc 12 §5.1.</term>
  <term name="M(t)">Consolidation integral. Σ L_step(τ) · ΔΞ(τ) · k_replay(τ) · exp(-(t-τ)/τ_decay). The "physical M(t)" is the audit substrate (CF_MEJEPA_TRAIN_CERTS).</term>
  <term name="T(train, deploy)">State-transfer term. exp(-d(P_train, P_deploy)/λ). Measures distribution shift from training corpus to deployment.</term>
  <term name="performance_deploy">M(t) · T(train, deploy). Phase 8 ≥0.95 ship gate target.</term>
  <term name="sampling_weight">Per-example training-batch weight. base · L_step · (1 + 5·override) · decay^age_days.</term>
  <term name="k_replay">Replay multiplier. 1.0 normal training; 2-4 during replay-buffer / EMA / SWA / distillation.</term>
  <term name="τ_decay">Decay time-constant for un-consolidated weights. 7 days un-EWC++-bound; 365 days high-Fisher consolidated.</term>
  <term name="UTML cycle classifier">Maps (L_step, ΔΩ, ΔΞ, mean(L_step), ΔM/mean(L_step)) to one of {Stuck, OptimizerDysregulated, LatentCollapsing, Confused, Optimal, Boring, Dissipating}.</term>
  <term name="Pruning rule">prune_curriculum_item iff (L_step &lt; 0.1 for N steps) AND (ΔΩ &gt; 0.4 throughout) AND (ΔΞ &gt; 0.4 throughout). All three signals must agree.</term>
  <term name="calibrated_confidence">clamp_[0.10, 0.95](raw_conf · conv_rate · strategy_agreement · evidence · ΔΩ_mean · ΔΞ_mean). Reported as confidence interval in verify(patch) verdicts.</term>
  <term name="Reward">0.4 · calibrated_confidence + 0.3 · mean(L_step · ΔΞ) + 0.3 · gtau_pass_rate. Phase 4b TCT integration.</term>
  <term name="Frozen instrument">An Instrument trait impl with zero trainable parameters. Deterministic function from typed event to feature output. Cannot drift; cannot collapse; no gradient flow.</term>
  <term name="Per-step training certificate">JSON record persisted to CF_MEJEPA_TRAIN_CERTS keyed by step. Hash-chained via parent_witness_hash → self_hash. Records all four factors + components + sampling_weight + grad_norm + drift_coverage + Merkle root + provenance.</term>
</glossary>

</functional_spec>
```

# SPEC-SELF-HEALING — ME-JEPA-Code Self-Healing Operations (Phase 5)

> **Level 2 — Functional Specification.** Defines WHAT the self-healing domain (Phase 5) must do, not HOW. Implementation details live in `TECH-SELF-HEALING.md`. Conflicts with the constitution resolve to the constitution per `<spec_authority_order>`.

```xml
<functional_spec id="SPEC-SELF-HEALING" version="1.0">

<metadata>
  <title>ME-JEPA-Code Self-Healing — Online observe() with EWC++, Drift Detection, A/B/C Retrain Promotion, Periodic Full Retrain, Continuous Calibration, Witness-Chain Integrity, LoRA Refresh on Plasticity Collapse, Distillation-Based Online Update</title>
  <status>draft</status>
  <owner>Phase 5 ME-JEPA implementation team (Claude Code outer optimizer)</owner>
  <created_date>2026-05-09</created_date>
  <last_updated>2026-05-09</last_updated>
  <related_specs>
    <spec_ref>SPEC-INSTRUMENTS</spec_ref>      <!-- Phase 1 panel substrate -->
    <spec_ref>SPEC-EMBEDDERS</spec_ref>        <!-- 21-embedder substrate; LoRA/distillation infrastructure -->
    <spec_ref>SPEC-CORPUS</spec_ref>           <!-- Phase 0 corpus + 10% calibration split + 10% holdout split -->
    <spec_ref>SPEC-PREDICTOR</spec_ref>        <!-- Phase 2 predictor (theta) -->
    <spec_ref>SPEC-TRAINING</spec_ref>         <!-- Phase 3 training loop; per-step training certs -->
    <spec_ref>SPEC-INFERENCE</spec_ref>        <!-- Phase 4a verify(); RealityPrediction.confidence -->
    <spec_ref>SPEC-TCT</spec_ref>              <!-- constellation re-fit on ΔΞ.constellation_violation_rate high -->
    <spec_ref>SPEC-WITNESS</spec_ref>          <!-- 73-byte SHAKE-256 chain integrity -->
    <spec_ref>SPEC-UTML</spec_ref>             <!-- L_step = ΔP × ΔK × ΔΩ × ΔΞ regulate-state-first cycle -->
    <spec_ref>SPEC-SHIFT-SUBSCRIBER</spec_ref> <!-- Phase 7 §3.8 feeder of observe() -->
    <spec_ref>SPEC-HYGIENE</spec_ref>          <!-- Phase 6 GC of old promotion-cycle weights -->
  </related_specs>
  <source_documents>
    <source type="design">docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §5 (entire self-healing section: §5.1 online observe + EWC++; §5.2 drift detection; §5.3 A/B/C retrain + promote; §5.4 periodic full retrain; §5.5 continuous calibration; §5.6 witness-chain integrity), §7 Phase 5 (deliverables + DoD + verification command)</source>
    <source type="design">docs/ruvectorfindings/10_MEJEPA_PIVOT_2026_05_09.md Section D (self-healing protocol with UTML cycle; the regulate-state-first ASSESS/REGULATE/OBSERVE/COMPUTE/CLASSIFY/INTERVENE/EVALUATE seven-step), Section F priorities (#5 self-healing + UTML cycle is the operating procedure)</source>
    <source type="design">docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §9 (training cycle UTML §IX adapted; coaching-order rule), §10 (pruning rule: all three ΔΩ/ΔΞ/L_step must agree)</source>
    <source type="design">docs/ruvectorfindings/12_EMBEDDER_INTEGRATION.md §6 (LoRA + distillation as ΔK/ΔΞ/ΔΩ instrumentation), §8 (self-healing via shipped LoRA + distillation: refresh_lora_adapter + distillation_loop with EMA τ=0.996 + signal_clarity quality gate), §11 items 15-16 (LoRA self-healing wiring + distillation-based online observe)</source>
    <source type="constitution">docs/ruvectorfindings/specs/constitution.md (utml_v1_invariants: regulate ΔΩ + ΔΞ FIRST coaching order, ΔΞ.target_collapse=0 by construction, pruning rule all-three-agree; failure_taxonomy: MEJEPA_OBSERVE_BATCH_NAN, MEJEPA_OBSERVE_DRIFT_DETECTED, MEJEPA_OBSERVE_INTEGRITY_VIOLATION, MEJEPA_RETRAIN_HOLDOUT_REGRESSION, MEJEPA_RETRAIN_QUOTA_EXCEEDED; coding_standards error_handling: no silent fallback, no unwrap_or(0.5) on confidence intervals, no mock data in tests; performance_budgets: rocksdb_disk_total_quota ≤ 115 GB)</source>
    <source type="memory">memory/decisions/2026-05-09--single-loop-claude-code-actor-mejepa-data-factory.md (the Claude-Code outer reality loop is the data factory; the shift-subscriber feeds observe(); no inner-actor LLM)</source>
    <source type="memory">memory/feedback_no_backwards_compat.md (pre-production; wipe DB on schema change; no migration shims)</source>
  </source_documents>
</metadata>

<overview>
The self-healing domain is the operational continuity layer of ME-JEPA-Code. It runs after Phase 4
inference is shipped and turns ME-JEPA from a static checkpoint into a system that learns from
every Claude-Code reality-loop turn while structurally protecting against catastrophic forgetting,
silent calibration drift, weight corruption, and substrate degradation. Eight components compose
the surface: (1) **online `observe()` with EWC++** — every `(panel, oracle_outcome)` pair from the
shift-subscriber takes one regularized SGD step on the predictor with a Fisher-information
penalty against earlier-task weight snapshots; (2) **drift detection** — the rolling 1,000-attempt
window of `(predicted_set, actual_oracle)` pairs computes empirical conformal coverage and maps
it to four severity bands (Healthy 88–92 % / Soft 85–88 % / Hard 80–85 % / Catastrophic &lt; 80 %);
(3) **A/B/C retrain + promotion gating** — on Hard or Catastrophic drift, the system spawns Mode A
(current production), Mode B (retrain from corpus snapshot), Mode C (A+B ensemble weighted by
holdout score), evaluates all three, and promotes the winner only if it beats Mode A on three
gates (coverage ≥ stated, oracle-agreement ≥ A's score, OOD distribution does not regress);
(4) **periodic full retrain** — every N=10,000 attempts the system retrains from a corpus snapshot
regardless of drift, catching slow accumulated drift the rolling window cannot see;
(5) **continuous calibration** — every 1,000 attempts the conformal threshold τ and OOD bandwidth
σ² are recomputed on the most-recent 1,000 calibration examples (sliding window) so the prediction
set adapts to recent input distribution; (6) **witness-chain integrity check** — every 10,000
attempts (or on drift) the chain is replayed end-to-end (`chain.len() % 73 == 0`, `prev_hash`
chain valid); break ⇒ halt inference and restore from last verified checkpoint;
(7) **LoRA adapter refresh on plasticity collapse** — when ΔΩ.effective_plasticity &lt; 0.4 the
shipped per-embedder LoRA adapter (rank=32) is fine-tuned on a fresh corpus slice for 10 epochs;
frozen-base embedders never lose plasticity, so only the adapter needs refreshing;
(8) **distillation-based online update** — for each recent observation, oracle_score is the teacher
and embedder_score the student under InfoNCE loss with EMA update τ=0.996; the shipped
`quality_gate: true` skips low-confidence examples (signal_clarity baked in).

The whole loop runs under the **UTML v1.0 regulate-state-first cycle** per doc 11 §9 and doc 10 §D:
ASSESS ΔΩ + ΔΞ FIRST (state intervention before content), REGULATE if either &lt; 0.4 (continual
backprop / SAM tightening / LR cosine restart / VICReg coefficient bump / constellation re-fit /
halt+rollback when stability_floor → 0), OBSERVE ΔP and ΔK, COMPUTE L_step = ΔP × ΔK × ΔΩ × ΔΞ,
CLASSIFY learning state (Stuck / Optimizer-Dysregulated / Latent-Collapsing / Confused / Optimal /
Mastery / Dissipating), INTERVENE substrate-first then content, EVALUATE next eval window. Without
this ordering, prediction-error tweaks waste compute on a degenerate substrate.

What self-healing **MUST NOT** mean (per doc 10 §D, locked): NOT silent fallback (every drift
event surfaces a journal entry, a blocker if unresolvable, and a `status_change` field on every
prediction made during the heal cycle); NOT unsupervised weight changes (every promotion is
witness-chain-recorded with a 32-byte holdout-evaluation summary hash); NOT accepting noisy labels
as gold (`signal_clarity` from ΔE multiplicatively gates updates so weak labels contribute less).
Robust error logging is mandatory per the operator directive: every fallible op returns a
taxonomy-defined error with `.code()`; no `unwrap_or(0.5)` on confidence intervals; no mock data
in tests; every failure logs context (input shapes, file paths, SHAs, cert offsets) before
returning.

Beneficiaries: (1) the Phase 4 inference layer benefits from continually-fresh calibration thresholds
and OOD bandwidths without manual operator intervention; (2) the operator observes promotion events
through the witness chain plus SMAP report and can roll back any promotion using the retained
old-weights snapshot; (3) the Phase 7 mejepa-shift-subscriber consumes the `observe()` API as its
sole termination point per the cross-cutting SPEC-SHIFT-SUBSCRIBER contract; (4) downstream consumers
of `mcp__cgreality__mejepa_verify` see `degraded` / `retraining` / `paused` `status_change` fields
when the heal cycle is active so they can treat verdicts as advisory; (5) the corpus itself
benefits from active learning supervision when conformal sets exceed three (high-uncertainty in
distribution → surface for Docker oracle re-run), tightening the deployment-distribution gap T per
UTML §VIII.
</overview>

<user_types>

  <user_type id="UT-01" name="mejepa-shift-subscriber (Phase 7)">
    <description>The Tokio task that tails the shift log + witness-chain bytes and fires `observe()` on every shift event with a populated panel + oracle outcome (cross-cutting with SPEC-SHIFT-SUBSCRIBER). It is the only external caller of `observe()` in normal operation.</description>
    <permissions>WRITE: `observe(&amp;mut MeJepaCompiler, panel, oracle_outcome)` via internal in-process FFI; READ: CF_MEJEPA_SHIFT_WATERMARK to resume.</permissions>
    <goals>Per-shift `observe()` latency p50 &lt; 5 s, p99 &lt; 30 s (cross-cutting budget); never silently drop an observation; surface NaN/Inf as MEJEPA_OBSERVE_BATCH_NAN immediately.</goals>
  </user_type>

  <user_type id="UT-02" name="Self-Healing Background Scheduler">
    <description>A long-running scheduler component owned by Phase 5 that runs three time-driven jobs: drift detection on every observation, continuous calibration every 1,000 observations, witness-chain integrity check every 10,000 observations (or on drift), periodic full retrain every N=10,000 observations.</description>
    <permissions>WRITE: drift severity transitions to CF_MEJEPA_DRIFT_HISTORY; WRITE: heal reports to CF_MEJEPA_HEAL_REPORTS; WRITE: promotion witness-chain entries; READ: CF_MEJEPA_LIVE_PREDICTIONS rolling window; READ: CF_MEJEPA_TRAIN_CERTS for ΔΩ/ΔΞ window.</permissions>
    <goals>Detect synthetic 70 %-coverage drift within 1,000 observations; complete A/B/C promotion latency &lt; 60 min on RTX 5090; never propagate a regression (Mode B/C never promoted unless they beat A on all three gates).</goals>
  </user_type>

  <user_type id="UT-03" name="Operator (human reviewer)">
    <description>The human running the system. Observes drift events, signs off on calibration version bumps, decides whether OOD-flagged inputs justify expanding the corpus, can manually trigger `mejepa-heal-drill` for synthetic stress tests.</description>
    <permissions>READ all CF_MEJEPA_HEAL_REPORTS, CF_MEJEPA_DRIFT_HISTORY, FSV evidence at /tmp/contextgraph-mejepa-self-healing-fsv/; APPROVE manual rollback of a promotion via `mcp__cgreality__mejepa_rollback_to`; APPROVE max-age overrides per SEC-MEJEPA-04; cannot silently mutate weights or skip the witness-chain promotion entry.</permissions>
    <goals>Verify that drift events surface within 1,000 observations; verify that no promotion ever lands without a witness-chain entry; verify that LoRA refresh fires when plasticity collapses on synthetic stress.</goals>
  </user_type>

  <user_type id="UT-04" name="Phase 4a Inference Layer (MeJepaCompiler)">
    <description>The inference compiler whose `&amp;mut self` `observe()` method is the surface but whose body lives in this spec. It also reads the active calibration record (continuously updated by §5.5) and the latest constellation centroids (re-fit by REGULATE step on ΔΞ.constellation_violation_rate high).</description>
    <permissions>READ active calibration version (CF_MEJEPA_CALIBRATION_HISTORY tail); READ active constellation version; READ CF_MEJEPA_TRAIN_CERTS for ΔΩ/ΔΞ window. WRITE only via `observe()` body (delegated here); never mutates calibration or constellation directly — those are Phase 5 owned.</permissions>
    <goals>Continuity: the inference layer never observes a half-applied EWC++ Fisher update; atomic writes via `write_text_checked` (already shipped in `witness_chain_io.rs`) provide the crash-safe boundary.</goals>
  </user_type>

  <user_type id="UT-05" name="A/B/C Promotion Gate">
    <description>The component that, on Hard or Catastrophic drift, runs Mode A / B / C in parallel against the holdout split, computes per-mode scores (coverage + oracle agreement + OOD distribution KL), and either promotes a winner or returns MEJEPA_RETRAIN_HOLDOUT_REGRESSION.</description>
    <permissions>WRITE: new weight blob to CF_MEJEPA_WEIGHT_BLOBS keyed by `theta_sha`; WRITE: promotion witness-chain entry (op_type=ModelPromote, weights_sha, mode_winner, evaluation_summary_sha); READ: holdout split from CF_MEJEPA_MUTATION_CORPUS; cannot silently overwrite the active weight pointer — the promotion record is the pointer.</permissions>
    <goals>Promote Mode B or C only when all three gates pass; never silently fall back to Mode A when no winner exists (return error instead so operator can intervene).</goals>
  </user_type>

  <user_type id="UT-06" name="Reality-Loop Optimizer (Claude Code, this agent)">
    <description>The outer-loop driver per CLAUDE.md §11.2. Each /loop turn produces shift-log entries that the subscriber forwards to `observe()`. The optimizer is also the operator's surface for invoking the heal-drill verification command.</description>
    <permissions>READ Phase 5 status via `mcp__cgreality__mejepa_heal_status`; WRITE recommendations referencing heal events as remaining_risks; cannot directly call `observe()` (subscriber owns it) or trigger an unscheduled retrain (scheduler owns it).</permissions>
    <goals>Surface heal events in recommendations so the operator narrative captures them; never reintroduce inner-actor LLM (Qwen, Hermes) per anti_pattern; never bypass EWC++ regularization with a raw SGD step.</goals>
  </user_type>

</user_types>

<user_stories>

  <story id="US-HEAL-01" priority="must-have">
    <narrative>
      <as_a>mejepa-shift-subscriber (UT-01)</as_a>
      <i_want_to>Call `observe(panel, oracle_outcome)` for every shift event so the predictor folds the new (panel, oracle) pair into its weights with EWC++ Fisher protection</i_want_to>
      <so_that>ME-JEPA's prediction quality continually tracks the deployment distribution without catastrophic forgetting on older tasks</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-HEAL-01-01">
        <given>A `Panel` with 5,120-d filled and an `OracleOutcome` from a real Docker run with finite per-test pass logits</given>
        <when>The subscriber invokes `observe(&amp;panel, &amp;oracle_outcome)`</when>
        <then>The function: (1) computes online Fisher-information update; (2) takes one SGD step on `L_total = L_predict + λ_ewc · Σ_old_tasks F_i · (θ_i − θ_i*)²`; (3) checks for sona-style task boundary and snapshots if detected; (4) returns `Ok(ObserveOutput { theta_sha_after, l_step, delta_p, delta_k, delta_omega, delta_xi, fisher_rank })` within p50 &lt; 5 s, p99 &lt; 30 s</then>
      </criterion>
      <criterion id="AC-HEAL-01-02">
        <given>A `Panel` whose forward pass produces a non-finite gradient or activation (NaN / Inf injected via synthetic adversarial input)</given>
        <when>`observe(...)` runs</when>
        <then>Returns `Err(ObserveError::BatchNan { component, witness_chain_offset })` with `.code() == "MEJEPA_OBSERVE_BATCH_NAN"`; the predictor weights are NOT mutated (transactional rollback); the failure is logged with input shapes + file paths per constitution §error_handling; the SMAP blocker is appended to memory/blockers/ with severity:high</then>
      </criterion>
      <criterion id="AC-HEAL-01-03">
        <given>10 sequential task boundaries detected by `EwcPlusPlus::detect_task_boundary()` (per sona-style automatic detection: significant shift in the loss surface curvature)</given>
        <when>10 task snapshots have been taken</when>
        <then>The Fisher matrix is non-degenerate (full rank — rank-deficient is a bug per Phase 5 DoD); each snapshot persists to CF_MEJEPA_FISHER_SNAPSHOTS with corpus_sha + step_offset + frozen_at provenance</then>
      </criterion>
      <criterion id="AC-HEAL-01-04">
        <given>Initial λ_ewc = 2,000 with cap λ_max = 15,000 (per spec defaults)</given>
        <when>The system runs 10 task boundaries</when>
        <then>λ adapts in [2,000, 15,000]; never exceeds λ_max even when many old tasks accumulate (ramp policy clamped); the FSV evidence file ewc-fisher-evidence.json captures the per-boundary λ trajectory</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-HEAL-02" priority="must-have">
    <narrative>
      <as_a>Self-Healing Background Scheduler (UT-02)</as_a>
      <i_want_to>Maintain a rolling 1,000-attempt window of `(predicted_set, actual_oracle)` pairs and compute empirical coverage per doc 09 §5.2</i_want_to>
      <so_that>I detect drift severity (Healthy / Soft / Hard / Catastrophic) on every new observation and trigger downstream heal actions correctly</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-HEAL-02-01">
        <given>1,000 prior `RealityPrediction` records each with `outcome_set` and the corresponding actual oracle outcome from CF_MEJEPA_ORACLE_VERDICTS</given>
        <when>The scheduler computes `empirical_coverage = count(actual ∈ predicted_set) / 1000`</when>
        <then>Maps to `DriftSeverity::Healthy` if coverage ∈ [0.88, 0.92], `Soft` if [0.85, 0.88), `Hard` if [0.80, 0.85), `Catastrophic` if &lt; 0.80; the severity transition is durably recorded in CF_MEJEPA_DRIFT_HISTORY with timestamp + window_window_start_offset + severity</then>
      </criterion>
      <criterion id="AC-HEAL-02-02">
        <given>Drift severity transitions from Healthy to Hard</given>
        <when>The scheduler observes the transition</when>
        <then>(1) emits a journal entry to memory/journal/; (2) sets `status_change: "retraining"` on every prediction made during the heal cycle (cross-cutting with SPEC-INFERENCE); (3) spawns the A/B/C retrain pipeline (see US-HEAL-03); (4) returns `Err(ObserveError::DriftDetected { severity: Hard, empirical_coverage })` from concurrently-running observe() calls so the subscriber pauses its inflight observations until the retrain completes</then>
      </criterion>
      <criterion id="AC-HEAL-02-03">
        <given>Synthetic drift injection: force 70 % empirical coverage on the rolling window via a deterministic mock oracle that mismatches predictions</given>
        <when>The drift drill `cargo run --release -p context-graph-mejepa -- heal-drill --inject-drift hard` runs</when>
        <then>`drift_detected: hard` reported within ~700 observations (allow ±300 tolerance for the rolling-window lag); `retrain_triggered: yes`; `mode_winner: B` or `C`, never `A` unmodified; `promotion_latency &lt; 60 min` on RTX 5090; FSV evidence at /tmp/contextgraph-mejepa-self-healing-fsv/drift-drill-evidence.json</then>
      </criterion>
      <criterion id="AC-HEAL-02-04">
        <given>Drift severity is Catastrophic (coverage &lt; 80 %)</given>
        <when>The scheduler detects it</when>
        <then>Inference is paused (compile/verify return `status_change: "paused"`); a full retrain from corpus is triggered (not just A/B/C from current data); the operator is surfaced via memory/blockers/ severity:critical</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-HEAL-03" priority="must-have">
    <narrative>
      <as_a>A/B/C Promotion Gate (UT-05)</as_a>
      <i_want_to>Run Mode A (current production), Mode B (retrain from corpus), and Mode C (A+B ensemble weighted by holdout score) in parallel and promote the winner only when it beats Mode A on three gates</i_want_to>
      <so_that>Promotions never regress the system, weight changes are auditable through the witness chain, and rollback is one promotion-cycle worth of weights away</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-HEAL-03-01">
        <given>Hard or Catastrophic drift detected; the holdout split (10 % per SPEC-CORPUS, stratified by language) is loaded</given>
        <when>The gate spawns Mode A / B / C evaluation</when>
        <then>Mode A: existing weights θ_A frozen; Mode B: train_from_scratch on a snapshot of CF_MEJEPA_MUTATION_CORPUS at current_step; Mode C: ensemble with weights w_A, w_B chosen to maximize holdout score (small grid-search over [0,1] step 0.1); all three are evaluated on the holdout split with the SAME calibration version</then>
      </criterion>
      <criterion id="AC-HEAL-03-02">
        <given>Holdout-split evaluation produces per-mode (coverage, oracle_agreement, ood_distribution_kl) triples</given>
        <when>The gate computes the winner</when>
        <then>A mode is promotable iff: (1) `coverage ≥ stated_coverage` (default 0.90); (2) `oracle_agreement ≥ A's_score` (no regression); (3) `ood_distribution_kl &lt;= A's_kl` (OOD distribution does not regress). If multiple modes pass, the highest oracle_agreement wins. If none pass, return `Err(HealError::HoldoutRegression { mode_a_score, mode_b_score, mode_c_score })` with `.code() == "MEJEPA_RETRAIN_HOLDOUT_REGRESSION"`; do NOT silently keep A active</then>
      </criterion>
      <criterion id="AC-HEAL-03-03">
        <given>Mode B wins promotion</given>
        <when>The gate writes the new active-weight pointer</when>
        <then>(1) the new weight blob θ_B is persisted to CF_MEJEPA_WEIGHT_BLOBS keyed by `theta_sha = sha256(weights_serialized)` (deduplicated); (2) a witness-chain entry is appended: `(timestamp, op_type=ModelPromote, weights_sha=theta_sha_b, mode_winner="B", evaluation_summary_sha=sha256(holdout_eval_json))`; (3) the prior Mode A weights are retained in CF_MEJEPA_WEIGHT_BLOBS for one promotion-cycle (next promotion GCs them); (4) FSV evidence at /tmp/contextgraph-mejepa-self-healing-fsv/abc-promotion-evidence.json captures all three modes' scores plus the chosen winner + theta_sha</then>
      </criterion>
      <criterion id="AC-HEAL-03-04">
        <given>Two promotions are triggered concurrently (e.g. simultaneous drift detection and operator-issued retrain)</given>
        <when>The second promotion attempts to acquire the promotion lock</when>
        <then>Returns `Err(HealError::PromotionDeadlock { holder: pid_or_thread_id })` with `.code() == "MEJEPA_HEAL_PROMOTION_DEADLOCK"`; the second promotion is rejected (no double-write to the active-weight pointer); the operator is surfaced; the journal records both attempts</then>
      </criterion>
      <criterion id="AC-HEAL-03-05">
        <given>A retrain is in progress and disk usage on RocksDB exceeds 90 % of `rocksdb_disk_total_quota` (default 115 GB)</given>
        <when>The gate detects the quota pressure</when>
        <then>Returns `Err(HealError::QuotaExceeded { used_gb, quota_gb })` with `.code() == "MEJEPA_RETRAIN_QUOTA_EXCEEDED"`; the retrain is paused; the Phase 6 hygiene module is triggered to evict cold-tier corpus; on success the retrain resumes</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-HEAL-04" priority="must-have">
    <narrative>
      <as_a>Self-Healing Background Scheduler (UT-02)</as_a>
      <i_want_to>Trigger a full retrain every N=10,000 observations regardless of drift status, snapshot the corpus immutably, and promote only if winner_score &gt; current_score + 0.02</i_want_to>
      <so_that>Slow accumulated drift the rolling window cannot see is caught and corrected; corpus reproducibility is preserved via the immutable snapshot</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-HEAL-04-01">
        <given>The observation counter has just crossed an N=10,000 multiple</given>
        <when>The scheduler triggers `schedule_full_retrain()`</when>
        <then>(1) `snapshot_current_corpus()` writes an immutable corpus snapshot keyed by `corpus_sha + frozen_at`; (2) `train_from_scratch(snapshot)` ignores online weights and trains fresh; (3) `evaluate_on_holdout()` computes the score; (4) promote only if `winner_score &gt; current_score + 0.02` (the +0.02 floor prevents jitter promotions); (5) regardless of promotion the snapshot is retained for reproducibility</then>
      </criterion>
      <criterion id="AC-HEAL-04-02">
        <given>The full retrain produces winner_score = current_score + 0.015 (does not exceed +0.02 floor)</given>
        <when>The gate runs</when>
        <then>No promotion occurs; the heal report records `mode_winner: "A_unchanged_no_floor_clearance"`; old weights remain active; the snapshot is still durably retained</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-HEAL-05" priority="must-have">
    <narrative>
      <as_a>Self-Healing Background Scheduler (UT-02)</as_a>
      <i_want_to>Recompute the conformal threshold τ and OOD bandwidth σ² every 1,000 observations on the most-recent 1,000 calibration examples (sliding window) per doc 09 §5.5</i_want_to>
      <so_that>The prediction set adapts to recent input distribution rather than using a stale calibration that drifts away from deployment</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-HEAL-05-01">
        <given>The observation counter has just crossed a 1,000-multiple AND ≥ 1,000 examples are available in the calibration sliding window</given>
        <when>Continuous calibration runs</when>
        <then>Computes new τ as `(⌈(N+1)·(1−α)⌉/N)`-quantile of non-conformity scores per SPEC-INFERENCE REQ-INFER-06; computes new σ² so calibration examples score &lt; 0.30; persists to CF_MEJEPA_CALIBRATION_HISTORY with `frozen_at`, `corpus_sha`, `embedder_versions`; the inference layer reads the new record on next compile() call</then>
      </criterion>
      <criterion id="AC-HEAL-05-02">
        <given>Fewer than 1,000 calibration examples are available (early in deployment)</given>
        <when>Continuous calibration runs</when>
        <then>Returns `Err(HealError::ConformalInsufficientSamples { observed, required: 1000 })` with `.code() == "MEJEPA_CALIBRATION_INSUFFICIENT_SAMPLES"`; falls back to the last-known calibration record loaded from CF_MEJEPA_CALIBRATION_HISTORY tail; surfaces `degraded` status on every prediction made under that threshold per doc 10 §D ("MUST NOT mean silent fallback")</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-HEAL-06" priority="must-have">
    <narrative>
      <as_a>Self-Healing Background Scheduler (UT-02)</as_a>
      <i_want_to>Replay the witness chain end-to-end every 10,000 observations (or on drift) per doc 09 §5.6</i_want_to>
      <so_that>Disk corruption or tampering is detected before it propagates into a promotion or a calibration update, and the system halts inference until the operator can investigate</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-HEAL-06-01">
        <given>A witness chain whose `chain.len() % 73 != 0` (truncation) or whose `prev_hash` chain is broken at offset i</given>
        <when>The integrity check runs</when>
        <then>Returns `Err(HealError::IntegrityViolation { kind: WrongSize | BrokenAt(i) })` with `.code() == "MEJEPA_OBSERVE_INTEGRITY_VIOLATION"`; halts inference (`status_change: "paused"` on every subsequent prediction); restores from last verified checkpoint (the prior promotion's weights_sha and the prior calibration version); surfaces a memory/blockers/ entry severity:critical</then>
      </criterion>
      <criterion id="AC-HEAL-06-02">
        <given>A 100,000-entry witness chain that replays clean</given>
        <when>The check runs</when>
        <then>Returns `Ok(ChainIntegrityReport::Valid { entry_count: 100_000 })`; FSV evidence at /tmp/contextgraph-mejepa-self-healing-fsv/witness-integrity-evidence.json captures the entry count + first/last entry SHAs + chain replay duration_ms</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-HEAL-07" priority="must-have">
    <narrative>
      <as_a>Self-Healing Background Scheduler (UT-02), running the UTML §IX REGULATE step</as_a>
      <i_want_to>Detect ΔΩ.effective_plasticity &lt; 0.4 in the per-step training-cert window and refresh the corresponding embedder's LoRA adapter on a fresh corpus slice (rank=32, epochs=10)</i_want_to>
      <so_that>Plasticity collapse — the inability of the predictor to incorporate new gradients — is repaired without re-initializing dormant units (which would lose old-task knowledge), per doc 12 §6 + §8</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-HEAL-07-01">
        <given>The last 128 training certs in CF_MEJEPA_TRAIN_CERTS show mean ΔΩ.effective_plasticity = 0.32 (below the 0.4 floor)</given>
        <when>The REGULATE step runs the substrate-first regulation per UTML §IX</when>
        <then>(1) Identifies which embedder's LoRA adapter is the bottleneck (highest contribution to ΔK degradation); (2) calls `refresh_lora_adapter(embedder_id, fresh_corpus_slice, rank=32, epochs=10)` against the shipped `crates/context-graph-embeddings/src/training/lora.rs` infrastructure; (3) on success, writes a refresh report to CF_MEJEPA_HEAL_REPORTS; (4) on failure (NaN gradient, OOM), returns `Err(HealError::LoraRefreshFail { embedder_id, cause })` with `.code() == "MEJEPA_HEAL_LORA_REFRESH_FAIL"`</then>
      </criterion>
      <criterion id="AC-HEAL-07-02">
        <given>The frozen-base embedder weights are intact (frozen by construction per constitution `<utml_v1_invariants>` rule on `ΔΞ.target_collapse=0`)</given>
        <when>LoRA refresh runs</when>
        <then>Only the LoRA adapter weights are mutated; the base embedder weights are byte-equal before and after; the `frozen_base_sha` field on the FSV evidence at /tmp/contextgraph-mejepa-self-healing-fsv/lora-refresh-evidence.json verifies this byte-equality</then>
      </criterion>
      <criterion id="AC-HEAL-07-03">
        <given>The Fisher matrix accumulated across past task boundaries is rank-deficient (a bug per Phase 5 DoD)</given>
        <when>EWC++ snapshots a new task</when>
        <then>Returns `Err(HealError::FisherRankDeficient { rank, dim })` with `.code() == "MEJEPA_HEAL_FISHER_RANK_DEFICIENT"`; halts further EWC++ updates; operator-surfaced</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-HEAL-08" priority="must-have">
    <narrative>
      <as_a>Self-Healing Background Scheduler (UT-02), running the distillation-based online observe per doc 12 §8</as_a>
      <i_want_to>For every recent observation, treat oracle_score as the teacher and embedder_score as the student, compute InfoNCE loss, backprop through the student, and EMA-update student weights with τ=0.996</i_want_to>
      <so_that>Embedders consolidate the new ground-truth signal alongside the predictor, the `quality_gate: true` (signal_clarity baked-in) skips low-confidence examples, and the shipped distillation infrastructure is reused rather than reinvented</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-HEAL-08-01">
        <given>A batch of recent observations from the shift-subscriber, each carrying `(panel, oracle_outcome, signal_clarity)`</given>
        <when>The distillation loop runs against the shipped `crates/context-graph-embeddings/src/training/distillation.rs`</when>
        <then>For each active embedder: `teacher_score = oracle.score(panel)`, `student_score = embedder.score(panel)`, `loss = info_nce_loss(student, teacher)`, `student.backward(loss)`, `ema_update(student.weights, τ=0.996)`; examples with `signal_clarity &lt; quality_gate_threshold` (default 0.5) are skipped</then>
      </criterion>
      <criterion id="AC-HEAL-08-02">
        <given>EMA τ=0.996 is the doc-12-mandated coupling rate (the "future-self prior" of UTML §VIII)</given>
        <when>Student weights update</when>
        <then>The update is `w_student ← τ·w_student + (1−τ)·w_student_after_grad` (i.e., Polyak averaging); never overwritten via plain assignment; the τ value is configurable via `MeJepaHealConfig.distillation_ema_tau` but defaults to 0.996</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-HEAL-09" priority="must-have">
    <narrative>
      <as_a>Operator (UT-03)</as_a>
      <i_want_to>The whole self-healing loop to run under the UTML v1.0 regulate-state-first cycle: ASSESS ΔΩ + ΔΞ FIRST, REGULATE if either &lt; 0.4 (substrate intervention), then OBSERVE / COMPUTE / CLASSIFY / INTERVENE / EVALUATE</i_want_to>
      <so_that>Prediction-error tweaks never waste compute on a degenerate substrate; coaching order is enforced per doc 11 §9 + doc 10 §D + constitution `<utml_v1_invariants>` rule 3</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-HEAL-09-01">
        <given>A heal cycle iteration triggered by drift or by the periodic schedule</given>
        <when>The cycle runs</when>
        <then>Step 1 ASSESS: read ΔΩ.{effective_plasticity, landscape_health, stability_floor} and ΔΞ.{target_collapse, predictor_redundancy, constellation_violation_rate} from the last-128-step training-cert window. Step 2 REGULATE if ΔΩ &lt; 0.4 OR ΔΞ &lt; 0.4: branch to (effective_plasticity collapsed → continual backprop selective re-init OR LoRA refresh per US-HEAL-07; landscape_health bad → SAM perturbation tightening + LR cosine-anneal restart; stability_floor → 0 → halt + rollback + memory/blockers/ entry; predictor_redundancy high → bump VICReg coefficients; constellation_violation_rate high → re-fit constellation per SPEC-TCT). Step 3 OBSERVE ΔP, ΔK. Step 4 COMPUTE L_step = ΔP × ΔK × ΔΩ × ΔΞ (multiplicative gating; never additive). Step 5 CLASSIFY into one of seven learning states. Step 6 INTERVENE substrate-first then content. Step 7 EVALUATE on next eval window</then>
      </criterion>
      <criterion id="AC-HEAL-09-02">
        <given>ΔΞ.target_collapse &gt; 0 detected (a bug — instruments must be frozen by construction per constitution rule 1 of utml_v1_invariants)</given>
        <when>The cycle runs</when>
        <then>Returns `Err(HealError::CriticalBug { kind: TargetCollapseNonZero })` immediately; halts the cycle; emits memory/blockers/ severity:critical labelled "instrument has accidentally become trainable — fix at trait level"</then>
      </criterion>
      <criterion id="AC-HEAL-09-03">
        <given>The pruning rule per doc 11 §10: prune curriculum item only if (L_step &lt; 0.1 for N steps) AND (ΔΩ &gt; 0.4 throughout) AND (ΔΞ &gt; 0.4 throughout)</given>
        <when>The CLASSIFY step considers pruning</when>
        <then>All three conditions must hold simultaneously. If any is violated, the diagnosis is rerouted: ΔΩ low → Optimizer-Dysregulated (REGULATE first, do NOT prune); ΔΞ low → Latent-Collapsing (REGULATE first, do NOT prune); per constitution `<utml_v1_invariants>` rule 5 verbatim</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-HEAL-10" priority="should-have">
    <narrative>
      <as_a>Operator (UT-03)</as_a>
      <i_want_to>Execute `cargo run --release -p context-graph-mejepa -- heal-drill --inject-drift hard` and observe a complete heal cycle end-to-end with deterministic synthetic stress</i_want_to>
      <so_that>I can sign off Phase 5 DoD before promoting the self-healing components to production</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-HEAL-10-01">
        <given>The drill is invoked with `--inject-drift hard`</given>
        <when>The command runs</when>
        <then>Output reports: `drift_detected: hard, after ~700 observations`; `retrain_triggered: yes`; `mode_winner: B` or `C` (never `A` unmodified); `promotion_latency &lt; 60 min` on RTX 5090; FSV evidence files written: drift-drill-evidence.json, abc-promotion-evidence.json, ewc-fisher-evidence.json, lora-refresh-evidence.json, witness-integrity-evidence.json (all mode 0o600 per SEC-MEJEPA-07)</then>
      </criterion>
    </acceptance_criteria>
  </story>

</user_stories>

<requirements>

  <requirement id="REQ-HEAL-01" story_ref="US-HEAL-01" priority="must">
    <description>Expose `pub fn observe(&amp;mut self, panel: &amp;Panel, oracle_outcome: &amp;OracleOutcome) -&gt; Result&lt;ObserveOutput, ObserveError&gt;` on `MeJepaCompiler`. The body: (1) computes per-example Fisher information online via `EwcPlusPlus::update_fisher_online`; (2) takes one SGD step on `L_total = L_predict + λ_ewc · Σ_old_tasks F_i · (θ_i − θ_i*)²`; (3) checks `EwcPlusPlus::detect_task_boundary()` and snapshots if boundary detected; (4) returns ObserveOutput carrying `theta_sha_after`, `l_step`, `delta_p`, `delta_k`, `delta_omega`, `delta_xi`, `fisher_rank`.</description>
    <rationale>Per doc 09 §5.1 verbatim; signature matches SPEC-INFERENCE REQ-INFER-01 trait stub; `&amp;mut self` mandatory for online weight update.</rationale>
    <constraints>EWC++ defaults: `initial_lambda=2000`, `max_lambda=15000`. Tune empirically (constitution `<performance_budgets>` allows the empirical band but not violation of max_lambda). All loss components must be finite at exit; constructor returns Err on NaN/Inf. Atomic durability via `write_text_checked` (already shipped in `witness_chain_io.rs`) so a crash mid-update never leaves half-applied EWC++ Fisher updates per doc 09 §10 item 20.</constraints>
    <verification_method>Unit test asserts signature stability; integration test injects 100 observations and asserts (a) weight delta is non-zero, (b) Fisher matrix grows full-rank, (c) λ stays in [2000, 15000], (d) atomic write semantics (kill the process mid-write and verify the chain remains consistent).</verification_method>
  </requirement>

  <requirement id="REQ-HEAL-02" story_ref="US-HEAL-01" priority="must">
    <description>The `EwcPlusPlus` struct carries: `fisher_matrix: FisherMatrix` (online running estimate), `lambda: f32` (current regularization weight in [initial_lambda, max_lambda]), `task_snapshots: Vec&lt;TaskSnapshot&gt;` (one per detected boundary; bounded; oldest evicted via SVD merge per doc 12 §11 risk #5 cap of 1 base + ≤4 deltas), `boundary_detector: BoundaryDetectorState`. `EwcPlusPlus::update_fisher_online(panel, oracle_outcome) -&gt; FisherUpdate`, `EwcPlusPlus::detect_task_boundary() -&gt; bool` (sona-style automatic), `EwcPlusPlus::snapshot_current_task()`, `EwcPlusPlus::current_lambda() -&gt; f32`.</description>
    <rationale>Per doc 09 §5.1 + doc 11 §V.3 (Fisher protection is one of three ΔK signals). Sona-style automatic boundary detection per `crates/sona/src/ewc.rs` clean-room reimplementation per doc 09 §5.1.</rationale>
    <constraints>Fisher matrix must remain full-rank after 10 task boundaries (rank-deficient is a bug per Phase 5 DoD). Task snapshots bounded by per-embedder cap (1 base + ≤4 deltas) to prevent doc-12-§11-risk-#5 explosion; older snapshots merge via SVD into the base.</constraints>
    <verification_method>Unit test injects 10 boundaries and asserts `fisher_rank == dim` (full-rank); integration test asserts task_snapshots stays within cap after 100 boundaries (verifying SVD merge fires).</verification_method>
  </requirement>

  <requirement id="REQ-HEAL-03" story_ref="US-HEAL-02" priority="must">
    <description>Expose `pub fn detect_drift(&amp;self) -&gt; DriftSeverity` on `DriftDetector`. The detector maintains a `RingBuffer&lt;DriftSample, 1000&gt;` with each `DriftSample = (predicted_set: Vec&lt;OracleOutcome&gt;, actual_oracle: OracleOutcome, witness_chain_offset: u64)`. Computes `empirical_coverage = count(actual ∈ predicted_set) / window_size`. Maps to severity per Table.</description>
    <rationale>Per doc 09 §5.2 verbatim. The 1,000-attempt window is the canonical drift-detection horizon; smaller windows have too much variance, larger windows lag too much.</rationale>
    <constraints>Severity table: Healthy 88-92 % → continue; Soft 85-88 % → increase exploration + surface to operator; Hard 80-85 % → pause inference + spawn A/B/C retrain; Catastrophic &lt;80 % → refuse predictions + full retrain from corpus. Window must be persisted (CF_MEJEPA_DRIFT_WINDOW) so a restart resumes the same buffer state.</constraints>
    <verification_method>Synthetic drift drill (US-HEAL-02-03) is the canonical verification; deterministic mock-oracle injection forces 70 % coverage; assert detection within 1,000 observations.</verification_method>
  </requirement>

  <requirement id="REQ-HEAL-04" story_ref="US-HEAL-03" priority="must">
    <description>Expose `pub fn retrain_and_promote(&amp;mut self) -&gt; Result&lt;HealReport, HealError&gt;` on `AbcPromoter`. The promoter holds `mode_a: ModelHandle`, `mode_b: ModelHandle`, `mode_c: EnsembleHandle`, `holdout: HoldoutDataset`. Steps: (1) snapshot Mode A; (2) train Mode B from a snapshot of CF_MEJEPA_MUTATION_CORPUS; (3) ensemble Mode C with weights chosen via grid-search on holdout; (4) evaluate all three on holdout; (5) promote winner if all three gates pass; (6) write witness-chain entry on promotion; (7) return HealReport.</description>
    <rationale>Per doc 09 §5.3 verbatim. The three-mode parallel evaluation is the canonical promotion gate; the witness-chain entry makes promotions auditable.</rationale>
    <constraints>Promotion gates (all three required): `coverage ≥ stated_coverage`; `oracle_agreement ≥ A's_score`; `ood_distribution_kl ≤ A's_kl`. If no mode passes all three, return MEJEPA_RETRAIN_HOLDOUT_REGRESSION; do NOT silently keep A active without surfacing. Concurrent promotions return MEJEPA_HEAL_PROMOTION_DEADLOCK. Old weights retained for one promotion-cycle then GC'd by Phase 6 hygiene.</constraints>
    <verification_method>Drift drill (US-HEAL-02-03) end-to-end test asserts mode_winner ∈ {B, C}, never A unmodified; promotion latency &lt; 60 min on RTX 5090.</verification_method>
  </requirement>

  <requirement id="REQ-HEAL-05" story_ref="US-HEAL-04" priority="must">
    <description>Schedule `schedule_full_retrain()` every N=10,000 observations regardless of drift. The body: `snapshot_current_corpus()` (immutable), `train_from_scratch(snapshot)` (ignores online weights), `evaluate_on_holdout()`, `promote_weights(new_θ) iff winner_score &gt; current_score + 0.02`.</description>
    <rationale>Per doc 09 §5.4 verbatim. The +0.02 floor prevents jitter promotions from rounding-error fluctuations.</rationale>
    <constraints>Snapshot retained regardless of promotion outcome (reproducibility). N is configurable via `MeJepaHealConfig.full_retrain_period` but defaults to 10,000.</constraints>
    <verification_method>Stress test runs the full retrain 3 times; asserts (a) snapshots are byte-equal across re-train invocations on the same `corpus_sha`; (b) promotion happens iff +0.02 floor cleared; (c) retrain wallclock is bounded.</verification_method>
  </requirement>

  <requirement id="REQ-HEAL-06" story_ref="US-HEAL-05" priority="must">
    <description>Schedule continuous calibration every 1,000 observations. The body: load the most-recent 1,000 calibration examples (sliding window from CF_MEJEPA_CALIBRATION_HISTORY); recompute τ via `(⌈(N+1)·(1−α)⌉/N)`-quantile of non-conformity scores per SPEC-INFERENCE REQ-INFER-06; recompute σ² so calibration examples score &lt; 0.30; persist new record to CF_MEJEPA_CALIBRATION_HISTORY with provenance.</description>
    <rationale>Per doc 09 §5.5 verbatim. Sliding window adapts the prediction set to recent input distribution.</rationale>
    <constraints>If &lt; 1,000 samples in window: return MEJEPA_CALIBRATION_INSUFFICIENT_SAMPLES; fall back to last-known record AND surface `degraded` status on every prediction during fallback. Provenance metadata (corpus_sha, embedder_versions, frozen_at) per SEC-MEJEPA-04 is mandatory.</constraints>
    <verification_method>Integration test runs 5,000 observations and asserts (a) calibration record updated 5 times (every 1,000); (b) τ and σ² are non-degenerate; (c) FSV evidence captures calibration-version trajectory.</verification_method>
  </requirement>

  <requirement id="REQ-HEAL-07" story_ref="US-HEAL-06" priority="must">
    <description>Schedule witness-chain integrity check every 10,000 observations OR on drift severity transition Hard/Catastrophic. The body: read the durable chain; assert `chain.len() % 73 == 0`; iterate entries computing `prev_hash = shake256_256(prev_entry_bytes)` and asserting `entry.prev_hash == prev_hash`; on break return MEJEPA_OBSERVE_INTEGRITY_VIOLATION and halt inference.</description>
    <rationale>Per doc 09 §5.6 verbatim. Chain integrity is a halt-and-investigate condition, not a soft warning.</rationale>
    <constraints>Reuse the shipped `crates/context-graph-witness/src/witness_chain.rs` machinery (do NOT reinvent). On break: (1) inference paused (`status_change: paused`); (2) operator-surfaced via memory/blockers/ severity:critical; (3) restore from last verified checkpoint (last good promotion's weights_sha + last good calibration version).</constraints>
    <verification_method>Inject one corrupted byte at offset i in a 100,000-entry chain; assert IntegrityViolation { kind: BrokenAt(i) } returned; assert inference paused; FSV evidence at witness-integrity-evidence.json.</verification_method>
  </requirement>

  <requirement id="REQ-HEAL-08" story_ref="US-HEAL-07" priority="must">
    <description>When the last-128-step training-cert window shows ΔΩ.effective_plasticity &lt; 0.4, the REGULATE step calls `refresh_lora_adapter(embedder_id, fresh_corpus_slice, rank=32, epochs=10)` against the shipped `crates/context-graph-embeddings/src/training/lora.rs` infrastructure. Frozen-base embedder weights are NEVER mutated; only the LoRA adapter weights change.</description>
    <rationale>Per doc 12 §6 + §8 verbatim. LoRA refresh is the more elegant alternative to Sutton-2024 dormant-unit re-initialization because the base embedders are frozen by construction.</rationale>
    <constraints>Reuse the shipped LoRA infrastructure (do NOT reinvent). The base embedder weights' SHA-256 must be byte-equal before and after refresh (FSV-asserted). On NaN/OOM: return MEJEPA_HEAL_LORA_REFRESH_FAIL.</constraints>
    <verification_method>FSV harness lora-refresh-evidence.json captures (a) pre-refresh ΔΩ.effective_plasticity, (b) post-refresh ΔΩ.effective_plasticity (must rise above 0.4), (c) base_embedder_sha before/after (must be equal), (d) lora_adapter_sha before/after (must differ).</verification_method>
  </requirement>

  <requirement id="REQ-HEAL-09" story_ref="US-HEAL-08" priority="must">
    <description>Run a distillation-based online observe loop against the shipped `crates/context-graph-embeddings/src/training/distillation.rs`. For each batch of recent observations: pull `oracle_score(panel)` (teacher), pull `embedder.score(panel)` (student), compute `info_nce_loss(student, teacher)`, backprop through student, EMA-update student weights with τ=0.996. The shipped `quality_gate: true` skips low-confidence (low signal_clarity) examples.</description>
    <rationale>Per doc 12 §6 + §8 verbatim. Reuses shipped infrastructure; signal_clarity gating is UTML's `signal_clarity` term baked-in.</rationale>
    <constraints>τ=0.996 is doc-12-mandated (the "future-self prior" of UTML §VIII); configurable via MeJepaHealConfig.distillation_ema_tau. EMA update form: `w ← τ·w + (1−τ)·w_after_grad` (Polyak averaging). Frozen-base embedders are not distilled (only the LoRA adapter / projection head).</constraints>
    <verification_method>Integration test runs distillation on 1,000 observations; asserts (a) EMA update form is correct (test with synthetic τ=0.5 and verify Polyak); (b) low-signal-clarity examples are skipped; (c) student loss decreases over 1,000 steps.</verification_method>
  </requirement>

  <requirement id="REQ-HEAL-10" story_ref="US-HEAL-09" priority="must">
    <description>The whole self-healing loop runs under the UTML v1.0 regulate-state-first cycle: ASSESS ΔΩ + ΔΞ FIRST → REGULATE if either &lt; 0.4 → OBSERVE ΔP + ΔK → COMPUTE L_step = ΔP × ΔK × ΔΩ × ΔΞ (multiplicative, not additive) → CLASSIFY into seven learning states (Stuck / Optimizer-Dysregulated / Latent-Collapsing / Confused / Optimal / Mastery / Dissipating) → INTERVENE substrate-first → EVALUATE on next eval window.</description>
    <rationale>Per doc 11 §9 + doc 10 §D + constitution `<utml_v1_invariants>` rule 3 verbatim. Coaching order: regulate substrate (ΔΩ, ΔΞ) before content (ΔP, ΔK).</rationale>
    <constraints>ΔΞ.target_collapse must be 0 by construction (frozen instruments per constitution rule 1 of utml_v1_invariants). If &gt; 0 detected: return MEJEPA_HEAL_CRITICAL_BUG_TARGET_COLLAPSE_NONZERO and halt cycle. The pruning rule per doc 11 §10: prune curriculum item iff (L_step &lt; 0.1 for N steps) AND (ΔΩ &gt; 0.4 throughout) AND (ΔΞ &gt; 0.4 throughout) — all three required.</constraints>
    <verification_method>Synthetic test cases for each of the 7 learning states; assert the cycle classifies correctly; assert REGULATE fires before INTERVENE-content when either ΔΩ or ΔΞ low; assert pruning never fires when ΔΩ or ΔΞ low.</verification_method>
  </requirement>

  <requirement id="REQ-HEAL-11" story_ref="US-HEAL-02" priority="must">
    <description>Drift events MUST surface (per doc 10 §D, locked): (1) a journal entry to memory/journal/; (2) a memory/blockers/ entry if severity ≥ Hard; (3) a `status_change` field on every `RealityPrediction` made during the heal cycle ("retraining" | "degraded" | "paused"); (4) a recommendation `remaining_risks` mention via the Stop-hook contract per CLAUDE.md §11.6.</description>
    <rationale>Per doc 10 §D ("MUST NOT mean silent fallback") verbatim. Operator visibility into heal events is non-negotiable.</rationale>
    <constraints>SMAP frontmatter on every memory/blockers/ + memory/journal/ entry per CLAUDE.md §6 (namespace, agent_id, timestamp, status, tags). `status_change` field is mandatory on all RealityPrediction records persisted during the heal cycle.</constraints>
    <verification_method>Drift drill asserts journal + blocker + status_change + recommendation are all written before and after the heal cycle.</verification_method>
  </requirement>

  <requirement id="REQ-HEAL-12" story_ref="US-HEAL-03" priority="must">
    <description>Every promotion (Mode A → B / C) MUST be witness-chain-recorded (per doc 10 §D, locked). The entry: `(timestamp, op_type=ModelPromote, weights_sha=theta_sha_winner, mode_winner ∈ {A, B, C}, evaluation_summary_sha=sha256(holdout_eval_json))`. Old weights retained for one promotion-cycle then GC'd by Phase 6 hygiene per SPEC-HYGIENE.</description>
    <rationale>Per doc 09 §5.3 + doc 10 §D ("MUST NOT mean unsupervised weight changes") verbatim. The witness-chain entry is the auditable trail; rollback uses the retained one-cycle-old weights.</rationale>
    <constraints>The promotion entry MUST chain to the prior witness entry's hash; integrity check per REQ-HEAL-07 must replay clean over the new entry. The 32-byte evaluation_summary_sha pins the holdout JSON record so it can be recomputed.</constraints>
    <verification_method>Drift drill asserts (a) one new entry appended; (b) chain replays clean after promotion; (c) the holdout-eval JSON at the SHA matches the entry; (d) old weights still present in CF_MEJEPA_WEIGHT_BLOBS for one cycle.</verification_method>
  </requirement>

  <requirement id="REQ-HEAL-13" story_ref="US-HEAL-08" priority="must">
    <description>`signal_clarity` from ΔE multiplicatively gates the observe() update (per doc 10 §D, locked). Examples with low signal_clarity (default threshold 0.5) contribute proportionally less weight to the SGD step or are skipped entirely (depending on `MeJepaHealConfig.skip_below_signal_clarity` boolean).</description>
    <rationale>Per doc 10 §D ("MUST NOT mean accepting noisy labels as if they were gold") verbatim. The shipped `quality_gate: true` flag in distillation.rs implements this in the embedder pipeline; the predictor must do the same.</rationale>
    <constraints>signal_clarity field is required on every shift-subscriber-supplied observation; the subscriber computes it from ΔE per UTML §VIII. No `unwrap_or(0.5)` fallback (constitution `<anti_patterns>` rule).</constraints>
    <verification_method>Unit test: feed two observations identical except for signal_clarity (0.9 vs 0.2); assert weight delta on the second is proportionally smaller (or zero if skip_below threshold).</verification_method>
  </requirement>

  <requirement id="REQ-HEAL-14" story_ref="US-HEAL-01" priority="must">
    <description>The `ObserveError` taxonomy includes (per constitution `<failure_taxonomy>` extended for Phase 5): `BatchNan { component, witness_chain_offset }`, `DriftDetected { severity, empirical_coverage }`, `IntegrityViolation { kind: WrongSize | BrokenAt(usize) }`, `HoldoutRegression { mode_a_score, mode_b_score, mode_c_score }`, `QuotaExceeded { used_gb, quota_gb }`, `FisherRankDeficient { rank, dim }`, `LoraRefreshFail { embedder_id, cause }`, `PromotionDeadlock { holder }`, `ConformalInsufficientSamples { observed, required }`, `CriticalBug { kind: TargetCollapseNonZero | StabilityFloorZero }`, `FeatureDisabled`. Each carries a structured payload and `.code()` returns the SCREAMING_SNAKE tag.</description>
    <rationale>Per constitution `<error_handling>` "robust error logging" + `<failure_taxonomy>` verbatim plus the new Phase 5 codes (BatchNan, DriftDetected, IntegrityViolation, HoldoutRegression, QuotaExceeded, FisherRankDeficient, LoraRefreshFail, PromotionDeadlock).</rationale>
    <constraints>Every variant MUST log input shapes + file paths + cert offsets before returning per constitution §error_handling. No silent fallback. No `unwrap_or(0.5)` on confidence intervals.</constraints>
    <verification_method>Unit test for each variant asserts `.code()` returns expected SCREAMING_SNAKE tag; FSV harness writes one example of each error case.</verification_method>
  </requirement>

  <requirement id="REQ-HEAL-15" story_ref="US-HEAL-10" priority="must">
    <description>Phase 5 DoD verification command: `cargo run --release -p context-graph-mejepa -- heal-drill --inject-drift hard` reports `drift_detected: hard, after ~700 observations`, `retrain_triggered: yes`, `mode_winner: B` or `C`, `promotion_latency &lt; 60 min` on RTX 5090. FSV evidence at /tmp/contextgraph-mejepa-self-healing-fsv/{drift-drill-evidence.json, abc-promotion-evidence.json, ewc-fisher-evidence.json, lora-refresh-evidence.json, witness-integrity-evidence.json}, all mode 0o600 per SEC-MEJEPA-07.</description>
    <rationale>Per doc 09 §7 Phase 5 DoD verbatim.</rationale>
    <verification_method>The command IS the verification; FSV evidence captures the output.</verification_method>
  </requirement>

  <requirement id="REQ-HEAL-16" story_ref="US-HEAL-09" priority="must">
    <description>Tests use real data only (real Python source, real Docker oracle output, real witness-chain bytes) per CLAUDE.md §11.9 + constitution `<anti_patterns>` rule on DataIntegrity. No mock embedders, no mock oracle outputs, no mock chain bytes that pass while project is broken. The drift drill's "synthetic drift" is a deterministic mock-oracle injection at the verification boundary, not a mock embedder or mock model.</description>
    <rationale>Per CLAUDE.md §11.9 + constitution verbatim. Mock data corrupts the FSV evidence and hides bugs.</rationale>
    <verification_method>Code review checklist item: every test that asserts a heal event runs against a real predictor + real corpus + real chain.</verification_method>
  </requirement>

  <requirement id="REQ-HEAL-17" story_ref="US-HEAL-02" priority="should">
    <description>MCP tool `mcp__cgreality__mejepa_heal_status() -&gt; HealStatus { drift_severity, last_promotion_at, last_calibration_at, last_integrity_check_at, observation_counter, ewc_lambda, fisher_rank }` exposes the heal cycle's current state for operator inspection. Read-only; no parameters; no side effects.</description>
    <rationale>Per doc 10 §F priority #7 plus operator visibility requirement (UT-03 goal).</rationale>
    <constraints>Tool name follows `mcp_tool_namespace_lock` per constitution. JSON schema enforces `additionalProperties: false`.</constraints>
    <verification_method>Integration test invokes the MCP tool; asserts schema compliance + round-trip equivalence with direct in-process call.</verification_method>
  </requirement>

  <requirement id="REQ-HEAL-18" story_ref="US-HEAL-03" priority="should">
    <description>MCP tool `mcp__cgreality__mejepa_rollback_to(target_witness_chain_offset: u64)` allows operator-issued rollback to a prior promotion's weights_sha. The tool: (1) verifies the target offset references a `op_type=ModelPromote` entry; (2) loads the corresponding `weights_sha` from CF_MEJEPA_WEIGHT_BLOBS (must still be retained, i.e. within one promotion-cycle); (3) appends a new `op_type=ModelRollback` witness entry; (4) updates the active-weight pointer.</description>
    <rationale>Operator must have a one-call rollback path; per doc 10 §D the witness-chain trail makes rollback trivially auditable.</rationale>
    <constraints>If target weights_sha already GC'd (older than one promotion-cycle), return MEJEPA_HEAL_ROLLBACK_TARGET_GONE; do not silently fall back. Operator authentication via MCP boundary (no auth check inside the tool — outer trust model).</constraints>
    <verification_method>Integration test promotes B then rolls back to A's offset; asserts active-weight pointer reverted; asserts new ModelRollback entry chains correctly.</verification_method>
  </requirement>

  <requirement id="REQ-HEAL-19" story_ref="US-HEAL-04" priority="must">
    <description>Performance budgets per constitution `<performance_budgets>`: per-shift `observe()` p50 &lt; 5 s, p99 &lt; 30 s (cross-cutting with SPEC-SHIFT-SUBSCRIBER); A/B/C promotion latency &lt; 60 min on RTX 5090 (per Phase 5 DoD); full retrain wallclock &lt; 12 hours (per `phase3_training_walltime` constitution budget); RocksDB total &lt;= 115 GB (per `rocksdb_disk_total_quota`).</description>
    <rationale>Per Phase 5 DoD (doc 09 §7) + constitution budgets verbatim.</rationale>
    <verification_method>Drift drill measures and reports each metric; FSV evidence captures the timings.</verification_method>
  </requirement>

  <requirement id="REQ-HEAL-20" story_ref="US-HEAL-01" priority="must">
    <description>Cross-cutting integration with SPEC-SHIFT-SUBSCRIBER: the subscriber is the SOLE external caller of `observe()` in normal operation (operator-issued heal-drill is the test-time-only second caller). The subscriber respects `MEJEPA_OBSERVE_DRIFT_DETECTED` and pauses inflight observations until the heal cycle completes. The watermark CF_MEJEPA_SHIFT_WATERMARK is updated AFTER each successful observe() so a crash cannot re-process a shift.</description>
    <rationale>Per doc 09 §3.8 + cross-cutting with SPEC-SHIFT-SUBSCRIBER. The subscriber's contract with this spec is well-defined: it owns the watermark, this spec owns the observe() body.</rationale>
    <constraints>Watermark update is atomic via `write_text_checked` (shipped `witness_chain_io.rs`); a crash mid-observe() does not skip the shift. On error returned from observe(), the watermark is NOT advanced (the subscriber retries on resume).</constraints>
    <verification_method>Integration test injects a crash mid-observe(); asserts on resume the same shift is re-processed exactly once; FSV evidence captures the watermark trajectory.</verification_method>
  </requirement>

</requirements>

<business_rules>

  <rule id="BR-HEAL-01" req_ref="REQ-HEAL-01">
    <condition>Any caller invokes `observe()` from outside the workspace (i.e., over MCP)</condition>
    <action>The call MUST go through an internal MCP shim with operator authentication; direct in-process FFI is reserved for the workspace's shift-subscriber and the heal-drill CLI. By default no MCP tool exposes observe() (Phase 7 may expose `mejepa_observe` for testing only).</action>
    <exception>The `mejepa-heal-drill` CLI is allowed to call observe() directly with synthetic inputs for verification.</exception>
  </rule>

  <rule id="BR-HEAL-02" req_ref="REQ-HEAL-03">
    <condition>Drift severity is Soft</condition>
    <action>Increase exploration (e.g., widen the conformal interval temporarily) AND surface to operator via journal entry; do NOT pause inference; do NOT spawn A/B/C retrain.</action>
    <exception>None. Soft drift is informational; only Hard or Catastrophic triggers the retrain pipeline.</exception>
  </rule>

  <rule id="BR-HEAL-03" req_ref="REQ-HEAL-10">
    <condition>ΔΩ &lt; 0.4 OR ΔΞ &lt; 0.4</condition>
    <action>REGULATE substrate FIRST; do NOT advance to OBSERVE ΔP / ΔK or INTERVENE on content until substrate health restored; per UTML §IX coaching order.</action>
    <exception>None. The coaching order is locked by constitution `<utml_v1_invariants>` rule 3.</exception>
  </rule>

  <rule id="BR-HEAL-04" req_ref="REQ-HEAL-12">
    <condition>Any weight promotion (Mode A → B / C / rollback)</condition>
    <action>MUST write a witness-chain entry with `op_type ∈ {ModelPromote, ModelRollback}` + `weights_sha` + `evaluation_summary_sha`; integrity check must replay clean.</action>
    <exception>None. Per doc 10 §D ("MUST NOT mean unsupervised weight changes") locked.</exception>
  </rule>

  <rule id="BR-HEAL-05" req_ref="REQ-HEAL-13">
    <condition>signal_clarity &lt; configured threshold (default 0.5) on a shift-subscriber-supplied observation</condition>
    <action>Multiplicatively gate the SGD step proportional to signal_clarity (or skip entirely if `skip_below_signal_clarity=true`); never silently treat low-clarity as gold.</action>
    <exception>None. Per doc 10 §D ("MUST NOT mean accepting noisy labels as if they were gold") locked.</exception>
  </rule>

  <rule id="BR-HEAL-06" req_ref="REQ-HEAL-08">
    <condition>LoRA refresh runs because ΔΩ.effective_plasticity &lt; 0.4</condition>
    <action>Mutate ONLY the LoRA adapter weights; the frozen-base embedder weights MUST be byte-equal before and after (FSV-asserted).</action>
    <exception>None. Frozen-base embedders are locked by construction per constitution rule 1 of utml_v1_invariants.</exception>
  </rule>

  <rule id="BR-HEAL-07" req_ref="REQ-HEAL-04">
    <condition>No mode passes all three promotion gates (coverage / oracle_agreement / OOD distribution)</condition>
    <action>Return MEJEPA_RETRAIN_HOLDOUT_REGRESSION; do NOT silently keep A active without surfacing; the operator decides whether to expand the corpus or accept a degraded threshold via override.</action>
    <exception>None.</exception>
  </rule>

</business_rules>

<edge_cases>

  <edge_case id="EC-HEAL-01" req_ref="REQ-HEAL-01">
    <scenario>The first ever observe() call (cold start) — no prior task snapshots exist, Fisher matrix is uninitialized.</scenario>
    <expected_behavior>EwcPlusPlus initializes Fisher matrix to zero; takes the first SGD step with zero EWC penalty; immediately snapshots task_0 after the step; subsequent steps use the snapshot. No error.</expected_behavior>
    <priority>high</priority>
  </edge_case>

  <edge_case id="EC-HEAL-02" req_ref="REQ-HEAL-02">
    <scenario>EwcPlusPlus accumulates 100 task snapshots without SVD merge; memory pressure on Fisher matrix storage.</scenario>
    <expected_behavior>After cap of (1 base + ≤4 deltas per embedder) reached, SVD merge fires automatically; oldest deltas absorbed into base; total snapshot count stays bounded; per doc 12 §11 risk #5.</expected_behavior>
    <priority>high</priority>
  </edge_case>

  <edge_case id="EC-HEAL-03" req_ref="REQ-HEAL-03">
    <scenario>The rolling drift window contains exactly 999 samples (one short of full).</scenario>
    <expected_behavior>Drift detection refuses to compute severity (returns DriftSeverity::WarmupNotReady); inference continues normally; scheduler waits for the 1,000th sample.</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-HEAL-04" req_ref="REQ-HEAL-03">
    <scenario>Drift severity oscillates between Healthy and Soft on adjacent windows (boundary jitter).</scenario>
    <expected_behavior>A 3-window hysteresis prevents flapping: severity transition only fires when the new severity holds for 3 consecutive non-overlapping windows OR a single window is &gt;1 severity-band away from the prior (e.g., Healthy → Hard skips hysteresis).</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-HEAL-05" req_ref="REQ-HEAL-04">
    <scenario>Mode B training crashes mid-retrain (NaN gradient on hour 3 of a 4-hour retrain).</scenario>
    <expected_behavior>Return HealError::BatchNan; release the promotion lock; Mode A remains active; SMAP blocker severity:high; do NOT retry automatically (operator decides).</expected_behavior>
    <priority>high</priority>
  </edge_case>

  <edge_case id="EC-HEAL-06" req_ref="REQ-HEAL-04">
    <scenario>Mode C ensemble grid-search returns w_A=1.0 (Mode A best alone in ensemble).</scenario>
    <expected_behavior>Mode C is effectively Mode A; promotion gate sees identical scores; returns HoldoutRegression with `mode_winner: "A_unchanged"` (informational, not error); Mode A remains active.</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-HEAL-07" req_ref="REQ-HEAL-06">
    <scenario>Continuous calibration runs but new τ is identical to old τ (sliding window unchanged in distribution).</scenario>
    <expected_behavior>New record persisted with same τ but new `frozen_at` timestamp + `corpus_sha` (the corpus may have grown); cache key invalidation triggers downstream refresh; not an error.</expected_behavior>
    <priority>low</priority>
  </edge_case>

  <edge_case id="EC-HEAL-08" req_ref="REQ-HEAL-07">
    <scenario>Witness chain integrity check finds break at offset 0 (the very first entry has non-zero prev_hash).</scenario>
    <expected_behavior>This is the recurring `CCREALITY_WITNESS_PREV_HASH_MISMATCH` issue documented in doc 09 §0; per `witness_chain_legacy.rs` reconciliation, attempt legacy-format detection FIRST; if legacy format confirmed, repair and continue; if not legacy, return IntegrityViolation { kind: BrokenAt(0) }.</expected_behavior>
    <priority>critical</priority>
  </edge_case>

  <edge_case id="EC-HEAL-09" req_ref="REQ-HEAL-08">
    <scenario>Multiple embedders simultaneously have ΔΩ.effective_plasticity &lt; 0.4 (broad plasticity collapse).</scenario>
    <expected_behavior>Refresh LoRA adapters serially in priority order (highest ΔK contribution first); cap concurrent refreshes at 1 (VRAM constraint); FSV evidence captures the refresh order.</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-HEAL-10" req_ref="REQ-HEAL-09">
    <scenario>Distillation runs but oracle_score is missing (no Docker run for this observation; only static-analysis signal).</scenario>
    <expected_behavior>signal_clarity = 0 (no ground-truth teacher); skip the example per `quality_gate: true`; do not fall back to a pseudo-teacher.</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-HEAL-11" req_ref="REQ-HEAL-10">
    <scenario>Two REGULATE actions are needed simultaneously (effective_plasticity collapsed AND constellation_violation_rate high).</scenario>
    <expected_behavior>Run them in priority order: stability_floor first (halt+rollback if zero); then effective_plasticity (LoRA refresh); then constellation_violation (re-fit centroids); then predictor_redundancy (VICReg coefficient bump); then landscape_health (SAM tightening). Serial; no concurrent regulations.</expected_behavior>
    <priority>high</priority>
  </edge_case>

  <edge_case id="EC-HEAL-12" req_ref="REQ-HEAL-11">
    <scenario>Drift detected mid-promotion (the A/B/C retrain itself triggers a new drift signal).</scenario>
    <expected_behavior>The new drift event is journal-recorded but does NOT spawn a second A/B/C retrain (lock held); resolved on completion of the in-flight retrain; if the new drift remains after promotion, a fresh retrain is allowed.</expected_behavior>
    <priority>high</priority>
  </edge_case>

  <edge_case id="EC-HEAL-13" req_ref="REQ-HEAL-12">
    <scenario>Promotion writes the witness-chain entry but crashes before updating the active-weight pointer (atomicity boundary).</scenario>
    <expected_behavior>On restart, the witness-chain replay shows ModelPromote at offset N but no weight pointer update. The recovery routine: read the latest ModelPromote entry; verify its weights_sha is in CF_MEJEPA_WEIGHT_BLOBS; load and activate; emit a recovery journal entry. If weights_sha is missing, fall back to the prior ModelPromote.</expected_behavior>
    <priority>critical</priority>
  </edge_case>

  <edge_case id="EC-HEAL-14" req_ref="REQ-HEAL-18">
    <scenario>Operator issues `mejepa_rollback_to` for an offset whose weights_sha was already GC'd by Phase 6 hygiene (older than one promotion-cycle).</scenario>
    <expected_behavior>Return MEJEPA_HEAL_ROLLBACK_TARGET_GONE with `gced_at` timestamp; surface to operator who must restore from backup or accept the limitation.</expected_behavior>
    <priority>high</priority>
  </edge_case>

  <edge_case id="EC-HEAL-15" req_ref="REQ-HEAL-20">
    <scenario>Subscriber resumes from a stale watermark (e.g., after a long pause) and 10,000 unprocessed shifts queue up.</scenario>
    <expected_behavior>Subscriber processes them in order; observe() runs in batched mode (per-batch Fisher update is averaged); EWC++ regularization holds; no special-case handling beyond the standard observe() path.</expected_behavior>
    <priority>medium</priority>
  </edge_case>

</edge_cases>

<error_states>

  <error id="ERR-HEAL-01" code="MEJEPA_OBSERVE_BATCH_NAN" req_ref="REQ-HEAL-01">
    <condition>Gradient or activation went non-finite during observe() (NaN or Inf detected).</condition>
    <user_message>"observe() failed: non-finite gradient at component {component} (witness_chain_offset {offset}). Predictor weights NOT mutated. See blocker for details."</user_message>
    <internal_message>Logged with: panel_sha, oracle_outcome digest, predictor checkpoint sha, current λ_ewc, fisher_rank, component name, witness_chain_offset.</internal_message>
    <recovery_action>Rollback to prior checkpoint (transactional — weights are not mutated on NaN); surface SMAP blocker severity:high; operator decides whether to investigate corpus quality or skip the offending observation via `mejepa_skip_observation`.</recovery_action>
  </error>

  <error id="ERR-HEAL-02" code="MEJEPA_OBSERVE_DRIFT_DETECTED" req_ref="REQ-HEAL-03">
    <condition>Empirical conformal coverage on the rolling 1,000-attempt window dropped below 0.85 (Hard) or 0.80 (Catastrophic).</condition>
    <user_message>"Drift detected: severity {severity}, empirical_coverage {coverage}. Spawning A/B/C retrain."</user_message>
    <internal_message>Logged with: window_start_offset, window_end_offset, predicted_set distribution, actual_oracle distribution, severity transition history.</internal_message>
    <recovery_action>Spawn A/B/C retrain (Hard) or trigger full corpus retrain (Catastrophic); pause inflight observe() calls; surface SMAP blocker severity:high (Hard) or severity:critical (Catastrophic).</recovery_action>
  </error>

  <error id="ERR-HEAL-03" code="MEJEPA_OBSERVE_INTEGRITY_VIOLATION" req_ref="REQ-HEAL-07">
    <condition>Witness chain replay broken: chain.len() % 73 != 0 OR prev_hash chain broken at offset i.</condition>
    <user_message>"Witness chain integrity violation at offset {i}: {kind}. Inference paused. Restoring from last verified checkpoint."</user_message>
    <internal_message>Logged with: chain length, breakpoint offset i, expected prev_hash, actual prev_hash, last-good promotion entry offset.</internal_message>
    <recovery_action>Halt inference (status_change: paused); restore weights to last good promotion's weights_sha; restore calibration to last good calibration version; surface SMAP blocker severity:critical.</recovery_action>
  </error>

  <error id="ERR-HEAL-04" code="MEJEPA_RETRAIN_HOLDOUT_REGRESSION" req_ref="REQ-HEAL-04">
    <condition>No mode (A, B, or C) passes all three promotion gates (coverage ≥ stated, oracle_agreement ≥ A's, OOD KL ≤ A's).</condition>
    <user_message>"A/B/C retrain regressed on holdout: A={a_score}, B={b_score}, C={c_score}. Mode A retained. Operator review required."</user_message>
    <internal_message>Logged with: per-mode coverage, oracle_agreement, OOD distribution KL, holdout snapshot SHA.</internal_message>
    <recovery_action>Mode A remains active; surface SMAP blocker severity:medium; operator decides whether to expand corpus or override threshold.</recovery_action>
  </error>

  <error id="ERR-HEAL-05" code="MEJEPA_RETRAIN_QUOTA_EXCEEDED" req_ref="REQ-HEAL-04">
    <condition>RocksDB disk usage exceeded 90 % of `rocksdb_disk_total_quota` (default 115 GB) during retrain.</condition>
    <user_message>"Retrain paused: disk usage {used_gb}/{quota_gb} GB. Triggering Phase 6 hygiene to evict cold-tier corpus."</user_message>
    <internal_message>Logged with: per-CF size breakdown, hottest evictable CFs, eviction plan.</internal_message>
    <recovery_action>Trigger Phase 6 hygiene (cold-tier eviction); on success resume retrain; on failure surface blocker severity:high.</recovery_action>
  </error>

  <error id="ERR-HEAL-06" code="MEJEPA_HEAL_FISHER_RANK_DEFICIENT" req_ref="REQ-HEAL-02">
    <condition>EWC++ Fisher matrix rank &lt; full after a task snapshot (a bug per Phase 5 DoD).</condition>
    <user_message>"Fisher matrix rank-deficient: rank={rank} dim={dim}. Halting EWC++ updates. See blocker."</user_message>
    <internal_message>Logged with: snapshot sequence, dim, rank computation, eigenvalue spectrum.</internal_message>
    <recovery_action>Halt EWC++ updates; surface blocker severity:critical labelled "Fisher accumulation bug — likely zero-gradient subspace"; operator investigates.</recovery_action>
  </error>

  <error id="ERR-HEAL-07" code="MEJEPA_HEAL_LORA_REFRESH_FAIL" req_ref="REQ-HEAL-08">
    <condition>LoRA adapter refresh failed (NaN gradient during refresh, OOM, or post-refresh ΔΩ.effective_plasticity did not rise above 0.4).</condition>
    <user_message>"LoRA refresh failed for embedder {embedder_id}: {cause}. Plasticity collapse persists."</user_message>
    <internal_message>Logged with: pre-refresh ΔΩ.effective_plasticity, fresh_corpus_slice SHA, epochs completed, NaN component (if applicable), VRAM usage.</internal_message>
    <recovery_action>Surface blocker severity:high; operator may attempt manual continual-backprop dormant-unit re-init via `mejepa_dormant_reinit` (separate codepath).</recovery_action>
  </error>

  <error id="ERR-HEAL-08" code="MEJEPA_HEAL_PROMOTION_DEADLOCK" req_ref="REQ-HEAL-04">
    <condition>Two promotions attempt to acquire the promotion lock concurrently.</condition>
    <user_message>"Promotion deadlock: another promotion holds the lock (holder {holder}). Retry after current promotion completes."</user_message>
    <internal_message>Logged with: holder PID/thread_id, lock acquisition timestamp, current promotion mode_winner.</internal_message>
    <recovery_action>The second promotion is rejected; the first completes normally; the second can be retried by the operator after lock release.</recovery_action>
  </error>

  <error id="ERR-HEAL-09" code="MEJEPA_CALIBRATION_INSUFFICIENT_SAMPLES" req_ref="REQ-HEAL-06">
    <condition>Fewer than 1,000 samples in the conformal calibration sliding window (early in deployment).</condition>
    <user_message>"Continuous calibration deferred: {observed}/1000 samples. Falling back to last-known calibration. Predictions during this period carry status_change=degraded."</user_message>
    <internal_message>Logged with: observed count, last-known calibration version + frozen_at.</internal_message>
    <recovery_action>Fall back to last-known calibration record; surface `degraded` status on every prediction during fallback per doc 10 §D.</recovery_action>
  </error>

  <error id="ERR-HEAL-10" code="MEJEPA_HEAL_CRITICAL_BUG_TARGET_COLLAPSE_NONZERO" req_ref="REQ-HEAL-10">
    <condition>ΔΞ.target_collapse &gt; 0 detected (a critical bug — instruments must be frozen by construction).</condition>
    <user_message>"CRITICAL BUG: ΔΞ.target_collapse = {value}. An instrument has accidentally become trainable. Halting heal cycle. Fix at trait level."</user_message>
    <internal_message>Logged with: per-instrument target_collapse contribution, last code_version, training cert offset.</internal_message>
    <recovery_action>Halt heal cycle; surface blocker severity:critical; operator must fix the trait-level bug (likely an instrument's `encode()` no longer being a pure function).</recovery_action>
  </error>

</error_states>

<data_requirements>

  <entity name="ObserveOutput">
    <field name="theta_sha_after" type="[u8; 32]" required="true">
      <description>SHA-256 of predictor weights AFTER the observe() step</description>
      <constraints>Computed deterministically over the canonical weight serialization.</constraints>
    </field>
    <field name="l_step" type="f32" required="true">
      <description>UTML L_step = ΔP × ΔK × ΔΩ × ΔΞ for this observation</description>
      <constraints>finite; ∈ [0, 1]; multiplicative gating per UTML invariant rule 1.</constraints>
    </field>
    <field name="delta_p" type="f32" required="true"><description>UTML ΔP factor</description><constraints>finite; ∈ [0, 1]</constraints></field>
    <field name="delta_k" type="f32" required="true"><description>UTML ΔK factor</description><constraints>finite; ∈ [0, 1]</constraints></field>
    <field name="delta_omega" type="f32" required="true"><description>UTML ΔΩ factor</description><constraints>finite; ∈ [0, 1]</constraints></field>
    <field name="delta_xi" type="f32" required="true"><description>UTML ΔΞ factor</description><constraints>finite; ∈ [0, 1]; target_collapse component MUST be 0 by construction</constraints></field>
    <field name="fisher_rank" type="usize" required="true"><description>Rank of EWC++ Fisher matrix after this update</description><constraints>≤ predictor parameter count; full rank is required after 10 task boundaries</constraints></field>
  </entity>

  <entity name="DriftSeverity">
    <field name="variant" type="enum" required="true">
      <description>Healthy | Soft | Hard | Catastrophic | WarmupNotReady</description>
      <constraints>Mapped from empirical_coverage per the severity table.</constraints>
    </field>
    <field name="empirical_coverage" type="f32" required="true"><description>count(actual ∈ predicted_set) / window_size</description><constraints>∈ [0, 1]</constraints></field>
    <field name="window_start_offset" type="u64" required="true"><description>Witness-chain offset where the rolling window starts</description></field>
    <field name="window_end_offset" type="u64" required="true"><description>Witness-chain offset where the rolling window ends</description></field>
  </entity>

  <entity name="HealReport">
    <field name="mode_winner" type="enum" required="true"><description>A | B | C | A_unchanged_no_floor_clearance | A_unchanged_no_winner</description></field>
    <field name="mode_a_score" type="f32" required="true"><description>Holdout score of Mode A</description></field>
    <field name="mode_b_score" type="f32" required="true"><description>Holdout score of Mode B</description></field>
    <field name="mode_c_score" type="f32" required="true"><description>Holdout score of Mode C</description></field>
    <field name="mode_c_weights" type="(f32, f32)" required="true"><description>(w_A, w_B) ensemble weights for Mode C</description></field>
    <field name="weights_sha_winner" type="[u8; 32]" required="true"></field>
    <field name="evaluation_summary_sha" type="[u8; 32]" required="true"><description>SHA-256 of the holdout-eval JSON record</description></field>
    <field name="witness_chain_offset" type="u64" required="true"><description>Offset of the ModelPromote entry</description></field>
    <field name="promotion_latency_seconds" type="u64" required="true"><description>Wallclock from drift detection to promotion</description></field>
    <field name="status_change" type="String" required="true"><description>"retraining" during the cycle, "active" after</description></field>
  </entity>

  <entity name="HealStatus">
    <field name="drift_severity" type="DriftSeverity" required="true"></field>
    <field name="last_promotion_at" type="i64" required="true"><description>Unix timestamp seconds; -1 if never</description></field>
    <field name="last_calibration_at" type="i64" required="true"><description>Unix timestamp seconds; -1 if never</description></field>
    <field name="last_integrity_check_at" type="i64" required="true"></field>
    <field name="observation_counter" type="u64" required="true"><description>Total observations consumed by observe() since deployment</description></field>
    <field name="ewc_lambda" type="f32" required="true"><description>Current λ_ewc value</description><constraints>∈ [initial_lambda, max_lambda]</constraints></field>
    <field name="fisher_rank" type="usize" required="true"><description>Current Fisher matrix rank</description></field>
    <field name="active_weights_sha" type="[u8; 32]" required="true"></field>
    <field name="active_calibration_version" type="String" required="true"></field>
    <field name="active_constellation_version" type="String" required="true"></field>
  </entity>

  <entity name="MeJepaHealConfig">
    <field name="initial_lambda" type="f32" required="true"><description>EWC++ starting regularization weight</description><constraints>default 2000; ∈ [1, 100000]</constraints></field>
    <field name="max_lambda" type="f32" required="true"><description>EWC++ upper cap</description><constraints>default 15000; ≥ initial_lambda</constraints></field>
    <field name="drift_window_size" type="usize" required="true"><description>Rolling window size for drift detection</description><constraints>default 1000</constraints></field>
    <field name="full_retrain_period" type="u64" required="true"><description>Observations between forced full retrains</description><constraints>default 10000</constraints></field>
    <field name="continuous_calibration_period" type="u64" required="true"><description>Observations between calibration recomputes</description><constraints>default 1000</constraints></field>
    <field name="integrity_check_period" type="u64" required="true"><description>Observations between chain integrity checks</description><constraints>default 10000</constraints></field>
    <field name="full_retrain_promotion_floor" type="f32" required="true"><description>Min score improvement to promote on full retrain</description><constraints>default 0.02</constraints></field>
    <field name="lora_refresh_rank" type="usize" required="true"><description>Rank for LoRA refresh adapters</description><constraints>default 32</constraints></field>
    <field name="lora_refresh_epochs" type="u32" required="true"><description>Epochs for LoRA refresh</description><constraints>default 10</constraints></field>
    <field name="distillation_ema_tau" type="f32" required="true"><description>EMA coupling rate for distillation</description><constraints>default 0.996; ∈ (0, 1)</constraints></field>
    <field name="signal_clarity_threshold" type="f32" required="true"><description>Below this, observation is gated or skipped</description><constraints>default 0.5; ∈ [0, 1]</constraints></field>
    <field name="skip_below_signal_clarity" type="bool" required="true"><description>If true, skip; else gate proportionally</description><constraints>default true</constraints></field>
    <field name="quota_warn_pct" type="f32" required="true"><description>RocksDB usage triggering quota warning</description><constraints>default 0.90; ∈ (0, 1)</constraints></field>
  </entity>

</data_requirements>

<non_functional_requirements>

  <nfr id="NFR-HEAL-01" category="performance">
    <description>Per-shift observe() latency p50 &lt; 5 s; p99 &lt; 30 s on RTX 5090 (cross-cutting with SPEC-SHIFT-SUBSCRIBER).</description>
    <metric>FSV harness measures p50/p95/p99 over 1,000 sequential observations.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-HEAL-02" category="performance">
    <description>A/B/C promotion latency &lt; 60 min on RTX 5090 (per Phase 5 DoD).</description>
    <metric>Drift drill measures wallclock from drift detection to ModelPromote witness entry.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-HEAL-03" category="reliability">
    <description>EWC++ Fisher matrix non-degeneracy: full rank after 10 task boundaries (rank-deficient is a bug per Phase 5 DoD).</description>
    <metric>FSV asserts `fisher_rank == predictor_dim` after 10 boundaries.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-HEAL-04" category="reliability">
    <description>Drift detection sensitivity: synthetic 70 % coverage detected within 1,000 observations (per Phase 5 DoD; ±300 tolerance for the rolling-window lag).</description>
    <metric>Drift drill reports detection latency in observations.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-HEAL-05" category="reliability">
    <description>Atomic durability: a crash mid-observe() never leaves half-applied EWC++ Fisher updates (reuse `write_text_checked` from witness_chain_io.rs per doc 09 §10 item 20).</description>
    <metric>Crash-injection test asserts on-restart consistency.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-HEAL-06" category="security">
    <description>FSV evidence files under /tmp/contextgraph-mejepa-self-healing-fsv/ created with mode 0o600 per SEC-MEJEPA-07.</description>
    <metric>Test asserts file mode after creation.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-HEAL-07" category="security">
    <description>Promotion entries signed with ed25519 keypair stored in operator-controlled keyring per SEC-MEJEPA-03.</description>
    <metric>Witness-chain entry verification asserts signature validity.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-HEAL-08" category="reliability">
    <description>Determinism: given identical (corpus_snapshot_sha, code_version, frozen_seeds), full retrain produces byte-equal weights_sha across runs.</description>
    <metric>Stress test runs full retrain twice on the same snapshot; asserts byte-equal output.</metric>
    <priority>should</priority>
  </nfr>

  <nfr id="NFR-HEAL-09" category="performance">
    <description>VRAM budget during heal cycle: ≤ 29 GB on RTX 5090 (per `vram_steady_state` constitution budget); LoRA refresh + Mode B retrain serialize when budget tight.</description>
    <metric>Heal cycle reports peak VRAM via nvidia-smi sampling.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-HEAL-10" category="reliability">
    <description>No silent fallback: every fallible op returns a taxonomy-defined error with `.code()`. No `unwrap_or(0.5)` on confidence intervals. No mock data in tests.</description>
    <metric>Code-review checklist + grep audit for `unwrap_or(0.5)` and mock embedders/oracles.</metric>
    <priority>must</priority>
  </nfr>

</non_functional_requirements>

<dependencies>

  <dependency type="internal">
    <name>context-graph-witness</name>
    <description>SHAKE-256 73-byte witness chain; chain integrity replay; ed25519 signing; atomic write_text_checked. Already shipped.</description>
    <impact>Without it, REQ-HEAL-07 (integrity check) and REQ-HEAL-12 (promotion entry) cannot be satisfied. Hard dependency.</impact>
  </dependency>

  <dependency type="internal">
    <name>context-graph-embeddings (LoRA + distillation)</name>
    <description>Shipped LoRA infrastructure (rank/alpha/dropout on Q/K/V) and distillation infrastructure (InfoNCE + EMA + quality_gate). Reused for REQ-HEAL-08 + REQ-HEAL-09.</description>
    <impact>Without it, LoRA refresh and distillation-based observe must reinvent — not allowed per CLAUDE.md §11.4 (use shipped infrastructure). Hard dependency.</impact>
  </dependency>

  <dependency type="internal">
    <name>context-graph-mejepa (Phase 2 predictor)</name>
    <description>The predictor θ that observe() updates. Shipped (deferred). Required for the SGD step in REQ-HEAL-01.</description>
    <impact>Without it, observe() has nothing to update. Hard dependency on Phase 2 ship.</impact>
  </dependency>

  <dependency type="internal">
    <name>context-graph-mejepa-train (Phase 3 training loop)</name>
    <description>Provides train_from_scratch for Mode B retrain (REQ-HEAL-04 + REQ-HEAL-05).</description>
    <impact>Without it, A/B/C and full retrain cannot run. Hard dependency on Phase 3 ship.</impact>
  </dependency>

  <dependency type="internal">
    <name>context-graph-mejepa-corpus</name>
    <description>Holdout split + calibration split + mutation corpus snapshots. Shipped (partial).</description>
    <impact>Without it, holdout evaluation cannot run. Hard dependency.</impact>
  </dependency>

  <dependency type="internal">
    <name>context-graph-mejepa-shift-subscriber (Phase 7)</name>
    <description>Sole external caller of observe() in normal operation. Cross-cutting with SPEC-SHIFT-SUBSCRIBER.</description>
    <impact>Without it, observe() is never called outside the heal-drill CLI. Hard dependency on Phase 7 ship for production use.</impact>
  </dependency>

  <dependency type="internal">
    <name>context-graph-mejepa-tct (Phase 4b)</name>
    <description>Constellation guard re-fit on ΔΞ.constellation_violation_rate high (REGULATE step branch). Shipped (deferred).</description>
    <impact>Without it, the REGULATE step cannot re-fit centroids; falls back to surfacing blocker. Soft dependency.</impact>
  </dependency>

  <dependency type="internal">
    <name>context-graph-mejepa-instruments</name>
    <description>Frozen instrument substrate; their byte-equal stability is the precondition for ΔΞ.target_collapse=0.</description>
    <impact>Hard dependency on Phase 1 ship.</impact>
  </dependency>

  <dependency type="external">
    <name>candle-cuda + RTX 5090</name>
    <description>GPU substrate for predictor + LoRA + distillation. Per constitution `<tech_stack>`.</description>
    <impact>Without it, retrain wallclock budget is unmeetable. Hard dependency.</impact>
  </dependency>

  <dependency type="external">
    <name>RocksDB</name>
    <description>Storage for CF_MEJEPA_FISHER_SNAPSHOTS, CF_MEJEPA_DRIFT_HISTORY, CF_MEJEPA_HEAL_REPORTS, CF_MEJEPA_WEIGHT_BLOBS, etc.</description>
    <impact>Hard dependency. Quota enforcement per REQ-HEAL-19.</impact>
  </dependency>

</dependencies>

<out_of_scope>
  <item>Active learning loop (§3.7.4): when conformal set size &gt; 3 AND OOD score &lt; 0.5, surface for operator label or Docker oracle re-runs — handled by SPEC-INFERENCE + Phase 7 MCP tools, not Phase 5.</item>
  <item>Cross-task transfer (§3.7.5): patch-similarity graph upweights similar tasks during fine-tune — handled by SPEC-TRAINING / SPEC-CORPUS, not Phase 5.</item>
  <item>Phase 6 Garbage Collection: nightly demote / evict / compact / prune passes — handled by SPEC-HYGIENE.</item>
  <item>Phase 7 mejepa-shift-subscriber Tokio task body: this spec specifies the observe() callee contract; the subscriber lifecycle, watermark management, and shift-log tail logic live in SPEC-SHIFT-SUBSCRIBER.</item>
  <item>The MCP tool surface itself (server-side handlers, JSON-RPC plumbing): handled by SPEC-MCP-TOOLS (Phase 7).</item>
  <item>Constellation Gτ guard internals (the centroid construction algorithm, distance metrics): handled by SPEC-TCT. This spec specifies only the cross-cutting surface (re-fit on REGULATE).</item>
  <item>Inference-time conformal interval computation, OOD scoring formulas, seven-gate verify ladder: handled by SPEC-INFERENCE. This spec specifies only the heal-cycle interaction (status_change field, calibration recomputation cadence).</item>
  <item>UTML factor formulas (effective_plasticity / landscape_health / stability_floor / target_collapse / predictor_redundancy / constellation_violation_rate computation): handled by SPEC-UTML. This spec specifies only the heal-cycle assertions (regulate-state-first, &lt; 0.4 thresholds, pruning rule).</item>
</out_of_scope>

<test_plan>

  <test_case id="TC-HEAL-01" type="unit" req_ref="REQ-HEAL-01">
    <description>observe() with valid panel + oracle returns Ok(ObserveOutput) with non-zero theta_sha delta and all UTML factors finite</description>
    <preconditions>Loaded predictor checkpoint + initial EWC++ state</preconditions>
    <inputs>Real Panel built from a Phase 0 mutation corpus example + corresponding OracleOutcome</inputs>
    <expected_result>Ok(ObserveOutput); l_step ∈ [0, 1]; ΔΞ.target_collapse component == 0; weight delta != 0</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-HEAL-02" type="unit" req_ref="REQ-HEAL-01">
    <description>observe() with NaN-injected panel returns Err(BatchNan) without mutating weights</description>
    <preconditions>Loaded predictor checkpoint; capture pre-call weight SHA</preconditions>
    <inputs>Panel with NaN at slot E_AST</inputs>
    <expected_result>Err(ObserveError::BatchNan); post-call weight SHA == pre-call weight SHA (no mutation)</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-HEAL-03" type="integration" req_ref="REQ-HEAL-02">
    <description>EWC++ Fisher matrix is full-rank after 10 task boundaries</description>
    <preconditions>Cold-start EWC++; predictor with dim=D</preconditions>
    <inputs>Sequence of 100 observations spanning 10 detected boundaries</inputs>
    <expected_result>fisher_rank == D; per-snapshot fisher_rank trajectory captured in FSV ewc-fisher-evidence.json</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-HEAL-04" type="integration" req_ref="REQ-HEAL-03">
    <description>Drift detection: synthetic 70 % coverage detected within 1,000 observations</description>
    <preconditions>Mock oracle that mismatches predicted_set 30 % of the time deterministically</preconditions>
    <inputs>1,000 observations with the mock oracle</inputs>
    <expected_result>DriftSeverity::Hard reported between observation 700 and 1,000; transition logged in CF_MEJEPA_DRIFT_HISTORY</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-HEAL-05" type="e2e" req_ref="REQ-HEAL-04, REQ-HEAL-12">
    <description>End-to-end heal-drill: drift injection → A/B/C retrain → promotion → witness-chain entry</description>
    <preconditions>Phase 0 corpus + Phase 2 predictor + Phase 3 training loop all loaded</preconditions>
    <inputs>cargo run --release -p context-graph-mejepa -- heal-drill --inject-drift hard</inputs>
    <expected_result>drift_detected: hard after ~700 obs; mode_winner ∈ {B, C}; promotion_latency &lt; 60min; ModelPromote witness entry chains clean; FSV evidence files written</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-HEAL-06" type="integration" req_ref="REQ-HEAL-05">
    <description>Periodic full retrain triggers at observation count == 10,000 multiple</description>
    <preconditions>Observation counter at 9,995</preconditions>
    <inputs>5 additional observations</inputs>
    <expected_result>Full retrain spawned at observation 10,000; corpus snapshot taken; promote iff +0.02 floor cleared</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-HEAL-07" type="integration" req_ref="REQ-HEAL-06">
    <description>Continuous calibration runs every 1,000 observations and persists new τ + σ²</description>
    <preconditions>Observation counter at 999; full calibration window populated</preconditions>
    <inputs>5,000 additional observations</inputs>
    <expected_result>5 new calibration records persisted to CF_MEJEPA_CALIBRATION_HISTORY; each with `frozen_at` strictly increasing</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-HEAL-08" type="integration" req_ref="REQ-HEAL-07">
    <description>Witness-chain integrity check: corrupt one byte at offset i; assert IntegrityViolation { kind: BrokenAt(i) }</description>
    <preconditions>100,000-entry chain</preconditions>
    <inputs>Corrupt byte at offset 5,000 * 73 + 30; run integrity check</inputs>
    <expected_result>Err(IntegrityViolation { kind: BrokenAt(5_000) }); inference paused; blocker severity:critical written</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-HEAL-09" type="integration" req_ref="REQ-HEAL-08">
    <description>LoRA refresh on plasticity collapse: pre/post base embedder SHAs byte-equal</description>
    <preconditions>Force ΔΩ.effective_plasticity = 0.30 by injecting low-quality observations</preconditions>
    <inputs>REGULATE step</inputs>
    <expected_result>refresh_lora_adapter called; base_embedder_sha unchanged; lora_adapter_sha changed; post-refresh ΔΩ.effective_plasticity &gt; 0.4</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-HEAL-10" type="integration" req_ref="REQ-HEAL-09">
    <description>Distillation-based online observe with EMA τ=0.996</description>
    <preconditions>Recent observations with signal_clarity range [0.2, 0.95]</preconditions>
    <inputs>Distillation loop on the batch</inputs>
    <expected_result>Examples with signal_clarity &lt; 0.5 skipped; EMA update form `w ← τ·w + (1−τ)·w_grad` verified; student loss decreases</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-HEAL-11" type="unit" req_ref="REQ-HEAL-10">
    <description>UTML cycle: regulate-state-first ordering enforced when ΔΩ &lt; 0.4</description>
    <preconditions>Synthetic training cert window with ΔΩ.effective_plasticity = 0.30, ΔP = 0.1</preconditions>
    <inputs>One heal-cycle iteration</inputs>
    <expected_result>REGULATE fires before INTERVENE-on-content; LoRA refresh attempted before any curriculum change</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-HEAL-12" type="unit" req_ref="REQ-HEAL-10">
    <description>Pruning rule: all three (L_step, ΔΩ, ΔΞ) must agree</description>
    <preconditions>Synthetic curriculum item with L_step=0.05 for N steps but ΔΩ=0.30</preconditions>
    <inputs>CLASSIFY step considers pruning</inputs>
    <expected_result>Pruning REJECTED; classification routes to Optimizer-Dysregulated; REGULATE ΔΩ first</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-HEAL-13" type="integration" req_ref="REQ-HEAL-04">
    <description>A/B/C: when no mode beats A on all three gates, return HoldoutRegression</description>
    <preconditions>Mode B and C retrains result in lower coverage than A on holdout</preconditions>
    <inputs>retrain_and_promote()</inputs>
    <expected_result>Err(HealError::HoldoutRegression); Mode A retained; blocker severity:medium written</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-HEAL-14" type="integration" req_ref="REQ-HEAL-04, EC-HEAL-13">
    <description>Crash-recovery: kill mid-promotion after witness-chain entry but before active-weight pointer update; assert recovery on restart</description>
    <preconditions>Mid-promotion state</preconditions>
    <inputs>SIGKILL after ModelPromote entry written; restart</inputs>
    <expected_result>Recovery routine reads latest ModelPromote; loads weights_sha from CF_MEJEPA_WEIGHT_BLOBS; activates; recovery journal entry written</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-HEAL-15" type="integration" req_ref="REQ-HEAL-17">
    <description>MCP heal_status tool returns current state</description>
    <preconditions>Active heal cycle with known drift_severity, observation_counter, etc.</preconditions>
    <inputs>mcp__cgreality__mejepa_heal_status invocation</inputs>
    <expected_result>Schema-compliant HealStatus JSON; round-trip equivalent with direct in-process call</expected_result>
    <priority>medium</priority>
  </test_case>

  <test_case id="TC-HEAL-16" type="manual" req_ref="REQ-HEAL-18">
    <description>Operator rollback to a prior promotion's weights_sha</description>
    <preconditions>2 prior promotions in chain; weights_sha both retained</preconditions>
    <inputs>mcp__cgreality__mejepa_rollback_to(target_offset)</inputs>
    <expected_result>Active-weight pointer reverted; new ModelRollback entry chains correctly; verify() with rolled-back weights matches original behavior</expected_result>
    <priority>medium</priority>
  </test_case>

</test_plan>

<open_questions>

  <question id="Q-HEAL-01" status="open" assignee="operator">
    <text>What is the canonical sona-style task-boundary detection algorithm? Doc 09 §5.1 says "per sona's automatic detection" but the clean-room reimplementation needs a concrete algorithm.</text>
    <context>Affects REQ-HEAL-02. Candidates: (a) loss-surface curvature shift detection via Hessian-trace estimator; (b) running-mean-of-loss vs all-time-mean divergence above a threshold; (c) explicit task_id changes carried in OracleOutcome.</context>
    <resolution></resolution>
  </question>

  <question id="Q-HEAL-02" status="open" assignee="operator">
    <text>Should the A/B/C promotion gate use the SAME calibration version for all three modes, or recalibrate per mode?</text>
    <context>Affects REQ-HEAL-04. Spec currently says "SAME calibration version" to ensure apples-to-apples comparison. Per-mode recalibration would let Mode B's stronger predictor justify a tighter τ — but introduces a confound.</context>
    <resolution></resolution>
  </question>

  <question id="Q-HEAL-03" status="open" assignee="operator">
    <text>The 3-window hysteresis for drift severity transitions (EC-HEAL-04) — is 3 windows the right anti-flapping constant, or should it be tunable?</text>
    <context>Affects EC-HEAL-04. Smaller hysteresis (1 window) means faster reaction but more flapping. Larger hysteresis (5 windows) means stable but slow response.</context>
    <resolution></resolution>
  </question>

  <question id="Q-HEAL-04" status="open" assignee="operator">
    <text>When ΔΞ.constellation_violation_rate is high, the REGULATE step says "re-fit constellation centroids on fresh deterministic-walk data." How frequently is "fresh" — and does this require operator approval given centroids are normally frozen by SEC-MEJEPA-04 (max-age 90 days) provenance?</text>
    <context>Affects REQ-HEAL-10 + cross-cutting with SPEC-TCT. Centroid re-fit is a non-trivial operation; doing it autonomously may violate the SEC-MEJEPA-04 spirit.</context>
    <resolution></resolution>
  </question>

  <question id="Q-HEAL-05" status="open" assignee="operator">
    <text>The doc 09 §5.4 N=10,000 full-retrain period — is this in observations or in `(panel, oracle_outcome)` pairs accepted by observe()? Synonyms in the spec; should clarify.</text>
    <context>Affects REQ-HEAL-05. Resolution: observations is the same as accepted observe() calls (signal_clarity gating may skip some). Spec uses "observations" consistently.</context>
    <resolution></resolution>
  </question>

</open_questions>

<glossary>
  <term name="EWC++">Elastic Weight Consolidation Plus Plus — Fisher-information-weighted L2 regularization that protects weights important to old tasks while allowing free movement on weights unimportant to old tasks. Per crates/sona/src/ewc.rs clean-room reimplementation. Inspired by Kirkpatrick et al. 2017 + Schwarz et al. 2018 online refinement.</term>
  <term name="A/B/C retrain">Three-mode parallel retrain: A=current production weights θ_A, B=retrain from corpus θ_B, C=ensemble of A+B θ_C. Promote winner only on three gates. Per doc 09 §5.3.</term>
  <term name="Drift severity">Mapping from empirical conformal coverage to {Healthy, Soft, Hard, Catastrophic}. Per doc 09 §5.2 table.</term>
  <term name="LoRA adapter refresh">Fine-tuning the rank=32 LoRA adapter (not the frozen base embedder) on a fresh corpus slice for 10 epochs when ΔΩ.effective_plasticity collapses. Per doc 12 §6 + §8.</term>
  <term name="Distillation EMA τ">Polyak-averaging coupling rate (default 0.996) for student-weight update during online distillation. The "future-self prior" of UTML §VIII. Per doc 12 §8.</term>
  <term name="UTML regulate-state-first cycle">Per doc 11 §9 + doc 10 §D: ASSESS ΔΩ + ΔΞ FIRST, then REGULATE if either &lt; 0.4, then OBSERVE/COMPUTE/CLASSIFY/INTERVENE/EVALUATE. Coaching order locked by constitution `<utml_v1_invariants>` rule 3.</term>
  <term name="Pruning rule">Per doc 11 §10: prune curriculum item iff (L_step &lt; 0.1 for N steps) AND (ΔΩ &gt; 0.4 throughout) AND (ΔΞ &gt; 0.4 throughout). All three required.</term>
  <term name="Continuous calibration">Sliding-window recomputation of conformal threshold τ + OOD bandwidth σ² every 1,000 observations on the most-recent 1,000 calibration examples. Per doc 09 §5.5.</term>
  <term name="signal_clarity">UTML ΔE-derived gating multiplier (∈ [0, 1]) that controls how strongly an observation contributes to observe(). Low signal_clarity = noisy label = small weight. Per doc 10 §D.</term>
  <term name="Witness-chain integrity check">End-to-end replay of the 73-byte SHAKE-256 chain every 10,000 observations or on drift. `chain.len() % 73 == 0` AND prev_hash chain valid. Break ⇒ halt + restore. Per doc 09 §5.6.</term>
  <term name="ModelPromote witness entry">73-byte SHAKE-256 entry recording (timestamp, op_type=ModelPromote, weights_sha, mode_winner, evaluation_summary_sha) on every promotion. Per doc 09 §5.3 + doc 10 §D.</term>
  <term name="Plasticity collapse">ΔΩ.effective_plasticity &lt; 0.4 — the predictor loses the ability to incorporate new gradients. Symptom of dormant ReLU units, low-rank weight drift. Per doc 11 §V.1.</term>
  <term name="Holdout regression">Retrain produces lower holdout score than current production (Mode A). Triggers MEJEPA_RETRAIN_HOLDOUT_REGRESSION; Mode A retained. Per doc 09 §5.3 promotion gates.</term>
</glossary>

</functional_spec>
```

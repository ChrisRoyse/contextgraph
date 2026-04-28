# 04 — Consolidation `M(t)` and State-Transfer `T(train, deploy)`

**Purpose.** UTL v2 introduces two terms that capture *durability* (M) and *deployability* (T) of learning, distinct from per-moment learning efficiency L. Both are time-dependent and require persistent storage; both have direct embedder-based readouts; both have established empirical foundations (sleep-gated consolidation; state-dependent retrieval).

This file specifies how to compute M and T from session logs, how to wire them into Chris's audit log column families, and how spaced-repetition / retrieval-practice schedules drop out of the framework.

---

## 1. The consolidation integral M(t)

### 1.1 UTL v2 form (flat)

```
M(t) = ∫₀ᵗ L(τ) · ΔE(τ) · k(τ) · e^{-(t-τ)/τ_decay(τ)} dτ
```

Where `k(τ)` is sleep-dependent (≈1.0 awake; ≈3–5× during slow-wave sleep within first 24 h post-encoding) and `τ_decay(τ)` is short for unconsolidated traces (hours) and long once integrated (months to years).

### 1.2 Differentiated form (recommended for v2.1, see file 01)

```
M(t) = ∫₀ᵗ ΔS_eff(τ) · ΔC_eff(τ) · stress_floor(τ) · k_state(τ) · e^{-(t-τ)/τ_decay(τ)} dτ

where:
  ΔS_eff   = ΔS · stress_floor
  ΔC_eff   = ΔC · HRV_coherence^0.5
  k_state  = plasticity_window^0.5 · k_sleep
```

This factors the equation so each ΔE subcomponent gates the right stage. Cortisol gates encoding sensitivity; HRV gates integration; plasticity-window gates consolidation rate.

### 1.3 Discrete computation (the form you'd actually run)

In practice, time is discretised at the session level (or the per-trial level). For a trial sequence indexed by τ:

```
M_discrete(t) = Σ_{τ ≤ t} L(τ) · ΔE(τ) · k(τ) · exp(-(t-τ) · η_decay(τ))
```

Where `η_decay(τ) = 1 / τ_decay(τ)` is the per-step decay rate. Two regimes:

- **Unconsolidated (τ since encoding < 24 h, no sleep yet).** η_decay = 1/3600 per second (i.e., the trace half-lives in roughly 1 hour). This is the period where retrieval practice is most valuable for cementing.
- **Consolidated (post-sleep, especially post-slow-wave).** η_decay drops to 1/(7 · 86400) per second (week-scale half-life) for a single retrieval; further retrievals push half-life to month-scale.

The transition is gated by `k(τ)`: when the learner sleeps within 24 h of encoding, multiply that interval's contribution to the integral by 2–4 to reflect the consolidation boost.

### 1.4 Sleep detection

The simplest k(τ) signal is wearable-sleep-stage data, which Apple Watch / Oura / WHOOP / Fitbit all expose via API. PaPaGei's PPG embedding can also infer sleep stage directly (Multimodal Sleep Foundation Model with 500K hours of sleep recordings is now available — the 2024–2025 work on cross-modal physiological foundation models includes sleep as a downstream task).

Operational rule: for each L(τ) contribution, look up the time elapsed to the next slow-wave sleep window. If it is within 24 h, multiply by k = 3.0 (mid-range of the 2–4 literature). If sleep is delayed beyond 24 h, k = 1.0 (no consolidation boost; the trace decays at the unconsolidated rate). If a daytime nap with NREM2+ occurs, partial credit (k = 1.5).

### 1.5 The retrieval-practice multiplier

Roediger & Karpicke (2006) — testing-effect d ≈ 0.5–0.8 — establishes that retrieval is a more effective consolidator than re-exposure. When a previously encoded item is retrieved correctly during a new session, the trace's `τ_decay` increases (memory becomes more durable). Operationally:

```
on retrieval:
  if correct: τ_decay *= 2.0          # double the half-life
  if incorrect: τ_decay *= 0.7        # truncate the half-life — re-encoding is needed
```

This produces the SuperMemo / Anki / FSRS pattern naturally: items that are retrieved correctly get longer intervals; items that fail get shorter ones. The difference is that UTL's parameters are derived from ΔE-modulated dynamics, not from a hard-coded scheduling formula.

### 1.6 Comparison to FSRS / SuperMemo / SM-20

Anki's v3 scheduler is the default and only scheduler path as of Anki/AnkiMobile 23.10 and AnkiDroid 2.17. Within that scheduler, **FSRS** (Free Spaced Repetition Scheduler) is an integrated alternative to the legacy SuperMemo-2-derived algorithm, not simply the default setting. FSRS uses review history to fit per-user memory parameters; current Anki docs describe it as an alternative that can remember more material in the same study time by estimating forgetting more accurately. SM-20 (the 2026 SuperMemo) uses a more elaborate model with per-item complexity terms. Both are reduced forms of the M(t) integral above, with state ΔE held constant (FSRS doesn't model embodiment).

**Implication.** FSRS is what UTL v2's M(t) becomes when you fix ΔE = 1 (assume the learner is always regulated). UTL v2 is the strict generalisation that says: *a session's contribution to M depends on the learner's state during that session*, and a session in low-ΔE state should not extend the next-review interval the way a high-ΔE session does. This produces a **state-aware FSRS** that we can verify against actual long-term retention.

This is a publishable result: an FSRS variant that incorporates session-time HRV / EEG state data should outperform vanilla FSRS on retention prediction. File 08 (falsification experiments) lists this as Experiment 4.

### 1.7 The Dissipating state (UTL v2's diagnostic)

Defined as: `mean(L over last 5 steps) > 0.3` AND `ΔM(24h) / mean(L) < 0.2`.

This is a per-learner monitor that we can run nightly: compute M's gain over the previous 24 h, divide by the mean L during sessions of that 24 h. If the ratio is below 0.2, the learner is dissipating — high engagement, poor retention. Cause is ΔE-low: chronic stress, sleep debt, post-prandial dip, anxiety.

The intervention is structural: improve sleep, reduce stress, schedule learning to high-ΔE windows. This is exactly the corporate-L&D failure mode UTL v2 Section 11.3 names.

---

## 2. The state-transfer term T(train, deploy)

### 2.1 UTL v2 form

```
T(train, deploy) = exp(-d(state_train, state_deploy) / λ)
```

Where `d` is a distance over the joint state vector (cortisol_z, plasticity_window, valence_arousal, contextual_cues) and λ ≈ 1.5 is a transfer-bandwidth constant (tighter for high-stakes motor skills, looser for declarative knowledge).

`performance_deploy = M(t) · T(train, deploy)`.

### 2.2 The state vector

Operationally, the joint state vector is the concatenation of every embedder's output during a session:

```
state = concat(
  φ_E14(speech),               # 1024-D (Wav2Vec2-MSP-DIM)
  φ_E15(face),                 # 512-D (OpenFace)
  φ_E17(PPG),                  # 512-D (PaPaGei)
  φ_E18(EDA),                  # 64-D (WESAD CNN penult.)
  φ_E19(EEG),                  # 768-D (LaBraM)
  contextual_cues             # session metadata: time, location, device, etc.
)
```

Total dimensionality ~2900-D + metadata. d is computed as L2 distance after per-dimension normalisation against the learner's lifetime distribution.

### 2.3 λ calibration

UTL v2 says λ ≈ 1.5 default, tighter for high-stakes motor skills, looser for declarative. Calibrate empirically per skill: λ should be set such that for the same learner training and deploying in *similar* states (within the inter-session variance of normal life), T ≥ 0.7. For deploying in *different* states (training calm, deploying stressed), T drops to 0.3–0.5.

### 2.4 The "variability of practice" prescription

Schmidt (1975) and Meichenbaum (1985) — UTL v2's training-design implication. To maximise deployed performance, train across a *distribution* of states close to deployment.

Operationally this means:

- Track each session's state vector in a new column family `CF_LEARNER_STATE_HISTORY`.
- For each learner-skill, compute the centroid of *deployment* states (e.g., the OR for a surgeon, the exam room for a student, the meeting context for a sales rep).
- Recommend training sessions in states whose distance to the deployment centroid is in the [0.3, 0.7] range — close enough that transfer is preserved, distant enough to add variability.

### 2.5 Stress-inoculation training operationalised

For deployment contexts that are characteristically high-stress (surgery, public speaking, exam), the training sequence should *progressively elevate* the state vector toward the deployment centroid. UTL v2 names this in Section 5.3; the embedder programme makes it actionable:

1. Initial sessions: low-stress training (high ΔE, easy environment).
2. Mid-curriculum: introduce mild stressors (timed sessions, public-feedback contexts) — push state vector toward deployment.
3. Late-curriculum: high-stress simulation (full-deployment conditions).
4. Deploy: T(train, deploy) is high because train_state has been steadily approaching deploy_state.

---

## 3. The deployed-performance prediction

```
performance_deploy(t) = M(t) · T(train_history, deploy_state)
```

This is a per-learner per-skill scalar that can be compared to actual deployed performance (a surgeon's complication rate, a student's exam score, a sales-rep's close rate). The system's prediction:

- High M, low T → "knows it but freezes" (matches the surgeon-in-OR pattern).
- Low M, high T → "calm but underprepared".
- High M, high T → expected success.
- Low M, low T → expected failure on both axes.

These four quadrants are diagnostic and prescriptive: each suggests a different intervention.

---

## 4. Storage layer in Context Graph

Add five column families to support M and T persistently:

| CF | Key | Value | Purpose |
|---|---|---|---|
| `CF_LEARNER_M_PER_TRACE` | `[learner_id:16][trace_id:16]` | `(M_value: f64, last_update_ts: u64, decay_rate: f32)` | Per-trace consolidation value, updated on retrieval |
| `CF_LEARNER_STATE_HISTORY` | `[learner_id:16][session_ts:u64]` | bincode `StateVector` | Full state vector per session |
| `CF_LEARNER_GOAL_STATES` | `[learner_id:16][skill_id:16]` | `StateVector` (deployment centroid) | What "deploy" looks like for this learner-skill pair |
| `CF_LEARNER_RETRIEVAL_LOG` | `[learner_id:16][trace_id:16][ts:u64]` | `(correct: bool, score: f32, state_at_retrieval: StateVector)` | Retrieval-practice events with state context |
| `CF_LEARNER_K_SLEEP` | `[learner_id:16][session_ts:u64]` | `(k: f32, slow_wave_minutes: u16)` | Sleep-derived consolidation multiplier |

Total: 5 new CFs. Combined with the 6 from file 02 (constellation, baseline, ΔE history, etc.), the Context Graph extension is +11 CFs (59 → 70).

---

## 5. The retrieval scheduler (UTL-derived spaced repetition)

Combine M and T into a per-trace next-review-time prediction:

```
def next_review_time(trace: TraceID, learner: LearnerID) -> datetime:
    M_t = current_M(trace, learner)
    decay = current_decay(trace, learner)
    target_M = 0.7  # threshold for "needs review"
    elapsed_to_threshold = log(M_t / target_M) / decay
    # Schedule review just before forecasted M crosses target
    return now() + elapsed_to_threshold * 0.9   # 10 % safety margin
```

This produces a Anki/FSRS-compatible per-card schedule, with the addition that consolidation rate `k_state` factored into M depends on session-time ΔE (so a card studied during a low-ΔE session gets reviewed sooner; a card studied during a high-ΔE session can wait longer).

The output is a JSON-RPC response from a new MCP tool `next_review_for_trace(learner_id, trace_id)`.

---

## 6. The connection to Steve's Anki-card protocol for the bar exam

Steve's audio describes a 10-week plan for his friend Chris George preparing for the Ohio bar, using Anki cards and UTL grounding. The operationalised version of this plan is:

- Each bar-exam concept becomes a "trace" with a TraceID.
- Phase 1 (centroid construction): build a goal centroid for each major topic using model-bar-exam-answer documents. Use Context Graph's existing topic-detection to cluster concepts.
- Phase 2 (study sessions): each session, log L(τ), ΔE(τ), and the state vector. Update M for every retrieved trace.
- Phase 3 (runtime guard): before each study session, check ΔE; if below threshold, do 5 minutes of paced 0.10-Hz breathing first (per UTL Part VIII).
- Phase 4 (state-transfer training): in the final 2 weeks, simulate exam-room conditions (timed, high-stakes, less ΔE-supported) so T(study_state, exam_state) is high on exam day.
- Phase 5 (review scheduler): the next-review-time formula above produces an Anki-compatible deck.

This is what Steve's plan looks like when the framework is mechanised.

---

## 7. The connection to AI training (preview of file 05)

For an AI system, M(t) becomes:

- Validation loss on a frozen evaluation set, which is monotonically decreasing during successful training.
- Sparse-autoencoder feature counts (Anthropic's monosemanticity work) — number of interpretable features rises with training.
- Linear probe accuracy on held-out tasks.
- Representational similarity (CKA, SVCCA) to a "regulated checkpoint" (a known-good model state we want to remain near).

T(train, deploy) for an AI system becomes:

- Distribution-shift distance between training mixture and deployment query distribution.
- Per-domain held-out accuracy on tasks specifically chosen to be far from training distribution.

These are developed in detail in file 05.

---

## 8. The minimum-viable scheduler

For Phase 0 (file 09, week 1–2) we don't need the full M(t) integral. The minimum viable scheduler is:

```
M_minimal(trace) = number_of_correct_retrievals - 0.3 · number_of_incorrect_retrievals
next_review_time = last_review + 2^M_minimal · base_interval
```

Plus: gate every encoding session on `ΔE > 0.4` (UTL v2's threshold). If state is low, queue the session for later rather than encoding into a low-quality trace.

This is implementable in a weekend on top of Context Graph's existing audit log. The full M(t) integral is the Phase 2 deliverable.

---

## Sources

- [Diekelmann & Born (2010) The memory function of sleep](https://www.nature.com/articles/nrn2762) — sleep-gated consolidation
- [Walker (2009) The role of sleep in cognition and emotion](https://nyaspubs.onlinelibrary.wiley.com/doi/abs/10.1111/j.1749-6632.2009.04416.x)
- [Roediger & Karpicke (2006) Test-enhanced learning](https://journals.sagepub.com/doi/10.1111/j.1467-9280.2006.01693.x) — retrieval > re-reading, d ≈ 0.5–0.8
- [Cousins et al. (2023) Targeted memory reactivation during sleep](https://pubmed.ncbi.nlm.nih.gov/) — 10–25 % consolidation boost
- [Godden & Baddeley (1975) Context-dependent memory](https://onlinelibrary.wiley.com/doi/10.1111/j.2044-8295.1975.tb01468.x) — state-dependent retrieval foundation
- [Schmidt (1975) Schema theory of motor learning](https://psycnet.apa.org/record/1976-10135-001) — variability of practice
- [Meichenbaum (1985) Stress Inoculation Training](https://psycnet.apa.org/record/1985-99020-000)
- [Anki FAQs — FSRS](https://faqs.ankiweb.net/what-spaced-repetition-algorithm)
- [Anki Manual — Deck Options / FSRS](https://docs.ankiweb.net/deck-options)
- [Anki FAQs — v3 Scheduler](https://faqs.ankiweb.net/the-2021-scheduler.html)
- [Spaced Repetition Systems Have Gotten Way Better — Domenic Denicola on FSRS](https://domenic.me/fsrs/)
- [DRL-SRS: A Deep Reinforcement Learning Approach for Optimizing Spaced Repetition Scheduling](https://www.mdpi.com/2076-3417/14/13/5591)
- [LECTOR: LLM-Enhanced Concept-based Test-Oriented Repetition for Adaptive Spaced Learning](https://arxiv.org/html/2508.03275v1)
- [Mander, Winer & Walker (2017) Sleep and human aging](https://www.sciencedirect.com/science/article/pii/S0896627317301216) — sleep-stage changes with age
- [PaPaGei multimodal sleep foundation model — see file 02](https://github.com/nokia-bell-labs/papagei-foundation-model)
